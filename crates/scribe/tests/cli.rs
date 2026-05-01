//! Smoke tests for the `scribe` binary.

use std::process::Command;

/// Path to the compiled `scribe` binary, provided by Cargo for integration tests.
const SCRIBE_BIN: &str = env!("CARGO_BIN_EXE_scribe");

#[test]
fn help_exits_zero() {
    let status = Command::new(SCRIBE_BIN)
        .arg("--help")
        .status()
        .expect("failed to invoke scribe --help");
    assert!(status.success(), "scribe --help exited with {status:?}");
}

#[test]
fn transcribe_help_exits_zero() {
    let status = Command::new(SCRIBE_BIN)
        .args(["transcribe", "--help"])
        .status()
        .expect("failed to invoke scribe transcribe --help");
    assert!(
        status.success(),
        "scribe transcribe --help exited with {status:?}"
    );
}

#[test]
fn doctor_help_exits_zero() {
    let status = Command::new(SCRIBE_BIN)
        .args(["doctor", "--help"])
        .status()
        .expect("failed to invoke scribe doctor --help");
    assert!(
        status.success(),
        "scribe doctor --help exited with {status:?}"
    );
}
