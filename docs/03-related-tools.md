# Related raibid-labs tools

Scribe lives in a constellation. Where it touches other projects:

## Scryforge

[github.com/raibid-labs/scryforge](https://github.com/raibid-labs/scryforge)

A Fusabi-powered, pluggable TUI information rolodex. Aggregates RSS, email,
Spotify, YouTube, Reddit, and bookmarks behind a single terminal interface
backed by a local daemon.

**Tie-in:** Scryforge will gain a `transcribe-to-vault` action on podcast
and YouTube items. The action body shells out to
`scribe --input <url-or-file> --out-dir <stream-config-path>`. Per-stream
config decides destination — research podcasts go one place, casual
listening goes another.

Scryforge does not embed whisper. Scribe is the engine; Scryforge is the
workflow surface that knows *which* audio to transcribe.

The same binary serves the CLI, Scryforge, and the gudpkm n8n flow with
one implementation.

## voice-stuff (legacy)

`~/raibid-labs/voice-stuff/`, not yet on GitHub. Python prototype for
push-to-talk dictation: hold a hotkey, speak, transcript types into the
focused window via `xdotool`.

**Relationship:** Shares the underlying `whisper.cpp` build with Scribe
(same CUDA-compiled binary, same model files). Different problem, though —
voice-stuff is live mic → keystrokes; Scribe is files → markdown.

**Status:** Frozen. Slated to be split + rewritten in Rust under a new name
("Murmur" is the working name) for the dictation half, with a separate
project for the voice-agent half (Phase 2 of the original voice-stuff
design).

## Murmur (planned)

Future Rust replacement for voice-stuff Phase 1. Push-to-talk → STT →
typed output.

**Relationship:** Will share an `audio-capture` library crate and the
`whisper.cpp` subprocess wrapper with Scribe once both projects mature.
Until then, both projects shell out to `whisper-cli` independently.

## Phage

[github.com/raibid-labs/phage](https://github.com/raibid-labs/phage)

Context composition engine for AI agents. Uses Fusabi `.fsx` configs as a
DSL on top of a Rust runtime.

**Relationship:** Pattern reference. Scribe's eventual `.fsx` config layer
(defaults for model path, output dir, frontmatter template) borrows the
Fusabi-as-config-DSL pattern from Phage.

## Hibana

`~/raibid-labs/hibana/`. Observability ingest pipeline.

**Relationship:** None directly today. If Scribe ever needs structured
operational telemetry (transcription latency, error rates, GPU utilization
per file), it would emit to Hibana per the
[Hibana-first observability convention](https://github.com/raibid-labs/hibana).

## gudpkm n8n stack

`~/github.com/gudpkm/stacks/n8n/workflows/voice_memo.md`.

The `voice_memo` n8n workflow has a webhook that accepts
`{transcript, audio?}`. The audio branch is a documented TODO — currently
writes a placeholder note tagged `sort/needs-review`.

**Relationship:** The TODO closes by having the workflow shell out to
`scribe` (via n8n's Execute Command node or a sidecar HTTP wrapper) when
the audio branch fires. That makes phone-uploaded voice memos transcribe
automatically into the gudpkm vault.
