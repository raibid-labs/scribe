//! Scribe CLI entry point.
//!
//! Top-level clap-derived CLI with `transcribe` and `doctor` subcommands.
//! Both are now fully wired: `doctor` (issue #5) verifies the runtime,
//! `transcribe` (issue #6) runs the ffmpeg → whisper.cpp → markdown +
//! JSON pipeline.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod config;
mod doctor;
mod ffmpeg;
mod output;
mod pipeline;
mod which;
mod whisper;

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

    /// Preserve the intermediate 16 kHz WAV. Useful for debugging.
    #[arg(long)]
    keep_temp: bool,
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

fn run_transcribe(args: TranscribeArgs) -> Result<()> {
    let cli = config::CliInputs {
        input: args.input,
        out_dir: args.out_dir,
        model: args.model,
        keep_temp: args.keep_temp,
    };
    let md_path = pipeline::run(cli)?;
    println!("{}", md_path.display());
    Ok(())
}

fn run_doctor() -> Result<bool> {
    doctor::run()
}
