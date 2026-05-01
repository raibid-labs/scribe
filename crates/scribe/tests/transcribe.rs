//! End-to-end integration test for `scribe transcribe`.
//!
//! Runs the compiled `scribe` binary against the bundled
//! `tests/fixtures/jfk.wav` clip. Because the pipeline shells out to
//! `ffmpeg` and `whisper-cli` and needs a real model file, this test
//! **skips itself with a clean pass** on hosts where any of those is
//! missing — that's the only way `cargo test --all` can stay green on
//! CI runners that don't have the heavy AI toolchain installed.
//!
//! What "skip" means here: we `eprintln!` a clear message explaining
//! which dependency was missing, then `return` from the test. From
//! Cargo's perspective the test passed. The orchestrator (whoever ran
//! the suite) gets to see the skip reason in `--nocapture` output.
//!
//! See issue #7 for the spec this test enforces.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// Path to the compiled `scribe` binary, provided by Cargo for integration tests.
const SCRIBE_BIN: &str = env!("CARGO_BIN_EXE_scribe");

/// The fixture audio file. ~11 s of JFK's 1961 inaugural — see
/// `tests/fixtures/README.md` for license + provenance.
const FIXTURE_WAV: &str = "tests/fixtures/jfk.wav";

/// A word we expect to appear (case-insensitively) in the transcribed body.
/// "country" is in the famous line twice; if the pipeline ran whisper for
/// real against this clip, this substring will be there.
const EXPECTED_BODY_WORD: &str = "country";

/// Look up `name` on `$PATH`, returning the first executable hit.
///
/// Reimplemented locally rather than `pub`-ing `crate::which::which_on_path`
/// because integration tests live outside the crate (the `scribe` crate
/// is a binary, not a library) and can't `use` private modules. Keeping
/// this duplicate keeps the pipeline source frozen as of PR #15.
fn which_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_executable_file(path: &Path) -> bool {
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

/// Mirror of `crate::doctor::resolve_whisper_bin` — env first, then the
/// canonical voice-stuff default under `$HOME`.
fn resolve_whisper_bin() -> Option<PathBuf> {
    if let Some(v) = std::env::var_os("SCRIBE_WHISPER_BIN") {
        if !v.is_empty() {
            return Some(PathBuf::from(v));
        }
    }
    let home = dirs::home_dir()?;
    Some(home.join("raibid-labs/voice-stuff/third_party/whisper.cpp/build/bin/whisper-cli"))
}

/// Mirror of `crate::doctor::resolve_model_dir`.
fn resolve_model_dir() -> Option<PathBuf> {
    if let Some(v) = std::env::var_os("SCRIBE_MODEL_PATH") {
        if !v.is_empty() {
            return Some(PathBuf::from(v));
        }
    }
    let home = dirs::home_dir()?;
    Some(home.join("raibid-labs/voice-stuff/models"))
}

/// Find any `*.bin` file in the model dir we can use.
fn first_model_in(dir: &Path) -> Option<PathBuf> {
    let read = std::fs::read_dir(dir).ok()?;
    let mut bins: Vec<PathBuf> = read
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let is_bin = p.extension().and_then(|s| s.to_str()) == Some("bin");
            // Follow symlinks: voice-stuff symlinks ggml-*.bin into ./models/.
            let resolves_to_file = std::fs::metadata(&p).map(|m| m.is_file()).unwrap_or(false);
            (is_bin && resolves_to_file).then_some(p)
        })
        .collect();
    bins.sort();
    bins.into_iter().next()
}

/// Why an integration run was skipped. `Display` is the message printed
/// to stderr before the test returns clean.
#[derive(Debug)]
enum SkipReason {
    Ffmpeg,
    Ffprobe,
    WhisperCli(PathBuf),
    Model(Option<PathBuf>),
    Fixture(PathBuf),
}

impl std::fmt::Display for SkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkipReason::Ffmpeg => write!(f, "ffmpeg not found on PATH"),
            SkipReason::Ffprobe => write!(f, "ffprobe not found on PATH"),
            SkipReason::WhisperCli(p) => write!(
                f,
                "whisper-cli not found or not executable at {} (set $SCRIBE_WHISPER_BIN)",
                p.display()
            ),
            SkipReason::Model(Some(p)) => {
                write!(f, "no usable *.bin model file found in {}", p.display())
            }
            SkipReason::Model(None) => {
                write!(
                    f,
                    "cannot resolve model directory (no $SCRIBE_MODEL_PATH and no $HOME)"
                )
            }
            SkipReason::Fixture(p) => {
                write!(
                    f,
                    "fixture audio missing at {} (run from crate root?)",
                    p.display()
                )
            }
        }
    }
}

/// Resolve absolute path to the fixture WAV relative to the manifest dir.
/// `CARGO_MANIFEST_DIR` is set by Cargo for integration tests, so this
/// works regardless of where `cargo test` was invoked from.
fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_WAV)
}

/// All-or-nothing pre-flight: returns `Ok` only if every external dep the
/// pipeline needs is present and the fixture is on disk. Otherwise the
/// `SkipReason` tells the caller exactly what's missing.
struct ReadyEnv {
    /// Path to the model file we'll pass via `--model`. We hand the whole
    /// filename (including extension) so `config::resolve_model_path` will
    /// pick it up via its "literal name" branch.
    model_arg: OsString,
    /// Directory holding `model_arg`. Threaded into `SCRIBE_MODEL_PATH`.
    model_dir: PathBuf,
    /// Path to the fixture WAV.
    fixture: PathBuf,
}

fn check_environment() -> Result<ReadyEnv, SkipReason> {
    if which_on_path("ffmpeg").is_none() {
        return Err(SkipReason::Ffmpeg);
    }
    if which_on_path("ffprobe").is_none() {
        return Err(SkipReason::Ffprobe);
    }
    let whisper = resolve_whisper_bin()
        .ok_or_else(|| SkipReason::WhisperCli(PathBuf::from("<unresolved>")))?;
    if !is_executable_file(&whisper) {
        return Err(SkipReason::WhisperCli(whisper));
    }
    let model_dir = resolve_model_dir().ok_or(SkipReason::Model(None))?;
    let model_file =
        first_model_in(&model_dir).ok_or_else(|| SkipReason::Model(Some(model_dir.clone())))?;
    let fixture = fixture_path();
    if !fixture.is_file() {
        return Err(SkipReason::Fixture(fixture));
    }

    // Pass the literal filename (e.g. `ggml-base.en.bin`) so
    // `config::resolve_model_path`'s "literal" branch finds it without
    // having to know whether the model was named base.en or large-v3.
    let model_arg = model_file
        .file_name()
        .expect("model_file must have a filename")
        .to_owned();

    Ok(ReadyEnv {
        model_arg,
        model_dir,
        fixture,
    })
}

/// End-to-end happy path: invoke `scribe transcribe` against the fixture,
/// assert the two output files exist with the right naming, parse the
/// markdown frontmatter + body, parse the JSON sidecar.
///
/// On any missing-dep host this prints the skip reason to stderr and
/// returns — the test passes clean from Cargo's perspective.
#[test]
fn transcribe_jfk_end_to_end() {
    let env = match check_environment() {
        Ok(e) => e,
        Err(reason) => {
            eprintln!("SKIP transcribe_jfk_end_to_end: {reason}");
            return;
        }
    };

    let tmp = TempDir::new().expect("tempdir");
    let out_dir = tmp.path();

    let output = Command::new(SCRIBE_BIN)
        .arg("transcribe")
        .arg("--input")
        .arg(&env.fixture)
        .arg("--out-dir")
        .arg(out_dir)
        .arg("--model")
        .arg(&env.model_arg)
        // Force the resolver to see our model dir even if the host's
        // env points elsewhere. We don't override SCRIBE_WHISPER_BIN —
        // the default-resolution path is exactly what we want to test.
        .env("SCRIBE_MODEL_PATH", &env.model_dir)
        // Ensure tracing doesn't spam the test output.
        .env("RUST_LOG", "warn")
        .output()
        .expect("failed to invoke scribe transcribe");

    if !output.status.success() {
        panic!(
            "scribe transcribe failed (status {}):\n--- stdout ---\n{}\n--- stderr ---\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // The pipeline prints the absolute md path on stdout; we don't rely
    // on it for assertions (we re-discover the files), but a non-empty
    // stdout is a useful sanity check.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.trim().is_empty(),
        "scribe transcribe produced no stdout (expected the .md path)"
    );

    // Discover the artifacts. Slug should be `jfk` (stem of jfk.wav);
    // basename matches `jfk-YYYYMMDD-HHMMSS`.
    let entries: Vec<_> = std::fs::read_dir(out_dir)
        .expect("read out_dir")
        .flatten()
        .map(|e| e.path())
        .collect();

    let md_path = entries
        .iter()
        .find(|p| p.extension().and_then(|s| s.to_str()) == Some("md"))
        .unwrap_or_else(|| panic!("no .md found in out_dir; entries: {entries:?}"));
    let json_path = entries
        .iter()
        .find(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .unwrap_or_else(|| panic!("no .json found in out_dir; entries: {entries:?}"));

    let md_stem = md_path
        .file_stem()
        .and_then(|s| s.to_str())
        .expect("md filename");
    let json_stem = json_path
        .file_stem()
        .and_then(|s| s.to_str())
        .expect("json filename");
    assert_eq!(
        md_stem, json_stem,
        ".md and .json basenames must match (got {md_stem} vs {json_stem})"
    );
    assert!(
        is_basename_well_formed("jfk", md_stem),
        "basename {md_stem:?} does not match jfk-YYYYMMDD-HHMMSS"
    );

    // --- Markdown structure ---
    let md = std::fs::read_to_string(md_path).expect("read md");
    let (frontmatter, body) = split_frontmatter_and_body(&md);

    for key in [
        "type",
        "source",
        "title",
        "created",
        "duration_sec",
        "model",
        "sidecar",
        "tags",
    ] {
        let needle = format!("{key}:");
        assert!(
            frontmatter.contains(&needle),
            "frontmatter missing key {key:?}\n--- frontmatter ---\n{frontmatter}"
        );
    }
    // ISO-8601 sanity: created should look like 2026-04-28T...Z.
    assert!(
        frontmatter
            .lines()
            .any(|l| l.starts_with("created: ") && l.contains('T') && l.trim_end().ends_with('Z')),
        "created field is not ISO-8601 UTC\n--- frontmatter ---\n{frontmatter}"
    );
    // sidecar field should reference the .json filename, not a path.
    let json_filename = json_path
        .file_name()
        .and_then(|s| s.to_str())
        .expect("json filename");
    assert!(
        frontmatter.contains(&format!("sidecar: {json_filename}")),
        "sidecar field does not point at {json_filename}\n--- frontmatter ---\n{frontmatter}"
    );

    // Body cleanness: non-empty and does not start with `---`.
    assert!(!body.trim().is_empty(), "body is empty");
    assert!(
        !body.trim_start().starts_with("---"),
        "body unexpectedly starts with `---`; frontmatter separation broken\n--- body ---\n{body}"
    );
    // JFK-specific content check: confirms whisper actually ran. Compare
    // case-insensitively because punctuation/casing may shift slightly
    // with model versions.
    assert!(
        body.to_lowercase().contains(EXPECTED_BODY_WORD),
        "body does not contain the expected word {EXPECTED_BODY_WORD:?} \
         (transcription almost certainly didn't run for real)\n--- body ---\n{body}"
    );

    // --- JSON sidecar ---
    let json_text = std::fs::read_to_string(json_path).expect("read json");
    let value: serde_json::Value =
        serde_json::from_str(&json_text).expect("sidecar must be valid JSON");

    // whisper-cli's JSON top-level shape (verified empirically against
    // the 1.7.x build at voice-stuff/third_party/whisper.cpp): a top-level
    // `transcription` array of segments, each with `text`, `offsets`
    // (with `from` / `to` in ms), and `timestamps` (string form).
    let transcription = value
        .get("transcription")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("sidecar missing top-level `transcription` array; got: {value}"));
    assert!(!transcription.is_empty(), "transcription array is empty");
    let first = &transcription[0];
    assert!(
        first.get("text").and_then(|v| v.as_str()).is_some(),
        "first transcription segment missing `text`: {first}"
    );
    let offsets = first
        .get("offsets")
        .unwrap_or_else(|| panic!("first segment missing `offsets`: {first}"));
    assert!(
        offsets.get("from").and_then(|v| v.as_u64()).is_some(),
        "offsets.from is not a number: {offsets}"
    );
    assert!(
        offsets.get("to").and_then(|v| v.as_u64()).is_some(),
        "offsets.to is not a number: {offsets}"
    );
}

/// Verify a basename matches `<expected_slug>-YYYYMMDD-HHMMSS`. Returns
/// false on any deviation. This is intentionally strict — if the slug
/// derivation drifts, this catches it.
fn is_basename_well_formed(expected_slug: &str, basename: &str) -> bool {
    let prefix = format!("{expected_slug}-");
    let Some(ts) = basename.strip_prefix(&prefix) else {
        return false;
    };
    // YYYYMMDD-HHMMSS = 8 digits, dash, 6 digits = length 15.
    if ts.len() != 15 {
        return false;
    }
    let bytes = ts.as_bytes();
    bytes[..8].iter().all(|b| b.is_ascii_digit())
        && bytes[8] == b'-'
        && bytes[9..].iter().all(|b| b.is_ascii_digit())
}

/// Split a markdown file at the closing `---` of the YAML frontmatter.
/// Returns `(frontmatter_block, body)`. Panics if the file doesn't begin
/// with a frontmatter block — that's a contract violation worth failing
/// loudly on.
fn split_frontmatter_and_body(md: &str) -> (&str, &str) {
    let rest = md
        .strip_prefix("---\n")
        .unwrap_or_else(|| panic!("markdown does not start with `---\\n`:\n{md}"));
    let end = rest
        .find("\n---\n")
        .unwrap_or_else(|| panic!("no closing `---` for frontmatter:\n{md}"));
    // `frontmatter` includes the opening `---\n` and closing `---\n` for
    // readability in error messages.
    let frontmatter_end = "---\n".len() + end + "\n---\n".len();
    let (fm, body) = md.split_at(frontmatter_end);
    (fm, body)
}

#[cfg(test)]
mod helper_tests {
    use super::*;

    #[test]
    fn basename_well_formed_accepts_canonical() {
        assert!(is_basename_well_formed("jfk", "jfk-20260428-143000"));
    }

    #[test]
    fn basename_well_formed_rejects_wrong_slug() {
        assert!(!is_basename_well_formed("jfk", "other-20260428-143000"));
    }

    #[test]
    fn basename_well_formed_rejects_short_timestamp() {
        assert!(!is_basename_well_formed("jfk", "jfk-2026-143000"));
    }

    #[test]
    fn basename_well_formed_rejects_non_digit_timestamp() {
        assert!(!is_basename_well_formed("jfk", "jfk-2026042X-143000"));
    }

    #[test]
    fn split_frontmatter_recovers_both_halves() {
        let md = "---\ntype: x\n---\n\nbody line one\nbody line two\n";
        let (fm, body) = split_frontmatter_and_body(md);
        assert!(fm.starts_with("---\n"));
        assert!(fm.ends_with("---\n"));
        assert!(fm.contains("type: x"));
        assert_eq!(body, "\nbody line one\nbody line two\n");
    }
}
