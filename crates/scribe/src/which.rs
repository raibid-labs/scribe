//! Subprocess + filesystem lookup helpers.
//!
//! General-purpose wrappers around [`std::process::Command`] used by the
//! `doctor` subcommand to verify external dependencies. Designed to be reused
//! by the `transcribe` pipeline (issue #6) when it needs to locate
//! `ffmpeg` and `whisper-cli`.
//!
//! Nothing in this module mutates the host: no installs, no downloads, no
//! filesystem writes. Lookups only.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Result of probing an external binary for its identifying banner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeOutput {
    /// First non-empty line of stdout (or stderr if stdout was empty).
    /// Useful as a "version line" surrogate for tools that don't expose
    /// a real `--version` flag.
    pub first_line: String,
    /// Process exit code, or `None` if the process was killed by a signal.
    pub exit_code: Option<i32>,
}

/// Look up a binary on `PATH` the same way the shell would.
///
/// Returns the resolved absolute path on success, or `None` if the binary is
/// not found. This is a minimal reimplementation of the `which` crate so we
/// don't pull in a dependency just for this.
pub fn which_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Resolve a binary path using an environment variable override, falling back
/// to a default path if the env var is unset or empty.
///
/// The returned path is *not* validated — callers should follow up with
/// [`is_executable_file`] (or run the binary) to confirm it actually works.
///
/// Currently unused by `doctor` (which needs to combine env-var lookup with
/// `dirs::home_dir()` and so handles its own resolution), but kept here for
/// the `transcribe` pipeline (issue #6), which will resolve absolute paths
/// to ffmpeg / whisper-cli the same way.
#[allow(dead_code)]
pub fn env_or_default<P: Into<PathBuf>>(env_var: &str, default: P) -> PathBuf {
    match std::env::var_os(env_var) {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => default.into(),
    }
}

/// Return true if `path` exists, is a regular file, and is marked executable
/// for the current user (or any executable bit on Unix).
pub fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Run a binary with the given args and capture its first line of output.
///
/// Combines stdout and stderr (preferring stdout) so we can identify tools
/// that print their banner to either stream. Returns `Err` only if the
/// process could not be spawned at all.
pub fn probe<I, S>(bin: &Path, args: I) -> std::io::Result<ProbeOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(bin).args(args).output()?;
    let first_line = first_nonempty_line(&output.stdout)
        .or_else(|| first_nonempty_line(&output.stderr))
        .unwrap_or_default();
    Ok(ProbeOutput {
        first_line,
        exit_code: output.status.code(),
    })
}

fn first_nonempty_line(buf: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(buf);
    text.lines()
        .map(str::trim_end)
        .find(|l| !l.is_empty())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_or_default_prefers_env() {
        // SAFETY: tests are single-threaded for env access by default; if we
        // ever flip to multi-threaded test runners, swap this for a serial_test.
        let key = "SCRIBE_TEST_ENV_OR_DEFAULT_PREFERS_ENV";
        std::env::set_var(key, "/tmp/from-env");
        let p = env_or_default(key, "/tmp/default");
        std::env::remove_var(key);
        assert_eq!(p, PathBuf::from("/tmp/from-env"));
    }

    #[test]
    fn env_or_default_falls_back_when_unset() {
        let key = "SCRIBE_TEST_ENV_OR_DEFAULT_FALLS_BACK_WHEN_UNSET";
        std::env::remove_var(key);
        let p = env_or_default(key, "/tmp/default");
        assert_eq!(p, PathBuf::from("/tmp/default"));
    }

    #[test]
    fn env_or_default_falls_back_when_empty() {
        let key = "SCRIBE_TEST_ENV_OR_DEFAULT_FALLS_BACK_WHEN_EMPTY";
        std::env::set_var(key, "");
        let p = env_or_default(key, "/tmp/default");
        std::env::remove_var(key);
        assert_eq!(p, PathBuf::from("/tmp/default"));
    }

    #[test]
    fn first_nonempty_line_skips_blanks() {
        let s = first_nonempty_line(b"\n\n  \nhello\nworld\n").unwrap();
        assert_eq!(s, "hello");
    }

    #[test]
    fn first_nonempty_line_handles_empty_input() {
        assert!(first_nonempty_line(b"").is_none());
        assert!(first_nonempty_line(b"\n\n").is_none());
    }

    #[test]
    fn is_executable_file_rejects_missing() {
        assert!(!is_executable_file(Path::new(
            "/definitely/does/not/exist/scribe-test"
        )));
    }

    #[test]
    fn is_executable_file_detects_real_binary() {
        // /bin/sh exists and is executable on every supported platform.
        assert!(is_executable_file(Path::new("/bin/sh")));
    }

    #[test]
    fn which_on_path_finds_sh() {
        // `sh` should always be resolvable on a POSIX host.
        let p = which_on_path("sh").expect("sh must be on PATH");
        assert!(p.is_absolute());
        assert!(is_executable_file(&p));
    }

    #[test]
    fn which_on_path_returns_none_for_garbage() {
        assert!(which_on_path("scribe-definitely-not-a-real-binary-xyz").is_none());
    }

    #[test]
    fn probe_captures_first_line_of_sh() {
        let sh = which_on_path("sh").expect("sh must be on PATH");
        let out = probe(&sh, ["-c", "echo first; echo second"]).unwrap();
        assert_eq!(out.first_line, "first");
        assert_eq!(out.exit_code, Some(0));
    }
}
