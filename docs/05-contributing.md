# Contributing

Dev environment setup and contribution conventions for Scribe.

## Prerequisites

Scribe is a Rust CLI that shells out to `ffmpeg` and `whisper-cli`. You
need all three on your `PATH` (or pointed at via env vars — see
[`04-cli-reference.md`](./04-cli-reference.md)).

### Rust

Install via [rustup](https://rustup.rs/):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

The workspace targets the **stable** toolchain, edition 2021. A
`rust-toolchain.toml` may pin a specific stable release once issue
[#1](https://github.com/raibid-labs/scribe/issues/1) lands; until then,
current stable is fine.

### just

`just` runs the project's task recipes. Install:

```sh
cargo install just
```

The `justfile` is tracked under issue
[#2](https://github.com/raibid-labs/scribe/issues/2) (pending — recipes
do not exist yet). The intended recipe set:

- `just build` — `cargo build --all`
- `just test` — `cargo test --all`
- `just fmt` — `cargo fmt --all`
- `just fmt-check` — `cargo fmt --all -- --check`
- `just lint` — `cargo clippy --all-targets --all-features -- -D warnings`
- `just check` — `cargo check --all`
- `just run *args` — `cargo run -p scribe -- {{args}}`
- `just doctor` — `cargo run -p scribe -- doctor`
- `just install` — `cargo install --path crates/scribe`
- `just clean` — `cargo clean`

Until #2 merges, run the underlying `cargo` commands directly.

### ffmpeg

Debian / Ubuntu:

```sh
sudo apt install ffmpeg
```

macOS:

```sh
brew install ffmpeg
```

### whisper.cpp

Scribe expects a working `whisper-cli` binary on `PATH` or pointed at via
`$SCRIBE_WHISPER_BIN`. The simplest path is to follow the build
instructions in `~/raibid-labs/voice-stuff/docs/setup.md` — that repo's
build (CUDA-enabled, against a known-good whisper.cpp commit) is the
canonical engine for the raibid-labs stack.

Until voice-stuff is published to GitHub, those local instructions are
the canonical reference.

### Models

Whisper model files (`.bin`) are also managed by voice-stuff. Use its
`just fetch-model <name>` recipe to download a specific model:

```sh
cd ~/raibid-labs/voice-stuff
just fetch-model large-v3
```

Models land in `~/raibid-labs/voice-stuff/models/`. Point Scribe at them
via `$SCRIBE_MODEL_PATH` (or rely on the default).

## Building and testing

```sh
cargo build --all
cargo test --all
```

## Code style

Required before pushing:

```sh
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
```

Clippy must be clean — CI enforces `-D warnings`.

## PR conventions

- Branch from `main`. One issue per branch where possible.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/):
  `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`, `ci:`.
- Reference the issue in the PR body (`Closes #N`).
- Do **not** merge your own PR. Hand off to a reviewer.
- Each issue's `Rules of engagement` block names the branch
  (`feat/<thing>`, `docs/<thing>`, etc.) and the base branch (`main`).
  Follow it.

## Adding a new input adapter

Targeted at v0.2 — see [`02-roadmap.md`](./02-roadmap.md).

The pipeline downstream of ffmpeg is fixed. A new input adapter's job is
to land an audio (or audio-bearing) file at a path ffmpeg can read.
Shape:

1. Add a variant or matcher to the input-resolution layer (`config.rs` /
   a future `input.rs`) that recognizes the new source kind.
2. Implement a fetch step that produces a local file path. For URLs, a
   plain HTTP GET to a temp file. For YouTube, shell out to `yt-dlp`.
   For RSS, parse the feed and fetch the enclosure URL.
3. Hand the resulting path to the existing pipeline. Do not reinvent the
   ffmpeg / whisper steps.
4. Add an integration test against a small fixture for the new input
   kind.

## Adding a new output format

Targeted at v0.3 — see [`02-roadmap.md`](./02-roadmap.md).

The JSON sidecar is the source of truth for any timestamped format. For
plain text or alternative markdown shapes, derive from the verbatim text
body. Shape:

1. Add a `--format <NAME>` flag (or a `--out-format` repeat flag) on
   `transcribe`.
2. Implement the format writer in a new module under `output/` (e.g.
   `output/srt.rs`, `output/vtt.rs`). Read from the JSON segments, not
   from the markdown.
3. Preserve the verbatim contract — the new format must reflect the
   words whisper.cpp emitted, with no rewriting.
4. Document the format in [`04-cli-reference.md`](./04-cli-reference.md).
