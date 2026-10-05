# Analyzer Engine Backend Review

**Date:** 2026-06-11 · **Branch:** `fix/waveforms-and-playback`
**Scope reviewed:** Rust orchestration (`pioneer-test-ui/src/main.rs`), Python engine
(`analysis/src/fourfour_analysis/` — CLI, backends, DSP modules), storage
(`pioneer-library` `set_analysis`/`get_analysis`).

The app's analysis path is:
`analyze_tracks`/`analyze_track_ids` (Tauri) → queue/coordinator → spawn
`python -m fourfour_analysis analyze <path> --backend deeprhythm_essentia --json`
per track (max 3 in flight, 120s timeout) → `parse_analyzer_stdout` →
`python_result_to_analysis` → `LocalLibrary::set_analysis` (ANLZ DAT/EXT files + SQLite index).

---

## High severity

### 1. Python analysis failures are stored as successful, zeroed analyses
`cli.py:_analyze_with_backend` catches all exceptions and returns
`{"status": "error", "error": ...}` **with exit code 0**. The Rust side
(`parse_analyzer_stdout`, main.rs:273) never checks `status` — the error dict has
no `bpm` key, so the backend-keyed fallback (`obj.values().find(is_object)`)
returns the error object itself, and `python_result_to_analysis` converts it to
`bpm=0.0, key="", empty beats/waveform`. That gets persisted via `set_analysis`,
the track flips to `has_analysis=true`, and it never appears in
`get_unanalyzed_track_ids` again. **A failed track silently becomes a permanently
"analyzed" track with a flat waveform and BPM 0.** This is very likely a
contributor to the degenerate-PWAV `#waveform-bug` the code comments mention.

Fix: check `status`/`error` in `parse_analyzer_stdout` and return `Err`;
optionally also have the CLI exit non-zero on error.

### 2. Partial failures (BPM/key None) also stored as success
Inside `DeepRhythmEssentiaBackend.analyze_track`, if DeepRhythm returns nothing,
`bpm=None`, beats stay empty, cues skipped — but the result is still `status: ok`.
Rust does `unwrap_or(0.0)` / `unwrap_or("")` and persists. Same permanent-zero
problem as #1, just per-feature. There is no re-queue or "failed" state in the
library schema at all — consider storing analysis status, or refusing to persist
a result with `bpm == 0 && beats.is_empty()`.

### 3. Beat grid downbeat anchor is discarded
`lexicon_bpm._generate_beats` returns `(beats, anchor_idx)` where `anchor_idx`
marks the detected first musical beat, specifically so bar numbering can align
to it (beats are extended *backward* from the anchor to t=0).
`deeprhythm_essentia.py:83` throws it away (`beats, _anchor_idx = ...`) and then
assigns `bar_position=(i % 4) + 1` from index 0 (line 116-118). Whenever
`len(beats_backward) % 4 != 0`, **every downbeat in the track is shifted by 1–3
beats** — wrong bar markers in the app UI and wrong phrase/bar display on CDJs.

Fix: `bar_position = ((i - anchor_idx) % 4) + 1`.

---

## Medium severity

### 4. Energy: transient-density feature is mathematically dead
`lexicon_energy.py`: `density = strong_beats / duration * 60`. With 14ms
segments the theoretical max is ~71 strong beats/sec → density ≤ ~4,300. Then
`score = (density − 550_000) / 150_000` — always negative, always clamped to 0.
The 0.5-weighted density feature contributes nothing, ever; energy is actually
`0.5*rms + 0.3*tempo` (weights also sum to 1.3, not 1.0, per the docstring's
"50/30/50"). The constants were presumably ported from a different unit
(samples, not beats/min). Either fix the normalization or delete the feature —
right now it's misleading dead weight that also costs a full-track RMS pass.

### 5. Coordinator race: queued work can be silently dropped
`run_track_analysis` (main.rs:561): if `try_begin()` fails, items are merged into
the *active* run's queue. But the owning run does `register_run` **after**
`try_begin`, and `end_run` clears the queue only when the guard drops — two
windows exist where `running == true` but `active_queue` is `None`/stale:
- after a loser's `try_begin` fails but before the winner's `register_run`
- after the winner's drain loop exits but before `RunGuard` drops

In both, `merge_into_analysis_queue` → `with_queue` returns `None` and the items
vanish without error; the command returns as if the work was queued. Narrow
window, but it's the kind of "analysis randomly didn't run" bug that's
unreproducible later. Fix: make `try_begin` + `register_run` atomic (register the
queue before flipping `running`, or hold one lock for both), and have the loser
retry `try_begin` when `with_queue` returns `None`.

### 6. `analyze_track_python` command is stale and broken
main.rs:965 parses stdout as `Vec<serde_json::Value>` (legacy array format), but
the CLI now emits a backend-keyed object — this command fails on parse if ever
called. The frontend doesn't invoke it (checked `app/src`). Delete it, along with
the hand-rolled 100ms-poll thread-scope timeout inside it.

### 7. Benchmark cache/record drops waveform fields
`base.py:analyze_track_cached` rebuilds `AnalysisResult` without
`waveform_overview`, `waveform_preview`, `waveform_fft_bands`;
`cache.py:_dict_to_record` likewise. Cached benchmark results silently lose
those fields, so any future waveform scoring or raw-result inspection from cache
is wrong. Pass through all fields (use `dataclasses.replace`) in both places.

### 8. Registry: duplicate `final_stack` key
`registry.py:ANALYSIS_VARIANTS` defines `"final_stack"` twice (lines 14 and 45);
the second silently overwrites the first and the two disagree about
`heavy_deps`. `load_backend` also has a duplicate dead `elif backend_key ==
"final_stack"` branch (line 105). Mostly harmless today but it means the variant
table lies about dependencies.

---

## Low severity / design notes

### 9. Per-track process spawn pays model-load cost every time
Each track spawns a fresh Python process that imports torch, loads the
DeepRhythm model, and imports essentia — several seconds of fixed overhead per
track, ×3 concurrent processes (~GBs of RAM). A persistent worker process (stdin
job loop) or batch invocation would cut analysis wall time substantially. The
120s timeout exists mainly to cover this overhead.

### 10. Audio decoded 3+ times per track
In `deeprhythm_essentia`: `load_audio()` (ffmpeg/soundfile), DeepRhythm's own
internal load, and Essentia's `MonoLoader` each decode the full file. The
waveform path then resamples separately. One decode shared across features
would be a meaningful speedup, second only to #9.

### 11. DB + ANLZ writes happen under the library mutex on the async runtime
`run_track_analysis` holds the `SharedLibrary` `std::sync::Mutex` while writing
ANLZ files to disk (`set_analysis`). UI commands (e.g. `get_analysis_data`,
`load_state`) block during those writes. Fine at current scale; consider
`spawn_blocking` or a write queue if the library grows.

### 12. `analyze.py` orchestrator is a legacy parallel path
`analyze.py` (`analyze_track`/`analyze_batch`, exported from `__init__.py`) uses
`FinalStackBackend` + the separate `waveform.py` generators (`soundfile`-only —
fails on MP3 unless libsndfile ≥1.1) and emits the *old* `pioneer_3band_*` field
names. The app doesn't use it; only tests do. `FinalStackBackend` itself also
drops `waveform_overview`/`waveform_preview` from the Lexicon result. Either
delete the legacy path or fold it into the CLI path — two orchestrators that
disagree on output schema is how the dual-format parsing in
`parse_analyzer_stdout` accreted.

### 13. Cue detection edge cases (`lexicon_cues.py`)
- `_filter_sections` rounds `end_bar` down to 4-bar boundaries but never adjusts
  the *next* section's `start_bar`, so sections can overlap/gap after rounding.
- With no high-energy sections, "Breakdown" falls on the first low section,
  which is usually the intro.
- Emergency-loop step is a TODO.
All cosmetic for cue placement, none crash.

### 14. Misc
- `python_result_to_analysis`: `key=None` → stored as `""`; consider keeping it
  absent so UI can distinguish "unknown" from "empty".
- `_detect_bpm`: `float(result) if result else None` — would crash if
  DeepRhythm's `predict` ever returns a `(bpm, confidence)` tuple (some versions
  do); pin the dependency or handle both.
- `parse_analyzer_stdout` multi-backend fallback picks the first object in
  serde_json's (alphabetical) map order — fine today with a single backend, but
  fragile.
- Energy docstring weights ("50/30/50%") don't match the claim of percentages.

## Verified-correct things worth noting
- JSON contract matches: Python `asdict` field names (`time_seconds`,
  `bar_position`, `loop_end_seconds`, `{r,g,b}` 0–127 color objects) line up with
  the Rust parser, including both current and legacy 3-band key names.
- Timeout/kill handling on the per-track subprocess (`kill_on_drop` + tokio
  timeout) is correct.
- Queue merge dedup (`merge_into_analysis_queue`) correctly skips duplicates and
  handles front-promotion; `prioritize_analysis` is sound.
- `set_analysis`/`get_analysis` round-trip: DAT + sibling EXT are both written
  and read back (PWV3/4 color parsed from EXT; DB index authoritative for
  bpm/key/energy); the `effective_preview` fallback for degenerate PWAV is a
  reasonable mitigation — but see #1, which is likely the *source* of those
  degenerate PWAVs.
- Beat-tempo inference in Rust (per-beat BPM×100 from deltas) is sound.

## Not yet reviewed
- `reader/anlz.rs` + `writer/anlz.rs` byte-level round-trip fidelity (only the
  entry points were checked).
- `lexicon_key.py`, `pwv7_research.py`, `manifest.py`, `compare.py`,
  `scoring.py` internals.
- `groundtruth.normalize_key` correctness for all Essentia key spellings.

## Suggested priority
1. Fix #1/#2 (error masking) — small Rust+Python change, stops bad data entering libraries.
2. Fix #3 (anchor) — one-line Python change, directly visible in UI/CDJ bar markers.
3. Fix #5 (coordinator race) and delete #6 (dead command) in the same main.rs pass.
4. #4, #7, #8 as cleanup; #9/#10 as the next performance milestone.
