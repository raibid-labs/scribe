# Scribe

A small Linux/CUDA utility for turning audio files into **verbatim** Markdown
transcripts plus timestamped JSON sidecars. Built around `whisper.cpp` and
`ffmpeg`. Part of the [raibid-labs](https://github.com/raibid-labs)
ecosystem.

## What it does

Take an audio file → run it through `ffmpeg` (resample to 16 kHz mono PCM) →
run it through `whisper.cpp` (CUDA-accelerated on NVIDIA GPUs) → write two
files to a configurable output directory:

- `<slug>-<timestamp>.md` — verbatim transcript wrapped in YAML frontmatter
- `<slug>-<timestamp>.json` — segment-level timestamps for downstream tooling

## Design contract

- **Verbatim only.** No summarization, no paraphrase, no condensation, no LLM
  in the path. If you wanted Snipd, this isn't it.
- **Single-purpose.** Audio in, faithful text out. Annotation, link insertion,
  search, summarization — those are downstream concerns the user (or another
  tool) handles.
- **Two-file output.** Markdown for humans, JSON for tooling. Same base name,
  same directory.
- **Configurable.** Output directory, model choice, and whisper.cpp flags
  surface via CLI flags and (eventually) `.fsx` config.

See [`docs/01-architecture.md`](./docs/01-architecture.md) for the full
design rationale.

## Status

Phase 0 — repository init. No implementation yet. See
[issues](https://github.com/raibid-labs/scribe/issues) for the work plan.

## Quick start

(Forthcoming once the implementation lands.)

```sh
scribe transcribe --input podcast.mp3 --out-dir ~/transcripts
```

## Documentation

- [`docs/00-index.md`](./docs/00-index.md) — index and overview
- [`docs/01-architecture.md`](./docs/01-architecture.md) — design rationale,
  the verbatim contract, dual-output design, why no LLM is in the path
- [`docs/02-roadmap.md`](./docs/02-roadmap.md) — planned input adapters,
  output formats, deferrals, explicit non-goals
- [`docs/03-related-tools.md`](./docs/03-related-tools.md) — how Scribe fits
  with other raibid-labs projects (Scryforge, voice-stuff, Phage, gudpkm
  n8n)
- [`docs/04-cli-reference.md`](./docs/04-cli-reference.md) — subcommands,
  flags, env vars, exit codes, output layout
- [`docs/05-contributing.md`](./docs/05-contributing.md) — dev environment
  setup, code style, PR conventions, how to extend inputs and outputs
- [`docs/06-troubleshooting.md`](./docs/06-troubleshooting.md) — common
  failure modes and fixes
- [`docs/07-self-hosted-ci.md`](./docs/07-self-hosted-ci.md) — self-hosted
  GitHub Actions runner for the end-to-end integration test, security
  model, host hardening

## Related raibid-labs projects

- **[Scryforge](https://github.com/raibid-labs/scryforge)** — Fusabi-powered
  TUI information rolodex (RSS, email, YouTube, Spotify, Reddit, bookmarks).
  Will eventually invoke Scribe as the engine behind a `transcribe-to-vault`
  action on podcast and YouTube items.
- **voice-stuff** (legacy, local) — Python push-to-talk dictation prototype.
  Shares the underlying `whisper.cpp` build with Scribe. Slated for a Rust
  rewrite as **Murmur** (dictation) plus a separate voice-agent project.
- **[Phage](https://github.com/raibid-labs/phage)** — context composition
  engine. Pattern reference for Scribe's eventual `.fsx` config layer
  (Fusabi-as-config-DSL).
- **gudpkm n8n stack** — the `voice_memo` workflow has a documented TODO to
  call Scribe for the audio-upload branch of its webhook.

See [`docs/03-related-tools.md`](./docs/03-related-tools.md) for the full
picture.

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](./LICENSE-MIT) or
  http://opensource.org/licenses/MIT)

at your option.
