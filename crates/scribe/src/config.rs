//! Resolution of `scribe transcribe` runtime configuration.
//!
//! Combines CLI flags with environment variables and built-in defaults,
//! producing a [`Config`] that the rest of the pipeline consumes without
//! re-reading the environment. Flag values always win over env vars.
//!
//! Env vars honored (also documented in `docs/04-cli-reference.md`):
//!
//! - `SCRIBE_MODEL` — default model name when `--model` is not passed.
//! - `SCRIBE_MODEL_PATH` — directory to resolve model names against.
//! - `SCRIBE_WHISPER_BIN` — path to the `whisper-cli` binary.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::doctor::{resolve_model_dir, DEFAULT_WHISPER_BIN_REL, ENV_MODEL_PATH, ENV_WHISPER_BIN};
use crate::which::{env_or_default, is_executable_file};

/// Env var holding the default model *name* (e.g. `large-v3`).
pub const ENV_MODEL: &str = "SCRIBE_MODEL";

/// Built-in default model name when neither `--model` nor `SCRIBE_MODEL`
/// is set. Picked because it's the default model the voice-stuff justfile
/// fetches and what `scribe doctor` is likely to find on this host.
pub const DEFAULT_MODEL_NAME: &str = "base.en";

/// Fully-resolved configuration for one `scribe transcribe` invocation.
///
/// All paths in here are absolute and have been validated to the extent
/// possible without spawning subprocesses (binary executable, model file
/// exists). The pipeline trusts these unconditionally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Absolute path to the input audio/video file.
    pub input: PathBuf,
    /// Absolute path to the output directory (created if missing).
    pub out_dir: PathBuf,
    /// Resolved logical model name (e.g. `base.en`, `large-v3`).
    pub model_name: String,
    /// Absolute path to the model `.bin` file, resolved against
    /// `$SCRIBE_MODEL_PATH` (or its default).
    pub model_path: PathBuf,
    /// Absolute path to the `whisper-cli` binary.
    pub whisper_bin: PathBuf,
    /// If true, the intermediate 16 kHz WAV is preserved for debugging.
    pub keep_temp: bool,
}

/// Inputs sourced from CLI flags, mirrored from `TranscribeArgs` in `main.rs`.
///
/// Kept as a separate struct so `Config::resolve` is easy to unit-test
/// without dragging clap-derived types into the test harness.
#[derive(Debug, Clone)]
pub struct CliInputs {
    pub input: PathBuf,
    pub out_dir: PathBuf,
    pub model: Option<String>,
    pub keep_temp: bool,
}

impl Config {
    /// Resolve a complete [`Config`] from CLI flags + the process environment.
    ///
    /// Side effects: canonicalizes `--input` (must already exist), creates
    /// `--out-dir` if missing then canonicalizes it, resolves the model file
    /// against `$SCRIBE_MODEL_PATH`, and validates that the whisper-cli
    /// binary exists and is executable.
    pub fn resolve(cli: CliInputs) -> Result<Self> {
        // --input must exist; canonicalize so the source frontmatter field
        // is an absolute path that survives `cd` from the consumer.
        let input = cli
            .input
            .canonicalize()
            .with_context(|| format!("input file not found: {}", cli.input.display()))?;

        // --out-dir is created if missing (mkdir -p semantics), then canonicalized.
        std::fs::create_dir_all(&cli.out_dir)
            .with_context(|| format!("failed to create out-dir: {}", cli.out_dir.display()))?;
        let out_dir = cli
            .out_dir
            .canonicalize()
            .with_context(|| format!("failed to resolve out-dir: {}", cli.out_dir.display()))?;

        // Model name precedence: --model > $SCRIBE_MODEL > DEFAULT_MODEL_NAME.
        let model_name = match cli.model {
            Some(n) if !n.trim().is_empty() => n,
            _ => env_var_nonempty(ENV_MODEL).unwrap_or_else(|| DEFAULT_MODEL_NAME.to_string()),
        };

        // Resolve the .bin file under $SCRIBE_MODEL_PATH (or its default).
        let model_dir = resolve_model_dir().ok_or_else(|| {
            anyhow!(
                "cannot resolve model directory (no ${} and no $HOME)",
                ENV_MODEL_PATH
            )
        })?;
        let model_path = resolve_model_path(&model_dir, &model_name).with_context(|| {
            format!(
                "model `{model_name}` not found in {} (looked for ggml-{model_name}.bin and {model_name}.bin)",
                model_dir.display(),
            )
        })?;

        // whisper-cli: $SCRIBE_WHISPER_BIN > default. Use env_or_default
        // (now load-bearing — drops the dead-code suppression in which.rs).
        let whisper_bin = match dirs::home_dir() {
            Some(home) => env_or_default(ENV_WHISPER_BIN, home.join(DEFAULT_WHISPER_BIN_REL)),
            None => PathBuf::from(env_var_nonempty(ENV_WHISPER_BIN).ok_or_else(|| {
                anyhow!("cannot resolve whisper-cli (no ${ENV_WHISPER_BIN} and no $HOME)")
            })?),
        };
        if !is_executable_file(&whisper_bin) {
            return Err(anyhow!(
                "whisper-cli not found or not executable at {} (set ${} or build whisper.cpp — see docs/06-troubleshooting.md)",
                whisper_bin.display(),
                ENV_WHISPER_BIN
            ));
        }

        Ok(Self {
            input,
            out_dir,
            model_name,
            model_path,
            whisper_bin,
            keep_temp: cli.keep_temp,
        })
    }
}

/// Look up a model `.bin` by logical name in `dir`.
///
/// Tries `<dir>/ggml-<name>.bin` first (matches the voice-stuff layout),
/// then `<dir>/<name>.bin` for users who name their files explicitly.
/// Returns `None` if neither exists or `dir` is not a directory.
pub fn resolve_model_path(dir: &Path, name: &str) -> Option<PathBuf> {
    let ggml_form = dir.join(format!("ggml-{name}.bin"));
    if ggml_form.is_file() {
        return Some(ggml_form);
    }
    let plain_form = dir.join(format!("{name}.bin"));
    if plain_form.is_file() {
        return Some(plain_form);
    }
    // Also accept the case where the user passed a name that already has
    // the extension (e.g. `--model ggml-base.en.bin`).
    let literal = dir.join(name);
    if literal.is_file() {
        return Some(literal);
    }
    None
}

fn env_var_nonempty(key: &str) -> Option<String> {
    match std::env::var(key) {
        Ok(v) if !v.is_empty() => Some(v),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::which::ENV_TEST_LOCK as ENV_LOCK;

    /// Set/unset one env var around a closure, restoring prior state on exit.
    /// Caller must hold [`ENV_LOCK`] before calling — these tests touch
    /// process-global state that other modules' tests also touch.
    fn with_env<R>(key: &str, value: Option<&str>, f: impl FnOnce() -> R) -> R {
        let prev = std::env::var_os(key);
        match value {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
        let r = f();
        match prev {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
        r
    }

    #[test]
    fn resolve_model_path_prefers_ggml_form() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("ggml-base.en.bin"), b"x").unwrap();
        std::fs::write(tmp.path().join("base.en.bin"), b"x").unwrap();
        let p = resolve_model_path(tmp.path(), "base.en").unwrap();
        assert_eq!(p.file_name().unwrap(), "ggml-base.en.bin");
    }

    #[test]
    fn resolve_model_path_falls_back_to_plain_form() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("base.en.bin"), b"x").unwrap();
        let p = resolve_model_path(tmp.path(), "base.en").unwrap();
        assert_eq!(p.file_name().unwrap(), "base.en.bin");
    }

    #[test]
    fn resolve_model_path_accepts_full_filename() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("custom-name.bin"), b"x").unwrap();
        let p = resolve_model_path(tmp.path(), "custom-name.bin").unwrap();
        assert_eq!(p.file_name().unwrap(), "custom-name.bin");
    }

    #[test]
    fn resolve_model_path_returns_none_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(resolve_model_path(tmp.path(), "nonexistent").is_none());
    }

    /// Build a fake whisper-cli binary inside `dir` and return its path.
    /// On Unix it gets `0o755` so [`is_executable_file`] accepts it; on
    /// other platforms its mere existence is enough.
    fn fake_whisper_bin(dir: &Path) -> PathBuf {
        let path = dir.join("whisper-cli");
        std::fs::write(&path, b"#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    #[test]
    fn resolve_uses_flag_over_env_for_model_name() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let model_dir = tempfile::tempdir().unwrap();
        std::fs::write(model_dir.path().join("ggml-flag-wins.bin"), b"x").unwrap();
        std::fs::write(model_dir.path().join("ggml-env-loses.bin"), b"x").unwrap();

        let input = tempfile::NamedTempFile::new().unwrap();
        let whisper_dir = tempfile::tempdir().unwrap();
        let whisper_bin = fake_whisper_bin(whisper_dir.path());
        let out_dir = tempfile::tempdir().unwrap();

        let cfg = with_env(ENV_MODEL, Some("env-loses"), || {
            with_env(
                ENV_MODEL_PATH,
                Some(model_dir.path().to_str().unwrap()),
                || {
                    with_env(ENV_WHISPER_BIN, Some(whisper_bin.to_str().unwrap()), || {
                        Config::resolve(CliInputs {
                            input: input.path().to_path_buf(),
                            out_dir: out_dir.path().to_path_buf(),
                            model: Some("flag-wins".to_string()),
                            keep_temp: false,
                        })
                    })
                },
            )
        })
        .expect("config should resolve");

        assert_eq!(cfg.model_name, "flag-wins");
        assert_eq!(cfg.model_path.file_name().unwrap(), "ggml-flag-wins.bin");
    }

    #[test]
    fn resolve_uses_env_when_no_flag() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let model_dir = tempfile::tempdir().unwrap();
        std::fs::write(model_dir.path().join("ggml-env-name.bin"), b"x").unwrap();

        let input = tempfile::NamedTempFile::new().unwrap();
        let whisper_dir = tempfile::tempdir().unwrap();
        let whisper_bin = fake_whisper_bin(whisper_dir.path());
        let out_dir = tempfile::tempdir().unwrap();

        let cfg = with_env(ENV_MODEL, Some("env-name"), || {
            with_env(
                ENV_MODEL_PATH,
                Some(model_dir.path().to_str().unwrap()),
                || {
                    with_env(ENV_WHISPER_BIN, Some(whisper_bin.to_str().unwrap()), || {
                        Config::resolve(CliInputs {
                            input: input.path().to_path_buf(),
                            out_dir: out_dir.path().to_path_buf(),
                            model: None,
                            keep_temp: false,
                        })
                    })
                },
            )
        })
        .expect("config should resolve");

        assert_eq!(cfg.model_name, "env-name");
    }

    #[test]
    fn resolve_falls_back_to_default_model_name() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let model_dir = tempfile::tempdir().unwrap();
        let bin_name = format!("ggml-{DEFAULT_MODEL_NAME}.bin");
        std::fs::write(model_dir.path().join(&bin_name), b"x").unwrap();

        let input = tempfile::NamedTempFile::new().unwrap();
        let whisper_dir = tempfile::tempdir().unwrap();
        let whisper_bin = fake_whisper_bin(whisper_dir.path());
        let out_dir = tempfile::tempdir().unwrap();

        let cfg = with_env(ENV_MODEL, None, || {
            with_env(
                ENV_MODEL_PATH,
                Some(model_dir.path().to_str().unwrap()),
                || {
                    with_env(ENV_WHISPER_BIN, Some(whisper_bin.to_str().unwrap()), || {
                        Config::resolve(CliInputs {
                            input: input.path().to_path_buf(),
                            out_dir: out_dir.path().to_path_buf(),
                            model: None,
                            keep_temp: false,
                        })
                    })
                },
            )
        })
        .expect("config should resolve with default model");

        assert_eq!(cfg.model_name, DEFAULT_MODEL_NAME);
    }

    #[test]
    fn resolve_creates_missing_out_dir() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let model_dir = tempfile::tempdir().unwrap();
        let bin_name = format!("ggml-{DEFAULT_MODEL_NAME}.bin");
        std::fs::write(model_dir.path().join(&bin_name), b"x").unwrap();

        let input = tempfile::NamedTempFile::new().unwrap();
        let whisper_dir = tempfile::tempdir().unwrap();
        let whisper_bin = fake_whisper_bin(whisper_dir.path());

        let parent = tempfile::tempdir().unwrap();
        let out_dir = parent.path().join("nested/created/by/resolve");
        assert!(!out_dir.exists());

        let cfg = with_env(
            ENV_MODEL_PATH,
            Some(model_dir.path().to_str().unwrap()),
            || {
                with_env(ENV_WHISPER_BIN, Some(whisper_bin.to_str().unwrap()), || {
                    // Also clear ENV_MODEL so this test isn't sensitive to
                    // whatever the surrounding shell sets.
                    with_env(ENV_MODEL, None, || {
                        Config::resolve(CliInputs {
                            input: input.path().to_path_buf(),
                            out_dir: out_dir.clone(),
                            model: None,
                            keep_temp: false,
                        })
                    })
                })
            },
        )
        .expect("config should create missing out-dir");

        assert!(cfg.out_dir.is_dir());
    }

    #[test]
    fn resolve_errors_on_missing_input() {
        let cfg = Config::resolve(CliInputs {
            input: PathBuf::from("/definitely/does/not/exist/scribe-test.wav"),
            out_dir: PathBuf::from("/tmp"),
            model: None,
            keep_temp: false,
        });
        assert!(cfg.is_err());
    }

    // Reference doctor::resolve_whisper_bin from inside tests so we know
    // the env-var precedence path we depend on stays functional.
    #[test]
    fn doctor_resolve_whisper_bin_is_reachable() {
        let _ = crate::doctor::resolve_whisper_bin();
    }

    /// A whitespace-only `--model` flag must NOT be treated as a real
    /// model name (`"   "`); it falls through to the next precedence
    /// level. Pins down the `n.trim().is_empty()` branch in
    /// `Config::resolve` — the friendly UX where users can pass `--model
    /// ""` (or have a shell expansion produce blank) and still get the
    /// env-var or default behavior.
    #[test]
    fn resolve_treats_whitespace_only_model_flag_as_unset() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let model_dir = tempfile::tempdir().unwrap();
        std::fs::write(model_dir.path().join("ggml-from-env.bin"), b"x").unwrap();

        let input = tempfile::NamedTempFile::new().unwrap();
        let whisper_dir = tempfile::tempdir().unwrap();
        let whisper_bin = fake_whisper_bin(whisper_dir.path());
        let out_dir = tempfile::tempdir().unwrap();

        let cfg = with_env(ENV_MODEL, Some("from-env"), || {
            with_env(
                ENV_MODEL_PATH,
                Some(model_dir.path().to_str().unwrap()),
                || {
                    with_env(ENV_WHISPER_BIN, Some(whisper_bin.to_str().unwrap()), || {
                        Config::resolve(CliInputs {
                            input: input.path().to_path_buf(),
                            out_dir: out_dir.path().to_path_buf(),
                            model: Some("   ".to_string()),
                            keep_temp: false,
                        })
                    })
                },
            )
        })
        .expect("config should resolve, falling through whitespace flag to env");

        assert_eq!(
            cfg.model_name, "from-env",
            "whitespace-only --model should not shadow $SCRIBE_MODEL"
        );
    }

    /// `--keep-temp` is plumbed through unchanged. Trivial but worth
    /// pinning; the resolver could theoretically grow conditional logic
    /// around it (e.g., always-keep when `RUST_LOG=debug`) and we want
    /// the default behavior fixed.
    #[test]
    fn resolve_passes_keep_temp_through() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let model_dir = tempfile::tempdir().unwrap();
        let bin_name = format!("ggml-{DEFAULT_MODEL_NAME}.bin");
        std::fs::write(model_dir.path().join(&bin_name), b"x").unwrap();

        let input = tempfile::NamedTempFile::new().unwrap();
        let whisper_dir = tempfile::tempdir().unwrap();
        let whisper_bin = fake_whisper_bin(whisper_dir.path());
        let out_dir = tempfile::tempdir().unwrap();

        let cfg = with_env(ENV_MODEL, None, || {
            with_env(
                ENV_MODEL_PATH,
                Some(model_dir.path().to_str().unwrap()),
                || {
                    with_env(ENV_WHISPER_BIN, Some(whisper_bin.to_str().unwrap()), || {
                        Config::resolve(CliInputs {
                            input: input.path().to_path_buf(),
                            out_dir: out_dir.path().to_path_buf(),
                            model: None,
                            keep_temp: true,
                        })
                    })
                },
            )
        })
        .expect("config should resolve");

        assert!(cfg.keep_temp);
    }
}
