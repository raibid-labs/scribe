//! Scribe CLI entry point.
//!
//! This is the v0.1 skeleton: clap-derived top-level CLI with `transcribe`
//! and `doctor` subcommands. Both subcommands are stubs that print
//! "not implemented" and exit 0. The real pipeline lands in issues #5
//! (`doctor`) and #6 (`transcribe`).

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

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

fn main() -> Result<()> {
    init_tracing();

    let cli = Cli::parse();

    match cli.command {
        Command::Transcribe(args) => run_transcribe(args),
        Command::Doctor => run_doctor(),
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

fn run_doctor() -> Result<()> {
    println!("scribe doctor: not implemented");
    Ok(())
}
