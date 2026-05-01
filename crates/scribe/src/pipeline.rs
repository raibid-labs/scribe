//! End-to-end orchestration of `scribe transcribe`.
//!
//! Composes [`crate::config`], [`crate::ffmpeg`], [`crate::whisper`], and
//! [`crate::output`] into the single function [`run`] that `main` calls.
//!
//! The order:
//!
//! 1. Resolve the [`Config`] from CLI flags + env.
//! 2. ffprobe for duration (so we have it for the frontmatter).
//! 3. ffmpeg resample to a temp 16 kHz mono PCM WAV.
//! 4. whisper-cli on that WAV, writing `.txt` + `.json` directly into
//!    `--out-dir`.
//! 5. Read the `.txt`, wrap in YAML frontmatter, write `.md`.
//! 6. Delete the `.txt` (markdown is canonical), drop the temp WAV
//!    unless `--keep-temp`.
//! 7. Print the final `.md` path on stdout.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use tempfile::Builder as TempBuilder;
use tracing::info;

use crate::config::{CliInputs, Config};
use crate::ffmpeg;
use crate::output::{self, FrontmatterMeta};
use crate::whisper;

/// Run the full pipeline for one input file. Returns the absolute path of
/// the markdown that was written.
pub fn run(cli: CliInputs) -> Result<PathBuf> {
    let cfg = Config::resolve(cli)?;
    info!(
        "transcribe: input={} model={} out_dir={}",
        cfg.input.display(),
        cfg.model_name,
        cfg.out_dir.display()
    );

    // Capture the timestamp once so the slug, the basename, and the
    // frontmatter `created` field all line up.
    let now = Utc::now();
    let stem = cfg
        .input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "untitled".to_string());
    let basename = output::basename(&stem, now);

    // 1. ffprobe for duration — non-fatal in the sense that a `.wav` whose
    //    container reports nothing useful shouldn't kill the whole run,
    //    but we surface the failure as a 0.0 with a warning rather than
    //    swallowing it silently. (Spec says read from ffprobe; we comply
    //    when ffprobe answers, log and continue otherwise.)
    let duration_sec = match ffmpeg::probe_duration_sec(&cfg.input) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(
                "ffprobe could not determine duration for {}: {:#} (continuing with 0.0)",
                cfg.input.display(),
                e
            );
            0.0
        }
    };

    // 2. ffmpeg → temp WAV. tempfile gives us automatic cleanup; if
    //    --keep-temp is set we call `into_temp_path().keep()` to detach
    //    it from the Drop guard.
    let temp_wav = TempBuilder::new()
        .prefix("scribe-")
        .suffix(".wav")
        .tempfile()
        .context("failed to create temp WAV file")?;
    let temp_wav_path = temp_wav.path().to_path_buf();
    ffmpeg::resample_to_wav(&cfg.input, &temp_wav_path)?;

    // 3. whisper-cli — writes `<out_dir>/<basename>.{txt,json}`.
    let out_prefix = cfg.out_dir.join(&basename);
    let artifacts = whisper::transcribe(
        &cfg.whisper_bin,
        &cfg.model_path,
        &temp_wav_path,
        &out_prefix,
    )?;

    // 4. Read whisper's verbatim text. Anything that mutates `body` here
    //    is a verbatim-contract bug — see docs/01-architecture.md.
    let body = fs::read_to_string(&artifacts.txt).with_context(|| {
        format!(
            "failed to read whisper text output at {}",
            artifacts.txt.display()
        )
    })?;

    // 5. Frontmatter + write markdown.
    let sidecar_filename = artifacts
        .json
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("{basename}.json"));
    let meta = FrontmatterMeta {
        source: &cfg.input,
        title: &stem,
        created: now,
        duration_sec,
        model: &cfg.model_name,
        sidecar_filename: &sidecar_filename,
    };
    let frontmatter = output::render_frontmatter(&meta);
    let md_path = output::write_markdown(&cfg.out_dir, &basename, &frontmatter, &body)?;

    // 6. Delete the .txt (markdown is canonical). The .json stays put.
    if let Err(e) = fs::remove_file(&artifacts.txt) {
        tracing::warn!(
            "failed to remove intermediate text file at {}: {}",
            artifacts.txt.display(),
            e
        );
    }

    // 7. Honor --keep-temp.
    if cfg.keep_temp {
        // Persist the tempfile beyond the Drop guard. We can't use
        // `into_temp_path().keep()` directly because we still hold
        // `temp_wav` by value; closing it that way is the right shape.
        let kept = temp_wav.into_temp_path();
        let kept_path = kept.to_path_buf();
        kept.keep()
            .with_context(|| format!("failed to retain temp WAV at {}", kept_path.display()))?;
        tracing::info!("--keep-temp: temp WAV preserved at {}", kept_path.display());
    }
    // Otherwise: `temp_wav` drops at end of scope and is unlinked.

    Ok(md_path)
}
