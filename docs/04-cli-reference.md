# CLI reference

> This is the intended CLI surface; implementation tracks issues
> [#5](https://github.com/raibid-labs/scribe/issues/5) and
> [#6](https://github.com/raibid-labs/scribe/issues/6).

Scribe is a single binary with subcommands. The top-level shape:

```
scribe <SUBCOMMAND> [FLAGS]
```

Subcommands:

- [`transcribe`](#scribe-transcribe) — run the audio → markdown + JSON
  pipeline
- [`doctor`](#scribe-doctor) — verify ffmpeg, whisper-cli, and a usable
  model are installed

## `scribe transcribe`

Resample an audio or video file with ffmpeg, run it through whisper.cpp,
and write a markdown transcript plus a JSON sidecar to `--out-dir`.

```
scribe transcribe --input <FILE> --out-dir <DIR> [--model <NAME>] [--keep-temp]
```

### Flags

| Flag | Required | Description |
| --- | --- | --- |
| `--input <FILE>` | yes | Path to an audio or video file. Any format ffmpeg can read. |
| `--out-dir <DIR>` | yes | Directory for the `.md` and `.json` outputs. Created if missing. |
| `--model <NAME>` | no | Whisper model name. Resolved against `$SCRIBE_MODEL_PATH` (or the default model dir). Defaults to `$SCRIBE_MODEL` if set, else a built-in default (`large-v3` if available, else `base.en`). |
| `--keep-temp` | no | Do not delete the intermediate 16 kHz WAV. Useful for debugging. |

### Environment variables

- `SCRIBE_MODEL` — default model name when `--model` is not passed.
- `SCRIBE_MODEL_PATH` — directory to resolve model names against.
- `SCRIBE_WHISPER_BIN` — path to the `whisper-cli` binary. Falls back to
  `~/raibid-labs/voice-stuff/third_party/whisper.cpp/build/bin/whisper-cli`.

### Output

For an input named `My Podcast Ep 42.mp3` transcribed at
`2026-04-28T14:30:00Z`, with `--out-dir ~/transcripts`:

```
~/transcripts/my-podcast-ep-42-20260428-143000.md
~/transcripts/my-podcast-ep-42-20260428-143000.json
```

The slug is the input filename stem, lowercased, with non-alphanumerics
collapsed to `-`. The timestamp is `YYYYMMDD-HHMMSS` in UTC.

The markdown wraps the verbatim whisper output in YAML frontmatter:

```yaml
---
type: transcript
source: /abs/path/to/My Podcast Ep 42.mp3
title: My Podcast Ep 42
created: 2026-04-28T14:30:00Z
duration_sec: 3127.4
model: large-v3
sidecar: my-podcast-ep-42-20260428-143000.json
tags: [type/transcript]
---
```

The JSON sidecar is whisper.cpp's segment-level output, untouched.

### Examples

Basic:

```sh
scribe transcribe --input podcast.mp3 --out-dir ~/transcripts
```

With an explicit model:

```sh
scribe transcribe \
  --input lecture.m4a \
  --out-dir ~/transcripts/lectures \
  --model base.en
```

Keep the temp WAV for debugging:

```sh
scribe transcribe --input voice-memo.m4a --out-dir . --keep-temp
```

### Exit codes

- `0` — markdown + JSON written.
- non-zero — any pipeline step failed; an error is printed to stderr.

## `scribe doctor`

Verify the runtime dependencies. Prints a status line for each.

```
scribe doctor
```

### Checks

- `ffmpeg` — resolves with `which ffmpeg`, prints the first line of
  `ffmpeg -version`.
- `whisper-cli` — checks `$SCRIBE_WHISPER_BIN`, then a sensible default
  (`~/raibid-labs/voice-stuff/third_party/whisper.cpp/build/bin/whisper-cli`).
  Prints version if available.
- `model` — checks `$SCRIBE_MODEL_PATH`, then a default model dir
  (`~/raibid-labs/voice-stuff/models/`). Lists any `.bin` files found.
- `GPU` — runs `nvidia-smi --query-gpu=name --format=csv,noheader` if
  available; prints `n/a` otherwise. Non-fatal — CPU-only is acceptable.

### Exit codes

- `0` — all checks pass, or only the GPU check is missing.
- `1` — ffmpeg, whisper-cli, or model is missing.

### Example

```sh
$ scribe doctor
ffmpeg       /usr/bin/ffmpeg            ffmpeg version 6.1.1
whisper-cli  ~/raibid-labs/.../whisper-cli  whisper.cpp v1.7.x
model        ~/raibid-labs/voice-stuff/models  ggml-large-v3.bin, ggml-base.en.bin
GPU          NVIDIA GB10
```
