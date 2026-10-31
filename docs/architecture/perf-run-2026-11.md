# Perf run, November 2026

The measuring base for the "as fast as possible" work: startup at 10k-50k, track load, analysis at 10k+, UI during analysis and import at 10k+. Each later worker adds a dated "Run N" section **above the previous run** with the same budget table, so the top of this file is always the latest state. Older Q50 baseline and before/after: `perf-q50-2026-10-30.md`.

## Run 1: baseline at 10k and 25k (2026-10-31, `r1-perf10k`)

Code: `ov-1028` (`a3387c2`) plus this item's harness; no app behaviour changed. Box: `dev`, 16 cores, analysis at 12 threads (`FF_PERF_THREADS=12`), everything under `nice -n 10`. Load average: 10k run 7.5 at start, 10.1 at the end (quiet); 25k run 7.8 at start and **61** at the end (other workers), so the 25k analysis, bulk-UI and first frontend numbers were unusable and are handled as noted. Reports: `benchmark/results/q50/perf-10k-20261031-030402.json`, `perf-25k-20261031-031914.json`; raw in `~/.cache/q50-perf/raw-{10k,25k}-*`.

Fixture: 10k = 10,000 files, 7.3 GB (5,467 mp3, 1,998 m4a, 1,506 flac, 525 wav, 480 aiff), 8,132 with covers, 6,937 analysed, 2,582 rated, 791 coloured, 17 folders, 60 playlists. 25k = 25,000 files, 17.4 GB, same mix. Import into an empty library found **0 duplicates** at both sizes (the copies stay distinct, see "Why copies do not collapse"). Frontend startup used the real `load_state` JSON of the fixture library as the mock payload (`FF_PERF_STATE`), not synthetic rows.

### Budget table

`now` is what the budgets in `scripts/perf-budgets.json` are set to; x off = how far from the target.

| Metric | 10k now | 10k target | x off | 25k now | 25k target | x off |
|---|---:|---:|---:|---:|---:|---:|
| cold startup to first row (backend + headless) | 1,085 ms | 500 | 2.2 | 2,212 ms * | 900 | 2.5 |
| cold startup to interactive | 1,319 ms | 550 | 2.4 | 2,815 ms * | 1,000 | 2.8 |
| warm startup to first row | 694 ms | 300 | 2.3 | 1,660 ms * | 600 | 2.8 |
| warm startup to interactive | 748 ms | 330 | 2.3 | 1,748 ms * | 650 | 2.7 |
| backend `load_state`, warm | 108 ms | 60 | 1.8 | 248 ms | 150 | 1.7 |
| `load_state` payload | 7.9 MB | 4 | 2.0 | 19.8 MB | 10 | 2.0 |
| track load to first audio (backend) | 2.6 ms | 10 | met | 8.1 ms | 10 | met |
| full waveform detail, first time (5-7 min files) | 1,051 ms | 120 | **8.8** | 986 ms | 120 | **8.2** |
| full waveform detail, cached | 2.5 ms | 2 | 1.3 | 2.6 ms | 2 | 1.3 |
| expand to detail drawn (frontend) | 102 ms | 50 | 2.0 | 241 ms ** | 50 | 4.8 |
| analysis throughput, 5-7 min tracks | 9.0 trk/min | 30 | **3.3** | (10k sample) | 30 | 3.3 |
| time to analyse 10k tracks (extrapolated) | 18.5 h | 5.6 | **3.3** | (10k sample) | 5.6 | 3.3 |
| analysis first result | 16.8 s | 5 | **3.4** | (10k sample) | 5 | 3.4 |
| import of all fixture files, end to end (backend) | 18.0 s | 2.5 | **7.2** | 43.0 s ** | 6.0 | 7.2 |
| re-import the same folder (all already in library) | 1,894 ms | 600 | **3.2** | 4,896 ms ** | 1,500 | 3.3 |
| import UI click to rows (frontend) | 119 ms | 100 | 1.2 | 806 ms ** | 150 | 5.4 |
| scroll during analysis: frame p95 | 32.8 ms | 28 | 1.2 | not measured | | |
| scroll during analysis: frames dropped | 66 | 40 | 1.6 | not measured | | |
| scroll during analysis: long-task total | 57 ms | 0 | open | not measured | | |
| typing during analysis: worst keystroke | 150 ms | 100 | 1.5 | not measured | | |

\* 25k frontend startup from a startup-only rerun at load 36-50 (the in-suite one at load 60 gave 29.5 s cold first row with a 21.7 s first contentful paint, which is the harness injecting 20 MB under load, not the app). \** measured at load 50-60, expect these to come down on a quiet box. 25k analysis rows copy the 10k sample (same 12 files, the number does not depend on library size); the in-suite 25k analysis ran at load 61 and gave 1.3 trk/min. The 25k bulk-analysis UI spec hit its 300 s test timeout at load 60 and has no `now` yet. 50k: no fixture or numbers yet (`now` is null, so the budget column says unset).

### Where the time goes (10k)

- **Waveform detail, first time: 1,051 ms median, 1,212 p95** on the 5-7 min fixture files (mixed mp3, m4a, flac, wav, aiff). Q50 measured 339 ms on the dataset files, which are shorter and mostly mp3; this is the realistic case.
- **Import of 10k files: 18.0 s.** Plan 7.2 s (scan with lofty 6.6 s cold, 1.4 s warm; survey 0.4 s; duplicate plan 0.23 s) and commit 10.8 s, of which `add_tracks` is 10.5 s: 8,132 embedded covers (12 KB-850 KB) are stored full size inside the one transaction. Re-importing the same folder still costs 1.9 s, all of it tag reads (1.3 s) for files whose path is already in the library.
- **Analysis: 9 trk/min, first result 16.8 s, peak RSS 5.5 GB.** Per track (one at a time, 12 threads): total 12.5 s, predict (ONNX) 9.8 s = 79%, resample 1.2 s, mel 1.0 s, waveform bands 0.9 s, decode 0.45 s, key 0.4 s.
- **Startup.** Backend warm `load_state` 108 ms (build_track_infos 57, JSON 7.9 MB). Hidden cold cost: the bench's separate track-row query takes **490 ms** after a page-cache eviction (62 ms warm) and then warms the cache for `load_state`, so the cold composite understates a real cold start by up to about 0.4 s. Headless boot: cold first row 948 ms, interactive 1,181 ms, total blocking 348 ms, longest task 244 ms; warm 586 / 640 ms, longest task 123 ms; JS heap 54 MB.
- **UI during analysis:** scroll p95 32.8 ms against 25.9 ms idle, 66 frames lost against 27 idle, one 57 ms long task in the fast pass; worst keystroke 150 ms.

### Worst 5 against target (10k)

1. Full waveform detail, first time: 1,051 ms, 8.8x off 120 ms.
2. Import of 10k files end to end: 18.0 s, 7.2x off 2.5 s (covers in the commit transaction, cold tag scan).
3. Analysis first result: 16.8 s, 3.4x off 5 s.
4. Analysis throughput: 9 trk/min (18.5 h for 10k, extrapolated), 3.3x off 30 trk/min.
5. Re-import of an already-imported folder: 1.9 s, 3.2x off 0.6 s (tags read again for known paths).

Next after those: startup to first row and interactive at 2.2-2.4x (all four), the 7.9 MB payload at 2x, expand to detail at 2x.

### Mac script dry run

`scripts/mac/perf-night.sh --dry-run --build --size 10k` ran on the box end to end (backend suite on the fixture, launch commands printed, summary written with PASS/FAIL against the targets). The probe itself was exercised for real on the box: the release app (`cargo build --release`, frontend served by `vite preview` on :5200 because a plain cargo build uses `devUrl`) under `xvfb-run` with a sandboxed HOME on the 10k library, three runs (first launch, warm with track load and idle scroll, analysis with scroll). All marks arrived (`setup_done`, `load_state_*`, `first_row`, `interactive`, `track_load` 5/5, `scroll_idle`, `scroll_during_analysis`, `probe_done`) and the summary table filled in. The numbers from that run are WebKitGTK with software rendering in Xvfb at load 40-60 (first row 4.9-7.7 s, scroll frames 240-640 ms) and are not a measurement of anything; they only prove the pipeline. The Mac bundle from `cargo tauri build` embeds the frontend, so no server is needed there.


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
| `library/` | `library.db` built by `bench_startup fixture-lib` from `music/` with the app's own import scan (lofty tag reads, embedded covers stored): 70% analysed (synthetic `AnalysisResult` with ANLZ files, 3-8 min nominal grids and waveforms), 30% unanalysed; 25% with a rating (user-edits sidecar), 8% with a colour (imported sidecar); 7 top folders and up to 2 subfolders each (17 folders at 10k), about 60 playlists of 12-800 tracks and one of N/5. |
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

Steps: build or use the installed app; `scripts/perf.sh --size 10k` with the frontend skipped (generates the fixture on the Mac, runs the backend suite); then launches the real app with `HOME` pointed at a scratch folder holding a copy of the fixture library (the real config, library and caches are never touched): run 1 (first launch), runs 2-3 (warm OS file cache; each run still gets a fresh HOME, so WebKit caches start empty; track load and idle scroll), and one analysis run (starts `analyze_tracks`, scrolls the table for 10 s while it runs, cancels). Screenshots at interactive and mid-analysis (`screencapture -x`; needs screen recording permission for the shell). Output: `build/mac/perf-<date>/summary.md` with PASS/FAIL against the targets, `backend.json`, `app-run-*.jsonl`, `*.png`, logs.

The app side is the perf probe (`pioneer-test-ui/src/perf_probe.rs` and `perf_probe.js`), active only when `FF_PERF_LOG` is set: Rust appends JSON lines (`setup_done`, `ready`, `load_state_start`, `load_state_built`, and every probe mark) with ms since process start; the probe script (added with `append_invoke_initialization_script`, configured by `FF_PERF_PROBE`) reports first row, interactive (first of 10 frames under 50 ms after the first row), track load to `ff:player-ready`, and scroll frame stats. WKWebView has no long-task API, so frames over 50 ms stand in for long tasks there. Without `FF_PERF_LOG` nothing is added and nothing is written.
