# scribe

The `scribe` binary crate is the user-facing CLI for the [Scribe](../../README.md)
verbatim audio-to-Markdown transcription utility. It hosts the top-level
`scribe transcribe` and `scribe doctor` commands; the pipeline itself
(ffmpeg + whisper.cpp subprocess invocations, dual Markdown + JSON output)
lands in subsequent issues. See the [top-level README](../../README.md)
and [`docs/01-architecture.md`](../../docs/01-architecture.md) for design
rationale.
