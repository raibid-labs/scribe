# Roadmap

Scribe's expansion is **lateral, not vertical** — it grows by adding input
sources and output formats, not by climbing the abstraction ladder into
summarization or agent territory.

## v0.1 (MVP)

The initial release. Tracked across the issues at
https://github.com/raibid-labs/scribe/issues:

- Cargo workspace + `crates/scribe/` binary crate
- Justfile with standard recipes (build, test, fmt, lint, run, doctor,
  install, release)
- CI on GitHub Actions: `cargo fmt`, `cargo clippy`, `cargo test`
- Release pipeline: tag-driven, multi-arch binaries
  (`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`)
- `scribe doctor` — verifies ffmpeg, whisper-cli, model presence; reports
  versions
- `scribe transcribe --input <file> --out-dir <dir>` — the core pipeline
- Integration tests against a fixture audio file
- Expanded design docs in this directory

## v0.2 — input adapters

Same pipeline downstream of ffmpeg; new front doors.

- **URL fetch.** `--input https://...` for direct audio URLs.
- **YouTube.** `--input <youtube-url>` shells out to `yt-dlp` to extract
  audio.
- **RSS enclosures.** Parse a podcast feed item, fetch the enclosure,
  transcribe.
- **stdin.** Pipe audio in. Useful for chaining with other tools.

## v0.3 — output formats

Same data, more shapes.

- **SRT / VTT** — subtitle formats, derived from the JSON segments.
- **Plain text** — `--format txt` for the simplest case.
- **Custom Markdown templates** — let users override the frontmatter / body
  shape via `.fsx` or a template file.

## v0.4 — operational concerns

- **Watchfolder mode.** Drop a file into a directory, get a transcript
  automatically.
- **Batch mode.** Take a directory of files, transcribe in parallel.
- **Resume.** If transcription dies mid-file, pick up where it left off.

## Deferred

- **Diarization** (speaker labels). Defer to `whisperx` (Python,
  pyannote-backed) when a specific transcript demands it. The aarch64 +
  CUDA + Python wheel risk is real and only worth taking on when actually
  needed.
- **Translation.** whisper.cpp supports it natively (`--translate`); not
  exposed until someone asks.
- **Real-time streaming.** Different problem (latency-bounded), different
  tool. Belongs in the future Murmur dictation utility, not here.
- **`libwhisper` / `libavformat` bindings.** Subprocess is fine until
  profiling says otherwise.

## Explicit non-goals

- Summarization, condensation, "highlights extraction." Verbatim contract;
  see [`01-architecture.md`](./01-architecture.md).
- Hosting a daemon or running as a service. Scribe is a CLI; long-lived
  hosting is Scryforge's job.
- Multi-tenant features, auth, user accounts. Not the right shape for a
  single-user utility.
- Cross-platform parity. Linux + CUDA is the primary target. macOS may work
  via CPU whisper.cpp, but is not actively supported.
