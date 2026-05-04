# Scribe documentation

This directory holds Scribe's design and reference docs.

## Documents

- [`01-architecture.md`](./01-architecture.md) — design rationale, the
  verbatim contract, dual-output (`.md` + `.json`), why no LLM is in the
  path, why subprocess instead of bindings
- [`02-roadmap.md`](./02-roadmap.md) — planned input adapters (RSS
  enclosures, YouTube via `yt-dlp`), planned output formats (SRT, VTT),
  diarization deferral, explicit non-goals
- [`03-related-tools.md`](./03-related-tools.md) — how Scribe fits with
  Scryforge, voice-stuff, the gudpkm n8n flow, the future Murmur dictation
  utility, and Phage
- [`04-cli-reference.md`](./04-cli-reference.md) — every subcommand and
  flag, env vars, exit codes, output layout
- [`05-contributing.md`](./05-contributing.md) — dev environment setup
  (rustup, just, ffmpeg, whisper.cpp, models), code style, PR
  conventions, how to extend inputs and outputs
- [`06-troubleshooting.md`](./06-troubleshooting.md) — common failure
  modes (missing ffmpeg / whisper-cli / model, CUDA absence, whisper
  hallucinations on silence)
- [`07-self-hosted-ci.md`](./07-self-hosted-ci.md) — self-hosted GitHub
  Actions runner pattern for the end-to-end integration test, security
  model (no fork PRs, push-to-main only), runner registration, host
  hardening, and removal

## Pointers

- Project README: [`../README.md`](../README.md)
- Issues / planned work: https://github.com/raibid-labs/scribe/issues
- License: dual MIT / Apache-2.0 ([`../LICENSE-MIT`](../LICENSE-MIT),
  [`../LICENSE-APACHE`](../LICENSE-APACHE))
