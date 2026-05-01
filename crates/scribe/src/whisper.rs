//! Subprocess wrapper around `whisper-cli` (whisper.cpp).
//!
//! The single entry point [`transcribe`] runs:
//!
//! ```text
//! <whisper-bin> -m <model> -f <wav> -of <out-prefix> -otxt -ojf
//! ```
//!
//! and on success returns the absolute paths of the two artifacts whisper
//! wrote: `<out-prefix>.txt` (verbatim text) and `<out-prefix>.json`
//! (segment-level JSON). The orchestrator is responsible for transforming
//! the `.txt` into the final `.md` and for the `.json` sidecar's final
//! placement (it's already where it needs to be).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use tracing::debug;

/// Paths to the artifacts whisper-cli wrote, given an `-of <prefix>` flag.
#[derive(Debug, Clone)]
pub struct WhisperArtifacts {
    pub txt: PathBuf,
    pub json: PathBuf,
}

/// Run whisper-cli end-to-end.
///
/// `out_prefix` is passed as the `-of` flag value; whisper appends
/// `.txt` and `.json` to it. The prefix should be `<out_dir>/<basename>`
/// so the artifacts land directly in the user's output directory.
///
/// stderr is captured and surfaced on failure. stdout is suppressed —
/// whisper.cpp prints the transcript to stdout by default *in addition*
/// to the `-of` files, and we don't want that noise in the user's
/// terminal.
pub fn transcribe(
    whisper_bin: &Path,
    model: &Path,
    wav: &Path,
    out_prefix: &Path,
) -> Result<WhisperArtifacts> {
    debug!(
        "whisper-cli: transcribing {} with model {} → {}.{{txt,json}}",
        wav.display(),
        model.display(),
        out_prefix.display()
    );
    let output = Command::new(whisper_bin)
        .arg("-m")
        .arg(model)
        .arg("-f")
        .arg(wav)
        .arg("-of")
        .arg(out_prefix)
        .args(["-otxt", "-ojf"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("failed to invoke whisper-cli at {}", whisper_bin.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "whisper-cli failed (status {}) on {}:\n{}",
            output.status,
            wav.display(),
            stderr.trim()
        ));
    }
    let txt = with_extension(out_prefix, "txt");
    let json = with_extension(out_prefix, "json");
    if !txt.is_file() {
        return Err(anyhow!(
            "whisper-cli did not produce expected text output at {}",
            txt.display()
        ));
    }
    if !json.is_file() {
        return Err(anyhow!(
            "whisper-cli did not produce expected JSON output at {}",
            json.display()
        ));
    }
    Ok(WhisperArtifacts { txt, json })
}

/// Append a literal extension to `prefix` without replacing whatever the
/// path already ends with. We can't use `Path::with_extension` because the
/// prefix already includes a timestamp (`...-20260428-143000`) that
/// `with_extension` would treat as the existing extension and clobber.
fn with_extension(prefix: &Path, ext: &str) -> PathBuf {
    let mut s = prefix.as_os_str().to_owned();
    s.push(".");
    s.push(ext);
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_extension_preserves_dotted_prefix() {
        let p = with_extension(Path::new("/tmp/my-podcast-ep-42-20260428-143000"), "txt");
        assert_eq!(
            p,
            PathBuf::from("/tmp/my-podcast-ep-42-20260428-143000.txt")
        );
    }

    #[test]
    fn with_extension_does_not_strip_dotted_segment() {
        // A naive `Path::with_extension` would turn this into `.txt`,
        // dropping the `143000` suffix. Confirm we don't.
        let p = with_extension(Path::new("/tmp/file.with.dots"), "txt");
        assert_eq!(p, PathBuf::from("/tmp/file.with.dots.txt"));
    }
}
