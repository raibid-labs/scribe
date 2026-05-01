# Test fixtures

Audio fixtures used by `tests/transcribe.rs`. Bundled into the repo so the
integration test is self-contained on hosts that have ffmpeg + whisper-cli +
a model installed.

## `jfk.wav`

- **Source:** copied verbatim from the upstream
  [`whisper.cpp`](https://github.com/ggerganov/whisper.cpp) repository at
  `samples/jfk.wav` (commit reachable via `make samples`). The whisper.cpp
  project distributes it as a canonical short-audio test sample.
- **Format:** RIFF WAVE, 16-bit signed PCM, mono, 16 kHz, ~11 seconds,
  352 KB.
- **Audio content:** an excerpt of John F. Kennedy's 1961 inaugural address —
  the famous "ask not what your country can do for you" line. The original
  recording is a work of the United States federal government, which under
  [17 U.S.C. § 105] is **not subject to copyright protection in the United
  States** (i.e., public domain in the U.S.).
- **Surrounding code license:** the whisper.cpp repository itself is MIT
  ([LICENSE](https://github.com/ggerganov/whisper.cpp/blob/master/LICENSE),
  Copyright (c) 2023-2026 The ggml authors). Redistribution of the WAV
  alongside Scribe (also MIT-or-Apache-2.0) is therefore unencumbered: the
  audio content is public domain and the surrounding project license is
  permissive.

[17 U.S.C. § 105]: https://www.law.cornell.edu/uscode/text/17/105

### Why this fixture

- Small (≤ 1 MB target — actual 344 KB).
- Already in the exact format (16 kHz mono PCM s16le) that the pipeline's
  ffmpeg step would produce, so the resample step is a near no-op and the
  test exercises end-to-end orchestration without needing extra
  format-conversion overhead.
- Famous, recognizable transcript. The integration test asserts the body
  contains the substring `country`, which gives high confidence the real
  whisper.cpp inference path executed (rather than, say, a stub or an
  empty-output edge case).
