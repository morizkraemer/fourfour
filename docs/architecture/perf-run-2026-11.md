# Perf run, November 2026

The measuring base for the "as fast as possible" work: startup at 10k-50k, track load, analysis at 10k+, UI during analysis and import at 10k+. Each later worker appends a dated section **below the newest one** with the same budget table, so the top of this file is always the latest state. Older Q50 baseline and before/after: `perf-q50-2026-10-30.md`.

<!-- RUN-SECTIONS -->

## How to run

```bash
scripts/perf.sh --size 10k      # or 25k, 50k; prints the full table, then the budget table
scripts/perf.sh                 # the old Q50 setup (synthetic library + dataset files), no budgets
```

The first `--size` run generates the fixture (see below) and takes longer; later runs reuse it. The report JSON lands in `benchmark/results/q50/perf-<size>-<stamp>.json` with a `budgets` array (`value`, `now`, `target`, `budget` pass/FAIL, `goal` met/open, `off_by`). Knobs as before (`FF_PERF_SKIP`, `FF_PERF_ANALYSIS`, `FF_PERF_THREADS`, `FF_PERF_JOBS`, `FF_FIXTURE_ROOT`).

- **Budget** = `now` in `scripts/perf-budgets.json` plus 25% noise. A FAIL means a regression against the measured state; refresh `now` when a speed fix lands.
- **Target** = where the speed work should get to; `x off` = how far the measurement is from it. Each target has a one-line reason in the JSON (`why`).
- Composite startup rows are backend `load_state` + headless frontend boot (the mock answers in-page), so they are an estimate of the real app; the Mac script measures the real thing.

## The fixture (`scripts/ff-fixture.py`)

`python3 scripts/ff-fixture.py 10k --bench <target>/release/bench_startup` (perf.sh does this) writes `~/.cache/ff-fixture/<size>/`, out of the repo:

| Part | What |
|---|---|
| `pool-v1/` (shared by all sizes) | 120 cover images (500-1200 px JPEG, 12 KB-850 KB) and 300 distinct synthetic masters, 7-13 s: 150 mp3 (192-320k), 60 m4a (AAC 256-320k), 50 flac (16/24-bit), 20 wav, 20 aiff; 44.1 or 48 kHz; 85% of masters have embedded art (wav never). Plus 24 full-length masters (300-420 s, all five formats) for analysis and track load. Each master is its own tempo (85-176 BPM), root note, major or minor chord, kick, offbeat hat and bass. |
| `music/` | N files. Each is a **real copy** of a master with its own tags written by `ffmpeg -c copy` (audio packets untouched): title, artist (about N/6 artists, skewed so some have many tracks, 15% "feat.", accents, Cyrillic, CJK, long titles), album, album artist (12% compilations), genre (26), year, track number, label, comment, BPM and key tags on some. Folder layout `Genre/A/Artist/NN Artist - Title.ext`. Mix: 55% mp3, 20% m4a, 15% flac, 5% wav, 5% aiff. The 24 long masters are copied in as `_Long Mixes/`. |
| `library/` | `library.db` built by `bench_startup fixture-lib` from `music/` with the app's own import scan (lofty tag reads, embedded covers stored): 70% analysed (synthetic `AnalysisResult` with ANLZ files, 3-8 min nominal grids and waveforms), 30% unanalysed; 25% with a rating (user-edits sidecar), 8% with a colour (imported sidecar); 7 top folders with subfolders, about 70 playlists of 12-800 tracks and one of N/5. |
| `long.txt` | The 24 long files: the track-load (12) and analysis (12) samples. |
| `fixture.json` | Size, seed, version, file count, bytes, format split. A matching manifest makes a rerun a no-op; `--force` rebuilds. |

**Why copies do not collapse.** The library's duplicate check (`pioneer-library/src/duplicates.rs`) has no audio fingerprint: it matches the same path, the same file name and size, or the same normalised artist and title within 2 s. Every fixture file has a unique artist and title pair (the generator keeps a set and redraws on a clash) and a unique file name, so none of the three rules fires, and the import plans N new tracks. Symlinks were not used: they would share the master's tags and every copy would be a "same tags" duplicate. The waveform-detail cache keys on path, size and mtime, so copies do not share cache entries either. What does repeat is the audio itself (300 distinct clips): analysis results of copies of one master are the same, which does not matter for timing.

**Limits.** Clips are short (7-13 s), so the 10k `music/` is about 7 GB instead of 90 GB, and the analysed rows carry nominal 3-8 min analysis blobs on top of a short file. Analysis throughput and track load therefore use the 24 full-length files, and "time to analyse 10k" is extrapolated from that timed sample (10,000 / tracks per minute), not a full 10k run. Synthetic audio is not music: decode, resample and model cost are per second of audio and do not care, but BPM/key accuracy is not measured here.

## Mac night (`scripts/mac/perf-night.sh`)

Prepared, not run (a visible app window needs Merlin's go). On the Mac, from the repo root:

```bash
scripts/mac/perf-night.sh --size 10k --app /Applications/fourfour.app   # or --build for an unsigned bundle
ssh mbp 'cd ~/dev/fourfour && scripts/mac/perf-night.sh --size 10k --build'   # from dev, or as a mac-queue entry
```

Steps: build or use the installed app; `scripts/perf.sh --size 10k` with the frontend skipped (generates the fixture on the Mac, runs the backend suite); then launches the real app with `HOME` pointed at a scratch folder holding a copy of the fixture library (the real config, library and caches are never touched): run 1 (first launch), runs 2-3 (warm; track load and idle scroll), and one analysis run (starts `analyze_tracks`, scrolls the table for 10 s while it runs, cancels). Screenshots at interactive and mid-analysis (`screencapture -x`; needs screen recording permission for the shell). Output: `build/mac/perf-<date>/summary.md` with PASS/FAIL against the targets, `backend.json`, `app-run-*.jsonl`, `*.png`, logs.

The app side is the perf probe (`pioneer-test-ui/src/perf_probe.rs` and `perf_probe.js`), active only when `FF_PERF_LOG` is set: Rust appends JSON lines (`setup_done`, `ready`, `load_state_start`, `load_state_built`, and every probe mark) with ms since process start; the probe script (added with `append_invoke_initialization_script`, configured by `FF_PERF_PROBE`) reports first row, interactive (first of 10 frames under 50 ms after the first row), track load to `ff:player-ready`, and scroll frame stats. WKWebView has no long-task API, so frames over 50 ms stand in for long tasks there. Without `FF_PERF_LOG` nothing is added and nothing is written.
