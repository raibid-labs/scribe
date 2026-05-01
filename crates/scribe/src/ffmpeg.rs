//! Subprocess wrappers around `ffmpeg` and `ffprobe`.
//!
//! Two operations:
//!
//! - [`probe_duration_sec`] runs `ffprobe` to get the audio/video duration
//!   in seconds, used to populate the `duration_sec:` frontmatter field.
//! - [`resample_to_wav`] runs `ffmpeg` to resample the input to the
//!   16 kHz mono PCM s16le WAV that whisper.cpp expects.
//!
//! Both commands write to caller-supplied paths so the orchestrator owns
//! tempfile lifetime (and `--keep-temp` semantics).

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use tracing::debug;

use crate::which::which_on_path;

/// Run `ffprobe -v error -show_entries format=duration -of
/// default=nw=1:nokey=1 <input>` and parse the result as seconds.
///
/// Returns an error if `ffprobe` is not on `PATH`, fails to spawn,
/// exits non-zero, or prints something we can't parse as `f64`.
pub fn probe_duration_sec(input: &Path) -> Result<f64> {
    let bin = which_on_path("ffprobe")
        .ok_or_else(|| anyhow!("ffprobe not found on PATH (install ffmpeg)"))?;
    let output = Command::new(&bin)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=nw=1:nokey=1",
        ])
        .arg(input)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("failed to invoke ffprobe at {}", bin.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "ffprobe exited with status {}: {}",
            output.status,
            stderr.trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    trimmed
        .parse::<f64>()
        .with_context(|| format!("ffprobe returned unparseable duration: {trimmed:?}"))
}

/// Run `ffmpeg -y -i <input> -ar 16000 -ac 1 -c:a pcm_s16le <out>`.
///
/// `-y` forces overwrite — we expect `out` to be a freshly-created
/// tempfile that the caller already owns. ffmpeg's stderr is captured and
/// surfaced on failure so the user sees the actual decoder error rather
/// than a bare exit code.
pub fn resample_to_wav(input: &Path, out: &Path) -> Result<()> {
    let bin = which_on_path("ffmpeg")
        .ok_or_else(|| anyhow!("ffmpeg not found on PATH (see docs/06-troubleshooting.md)"))?;
    debug!(
        "ffmpeg: resampling {} → {} (16 kHz mono PCM s16le)",
        input.display(),
        out.display()
    );
    let output = Command::new(&bin)
        .arg("-y")
        .arg("-i")
        .arg(input)
        .args(["-ar", "16000", "-ac", "1", "-c:a", "pcm_s16le"])
        .arg(out)
        // ffmpeg is chatty on stderr even when it succeeds; suppress its
        // banner from the user's terminal but capture for error context.
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("failed to invoke ffmpeg at {}", bin.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "ffmpeg failed (status {}) while resampling {}:\n{}",
            output.status,
            input.display(),
            stderr.trim()
        ));
    }
    Ok(())
}
