//! Filename slug, YAML frontmatter, and final markdown writing.
//!
//! All the *naming* logic lives here so the pipeline orchestrator doesn't
//! have to. Two responsibilities:
//!
//! 1. Turn an input filename + a UTC instant into a stable basename of the
//!    form `<slug>-<YYYYMMDD-HHMMSS>` that both the markdown and the JSON
//!    sidecar share.
//! 2. Wrap the verbatim text whisper-cli emitted in YAML frontmatter and
//!    write the resulting `.md`. The text body is byte-for-byte the same
//!    as the input — no whitespace munging, no rewriting.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, SecondsFormat, Utc};

/// Metadata captured before whisper-cli runs, threaded through to the
/// frontmatter writer.
///
/// Kept separate from `Config` so frontmatter assembly is unit-testable
/// without spinning up a real pipeline.
#[derive(Debug, Clone)]
pub struct FrontmatterMeta<'a> {
    /// Absolute path of the original `--input` file, written verbatim into
    /// the `source:` field.
    pub source: &'a Path,
    /// Display title — currently just the input filename stem.
    pub title: &'a str,
    /// Capture instant; serialized as ISO-8601 in UTC with second precision.
    pub created: DateTime<Utc>,
    /// Source file duration in seconds, from `ffprobe`.
    pub duration_sec: f64,
    /// Logical model name (e.g. `base.en`).
    pub model: &'a str,
    /// Filename of the JSON sidecar, *not* a path — relative to the same
    /// directory as the markdown.
    pub sidecar_filename: &'a str,
}

/// Lower-case ASCII alphanumerics; everything else collapses to a single `-`.
/// Leading/trailing dashes are trimmed. Empty input → `"untitled"`.
///
/// Examples:
/// - `"My Podcast Ep 42"` → `"my-podcast-ep-42"`
/// - `"jfk.wav"` (stem `"jfk"`) → `"jfk"`
/// - `"   "` → `"untitled"`
/// - `"日本語"` → `"untitled"` (no ASCII alphanumerics survive)
pub fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_dash = true; // suppress leading dashes
    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    // Trim trailing dash.
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "untitled".to_string()
    } else {
        out
    }
}

/// Format a UTC instant as `YYYYMMDD-HHMMSS` for use in transcript filenames.
pub fn timestamp_for_filename(now: DateTime<Utc>) -> String {
    now.format("%Y%m%d-%H%M%S").to_string()
}

/// Compute the shared basename for the `.md` and `.json` outputs.
///
/// `input_stem` is the filename without extension, as you'd get from
/// `Path::file_stem`. `now` is the capture time — usually `Utc::now()`,
/// but injectable for deterministic tests.
pub fn basename(input_stem: &str, now: DateTime<Utc>) -> String {
    format!("{}-{}", slugify(input_stem), timestamp_for_filename(now))
}

/// Render the YAML frontmatter block exactly as specified in issue #6 +
/// `docs/04-cli-reference.md`.
///
/// The block is bracketed by `---\n` lines and ends with a blank line so
/// the body that follows starts on a new paragraph. The `created` field is
/// ISO 8601 in UTC with second precision (e.g. `2026-04-28T14:30:00Z`).
/// `duration_sec` is rendered with one decimal place to match the docs
/// example.
pub fn render_frontmatter(meta: &FrontmatterMeta<'_>) -> String {
    let created = meta.created.to_rfc3339_opts(SecondsFormat::Secs, true);
    format!(
        "---\n\
         type: transcript\n\
         source: {source}\n\
         title: {title}\n\
         created: {created}\n\
         duration_sec: {duration:.1}\n\
         model: {model}\n\
         sidecar: {sidecar}\n\
         tags: [type/transcript]\n\
         ---\n\n",
        source = meta.source.display(),
        title = meta.title,
        created = created,
        duration = meta.duration_sec,
        model = meta.model,
        sidecar = meta.sidecar_filename,
    )
}

/// Write `<out_dir>/<basename>.md` containing `frontmatter` followed by
/// `body`. The body is written byte-for-byte; this function is the single
/// gateway through which the verbatim contract is honored.
pub fn write_markdown(
    out_dir: &Path,
    basename: &str,
    frontmatter: &str,
    body: &str,
) -> Result<PathBuf> {
    let path = out_dir.join(format!("{basename}.md"));
    let mut contents = String::with_capacity(frontmatter.len() + body.len());
    contents.push_str(frontmatter);
    contents.push_str(body);
    fs::write(&path, contents)
        .with_context(|| format!("failed to write markdown to {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("My Podcast Ep 42"), "my-podcast-ep-42");
    }

    #[test]
    fn slugify_collapses_runs() {
        assert_eq!(slugify("hello   world!!!foo"), "hello-world-foo");
    }

    #[test]
    fn slugify_strips_edges() {
        assert_eq!(slugify("---hello---"), "hello");
        assert_eq!(slugify("!!!a!!!"), "a");
    }

    #[test]
    fn slugify_lowercases() {
        assert_eq!(slugify("CamelCase"), "camelcase");
    }

    #[test]
    fn slugify_keeps_digits() {
        assert_eq!(slugify("Episode 007"), "episode-007");
    }

    #[test]
    fn slugify_empty_becomes_untitled() {
        assert_eq!(slugify(""), "untitled");
        assert_eq!(slugify("   "), "untitled");
        assert_eq!(slugify("---"), "untitled");
    }

    #[test]
    fn slugify_drops_non_ascii() {
        // Spec says non-alphanumerics → '-'. We treat non-ASCII as
        // non-alphanumeric so the slug stays URL-safe.
        assert_eq!(slugify("日本語"), "untitled");
        assert_eq!(slugify("café-au-lait"), "caf-au-lait");
    }

    #[test]
    fn timestamp_format_is_compact() {
        let t = Utc.with_ymd_and_hms(2026, 4, 28, 14, 30, 0).unwrap();
        assert_eq!(timestamp_for_filename(t), "20260428-143000");
    }

    #[test]
    fn basename_combines_slug_and_ts() {
        let t = Utc.with_ymd_and_hms(2026, 4, 28, 14, 30, 0).unwrap();
        assert_eq!(
            basename("My Podcast Ep 42", t),
            "my-podcast-ep-42-20260428-143000"
        );
    }

    #[test]
    fn frontmatter_matches_spec() {
        let t = Utc.with_ymd_and_hms(2026, 4, 28, 14, 30, 0).unwrap();
        let meta = FrontmatterMeta {
            source: Path::new("/abs/path/to/My Podcast Ep 42.mp3"),
            title: "My Podcast Ep 42",
            created: t,
            duration_sec: 3127.4,
            model: "large-v3",
            sidecar_filename: "my-podcast-ep-42-20260428-143000.json",
        };
        let got = render_frontmatter(&meta);
        let expected = "---\n\
                        type: transcript\n\
                        source: /abs/path/to/My Podcast Ep 42.mp3\n\
                        title: My Podcast Ep 42\n\
                        created: 2026-04-28T14:30:00Z\n\
                        duration_sec: 3127.4\n\
                        model: large-v3\n\
                        sidecar: my-podcast-ep-42-20260428-143000.json\n\
                        tags: [type/transcript]\n\
                        ---\n\n";
        assert_eq!(got, expected);
    }

    #[test]
    fn frontmatter_renders_integer_duration_with_one_decimal() {
        let t = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let meta = FrontmatterMeta {
            source: Path::new("/x.mp3"),
            title: "x",
            created: t,
            duration_sec: 11.0,
            model: "base.en",
            sidecar_filename: "x-20260101-000000.json",
        };
        let got = render_frontmatter(&meta);
        assert!(got.contains("duration_sec: 11.0\n"));
    }

    #[test]
    fn write_markdown_preserves_body_verbatim() {
        let tmp = tempfile::tempdir().unwrap();
        let body = " leading space and\n trailing newline preserved \n";
        let path = write_markdown(tmp.path(), "x-1", "---\nhdr\n---\n\n", body).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents, format!("---\nhdr\n---\n\n{body}"));
    }
}
