//! Scribe CLI entry point.
//!
//! Top-level clap-derived CLI with `transcribe` and `doctor` subcommands.
//! `doctor` is fully wired (issue #5); `transcribe` is still the v0.1
//! stub awaiting issue #6.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod doctor;
mod which;

/// Verbatim audio-to-Markdown transcription CLI.
///
/// Takes an audio file, runs it through ffmpeg + whisper.cpp, and emits
/// a verbatim Markdown transcript plus a timestamped JSON sidecar.
#[derive(Debug, Parser)]
#[command(name = "scribe", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Transcribe an audio file to Markdown + JSON.
    Transcribe(TranscribeArgs),

    /// Verify ffmpeg, whisper-cli, and model availability.
    Doctor,
}

#[derive(Debug, clap::Args)]
struct TranscribeArgs {
    /// Input audio file (mp3, m4a, wav, mp4, ...).
    #[arg(long, value_name = "FILE")]
    input: PathBuf,

    /// Output directory for the .md and .json files.
    #[arg(long, value_name = "DIR")]
    out_dir: PathBuf,

    /// Optional whisper.cpp model name (e.g. `large-v3`).
    #[arg(long, value_name = "NAME")]
    model: Option<String>,
}

fn main() -> ExitCode {
    init_tracing();

    let cli = Cli::parse();

    let result: Result<ExitCode> = match cli.command {
        Command::Transcribe(args) => run_transcribe(args).map(|_| ExitCode::SUCCESS),
        Command::Doctor => run_doctor().map(|healthy| {
            if healthy {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }),
    };

    match result {
        Ok(code) => code,
        Err(e) => {
            tracing::error!("{:#}", e);
            ExitCode::from(1)
        }
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
}

fn run_transcribe(_args: TranscribeArgs) -> Result<()> {
    println!("scribe transcribe: not implemented");
    Ok(())
}

fn run_doctor() -> Result<bool> {
    doctor::run()
}
