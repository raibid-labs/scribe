# Troubleshooting

Common failure modes and their fixes. Run [`scribe doctor`](./04-cli-reference.md#scribe-doctor)
first — it reports most of these directly.

## `ffmpeg: command not found`

ffmpeg is not on `PATH`. Install it.

Debian / Ubuntu:

```sh
sudo apt install ffmpeg
```

macOS:

```sh
brew install ffmpeg
```

Verify:

```sh
which ffmpeg
ffmpeg -version
```

## `whisper-cli: not found`

Scribe could not locate the `whisper-cli` binary. Two fixes:

1. Set `SCRIBE_WHISPER_BIN` to the absolute path:

   ```sh
   export SCRIBE_WHISPER_BIN=/path/to/whisper-cli
   ```

2. Build whisper.cpp via the voice-stuff repo. Until voice-stuff is on
   GitHub, the canonical build instructions live in
   `~/raibid-labs/voice-stuff/docs/setup.md`. The resulting binary
   lands at:

   ```
   ~/raibid-labs/voice-stuff/third_party/whisper.cpp/build/bin/whisper-cli
   ```

   That is Scribe's default lookup path — no env var needed if the
   binary is there.

## `model file not found`

Scribe could not find a usable `.bin` model file. Two fixes:

1. Fetch the model via voice-stuff's recipe:

   ```sh
   cd ~/raibid-labs/voice-stuff
   just fetch-model large-v3
   ```

   Models land in `~/raibid-labs/voice-stuff/models/`, which is Scribe's
   default model dir.

2. Point Scribe at a different model dir:

   ```sh
   export SCRIBE_MODEL_PATH=/path/to/your/models
   ```

   Verify with `scribe doctor` — it lists every `.bin` it finds.

## `CUDA unavailable` / GPU not detected

The `GPU` line in `scribe doctor` reports `n/a` or fails.

- **macOS:** expected. There is no CUDA on macOS. The `whisper-cli`
  binary will fall back to CPU. CPU mode is acceptable for short clips
  but slow for podcast-length audio.
- **Linux:** check the NVIDIA driver:

  ```sh
  nvidia-smi
  ```

  If `nvidia-smi` works but `whisper-cli` still runs on CPU, the
  whisper.cpp build was not compiled with CUDA support. Rebuild it via
  the voice-stuff instructions.

GPU absence is non-fatal — `scribe doctor` exits 0 even when only the
GPU check fails. The transcription will still run, just slower.

## "thanks for watching" / "♪" appears in transcripts

Not a bug. Whisper occasionally hallucinates filler text on long
silences — most often "thanks for watching", musical-note glyphs, or a
loop of the last spoken phrase. See
[`01-architecture.md`](./01-architecture.md#verbatim-only).

Per the verbatim contract, **Scribe does not filter these**. The
markdown contains exactly the text whisper.cpp emitted. Downstream
tooling — annotation scripts, scryforge actions, ad-hoc `sed` — is the
right place to clean them.

If they are dense enough to be a problem, consider:

- A different model (`large-v3` hallucinates less than `base.en`).
- Pre-trimming long silences with ffmpeg's `silenceremove` filter
  before transcription.
- Post-filtering the JSON sidecar in your downstream tool.
