//! `scribe doctor` — verify that the external dependencies the transcribe
//! pipeline relies on are present.
//!
//! Checks performed (each emits a `tracing` line at info / warn / error):
//!
//! 1. `ffmpeg` — must be on `PATH`. Reports the first line of `ffmpeg -version`.
//! 2. `whisper-cli` — looked up via `$SCRIBE_WHISPER_BIN`, falling back to
//!    `~/raibid-labs/voice-stuff/third_party/whisper.cpp/build/bin/whisper-cli`.
//! 3. `model` — directory looked up via `$SCRIBE_MODEL_PATH`, falling back to
//!    `~/raibid-labs/voice-stuff/models/`. Lists every `*.bin` file found.
//! 4. `GPU` — `nvidia-smi --query-gpu=name --format=csv,noheader`. Non-fatal.
//!
//! Returns `Ok(true)` if every fatal check passes, `Ok(false)` if any of
//! ffmpeg / whisper-cli / model is missing. The caller (in `main.rs`) maps
//! that boolean to an exit code.
//!
//! Nothing in this module mutates the host. No installs, no model fetches.
//! Remediation is documented in `docs/06-troubleshooting.md`.

use std::path::{Path, PathBuf};

use anyhow::Result;
use tracing::{error, info, warn};

use crate::which::{is_executable_file, probe, which_on_path};

/// Env var holding an explicit path to the `whisper-cli` binary.
pub const ENV_WHISPER_BIN: &str = "SCRIBE_WHISPER_BIN";
/// Env var holding an explicit path to the model directory.
pub const ENV_MODEL_PATH: &str = "SCRIBE_MODEL_PATH";

/// Default location of the `whisper-cli` binary, relative to `$HOME`.
pub const DEFAULT_WHISPER_BIN_REL: &str =
    "raibid-labs/voice-stuff/third_party/whisper.cpp/build/bin/whisper-cli";
/// Default location of the model directory, relative to `$HOME`.
pub const DEFAULT_MODEL_DIR_REL: &str = "raibid-labs/voice-stuff/models";

/// Run every doctor check and return `Ok(true)` if the host is healthy enough
/// to transcribe. GPU absence is non-fatal.
pub fn run() -> Result<bool> {
    let ffmpeg_ok = check_ffmpeg();
    let whisper_ok = check_whisper_cli();
    let model_ok = check_model();
    let _gpu_ok = check_gpu(); // non-fatal

    let healthy = ffmpeg_ok && whisper_ok && model_ok;
    if healthy {
        info!("doctor: all required dependencies present");
    } else {
        warn!("doctor: one or more required dependencies missing — see lines above");
    }
    Ok(healthy)
}

/// Resolve the configured whisper-cli path: env var first, then default
/// under `$HOME`. Returns `None` if `$HOME` is unset *and* the env var is
/// also unset (extremely unusual).
pub fn resolve_whisper_bin() -> Option<PathBuf> {
    if let Some(p) = nonempty_env(ENV_WHISPER_BIN) {
        return Some(p);
    }
    Some(dirs::home_dir()?.join(DEFAULT_WHISPER_BIN_REL))
}

/// Resolve the configured model directory: env var first, then default
/// under `$HOME`. Returns `None` if `$HOME` is unset and the env var is
/// also unset.
pub fn resolve_model_dir() -> Option<PathBuf> {
    if let Some(p) = nonempty_env(ENV_MODEL_PATH) {
        return Some(p);
    }
    Some(dirs::home_dir()?.join(DEFAULT_MODEL_DIR_REL))
}

fn nonempty_env(key: &str) -> Option<PathBuf> {
    match std::env::var_os(key) {
        Some(v) if !v.is_empty() => Some(PathBuf::from(v)),
        _ => None,
    }
}

/// List every `*.bin` entry in `dir` that resolves to a regular file, sorted.
///
/// Symlinks are followed (`std::fs::metadata`, not `symlink_metadata`) so a
/// `models/ggml-base.en.bin` symlink into `whisper.cpp/models/` counts —
/// that's how voice-stuff lays its model dir out.
pub fn list_bin_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = read
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let is_bin = p.extension().and_then(|s| s.to_str()) == Some("bin");
            if !is_bin {
                return None;
            }
            // Follow symlinks: voice-stuff symlinks ggml-*.bin into ./models/.
            let resolves_to_file = std::fs::metadata(&p).map(|m| m.is_file()).unwrap_or(false);
            resolves_to_file.then_some(p)
        })
        .collect();
    out.sort();
    out
}

fn check_ffmpeg() -> bool {
    let Some(path) = which_on_path("ffmpeg") else {
        error!(
            "ffmpeg: MISSING on PATH (install via `apt install ffmpeg` or `brew install ffmpeg`)"
        );
        return false;
    };
    match probe(&path, ["-version"]) {
        Ok(out) => {
            info!("ffmpeg: {} ({})", path.display(), out.first_line);
            true
        }
        Err(e) => {
            error!(
                "ffmpeg: found at {} but failed to execute: {}",
                path.display(),
                e
            );
            false
        }
    }
}

fn check_whisper_cli() -> bool {
    let Some(path) = resolve_whisper_bin() else {
        error!(
            "whisper-cli: cannot resolve path (no ${} and no $HOME)",
            ENV_WHISPER_BIN
        );
        return false;
    };
    if !is_executable_file(&path) {
        error!(
            "whisper-cli: MISSING at {} (set ${} or build whisper.cpp — see docs/06-troubleshooting.md)",
            path.display(),
            ENV_WHISPER_BIN
        );
        return false;
    }
    // whisper-cli has no --version flag; --help prints the usage banner,
    // which is enough to confirm the binary is the one we expect and runs.
    match probe(&path, ["--help"]) {
        Ok(out) => {
            info!("whisper-cli: {} ({})", path.display(), out.first_line);
            true
        }
        Err(e) => {
            error!(
                "whisper-cli: found at {} but failed to execute: {}",
                path.display(),
                e
            );
            false
        }
    }
}

fn check_model() -> bool {
    let Some(dir) = resolve_model_dir() else {
        error!(
            "model: cannot resolve model dir (no ${} and no $HOME)",
            ENV_MODEL_PATH
        );
        return false;
    };
    if !dir.is_dir() {
        error!(
            "model: directory MISSING at {} (set ${} or fetch a model — see docs/06-troubleshooting.md)",
            dir.display(),
            ENV_MODEL_PATH
        );
        return false;
    }
    let bins = list_bin_files(&dir);
    if bins.is_empty() {
        error!(
            "model: no *.bin files in {} (run `just fetch-model base.en` in voice-stuff)",
            dir.display()
        );
        return false;
    }
    let names: Vec<String> = bins
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    info!(
        "model: {} ({} found: {})",
        dir.display(),
        bins.len(),
        names.join(", ")
    );
    true
}

fn check_gpu() -> bool {
    let Some(path) = which_on_path("nvidia-smi") else {
        info!("GPU: n/a (nvidia-smi not on PATH)");
        return false;
    };
    match probe(&path, ["--query-gpu=name", "--format=csv,noheader"]) {
        Ok(out) if out.exit_code == Some(0) && !out.first_line.is_empty() => {
            info!("GPU: {}", out.first_line);
            true
        }
        Ok(out) => {
            // nvidia-smi present but reported no devices or non-zero exit.
            info!(
                "GPU: n/a (nvidia-smi exit={:?}, output={:?})",
                out.exit_code, out.first_line
            );
            false
        }
        Err(e) => {
            info!("GPU: n/a (nvidia-smi failed: {})", e);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_whisper_bin_prefers_env() {
        let key = ENV_WHISPER_BIN;
        let prev = std::env::var_os(key);
        std::env::set_var(key, "/custom/whisper-cli");
        let p = resolve_whisper_bin().unwrap();
        // Restore *before* asserting, so a panic doesn't leak state.
        match prev {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
        assert_eq!(p, PathBuf::from("/custom/whisper-cli"));
    }

    #[test]
    fn resolve_whisper_bin_uses_default() {
        let key = ENV_WHISPER_BIN;
        let prev = std::env::var_os(key);
        std::env::remove_var(key);
        let p = resolve_whisper_bin();
        match prev {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
        let p = p.expect("HOME must be set in test env");
        assert!(p.ends_with(DEFAULT_WHISPER_BIN_REL));
        assert!(p.is_absolute());
    }

    #[test]
    fn resolve_model_dir_prefers_env() {
        let key = ENV_MODEL_PATH;
        let prev = std::env::var_os(key);
        std::env::set_var(key, "/custom/models");
        let p = resolve_model_dir().unwrap();
        match prev {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
        assert_eq!(p, PathBuf::from("/custom/models"));
    }

    #[test]
    fn resolve_model_dir_uses_default() {
        let key = ENV_MODEL_PATH;
        let prev = std::env::var_os(key);
        std::env::remove_var(key);
        let p = resolve_model_dir();
        match prev {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
        let p = p.expect("HOME must be set in test env");
        assert!(p.ends_with(DEFAULT_MODEL_DIR_REL));
        assert!(p.is_absolute());
    }

    #[test]
    fn list_bin_files_returns_empty_for_missing_dir() {
        let p = Path::new("/definitely/does/not/exist/scribe-doctor-test");
        assert!(list_bin_files(p).is_empty());
    }

    #[test]
    #[cfg(unix)]
    fn list_bin_files_follows_symlinks_to_regular_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        // Real .bin lives in a sibling dir; the model dir holds only a symlink.
        let real_dir = tempfile::tempdir().unwrap();
        let real_bin = real_dir.path().join("ggml-base.en.bin");
        std::fs::write(&real_bin, b"fake-model-bytes").unwrap();
        std::os::unix::fs::symlink(&real_bin, dir.join("ggml-base.en.bin")).unwrap();

        // Also a dangling symlink that should NOT count.
        std::os::unix::fs::symlink("/nonexistent/path.bin", dir.join("dangling.bin")).unwrap();

        let bins = list_bin_files(dir);
        let names: Vec<String> = bins
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["ggml-base.en.bin"]);
    }

    #[test]
    fn list_bin_files_filters_extensions_and_sorts() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        // Create a mix: two .bin, one .txt, one .gguf, plus a subdir.
        std::fs::write(dir.join("zeta.bin"), b"x").unwrap();
        std::fs::write(dir.join("alpha.bin"), b"x").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        std::fs::write(dir.join("model.gguf"), b"x").unwrap();
        std::fs::create_dir(dir.join("nested.bin")).unwrap();

        let bins = list_bin_files(dir);
        let names: Vec<String> = bins
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["alpha.bin", "zeta.bin"]);
    }
}
