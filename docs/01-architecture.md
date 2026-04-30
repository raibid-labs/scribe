# Architecture

## Goal

Take an audio file. Produce a faithful, machine-readable record of every
word the speaker said. Stop there.

## Pipeline

```
input file (mp3/m4a/wav/mp4/...)
    │
    ▼
ffmpeg
    │   resample to 16 kHz mono PCM s16le
    ▼
whisper.cpp (CUDA)
    │   -otxt → verbatim text
    │   -ojf  → segment-level JSON with start/end (ms)
    ▼
markdown writer
    │   wrap text in YAML frontmatter
    │   leave JSON untouched as a sidecar
    ▼
<out-dir>/<slug>-<ts>.md   +   <out-dir>/<slug>-<ts>.json
```

## Design contract

### Verbatim only

The pipeline emits the words Whisper heard, full stop. No summarization, no
paraphrase, no condensation, no LLM in the path.

This is a hard constraint, not a default. Snipd-style condensation is an
explicit non-goal. Downstream tooling (annotation scripts, scryforge
actions, on-demand Nemotron summaries) handles everything else; Scribe's job
is to provide the raw material.

Whisper does have one known failure mode — it occasionally hallucinates
filler text ("thanks for watching", "♪") on long silences. That's noise to
clean, not paraphrase. The pipeline does not try to filter it; downstream
tools can.

### Dual output

For every input, two files come out, side by side, sharing a base name:

- `<slug>-<timestamp>.md` — human-readable. YAML frontmatter (source,
  duration, model, sidecar reference) + verbatim text body.
- `<slug>-<timestamp>.json` — machine-readable. whisper.cpp's segment-level
  JSON (`start` / `end` in ms, `text` per segment). With `-ml 1`,
  word-level.

Why both: humans skim the markdown, tools consume the JSON. The JSON is
what makes "find every mention of product X and link to that point in the
audio" a one-liner.

### Configurable, not opinionated

Output directory, model choice, and whisper.cpp flag surface through CLI
flags. A future `.fsx` config layer (borrowing the Fusabi-as-config-DSL
pattern from [Phage](https://github.com/raibid-labs/phage)) handles
defaults. There is no hardcoded vault path, no hardcoded model.

## What Scribe is not

- A summarizer. (Verbatim contract.)
- A note-taker. (Output is files; what tools consume them is not Scribe's
  concern.)
- A media intelligence platform. (Topic extraction, chaptering, sentiment —
  all downstream.)
- An RSS reader, a podcast app, or a video-downloader UI. (See
  [`03-related-tools.md`](./03-related-tools.md) — Scryforge is the workflow
  surface; Scribe is the engine.)
- A live-dictation tool. (Different problem entirely — see "Murmur" in
  [`03-related-tools.md`](./03-related-tools.md).)

## Why subprocess, not bindings

The first cut shells out to `ffmpeg` and `whisper-cli` rather than binding
`libavformat` or `libwhisper`. Reasons:

- Both binaries are stable, well-documented, and trivially upgradable in
  isolation.
- `whisper.cpp` is built once on the host with CUDA support; embedding it
  would mean re-solving that build per release per target.
- Subprocess output is easy to test against fixtures.

Bindings can replace subprocesses later if there's a measured reason
(latency, resource control, finer error reporting). Don't pre-optimize.

## Hardware assumptions

Primary target is the NVIDIA DGX Spark (aarch64 Linux + GB10 GPU + CUDA 13)
that the rest of the raibid-labs stack already runs on. The whisper.cpp
build at `~/raibid-labs/voice-stuff/third_party/whisper.cpp/build/bin/whisper-cli`
is the canonical engine; Scribe locates it via config / env, does not embed
it.

CPU-only fallback is plausible but not actively supported — Scribe does not
ship a non-CUDA whisper.cpp build, and the expected throughput on CPU-only
podcast-length audio is poor enough that we treat it as out of scope.
