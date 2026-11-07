# Perf run, November 2026

The measuring base for the "as fast as possible" work: startup at 10k-50k, track load, analysis at 10k+, UI during analysis and import at 10k+. Dated sections from the workers, newest last; each should restate the budget table from `scripts/perf.sh --size` (Run 1 below). Q50 baseline and before/after: `perf-q50-2026-10-30.md`. The reference sections (how to run, the fixture, the Mac night) follow Run 1.

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

## R2 startup at 10k-50k (2026-10-31)

Branch `worktree-agent-afd8c66b477699041` from `ov-1028`. Linux `dev` box, headless Chromium, release `bench_startup`.

**Changed**

- Boot snapshot ("cache the last open list"): the first screen of All Tracks in the current sort (about 40 rows, display fields and row preview) is kept in `localStorage` (`fourfour.bootSnapshot.v1`), rewritten 1.5 s after the list changes. At boot it paints through the normal row pool while the real load runs; the table is `inert` until then. The snapshot is only used for the same library path and the same layout and sort; the real `load_state` answer replaces it completely, so edited, deleted or outside-changed tracks never outlive the load.
- `load_state` no longer carries row previews (8.5 MB to 3.2 MB at 10k, 43 to 16 MB at 50k). Rows on screen fetch theirs with `get_row_previews(trackIds)`, one batched call per task, once per track object.
- `load_state` starts from `main.ts` before the UI mounts instead of after two other IPC round trips.

**Backend** (`bench_startup run`, median of 3 warm / 1 cold after page-cache eviction, load 6-9)

| Tracks | `load_state` warm before / after | cold before / after | of which JSON before / after | payload before / after |
|---|---:|---:|---:|---:|
| 10k | 104 / 85 ms | 114 / 86 ms | 46 / 18 ms | 8.5 / 3.2 MB |
| 25k | 247 / 200 ms | 255 / 186 ms | 106 / 42 ms | 21.4 / 8.1 MB |
| 50k | 488 / 370 ms | 504 / 387 ms | 209 / 85 ms | 42.9 / 16.3 MB |

Process start (bench binary) 10-20 ms, DB open 1.5 ms warm / 8-12 ms cold. The rest of `load_state` is the list query and the sidecar label scan, run side by side (50k: 240 and 230 ms).

**Frontend** (`app/smoke/perf-startup.spec.ts`: the real `load_state` JSON served over HTTP and parsed in the page, the backend time above simulated; before = `ov-1028` app, after = this branch; median of 3; load about 9). "First launch" has no snapshot yet; "cold" is a fresh browser with the snapshot from a previous session; "warm" is a reload.

| Tracks | Case | First row before / after | Real rows before / after | Interactive before / after |
|---|---|---:|---:|---:|
| 10k | first launch | 827 / 712 ms | 827 / 712 ms | 885 / 879 ms |
| 10k | cold | 781 / 437 ms | 781 / 885 ms | 832 / 1,050 ms |
| 10k | warm | 554 / 264 ms | 554 / 616 ms | 616 / 678 ms |
| 25k | first launch | 1,102 / 926 ms | 1,102 / 926 ms | 1,187 / 1,276 ms |
| 25k | cold | 1,117 / 460 ms | 1,118 / 1,021 ms | 1,188 / 1,021 ms |
| 25k | warm | 884 / 248 ms | 884 / 728 ms | 942 / 766 ms |
| 50k | first launch | 1,852 / 1,660 ms | 1,852 / 1,660 ms | 1,909 / 1,758 ms |
| 50k | cold | 1,844 / 561 ms | 1,844 / 1,741 ms | 2,155 / 1,810 ms |
| 50k | warm | 1,556 / 320 ms | 1,556 / 1,288 ms | 1,665 / 1,338 ms |

Split at 50k warm, after: bundle eval to first IPC about 170 ms, `load_state` requested at 150-190 ms, answered at 770 ms, then mapping 330 ms (four accessor properties per track object put each track on V8's slow object path; a class would be 4x faster but would drop the per-row `$state` proxy), list set 35-90 ms, first real row at 1,290 ms.

**Read with care.** One quiet pair (load about 9); three more pairs ran at load 30-75 and swing 2-3x either way, so they are not in the table. The 10k cold/warm rows show the snapshot costs about 60-100 ms on the real rows (the page paints the snapshot first); the snapshot rows moved into the shared row pool after this pair to cut the swap, not re-measured quiet. Real-app IPC, WKWebView parse and window show are not measured here; r1's Mac script covers them.

**Not done / next**

- Map tracks without per-object accessors (about 300 ms at 50k), keeping row reactivity.
- List query without the preview blob column (about 15% of the query at 50k, tried, not kept).
- Sidecar label scan: 50k directory reads on every start (230 ms at 50k).
- Window shown on first frame: the window already opens on `#0a0a0a`, so no white flash; not changed.
- Google Fonts stylesheet in `index.html` can hold the first script until it loads; no effect measured here (fast network), could stall an offline Mac.

## R2b startup at 10k-50k (2026-10-31)

Branch `worktree-agent-a0ac75e69e0fcf50f` from `ov-1028`. Linux `dev` box, headless Chromium, release `bench_startup`, same r2 bench libraries and payloads.

**Changed**

- Track mapping: track objects are plain data. The four derived row fields (`cover`, `rowCover`, `status`, `bands`) no longer sit on each object as getters; one shared proxy handler around each row's `$state` proxy computes them on read. Per-row reactivity, `'bands' in row`, identity in other state and the `bumpLibraryTracks(changedIds)` contract are unchanged (`library-rows.test.ts`). Mapping 50k tracks: about 330 ms to 40-115 ms. Map chunks yield through a `MessageChannel` instead of nested `setTimeout(0)`, which Chromium clamps to 4 ms after five levels.
- Sidecar labels: `load_state` reads ratings and colours from `track-labels.json` next to `library.db` (the last scan), trusted while the `anlz` folder's mtime is unchanged (no track folder added or removed). Setting a rating updates the cache; a Rekordbox import drops it. After the list is on screen the app calls `refresh_track_labels`, which scans every track folder off the UI thread and returns only what changed outside the app; rows take it through `bumpLibraryTracks`. First launch (no cache) scans as before.
- `load_state`'s list query no longer reads the row preview blobs it then dropped.
- Fonts: Geist and Geist Mono are bundled (`app/src/assets/fonts`, OFL 1.1, same faces, weights and unicode ranges as the Google stylesheet). Nothing at start touches the network.
- Search text warmup after the first rows builds each haystack with one `toLowerCase` (less GC at 50k).

**Backend** (`bench_startup run`, before = `ov-1028` binary, after = this branch, interleaved; median of 3 cold (page cache of the library files dropped with `posix_fadvise`) and 5 warm runs; load 14.2-16.3)

| Tracks | `load_state` cold before / after | warm before / after | labels scan (now after first paint) |
|---|---:|---:|---:|
| 10k | 84 / 66 ms | 87 / 65 ms | 47 ms |
| 25k | 260 / 178 ms | 238 / 173 ms | 108 ms |
| 50k | 449 / 334 ms | 446 / 319 ms | 222 ms |

**Frontend** (`app/smoke/perf-startup.spec.ts`, backend time simulated with the warm medians above; each run is the spec's median of 3; before/after are the medians of 2-3 such runs at load 9-14.4)

| Tracks | Case | First row before / after | Real rows before / after | Interactive before / after |
|---|---|---:|---:|---:|
| 10k | first launch | 750 / 680 ms | 750 / 680 ms | 812 / 740 ms |
| 10k | cold | 495 / 501 ms | 895 / 859 ms | 895 / 873 ms |
| 10k | warm | 238 / 264 ms | 509 / 525 ms | 677 / 525 ms |
| 25k | first launch | 1,010 / 878 ms | 1,011 / 878 ms | 1,041 / 1,074 ms |
| 25k | cold | 481 / 471 ms | 1,073 / 932 ms | 1,234 / 932 ms |
| 25k | warm | 238 / 282 ms | 722 / 681 ms | 860 / 906 ms |
| 50k | first launch | 1,532 / 1,150 ms | 1,532 / 1,150 ms | 1,784 / 1,546 ms |
| 50k | cold | 467 / 457 ms | 1,484 / 1,245 ms | 1,691 / 1,518 ms |
| 50k | warm | 262 / 277 ms | 1,296 / 896 ms | 1,711 / 1,263 ms |

Split at 50k warm, after: `load_state` answered at about 750-960 ms, mapping 40-115 ms, `$state` proxies for 50k rows 100-145 ms, first real row 130-200 ms after the answer.

**The 10k regression is real.** Same build, 10k, with and without the boot snapshot (`FF_PERF_NO_SNAPSHOT`), load about 11: real rows cold 859 / 689 ms, warm 496 / 399 ms. The snapshot paint keeps the main thread busy when the `load_state` answer lands (answered 776 vs 566 ms cold). Boot rows without waveforms cut about half of it (cold 750, warm 442 ms): the row waveform canvases are drawn for the boot rows and again for the real ones. At 25k-50k the answer lands well after the snapshot paint, so the snapshot is a clean win there. Not fixed here; options are in the run report.

**Read with care.** Runs at load 9-16 still swing 50-150 ms between sets (see the 10k and 25k warm rows); differences under about 80 ms are noise.

**Next**

- `$state` proxy creation for every row (100-145 ms at 50k) is now the biggest frontend step; creating them lazily needs care, because a proxy created inside an effect does not register that effect as its reader.
- Idle-time search index warmup still costs about 100 ms of CPU at 50k right after the first rows.
- The list query itself (240 ms at 50k) walks the `analyses` rows with their preview blobs; a covering index would avoid that but is a schema change.

## R4 track load (2026-10-31)

Branch `worktree-agent-acdcf43b4154edf8e` from `ov-1028` (`ea7ad12`). Linux `dev` box, release `bench_startup`, the 12 long fixture files (5-7 min, aiff, flac, m4a, mp3). Box load was 16-65 the whole time (other workers), so every number below carries its load; read the ratios, not the absolute times.

**Where the second went.** Click to first audio was already 2-8 ms; the time was the first open of the zoomed waveform (`get_waveform_detail`, a cache miss). Per file at load 16-33 (`bench_startup detail`): one-thread decode 410-1,390 ms, the 2400 columns/s meter 760-1,140 ms, and `measure_detail` 830-1,210 ms, i.e. the meter itself. The decode was already spread over 6 threads, but the meter ran on the one thread that received the segments in order, so the whole measurement took as long as the meter. IPC was not a factor: the series goes out as one binary response (4-5 MB, read zero-copy by `bandsFromDetailBytes`), cached reads take 2.5-3 ms, and the frontend draws it without copying.

**Changed**

- Parallel meter: each decode segment now measures its own stretch with a `DetailMeter::starting_at` its track frame, after warming the band filters on the preroll audio its decoder already makes; the frame a segment starts at is chained from the segment before (known once that one is decoded, not metered), and the caller `join`s the stretches. Bytes are identical to the one-thread meter on all aiff, flac and mp3 files (m4a's parallel decode already differed slightly before, unchanged). `measure_waveforms_and_detail` (analysis) is unchanged.
- A pressed row warms its detail at UI priority, beside the background queue (`prefetch_waveform_detail { urgent: true }`); before, the press queued a warm at nice 10 / utility QoS and the click's `get_waveform_detail` then waited on its lock. A background warm no longer blocks `get_waveform_detail` for the same file: they measure side by side, both write the same bytes (partial files now have unique names; the cache trim skips them).
- Detail cache cap 512 MiB to 2 GiB (about 450 tracks of 6 minutes).
- Boot snapshot only from 20,000 tracks on (`BOOT_SNAPSHOT_MIN_TRACKS`); smaller libraries remove and ignore a stored one.

**Waveform detail, first open** (`bench_startup audio`, 12 files, median / p95; before = `ov-1028` binary, interleaved with after)

| Fixture | Load | Before | After |
|---|---:|---:|---:|
| 10k (per file, `bench_startup detail`) | 16-33 | 830-1,210 ms | 480-690 ms |
| 10k | 44-51 | 1,309-1,326 / 1,967-2,499 ms | 773-830 / 1,066-1,184 ms |
| 25k | 41-46 | 1,265 / 2,951 ms | 1,006 / 1,845 ms |
| 25k | 49-52 | 2,478 / 3,456 ms | 1,662 / 2,437 ms |
| cached read, all | 41-52 | 2.9-6.9 ms | 2.7-6.8 ms |
| first audio packet after open | 41-52 | 0.5 ms | 0.5-0.8 ms |

At load 16 the pipeline is limited by the six decode threads doing decode plus filters (about 40 ms of work per 256-packet segment); twelve threads gave nothing more on this loaded box. On a quiet 8-core machine the same split should land around 250-350 ms. The 50k fixture was not generated (35 GB more on a 76%-full disk); the detail numbers do not depend on library size, since they use the same 24 long files.

**Boot snapshot threshold** (`app/smoke/perf-startup.spec.ts`, payloads from the 10k and 25k fixtures, 15k and 20k cut from the 25k one; backend time simulated at 65/100/135/173 ms; median of 3; load 19-27, a second pair at load 54-65 was too noisy to use)

| Tracks | Case | First row with / without snapshot | Real rows with / without |
|---|---|---:|---:|
| 10k | cold | 894 / 1,060 ms | 1,579 / 1,060 ms |
| 10k | warm | 463 / 772 ms | 1,100 / 772 ms |
| 15k | cold | 898 / 818 ms | 1,742 / 818 ms |
| 15k | warm | 487 / 650 ms | 1,095 / 650 ms |
| 20k | cold | 597 / 1,053 ms | 1,244 / 1,053 ms |
| 20k | warm | 272 / 716 ms | 860 / 716 ms |
| 25k | cold | 563 / 1,145 ms | 1,351 / 1,145 ms |
| 25k | warm | 363 / 852 ms | 990 / 852 ms |

Up to 15k the snapshot rows come at most 300 ms earlier and push the real rows back 450-920 ms; from 20k on they come 450-580 ms earlier for 140-190 ms on the real rows. So the threshold is 20k, not 15k.

**Next**

- The biggest remaining lever is outside this item's lane: the Rust analysis path (`analysis_run.rs`, `Runner::Rust`) does not store the detail series; only the Python path calls `measure_waveforms_and_detail` + `store_detail`. With it, every analysed track opens from the cache (3 ms) and the 120 ms target is met for everything but unanalysed tracks.
- Cache files are 12 KB per second of audio uncompressed; a compact encoding would let 2 GiB hold several thousand tracks.
- Below the decode threads: the meter runs f64 biquads per frame; f32 would roughly halve it but change bytes slightly.

## R3 analysis throughput (2026-10-31, `r3-analysis`)

Code: `ov-1028` plus the `analysis_run` test-harness fix (`8131af8`). Box: `dev`, 16 cores (Xeon Gold 6130, AVX-512), 64 GB, analysis at 12 threads, `nice -n 10`. Load average 14-50 from other workers the whole time, so wall-clock numbers swing by up to 2x between runs; CPU seconds and output identity are the reliable signals. Raw files: `~/.cache/r3-bench/` (`out-*`, `time-*`, `rss-*`, `dump-*`).

### Where the time goes

Per-thread CPU of a 12-track run (12 full-length fixture files, 5-7 min; `threads.sh`, final code): beat-model chunk threads (`rten-0`, one single-thread pool per chunk worker) **762 of 838 CPU-s = 91%**; the job threads (decode, 22.05k resample, mel driver, fit, DB/ANLZ write) 49 s = 6%; key and waveform threads 27 s = 3%. About 64 CPU-s per 6-min track, 13 chunks of 30 s at about 5 s each single-threaded on an idle core.

Inside one chunk (`RunOptions::with_timing_by_shape`, 4.8 s): MatMul 43%, FusedMatMul (linear layers, GELU MLP) 21%, Softmax 12%, Concat/Split (rotary embeddings) 7%. The attention steps alone are about 2.4 s: the time attention of every block builds full 1500 x 1500 score matrices (`[32,1,1500,1500]` in the frontend, `[1,16,1500,1500]` in the 6 transformer layers), writes them, softmaxes them and reads them again. Those steps run at 1.3 ns per element, i.e. memory-bound, and they are also why one chunk holds about 300 MB of activations (rten reports `Max activation bytes 299 MB`), which is what limits the chunk slots on an 8 GB Mac to 3.

Thread scaling (same binary, 12 tracks): 6 threads 3.1 trk/min, 12 threads 6.7-8.5 trk/min at the same ~800 CPU-s. The work scales linearly with cores and the CPU-s per track do not drop with fewer threads, so there is no headroom to win by running fewer model threads; it is plain CPU work.

### What changed

- **The model no longer waits for the fast passes** (`analysis_run.rs`, `take_next`). Before, every queued track got its fast pass (waveform, key) before any grid pass could start, so in a 10k queue the beat model sat idle until all 10k fast passes had been taken: about 10k x 2.5 CPU-s / 11 workers, roughly 40 min on this box and hours on a 2-worker Mac, before the first BPM. Now, while new tracks still wait for their fast pass, up to `min(2, workers - 1)` model jobs take the next new track **whole** (`Pass::All`: one decode, waveform and key stored first, then the grid); the other workers keep taking fast passes, so the waveforms-then-keys-then-grids order still holds for the rest of the queue. Once every track had its fast or whole pass, grid passes run with the full model-job cap as before. A lone track (a run of one) now runs whole with every thread instead of fast then grid (one decode instead of two).
- **A resumed track that only needs its grid no longer blocks the queue**: when the model is busy it moves to the grid queue instead of sitting at the queue front and stopping the fast passes behind it.
- **Key lands with the waveform** (`analyze.rs`, `service.rs`): the early partial result carries the key next to the waveform, so a whole job (now lane or the new model jobs) shows its key at once instead of with the grid; `partial_store` stores waveform and key stages and the track's key.
- **The 44.1k resample runs beside the model** (`analyze.rs`): it moved into the key thread, so the model starts that much earlier, and the grid pass (no key) skips it entirely.
- `bench_analysis --fast-first` keeps the old order for A/B in the same binary.

### Before / after

12 full-length tracks (`perf.sh` analysis sample), app path (`run_track_analysis`, staged), back to back:

| Run | Load at start | Tracks/min | First result | All waveforms | All keys | CPU user + sys | Peak RSS |
|---|---|---:|---:|---:|---:|---:|---:|
| before 1 | 17 | 6.74 | 18.3 s | 6.6 s | 6.8 s | 785 + 74 s | 5.24 GB |
| before 2 | 21 | 8.20 | 16.9 s | 6.3 s | 6.4 s | 778 + 68 s | 5.20 GB |
| after 1 | 14 | 8.33 | 18.7 s | 9.7 s | 24.6 s * | 762 + 69 s | 5.43 GB |
| after 2 | 21 | 8.52 | 18.3 s | 6.8 s | 23.5 s * | 759 + 67 s | 5.41 GB |
| after, final (key with waveform) | 19 | 7.12 | 18.7 s | 8.6 s | 25.5 s * | 781 + 80 s | 5.29 GB |

\* "All keys" is when the bench saw the last key. It counts a track's second partial event as its key; a whole job sends only one partial (waveform and, in the final code, the key together), so for the 1-2 whole jobs the bench falls back to their grid time. In runs after 1-2 those keys really did land only with the grid; in the final code they land with the waveform, and the bench column overstates it. Tracks that took the fast pass have their key at the same time as before.

At 12 tracks the change is within noise (the fast phase is only 6 s of a 90-110 s run; CPU -2%). It matters with a long queue:

| 500 tracks (476 short fixture clips + 24 full-length, interleaved), same binary | Load | First result | All waveforms | CPU-s / track | Peak RSS |
|---|---|---:|---:|---:|---:|
| `--fast-first` (old order) | 19 → 53 | 15.2 s | 15.0 s | ~5.4 | 5.87 GB |
| new order | 16 → 24 | **3.0 s** | 20.8 s | ~4.7 | 5.99 GB |

With the old order the first result is always "all fast passes + one grid", so it grows with the queue: 15 s at 500 short clips, about 40 min at 10k full-length tracks here, hours on the Mac. With the new order it is one whole analysis regardless of queue length: 3 s for a 10 s clip, 17-19 s for a 6-min track at 12 threads on this loaded box (two rounds of chunks: 13 chunks on 12 threads). Waveform coverage of the whole queue takes a little longer (20.8 vs 15.0 s here) because two workers now run the model. Throughput of the two 500-track runs is not comparable (load went to 53 during the fast-first run).

**Output identity.** Stored results (BPM, key, beat grid with bar positions, waveform preview, colour waveform) compared with `cmp.py` on `bench_analysis --dump`: 12/12 identical on the fixture set in every before/after pair (tracks that ran whole and tracks that ran fast + grid alike), and **49/49 identical on the 50-track o3 dev split** (real music; one file does not decode, as before). The 44.1k resample, the key and the model see the same samples as before, so this is expected. `tracks_sharing_the_chunk_threads_get_the_logits_they_get_alone_and_a_paused_one_waits` (`--ignored`, real weights) passes.

**Memory.** RSS over the 500-track runs (`rss-mix500-*.txt`, sampled every 5 s) swings between 1.3-1.9 GB (between model chunks) and 5-6 GB (12 chunks in flight, ~300-400 MB of activations each); the troughs rose by about 0.3 GB over 7-19 minutes and the peaks did not grow. A full 10k run was not done, so "flat over 10k" is not proven; the per-track state (decoded audio, chunk results) is freed when a track ends and the queues hold only ids and paths.

**Resume after restart.** Already works: at launch the app calls `resume_analysis`, which queues every track with missing stages first (`get_incomplete_analysis_ids`, they resume at the missing stage), then never-analysed tracks when "analyse new tracks" is on (default on). Not changed. Gap: tracks the user queued by hand while that setting is off are not resumed (the queue itself is not persisted).

**Accuracy eval.** `ffa` built from this branch (`prototyping/analysis-packaging`, `--workers 4 --threads 1 --reference ref.json --list dev.txt`) scored with `benchmark/scripts/o3-score.py`: 50 scored, BPM within 0.1 47 (94.0%), within 1.0 50 (100%), **grid within 20 ms 44 (88.0%)**, bar right 27 of 46 aligned. The 2026-10-28 record on the same split was 44 of 49 scored (one file did not decode then), BPM within 0.1 46: not lower. Key: `cyankey::detect` is unchanged and gets the same samples; the app-path identity above covers it.

### CPU share (for run item 5)

Thread count per job is unchanged. On an 8 GB Mac (7 analysis threads, 2 workers by memory, 3 chunk slots) the fast phase now runs one whole job (3 chunk threads + its decode/mel and key/waveform threads) beside one fast worker, where it ran two fast workers before, so the peak is about the same 6-7 busy threads; the analysis just starts the model sooner and finishes the same work sooner. It did not get worse; it did not get better either. Lowering the share is a choice between fewer chunk slots (slower, linear) and leaving cores free while the user interacts.

### Not done, and why

- **Throughput target (30 trk/min) is out of reach on CPU with this model and runtime.** 91% of the CPU is the beat model and it scales linearly with cores; 30 trk/min of 6-min tracks needs about 32 cores of this box. Every in-scope trick moves a few percent at most: decode once for every track (fast + grid) ~1-2%; skipping the waveform recompute in the grid pass ~1% but would re-write ANLZ from a read-back (`get_analysis` → `set_analysis`), not proven lossless, left alone; prepacked weights no change (q50); jemalloc via `LD_PRELOAD` within noise (8.60 vs 8.52 trk/min, 723 vs 759 user-s, page faults up); fewer threads no gain (linear).
- **First result 5 s** is reachable only for short tracks: one chunk is about 5 s on an idle core, so a 6-min track is at least one round of 13 parallel chunks plus decode and mel. The new order makes it independent of queue length, which was the 10k problem.
- **The lever that would move both numbers** is the attention: a fused (flash-style) attention would not materialise the 1500 x 1500 matrices (about half the chunk time, and most of the 300 MB per chunk, so more chunk slots on the Mac). That needs a runtime with fused attention or a custom op, and is not bit-identical (softmax order), so it needs the grid/key eval. On the Mac, running the model on the GPU/ANE (onnxruntime with CoreML) is the other big lever. Both are new runtimes; decision for Merlin.
- **Next: waveform detail from the Rust analysis** (asked during this run, did not fit the timebox). The Rust path should store the detail series like the Python now lane (`store_detail`), so a zoom view opens from cache. Trap: the analysis decodes with symphonia 0.6, the app's detail path with 0.5, and MP3 bytes differ (q50-analysis2 `fe78598`). Plan: feed the detail meter from the same decoder the app path uses (or move both to one version), store only when a check on a few tracks shows identical bytes, and measure the extra cost per track against the 64 CPU-s of the model (expected around 1%).

## R7 stored detail (2026-10-31)

Branch `worktree-agent-acffa3352ed21e257` from `ov-1028` plus `r3-analysis` (`cd1a905`). Linux `dev` box, release `bench_analysis`, the 22 long fixture files of `~/.cache/ff-fixture/pool-v1/long` (2 aiff, 4 flac, 4 m4a, 12 mp3, 5-7 min). Load 15-36 during every run; the store runs always went second and each found a higher load than its plain run.

**Changed.** The Rust analysis now puts the zoomed waveform's detail series of every track it analyses into the app's detail cache (`waveform-detail` under the app data dir, the cache `get_waveform_detail` reads, 2 GiB cap with trim, unique partial names). It calls the app's own `warm_detail` → `measure_detail` (symphonia 0.5, the parallel `DetailMeter` of R4), so the bytes are the ones a cache miss writes, by construction, for every codec. It runs on a thread of its own beside the job's analysis: a grid pass starts it at once (the model runs for 10+ s, the detail takes about 0.6 s), a whole pass (now lane, whole jobs) starts it once the waveform partial is stored, so the first waveform is not held up. Fast passes skip it, so the waveform phase of a long queue keeps its speed; every track still gets it once, in its grid or whole pass. A track already in the cache (opened by the user) is skipped; a concurrent `get_waveform_detail` does not wait for it (`warm_detail` semantics). The Python now lane is unchanged.

**Why not share the analysis decode.** The analysis decodes with symphonia 0.6 (`fourfour-analysis-rs/decode.rs`), the app (playback and detail) with 0.5, and the two MP3 decoders give different samples (q50-analysis2 `fe78598`: 6 of 8 MP3s differ in 55-77k samples). One version for both would mean moving the analysis to 0.5 (changes analysis results on MP3) or the app's playback to 0.6 (outside this item's lane). m4a would not match either way: the app's parallel decode differs from a serial one there. Sharing the decode would save only the decode part of the cost; the meter is most of it (R4). So the detail meter gets its own 0.5 decode. When the app moves to symphonia 0.6, feeding the meter from the analysis decode for lossless and MP3 files saves roughly the decode share.

**Byte identity** (`bench_analysis --store-detail`: after the run, the stored file read through the cache vs a fresh `measure_detail`): 22/22 stored and identical in each of three runs: aiff 2/2, flac 4/4, mp3 12/12, m4a 4/4. Unit test `every_analysed_track_leaves_the_detail_series_the_app_would_measure_in_the_cache` (mixed fast + whole passes, wav).

**Analysis results unchanged.** The analysis crate is not touched. `--dump` of the stored results, plain vs store run: 22/22 identical in both pairs (and plain vs plain 22/22).

**First open of an analysed track's zoomed waveform**

| | Load | Median | Range |
|---|---:|---:|---:|
| before (cache miss, `measure_detail`) | 30-36 | 600 ms | 465-774 ms |
| after (stored by the analysis, cache read) | 30-36 | 2.9 ms | 2.2-10 ms |

**Cost.** `measure_detail` per track (process CPU around the call, 44 measurements): median 2.2 CPU-s (1.4-2.8; aiff/flac 1.4-2.3, mp3 1.7-2.8, m4a 1.9-2.8). The analysis is about 88 CPU-s per track here, so 2.5%, more than the 1% expected: the meter (f64 biquads at 2400 columns/s) is most of it and is the same work a cache miss does. Whole runs (22 tracks, median of 3): plain 1,922 CPU-s, store 1,996 CPU-s (+3.4 CPU-s per track, +3.9%, within the run-to-run noise of about 2% plus the rising load). Wall time and first result followed the load (plain 191-236 s / 20-25 s at load 15-25, store 205-304 s / 21-36 s at load 27-36), not separable from this box's noise; the detail work sits beside the model and starts after the first waveform. Peak RSS 6.7-7.0 GB plain vs 7.0-7.2 GB store (the model chunk peaks dominate; the detail holds at most 14 decoded segments and one series per job).

**Next**

- The meter in f32 (R4 "Next") would roughly halve the 2.5%; it changes bytes slightly, which is fine as long as both paths use it.
- Moving playback to symphonia 0.6 would allow feeding the meter from the analysis decode and drop the extra decode.
- Tracks analysed before this change get their detail on their first open, as before; a re-analysis or a background warm fills them.

## R6 import (2026-10-31)

Code: `ov-1028` (`67f6607`) plus `r6-import`. Box: `dev`, 16 cores, load 12-17 (calm runs) to 20-37 (busy). Base and new are the same bench (`bench_startup import`) built from both trees, run back to back on the fixture (read only): cold = page cache of the music evicted with `posix_fadvise(DONTNEED)` first (directory entries stay cached), warm = right after. Raw: `~/.cache/r6-import-work/cmp{10k,25k}.txt`.

### Before and after (backend, median)

"Rows" = scan + plan + commit: when `commit_import` returns and the frontend can append the rows (its own landing, `import ui click to rows`, is unchanged: 119 ms at 10k). "Covers" = the background fill after that.

| | 10k before | 10k after | 25k before | 25k after |
|---|---:|---:|---:|---:|
| import, cold, to rows | 14.0 s | 2.1 s (load 13); 3.7-4.8 s (load 21-31) | 53.9 s | 7.8 s (load 15); 16.3 s (load 20, disk busy) |
| import, warm, to rows | 12.4 s | 0.8 s (load 14); 1.1-1.6 s (load 21) | 36-38 s | 2.3-3.2 s |
| covers stored, in the background | in the commit | 8-10 s more | in the commit | 21-31 s more |
| re-import of the same folder | 1.97 s | 0.14 s | 9-10.6 s (r1: 4.9) | 0.26-0.58 s |
| commit (library locked) | 10.1 s | 0.3 s | 30-33 s | 0.6-1.0 s |

### What changed

- **Covers after the rows.** The scan drops each cover as soon as the tags are read; `add_tracks_covers_later` inserts the rows only and queues their ids in `pending_artwork` (new table, additive, no version bump: it is what makes the deferral survive a quit). A background fill (`store_pending_covers`, started after `commit_import`, at launch and on a library switch) reads the covers again on 8 threads while it stores the previous batch of 32 (about 6 MB, library lock 25 ms p50, 60 ms max), and emits `import-covers` at most every 500 ms; the app sets `has_artwork` and calls `bumpLibraryTracks(ids)`. A cover is only stored if the queued track still points at the file it was read from; one set by hand meanwhile wins. Rows come back with `has_artwork: false`, so nothing asks for a cover that is not there yet. Side effect: the plan no longer holds every cover in memory (1.5 GB at 10k).
- **Known paths are not read.** Paths already in the library skip the tag read (they were dropped as "already in library" after reading before); re-import is now one folder walk.
- **One folder walk, in parallel, without stats.** Survey and scan walked the tree twice, each `stat`ing every entry and `canonicalize`-ing every folder (the fixture has 9,439 folders); now one walk uses the listing's file type, canonicalises only at symlinks (same loop rule as before) and walks the subfolders of a dropped folder on 8 threads: 440 ms to 70-150 ms at 10k. The walk now also skips hidden files for reading, not only for counting, and counts files in symlinked folders, so progress `total` matches the files read.
- **Tag reads on 32 threads.** Cold 10k: 5.8 s on 8 threads, 3.0 s on 16, 2.2-2.6 s on 32, 1.7 s on 64 (same load).
- **Duplicate checks in parallel**, skipped when nothing new was read: plan 170-220 ms to 60-110 ms.
- **Commit.** One path query instead of one per track; bulk writes run with a 64 MB page cache (25k commit 1.2 s to 0.6 s); covers no longer in the transaction.

### What is left

- **The cold scan reads every embedded cover.** lofty reads an ID3v2 picture frame through even with `read_cover_art(false)` (it skips by reading), so a cold 10k import still reads 2.7 GB (1.5 GB of covers plus readahead) and is bound by the disk: 1.6 s on a calm box, 4-6 s when other workers use the disk. An ID3 frame walker that seeks past `APIC` (or a lofty change) is the next step; it sits in `pioneer-usb-writer/src/scanner.rs`, not touched here.
- **Covers take 8-10 s (10k) to 21-31 s (25k) in the background** (1.5 GB / 3.7 GB written to the log and checkpointed). A USB export started in that window writes the tracks still queued without artwork.
- Commit at 25k is 0.6-1.0 s, mostly the three indexes on `tracks` (`source_path` has both the UNIQUE index and `idx_tracks_source_path`; dropping the duplicate is a schema change).

## R5 UI headroom (2026-10-31)

Branch from `ov-1028` (`6fe845f`). Linux `dev` box, 16 cores, release `bench_analysis` at `nice -n 10`, headless Chromium with the smoke mock at 10,000 tracks. Load average 14-50 from other workers throughout; every number below names its load, and the box can only show direction, not the Mac's size of effect.

**Changed**

- **Two cores stay free.** The default analysis thread count is `cores - 2` (was `cores - 1`); `FOURFOUR_ANALYSIS_THREADS` still overrides it. The analysis already runs at utility QoS on macOS, which the scheduler puts behind the web view's user-interactive threads, so two free cores are two free performance cores there.
- **Analysis holds while the user scrolls, wheels or drags.** One capture listener on the window (`app/src/services/ui-busy.ts`) sends `analysis_ui_busy` at most every 200 ms while `library.analyzing`; the coordinator then holds background work until 500 ms pass without a signal: no new model chunk, no new job. Chunks already on a thread finish. The now lane (the track being played) is not held. Playback alone does not hold the analysis: a DJ previews tracks for most of a prep session, and holding then would stop the analysis most of the time.
- **Measuring knobs, not settings:** `FF_ANALYSIS_UI_PAUSE=off` (the command ignores the signal), `FF_ANALYSIS_QOS=background` (macOS: analysis on the efficiency cores only). `bench_analysis --ui-busy <busy ms>/<period ms>` (synthetic user), `--ui-busy-file <path>` (one signal per byte appended, used to forward the headless page's real signals), `--probe-reads <ms>` (a reader doing `build_analysis_json` under the library lock every few ms). The run log ends with "held for the UI Nx, S s".

**How fast the hold frees cores.** Cores the analysis process used, inside vs outside the busy phases (8 long files, 12 threads):

| synthetic user | load | analysis cores while busy | otherwise | held |
|---|---:|---:|---:|---:|
| 1.5 s busy every 5 s | 25 | 7.7 | 8.3 | 39% of the run |
| 3 s busy every 6 s | 22 | 7.0 | 8.3 | 56% |
| 8 s busy every 16 s | 23 | 2.5 | 8.5 | 50% |

A beat-model chunk cannot be stopped once it runs (one rten call, about 5 s on an idle core here and longer under load), so the hold frees cores only as chunks end: on this box it takes about 4-5 s to drain. A short flick of the list hardly changes the CPU; continued browsing (scroll, read, scroll within 500 ms) does. On the Mac chunks are shorter (faster cores, 3 slots on 8 GB), so the drain is shorter, but still around a chunk. Decode, mel and key of jobs already running are not held either.

**Throughput cost.** Wall time on this box swings 2x between identical runs (8 long files, 12 threads: 2.9-6.0 trk/min with no UI at all), so the cost is not measurable in wall time here; it follows from the held time instead. A held stretch costs at most the analysis work that would have run in it, and less, because running chunks finish: with the user scrolling 10% of the time (1 s of every 10 s) the run was held 14% of the time; 33% scrolling, 40% held. Process CPU for the 8 tracks was 615-685 s with or without the hold. The core cap costs nothing where memory already limits the model (8 GB Macs: 3 chunk slots and 3 threads per job either way) and up to 1/7 of model throughput where cores limit it (16 GB 8-core Mac: 7 -> 6 chunk slots; this box 15 -> 14, -7%).

**Scroll frames during a real analysis** (`perf-bulk-analysis.spec.ts` list scenario, real `bench_analysis` of the 24 long files as load; before = 15 threads, no hold; after = 14 threads, the page's own signals forwarded to the bench). Median of 4 rounds; the rounds ran at load 15-49:

| window | idle box p95 / lost | before p95 / lost | after p95 / lost |
|---|---:|---:|---:|
| scroll during fast pass | 33 ms / 75 | 51 ms / 178 | 53 ms / 173 |
| scroll during grid pass | 30 ms / 35 | 52 ms / 192 | 40 ms / 121 |
| scroll, BPM sort, fast pass | 37 ms / 80 | 58 ms / 193 | 62 ms / 201 |

Rounds 1 and 4 (load 15-33) were clearly better after (grid pass p95 43 -> 27 and 50 -> 30 ms, lost frames 166 -> 32 and 189 -> 59); rounds 2-3 were swamped by load 35-49 from other workers. On Linux the analysis at nice 10 takes little CPU time from Chromium in the first place, so most of the before/after gap here is cache and memory bandwidth plus the other workers. Raw: `~/.cache/r5-bench/frames-*.json`, `results.txt`, `cores.txt`.

**Library lock.** A reader taking the library lock every 16 ms during analysis (8 tracks): p50/p95/p99 wait 0 ms, but about 2.5 waits per track of 100-660 ms (max per run 205-662 ms). They are the analysis' own writes holding the lock: `set_analysis` (DB plus ANLZ files) takes 0.1-0.2 s per track in `record_outcome`, and the waveform partial stores hold it too. Any command that needs the lock in that moment waits: `get_analysis_data` for a finished row, loading a track into the player, the waveform detail's track lookup. Scrolling itself needs no lock (rows are in memory). Not changed here: the fix is to build the ANLZ bytes and the row outside the lock and hold it only for the SQLite write, which is in `pioneer-library` (outside this item). `build_analysis_json` itself takes 8.6-12 ms p50 per call.

**Main thread.** Nothing left worth changing found: progress and partial events already land in batches of at most one library update per 250 ms (fb-analysisfps), and the per-track `debug-log` events have no listener in `app/`, so Tauri does not send them to the web view.

**What only the Mac can confirm** (`scripts/mac/perf-night.sh` now runs three analysis app runs: `analysis-before` with `FF_ANALYSIS_UI_PAUSE=off` and `cores - 1` threads, `analysis` with the defaults, `analysis-bgqos` with background QoS; each scrolls 10 s, then 5 bursts of 1.5 s scroll + 2.5 s rest; plus four `bench_analysis` runs for the throughput cost: `cores - 1` threads, default, `--ui-busy 2000/6000`, background QoS. The summary has a "UI headroom" table.):

- whether WKWebView frames improve, and by how much in the bursts (the realistic case),
- the drain time with Apple Silicon chunk lengths,
- what background QoS (efficiency cores only) costs in throughput and gains in frames; it is the only lever that frees performance cores at once, mid-chunk.

**Next**

- ~~Library writes outside the lock~~ done in R5b below.
- If the Mac shows the drain is too slow for bursts: background QoS for the model threads by default (cost measured by the night), or a GPU/ANE runtime for the model (R3), which takes the model off the CPU entirely.

## R5b library lock (2026-10-31)

The analysis stores a track in three steps (`LocalLibrary::begin_analysis`, `AnalysisWrite::stage`, `commit_analysis`): read the track and what the stored analysis depends on (lock), build the ANLZ files into a staging folder in the track's folder (no lock), then rename them into place and write the track row and the index row in one transaction (lock). If the track, `edits.json`, `imported.json`, the index row or the stored DAT changed in between, the commit refuses and the analysis is stored the old way under the lock. Files are now replaced by rename instead of rewritten in place. `get_analysis_data` also scans the MP3 lead-in and builds the JSON after releasing the lock.

Same box, `bench_analysis --list list8 --threads 12 --probe-reads 16`, nice 10, before/after alternated, 3 rounds, load 15-48:

| | lock held per track store | probe wait max | probe waits > 16 ms |
|---|---:|---:|---:|
| before | 0.1-1.1 s (`DB+ANLZ write`) | 236-986 ms | 20-22 per run |
| after | 1.3-7.3 ms (1.4 typical) | 0.1-7 ms | 0 |

Byte identity: the ignored test `fixture_library_files_are_byte_identical_both_ways` (`FF_FIXTURE_LIBRARY=~/.cache/ff-fixture/10k/library FF_FIXTURE_TRACKS=1000`) stores 1000 fixture analyses (251 with `edits.json`) both ways and compares the three ANLZ files and the index row.

## R9 onnxruntime (2026-10-31, `r9-ort`)

Branch `r9-ort` from `ov-1028` (`b24b09b`), not landed. Box: `dev`, 16 cores (Xeon Gold 6130), load 10-34 from other workers during every run; read CPU seconds and the isolated per-chunk numbers, wall clock only as direction.

**Recommendation: Mac-only.** On this CPU onnxruntime saves about 30% of the CPU per chunk in isolation but gives no throughput in the app path (2x memory, later first result), so rten stays the default everywhere and `ort` stays only as the route to CoreML (GPU/ANE) on the Mac; drop it if the Mac run below shows no gain.

### What changed

- `fourfour-analysis-rs` feature `ort` (off by default; the default build is unchanged and runs rten): the beat model runs on onnxruntime through the `ort` crate (`=2.0.0-rc.14`, MIT/Apache, `src/ort_beat.rs`). The mel model stays on rten (a few percent of the time). Same input/output contract (`spectrogram` in, `beat`/`downbeat` logits out), same chunking and border handling, same shared chunk threads (`share_chunk_threads`, `analysis_ui_busy`, cores-2 untouched): each chunk thread opens its own single-threaded session (`Session::run` takes `&mut self`); all sessions share one prepacked copy of the weights and one process-wide ORT environment (ORT allows only one per process).
- `FOURFOUR_BEAT_RUNTIME=rten` switches an `ort` build back to rten at load time, so one binary does the A/B. `bt::beat_runtime()` says which one is active.
- Feature `coreml` (implies `ort`): on macOS the session gets the CoreML execution provider (MLProgram, all compute units) with CPU fallback; elsewhere it is the same as `ort`. On macOS the `ort` dependency always pulls the CoreML build of ONNX Runtime (pyke ships only that one for `aarch64-apple-darwin`); the cargo feature only decides whether the EP is registered. The CoreML branch was type-checked on Linux against the ort API; it has not been built or run on a Mac.
- `pioneer-test-ui` and `prototyping/analysis-packaging` forward both features (`--features ort` / `--features coreml`).
- CPU allocator: arena on, memory pattern off. Arena off costs every chunk 300 MB of fresh pages (sys 239 s vs 52-95 s per 12 tracks, per-chunk CPU back to rten's); memory pattern on adds about 7 GB of RSS for nothing.
- Ignored test `bt::ort_tests::ort_gives_the_rten_logits_for_a_chunk` (`--features ort`, real weights); with `FF_CHUNK_ROUNDS=7` and `--release --nocapture` it also prints the per-chunk CPU of both runtimes. The ignored `tests/generated_tracks.rs` (chunk sharing, pause, cancel) pass on `ort`.

**Build impact (affects the .dmg).** With `ort`, the `ort-sys` build script **downloads a prebuilt ONNX Runtime 1.31.0 static library** from `cdn.pyke.io` at build time (here `x86_64-unknown-linux-gnu.tar.lzma2`, a 112 MB `libonnxruntime.a`, sha256-checked, cached in `~/.cache/ort.pyke.io/`) and links it statically, so nothing extra ships next to the app. The first build needs network (or `ORT_LIB_LOCATION` pointing at a local ONNX Runtime build). There is no prebuilt for Intel Macs (`x86_64-apple-darwin`). `ffa` with `ort` (release, LTO, stripped) is 31.5 MB. The default build downloads nothing.

### Numbers (CPU execution provider vs rten, same tracks, same threads)

Per chunk (one 1500-frame chunk, one thread, release, process CPU, median of 7, interleaved):

| Config | rten | ort |
|---|---:|---:|
| arena on, memory pattern off (kept) | 4,900 ms | **3,360 ms** (-31%) |
| arena on, memory pattern on | 4,880 ms | 3,320 ms |
| arena off | 4,690 ms | 4,890 ms |

Wall per chunk in `ffa --workers 1 --threads 1` (3 long fixture tracks, load about 30): rten 5.1-8.5 s, ort 5.6-5.8 s.

App path (`bench_analysis --backend rust --threads 12`, the first 12 long files of `~/.cache/ff-fixture/pool-v1/long`, `nice -n 10`, interleaved, `/usr/bin/time`):

| Run | Load at start | Tracks/min | First result | user + sys CPU | Peak RSS |
|---|---|---:|---:|---:|---:|
| rten | 20 | 9.64 | 14.7 s | 756 + 63 s | 5.5 GB |
| rten | 20 | 10.01 | 14.5 s | 746 + 60 s | 5.4 GB |
| rten (earlier pair) | 11-24 | 6.6-8.6 | 17-31 s | 762 + 72 s | 5.2-5.4 GB |
| ort | 10 | 8.14 | 35.2 s | 705 + 67 s | 10.7 GB |
| ort | 21 | 9.13 | 31.8 s | 710 + 95 s | 10.7 GB |
| ort, memory pattern on | 27 | 6.50 | 37.6 s | 720 + 105 s | 18.2 GB |
| ort, arena off | 30 | 5.63 | 41.1 s | 702 + 239 s | 7.4 GB |

- Throughput: no gain. User CPU drops 6-8%: the 31% per chunk mostly disappears when 12 chunks run side by side (the attention steps are memory-bound, see R3, and onnxruntime does not fuse them on this graph either). Wall is the same or worse.
- First result about 2x later: every chunk thread builds its session (graph optimisation and weight prepack, about 2.5 s) on its first chunk, 12 at once. Fixable (build sessions at engine load, or cache the optimised model), not worth it while throughput is flat.
- Memory about 2x (each session keeps its own arena at peak activations).
- `ffa` on the 50-track dev split (`--workers 4 --threads 12`, memory pattern on) showed rten 4.48 vs ort 9.81 trk/min (peak RSS 6.2 vs 19.5 GB). The two ran back to back at load 25-35 and the interleaved app-path pairs did not reproduce it: load noise, not a result.

Logits vs rten (dev split, 50 real tracks, whole-track logits from `ffa --dump`): **max abs logit diff 2.0e-4** (median per track 7.9e-5). Beat peak frames identical on 50/50; final grid within 0.21 us of rten, BPM within 6e-8; the `ffa` output (BPM, beats, downbeats, key, waveform checksum) identical on 50/50. Synthetic chunk test: 5.0e-6.

Grid eval (`ffa ... --reference ref.json --list dev.txt` scored with `benchmark/scripts/o3-score.py`, dev split as in R3):

| | rten | ort |
|---|---:|---:|
| Grid within 20 ms | 44 / 50 (88.0%) | **44 / 50 (88.0%)** |
| BPM within 0.1 / 1.0 | 47 / 50, 50 / 50 | 47 / 50, 50 / 50 |
| Bar right (of 46 aligned) | 27 | 27 |

The grid bar does not drop. Key: `cyankey::detect` never sees the beat model, so the Beatport key eval (58.3% test) is unchanged by construction; the dev-split keys of both runtimes are identical on 50/50.

Raw: `~/.cache/r9-ort-work/` (`dev-{rten,ort}.jsonl`, `dump-dev-*`, `m-*.err`, `tput-*.err`, `chunk-*.jsonl`).

### Running it on the Mac (untested here)

```bash
# first build needs network: ort-sys downloads the aarch64-apple-darwin+coreml ONNX Runtime 1.31.0
cargo build --release -p pioneer-test-ui --features coreml --bin bench_analysis
export FOURFOUR_MODELS_DIR=<dir with beat_this.onnx and mel_spectrogram.onnx>
target/release/bench_analysis --backend rust --list list12.txt --threads 3                              # CoreML
FOURFOUR_BEAT_RUNTIME=rten target/release/bench_analysis --backend rust --list list12.txt --threads 3   # rten, same binary
# accuracy with the CoreML build
(cd prototyping/analysis-packaging && cargo build --release --features coreml --bin ffa)
prototyping/analysis-packaging/target/release/ffa --models "$FOURFOUR_MODELS_DIR" --workers 2 --threads 3 \
  --reference benchmark/results/o3/ref.json --list benchmark/results/o3/dev.txt --dump /tmp/dump-coreml --out /tmp/dev-coreml.jsonl
python3 -I benchmark/scripts/o3-score.py benchmark/results/o3/ref.json /tmp/dev-coreml.jsonl
# app bundle with CoreML
fourfour-analysis-rs/scripts/bundle-app.sh --bundles dmg --features coreml
```

What to watch: every chunk thread opens its own session, so CoreML compiles the model once per thread (slow first chunk, memory per session); compare `--threads` 1-3. The GPU/ANE may run in fp16, so logits can drift more than on CPU: check the max logit diff (`ffa --dump` with both runtimes) and that grid within 20 ms stays at 44/50 on dev (165/205 on test). If CoreML is not clearly faster per track than rten at similar memory, drop the feature.

## R10 first expand (2026-10-31, `r10-expand`, BLOCKED)

Goal was the cold first expand (first time the expanded player and its waveform view mount) at or under 30 ms longest task, pixels unchanged. Not reached; no app code changed. What is left is a decision (below).

Harness: `FF_PERF_PROFILE=first-expand` in `app/smoke/perf-q50.spec.ts` opens a fresh page per run (10k fixture state via `FF_PERF_STATE`), loads row 3, waits 1.5 s, expands once and records the longest task and click to detail drawn. `FF_PERF_TRACE_OUT=x.json` writes Chromium traces of the first three runs (`x-0.json` ...), `FF_PERF_PROFILE_OUT` a CPU profile of run 0. Raw files in `~/.cache/r10-expand/`.

The box was at load 30-60 the whole time (other workers), so wall-clock numbers are inflated about 3x; the thread CPU time (`tdur`) from the traces is the stable number and matches the 82-93 ms that q50-render measured at load 4-8.

| Before (`ov-1028`), 10k state | Value |
|---|---|
| longest task, wall, median of 7 (load 52-61) | 268 ms |
| click to detail drawn, wall, median of 7 | 438 ms |
| click task, thread CPU, 3 traces | 19-25 ms |
| frame task, thread CPU, 3 traces | 97-100 ms |

Where the frame task's ~100 ms CPU goes (median of 3 traces): animation-frame callback 63 ms = forced style and layout of the new panel ~18, zoom view first paint ~30 (`drawScrolledColumns` paints the full width of the ring once; later frames paint only the few new columns, 2-4 ms), overview ~7, rest of the paint ~8; then native paint 12, layerize 8, commit 7, Svelte microtasks 6, pre-paint 2. A playing idle frame in this headless software compositor already costs 12-15 ms (layerize + commit).

Tried, no gain, reverted:
- **Pre-warm** (`WaveformDisplay.warmUp`: paint the loaded track's data once in a hidden 640 px expanded display at idle after load). It ran, and the first real paint still cost the same 58-63 ms: the cost is per-pixel work for the full-width ring, not cold code.
- **Layout in a task before the frame** (`setTimeout(0)` that forces layout after mount). Chromium runs the frame straight after the click task, so the queued task lands after it; layout stays inside the frame.

Why 30 ms needs a decision: everything in the frame is needed for the first visible frame of the expanded panel (its layout, a full-width zoom paint, the overview, raster). Getting under 30 ms means doing that work in earlier frames while the panel is not yet shown:

1. **Staged reveal**: mount the expanded player hidden at its final size in the click task (the compact player stays and keeps animating), take canvas sizes from the `ResizeObserver` instead of a forced layout, pre-paint the zoom ring and the overview cache in chunks of a few ms across tasks, then swap. The first visible frame blits and paints only the columns played meanwhile, so pixels are the same, but the panel shows a few frames later than the click's frame. Estimate on this box: reveal frame ~30-40 ms CPU (relayout after the player height change, panel paint, compositor), click and chunk tasks under 20. Touches `Player.svelte` and `waveform-display.js` (`paint: 'frame'` mount path).
2. **Measure on the Mac first**: add the first expand to `scripts/mac/perf-night.sh`; WebKit with GPU compositing spends far less per frame than this headless software compositor. Build option 1 only if the Mac shows a task over 50 ms.
3. **Draw the zoom view one frame after the panel** (q50-render's option): ~30 ms off the task, but the panel's first frame shows an empty zoom area. Visible, so not done.

Decided (orchestrator, 2026-10-31): measure on the Mac first. `scripts/mac/perf-night.sh` now launches the app `--expand-runs` times (default 5), each with the probe step `expand` (`perf_probe.js` `firstExpand`: load row 3, wait for `ff:player-ready` plus 1.5 s, click expand, record frame gaps until the zoom view shows `data-bands="detail"` plus two frames). `summary.md` gets a "Cold first expand (R10)" table with the median longest frame and click to detail drawn. WKWebView has no long-task API, so the longest frame gap stands in for the longest task. **Trigger: a median longest frame over 50 ms on the Mac means building the staged reveal (option 1 above); at or under 50 ms it is not built.**

## R11 CoreML (2026-11-01, `r11-coreml`)

Branch `r11-coreml` = `r9-ort` + `ov-1028`, not landed. Mac: `mbp`, Apple M5 Pro (15 cores: 5 performance, 10 efficiency), 24 GB, macOS 26.3. Headless clone `~/dev/build/fourfour-r11` (git bundle from the box); `~/dev/build/fourfour-mac` and /Applications untouched. Merlin was using the Mac, so only builds and a 3-track check ran, all under `nice -n 10 taskpolicy -b` (efficiency cores only): no number below is representative of speed.

### Build on the Mac

- **The `ort` build did not link on macOS.** The prebuilt ONNX Runtime 1.31.0 for `aarch64-apple-darwin` (pyke, CoreML build, downloaded to `~/Library/Caches/ort.pyke.io/`) calls `___isPlatformVersionAtLeast` (clang's `@available` helper in compiler-rt), and rustc links with `-nodefaultlibs`, so `ffa --features coreml` failed with an undefined symbol. Fix: `fourfour-analysis-rs/build.rs` links `libclang_rt.osx.a` from `clang --print-resource-dir` when the target is macOS and `ort` is on (a no-op everywhere else). This also affects `bundle-app.sh --features coreml`; R9's Mac steps could not have worked without it.
- Linker warnings remain that the ORT objects were built for macOS 13.4 while the link targets 11.0: harmless here (macOS 26), but the `.dmg` with `ort` effectively needs macOS 13.4+.
- `ffa --features coreml` (release, LTO, stripped): 24.0 MB, 2 min 12 s incremental at background priority.

### Sanity (3 dev tracks, background priority)

Not finished: the Mac went offline (asleep or off the tailnet) about 15 min into the check, while the one-chunk test was still compiling at background priority, and did not come back within the timebox. The check is `scripts/mac/coreml-bench.sh --no-build --limit 3 --reps 1 --rten "1 1" --coreml "1 1" --rounds 3 --background` (output `~/.cache/r11-work/sanity/`); it may have finished on its own once the Mac woke. Still unknown: whether CoreML takes the graph (node coverage), the logit diff vs rten, and the per-chunk time.

### Tonight's run (`scripts/mac/coreml-bench.sh`)

Headless (no window, no screen recording). Builds `ffa --features coreml`, runs the one-chunk test (rten vs CoreML in one process, wall ms per chunk and session open time), then `ffa` on the same track list for each runtime/config, interleaved, `--reps` times, with `/usr/bin/time -l` and `ORT_LOG=info` (the CoreML EP logs how many graph nodes it took). `scripts/mac/coreml-bench-summary.py` writes `summary.md`: tracks/min, first result, model load, peak RSS, model seconds, CoreML node coverage, median per-chunk model ms per run, max logit diff and beat/grid agreement vs rten, and the `o3-score.py` grid eval for every config. Dry run on the box: `scripts/mac/coreml-bench.sh --dry-run --o3 benchmark/results/o3`; the summary was checked against the R9 dumps (reproduces R9's 2.0e-4 logit diff, 50/50 peaks, 44/50 grid).

Track data: the dev (50) and test (205) splits are all on the Mac under `~/local/2-Areas/1-Mukke/#02_DJ` (matched by file name and size, 255/255; the 10k fixture is not on the Mac). `~/.cache/r11-work/o3/` holds `ref.json`, `dev.txt`, `test.txt` rewritten to those paths (three box paths are duplicates of one Mac file each, so three test-split ids collapse); weights are in `~/.cache/fourfour-models`.

```bash
ssh mbp
cd ~/dev/build/fourfour-r11 && export PATH=$HOME/.cargo/bin:$PATH FFA_TARGET_DIR=$PWD/target-ffa
scripts/mac/coreml-bench.sh --split dev --reps 2 --rten "2 6;1 12" --coreml "1 1;2 1;2 2;4 2" --rounds 7
# optional, grid on the test split for the best CoreML config only (about 4x the dev time):
scripts/mac/coreml-bench.sh --no-build --split test --reps 1 --rten "2 6" --coreml "2 2" --rounds 3
```

Output: `build/mac/coreml-<date>/summary.md`. Expected time for the dev command: 12 `ffa` runs over 50 tracks; at rten's box rate (5-10 tracks/min) about 5-10 min each, so 1-2 h; CoreML runs shorter if it helps.

Decision rule (as R9): keep `coreml` only if a CoreML config beats the best rten config on tracks/min at similar or lower peak memory, with grid within 20 ms still 44/50 on dev and max logit diff small enough that beat peaks match on (nearly) all tracks. Otherwise drop the `ort`/`coreml` features.

## R24 CoreML fix (2026-11-01, `r24-coreml`)

Branch `r24-coreml` = `r11-coreml` + `ov-1028`, not landed. Mac `mbp`, clone `~/dev/build/fourfour-r11`, everything under `nice -n 10`, headless.

**Cause.** The exported model takes `spectrogram` as `[batch, time, 128]` with free dimensions, so the partial attention blocks compute their Slice `starts`/`ends` at run time from `Shape`/`Gather`/`Unsqueeze` chains (467 Shape, 425 Gather, 496 Unsqueeze nodes). ONNX Runtime 1.31's CoreML EP takes those shape nodes into its partitions (1372 of 1753 nodes) and hands the CPU Slice nodes left outside a 0-D tensor where the graph has a 1-D one, so the run fails at `/model/frontend/blocks/blocks.1/partial/attnF/Slice_6` ("Starts must be a 1-D array"). Reproduced with the Python `onnxruntime` 1.31.1 wheel on the Mac; ORT 1.19 does not fail but only takes 742 of 1813 nodes in 201 partitions and is slower than the CPU.

**Fix (no model change).** The CoreML session is opened with free dimension overrides `batch = 1`, `time = 1500` (the chunk size): ORT then folds the whole shape arithmetic into constants before partitioning (1052 nodes left), CoreML takes 904 of them in 75 partitions, and every Slice gets constant 1-D inputs. Chunks of another length (only tracks shorter than one chunk, about 30 s) go to a CPU session opened on first use. The compiled CoreML model is cached in `~/Library/Caches/fourfour-coreml/<size>-<mtime>/` (keyed by the model file, so a new model gets a new cache). The rten path and the model file are unchanged. Code: `fourfour-analysis-rs/src/ort_beat.rs`.

EP options tried first (Python, one 1500-frame chunk, single thread, Mac partly busy):

| CoreML config | Session open | Chunk run | Max logit diff vs CPU |
|---|---:|---:|---:|
| MLProgram, ALL, dynamic shape (as R11) | - | fails (Slice_6) | - |
| MLProgram, ALL, fixed shape, no cache | 19.1 s | 0.21 s | 3e-6 |
| same, cache cold / warm | 18.8 s / 3.8 s | 0.22 s | 3e-6 |
| MLProgram, CPUAndGPU, fixed | 17.0 s | 0.26 s | 3e-6 |
| MLProgram, CPUAndNeuralEngine, fixed | 16.0 s | 0.76 s | 5e-6 |
| MLProgram, CPUOnly, fixed | 11.2 s | 0.86 s | 5e-6 |
| CPU EP (reference) | - | 0.58-0.84 s | - |

`ALL` (in effect the GPU) is the fastest; the Neural Engine does not help this model. No offline graph edit was needed.

**Acceptance (Rust, release, `--features coreml` on the Mac).** `ort_gives_the_rten_logits_for_a_chunk` passes: max abs logit diff 7.2e-6 (limit 1e-2), median of 5 one-thread chunks rten 1593 ms vs CoreML 283 ms, warm session open 3.3 s (cold about 19 s, was 42 s and failing), CoreML nodes 904 of 1052 (75 partitions). A 3-track `ffa` run (1 worker, 2 threads) worked end to end, but peak RSS was 3.2 GB (rten 0.8 GB in R11's sanity): every chunk thread holds its own CoreML session, so memory grows with threads. Box: `cargo test -p fourfour-analysis-rs` default and `--features ort` pass, including the ignored chunk test on ORT CPU.

**Bench (verdict: keep).** Run by the waiting driver after the perf night: `coreml-bench.sh --split dev --reps 2 --rten "2 6;1 12" --coreml "1 1;2 1;2 2;4 2" --rounds 7` on `mbp` (M5 Pro), 50 dev tracks, `nice -n 10`, warm compiled-model cache. From `~/dev/build/fourfour-r11/build/mac/coreml-r24-dev/summary.md` (`w` = tracks in parallel, `t` = beat-model chunk threads, so CoreML sessions):

| Run | Tracks | Tracks/min | First result | Model load | Peak RSS | Model s (sum) | CoreML nodes |
|---|---:|---:|---:|---:|---:|---:|---|
| coreml-w1-t1-r1 | 50 | 18.00 | 10.5 s | 3849 ms | 2174 MB | 154 | 904 of 1052 |
| coreml-w1-t1-r2 | 50 | 17.79 | 10.5 s | 3875 ms | 2042 MB | 155 | 904 of 1052 |
| coreml-w2-t1-r1 | 50 | 21.22 | 10.2 s | 3601 ms | 2308 MB | 264 | 904 of 1052 |
| coreml-w2-t1-r2 | 50 | 20.49 | 10.3 s | 3694 ms | 2376 MB | 274 | 904 of 1052 |
| coreml-w2-t2-r1 | 50 | 30.89 | 11.6 s | 3553 ms | 3682 MB | 175 | 904 of 1052 |
| coreml-w2-t2-r2 | 50 | 26.10 | 11.5 s | 3634 ms | 3830 MB | 205 | 904 of 1052 |
| coreml-w4-t2-r1 | 50 | 31.97 | 11.1 s | 3585 ms | 4300 MB | 168 | 904 of 1052 |
| coreml-w4-t2-r2 | 50 | 25.91 | 11.4 s | 3975 ms | 4064 MB | 205 | 904 of 1052 |
| rten-w1-t12-r1 | 50 | 14.25 | 4.7 s | 15 ms | 5229 MB | 202 | - |
| rten-w1-t12-r2 | 50 | 12.65 | 5.1 s | 14 ms | 5277 MB | 226 | - |
| rten-w2-t6-r1 | 50 | 13.83 | 5.8 s | 21 ms | 3492 MB | 420 | - |
| rten-w2-t6-r2 | 50 | 12.41 | 7.0 s | 19 ms | 3482 MB | 468 | - |

- CoreML at 2 sessions (w2t2, w4t2) runs 26-32 tracks/min against rten's 12-14, about 2x, at 3.7-4.3 GB peak RSS against rten's 3.5-5.2 GB. One chunk on one thread: rten 1424 ms, CoreML 229 ms; session open 2.9 s warm.
- First result is about 11 s against 5 s: the model load (3.6 s warm) plus one session per chunk thread opened on its first chunk. R26 bridges this with rten.
- Agreement with rten (`ffa --dump`, every CoreML config): max abs logit diff 1.9e-4 (median per track 1.1e-4), beat peaks equal on 49/49, final grid within 1 ms and BPM within 0.01 on 49/49 (the summary pairs 49 of the 50 tracks). Grid eval identical for every config, CoreML and rten: grid within 20 ms 44/50, BPM within 0.1 47/50, bar right 41 of 46 aligned.
- The decision rule from R11 holds (faster at similar memory, grid 44/50, peaks match), so `coreml` stays and ships in R26.

## R26 ship (2026-11-01, `r26-coreml-ship`)

Branch `r26-coreml-ship` = `ov-1028` + `r24-coreml`, not landed. CoreML is now the beat-model engine of the macOS app, with rten as the bridge and the fallback. Linux and Windows builds are unchanged: no `ort` in them, rten only.

### What ships

- **macOS bundles build with `--features coreml`**: `scripts/mac/build-dmg.sh` passes it, and `fourfour-analysis-rs/scripts/bundle-app.sh` adds it when run on Darwin. The CI workflow lives on `ov-ci` and calls `build-dmg.sh` for the Mac job, so it picks this up when both land; the first build downloads ONNX Runtime from `cdn.pyke.io` (network needed) and needs Xcode's clang for `libclang_rt.osx.a` (R11). Apple Silicon only: pyke has no Intel macOS build, so an `x86_64-apple-darwin` or universal bundle does not build with this feature.
- **`bundle.macOS.minimumSystemVersion` is `13.4`** in `pioneer-test-ui/tauri.conf.json`: the ORT objects are built for macOS 13.4, so the app now needs Ventura 13.4 or later (it was Tauri's default 10.13). Tauri sets `MACOSX_DEPLOYMENT_TARGET` from it, which also removes R11's linker warnings.
- **Engine choice at run time** (`fourfour-analysis-rs/src/bt.rs`, `src/ort_beat.rs`). rten always loads (15 ms). With the `coreml` feature (and `FOURFOUR_BEAT_RUNTIME` not `rten`) a background thread opens the CoreML sessions at engine load: the first alone (so a new model compiles into the cache once), then the rest side by side. Until they are open, every chunk thread runs rten as before; then each thread with a session runs CoreML, and the threads without one finish their chunk and stop, leaving 2 chunk threads. Chunks that are not 1500 frames long (tracks under about 30 s) run on rten. If the environment, the cache directory, a session open or a chunk run fails, the reason is logged once (`beat model: coreml unavailable, using rten: ...`), the engine switches to rten for good, and the chunk-thread cap lifts back to the rten slot count. The app's analysis log line says `beat model=coreml` or `beat model=rten (coreml failed: ...)`; `bench_analysis` prints `BEAT runtime=... chunks_accelerated=N chunks_rten=M fallback=...`.
- **Config: 2 CoreML sessions** (`ORT_SESSIONS`), the app's worker plan unchanged. R24's sweep: 1 session 18-21 tracks/min, 2 sessions 26-32; `w2t2` and `w4t2` gave the same tracks/min (medians 28.5 and 28.9) and `w4` cost 0.3-0.5 GB more, so the number that matters is the session count (each session holds its own compiled model and activations), and 2 it is. The app's workers (6 on the M5 Pro, 2 threads per track) do decode, mel, key and the fast pass on the CPU as they did with rten; more of them feed the 2 sessions, they do not add sessions.
- **Compiled-model cache** `~/Library/Caches/fourfour-coreml/<content hash>-<chunk frames>/`: the key is a hash of the model file's bytes (was size and mtime), so any model change compiles afresh; entries of other keys are removed when a new key is created. If opening a session with the cache fails, the entry is deleted and the open retried once; if that fails too, rten takes over.
- Analysis output format unchanged; no new dependencies.

### Mac validation (`mbp`, M5 Pro, `~/dev/build/fourfour-r11`, `nice -n 10`, headless)

`bench_analysis --backend rust` (the app's runner and engine selection, staged) built with `--features coreml,dev-bins`; the same binary with `FOURFOUR_BEAT_RUNTIME=rten` for the rten rows; the first 12 (or 1) tracks of the dev split. Load on the Mac was 4-38 from other work during the runs, so read pairs, not absolute numbers.

| Run | Tracks/min | First result | Peak RSS | Chunks CoreML / rten |
|---|---:|---:|---:|---:|
| 12 tracks, CoreML (load 5) | 31.3 | 3.0 s | 7.7 GB | 114 / 50 |
| 12 tracks, CoreML (load 35) | 25.1 | 5.6 s | 7.1 GB | 114 / 50 |
| 12 tracks, CoreML, sessions opened one by one (loads 8-15) | 28.8, 26.9 | 3.5, 3.9 s | 7.0 GB | 114 / 50 |
| 12 tracks, rten (loads 9-16) | 18.7-20.4 | 3.6-5.8 s | 6.2-6.3 GB | 0 / 164 |
| 12 tracks, CoreML, cold cache (compiles, load 38) | 19.5 | 4.0 s | 7.4 GB | 10 / 154 |
| 12 tracks, CoreML before the rten bridge | 30.2, 31.5 | 6.5, 7.1 s | 5.0-5.1 GB | 158 / 6 |
| 12 tracks, rten, same session as the row above | 22.0, 22.9 | 3.1, 3.2 s | 6.3 GB | 0 / 164 |
| 1 track, CoreML | - | 2.8 s | 6.0 GB | 0 / 13 |
| 1 track, rten | - | 2.7-2.8 s | 5.5 GB | 0 / 13 |
| 1 track, CoreML before the rten bridge | - | 10.6 s | 3.1 GB | 1 / 12 |

- **Throughput**: CoreML 25-31 tracks/min against rten 19-23 in the app path, about 1.4x (the app's rten already runs 12 chunk threads and staged CPU work, so the gap is smaller than `ffa`'s 2x).
- **First result / "analyze this track now"**: the model load did dominate. Without the bridge the 2 CoreML threads waited for their sessions (3.6 s warm each) and a single track took 10.6 s against rten's 2.7 s; with the bridge a single track runs entirely on rten while the sessions open (2.8 s), and a priority request 3 s into a 12-track run (`--play-after 3 --play-index 11`) was fully analysed 6.2 s later (rten 5.7 s). No early warm-up at app start was needed.
- **Memory**: peak RSS is 0.8-1.4 GB above rten's because for a few seconds the rten bridge threads and the opening CoreML sessions overlap; before the bridge CoreML peaked 1.2 GB below rten. Steady state after the bridge is 2 sessions. If the peak matters, cap the bridge threads; not done here, first result was the brief.
- **Results** (`--dump`, stored app results, 12 tracks): CoreML vs rten keys 12/12 identical, BPM within 0.01 12/12, every beat time identical to the millisecond on 12/12 (warm, cold and pre-bridge runs). R24's dev-split numbers (49/49 peaks, grid within 1 ms) stand for the model itself.
- **Fallback**: `HOME=/nonexistent` (cache directory cannot be created) logged `beat model: coreml unavailable, using rten: Read-only file system (os error 30)` once and analysed on rten (13/13 chunks rten, result stored). **Broken cache**: every file of the cache entry truncated to 0 bytes; the next run opened CoreML without error (the entry was rebuilt, 408 non-empty files after), took about 13 s for one track on the rten bridge while recompiling, and the run after that used the cache again.
- **Bundle**: `scripts/mac/build-dmg.sh` built `fourfour_0.1.0_aarch64.dmg` (97 MB) with `coreml`, no linker warnings; `scripts/mac/check-dmg.sh` on it: failures=0 (ad-hoc signature, weights and licences, plist, `--self-check` ok). Inside: `LSMinimumSystemVersion` 13.4, binary `minos 13.4`, only the app binary in `Contents/MacOS` (59 MB). Not installed anywhere.
- **Dev bins moved to `pioneer-test-ui/src/dev_bin/`** (was `src/bin/`): tauri-cli 2.10 adds every file under `src/bin` as a bundle binary even when its `[[bin]]` has unmet `required-features`, so a clean macOS bundle (this clone, and so the CI runner) failed with `Failed to copy binary ... bench_startup does not exist`, and a target dir that had them (the perf-night clone) shipped them. r25's `required-features` fix alone did not cover that. The `[[bin]]` paths point at the new directory; nothing else changed.

## R30 CoreML memory (2026-11-02, `r30-coreml-mem`)

Branch `r30-coreml-mem` = `ov-1028` + one commit, not landed. Mac `mbp` (M5 Pro, 24 GB), clone `~/dev/build/fourfour-r11`, `bench_analysis --backend rust` built with `--features coreml,dev-bins`, `nice -n 10`, headless, first 12 (or 1) tracks of the dev split, peak RSS from `/usr/bin/time -l`. The Mac was busy with other work (load 8-38), so read the interleaved pairs, not absolute numbers.

**Where the extra memory was.** On the 24 GB Mac the engine shares 13 chunk slots (`chunk_slots`, 400 MB each). R26's bridge kept all 13 rten threads running until both CoreML sessions were open; the first session opens about 4 s in, the second about 8 s in (one after the other), so for about 4 s one running session (about 1-1.5 GB, R24) sat on top of 12 rten chunks. That window, not the session open, was most of the 1.3-1.5 GB over rten: a single track (13 chunks, done before the first session opens) was only 0.4 GB over rten, before and after.

**Fix** (`fourfour-analysis-rs/src/bt.rs`, `src/ort_beat.rs`). Each open session takes 6 rten slots off the bridge (`RTEN_CHUNK_SLOTS_PER_OPEN_SESSION`): with 13 slots the engine runs 13 rten threads until the first session opens, then that session plus 7 rten threads, then the 2 sessions. A thread without a session leaves when more threads run than that (it finishes its chunk first); the count of running chunk threads is kept under the queue lock, so no more leave than needed. If CoreML fails, the cap lifts back to the rten slots as before. Without the `ort` feature nothing changes.

Tried and dropped (12 tracks unless noted, same session):

| Variant | Peak RSS | Tracks/min | First result | Verdict |
|---|---:|---:|---:|---|
| R26 as shipped | 7.1-7.4 GB | 24-33 | 3.3-5.2 s | baseline |
| ORT CPU arena off | 7.3 GB | 31.0 | 3.6 s | no effect |
| `malloc_zone_pressure_relief` when a bridge thread leaves | 7.1 GB | 31.3 | 3.6 s | frees 0 MB |
| Bridge capped at 6 / 8 / 10 threads from the start | 5.3 / 5.6-5.7 / 6.1 GB | 31-33 | 4.7-5.8 s | single track 5.0-5.4 s (was 3.2 s) |
| Each open session takes 3 / 4 / 5 slots | 6.7 / 6.1-6.5 / 6.3 GB | 26-32 | 3.2-4.5 s | 4 and up work |
| Each open session takes 6 / 8 slots | 6.1-6.2 / 6.1 GB | 28-29 / 28 | 3.8-4.2 / 3.9-4.6 s | 6 kept |
| 6 slots, plus 1 slot reserved while a session opens | 5.8 GB | 23-28 | 5.3-7.2 s | single track 5.7 s, dropped |

**Before/after** (final binary, interleaved runs, loads in brackets):

| Run | Before (R26) | After (R30) | rten |
|---|---|---|---|
| 12 tracks, peak RSS | 7.34, 7.13, 7.29 GB | 6.07, 6.21, 6.12 GB | 5.80, 5.89 GB |
| 12 tracks, tracks/min | 32.9, 31.1, 25.8 | 27.2*, 30.1, 26.4 | 21.4, 19.3 |
| 12 tracks, first result | 3.3, 3.8, 5.2 s | 3.0, 3.6, 4.6 s | 3.4, 3.7 s |
| 1 track (load 33), time / peak RSS | 3.7 s / 5.53 GB | 3.5 s / 5.54 GB | 3.6 s / 5.12 GB |
| Priority (`--play-after 3 --play-index 11`), to full / peak RSS | 7.1 s / 7.00 GB | 6.4 s / 6.45 GB | 7.2 s / 5.79 GB |

\* first run of a freshly built binary: CoreML ran only 46 of 164 chunks. Every first run of a new binary in this session was like that (sessions open late) although the compiled-model cache entry stayed in place, so macOS seems to compile GPU code per executable once; the app will see this once after each update.

- **Memory**: peak RSS down 1.0-1.2 GB to 6.1-6.2 GB on 12 tracks, at R26's rten level (6.2-6.3 GB) but still 0.2-0.4 GB above rten measured in the same session (5.8-5.9 GB); priority case 0.55 GB down, 0.65 GB above rten. The rest is the first session opening while all 13 rten slots run (the single-track 0.4 GB, unchanged). Closing it means fewer rten threads at the start, which costs every track of 13+ chunks one more chunk round (+2 s for a single track, row above), so it stays.
- **Latency and throughput kept**: single track 3.5 s (before 3.7 s, rten 3.6 s, all at load 33), priority track fully analysed 6.4 s after the ask (before 7.1 s), 12 tracks 26-30 tracks/min (before 26-33 in the same pairs, rten 19-21).
- **Results**: keys, BPM and every beat (time, tempo, bar position) of the stored results identical to rten on 12/12 tracks for every after run (plain and priority) and 1/1 on the single track.
- Box: `cargo test --workspace` and `cargo test -p fourfour-analysis-rs --features ort` pass.

## R13 50k (2026-11-01, `r13-50k`)

Branch `r13-50k` from `ov-1028` (`ea0ddaa`). Linux `dev` box, headless Chromium, release `bench_startup`. Box load 25-55 the whole time (other workers): absolute times are inflated about 2x against a quiet box; read the 10k-to-50k ratios and the before/after of the same step, not the absolute values.

**Fixture.** `scripts/ff-fixture.py 50k --link-from 25k` builds the 50k fixture from hard links to the 25k fixture's files (`music/copy1`, `music/copy2`; no new audio on disk, a real 50k copy would be about 35 GB) and the library with the normal `bench_startup fixture-lib` scan: 50,000 tracks, 40,722 covers, 34,890 analysed, 12,520 rated, 3,935 coloured, 74 playlists. Titles of `copy2` get a " (copy 2)" suffix in the library so no artist and title pair repeats (same reason as the generator's redraw). `library.db` is 7.5 GB (covers are stored in it), 1.5 GB at 10k. Import samples: 1,000 hard-linked files new to each library (from the 25k fixture for the 10k library, from the 10k fixture for the 50k one; 2-3 duplicates each). Frontend scroll/search/sort use the synthetic smoke fixture (`FF_PERF_TRACKS`), startup the real `load_state` JSON of the fixture (`bench_startup dump`).

**Fixed**

- **Marking tracks for analysis was quadratic.** `markTracksPendingAnalysis`, `knownTrackIds` and `requestBackgroundAnalysis` looked up each id with a linear `find` over the reactive rows. Analyse a 2k selection: 25.7 s of frozen UI at 10k, over 5 min at 50k (the spec timed out). Now the indexed `libraryTrackById`: 0.56 s at 10k, 0.67 s at 50k, i.e. it no longer depends on library size. The same lookup is used by the track edits (`track-edits.svelte.ts`).
- **The palette scanned every row proxy per keystroke.** The header search opens the anchored palette, whose `sources` re-runs every action's `when` on each keystroke; "Analyze all unanalyzed" ran `library.tracks.some((t) => !t.analyzed)` through 50k row proxies (about 650 ms of a 14-key run, the largest item in the profile; in a fully analysed library it never stops early). It is now `hasUnanalyzedTracks()` on the plain track objects, like `unanalyzedTrackCount`; the context menu uses the same.

Same results: the lookups return the same row objects (the index falls back to `find`), and `analyzed` on the plain objects is kept in step by `bumpLibraryTracks`. No BPM, grid, key or library data path changed.

**10k vs 50k** (before = `ov-1028`, after = this branch; frontend medians from one `perf10k` run each, before 50k at load about 45, after at about 30)

| Metric | 10k before | 10k after | 50k before | 50k after | budget |
|---|---:|---:|---:|---:|---:|
| analyse 2k selected tracks (one task) | 25,745 ms | 559 ms | > 300 s | 665 ms | |
| search keystroke median / worst | 73 / 116 ms | 62 / 100 ms | 188 / 389 ms | 77 / 111 ms | 50 ms |
| palette open | 100 ms | 67 ms | 260 ms | 102 ms | |
| palette filter keystroke median | 88 ms | 60 ms | 225 ms | 48 ms | 50 ms |
| scroll frame p95 (steady) | 43 ms | 54 ms | 72 ms | 45 ms | |
| fling frame p95 / max | 67 / 78 ms | 62 / 65 ms | 146 / 196 ms | 92 / 92 ms | 50 ms |
| sort by title (asc) | 127 ms | 119 ms | 398 ms | 283 ms | |
| select to inspector median | 29 ms | 27 ms | 34 ms | 25 ms | |
| JS heap after boot (after GC) | 23 MB | 23 MB | 92 MB | 92 MB | |
| backend `load_state` warm (median of 3) | 118 ms | same | 480 ms | same | |
| `load_state` payload | 4.2 MB | same | 21.6 MB | same | |
| backend RSS, `bench_startup run` | 26 MB | same | 87 MB | same | |
| startup to interactive, first launch / cold / warm | 1,860 / 1,722 / 1,432 ms | same | 3,193 / 2,914 / 2,502 ms | same | 1.5 s |
| startup longest task / TBT, warm | 274 / 438 ms | same | 330 / 985 ms | same | |
| import 1k files: scan + plan | 400-704 ms | same | 1,415-3,366 ms | same | |
| import 1k files: duplicate plan alone | 175-181 ms | same | 1,038-1,469 ms | same | |
| import 1k files: commit (insert) | 86-114 ms | same | 242-5,005 ms | same | |
| import 1k files: covers | 1.34-1.40 s | same | 1.20-1.28 s | same | |

"same" = code path not changed here. Startup used `perf-startup.spec.ts` with the backend time simulated (118 / 480 ms), median of 3, load 31; R2b measured 1,263 ms warm at 50k on a quieter box with a smaller payload (16 MB synthetic paths against 21.6 MB real ones).

**Scaling read.** Heap (4x), payload (5.1x), `load_state` (4x), backend RSS (3.4x) and startup (about 1.7x) grow linearly or better. Sorts grow as n log n. Search, palette and scroll were not size-bound after the fix. The two quadratic paths found are fixed. The import's duplicate plan grows 6-8x for 5x tracks: it reads every track and folds artist and title (Unicode NFKD) for the whole library on every import.

**What is left**

- Startup at 50k is over the 1.5 s budget on this box (interactive 2.5-3.2 s at load 31). The profile is spread out: search index warmup about 230 ms, GC 300 ms, `$state` proxies, first rows; nothing over 15% alone. Next steps are R2b's: lazy row proxies, and a smaller payload (21.6 MB at 50k, mostly paths).
- Import plan at 50k: 1.0-1.5 s for the duplicate index before the dialog. Option: keep the folded keys in a column or cache them between imports (schema change, needs Merlin).
- Import commit at 50k swung 65 ms to 4.7 s for the same 1,000 inserts: the bench copies the 7.5 GB `library.db` first, so writeback of that copy competes. Covers stored inside `library.db` make it 5x larger than everything else in it; storing covers as files is a schema change.
- Analyse 2k selected still takes a 0.5-0.6 s task (marking rows, bumping the list); it no longer depends on library size.
- Not measured here: WKWebView on the Mac (`scripts/mac/perf-night.sh --size 50k` now has a fixture to run on; on the Mac use `--link-from 25k` too).

Reproduce: `scripts/perf.sh --size 50k` now builds the linked fixture (set `FF_FIXTURE_FULL=1` for real copies), or by hand `python3 scripts/ff-fixture.py 50k --link-from 25k --bench <target>/release/bench_startup`; frontend `FF_PERF_TRACKS=50000 FF_SMOKE_PORT=5314 FF_PERF_OUT=x.json npx playwright test perf10k --grep measure`; startup `FF_PERF_STARTUP=50000=state50k.json==480 npx playwright test perf-startup`; import `bench_startup import <1k folder> <work> ~/.cache/ff-fixture/50k/library/library.db`. Raw: `~/.cache/r13/`.

## R33 duplicate plan (2026-11-01, `r33-dupplan`)

Branch `r33-dupplan` from `ov-1028`. Linux `dev` box, release `bench_startup import` on the R13 fixtures (10k, and the 50k linked one), the same 1,000-file sample folders as R13. Box load 16-47 during the runs (other workers), so absolute times move a lot; each pair below is a before run and an after run back to back.

**Problem.** R13: the duplicate plan of an import grows 6-8x for 5x tracks (175 ms at 10k, 1.0-1.5 s at 50k) because every import read all tracks and folded artist, title (Unicode NFKD), path and file name again. Caching folded keys in a column would be a schema change; this does it in memory.

**Design.**

- `DuplicateIndex` (`pioneer-library/src/duplicates.rs`) is now a mutable index: `insert_all(candidates)` and `remove(candidate)` beside `new` and `find`. Per key it keeps the lowest id and the rest sorted by id (`Slots`), so removing a track promotes the next one and a key shared by several tracks answers exactly as a fresh index over the library in id order does. It holds only keys and ids, not the track fields: about 37 MB at 50k tracks (about 7 MB at 10k; measured with 50k synthetic candidates of realistic path length), built from `DuplicateCandidate`s as before.
- `LocalLibrary` owns the index (`duplicates: Mutex<Option<DuplicateIndex>>`) and hands it out through `with_duplicate_index`: built on the first import of a session, reused by every later one. It lives and dies with the `LocalLibrary` value, so change library, reset, restore and every reopen start with no index. `plan_for_library` uses it; `plan_import` takes the index plus a lookup for the existing track of each hit (title, artist, duration for the dialog row) instead of building an index and an id map itself.
- Writes keep it current: the write paths read the affected tracks' stored values from the database before the write (one primary-key query each, only when an index exists), and after it succeeds take the old keys out and put the new ones in. A failed write changes nothing. A write that changes no matched field still does one remove and one insert of the keys.
- Safety net: before each use the index size is compared with `COUNT(*)` of the tracks table; a mismatch (a write path that missed an insert or delete) rebuilds it.

**Invalidation paths covered.**

| Write | What happens to the index |
|---|---|
| `add_track`, `add_tracks`, `add_tracks_covers_later` (import commit) | the new rows are inserted (after the commit; a failed batch adds nothing) |
| `update_track` (tag edits, analysis result with track fields) | old keys out, new in |
| `commit_analysis` with a track | old keys out, new in |
| `remove_track`, `remove_tracks` | keys out |
| `relink_track`, `relocate_in_folder` | path and file keys replaced |
| `import_from_usb`, Rekordbox import, backup restore path relinking | index dropped, rebuilt on the next import |
| change library, reset, restore, reopen | new `LocalLibrary`, no index |
| any other change in the track count | rebuilt by the count check |

The only direct writers of `tracks` are these (`queries::insert_track`, `update_track`, `delete_tracks`, `update_source_path`, checked with grep); the schema migrations run at open.

**Equivalence tests.** `the_cached_duplicate_index_equals_a_fresh_scan_after_random_writes`: 600 random writes (adds with colliding paths, case variants, accents, equal tags and sizes, batch adds, updates that change path, size, tags and duration, single and double removes, analysis commits with a changed track, failing writes), after every one the cached index must equal (`PartialEq` over every map) `duplicate_index()` built from a full scan. Also relinks and relocations, the count check, tie handling in `Slots`, and `a_plan_from_the_cached_index_equals_one_from_a_fresh_index_after_library_writes` (whole `ImportPlan`, dialog rows included). Breaking the remove path or the analysis commit hook makes the random test fail at the first affected step. The existing import and duplicate tests pass unchanged apart from the new `plan_import` arguments.

**Measured** (`FF_IMPORT_AGAIN=<folder>` added to `bench_startup import`: after the first import is committed, a second import of another 1,000-file folder runs in the same process, which is what a session of several imports does; medians of 3-4 interleaved pairs, ms)

| Step | 10k before | 10k after | 50k before | 50k after |
|---|---:|---:|---:|---:|
| duplicate plan, first import of the session (builds the index) | 190 | 180 | 940 | 910 |
| duplicate plan, every later import | 265 | 67 | 990 | 74 |
| scan + plan, second import (1,000 files, 1,000 of them duplicates) | 435 | 234 | 1,090 | 307 |

The later-import plan in the bench finds 1,000 duplicates (the second folder is the other fixture's copy of the library's own files), which is the worst case: each hit reads its existing track once (the same bench with the track fields kept in the index took 23-34 ms at 10k and 28-33 ms at 50k). With the usual 2-3 duplicates the plan is the key folding of the 1,000 new files plus the lookups, a few ms, and no longer depends on library size. Scan (tag reads) is untouched and is now most of the second import: about 90-250 ms for 1,000 files. The first import of a session still pays the full build (the same folding as before, now with a hash lookup per key instead of two); building it in the background after startup would take the library lock for about a second at 50k and was left out.

**What is left.** First import after launch at 50k: about 0.9 s for the plan, unchanged. Resident cost of the index at 50k: about 37 MB for the rest of the session (backend RSS at 50k was 87 MB).

## R22 startup (2026-11-01, `r22-startup`)

Branch `r22-startup` from `ov-1028`. Linux `dev` box, headless Chromium, `perf-startup.spec.ts` on the real fixture libraries (`bench_startup dump`), backend time simulated as 118 ms (10k) and 480 ms (50k) for both sides so the frontend change is isolated. Box load 20-50 during the pairs below (other workers, swap full), so absolute times are high; each pair is a before run and an after run back to back.

**Changed**

- `load_state` sends the tracks as a column table (`TrackTable` in `pioneer-test-ui/src/dto.rs`, decoded by `app/src/services/track-table.ts` into the same `TrackInfo` objects, same key order): one array per field, folders as a parent tree (each folder sent once), artists, albums, genres, keys and colours as indexes into one string list, flags as a bit field, ratings and colours as sparse pairs. Payload 21.6 to 8.4 MB at 50k, 4.2 to 1.7 MB at 10k; JSON parse at 50k 150-270 to 55-110 ms. Checked on both fixtures: every decoded track deep-equals the old JSON object and its key order (0 diffs at 10k and 50k), playlists unchanged. Browser mocks still pass the old array form, which `loadState` accepts as before.
- Row `$state` proxies are built inside the existing 2,500-track map chunks instead of in one 50k pass after them (same proxies, no long task).
- Track mapping caches the BPM and time texts by value (`toFixed` and `padStart` were half of the mapping); same strings.
- Search warmup: idle slices stop after 8 ms (they ran to the 50 ms idle deadline plus a 250-track overrun, i.e. long tasks right after the first rows), and a track's haystack leaves out `raw` fields equal to the shown field (path, title, artist, album, genre, key), about half the text. Results are identical: query terms never contain spaces, so a match always lies inside one field, and a duplicate field adds no match. A search before the warmup ends builds the rest at once, as before.
- Sidebar: `library.sidebarData` (a getter that rebuilds favourites, the playlist tree and USB rows) was called by each of the nine template reads; `Sidebar.svelte` now reads one `$derived` of it.
- `perf-startup.spec.ts`: `FF_PERF_STARTUP_PROFILE=<prefix>` writes a CPU profile of the last warm reload (with `FF_PERF_STARTUP_PROFILE_FIRST=1` the last first launch) plus the probe marks.

**Frontend** (first real row / interactive, ms; median of the three cleanest pairs, each run the spec's median of 3)

| Tracks | Case | Before | After |
|---|---|---:|---:|
| 10k | first launch | 1,101 / 1,317 | 914 / 1,118 |
| 10k | cold | 1,225 / 1,454 | 964 / 1,234 |
| 10k | warm | 775 / 1,043 | 638 / 827 |
| 50k | first launch | 1,866 / 2,496 | 1,644 / 1,854 |
| 50k | cold | 2,040 / 2,383 | 1,568 / 1,568 |
| 50k | warm | 1,584 / 2,189 | 1,354 / 1,355 |

Quietest single pair (load about 20): 50k warm 1,903 to 1,354 ms, cold 1,886 to 1,568, first launch 2,267 to 1,690; 10k warm 995 to 741. Interactive now equals the first real row in most runs (no long task after it); before, search warmup and GC added 300-700 ms at 50k.

**Backend.** `load_state` wall time is unchanged within the noise at load 45-70 (50k warm 600-950 ms both sides). Building the table costs about what the smaller JSON saves (string interning of 50k rows); total CPU of `bench_startup run` at 50k 1.06 s before, about 1.10 s after.

**Split at 50k warm, after** (CPU profile, load about 25): answer to first real row about 390 ms: decode 50, map and proxies 130, first render 120 (sidebar rows 37, track rows 25), GC 50.

**What is left**

- 50k is at or under 1.5 s warm and cold on this box at load 20-30, first launch is not (1.6-1.9 s): it renders the whole table and its waveforms after the answer with no snapshot to show first.
- `$state` proxy creation for 50k rows (about 60 ms plus GC) is still eager. Creating them lazily needs a way to create state outside the reading effect, which Svelte does not export.
- The backend list query (about 240 ms at 50k) is unchanged; a covering index would be a schema change.
- JS heap after boot not re-measured (perf10k uses the synthetic mock rows, which do not go through the table).
- Real-app IPC on the Mac: WKWebView decodes the same JSON, so the smaller payload should help there too; not measured.

## R23 memory (2026-11-01, `r23-memory`)

Branch `r23-memory` from `ov-1028`. Linux `dev` box, headless Chromium, box load 25-55 the whole time (other workers), so cycle times are long (a cycle took 25 s to 2.5 min); read the growth, not the speed.

**Method.** `app/smoke/soak.spec.ts` (skipped unless `FF_SOAK_MINUTES` is set) loops over: scroll flings (24 frames of 9,000 px), typing and clearing a search, opening and closing the palette, switching playlists (Big Playlist, Peak Time, Warmup, All Tracks), double-click loading a track, opening the expanded player, seeking in the overview four times and closing it, settings open (every page) and close, and 40 `analysis-partial` plus `analysis-progress` events and one `analysis-complete`. It runs on the 10k synthetic tracks (`FF_PERF_TRACKS=10000`) with the zoom detail served by the mock. Every `FF_SOAK_SAMPLE_SECS` it forces a GC (`HeapProfiler.collectGarbage`) and records the JS heap, DOM nodes and event listeners (`Memory.getDOMCounters`), registered Tauri callbacks, live `setInterval` ids and live object URLs. `FF_SOAK_SNAPSHOT_DIR` also writes heap snapshots of the first sample, the middle and the end; `app/perf/heap-diff.mjs first last` lists growth by constructor. `FF_SOAK_ACTIONS=search,palette` runs a subset to bisect, `FF_SOAK_ARTWORK=1` gives the tracks covers. The spec fails when listeners, documents or intervals grow, or more than 2,600 object URLs stay alive. Backend: `bench_startup soak <library dir> [seconds]` runs, on a copy of the library, `load_state` (list, labels, JSON), 20 analysis reads and stores, 40 waveform-detail cache writes and reads, and a playlist save per round and prints RSS over time (`FF_SOAK_PARTS=state,analysis,detail,playlists` to bisect). `scripts/soak.sh [minutes]` (or `npm run soak`) runs both.

**Frontend, 45 min, 10k tracks** (first version of the loop; analysis events used ids up to 10,000, see below). Heap after GC:

| Minute | Cycle | JS heap | DOM nodes | Listeners | Callbacks | Intervals |
|---:|---:|---:|---:|---:|---:|---:|
| 1.7 | 2 | 87 MB | 5,688 | 112 | 17 | 1 |
| 9.4 | 4 | 91 MB | 5,727 | 112 | 17 | 1 |
| 18.8 | 14 | 111 MB | 5,905 | 112 | 17 | 1 |
| 25.6 | 22 | 125 MB | 5,962 | 112 | 17 | 1 |
| 31.7 | 26 | 133 MB | 5,975 | 112 | 17 | 1 |
| 47.3 | 33 | 150 MB | 6,016 | 112 | 17 | 1 |

Listeners, documents, callbacks and intervals stay flat. The heap climbed 2 MB per cycle until the snapshot diff showed why: 47 strings of 1.4-3.1 MB (108 MB) in the `waveform_color_detail.bands` of cached analysis payloads, held by `analysisCache` in `tauri.svelte.ts`, which is capped at 48 entries and was simply filling. The mock gives track `i` a duration of `180 + 7 i` s, so the ids up to 10,000 that the loop used made payloads of up to 70,000 s (a real one is 100-250 KB). It is a fixture effect, not a leak: the cache holds 48 entries and the heap stops growing when it is full. The loop now draws analysis ids from 1-60.

**Frontend, 15 min, 10k tracks with covers, ids 1-60** (65 cycles before, 46 after the fix below; the after run was slower because the box was busier):

| Minute | JS heap before | JS heap after | Live object URLs before | after |
|---:|---:|---:|---:|---:|
| 0.1 | 37.0 MB | 37.3 MB | 492 | 326 |
| 3.6 / 4.5 | 38.1 MB | 37.0 MB | 1,060 | 819 |
| 6.2 / 6.4 | 37.9 MB | 37.8 MB | 1,397 | 1,092 |
| 9.1 / 9.2 | 38.9 MB | 39.2 MB | 1,397 | 1,404 |
| 15.1 / 15.3 | 39.0 MB | 39.3 MB | 1,397 | 1,404 |

Heap second half: +5.5 MB before, +5.0 MB after (JIT code and lazily built row proxies; the snapshot diff of the middle against the end shows 6 MB in `blink::NetworkResourcesData` (DevTools keeping the blob image responses Playwright's session records) and 0.1 MB of JS objects). Detached DOM: 289 nodes, of which 44 are the 120x14 row-waveform canvases of the bitmap cache (capped at 800) and 29 template fragments; no detached tree reaches the page. The DOM node counter grows by about 2 per cycle: DevTools and Blink undo steps of the search box (419 `UndoStep`, the browser's own undo stack, capped at 1,000), not app nodes.

**Backend, `bench_startup soak`, 10k library** (RSS): rounds of `load_state` + 20 analysis stores + 40 cache writes + playlist save.

| Seconds | Rounds | RSS |
|---:|---:|---:|
| 2 | 1 | 14.5 MB |
| 250 | 116 | 24.7 MB |
| 500 | 235 | 34.3 MB |
| 751 | 401 | 48.3 MB |
| 1001 | 565 | 54.9 MB |
| 1251 | 722 | 54.9 MB |
| 1500 | 879 | 54.9 MB |

Each part alone is flat (75 s each: `state` 18.2 to 18.3 MB over 690 rounds, `analysis` 10.8 MB, `detail` 8.8 MB over 134 rounds, `playlists` 8.5 MB over 8,600 rounds); together RSS rises for about 1,000 s and then stops at 54.9 MB (allocator arenas filling with the 4 MB JSON strings; no growth over the last 500 s). The per-path lock map in `waveform_detail.rs` already drops unused locks.

**Leak found and fixed**

- **Thumbnail object URLs were never evicted.** `artwork.getRow` kept one blob URL (and a `$state` record key) per track ever scrolled into view, until the library was reloaded. Scrolling through a 50k library with covers accumulates tens of thousands of blobs. `rowCache` is now a first-in first-out cache of 2,000 thumbnails (about 6 MB); the oldest URL is revoked and its key deleted, a row that comes back after being evicted fetches again. Test `src/stores/artwork.test.ts` scrolls through 5,000 rows: 5,000 URLs created, 5,000 alive before the change, at most 2,000 after (the test fails before). The soak keeps its 1,397 live URLs under the cap, since its flings visit about 1,400 distinct rows; the cap only binds in longer scrolls.

Same results: only the 64 px row thumbnails are evicted; a visible row is never one of the oldest 2,000 inserted.

**Cover cache cap (R31, `r31-cover-cap`).** `artwork.get` (512 px: inspector, compact and expanded player, organize rail and picker cards, drop flash) used to keep one blob per track ever requested. It is now an LRU of 96 covers (`COVER_CACHE_LIMIT`); eviction runs when a cover is inserted, revokes the oldest cover and deletes its key. Consumers declare what they show with `artwork.keep(owner, () => ids)` (pull model: the source is read at eviction time, the returned function releases it). Registered: `player` (loaded track, `player.svelte.ts`), `inspector` (single selection, `selection.svelte.ts`), `drop-flash` (`organize-drop.svelte.ts`), `organize-rail` and `organize-picker` (component effects). Pinned covers are never revoked, so the cache can exceed 96 while more than that is shown. A requested evicted cover loads again as before. The organize rail and picker used to read `.cover` of every track in a playlist (fetching a 512 px cover for each); they now take the first covers that fit (`lib/coverTracks.ts`), so they only ever request what they show. The rail's `+N` counts tracks with artwork instead of covers loaded so far. Tests in `artwork.test.ts`: 1,000 selections keep at most 128 URLs, an in-use cover survives 600 evictions, 150 covers pinned by two consumers all survive, a released cover goes, an evicted cover reloads. The soak has a `select tracks` action (clicks about 160 rows per cycle) and fails above 200 live cover URLs (told apart by the mock padding 512 px covers by 8 KB); a 3-cycle run on 10k tracks with covers held 96.

**Guards added.** `smoke/leaks.spec.ts` (in the normal smoke run) opens and closes the palette, settings (three pages) and the expanded player 6 times and asserts listeners (+2), registered callbacks (0) and DOM nodes (+40) after a GC. No leak was found there, so it passes before and after; it pins what was verified.

**What is left**

- The 512 px cover cache is capped now (R31, below). Its limit is soft: covers that a consumer reports as in use stay, so the cache holds 96 plus whatever is shown.
- `trackAnalysisEpochs` (one small entry per track whose analysis changed), `detail-prefetch` and the other module maps are bounded by the library size or already capped (analysis cache 48, zoom detail 3, wire bands 512, bitmaps 800, row bands LRU).
- The soak did not run the real backend with the real app (WKWebView, Tauri IPC, real audio decode and the playback callback); that is a Mac session. Real-library memory (50k with covers) was not soaked either: 10k synthetic tracks, 15-45 min.
- Listener, interval and callback counts were flat in every sample of every run, so the open-close paths (palette, settings, expanded player, popovers, waveform detail) tear down what they set up.

## R15 staged reveal (2026-11-01, `r15-reveal`, not landed)

Branch `r15-reveal` = `ov-1028` + R10 option 1. Visible timing change (the panel shows a few frames after the click), so it waits for Merlin. Old path: `const STAGED_REVEAL = false` in `app/src/modules/Player.svelte`.

**What changed.**
- `Player.svelte`: on expand the expanded player mounts at once but hidden (`visibility: hidden`, absolute at its final size, `data-xp-staging`), the compact player stays and keeps animating. The panel is shown when its waveform reports ready, or after 1 s as a fallback. Revealed, its wrapper is `display: contents`, so the layout is the same as before.
- `WaveformDisplay.stage(onReady)` (`waveform-display.js`): while staged, each paint gives the zoom ring a 6 ms budget and continues in the next task (`setTimeout`, not the next frame: in WebKit frames came 200-400 ms apart while the main thread was idle). `drawBandRange` takes a `deadline`; the ring (`drawScrolledColumns`) then fills in slices of 128 columns from a fixed left column and catches up with an ordinary scroll when full. Without a deadline nothing changes.
- Pixels: each ring column still comes from its absolute index only. New test in `waveform-scroll-identity.test.ts`: the sliced fill draws nothing until full, then the same pixels as a full repaint over 20 scrolled frames.
- Harness: `first-expand` records the longest frame gap (WebKit has no long-task API); `FF_SMOKE_BROWSER=webkit` runs the smoke config in Playwright WebKit (works on the box, no sudo). The probe and harness only count "drawn" once the panel is shown.

**Where the WebKit time went (box, Playwright WebKit, instrumented, before).** Frame after the click: zoom first paint about 110-150 ms in JS (first `putImageData` into the new ring layer 25-35 ms, pixel fill 20 ms, the rest of the 3200 columns, beat grid text 14 ms first time), overview 10-20 ms. Then the next frame came 200-350 ms later while the main thread was idle: the wait is outside page script and main-thread layout (WebKit compositing of the new canvases). Chromium does all this in about 100 ms CPU (R10).

**Numbers** (10k state, 5 cold launches each, interleaved A/B, two rounds; box load 46-35 in round 1, 25-14 in round 2; medians):

| Browser | Round | Before longest frame | After longest frame | Before click to drawn | After click to drawn |
|---|---|---:|---:|---:|---:|
| Chromium | 1 | 217 ms | 133 ms | 332 ms | 456 ms |
| Chromium | 2 | 83 ms | 67 ms | 146 ms | 185 ms |
| WebKit | 1 | 439 ms | 315 ms | 707 ms | 1452 ms |
| WebKit | 2 | 344 ms | 283 ms | 529 ms | 1329 ms |

- Longest frame down 20-40 % in both engines, not to 30-50 ms. What is left in the reveal frame is the relayout (table shrinks, compact unmounts) plus WebKit compositing of the newly shown canvases (200-300 ms idle wait on this loaded box).
- Click to drawn goes up, a lot in WebKit (about +0.7 s): the staging tasks run between slow frames, and in the smoke mock the playhead jumps 3-5 s on the first transport sync after reveal (also before; mock timers drift under load), so the first visible frames repaint the whole ring anyway.
- Verdict on the box: a modest frame win, a clear latency loss in WebKit. Not worth landing on this evidence alone.

**What to look at on the Mac.** The probe now reports per launch: `longest_frame_at_ms`, `longest_busy_ms` (longest the page main thread went without running a task; covers script, style, layout, paint), `revealed_ms` and `pending_frames` (frames showing the zoom view while its detail series was still pending). Run `scripts/mac/perf-night.sh --expand-runs 5` on `ov-1028` and on `r15-reveal`:
- longest frame far above longest busy: the 500 ms is outside the web process main thread (UI process main thread blocked by a sync Tauri command, GPU process / IOSurface allocation for the big canvases). Staging does not fix that; look at the UI process (Instruments, Time Profiler on the app's main thread) during the click.
- pending frames > 0 in the slow launches: the detail series (measured from audio on first use; each perf-night launch has a fresh HOME, so the cache is empty) was still loading; the 3-of-5 split may be whether the dblclick prefetch finished within the 1.5 s settle.
- longest busy near the longest frame: page work; then compare `r15-reveal`.

## R29 WebKit startup (2026-11-01, `r29-mac-startup`)

Branch `r29-mac-startup` from `ov-1028`. Linux `dev` box, Playwright WebKit 27.2 (`npx playwright install webkit`, no sudo or system packages needed) and headless Chromium, `perf-startup.spec.ts` on the real 10k and 50k fixture payloads (`bench_startup dump`, column table), backend time simulated (118 / 480 ms). Box load 15-60 (other workers); every number below is from a before run and an after run back to back.

**Where the Mac's "1 s" comes from.** The Mac night's first row (1,640-1,940 ms warm) counts from process start; the Chromium harness counts from the page's navigation start. Tauri creates the window before the `setup` hook and then runs `setup` on the main thread, where WKWebView's custom-protocol loads are answered, so the page cannot load before `setup_done` (290-400 ms on the Mac). Each Mac run also starts with a fresh HOME, so every WKWebView launch is a first launch (no boot snapshot, no cache); compare it with the harness's "first launch", not "warm". `scripts/mac/perf-night-summary.mjs` now prints the page origin (process start to navigation start, from the probe's `page_start` time origin), when `load_state` reached the backend and the first row counted from the page origin, so the next Mac night splits the time.

**WebKit vs Chromium, same build and payload** (10k, warm, one profiled run each at load 25-40; WebKit Timeline + sampling profiler via the inspector session, Chromium trace; ms of main-thread work up to interactive)

| Part | WebKit | Chromium | Note |
|---|---:|---:|---|
| bundle evaluate + mount (skeleton rows, sidebar, player) | 104-178 | about 55 | JS, includes 24 skeleton `TrackRow`s |
| first forced layout after mount (`bind:clientHeight`) | 62 | under 15 | style and layout of the whole shell |
| `load_state` JSON fetch + parse (1.7 MB) | 20-60 | 12-25 | not a WebKit problem since R22's table |
| table decode | 10-14 | 8 | |
| map + row proxies (10k) | 27-140 | 25 | |
| answer to first row: Svelte render (rows, sidebar rebuild) | 184-229 | about 90 | `TrackRow`, `SidebarRow`, `Icon` (`{@html}` per icon) |
| style recalcs, total | 260-310 | 49 | about 5x per element |
| layout, total | 140-175 | 54 | about 2-3x |
| off-screen rows after the first paint (24 rows) | 150-290 | about 60 | JS + style + layout + paint |
| idle gap before frames settle | 210-380 | none | main thread idle, no rendering update; headless WPE compositor, see below |

Per row, measured in the page (clone N rows into the list, force style, then layout; median of 7): WebKit 1.0-1.6 ms style + 1.0-1.5 ms layout, Chromium 0.2-0.3 + 0.5-0.75 ms.

**Fixed**

- **The reset rule `*, *::before, *::after { box-sizing: border-box }`** (`design-system/base.css`) made WebKit resolve a `::before` and an `::after` style for every element. Removing only that rule from the live page cut WebKit's style time per inserted row by about 40% (48 rows: 30-33 to 17-19 ms, three rounds; "no `::`" and "no `*`" rule sets gave the same, every other rule group did not). On branch `r29-mac-startup` (c89aeff, not landed: it edits the prototyping design system, which the AGENTS fence reserves) the reset is `* { box-sizing: border-box }`. Same picture: of the 28 pseudo-element rules in the app, 25 set no padding or border (box-sizing changes nothing), the other 3 are absolutely positioned with all four insets 0 and no size (same box either way); full-page screenshots of the booted 10k list are byte-identical before and after in WebKit and Chromium. A future pseudo-element that sets a size together with padding or a border needs `box-sizing: border-box` in its own rule (noted in `base.css`).

**Before/after** (WebKit 10k warm, 8 back-to-back pairs, load 15-55; style = WebKit Timeline style recalcs up to interactive)

| | before median | after median | pairwise |
|---|---:|---:|---|
| style recalc total | 526 ms | 330 ms | lower in 8 of 8 (-12% at load 16, -40-60% at load 30-50) |
| first real row | 1,843 ms | 1,365 ms | -80 to +26 ms at load 16, -280 to -875 ms at load 30-55 |
| interactive (10 frames under 50 ms) | 2,572 ms | 2,036 ms | |

50k (3 pairs, load 18-38): style 306 to 179 ms in the quiet pair; end-to-end within noise. Chromium 10k (4 pairs): warm first row 445-738 before, 499-763 after, unchanged within the noise.

**Tried, not kept**

- Starting `app_version` and `get_library_path` together with `load_state` before mount (so `initLibrary` does not wait a round trip): in the real app under WebKitGTK the path still resolved 530-700 ms after `initLibrary` asked, because the answer waits for the busy page main thread, not for the backend. No gain, reverted.
- Removing the skeleton rows, the inline cell styles, or the deferred off-screen rows: no gain measurable above the noise.

**Real app on Linux (WebKitGTK under Xvfb).** Release app with the embedded frontend (`--features tauri/custom-protocol`), probe on: `load_state` reaches the backend 160-380 ms after the page origin and is built 80-370 ms later, but the first row comes at 3.0-3.8 s (page). Rendering updates are the cause: rAF ran 3 times in the first 8 s (for example at 374, 2,404 and 8,316 ms) while the main thread was idle most of that time; `WEBKIT_DISABLE_COMPOSITING_MODE=1` was worse. This is software compositing in Xvfb and is no measure of the Mac; the same kind of idle gap (210-380 ms, no main-thread work) shows up in headless Playwright WebKit right after the first rows, and the Chromium harness has none.

**What is left (WebKit)**

- Per-row style and layout are still 2-5x Chromium's. Next suspects in the bisection: rules with `var()` and `calc()`, and the 25 elements per row (5 star spans, the wave canvas and cursor).
- WebKit has no `requestIdleCallback`: `whenIdle` falls back to `setTimeout(0)`, so the label refresh and the search warmup run right after the first rows instead of in idle time.
- `Icon` parses its SVG with `{@html}` per instance (shows in the sidebar rebuild after `load_state`).
- Skeleton rows are full `TrackRow`s, built at mount and thrown away when the data arrives.

**Mac.** Not measured here; the Mac re-measure happens at the next Mac perf night (`scripts/mac/perf-night.sh`), whose summary now has the page-origin columns.

**Harness.** `FF_SMOKE_BROWSER=webkit` runs the smoke specs in Playwright WebKit (`devices['Desktop Safari']`, no Chromium launch flags). `perf-startup.spec.ts` reports `frameInteractive` (start of the first 10 frames under 50 ms after the first real row, the Mac probe's definition; WebKit has no long-task API, so the old `interactive` equals the first row there) and `longestFrame`; `FF_PERF_STARTUP_PROFILE=<prefix>` under WebKit writes `<prefix>-<tracks>.webkit.json` (Timeline records plus JSC samples, through Playwright's inspector session). Reproduce: `FF_SMOKE_BROWSER=webkit FF_PERF_STARTUP=10000=table10k.json==118 FF_SMOKE_PORT=5328 npx playwright test perf-startup`. Raw: `~/.cache/r29/` (deleted after this item; numbers above).

## R34 WebKit idle (2026-11-02, `r34-webkit-idle`)

Branch `r34-webkit-idle` from `ov-1028`. Linux `dev` box, R29 harness (`perf-startup.spec.ts`, Playwright WebKit and headless Chromium), 10k fixture as a column table (`bench_startup dump`), backend simulated as 118 ms. Box load 17-60 (other workers, swap full); every comparison is a before run and an after run back to back, the order flipped every other pair.

**What the "idle gap" was.** WebKit's Timeline shows that about 100 ms after the first real rows the whole page restyles and relays out (60-90 ms style plus 35-105 ms layout, scheduled by no script), and the next frame after it comes 150-250 ms later. The cause is the font: `geist.css` splits Geist and Geist Mono into unicode-range subsets, and the fixture's titles carry U+0301 (a combining accent, the decomposed form macOS file names use), which sits in the Cyrillic subset. The browser only fetches a subset when laid-out text needs it, so the first rows pull it in, show the fallback for those glyphs, and when it arrives (`font-display: swap`) WebKit restyles and relays out every element and repaints the viewport. A FontFace status probe showed exactly that: Latin subsets loaded before the rows, the Cyrillic subset 100 ms after them. Any library with non-Latin text (Latin Extended, Cyrillic, Vietnamese) hits it; real Mac libraries with decomposed accents hit it on every launch.

**Changed**

- `main.ts` asks every `@font-face` of the app's stylesheet to load at boot (`face.load()` for the 27 faces, 11 woff2 files, 150 KB, served locally), next to the early `load_state`. They are in before the rows arrive, so the first rows lay out with the final font and nothing restyles after the first paint. Final pixels are the same: full-page screenshots of the booted 10k list are byte-identical before and after in WebKit and in Chromium. Main-thread work before the first row is unchanged (WebKit Timeline, quiet pairs: 781 / 906 / 746 / 767 ms before, 771 / 724 / 799 ms after).

**WebKit 10k** (ms; settle = frame interactive minus first real row; longest = longest frame gap after the first row)

| Pair (load) | Case | Before real / fi / settle / longest | After real / fi / settle / longest |
|---|---|---|---|
| 1 (24-18) | first launch | 1,515 / 1,965 / 450 / 338 | 1,323 / 1,647 / 324 / 37 |
| 1 | warm | 1,078 / 1,625 / 547 / 235 | 989 / 1,308 / 319 / 39 |
| 1b (20-18) | first launch | 1,626 / 2,330 / 704 / 454 | 2,335 / 2,714 / 379 / 30 |
| 1b | warm | 1,242 / 1,915 / 673 / 226 | 950 / 1,302 / 352 / 27 |
| 3b (38-24) | first launch | 1,437 / 2,549 / 1,112 / 405 | 1,364 / 1,787 / 423 / 60 |
| 3b | warm | 996 / 1,661 / 665 / 320 | 1,067 / 1,359 / 292 / 36 |
| 7 pairs, median | warm | 1,242 / 1,926 / 707 / 327 | 1,329 / 1,619 / 382 / 32 |

Over all 10 WebKit pairs the longest frame after the first row went from 190-644 ms to 24-122 ms (every run), and the settle time roughly halved. Profiled pairs at load about 20: main-thread work in the 1.5 s after the first row 356-459 to 272-285 ms, longest busy block 127-190 to 64-72 ms. The first real row itself is unchanged within the noise (pairs go both ways by up to the load swing).

**Chromium 10k** (3 pairs, load 37-55, no regression): warm first row median 1,147 to 883 ms, frame interactive 1,434 to 1,075, longest frame 183-267 to 83-167 ms. Chromium had the same late font swap, only cheaper.

**Tried, not kept**

- Frame-aware idle scheduler for WebKit (no `requestIdleCallback`): wait for the next rendered frame (rAF, then a MessageChannel task) and give the work one 8 ms slice, with a timeout for hidden windows; used for the search warmup, `applyChangedTrackLabels` and the column-layout save. It works as designed (warmup runs one 7 ms slice per frame instead of a 1-2 ms `setTimeout(0)` task every 8 ms), but 4 interleaved pairs against the font fix alone showed no difference in settle time, longest frame or first row. The old fallback's tasks were already short; they never held a frame. Not kept (commit `d6efbc0`, unreferenced).
- Growing the off-screen rows in idle steps of 3 instead of all 12 at once: with the font fix the one-shot block is about 80 ms in WebKit; splitting it added paints and was worse in its pair (settle 279 to 705 ms). Not kept.

**Measured, not changed (fenced)**

- `Icon` (`prototyping/viewport/design-system/components/Icon.svelte`): the booted page has 107 icons. Building 107 SVGs the `{@html}` way (parse a string per instance) vs cloning a parsed glyph once per name: WebKit 5-6 ms vs 2-3 ms, Chromium 3.4 vs 1.1 ms (median of 30, standalone page, load 30). A clone cache would save about 3 ms of a 175-230 ms first render in WebKit: not worth a design-system change.
- Skeleton rows: not retried; R29 found no gain from removing them, and with the font fix the frames before the first row are not where the time goes.

**What is left (WebKit)**

- Per-row style and layout are still 2-5x Chromium's (R29). The answer-to-first-row render (175-230 ms at load 20) and the off-screen rows (one 80 ms frame) are the largest main-thread blocks left.
- A 150-200 ms frame gap after the first big paint remains in headless WebKit with the main thread idle (compositor; R29 saw the same under Xvfb). Not app-side.
- 50k not re-measured (the fixture dump binary was pruned).

**Validation.** `npm test` (886 passed), `npm run check` (0 errors), Chromium smoke on port 5332 (113 passed). WebKit smoke: 96 passed, 17 failed (`leaks`, one `popups`, one `sticks`, 14 `topbar`); the same 17 fail on `ov-1028` without this change, so the WebKit smoke was not green before.

## R36 covers (2026-11-02, `r36-covers`)

Branch `r36-covers` from `ov-1028`. Linux `dev` box, release `bench_startup import` on the R13 fixtures and 1,000-file sample folders (10k and the linked 50k), 16 cores, box load 18-60 (other workers; the A/B runs below are interleaved in random order and quote the load).

**Outcome: no speed-up shipped; the step is at its floor.** Nothing in the covers path decodes, resizes or encodes: `store_pending_covers` re-reads each file's tags with `read_track_metadata` and stores the embedded picture bytes as they are (`pioneer-usb-writer/src/scanner.rs`, read only). The levers from the brief that assume an image pipeline (parallel decode and resize, skipping a re-encode, a faster filter) have nothing to act on. What the step costs is writing the covers into SQLite.

**Profile** (10k, one run of 1,000 files, 806 covers, about 6 MB per 32-file batch, load 18; temporary timers, not kept; medians of 2-3 runs)

| Part | Time | Note |
|---|---:|---|
| covers step, wall | 880-980 ms | `covers_ms` |
| reading the files (8 threads, summed per batch) | 250-300 ms | hidden: it runs one batch ahead of the store |
| storing (library locked), total | 800-900 ms | this is the wall time |
| - building the rows (insert loop, inside the transaction) | 225-257 ms | copies each cover into the b-tree pages |
| - commit (about 190 MB into the WAL) | 584-672 ms | 46k pages, two `pwrite` calls per page |

Reading a cover alone (lofty with `read_properties(false)`, same picture choice, byte-equal output on all 806) against the full tag read: 0.28-0.56 against 0.35-0.54 ms per file serially (three rounds), the same within noise, so it was dropped (it would also have added `lofty` to a second crate).

Floor: plain SQLite (Python `sqlite3`, WAL, `synchronous=NORMAL`, 800 blobs of 236 KB, 25 per transaction, same disk) takes 0.20 s for the inserts and 0.72 s for the commits at load 18-20. The library already runs this at 0.23 and 0.45-0.60 s. The bytes cannot shrink (stored bytes must stay identical, no schema change, so no larger page size either), and a raw 190 MB write to the same disk is 0.6 s in large writes and 1.2 s in page-sized ones.

**Tried and measured**

| Change | 10k, covers ms (median, min of 12 interleaved rounds, load 22) | 50k (3 interleaved rounds, load 22) |
|---|---:|---:|
| before (checkpoint after every batch) | 963 / 916 | 929 / 920 |
| checkpoint every 32 MB of covers and once at the end | 990 / 790 | 839 / 809 |
| checkpoint every 64 MB and once at the end | 983 / 802 | not run |
| batch of 128 instead of 32 (one early run, load 18) | 981-1,063 against 913-1,063 | |

The commit-time checkpoint thread overlaps the next batch's write. Dropping it altogether took the step from 913-1,063 to 672-754 ms (two interleaved pairs at load 18), but the log then grows to the size of all covers (190 MB per 1,000 files, 4.7 GB for 25k) and the next user write would run that checkpoint inside the library lock. Checkpointing less often (32 and 64 MB) leaves the same checkpoint I/O overlapping the writes, and gave 0 to 10% (no difference at 10k over 12 rounds, 10% at 50k over 3), within the noise of a loaded box, for a larger log during the fill. Not kept.

**Kept.** Only a test: `stored_covers_of_a_folder_are_the_embedded_bytes_and_hash_to_a_pinned_value` (`pioneer-test-ui/src/import_plan.rs`) imports a folder of 70 files (covers of 1 byte to 40 KB, every seventh file without one, three store batches), checks each stored cover equals the embedded bytes and the whole set hashes to a pinned FNV-1a value, so any future speed-up of this path has to keep the stored bytes identical.

**What would move it.** Less written, i.e. storing covers as files or at a smaller size (a schema change, already listed under R13), or writing the covers during the track insert transaction instead of afterwards (same bytes, same cost, no longer deferred). Neither is a change inside this brief.

Raw: `~/.cache/r36/` (scripts `ab.py`, `exp.sh`, `mb.py`).

## R37 WebKit smoke (2026-11-02, `r37-webkit-smoke`)

Branch `r37-webkit-smoke` from `ov-1028`. The 17 WebKit smoke failures (`FF_SMOKE_BROWSER=webkit`) have two causes, both in the test harness; no app bug.

- **16 specs (14 `topbar`, 1 `popups`, 1 `sticks`): `ResizeObserver loop completed with undelivered notifications.`** The fixture in `smoke/app.ts` fails a test on any `pageerror`. Playwright WebKit reports the window `error` event that this condition raises as a page error; Playwright Chromium takes page errors from CDP exceptions and never sees it. The event fires in Chromium too (a probe with a `window` `error` listener logged the same sequence at 900 px in both browsers: the `GlobalHeader` observer sets `ui.searchMode`, the waveform observer runs, the header observer is re-delivered next frame). It only reaches `window.onerror`, the app has no handler, and the header settles on the next frame, so Mac users see nothing. `app.ts` now ignores exactly that message. The specs hit are the ones where the search field collapses (900, 700, 670 px), because the header observer changes the layout during boot.
- **`leaks`: `newCDPSession` and `Memory.getDOMCounters`.** CDP only exists in Chromium. Under WebKit the test still checks that no `_<n>` Tauri callbacks pile up over the cycles; the listener and node counters stay Chromium-only (the WebKit test name says so).
- **Also found, not one of the 17: `player` "without Q a dragged hot cue lands where it is dropped" failed about 1 run in 6 under WebKit** (0 in 8 under Chromium). Right after the Pause label shows, the zoom view is still settling for about 100-300 ms (the hot cue A flag sat 10-20 px left of its final spot, then moved), and the test read the flag position once and pressed there, missing the grab zone. `cueAFlag` now waits until two reads 100 ms apart agree. After the change 20 of 20 WebKit runs of the two cue-drag specs passed. The settle itself is app behavior in both browsers (the view corrects after the pause), not a Mac-only bug.

**Validation.** WebKit smoke and Chromium smoke green (numbers in the run report).

## R45 row cost (2026-11-02, `r45-rowcost`)

Branch `r45-rowcost` from `ov-1028`. Linux `dev` box, Playwright WebKit and headless Chromium at 1600x1000, 10k fixture as a column table (`bench_startup dump` of a copy of `~/.cache/ff-fixture/10k/library`; the dev bin did not build on `ov-1028`, fixed here), backend simulated as 118 ms. Box load 24-54 (other workers).

**Where the row markup and CSS live.** The row is `TrackRow.svelte` and `track-row.css` in `prototyping/viewport/design-system/components/`, cell geometry comes from `columnCellStyle` (`design-system/utils/column-layout.js`) and the reset from `design-system/base.css`: all fenced. App code owns only the `.ff-table__row` wrapper (one div with `top`, `contain: layout paint style`) and the scoped rules in `TrackTable.svelte`.

**Method.** R29's micro-measure, done in the booted 10k page: clone 48 of the visible rows into the list body, read a computed style (style time), then read an offset of the last cell (layout time; `document.body.offsetHeight` is not enough in WebKit, it skips the layout of absolutely positioned rows), remove, 9-11 rounds, median per row. Candidates were tried in the same page and interleaved round by round (inject a style tag, delete matching rules from the live sheets, or rewrite the cloned rows), so boot noise drops out. A row is 25 elements: the wrapper, `.ff-trow`, 11 cells, 8 text spans, fav box, canvas, wave cursor, cover.

**Per row, same page, interleaved** (ms style / layout, WebKit, two pages each)

| Variant | WebKit | Chromium |
|---|---|---|
| as is | 0.71-0.94 / 0.54-0.77 | 0.15-0.16 / 0.20-0.30 |
| reset `*, ::before, ::after` -> `*` (fenced, `base.css`) | 0.38-0.54 / 0.54-0.69 | 0.13-0.16 / 0.19-0.23 |
| plus cell geometry as classes instead of inline `style` (fenced) | 0.33-0.38 / 0.52-0.54 | |
| plus text in the cell instead of a span, no wave cursor (fenced, not pixel-identical as tried) | 0.27-0.33 / 0.42-0.52 | |
| no TrackTable rules (`data-file-status`, skeleton `::before`, `:hover`) | no change | no change |
| `contain: strict` on the row, `contain: layout` on cells or `.ff-trow`, no `contain` | no change or worse | |
| no canvas / no wave cursor element | -0.05 to -0.1 style, noise | |
| empty text spans (for scale only) | layout 0.33-0.40 | |

Rules that match a row element: 20, all plain class selectors except the universal reset and one `.ff-table__row .ff-trow__cell--status`. Only the universal reset costs measurable style time: it makes WebKit resolve a `::before` and `::after` style for each of the 25 elements. Layout is mostly text: emptying the spans cuts it by a third; the flex nesting and `contain` settings do not move it. Changing text rendering properties (`text-rendering`, `text-overflow`) made layout 2-3x slower in the measure (cold text caches), so they say nothing.

**App code: nothing to cut.** None of the app-owned per-row work (wrapper div, inline `top`, TrackTable's scoped rules, `contain` on the wrapper) is measurable; every gain sits in the fenced design system. No app change made.

**The fenced patch, measured as a build** (`base.css` from R29's `c89aeff`, `* { box-sizing: border-box }`, swapped in at build time by a Vite plugin outside the repo; no file under `prototyping/` edited). Booted page, interleaved, first launch each:

| | WebKit as is | WebKit patched | Chromium as is | Chromium patched |
|---|---:|---:|---:|---:|
| per row style (median, range) | 0.69-0.71 (0.58-0.88), 12 runs | 0.47 (0.40-0.56), 6 runs | 0.22 (0.17-0.28) | 0.19 (0.17-0.20) |
| per row layout (median) | 0.52-0.54 | 0.55 | 0.24 | 0.22 |
| first real row from navigation (median) | 1,287-1,290 ms | 1,357 ms | 839 ms | 794 ms |

Style per row -32% in WebKit (lower in 6 of 6 pairs), layout unchanged; first row within the noise at this load (R29 saw -280 to -875 ms only at load 30-55 in the warm case). Full-page screenshots of the booted 10k list are byte-identical between as-is and patched in WebKit and in Chromium (and as-is vs as-is, as a control).

**Proposed fenced patch** (needs the fence lifted for `prototyping/viewport/design-system`):

1. `base.css`: the reset becomes `* { box-sizing: border-box }` (R29 `c89aeff`, unlanded). Biggest lever: about a third of WebKit's per-row style time, pixel-identical.
2. `TrackRow.svelte` + `column-layout.js`: drop the per-cell inline `style`; the table sets each column's width once as a custom property (`--ff-col-<key>`) on the table root and `track-row.css` has `.ff-trow__cell--<key> { flex: 0 0 var(--ff-col-<key>); width: ...; min-width: ...; max-width: ... }`, flex columns as a class. Same declarations, so same pixels; about 0.05 ms style per row more in WebKit (10%).
3. Text cells without the inner span (and the wave cursor only while hovered): fewer elements, about 0.1 ms layout per row in WebKit, but the vertical centering must be matched exactly before it can be called pixel-identical; not verified.

**Harness.** Kept out of the repo (one-off): spec, config and the Vite plugin were in `~/.cache/r45/` and deleted after this item; the method above is enough to redo it.

## R42 real-app soak (2026-11-02, `r42-realsoak`)

Branch `r42-realsoak` from `ov-1028`. The release app (Tauri + WebKitGTK + real IPC + real backend, `cargo build --release -p pioneer-test-ui --features tauri/custom-protocol`, frontend embedded, no `.deb`) under `xvfb-run` on the Linux `dev` box (16 cores, 62 GB, box load 15-45 from other workers), temp `HOME` and `XDG_*`, a copy of the 10k fixture library (`~/.cache/ff-fixture/10k/library`, 10,000 tracks, 6,937 analysed, covers; the 7 GB of audio is read in place, the app never writes it), the real model weights (`FOURFOUR_MODELS_DIR`). No audio device on the box: `audio output unavailable at startup`, loading and seeking still work; playback itself and the cpal callback were not exercised.

**Method.** `FF_PERF_PROBE='{"soakMinutes":50,...}'` (with `FF_PERF_LOG`; off by default, so no change without those env vars) makes `perf_probe.js` loop, after the first interactive frame: scroll flings (24 frames of 9,000 px), typing and clearing a search, palette open/type/close (Ctrl+K), switching four playlists and back to All Tracks, double-click load of a track + expand + wait for the zoom detail + four overview seeks + close (the waveform detail open/close), settings open with every page and close, and `analyze_track_ids` on two real fixture tracks per cycle (real beat model, real stage passes). `soakActions` (a JSON list) runs a subset, `soakPauseMs` paces an empty list (idle control). `scripts/soak-real.sh [minutes]` starts the app, samples every 60 s from `/proc`: RSS and PSS of the app process (Rust backend plus the WebKitGTK UI process), thread and fd counts, RSS and PSS of the `WebKitWebProcess` child and the network process, the number of rows in `analyses`, and dumps threads and `smaps_rollup` when the app passes 3 GB. The probe logs DOM nodes and failures per sample (`performance.memory` does not exist in WebKitGTK, so there is no JS heap number; RSS of the web process is what is left). Env: `FF_SOAK_ACTIONS` (comma list or `none`), `FF_SOAK_AUTO_ANALYZE=off` (writes `auto_analyze_new_tracks: false` into the temp config), `FF_SOAK_SAMPLE_SECS`, `FF_SOAK_LIBRARY`, `FF_SOAK_MODELS`.

**50 min, all actions** (134 cycles, 19-47 s each; the first two cycles in the launch analysis, 2 of 134 loads timed out at 30 s there) next to a 50 min idle control (no actions) of the same build:

| Minute | App RSS (actions) | Web process (actions) | Threads / fds | App RSS (idle) | Web process (idle) |
|---:|---:|---:|---:|---:|---:|
| 1 | 1,409 MB | 656 MB | 396 / 46 | 1,423 MB | 570 MB |
| 3 | 2,161 MB | 780 MB | 127 / 43 | 2,009 MB | 615 MB |
| 5 | 2,068 MB | 846 MB | 120 / 43 | 2,092 MB | 485 MB |
| 10 | 2,253 MB | 884 MB | 121 / 43 | 2,154 MB | 613 MB |
| 15 | 2,161 MB | 822 MB | 122 / 43 | 2,002 MB | 379 MB |
| 20 | 2,204 MB | 934 MB | 123 / 46 | 2,018 MB | 351 MB |
| 25 | 2,229 MB | 945 MB | 121 / 43 | 1,974 MB | 349 MB |
| 30 | 2,170 MB | 964 MB | 116 / 43 | 2,020 MB | 382 MB |
| 35 | 2,148 MB | 1,001 MB | 124 / 44 | 2,087 MB | 381 MB |
| 40 | 2,124 MB | 932 MB | 119 / 43 | 2,044 MB | 386 MB |
| 45 | 2,101 MB | 1,154 MB | 122 / 44 | 2,004 MB | 404 MB |
| 50 | 2,128 MB | 1,065 MB | 120 / 43 | 2,001 MB | 454 MB |

- **App process: flat.** Slope after minute 10: -2.0 MB/min (actions), -0.5 and -1.3 MB/min (two idle runs). Threads and fds do not grow (43-46 fds, about 120 threads), DOM nodes 2,076 at the first sample, 2,074 at the last (max 2,685 while a panel is open). No failed action except the two launch-time load timeouts.
- **Web process: 650-1,150 MB under the loop, 350-750 MB idle.** Linear fit after minute 10 is +5.1 MB/min for the full loop (880 MB at minutes 10-20, 951, 958, 1,060 MB at 40-52) but the 32 min subset runs (fling only, load+seek only, search+palette+playlists+settings only) are flat after about minute 16 (fling 642, 766, 834, 733 MB; load 705, 731, 790, 784; ui 722, 715, 744, 688). Not shown to be unbounded in 50 min; 180 MB over 40 min is inside what JSC garbage, the 2,000-thumbnail row cache and the waveform bitmap caches explain. A 2 h run would settle it.

**Findings**

1. **Launch analysis leaves about 1.8 GB resident, then stays flat (glibc arenas, not a leak).** With defaults the app queues the 3,063 never-analysed tracks at launch; RSS climbs from 0.3 GB to 2.1-2.3 GB in 3-5 minutes and stays there for the rest of the run, in the idle control as well. With `FF_SOAK_AUTO_ANALYZE=off` the same app is at 277 MB idle and 584 MB under the full loop for 16 min (flat, 0 failures, 6,937 analyses). With `MALLOC_ARENA_MAX=2` the launch analysis ends at 646-800 MB. So what stays is freed memory the allocator keeps in the per-thread arenas of the 16 analysis and rten threads, not data Rust still references (the analysis `Analyzer`, the model pool and the 48-entry caches are bounded). macOS's allocator does not behave like this. A Linux build could cap arenas or trim after a run; not done here (needs `mallopt`/`malloc_trim` through libc, a new dependency, and the Linux target is not shipping yet).
2. **Analysis peaks at 5-7 GB RSS for 2-4 minutes, in four of twelve runs** (6.1, 6.7, 6.6 and 5.5 GB at minutes 39, 18, 4 and 4, back to 2.3 GB afterwards; `analysis`, `rten-0` and `beat-chunks` threads at the top of the dump, all 10,000 tracks already counted as analysed). Probably the 24 long mixes (7 minute aiff/flac, last in the queue) decoded and chunked in parallel; not confirmed per track. The analysis plan allows a quarter of the machine (15 GB here, 4 GB on a 16 GB Mac), so it is the transient the plan allows, not growth; the 50 min full run never hit it, the UI-busy pauses probably kept the queue short of those files.
3. **No Rust cache or map grows**: with the analysis out of the way (`FF_SOAK_AUTO_ANALYZE=off`) the app process is 584 MB at minute 3 and 584 MB at minute 16 under the full loop; the artwork thumbnail cache (400), the analysis cache and the waveform-detail lock map hold.

**No leak found, no fix made.** The only code change is the env-gated soak mode in `perf_probe.js` and `scripts/soak-real.sh`.

**What is left**

- A 2 h run of the full loop to settle the web-process slope (+5 MB/min between minutes 10 and 50).
- The same soak on the Mac with WKWebView: `performance.memory` is absent there as well; `vmmap`/`footprint` of the app and `com.apple.WebKit.WebContent` would replace `/proc`. Audio output and the playback callback (`playback_play`, tempo) were not exercised (no sound card).
- Arena retention after bulk analysis on Linux (finding 1) if a Linux build ships.
- Real 50k library with covers was not soaked in the real app (10k only).

## R49 Linux arenas (2026-11-02, `r49-linux-arena`)

Branch `r49-linux-arena` from `ov-1028`. Linux `dev` box (glibc 2.41, 16 cores), box load 15-85 from other workers during every run below, so numbers are compared in pairs that ran at the same time. Fixes finding 1 of R42: after the launch analysis the app stayed at about 2.1 GB although Rust held nothing.

**Change** (`pioneer-test-ui/src/heap.rs`, `cfg(all(target_os = "linux", target_env = "gnu"))`, `libc` already in the lock file): `main` calls `mallopt(M_ARENA_MAX, 4)` before any thread exists (skipped when `MALLOC_ARENA_MAX` is set, so the env var still wins) and `run_with_resolver` calls `malloc_trim(0)` when an analysis run ends. macOS and other targets compile to no-ops. No thread-pool change: the analysis threads are scoped per track and glibc reuses the arenas of dead threads, so keeping threads alive would not lower the arena count.

**Options measured** (`bench_analysis`, 500 short fixture tracks of the 10k library, default 14 threads, `FF_HEAP_MODE` env switch in the measuring build only; `--idle-secs` and an RSS line were added to the bench; it also needed `analysis_failures` and `command_error` in its module list to compile on `ov-1028`):

| Option | Peak RSS | RSS after the run | Note |
|---|---:|---:|---|
| none (before) | 1,526 MB | 1,343 MB | |
| `malloc_trim(0)` at run end | 1,583 MB | 1,057 MB | frees about 300 MB, peak unchanged: the retained memory is mostly fragmented, not whole free pages |
| `M_ARENA_MAX=2` | 646 MB | 370 MB | |
| `M_ARENA_MAX=4` | 760 MB | 620 MB | |

In the app (real soak, 12 min of the launch analysis, idle probe) the same: before 2.0-2.4 GB for the whole run, trim alone 2.2-2.3 GB (it only acts when the run ends), `M_ARENA_MAX=2` 630-830 MB. `M_MMAP_THRESHOLD` tuning (more `mmap`/`munmap` per decode buffer) was not measured: the arena cap already removes the retention and costs nothing.

**Throughput.** Variants ran at the same time on the same 150 tracks, so each saw the same load (the sequential runs were useless: wall time swung 408 s to 1,370 s with load 50-85, the analysis threads run at nice 10):

| Round | Variant | Tracks/min | user + sys CPU | RSS peak / after run |
|---|---|---:|---:|---:|
| 2 (load 50) | before | 62.6 | 227.8 + 42.7 s | 1,426 / 1,294 MB |
| 2 | arena 2 | 60.9 | 226.6 + 43.7 s | 583 / 355 MB |
| 2 | arena 4 | 59.7 | 228.0 + 44.5 s | 678 / 485 MB |
| 1 (load 55, all three stalled to 699 s) | before / arena 2 / arena 4 | 13.0 / 13.0 / 13.0 | 365 / 367 / 366 s | 1,357 / 595 / 678 MB peak |

CPU time is equal (no sign of arena lock contention), tracks/min within 5 % and ordered arena 2 > arena 4, so inside noise. 4 chosen over 2 as the safer cap for a quiet 16-core box, where 14 analysis threads share the arenas without any other load to hide contention; the box could not show that case. Raise or lower `ARENA_MAX` in `heap.rs` if a quiet machine shows otherwise.

**2 h full-loop soak on the final build** (`scripts/soak-real.sh 120`, all actions, 349 cycles, 0 failed actions, 120 samples; launch analysis of the 3,063 new tracks ran from minute 0 to 79 at load 20-75, 10,000 analysed at minute 79):

| Minutes | App RSS (avg) | Web process (avg / max) |
|---|---:|---:|
| 0-10 | 672 MB | 847 / 1,043 MB |
| 10-20 | 662 | 1,081 / 1,180 |
| 20-30 | 705 | 1,197 / 1,247 |
| 30-40 | 697 | 1,253 / 1,272 |
| 40-50 | 693 | 1,260 / 1,305 |
| 50-60 | 676 | 1,233 / 1,283 |
| 60-70 | 600 | 1,141 / 1,182 |
| 70-80 | 1,203 | 1,046 / 1,176 |
| 80-90 | 306 | 943 / 964 |
| 90-100 | 295 | 923 / 949 |
| 100-110 | 318 | 931 / 999 |
| 110-120 | 304 | 933 / 954 |

- **App process:** 570-750 MB during the analysis (R42: 2.0-2.3 GB), 270-410 MB once it ended and the trim ran (R42: still 2.1 GB). Threads 106-124 and 43 fds throughout, DOM nodes 2,076 at the first sample, 2,068 at the last (max 2,327). One spike: 2.5 GB at minute 75 and 5.8 GB at minute 76 (`rten-0` and `analysis` threads on top), back to 275 MB three minutes later: the 24 long mixes at the end of the queue, finding 2 of R42, unchanged by this fix.
- **Web process: not a leak.** It rose from 850 MB to 1,260 MB over the first 40 minutes, while the launch analysis streamed results into the UI, then fell back to 930 MB as the analysis finished and stayed flat there for the last 40 minutes (minutes 80-120: 943, 923, 931, 933 MB average). Linear fit: +3.6 MB/min over minutes 10-60, -3.3 MB/min over 10-120, about 0 over 80-120. R42's +5 MB/min between minutes 10 and 50 was the same hump, seen while the analysis was still running; the loop itself adds nothing over 2 h.

**Not covered:** the Mac allocator (unchanged by construction), audio playback (no sound card), a quiet-box throughput measurement, `M_MMAP_THRESHOLD`.

## R57 long tracks (2026-11-02, `r57-longmix-mem`)

Branch `r57-longmix-mem` from `ov-1028`. Linux `dev` box (16 cores, 62 GB, load 15-35 from other workers). Finding 2 of R42 and the 5.8 GB spike of R49. Fixtures (not committed): the 24 `_Long Mixes` of the 10k fixture decoded and concatenated with ffmpeg into a 60 min and a 120 min FLAC (44.1 kHz stereo) and a 120 min 320 kbit/s MP3 at 48 kHz. Measured with `cargo run --release --example stage_rss -- <file> 14 full` (`fourfour-analysis-rs/examples/stage_rss.rs`): one `analyze_controlled` run with the app's 14 analysis threads on this box, RSS sampled every 5 ms, peak per window of the `ms` timings (key and waveform run beside the beat resample); without `full` it runs the stages one after the other.

**Peak RSS per stage (MB)**

| Stage | 60 min before | 60 min after | 120 min before | 120 min after |
|---|---:|---:|---:|---:|
| decode | 705 | 705 | 1,311 | 1,311 |
| beat resample + key + waveform | 1,027 | 1,018 | 1,934 | 1,935 |
| mel spectrogram | 1,427 | 829 | 2,452 | 1,222 |
| beat chunks + fit | 6,403 | 5,828 | 7,250 | 6,021 |
| whole track (`time -v` max RSS) | 6,556 | 5,969 | 7,424 | 6,165 |

**Where the memory goes**

- **Beat-model chunks: about 370-450 MB each, 14 at once.** With one chunk thread the 60 min track peaks at 1,482 MB, with 16 at 7,259 MB. Every chunk of a track longer than 30 s is full size (1,500 frames), and the activations of the model (attention over 1,500 frames) are what costs; prepacking the weights changes nothing (372 MB above the mel stage either way, 10 min track, one thread). `chunk_slots` plans 400 MB per chunk inside a quarter of the machine, so on this 62 GB box it allows 14 chunks, about 5.6 GB. This is the spike of R42 and R49, and it does not need a long track: the 24 "long mixes" are 5-7 min and every other track of the fixture is 8 s (chunks of 400 frames, a fraction of the memory), so the fixture library hit full chunks only at the end of its queue. A real library of 3-7 min tracks keeps all chunk threads on full chunks for the whole run.
- **Audio buffers that grow with length** (120 min, 44.1 kHz): the decoded mono track 1.27 GB, the 22.05 kHz copy for the beat model 635 MB, the 11 kHz decimation for the key 318 MB (plus a 44.1 kHz copy of 1.27 GB for 48 kHz sources), the mel spectrogram 184 MB. The decoded track stayed alive for the whole analysis, through the mel stage and all chunks.

**Change.** `analyze_controlled` hands the decoded track to the key, waveform and beat paths as an `Arc`, each drops its handle when it is done with it, and the beat path drops it inside the new `BeatEngine::analyze_resampled` right after the 22.05 kHz resample, so it is freed before the mel spectrogram and the chunks run (`BeatEngine::analyze` keeps its signature). Saves the length of the track in f32 during the longest stages: -600 MB at 60 min, -1.2 GB at 120 min in the mel stage and under the chunks. Results unchanged: the JSON of `analyze-json --threads 14` (BPM, beat grid with bar positions, key, preview and color waveform bytes) hashes the same before and after on 58 fixture tracks (the 24 long mixes and 34 others) and on the three long files.

**Not done (needs a decision):** the peak is still the chunk activations, about 400 MB times the chunk threads, and an under-1 GB peak for a 2 h track would need both of these:

1. A fixed ceiling on the chunk budget (for example 2 GB, 3-4 chunk threads on any machine): peak about 2-2.5 GB for a 2 h track, but fewer model threads on large machines, so lower throughput there; needs a quiet-box throughput run before choosing the number.
2. Streaming decode: feed the waveform meter, the 22.05 kHz resampler and the mel stream packet by packet instead of holding the decoded track (1.3 GB at 2 h); a larger rewrite of `decode.rs` and `analyze.rs`, and the streamed resampler and key decimation have to be shown bit-identical first.

**R61: chunk budget ceiling, ready to switch on (default unchanged).** `chunk_slots` (`fourfour-analysis-rs/src/service.rs`) takes an optional ceiling: slots = min(the memory rule above, ceiling / 400 MB per chunk), at least 1. The named constant `CHUNK_BUDGET_CEILING_MB` is `None`, so behaviour is as before; the dev/bench env knob `FOURFOUR_CHUNK_BUDGET_MB=<MB>` overrides it for one process (read once; a non-number panics). 2048 gives 5 chunks, 1024 gives 2. `scripts/mac/chunk-budget-sweep.sh` runs `bench_analysis` (`--features dev-bins`) over N real-length tracks of the 10k fixture (default 200: the long mixes first, then every file over 2 MB, never the 8 s ones) once per budget in `unset 1024 2048 3072 4096`, and writes `sweep.tsv` (tracks/min, peak RSS from `/usr/bin/time -l` or `-v`, process-tree RSS from the bench, CPU seconds) plus a `summary.md` table with throughput relative to `unset` (`scripts/mac/chunk-budget-summary.py`). Options: `--n`, `--budgets "..."`, `--threads`, `--list`, `--out`, `--no-build`, `--dry-run`. Pick the default on a Mac night: run it on a quiet box, take the smallest budget that keeps tracks/min within a few percent of `unset`, and set `CHUNK_BUDGET_CEILING_MB` to it.

## R66 first expand (2026-11-02, `r66-expand-stall`)

Mac night (`perf-2026-11-01-2154`, real app, 10k): the first expand had one 472-520 ms frame in 3 of 5 launches and 19-20 ms in the other 2. **Cause: not the expanded player. It is the USB volume poll.** `Sidebar.svelte` calls `refreshVolumes()` on mount and then every 5 s (`setInterval(refreshVolumes, 5000)`). That invokes `get_mounted_volumes`, a sync Tauri command (`fn`, not `async fn`), so Tauri runs it on the UI process main thread. On macOS it runs `diskutil info -plist` once for every entry in `/Volumes` (`volumes.rs` `is_removable`). On `mbp` that is 3 entries at 0.13-0.17 s each, about 450 ms. While the UI process main thread is blocked, WKWebView shows no new frames, even though the page's own main thread is idle.

Evidence:
- **The stall shows up without an expand too.** Every 5 s idle-scroll window in the same night has exactly one long frame: 504, 498, 514, 428 and 432 ms (2154), 407 and 451 ms (2230). Scroll during analysis (10 s) has 2 long frames and the 20 s bursts have 3, which fits a 5 s period. Cold startup is likely hit too: the first poll runs at Sidebar mount.
- **The split follows the click time.** Click time is about `page_ms - drawn_ms - 10 frames`. The slow launches clicked at about 4.9-5.0 s page time, the fast ones at about 5.55 s. The sidebar mounts at about 0.5 s page time, so the second poll lands at about 5-5.5 s.
- **The frontend's first expand is cheap on the Mac.** Mac `mbp`, Playwright WebKit, headless, mock backend, 10k state, load 6-7, 5 launches with 3 expands each. Longest frame: first expand 26-34 ms, later expands 20-34 ms. Longest busy period of the page main thread: 18-34 ms. No 500 ms frame, and the mock makes no IPC call during the expand.
- Box (load 34-43), same harness:
  - Chromium: first expand frame task 90-100 ms thread CPU (longest frame 267-350 ms). The second expand is 57 ms CPU (67-217 ms). About 45 ms of that gap is colder JS on the first mount (JS 70-79 ms against 25-30 ms).
  - WebKit: every expand has a 190-680 ms frame, not only the first. This is software compositing on a loaded box (R15), unrelated to the Mac stall.

**Fix (not made here: Rust was fenced for this task).** Run the command off the main thread: in `pioneer-test-ui/src/main.rs` change `fn get_mounted_volumes()` to `#[tauri::command(async)]` (or make it `async fn`). Tauri then runs it on its async runtime, and the 450 ms `diskutil` cost no longer blocks frames. No frontend or visual change, and the poll stays at 5 s. Other sync commands on the same path also call `diskutil` or block on I/O from the main thread: `require_removable_volume` (used by `write_usb`, `get_stick` and the stick commands, `wipe_usb`), `eject_volume` (`diskutil eject`, can take seconds) and `write_usb` itself (the whole sync). Moving them to `async` the same way would stop USB actions from freezing the window. A frontend-only fix can only make the poll rarer (slower USB detection) or pause it during interactions. That hides the stall rather than removing it, so it was not done.

After the Rust change, run `scripts/mac/perf-night.sh --expand-runs 5`. Expected: every first expand around 20-35 ms, no 400-500 ms frame in the idle-scroll windows, and possibly a faster cold "interactive".

**Built on r67 (`r67-async-volumes`):** `get_mounted_volumes`, `eject_volume`, `get_stick`, `stick_add_tracks`, `stick_remove_tracks`, `stick_add_playlists`, `stick_remove_playlists` and `get_volume_capacity` are now `#[tauri::command(async)]`. `write_usb` and `wipe_usb` stay sync: nothing prevents a concurrent sync and wipe of one stick in the backend or the frontend (`sync_usb_many` has no guard either). Mac check pending (`perf-night --expand-runs 5`).

## R70 main-thread audit (2026-11-02, `r70-sync-audit`)

Every `#[tauri::command]` in `pioneer-test-ui/src` was checked. A sync `fn` runs on the UI process main thread, so one that blocks freezes WKWebView on macOS (R66). Measured with a throwaway `--release` test on the `bench_startup gen` fixture (10k and 50k tracks, 20 playlists), box under load (other builds running), so read these as upper-ish bounds. Commands taking the library mutex also block the main thread whenever `load_state`, a sync, an import or an analysis write holds that mutex.

| Command | Was | ms at 10k / 50k | Decision |
|---|---|---|---|
| `enqueue_analysis` (select all) | sync | 261 / 822 (200 ids: 4 / 3) | `command(async)` |
| `resume_analysis` (launch) | sync | 18 / 526 | `command(async)` |
| `remove_tracks` (1 / 100 / 1000) | sync | 3, 81, 780 / 13, 194, 2130 | `command(async)` |
| `set_test_cues` (rebuilds all infos) | sync | 98 / 288 | `command(async)` |
| `read_backup` (`get_all_tracks` for a count) | sync | 214 / 488 | `command(async)` |
| `save_state` (replace playlists; 30 big playlists of n/5 ids) | async (r72) | 3 (generated) to 461 / 0.03 (generated) to 2320 | moved in r72 with an ordered frontend save queue |
| `get_row_previews` (60 ids, scroll path) | sync | 1.1 / 0.7 | `command(async)`: waits on the library mutex, result merged by id |
| `reveal_in_finder` | sync | process spawn | `command(async)` |
| `read_usb_state` | sync | stick I/O, SQLCipher open | `command(async)` |
| `locate_track_file` | sync | library mutex + file check | `command(async)` |
| `change_library_path`, `reset_library` | sync | DB open, file deletes | `command(async)` |
| `get_track` / `resolve_analysis_work(1)` | inner | 0.05 / 0.13-0.19 | n/a (single ids are fast) |
| `get_library_health` | sync | health mutex only, held briefly | kept sync |
| settings getters and setters, `rekordbox_default_master_db`, `app_version`, `log_directory`, `log_frontend`, `perf_mark`, `cancel_analysis` | sync | microseconds (tiny config read or write, rate-limited log line) | kept sync |
| `playback_*` | sync | audio engine, not measured (no audio device on the box) | kept sync: strict call order (play, seek, pause, tempo) |
| `write_usb`, `wipe_usb` | sync | - | untouched (r69) |
| everything else (`load_state`, `get_track_artwork`, waveform, edit and import commands) | `async fn` | - | already off the main thread |

**Left open: `save_state`.** It is the largest remaining offender (460 ms at 10k and 2.3 s at 50k for very large playlists; a few ms for normal ones). The frontend sends a full playlists snapshot and callers do not serialise their calls (`saveState` in `library.svelte.ts`), so concurrent async runs could apply an older snapshot last. Moving it needs a frontend or backend queue that keeps the order, so it was not done here.

R72 (2026-11-02): done. `saveState` is a queue (one save in flight, later calls coalesce into one follow-up that reads the newest playlists when sent; a pagehide listener flushes a waiting save), and `save_state` is `#[tauri::command(async)]`.

`playback_play` may also touch the disk (`probe_duration_ms` when the frontend gives no duration hint, first-time output open). Measure it on the Mac before deciding.

The r67 source-level test (`volume_and_stick_commands_run_off_the_main_thread` in `main.rs`) now covers all moved commands.

## R89 Mac night 2 (2026-11-03, `r89-mac-night-2`)

Mac `mbp` (M5 Pro, 24 GB, macOS 26.3, on AC), headless runs under `nice -n 10`, one launch at a time, `HOME` pointed at a scratch folder as `perf-night.sh` does. Code: `ov-1028` c2bd463 plus one build fix (23534cc, below). Merlin was using the Mac from about 22:00 (load 11-18, Chrome and WindowServer busy), so the first four jobs ran on a quiet machine and the variant comparison (job 7) ran under load; read it as pairs. Never touched: `~/Library/Application Support/Pioneer`, any USB or CDJ, GitHub (code went over a `git bundle` + scp).

**Build fix on this branch.** `cargo tauri build` failed on macOS for ov-1028 (`unresolved module or unlinked crate libc` in `waveform_detail.rs` `free_space_of`): `libc` was only a dependency of `cfg(all(target_os = "linux", target_env = "gnu"))`, the function uses it on every unix. Now `[target.'cfg(unix)'.dependencies]`. `parked-all-v7` has the same line and needs the same change.

### 1. Install

`scripts/mac/build-dmg.sh` (coreml): `fourfour_0.1.0_aarch64.dmg` 97 MB, sha256 46aaeb9b...81ce3, `check-dmg.sh` failures=0, `--self-check` ok. Copied to `/Applications/fourfour.app` (the binary inside is `pioneer-test-ui`), replacing the 11-01 install; the 11-01 dmg is kept at `~/dev/build/fourfour-mac/build/mac/dmg-20261101-1221`. `codesign -v` on the installed copy says "code has no resources but signature indicates they must be present" (ad-hoc signature, not checked on the 11-01 install; the dmg check passed).

### 2. perf-night 10k on the new install (`perf-night.sh --size 10k --runs 3 --expand-runs 5`)

| Metric | 11-01 (240d8cb) | 11-03 (ov-1028) |
|---|---:|---:|
| Warm startup to first row | 1.6-1.9 s | 0.60-0.64 s (621, 635) |
| Warm startup to interactive | - | 0.64-0.65 s |
| Cold startup to first row / interactive | 2.8 s | 0.87 s / 0.91 s |
| Scroll during analysis, frame p95 | 18 ms | 18 ms (max 20, 0 dropped) |
| First expand, longest frame | 472-520 ms in 3 of 5 | 29-38 ms (median 34), click to detail drawn 66-84 ms, 0 pending frames |
| `load_state` (app) | - | 53-67 ms |
| Backend `load_state` / payload | 13.8 ms / 1.7 MB | 13 ms / 1.7 MB |
| Waveform detail first / cached | 93 ms / - | 74 ms / 0.6 ms |
| Import 10k fixture / re-import | 540 ms / - | 504 ms / 112 ms |
| Analysis, rten (perf.sh, 5-7 min tracks) | 16.2 trk/min | 24.8 trk/min (12 long files: 22.1-23.3) |
| Analysis, CoreML (`bench_analysis`, 12 long files, 2 runs) | 26-32 | 32.8 and 33.4 trk/min, first result 5.4-5.6 s, 114 chunks CoreML / 40 rten |

Still open against the targets: warm first row 300 ms and cold 500 ms (now 2x and 1.7x, down from 6x), analysis throughput on rten. The `perf-night.sh` analysis rows use the rten `bench_analysis`; the CoreML row comes from the same binary built with `--features pioneer-test-ui/dev-bins,pioneer-test-ui/coreml` (the app and dmg are built with coreml).

### 3. R66/R67 freeze check

Gone. In the new build the idle-scroll windows (5 s apart each) have 0 frames over 50 ms and max frame 19 ms (11-01: one 407-514 ms frame per window). First expand: longest frame 29-38 ms, longest main-thread busy 14-24 ms, 0 frames over 50 ms in all 5 launches. Scroll during analysis: max 20 ms, 0 dropped, bursts 5 runs max 33 ms (1 burst with 5 dropped frames in the R5 default analysis run, no frame over 50 ms).

### 4. Chunk budget sweep (`scripts/mac/chunk-budget-sweep.sh --n 200`)

The fixture has 24 real long mixes; the other 176 of the 200 tracks are the 2-3 MB files, so the 200-track sweep is mostly short tracks. A second sweep on the 24 long mixes only is included. Same binary, `FOURFOUR_BEAT_RUNTIME=rten` for the rten rows.

CoreML (the shipped Mac path), 200 tracks:

| Budget MB | tracks/min | vs unset | peak RSS MB |
|---|---:|---:|---:|
| unset | 141.6 | 100% | 6444 |
| 1024 | 143.0 | 101% | 4681 |
| 2048 | 143.7 | 101% | 4653 |
| 3072 | 141.5 | 100% | 4435 |
| 4096 | 137.6 | 97% | 5327 |

CoreML, the 24 long mixes: 36.2 / 37.1 / 36.6 / 36.2 / 36.8 trk/min for unset / 512 / 1024 / 2048 / 4096, peak RSS 6132 / 4956 / 4488 / 4224 / 5496 MB.

rten, 200 tracks: unset 120.8 trk/min (6155 MB), 1024 41.2 (34%, 1961 MB), 2048 84.9 (70%, 3311 MB), 3072 103.9 (86%, 3964 MB), 4096 122.4 (101%, 5032 MB).

rten, the 24 long mixes: unset 22.2 (6349 MB), 512 3.3 (15%, 1679 MB), 1024 5.8 (26%, 2096 MB), 2048 11.8 (53%, 3095 MB), 4096 19.4 (88%, 4978 MB).

**Recommendation.** With CoreML the budget costs nothing (within 3% of unset at every value) and lowers peak RSS by 1.0-2.2 GB, so a Mac with CoreML can run a 2048 ceiling. With rten the ceiling is a direct speed cap on long tracks (2048 halves long-mix throughput, 4096 costs 12%), so one global constant must not be below 4096. `CHUNK_BUDGET_CEILING_MB`: set it to 4096 as the global default (rten 101% on mixed material, 88% on pure long mixes, peak RSS 5.0 GB instead of 6.2-6.3 GB), or keep `None` if rten-only machines matter more than the 1.2 GB; if the ceiling can depend on the runtime, use 2048 when CoreML is active and none (or 4096) for rten. The 24 GB Mac peaks at 6.1-6.4 GB unset, about a quarter of its RAM, which is fine for a foreground analysis run but is the number a 8-16 GB machine would feel. Noise in peak RSS between neighbouring budgets is about 0.3-0.5 GB (4096 above 3072 on CoreML).

### 5. Playback command timing

`perf.sh` has `latency_load_and_seek_to_first_audio` (engine level, no audio device in the path): load with duration hint 0.6 ms, load without hint 0.6 ms, seek to first audio 1.7 ms, track load to first audio 0.7 ms (target 10). The time from a `playback_play` command through the real Tauri IPC, the output device and the first audible frame is not measured by any perf script (the probe covers startup, scroll, expand and track load to `ff:player-ready` only), so that part is skipped. The R70 note about `playback_play` touching the disk (`probe_duration_ms`, first output open) stays unmeasured; the 0.6 ms no-hint load says the duration probe is cheap on this fixture.

### 6. parked-all-v7 (15fd864, schema v8) on the Mac

Built in `~/dev/build/fourfour-parked` (own clone and target dir, with the libc fix above applied locally) as an unsigned `.app` (coreml), run with `perf-night.sh --app <that .app>`; `/Applications` untouched. **Isolation:** every app run uses `HOME=<scratch>` and a `config.json` there pointing at a scratch copy of the fixture library; the installed app's `~/Library/Application Support/com.pioneer.test-ui` (library, config, anlz, waveform-detail, 471 files) was snapshotted before and after the parked runs and is byte-for-byte unchanged (size, mtime, content hash). The only thing that moved was `~/Library/Caches/com.pioneer.test-ui` (Apple's CoreML `e5rt` compiled-model cache and the WebKit cache, which the OS keeps under the real home per bundle id): 1400 to 2663 files, system-managed, not app data. The v7 backend binaries in the mac clone refuse a v8 library ("saved by a newer version of the app"), which is the intended guard, so the backend table was not rerun on v8.

| Run | Warm first row | Warm interactive | Cold first row | App `load_state` | First expand longest frame (median of 5) | Scroll during analysis p95 |
|---|---:|---:|---:|---:|---:|---:|
| ov-1028 (job 2) | 621-635 ms | 637-652 ms | 868 ms | 53-67 ms | 34 ms | 18 ms |
| parked-all-v7, fresh v7 fixture copy per run (migrates v7 to v8 every launch) | 5.6 s | 5.65 s | 6.3 s | 4.9-5.4 s | 32 ms | 18 ms |
| parked-all-v7, fixture migrated once beforehand | 591-610 ms | 635-651 ms | 639 ms | 52-65 ms | 26 ms | 18 ms |

The one-off v7 to v8 migration of the 10k library takes 5.1-5.5 s (7.0 s when the box was loaded) inside the first `load_state`, so a user upgrading with a 10k library waits that long once (expect several times that at 50k, not measured). After it, schema v8 is neutral to slightly positive on the Mac: warm first row within noise of ov-1028, `load_state` about 5 ms lower, first expand 26 vs 34 ms. Analysis throughput on the v7 backend bench is the same (25.2 vs 24.8 trk/min), so v8 does not change analysis. Verdict: schema v8 is not a Mac performance win to wait for or a risk to avoid; the case for parked-all-v7 is the other features, with the one-time migration time to be shown in the UI.

### 7. r15-reveal (00acf20) and r29-css-v4 (2c37766) against ov-1028

Both branches are not on ov-1028's base; each was merged into r89-mac-night-2 in the parked clone (r29: clean; r15: conflicts in the probe, summary script, doc and playwright config resolved to ov-1028's side, so only the WaveformView and waveform-render/display changes come from r15), built as an `.app` and run with `perf-night --expand-runs 5` interleaved with the installed ov-1028 build, with Merlin using the Mac (load 11-18, so absolute startup numbers are worse than job 2).

| Build | First expand longest frame (median of 5, ms) | Range | Click to detail drawn | Warm first row | Cold first row |
|---|---:|---:|---:|---:|---:|
| ov-1028 (installed) | 28 | 22-31 | 66 ms | 648 ms | 734 ms |
| + r29-css-v4 | 27 | 23-29 | 66 ms | 643 ms | 669 ms |
| + r15-reveal | 19 | 18-20 | 65 ms | 676 ms | 679 ms |

(The first, quieter pass of the same variants gave the same picture: expand 25 ms for r29 and 19 ms for r15 against 34 ms for ov-1028.) Reading: with the r67 fix in place the first expand is already under the 50 ms R10 trigger, so r15's staged reveal is not needed for it; it still shaves about 9 ms off the longest frame and the click to drawn time does not change (65 vs 66 ms), and startup is unchanged. r29's CSS change is invisible in these numbers (expand, startup), because the probe measures neither row style cost nor scrolling through 10k rows. Neither is worth landing on first-expand grounds; r29 is harmless, r15 is only worth it if its other changes (sliced ring fill is pixel-identical) are wanted.

### 8. 50k on the installed ov-1028 build (`perf-night.sh --size 50k --runs 3 --expand-runs 3`)

Run at 22:41-23:00 with Merlin using the Mac (load 9-11). The 25k and 50k fixtures were generated for this (20 GB + 8.4 GB, free disk bottomed at about 69 GB) and deleted afterwards; the 10k fixture stays.

| Metric | 11-01 R22 (headless-equivalent, box) / 11-01 Mac | 11-03 Mac |
|---|---:|---:|
| Warm startup to first row | 1.35 s (R22, box) | 1.34-1.47 s (3 launches) |
| Warm startup to interactive | - | 1.39-1.53 s |
| Cold startup to first row / interactive | 1.57 s (R22, box) | 1.58 s / 1.63 s (targets 1.5 / 1.6 s, open by 5%) |
| App `load_state` | - | 399-531 ms |
| First expand, longest frame | - | 29-37 ms (3 launches, no frame over 50 ms) |
| Scroll during analysis p95 | - | 18 ms (max 39 ms, 2 dropped) |
| Backend `load_state` / payload | - | 60 ms / 8.4 MB |
| Analysis rten, 5-7 min tracks | - | 21.5 trk/min |
| Import, 50k end to end | - | 2.7 s |

The 50k Mac numbers match R22's box numbers; warm first row is 1.0-1.5 s against the 1.0 s target, so the 50k Mac targets are within 5-50%.

## R93 chunk ceiling default (2026-11-03, `r93-chunk-ceiling`)

`chunk_slots` now always applies a ceiling: 2048 MB when the beat model runs on CoreML (`beat_runtime() == "coreml"`), 4096 MB on rten and ort-cpu, as R89 recommended. `FOURFOUR_CHUNK_BUDGET_MB` still overrides it; the old `CHUNK_BUDGET_CEILING_MB` constant is gone. In `scripts/mac/chunk-budget-sweep.sh`, `unset` now means this runtime default; pass `1000000` for the old no-ceiling behaviour. The ceiling only changes how many chunks run at once, so results are bit-identical: `bench_analysis --dump` of 20 tracks (3 long mixes + 17 files over 2 MB of the 10k fixture, 16 threads, dev box, rten) is byte-identical before and after.

| Dev box, rten, 20 tracks | before (no ceiling) | after (4096) |
|---|---|---|
| tracks/min | 25.1 | 23.7 |
| peak RSS | 6.4 GB | 4.1 GB |

The box was busy (load 27-37 from other workers), so the 5% speed gap is within noise (an earlier run at 8 threads, where 4096 does not bind, was 3.3 GB for both). Unit tests cover the runtime selection.
## R95 50k startup (2026-11-03, `r95-50k-startup`)

Branch `r95-50k-startup` from `r89-mac-night-2`. Linux `dev` box, headless Chromium and Playwright WebKit, `perf-startup.spec.ts` on the real 10k and 50k fixture payloads (`bench_startup dump`, column table, 1.7 and 8.4 MB), backend time simulated as 118 / 450 ms. Box load 25-57 the whole time (other workers), so absolute times are 1.5-2x the quiet-box ones in R22; every number below comes from a base run and a new run back to back (base = the same probe against `r89-mac-night-2`'s library store).

**Where the 50k time went** (CPU profile of the unminified build, warm, answer to first real row, about 290 ms at load 34): the `$state` row proxies were the biggest piece of the frontend share. Timed inside the map loop: mapping the 50k plain objects 100-150 ms, creating their `$state` proxies 150-265 ms (each proxy allocates a handler with about nine closures, two maps and a source; they are what the 50 ms GC pauses collect). After that came the first render (rows, sidebar), the search warmup (`trackHaystack`, `fillSearchIndex`, `text`, about 300 ms of CPU spread over 1.2 s, no long task but heavy GC) and decode (30-45 ms).

**Changed** (`app/src/stores/library.svelte.ts`, `table-columns.svelte.ts`; no markup, no CSS, same rows, columns, order and search results)

- **Row state proxies are created on first touch.** A row is now its plain object behind one shared handler; the `$state` proxy for it is created the first time anything but `id` is read or written (`rowStateOf`), and every trap forwards to it. Reads of `id` (the `find`/`trackIndexById` loops) never create one. The proxy is created inside its own derived: `$state` created while an effect runs is registered as "created in this reaction" and that effect would not track its first reads (`library-rows.test.ts` "follows a field write on that row only" fails without the derived). Per-row reactivity, identity in other state, `'bands' in row`, `bumpLibraryTracks(ids)` and the derived fields are unchanged (`library-rows.test.ts`, 911 tests).
- **Sorting, favourites and Recently Added read the plain objects.** With a saved sort, the first sort read every row's sort key through its proxy, which now would create all 50k states. `sortValue` reads `plainRowOf(row)` (the plain object, kept current by `bumpLibraryTracks`, which is also what invalidates the sort cache) and falls back to the row for derived fields (`status`, `cover`); `syncTrackFavoriteFields` compares against the plain object; the Recently Added filter reads `raw.tempo` from the plain objects (it also no longer makes the filter depend on 50k `raw` signals).
- **Search text is a list aligned with the plain tracks** instead of a `Map` by id plus a second 50k-element copy: same strings, same refresh on bump, 15-25% less CPU for the build in a microbench (min of 10, `process.cpuUsage`, 50k real rows) and less GC.
- `perf-startup.spec.ts`: reports `heapMB` (after a forced GC), `cpuMs` (main-thread `ThreadTime` from navigation start to 2.5 s after the first row, Chromium, first launch and cold only: a reload gets a new thread, so warm reads negative and is not printed as a result) and `FF_PERF_SORT=<column>` to boot with a saved sort.

**Before / after** (median of the interleaved pairs; "answer to first row" = `firstRealRow - loadAnswered`, the part the frontend owns; 7-8 pairs each, load 25-57)

| Tracks | Case | Answer to first row base / new | New faster in |
|---|---|---:|---:|
| 10k | first launch | 388 / 272 ms | 7 of 7 pairs |
| 10k | cold | 388 / 366 ms | 7 of 7 (median pair -95 ms) |
| 10k | warm | 388 / 265 ms | 7 of 7 |
| 50k | first launch | 785 / 629 ms | 6 of 8 (median pair -206 ms) |
| 50k | cold | 586 / 357 ms | 7 of 8 (median pair -212 ms) |
| 50k | warm | 540 / 358 ms | 5 of 8 (median pair -79 ms) |

| Tracks | JS heap after boot, forced GC (base / new) | Main-thread CPU, boot to 2.5 s after the first row (first launch, cold) |
|---|---:|---:|
| 10k | 20.2 / 15.3 MB | 830-868 / 795-820 ms |
| 50k | 72.6 / 48.8 MB | 1,476-1,541 / 1,414-1,507 ms |

Absolute first row and interactive were 1.1-1.9 s (10k) and 1.5-2.5 s (50k) in these runs because of the load and move with it; do not compare them with R22's. Long-task numbers (`longestTask` 350-600 ms, `tbt`) did not change: the mount of the app shell (about 280 ms of the boot task at 50k, `TrackTable` skeleton rows 120 ms) and the first render after the answer are untouched by this change.

Steady-state UI at 10k (`perf10k` `measure`, one base and one new run, load 41-52): sort by title script time 76 / 39 ms, by artist 103 / 41 ms (first sort now reads plain objects), scroll p95 58 / 36 ms, search first keystroke 157 / 103 ms, JS heap 21 / 16 MB; nothing slower beyond the noise.

**Read with care.** Wall-clock pairs swing +-150 ms at this load (the 50k warm row won in 5 of 8 pairs only); the heap numbers are exact, the CPU numbers repeat within 2-3%, and the CPU gain (50-100 ms at 50k) is smaller than the wall gain because the proxy cost was largely GC. The real Mac app waits for the busy page main thread before it handles the `load_state` answer (R29), so the saved main-thread work should count there too; not measured (no Mac).

**Looked at, not changed**

- Search index build still costs about 300 ms CPU at 50k and runs after the first rows (8 ms slices, 250 ms delay). Deferring it to the first keystroke would put a 100-250 ms hitch on the first search; a 1.5 s start delay did not move `frameInteractive` in WebKit (the 200-400 ms gap after the first row there is the headless compositor idle gap of R29). Lowercasing per field with caches, one joined chunk string and the default string sort all measured no better than the current code.
- Payload: at 50k file names (2.6 MB) and titles (1.6 MB) are 50% of the 8.4 MB; ids, dirs and folder parents are 0.8 MB and delta-coding them would save under 6%, about 6 ms of parse. Not done.
- Backend `load_state` on the box at 50k is 450-470 ms (list query 300, `build_track_infos` 275 and JSON 160 overlap); on the Mac it is 60 ms, so it is not the Mac's cost. Not touched.
- The app shell mount before the answer (about 280 ms in the profile) is visual/markup territory, not touched.
## R92 import without covers (2026-11-03, `r92-import-covers`)

Branch `r92-import-covers` from `ov-1028` (`c2bd463`). Linux `dev` box, release `bench_startup import`, the R13 fixtures (10k, 25k), cold = page cache of the music evicted with `posix_fadvise(DONTNEED)`, before and after built from the same tree and run interleaved (load 12-17 at 10k, 18-24 at 25k, 35 for the large-cover folder; other workers were running). Raw: `~/.cache/r92/`.

**Change.** The first tag pass (`import_plan::read_tags`) no longer goes through `scanner::read_track_metadata` but through `tag_scan::read_track_tags`, which reads the same fields with the same defaults and the same file type choice (extension), with lofty's `read_cover_art(false)` and a reader that serves zeros for the picture bodies instead of reading them from disk. lofty skips an ID3 `APIC` frame and an MP4 `covr` atom by reading them through (see R6), so the option alone saves the disk reads for FLAC only. A small walker finds the picture bodies by seeking over frame and atom headers (ID3v2.2-2.4 at the start of an MP3, in the `id3 ` chunk of WAV and AIFF; `moov/udta/meta/ilst/covr` in MP4), and the reader returns zeros for those byte ranges. Anything unexpected (tag-level unsynchronisation, extended header, a frame that does not chain to the tag end, a non-syncsafe v2.4 size, a truncated tag, an odd atom layout) gives no ranges and the file is read as before. The cover pass (`store_pending_covers`) still uses `read_track_metadata`, unchanged. `pioneer-usb-writer/src/scanner.rs` is not touched, so the field mapping is duplicated in `pioneer-test-ui/src/tag_scan.rs`; a test compares both readers on generated MP3 (ID3v2.3 and 2.4, covers of 0 B to 300 KB), broken tags and MP4. `pioneer-test-ui/Cargo.toml` gains `lofty = "0.24"` (same version `pioneer-usb-writer` already uses, nothing new in `Cargo.lock` but the edge).

| scan, backend (median of 3, 25k: 2) | 10k before | 10k after | 25k before | 25k after | 300 files, 6.8 MB covers |
|---|---:|---:|---:|---:|---:|
| scan, cold | 1.47 s | 1.01 s | 7.4 s (3.4-11.3) | 3.9 s (2.3-5.6) | 0.82 s to 0.16 s |
| scan, warm | 0.34 s | 0.26 s | 1.06 s | 0.67 s | |
| rows (plan_import), cold | 1.6 s | 1.15 s | | | |

`perf.sh --size 10k` import step on the new build: 2,648 ms end to end (budget row "import of the fixture, end to end": pass).

The 10k cold scan is now 1.0 s; a plain parallel read of the first 16 KB of each file (32 threads, cold) takes 1.5-2 s here, so what is left is the per-file open and first-page latency, not cover bytes. The 25k cold numbers swing with disk load from the other workers (the two rounds are 5.6 and 2.3 s after, 11.3 and 3.4 s before). `posix_fadvise(RANDOM)` on each file made no difference and was dropped. The background cover pass is unchanged (8 s at 10k, 19-26 s at 25k).

**Identity.** Same 10k fixture imported by both builds, library kept after the cover pass: `tracks` table (all 20 columns, 10,000 rows, ordered by path) byte-identical; `artwork` table identical (8,132 covers, 1,515,238,096 bytes, md5 per cover), `pending_artwork` empty. `cargo test -p pioneer-test-ui`: 274 passed (the existing `parallel_scan_returns_what_the_scanner_returns` test also checks the new reader against `scanner::scan_files` and `scan_directory`).

Not covered by the skip: Ogg and Opus pictures (the comment is read whole), APE, and MP4 files whose tags are not under `moov/udta/meta/ilst`; they are read as before.

**R92b fix (`r92b-import-covers`).** `robustness_tests::huge_and_special_files_do_not_hang_the_import` went from 11 s to 150 s with R92 (debug build, 256 MB sparse zero files). Two causes. (1) The RIFF walker stepped over zero-size chunks 8 bytes at a time, one seek and read per step (32 million for a 256 MB WAV or AIFF of zeros, 150-190 s); it now gives up after 256 chunks. (2) lofty's per-byte zero skip and frame sync scan, run through the custom reader, is compiled in `pioneer-test-ui` at the debug opt-level, while `scanner::read_track_metadata` is instantiated in `pioneer-usb-writer`, which builds at opt-level 3 in debug; on a 256 MB MP3 of zeros that was 97 s against 1.5 s. The custom reader now serves only files where the walker found picture bodies; MP3, WAV, AIFF and MP4 files with no picture found go to `read_track_metadata` (same fields, `artwork` dropped), other types use a plain `BufReader<File>` with `read_cover_art(false)`. `read_track_tags` also refuses non-regular files before opening them. The test takes 9.3 s (base 11 s, R92 150 s, loaded box). 10k cold scan, interleaved, load 36-40: 1.02-1.05 s fixed against 1.12-1.19 s for R92 (the 8 KB read buffer matters: 64 KB cost 0.2 s cold).

## R104 non-model stages (2026-11-04, `r104-stages`)

Branch `r104-stages` from `ov-1028` (`8249974`). Linux `dev` box (Xeon Gold 6130, 16 vCPU), release builds, baseline built from a clean `git archive` of `8249974`, load average 14-23 during every run (other workers), base and new interleaved. `FOURFOUR_BEAT_CHUNK` does not exist on `ov-1028` (r96, chunk 500, is parked), so everything ran at the `ov-1028` default of 1500 frames; the beat model share at chunk 500 is scaled with the 1.86x of R94, not measured. `perf` is not usable here (`perf_event_paranoid` is 3), so stage CPU comes from `/proc/self/stat` around each stage (`fourfour-analysis-rs/examples/stage_cpu.rs`, 3 repetitions per stage, 10 ms tick).

**Tracks (12, read-only from `benchmark/datasets`).** 3 MP3 44.1 kHz (beatport) and 1 MP3 48 kHz, 4 FLAC 44.1 kHz, 3 WAV 44.1 kHz and 1 WAV 48 kHz; 120-535 s, mean 309 s.

### Stage profile, before (CPU ms per track, mean of 2 rounds)

| stage | CPU ms | share of non-model | share of all CPU, chunk 1500 | share of all CPU, chunk 500 (est.) | owner |
|---|---:|---:|---:|---:|---|
| decode (symphonia + mono downmix) | 700 | 15.6% | 1.0% | 1.7% | `decode.rs` |
| resample to 22.05 kHz (rubato sinc) | 937 | 20.9% | 1.3% | 2.3% | beat-this / rubato |
| resample to 44.1 kHz for the key (48 kHz sources only; 2 of 12 tracks, 2.5 s and 4.1 s each) | 547 | 12.2% | 0.8% | 1.3% | beat-this / rubato |
| mel spectrogram (rten ONNX, from a first single-thread run) | ~800 | 17.9% | 1.1% | 1.9% | model |
| waveform (3-band meter, 97% of it `Meter::push`) | 740 | 16.5% | 1.0% | 1.8% | `pioneer-usb-writer/src/waveform` |
| key (`cyankey`) | 582 | 13.0% | 0.8% | 1.4% | `cyankey.rs` |
| grid fit (single-thread run) | ~170 | 3.8% | 0.2% | 0.4% | `fit.rs` |
| mp3 timeline scan | 0 | | | | |
| **non-model total** | **~4,480** | | **6.2%** | **10.9%** | |
| beat model (`predict`, 1 thread, 0.22 CPU-s per audio-s at chunk 1500) | ~68,000 (chunk 500: ~36,600) | | 93.8% | 89.1% | model |

Cover and tag reads are not part of this pipeline (they belong to the import pass, R6 and R92). A full run shows 17% of the CPU as system time and about 1.3 million minor page faults per 120 s track (about 5 GB touched); the non-model stages only touch a few hundred MB. `MALLOC_TRIM_THRESHOLD_`, `MALLOC_MMAP_THRESHOLD_` (32 MB) and `MALLOC_TOP_PAD_` changed nothing (1.29-1.35 M faults, 5.8-6.2 s sys in all three runs). The faults come from the model runtime, not from these stages; a fault costs about 4.6 us on this VM, which is also why the plain `Vec` growth in `decode_mono` is not free.

### Changes (all bit-identical)

1. **Key decimation** (`cyankey::decimate`, 70-80% of the key stage). The 65-tap FIR was one scalar `f32` accumulator per output, so each output waited on a chain of 65 dependent adds (about 113 ns). Now 32 neighbouring outputs advance together in 32 lanes (compiled to SSE vectors); every lane still adds `h[i] * x` in tap order `i = 0..64`, so each output is the same sum with the same rounding, and no FMA is used. The input is deinterleaved into its `d` phases per segment of 2048 outputs so the lanes read contiguous floats; the taps' offsets are computed once (a `div` per tap in the inner loop was the first version's bottleneck). Edge outputs keep the old scalar path. 3.75 M outputs: 425 ms scalar, 77-95 ms now.
2. **Key STFT window fill** (`Stft::magnitudes`): the zero-padding at both ends is two `fill`s and the middle a plain multiply loop, same products as before.
3. **Decode downmix** (`decode_mono`): stereo goes through `(l + r) / 2.0` over `chunks_exact(2)` instead of the generic `chunks(channels)` + `iter().sum()` + runtime divide (`Sum` of two floats starts from -0.0, so the result is the same value). Mono and more than two channels are unchanged.

Tests: `decimation_matches_the_scalar_filter_bit_for_bit` (d = 2, 4, 7; 11 lengths around the lane, segment and edge limits; 1 and 3 threads; compares `to_bits` with the old per-tap loop) and `stft_frames_match_the_bounds_checked_fill_at_both_ends`.

### Result (CPU ms per track, mean of 2 rounds, 12 tracks)

| stage | before | after | change |
|---|---:|---:|---:|
| key | 582 | 231 | -60% (MP3 1.92 -> 0.81, FLAC 1.84 -> 0.74, WAV 1.91 -> 0.73 ms per audio-s) |
| decode | 700 | 609 | -13% (MP3 3.61 -> 3.36, FLAC 2.68 -> 2.44, WAV 1.20 -> 0.83 ms per audio-s) |
| waveform, resample 22.05, resample 44.1 | 740, 937, 547 | 730, 947, 514 | unchanged (noise) |
| non-model total | ~4,480 | ~4,000 | -10.7% |

That is -0.65% of a track's total CPU at chunk 1500 and about -1.2% at chunk 500: the stages were 6-11% of the analysis, so a 60% cut of one and a 13% cut of another is small in the full pipeline. Wall time per stage fell the same (decode 729 -> 637 ms, key 608 -> 249 ms); at 2 threads the key runs beside the beat model and only the decode is on the critical path, so the per-track wall gain is about 90 ms.

**Full pipeline, `analyze-json --threads 2`, 12 tracks (dominated by the model and by the load; read as "no regression"):**

| run | order | wall | user | sys | CPU per track | load at start |
|---|---|---:|---:|---:|---:|---:|
| base | 1 | 426.7 s | 732.2 s | 153.3 s | 73.8 s | 15.0 |
| new | 2 | 411.3 s | 718.4 s | 137.5 s | 71.3 s | 16.3 |
| new | 3 | 412.1 s | 707.8 s | 143.9 s | 71.0 s | 16.4 |
| base | 4 | 507.2 s | 727.4 s | 148.2 s | 73.0 s | 18.1 |

The stage savings are about 5.7 CPU-s over the 12 tracks, against a 27 s difference in the table, so most of that gap is system-time and load noise.

### Proof

- `cmp` of the `analyze-json` output of the baseline build and this branch, 12 tracks, run twice: identical, 11,255,993 bytes, sha256 `7717f075a856eb0a3954eab2e5f3641a2fc8dccc91e650f8f93e19ec3803e547` (both builds, both rounds). Beat grid, bpm, key, waveforms and every other field are in that JSON.
- The key's confidence and tuning are not in the JSON, so `stage_cpu` also hashes (FNV-1a over `f32`/`f64` bits) the decoded mono samples, the 44.1 kHz and 22.05 kHz resampled audio and the key class, confidence and tuning of every track: identical between baseline and branch for all 12 tracks, in both rounds.

### Looked at, not changed

- **Waveform `Meter::push`: 740 ms per track, the largest controllable stage left, in the `pioneer-usb-writer` lane (measured only).** It costs 53 ns per sample whatever the signal (noise, sine, real music; 5 `f64` biquads and 5 `max`es per sample), while one biquad in a plain loop here costs 8.5 ns per sample (the add chain is the floor) and the five filters are independent except mid-low-pass after mid-high-pass. So there should be 4x or more headroom without changing a single float operation (a `channels == 1` fast path that skips the per-sample channel sum and divide, filter state in locals); not tried, because the `Meter` fields are private and `waveform.rs` in this crate cannot do it from outside.
- **Resampling (rubato) is the biggest single block**: 3.1 ms per audio-s at 44.1 -> 22.05 kHz and 6.9 ms per audio-s at 48 -> 44.1 kHz (a 48 kHz track pays 2.5-4.1 s extra for the key). rubato already runs AVX+FMA kernels chosen at run time, and its output for 48 kHz depends on the 8192-frame chunking (beat-this documents it), so nothing here can change it bit for bit. The 44.1 kHz resample could only go if the key algorithm took 48 kHz input, which changes its output.
- **Mel (rten) and the beat model** are the model; not touched.
- **Key tuning pass**: its frames (every second one) are transformed again by the band pass. Keeping their spectra (about 7 KB per frame) would save about 13% of the key stage (about 1% of the non-model total) for a buffer that grows with the track; not done. A replacement for `Complex::norm` (libm `hypotf`, about 25% of the key frame cost) would risk differences in the last bit, so it was left alone.
- Decode is now mostly symphonia (MP3 300 ms, FLAC 590 ms per 120-330 s of audio) and first-touch page faults of the sample buffer.

Commits on `r104-stages`: `ae35686` decode, `0698573` + `5d33804` key, `aa12a4f`, `fbb86e5`, `2b3f40f` bench example. Validation: `cargo test --workspace`: 26 `test result: ok` lines, 549 passed, 0 failed.

## R106 model sys time (2026-11-04, `r106-model-faults`)

Branch `r106-model-faults` from `ov-1028` (`a1b37c0`). Linux `dev` box (16 vCPU, glibc 2.41), release builds, baseline from a clean `git archive` of `a1b37c0`, load average 15-45 from other workers, base and new interleaved, 2 analysis threads (`analyze-json --threads 2`, box rule). Tracks: the 12 of `~/.cache/r106-work/tracks.txt` (4 MP3, 4 FLAC, 4 WAV from `rekordbox-testdata-2025`, 255-420 s, mean 316 s) and one 46 min WAV (8 of them concatenated). `perf` and `strace` are not available, so the fault source was found with a counting `#[global_allocator]` and `/proc/self/stat` around single chunk runs (`fourfour-analysis-rs/examples/chunk_faults.rs`, `CHUNK_FRAMES`, `CHUNK_ROUNDS`). **Result: the cause is found and a fix exists (-22% CPU, outputs byte-identical), but it costs +10% to +23% peak RSS, so it is not the default; doc-only, no behaviour change on the branch.**

### Where the faults come from

- rten creates a fresh `BufferPool` inside every `Model::run` (`graph.rs`, `let pool = BufferPool::new()`, same in 0.27) and drops it when the run ends. `RunOptions` has no way to pass a pool in, so nothing the caller does can keep activations alive between chunks, and a persistent worker thread changes nothing.
- One beat-model chunk of 1500 frames allocates 38 blocks of 128 KB and more, 1,082 MB in total. Six of them are the attention matrices: one of 274 MB and five of 137 MB (959 MB, 88% of the bytes). All are freed at the end of the run.
- glibc cannot keep those: blocks above 32 MB (the largest `M_MMAP_THRESHOLD`) are always `mmap`ed and `munmap`ed, and the per-thread arenas that serve the rest are 64 MB heaps that glibc deletes and trims as soon as they are free. So every chunk faults in about 235k pages (about 940 MB) again: 1.0-1.2 s of system time against 4.7-5 s of user time (17-20%). The R104 `MALLOC_MMAP_THRESHOLD_`, `MALLOC_TRIM_THRESHOLD_` and `MALLOC_TOP_PAD_` runs changed nothing because they left the arenas and the 137/274 MB blocks alone.
- **At chunk 500 the problem does not exist.** `chunk_faults` with `CHUNK_FRAMES=500`: 30 blocks, 146 MB, none above 32 MB, 0-543 faults and 0 ms system time per chunk with the default allocator. The faults are the attention matrices of the 1500 frame chunk (they grow with the square of the length). If the parked chunk 500 (r96) lands, this item is gone without any allocator setting. `FOURFOUR_BEAT_CHUNK` does not exist on `ov-1028`.
- Not the cause: our own per-chunk `Vec`s (`extract_chunk` 768 KB, the output logits), the mel stream, decode (R104), or the thread pool scratch.

### What fixes it

`MALLOC_ARENA_MAX=1 MALLOC_MMAP_MAX_=0 MALLOC_TRIM_THRESHOLD_=1073741824` (env vars, no code): one arena, so everything lives in the brk heap; no mmap, so the big blocks live there too; trim only above 1 GB free at the top. All three are needed (single thread, `chunk_faults`, per chunk): default 235k faults and 1.1 s sys; `ARENA_MAX=1` alone 264k; `MMAP_MAX=0` + trim without `ARENA_MAX=1` 224-246k (heaps of non-main arenas are deleted regardless of the trim threshold); `ARENA_MAX=1` + `MMAP_THRESHOLD=32 MB` (mmap kept) 238k; all three 0 faults, 0 ms sys, wall 6.2 s to 5.1 s. `MALLOC_ARENA_MAX=4` + `MMAP_MAX=0` + trim (the app's R49 cap with the rest) did nothing in the full run (sys 52.9 s, 11.4 M faults), so the app's four arenas do not give this for free. A trim threshold of 256 MB kept 4.5 M of the 12 M faults (sys 20.8 s), 64 MB 5.9 M; 1 GB and 2 GB both reach 0.3 M.

### Numbers (12 tracks, `analyze-json --threads 2`, mean of 2 runs each, interleaved base, new, new, base)

| | user | sys | CPU per track | minor faults | wall | peak RSS |
|---|---:|---:|---:|---:|---:|---:|
| base (default glibc) | 787 s | 167.6 s | 79.6 s | 36.9 M | 463 s | 1,168 MB (1,189 / 1,147) |
| `ARENA_MAX=1 MMAP_MAX=0 TRIM=1G` | 740 s | 2.4 s | 61.8 s | 0.33 M | 353 s | 1,284 MB (1,269 / 1,299) |
| change | -6% | -98.6% | **-22.3%** | -99% | -24% | **+9.9%** (+6.7% to +13.3%) |

The 46 min WAV (one track, 2 threads): base user 511 s, sys 110 s, 24.6 M faults, wall 324 s, RSS 1,161 MB; with the settings user 488 s, sys 3 s, 0.35 M faults, wall 261 s (-21% CPU), **RSS 1,426 MB (+23%)**. A 4 track list (3 runs per setting, `small.txt`) gave the same picture: CPU 80.7 / 79.8 / 77.4 to 61.5 and 63.2 (trim 1 GB, 2 GB), RSS 1,078-1,085 to 1,147-1,176.

The load was 15-45 during the runs; user and sys CPU are what to read, wall followed them. Every run, base or not, produced the same bytes (`cmp`-equal by sha256): the 12 track `analyze-json` output `420835363d5a2b7aebd601db15940d87962381aab65610fbce25720a76cfb89a` (2 base and 2 new runs), the 4 track list `755c474e7c1201fa715baba4cccafd0c7a5e80a47ce7bd2232eb5ab8736986ad` (16 runs, every setting and the block cache), the 46 min file `6774687d1668460fd677d81f677522ff12de7a44992221a4393ef210c45354ae` (every run).

### Why not default

- **Peak RSS +10% on typical tracks, +23% on a long mix at 2 threads**, over the +10% allowed. With the brk heap the 137/274 MB blocks of the threads interleave and the heap ratchets up in steps (46 min file: 1,013, 1,287, 1,425, 1,466 MB during the chunks) and freed audio buffers (decode, resample) leave holes that stay resident; mmap returned both at once. A `malloc_trim(0)` after the mel stage did not lower the peak and doubled the faults (0.53 M). The cost grows with the number of chunk threads, which this box would not let me run (rule: 2 threads); at 14 threads the 4 GB ceiling of `chunk_slots` plans 400 MB per chunk and the shared heap would be the first thing to exceed it.
- **A block cache in a `#[global_allocator]`** (keep freed blocks of 32 MB and more for reuse by the same size, only once a size was requested twice, 3 GB cap, `release_cached` at the end of a run) also reaches 8-11k faults and sys 4 s per 4 tracks (CPU -22%, same bytes), but retains every block of the chunk, not only the live ones: single thread chunk RSS 489 -> 630 MB (+29%), 4 track list 1,360 MB (+25%), 46 min file 1,470 MB (+27%). More code (unsafe allocator in the app binary) and worse RSS than the three env vars; dropped.
- `GLIBC_TUNABLES=glibc.malloc.hugetlb=1` (transparent huge pages for malloc): 253k -> 112k faults, sys 990 -> 670 ms per chunk, RSS equal; only half the effect and it has to be set before the process starts. THP is `always` on this box but does not apply to most of these blocks.
- mimalloc / jemalloc: not in the offline crate cache and a C dependency in the app binary; not tried. The retention trade-off would be the same kind (memory kept to avoid faults).

### Recommendation

- Do nothing in the app. If chunk 500 (r96) lands the faults disappear without a setting.
- For Linux batch runs where RSS does not matter (benchmarks, the 10k fixture on `dev`), set the three env vars: -22% CPU, same output.
- If 1500 stays the default and Linux throughput matters, the real fix is in rten: reuse the `BufferPool` across runs (an upstream change; not possible with 0.24 or 0.27 from outside). That would keep about 490 MB per thread (the live peak) instead of the 630-650 MB a cache or a shared heap ends up holding.

Commits on `r106-model-faults`: `03c3018` `chunk_faults` example (allocation count and size, faults, user and sys per chunk run, peak RSS), plus this section. Not measured: 14 threads, the app (Tauri, its arena cap), the Mac.

## R108 throughput (2026-11-04, `r108-throughput`)

Branch `r108-throughput` from `r105-snap` (`903b702`: chunk 500 default, fit fix). Question: what split of parallel tracks, threads per track and beat-model threads gives the most tracks per minute at chunk 500, with identical output, on this box (Xeon Gold 6130, 16 vCPU, one thread per core, 64 GB) and on an 8 performance-core Mac-like budget (`taskset -c 0-7`, plus `FOURFOUR_ANALYSIS_MEMORY_MB` for the memory of the machine). rten on the CPU only: the Mac's CoreML path (2 sessions, chunk 1500) cannot be simulated here.

**Bench set.** The 24 full-length masters of the 10k fixture (`~/.cache/ff-fixture/10k/long.txt`: 5 formats, 304-415 s, 8,816 s of audio); a 12-track half of it for the 8-core runs. Release builds, `nice -n 5`, interleaved configs (rotated order per repetition). The box carried load 15-45 from other workers the whole time.

### How the pipeline uses threads today

- The beat model runs on one pool of **chunk threads shared by all tracks** (`BeatEngine::share_chunk_threads`, `chunk_slots`); each chunk runs on a single-thread rten pool (the setup whose logits match the reference bit for bit), so "model intra-op threads" is always 1 and the parallelism is across chunks. A track queues its chunks, the pool serves tracks in arrival order, and a track whose chunks are taken leaves its threads to the next one.
- **Worker threads** (`plan_pool`: min(threads, memory / 900 MB, tracks left)) each run one track end to end: decode, then in parallel the waveform plus key lane and the beat prefix (resample to 22.05 kHz, mel on a pool of `threads_per_track`), then wait for their chunks, then fit. `threads_per_track` = threads / workers (1 on this box) only widens the mel pool, the key lanes and the fit.
- **Decode of track N+1 during the model on N already happens**: with W workers, W-1 other tracks are in their prefix or their chunk phase while one waits. There is no separate prefetch to add; what matters is whether W is large enough that the chunk queue never runs empty. In the app the model stage is further capped at `max(threads/3, 3)` model jobs (4 on 14 threads); the other in-flight slots take fast passes.

### Measuring on a shared box

Wall-clock tracks/min on this box is biased by the other work: the scheduler gives a process cores in proportion to its runnable threads, so a config with more threads gets a bigger share and looks faster (8 cores, W=8 against W=4: 9.2 against 7.3 tracks/min measured, with the same CPU per track and 1% idle chunk threads in both). Every table therefore has, next to the measured number, a **quiet** estimate: the batch's CPU seconds plus the seconds the chunk threads waited for a chunk (`BeatStatus::chunk_idle_s`, new), spread over the cores the run may use (at most its thread count). It is what a machine with nothing else on it would take if the CPU time stays what it was. `benchmark/scripts/r108-table.py` prints both.

### Sweep, this box (analyze-json, 24 tracks, 3 reps, medians (min-max))

| config (threads x workers) | measured trk/min | quiet trk/min | CPU / track | peak RSS | first result | chunk threads idle |
|---|---:|---:|---:|---:|---:|---:|
| 16 x 14 (default plan) | 10.70 (10.03-11.56) | 19.6 | 48.2 s | 2,533 MB | 15.9 s | 1% |
| 16 x 8 | 10.20 (10.02-11.20) | 19.3 | 49.2 s | 2,152 MB | 14.4 s | 1% |
| 16 x 4 | 10.33 (10.02-10.90) | 19.4 | 49.0 s | 2,081 MB | 11.1 s | 1% |
| 16 x 2 | 8.93 (8.62-9.58) | 14.7 | 50.6 s | 2,077 MB | 11.5 s | 14% |
| 14 x 14 (the app's cores - 2) | 10.35 (9.63-10.99) | 17.1 | 48.6 s | 2,273 MB | 17.1 s | 1% |

### Sweep, 8 cores (`taskset -c 0-7`, 12 tracks, 2 reps)

| config | measured trk/min | quiet trk/min | CPU / track | peak RSS | first result | chunk threads idle |
|---|---:|---:|---:|---:|---:|---:|
| 8 x 8 | 9.16 (9.01-9.32) | 10.0 | 47.2 s | 1,284 MB | 13.0 s | 1% |
| 8 x 4 | 7.26 (6.61-7.91) | 9.9 | 48.0 s | 1,160 MB | 12.6 s | 1% |
| 8 x 2 | 7.17 (7.01-7.34) | 9.9 | 47.4 s | 1,147 MB | 13.0 s | 1% |
| 6 x 6 (the app's cores - 2) | 6.43 (6.37-6.48) | 7.6 | 47.3 s | 1,074 MB | 14.1 s | 1% |

### What the sweep says

- **The beat model is the whole cost and the default already keeps every core on it.** Chunk threads wait 1% of the time at every worker count from the default down to 4 (16 threads) and to 2 (8 threads); CPU per track is 47-49 s whatever the split (0.13 CPU-s per audio-s: 0.118 beat model, 0.013 the rest, as R104). There is no throughput left in the split on a free machine; oversubscription costs nothing measurable (sys time 2% of CPU).
- **Workers: 4 are enough for 16 chunk threads, 2 for 8.** With 2 workers and 16 chunk threads the pool starves 14% of the time (the prefix of a track, about 2.8 CPU-s of decode, resample and mel on one thread, is longer than the chunk phase of the other: 37 CPU-s over 16 threads) and the quiet rate falls 25%. Fewer workers than the default save memory and first-result time at equal rate: 4 instead of 14 is -18% peak RSS and -30% first result. The app already runs only `max(threads/3, 3)` model jobs, so its model stage is in that regime; no change.
- **Threads per track and intra-op threads do not pay.** Giving a track more threads only widens the mel pool, the key lanes and the fit (about 10% of the CPU); running one chunk on several rten threads would trade the efficiency of independent chunks for a lower per-track latency and changes how many chunks hold memory at once. The chunk stays on one thread (it is also what keeps the logits equal to the reference), so this was not swept.
- **The one big lever is the number of cores the app leaves to the UI.** `UI_CORES = 2` takes 12.5% of the quiet rate on 16 cores (17.1 against 19.6) and **24% on 8 cores (7.6 against 10.0)**. Left unchanged: it is a UI-smoothness decision (`analysis_backend::analysis_threads_for`), and the effect on the UI while analysis runs cannot be judged here. If the Mac's QoS (analysis below the UI) is enough, `cores - 1` is worth a Mac test.

### The app scheduler (bench_analysis, `--features dev-bins`)

| 16 cores, 24 tracks, app defaults (14 threads, 14 in flight, 4 model jobs), 2 reps | measured trk/min | CPU / track | peak RSS | first result | chunk threads idle |
|---|---:|---:|---:|---:|---:|
| staged (fast pass, then grid pass; the app) | 8.97 (8.16-9.79) | 52.8 s | 1,936 MB | 16.7 s | 1% |
| `--unstaged` (one whole pass per track) | 8.27 (8.20-8.34) | 52.0 s | 1,380 MB | 14.4 s | 1% |

At full width the staging costs 1.5% CPU (the second decode and waveform of the grid pass) and nothing in throughput; the chunk threads are busy in both. Not touched.

### Change: the memory plan was sized for 1500-frame chunks

`chunk_slots` plans `CHUNK_ACTIVATION_MB = 400` per chunk thread inside a quarter of the machine's memory. That is the 1500-frame chunk (R57); the default is now 500 frames. Peak RSS against the chunk threads (same worker count 4, other tracks the same size: 2,081 MB at 16 threads, 1,160 MB at 8) is **about 115 MB per chunk thread at chunk 500**. On a machine whose quarter-of-memory budget is small, the 400 MB estimate gives fewer threads than there are cores: an 8 GB machine (budget 2 GB) ran **3** chunk threads of the 6 it was given.

`chunk_activation_mb(frames)` now takes 400 MB at 1500 frames (unchanged, so CoreML's chunk size plans as before) and 120 MB at 500, a straight line between; `chunk_slots` reads the chunk size with the new `bt::chunk_frames()`. Tests: `chunk_activation_follows_the_chunk_length`, `short_chunks_leave_room_for_every_thread_on_a_small_machine` (6 threads on an 8 GB machine: 3 slots at 1500 frames, 6 at 500); the old slot tests keep their 400 MB through `chunk_slots_with_ceiling`.

| 8 cores, `FOURFOUR_ANALYSIS_MEMORY_MB=8192` (4 GB available), bench_analysis app defaults (6 threads, 2 workers), 12 tracks, 2 reps interleaved | measured trk/min | CPU / track | peak RSS | first result |
|---|---:|---:|---:|---:|
| base (3 chunk threads) | 3.17 (2.34-4.01) | 56.6 s | 555 MB | 34.8 s |
| new (6 chunk threads) | 6.56 (6.06-7.06) | 53.3 s | 748 MB | 17.1 s |

**2.1x on an 8 GB machine** (3 chunk threads can use 3 cores, 6 use 6; the measured ratio sits at that bound). Nothing changes where memory is not the limit: this box (16 and 8 cores, 26-64 GB) plans the same threads before and after, and a 16 GB machine already had 8 slots. On the same simulated machine with the new binary, 2 reps: 2 workers (the memory-planned count) 7.19 (7.05-7.32) trk/min against 6.92 (6.81-7.03) with 4 workers, so no worker floor is needed; `--unstaged` 7.53 (7.23-7.82) with chunk threads idle 1% against 8% staged (one fast pass and one model job share the 2 in-flight slots), the price of waveforms first on a 2-worker machine, not changed.

### Knobs and instrumentation added (bench only, defaults unchanged)

- `FOURFOUR_ANALYSIS_WORKERS` (`PoolOptions::default`, so the app path and `bench_analysis` read it too) and `analyze-json --workers`: caps the tracks in flight.
- `BeatStatus::chunk_idle_s`: seconds the beat-model threads, added up, waited for a chunk; printed by `analyze-json` (stderr) and `bench_analysis` (`BEAT` line).
- `benchmark/scripts/r108-run.py` (one batch: wall, CPU, peak RSS, first result, per-track finish times, sha256 of the output), `r108-sweep.sh` (interleaved configs, taskset, simulated memory, analyze-json or bench_analysis), `r108-table.py` (medians, spread, quiet rate).

### Proof

`analyze-json` over the 24 tracks, sha256 of stdout (beat grid, bpm, key, waveforms, bar phase): base `r105-snap` and new default both `523dc97f64be81ec8679e613f09ed968791b59d7bdaf4f0cef6ccea4647322cc`; base and new on 8 cores with 8 GB simulated (new: 2 workers, 3 threads per track, 6 chunk threads; base: 3) the same hash each; in those two runs the 24 tracks took 346 s (base) and 190 s (new), one run each, load 20-30. `cargo test --workspace`: 26 `test result: ok` lines, 546 passed, 0 failed.

### Not done, and why

- Mac CoreML path (2 sessions, chunk 1500, ANE/GPU): not simulable on Linux; the sweeps above are the rten CPU path. There the CPU lanes (decode, resample, mel, key, waveform) are what parallel tracks buy, and W has to cover those, not the chunk queue.
- Wall-clock differences between worker counts above 4: none exist on a free machine; the loaded box cannot show them without the share bias described above.
- A separate decode prefetch lane: redundant with 3-4 workers (idle 1%); with 2 workers and 16 threads it would recover part of the 14% idle, but that needs a machine that is both wide and short of memory, and the chunk estimate above removes the shortage that would produce it.

## R109 budget refresh (2026-11-04, `r109-budget`)

Branch `r109-budget` from `ov-1028` (`78f4e32`). Linux `dev` box, release `bench_startup`, headless Chromium, `scripts/perf.sh --size 10k` (analysis and playback steps skipped: analysis rows belong to r108, the playback test needs a separate test build). Load average 17-22 during the suite (other workers). Everything under `nice -n 10`.

### Budget table, 10k (old = `now` in `perf-budgets.json` before this item)

| Metric | old | now | target | x off | note |
|---|---:|---:|---:|---:|---|
| cold startup to first row | 1,085 ms | 1,212 | 500 | 2.4 | frontend 1,113 + backend |
| cold startup to interactive | 1,319 ms | 1,304 | 550 | 2.4 | |
| warm startup to first row | 694 ms | 873 | 300 | 2.9 | load 17-22; was measured quieter |
| warm startup to interactive | 748 ms | 1,009 | 330 | 3.1 | |
| backend `load_state`, warm | 108 ms | 129 (8 extra runs: median 101) | 60 | 1.7-2.1 | rows query 56-64, playlists 10, table build 15, JSON 6 |
| `load_state` payload | 1.7 MB | 1.7 | 4 | met | |
| waveform detail, first time | 800 ms | 849 | 120 | 7.1 | decode plus `DetailMeter` (usb-writer crate, fenced) |
| waveform detail, cached | 2.5 ms | 3.3 | 2 | 1.6 | |
| expand to detail drawn | 102 ms | 111 | 50 | 2.2 | |
| import of the fixture, end to end | 3,700 ms | 1,911 | 2,500 | met | R92 |
| re-import the same folder | 150 ms | 158 | 600 | met | |
| import UI click to rows | 119 ms | 38.7 | 100 | met | |
| scroll during analysis, frame p95 | 32.8 ms | 43 | 28 | 1.5 | idle scroll in the same run: p95 41 ms, 116 frames lost |
| scroll during analysis, frames dropped | 66 | 145 | 40 | 3.6 | idle scroll without any analysis: 116 |
| scroll during analysis, long tasks | 57 ms | 55 | 0 | open | one task per window |
| typing during analysis, worst keystroke | 150 ms | 224 | 100 | 2.2 | idle keystroke worst 120 ms |
| track load to first audio, analysis rows | not rerun | | | | other work |

The rows that got worse against the old `now` (warm startup, scroll, typing) were measured at load 17-22 and 2-3x the box's quiet load; the "old" numbers came from quieter runs. The scroll and typing rows are dominated by the headless software compositor: with no analysis running, idle scroll already loses 116 frames (the budget said 66), and a keystroke at idle is 62 ms median and 120 ms for the first (`(program)` time, native layout and paint, is 1.1 s of the 2.0 s in a profile of 14 keystrokes; the filter itself is under 10 ms per keystroke). `import end to end` and `import UI click to rows` were refreshed in `perf-budgets.json` (R92 and the R6 landing made them faster than the old `now`); the others were not touched, because a load-affected `now` would make quiet runs pass anything.

Why this one was picked: the worst x-off is waveform detail first time (7.1x), but its time is the band meter in `pioneer-usb-writer/src/waveform/` (fenced) plus the decode, which the app already runs on six threads. Next are the scroll and typing rows (harness-bound, see above), then startup (2.4-3.1x), which sits in the app and has a measurable lever.

### What changed: `load_state` is sent when the page begins

`app/index.html` now has a small classic inline script before the bundle: if `window.__TAURI_INTERNALS__` exists it calls `invoke('load_state', {}, undefined)` and parks the promise on `window`. `startLoadStateEarly()` (called first in `main.ts`) takes that promise over (and gives it the same `toCommandError` mapping `invoke` applies) instead of sending the command itself; with no parked promise it sends it as before. So there is still exactly one `load_state` call, with the same arguments, answered and decoded by the same code. Before, the call went out after the browser fetched, parsed and evaluated the 512 kB bundle and every module it imports (page time 190-270 ms in the headless harness); now it goes out when the HTML is parsed. Unit tests: `early-load-state.test.ts` (the parked call is the one `loadState` waits for and no second `invoke` happens; a rejection becomes the typed `CommandError`; without a parked call the bundle sends it).

**Before / after** (`perf-startup.spec.ts`, 50k real payload from `bench_startup dump`, backend simulated as 480 ms, 3 runs per case and median, base and new alternated, 4 pairs, load 12-24; "base" is the same tree without this change; ms, median of the pairs):

| 50k | first `load_state` call, base / new | first real row, base / new | median pair diff | new faster in |
|---|---:|---:|---:|---:|
| first launch | 191 / 67 | 1,556 / 1,203 | -85 | 3 of 4 |
| cold | 196 / 70 | 1,464 / 1,224 | -13 | 2 of 4 |
| warm | 208 / 79 | 1,415 / 1,366 | -49 | 3 of 4 |

10k (backend simulated as 118 ms, 3-4 pairs, load 13-36): the call goes out 113-190 ms earlier in 3 of 3 pairs, but first row moves by -13 to +27 ms (inside the noise). That is expected: the answer is read by the page main thread, which is busy with the bundle and the shell until about 650-700 ms, longer than the 10k backend takes. The call only helps while the backend is slower than the page's own start: 50k (450 ms on this box), a cold page cache (the first library query after a reboot is 490 ms at 10k), and slow disks. The harness runs its fake backend (a timer, a `fetch` of the 1.7-8.4 MB payload and `JSON.parse`) on the page main thread, so it understates the gain; the real app does the library work on a Rust thread. Not measured in the real app (no Mac, no GUI here): the Mac night should show `load_state` reaching the backend earlier (`page_start` columns) and, at 50k, first row 50-100 ms earlier.

### Identity

No Rust or stored-data change. `load_state` is the same command with the same arguments, issued once; its payload, the `library.db`, and the rendered rows are produced by unchanged code. Smoke (Chromium and WebKit, whose mock is installed before the page, so the inline script takes the mock path) and vitest pass.

### Other findings

- `bench_startup run` now prints `table_build_ms`, `table_json_ms` and `table_json_bytes`: at 10k the column table costs 15 ms to build and 6 ms to serialise of the 28 ms JSON stage; the rest is the playlists (60, up to 800 ids each). Callgrind of the run (`valgrind --tool=callgrind`, `perf` is blocked here): `FolderTable::folder` is half of `TrackTable::new` (about 8,600 distinct folders in the 10k fixture, 0.86 per track; a real library has one folder per album, so a fixture-driven fix would not move a real library), the string interning 30%. `get_track_list_rows` (56-64 ms warm, pioneer-library, fenced) is the largest part of `load_state`; its 120,000 `String` columns (113,951 allocations in one run) and `sqlite3_step` are what it spends.
- `perf-q50.spec.ts` with `FF_PERF_STATE` (what `perf.sh --size` passes) failed since R22: the dump is a column table and the mock expects `TrackInfo[]`, so the page showed 3 tracks and `loadTrack` read a missing row. The spec decodes the table now (`decodeTrackTable`); `perf.sh --size 10k` runs through again.
- Not done, by lane: the detail meter (usb-writer) and the library queries (pioneer-library).

## R111 rows query (2026-11-04, `r111-rows-query`)

Branch `r111-rows-query` from `ov-1028` (r109 merge). Linux `dev` box (16 cores), release `bench_startup`, load average 13-18 for the final pairs (30-65 earlier in the work; other workers). Datasets are copies of the `ff-fixture` 10k and 50k libraries (`library.db`, `track-labels.json`, `anlz` linked), plus a realistic-folder variant of each made by `benchmark/scripts/r111-folder-variant.py` (300 and 500 album folders, 5 per artist; the fixture has about 0.86 folders per track: 8,620 at 10k and 43,200 at 50k). `bench_startup rep <dir> <runs>` (new) repeats the whole backend `load_state` (state build plus JSON) in one process and prints median (min-max) per phase; `benchmark/scripts/r111-load-state.sh pairs` runs base and new binaries alternately and `r111-summarize.py` reduces the lines.

### Where the time went (callgrind, 10k fixture, instructions of one warm pass)

- Rows query 218 M Ir on the first call of a process (the one `load_state` makes; 111 M when repeated, malloc reuses the memory): the query itself (`sqlite3_step` 144 M over two passes, `sqlite3VdbeExec` 73 M of it) with rusqlite's three API calls per text column (`sqlite3_column_type`, `_text`, `_bytes`, about 550 Ir per column, 13 columns per row), and `Row::get::<String>` (six `String` per row: allocation, copy, UTF-8 check; 120,000 allocations). The two `LEFT JOIN` lookups (artwork index, analysis rowid) and the `CASE`/`ROUND`/`COALESCE` columns cost 29 M more than three plain scans merged in Rust (stand-alone copy of the query, startup removed: joined 102 M, tracks scan 60 M, analyses scan 12 M, artwork ids 1 M).
- `TrackTable::new` 74 M: `FolderTable::folder` 36 M (8,600 folders in the fixture) and SipHash over every artist, album, genre, key and folder string 29 M, plus the 60,000 `String` of `TrackListRow` and `TrackInfo` that exist only to be read once.
- Playlists 34 M: one query plus one `ORDER BY position` sort per playlist (77 playlists, 11,559 rows).
- `backfill_row_previews` scanned all analyses once more before the rows were read (3.7 ms at 10k, 19 ms at 50k) to find the NULL previews, and `track_count` was another table walk (1.3 ms at 10k).

### Changed (payload bytes identical, no schema or index change)

- `pioneer-library`: `for_each_track_list_row` (borrowed texts, no `String` per column) reads `tracks`, `analyses` and the `artwork` ids as three scans in id order and merges them in Rust (tempo, key and stage bits computed the way the SQL did; `sqlite_round` is SQLite's `ROUND`). `get_track_list_rows*` are built on it, so there is one definition of a list row. The scans run in one read transaction. The scan also counts analyses with a missing row preview, so `load_state` calls `backfill_row_previews` only when there is something to fill instead of scanning for it first. `get_all_playlists` reads `playlist_tracks` once and groups it in memory (same order: position, then track id) instead of one sorted query per playlist.
- `pioneer-test-ui`: `loaded_state` builds the column table straight from the borrowed rows (`TrackTableBuilder`): titles, file names, artists, albums, genres, keys and folders live in arenas with an open-addressing index, so a text is allocated once when it is new and never per row. `LoadedState` holds the table. Same strings table order (first seen, column by column, colours last in each row), same folder order.
- Large libraries (track ids spanning at least 4,000) are scanned in up to four id ranges on separate threads (`TrackListScanner`: a read-only connection each, the first range on the library's own connection); the tables are joined in id order (`TrackTableBuilder::append`), which gives the table one scan gives. Playlists and the labels cache are read concurrently before that, as before.

### Before / after (median of the per-process medians over interleaved pairs; range of those medians in brackets; `load_state` = state build plus JSON, warm)

| Library | Pairs x runs | `load_state` base | `load_state` new | Rows query alone base / new | Table build, base | JSON, base / new |
|---|---|---:|---:|---:|---:|---:|
| 10k, fixture folders | 5 x 15 | 83.7 ms (81.5-84.3) | 44.1 ms (40.8-48.3) | 50.2 / 38.9 | 14.0 | 8.2 / 6.7 |
| 10k, 295 folders | 5 x 15 | 74.8 ms (72.9-77.7) | 37.0 ms (35.9-41.6) | 49.1 / 38.4 | 7.6 | 7.3 / 5.9 |
| 50k, fixture folders | 4 x 8 | 466.1 ms (405.6-521.5) | 194.4 ms (161.1-295.4) | 284.1 / 224.2 | 81.2 | 46.0 / 39.9 |
| 50k, 500 folders | 4 x 8 | 417.4 ms (341.3-504.4) | 154.8 ms (148.7-280.6) | 314.8 / 229.2 | 41.9 | 30.3 / 31.7 |

Fastest single runs (base / new): 10k fixture 79.3 / 35.1 ms, 10k realistic 68.7 / 27.9, 50k fixture 389.2 / 149.1, 50k realistic 320.6 / 120.3. Box load was 13-20 during these pairs, with spikes from other workers (slowest single runs up to 1.9 s at 50k), which is why the 50k ranges are wide. An earlier set of pairs on the commit before the hash-function cleanup (load 13-18) gave 84.5 / 46.2, 80.1 / 51.1, 455.9 / 296.0 and 372.8 / 208.4 ms for the four rows: the same direction, the new side 40-60 ms slower at 50k there because of less idle CPU for the scan threads. In the new code the table is built while the rows are read, so "table build, base" has no new counterpart (the JSON column of the base is its `serialize_ms` minus the table build). The scan threads need idle cores: with the box at load 30 and above they gain much less. Without threads (commit `aa1d569`) the same code ran at 54-64 ms (10k fixture) and 223-229 ms (50k realistic).

The 60 ms target for 10k is met (44 and 37 ms median, fastest 35 and 28). What is left at 50k (realistic): rows scan 138-161 ms of CPU (about 2.8 us per row, split over the threads), playlists 9 ms, JSON 30 ms, joining the range tables about 10 ms.

### Identity

`bench_startup dump` writes the `load_state` response as `serde_json::to_vec(&loaded_state)`; sha256 of the base build (`f877cb0`, old code plus the `rep` command) and the final build:

| Dataset | Bytes | sha256 (base and new equal) |
|---|---:|---|
| 10k fixture | 1,699,963 | `38fe2965bc4aa7d3218fbb616669960f466e297f282bac861d8dd0dcd2249eb0` |
| 10k, 295 folders | 1,495,634 | `1817f860d6c7a7137ecb36b3a67899a2091bb0adfd61461ff93ea8e9614231cf` |
| 50k fixture | 8,416,233 | `297cb41d20496dc9c9a74e93c71f3e4f248c6fca76e0a855225339bdd41c0dc9` |
| 50k, 500 folders | 7,315,166 | `6b7ceabd09beffdf34a64ba8a773a3b04a6b39061b34b74b5969f42bda448b81` |
| 10k with edge data | | `7476e12bb6619ef408e7f00d97ec7e6231b5f9576442eb0d6346d661138f2814` |

The edge copy of the 10k library has empty first and every n-th artist, album, genre, key and title, bare, backslash and `C:\` paths, quotes and non-ASCII in titles, analyses with bpm 0, an empty key, bpm 127.995 and 128.005 (rounding), stages 3, 3% of the analyses without a row preview and a fifth of the covers deleted; the base ran on one copy and the new build on another (so it also filled the previews on the new path: the `analyses` table of both ends with the same sha256, `8e90e3c3...`). `library.db` and `track-labels.json` of the 10k and 50k copies have the same sha256 as the untouched fixtures after dozens of runs of both builds. The empty-string case found a bug in the first version of the builder (the per-column run caches started at index 0, so an empty first album was read as the first artist); the fixtures never have it, `loaded_state_sends_tracks_as_the_table_the_app_decodes` and the new tests do.

New tests: `dto::appended_tables_equal_the_table_of_all_rows` (joined tables equal one table at several cuts, shared strings across columns, ratings and colours), `track_infos::scanning_in_ranges_gives_the_table_of_one_scan` (1, 2, 3 and 7 id ranges on a library file give the JSON of one scan, ratings and colours included) and `id_ranges_cover_every_id_once`.

### Schema and the r84 branch

No schema or index change is needed and none would help much: the scan is CPU-bound in the per-column API (about 550 Ir per column), not in page reads (`mmap_size` changed nothing measurable and a larger `cache_size` was no better; a covering index would shrink 900 pages to about 600 at 10k and leave the 13 column reads). `r84-path-keys` (schema v8) does not change this: it adds four key columns to the end of `tracks` and replaces the `artwork` blob with a cover hash, which keeps the `artwork` id scan as it is. It touches `bench_startup.rs` as this branch does (a plain textual merge), not `queries.rs` or `lib.rs`.

### Looked at, not changed

- Raw sqlite3 calls through `rusqlite::ffi` would cut the three API calls per text column to one, but rusqlite 0.33 does not expose the statement handle, so it means preparing and stepping statements by hand with `unsafe`; not worth the risk for another 20-25% of the scan.
- The labels cache is needed before the first row (colours are interned in row order), so it cannot overlap with the scan; reading playlists on another connection while the first range scans would save about 9 ms at 50k.
- Ranges are cut by id value, so a library with many deleted ids gets uneven ranges (the result is still the same table). Each range sees its own read snapshot: a track imported while `load_state` runs can be missing from one range and present in the next (the old single query was one snapshot).

## R112 expand detail (2026-11-04, `r112-expand-detail`)

Branch `r112-expand-detail` from `ov-1028` (`6ec34d9`, r109 and r111 included). Linux `dev` box, headless Chromium 1217 and Playwright WebKit, the 10k fixture's real `load_state` as mock payload (`~/.cache/r95/state10k.json`), the mock serving the detail series (`__smokeDetail = 'serve'`), load average 15-30 (other workers). Result: **the 111 ms does not come from work the app can drop; no change to `app/src` or `$ds` code landed.** What landed is the measuring: three new profile tests in `perf-q50.spec.ts`, `scripts/perf-trace-phases.mjs`, and a second budget row that separates the work from the frame cadence. A fix for a steady-state cost found on the way is written up below as a proposal (it lives in the fenced `prototyping/viewport/design-system/waveform-display.js`) with its identity proof.

### Where the 111-120 ms goes

`expand to detail drawn` is: click, then the zoom canvas gets `data-bands="detail"`, then the harness's poll sees it in the next rAF, then two more rAFs. Phases from a Chromium trace of 5 expands (`FF_PERF_PROFILE=expand-phases`, `node scripts/perf-trace-phases.mjs trace.json`; main-thread exclusive time, median of the 5, tracing on so a little slower than the plain metric):

| Phase | ms | Notes |
|---|---:|---|
| wall, click to drawn | 129 | plain metric, 8 runs per round: 96-130 over four rounds |
| painted (data-bands set, JS side done) | 38 | plain metric: 27-35 (new row `expand to detail painted`) |
| main thread busy | 121 | wait (vsync, rAF scheduling) 8 |
| JS | 44 | Svelte mount of `ExpandedPlayer` 6-9, first full paint of both canvases 20-29 (zoom 13-18, overview 6-10), one 20 Hz `playback-tick` flush 3 |
| style + layout | 12.5 | |
| Paint | 7 | |
| Layerize | 23 | 9 in frame 1, 6.6-7 in each later frame |
| Commit | 22 | 6-6.6 in every frame |
| GC | 0-11 | one minor GC in some runs |
| frames | 4 | start at +7, +66, +99, +119 ms and last 57, 21, 19, 14 ms |

- **The work is about 38 ms** (click task 7-11 ms with the mount, frame 1 up to the paint). The first frame that shows the detail is presented about 65 ms after the click (frame 1 is 57 ms long: layout 5, rAF paint 20, Paint 5, Layerize 9, Commit 6).
- **The other 55-65 ms are three more frames.** The poll's rAF is registered before the display's own rAF (the poll starts synchronously after `click()`, the mount happens in the microtask after), so in frame 1 it runs before the paint and sees nothing; frame 2 sees `detail`, two more frames follow. Each of these frames costs 14-21 ms of main thread in headless Chromium (Layerize 6.6-7 + Commit 6-6.6 are both every-frame costs here) and starts 17-27 ms after the previous one.
- **Decode, band prep, the zero-copy read** and the Tauri call are not in it: the series is cached from the track load (`loadZoomDetail` at `loadTrack`), expand finds `bandsZoom` already on the track and `setDetailBands` is a no-op. `getBoundingClientRect` forces one layout (0.4 ms in the trace). No canvas resize churn (one resize per canvas, at the first paint), no repeated full redraw (the 20 Hz tick repaints only the scrolled columns, 1-4 ms).
- WebKit (Playwright, software compositing): drawn 335-580 ms, painted 64 ms. The frames are 80-100 ms each there (R15, R34); the work is the same 40-60 ms.

So the 50 ms target cannot be reached by cutting work: even with zero app work the harness waits three frames after the paint. `painted` is 27-38 ms, under 50. I left the `drawn` definition alone (budgets and earlier sections use it) and added `expand to detail painted ms (data-bands set)` next to it (`expand_detail_painted` in `perf-budgets.json`, now 33 ms, target 50, met). The `drawn` row now says what it measures.

### A steady-state cost found on the way (not landed: fenced file)

While playing with the expanded player open, **Layerize runs in every frame (87 of 87 frames, median 6.6 ms, 658 ms of a 1.5 s window, 44% of the main thread)** and Commit adds 2.2 ms. Chromium re-layerizes the whole page whenever a non-composited element's `transform` changes, and the overview playhead's `style.transform` changes every frame (position quantised to 1/64 px, 0.083 px per frame on a 5 minute track). The cost grows with the page's cc layers: the visible table rows have about 33 canvases (33 texture layers); hiding the row canvases halves it (6.7 to 3.0 ms), hiding the table to 0.4. Hiding only the overview playhead takes Layerize to 0.0 per frame (`FF_PERF_PROFILE=frame-cost`, variants in the test). `will-change: transform`, `translate3d` and the individual `translate` property all still re-layerize every frame; `left` does not re-layerize but snaps to whole pixels (different pixels). Driving the playhead with a paused Web Animation (`el.animate(...)`, `currentTime = x`) is composited, so a position change costs no Layerize at all.

Proposal for `prototyping/viewport/design-system/waveform-display.js` (the same change covers the overview playhead, the compact strip's played mask and the zoom playhead):

```diff
@@
 const WHEEL_LINE_PX = 16;
+const SLIDE_PX = 100000;
@@ constructor
         this._playheadX = new WeakMap();
+        this._slides = new WeakMap();
@@ _showPlayhead
         this._playheadX.set(el, x);
-        el.style.transform = `translateX(${x}px)`;
+        this._moveTo(el, x);
     }
@@ _showPlayedMask
         this._playheadX.set(mask, x);
-        mask.style.transform = `translateX(${x}px)`;
+        this._moveTo(mask, x);
     }
+
+    _moveTo(el, x) {
+        let slide = this._slides.get(el);
+        if (!slide) {
+            slide = el.animate([{ transform: `translateX(${-SLIDE_PX}px)` }, { transform: `translateX(${SLIDE_PX}px)` }], { duration: 2 * SLIDE_PX, fill: 'both' });
+            slide.pause();
+            this._slides.set(el, slide);
+        }
+        slide.currentTime = x + SLIDE_PX;
+    }
```

Measured with the change applied temporarily (reverted, nothing in the tree), expanded player playing, 1.5 s trace windows, base and new interleaved on the same box:

| | Layerize | Commit | frames |
|---|---:|---:|---:|
| before | 87 x 6.6 ms = 658 ms (also 611 ms in a second pair at load 29) | 87 x 2.3 = 242 ms | 87 |
| after | 92 x 0.0 ms = 27 ms (29 ms in the second pair) | 92 x 2.8 = 279 ms | 92 |

Effect on `expand to detail drawn` is none within the noise (the frames are vsync-bound, not work-bound): chromium, 4 interleaved rounds of 8 expands, round medians base 108 / 130 / (load 30, 400+) / 96 ms against new 112 / 118 / (load 30, 347) / 107 ms; WebKit, 3 rounds, base 475 / 446 / 400 ms against new 515 / 373 / 448 ms. The gain is the steady-state main thread while a track plays with the player open (7 ms per frame, 40% of the thread in headless Chromium). On WebKit (the Mac webview) it is unmeasured as a cost; the change was only checked for identity there.

**Identity** (`FF_PERF_PROFILE=expand-identity`, 3 tracks, 6 seek positions, 4 zoom levels per seek = 72 snapshots; each: sha1 of `toDataURL` of the overview canvas and of the zoom canvas, plus the sha1 of a screenshot of both canvases with the DOM playhead on top; the player paused so playheads are static):

- Chromium: 72 of 72 identical in overview canvas, zoom canvas and screenshot, base against new (60 distinct zoom canvases, all with `data-bands="detail"`).
- WebKit: base against new, 72 of 72 screenshots identical; 71 of 72 canvases identical. The one that differs (`track7 seek0.1 wheel0`, zoom canvas) also differs between two runs of the unchanged base (WebKit timing of the zoom view's window fetch), and base-run-2 against new is 72 of 72.
- Toy page, playhead at 40 sub-pixel positions at device pixel ratio 1 and 2: painted `translateX` and the paused animation give byte-identical PNGs (40 of 40 each).

### Not done

- Typing during analysis (worst keystroke 224 ms): not the same root cause. The keystroke path is the filter (under 10 ms) plus native layout and paint of the table (R109); the playhead is not involved. The only link is the per-frame Layerize/Commit cost that scales with the table's row canvases (33 texture layers); replacing a static row canvas by an image would cut it, but that is a change to how rows are drawn, so not done here.
- Moving the first paint into the click task would make the poll see it one frame earlier (about 20-30 ms on `drawn`), but the paint was deliberately moved out of the caller's task (`paint: 'frame'`) and that code is fenced.
- Not done: the `drawn` definition, a quiet-box run (load never under 15), the Mac.

### Tools added

`app/smoke/perf-q50.spec.ts`: `expandToDetail` now also returns `painted` and writes the marks `ex-click`, `ex-detail`, `ex-drawn`; tests `q50 expand phases` (trace of 5-12 expands, `FF_PERF_NOTRACE=1` for the plain metric only, `FF_PERF_CSS` injects a stylesheet), `q50 expanded frame cost` (traces of the playing expanded player, variants via `FF_PERF_VARIANTS`) and `q50 expand identity`. `scripts/perf-trace-phases.mjs` prints the phase table above.
## R113 search (2026-11-04, `r113-search`)

Branch `r113-search` from `ov-1028` (`6ec34d9`, r95 lazy rows and r111 column table included). Linux `dev` box (16 cores), headless Chromium and Playwright WebKit. Load average 10-17 for most of the work with spikes to 40 from other workers; every browser number below comes from interleaved base and new runs (base = `ov-1028` unchanged, same spec file), and a run whose median over all steps is above 1.5 x the pooled median was dropped as a load spike (the count is under each table). Real payloads from `bench_startup dump` (10k: 1.7 MB, 50k: 8.4 MB, the r95 dumps), `FF_PERF_STATE` as in R109. Headless compositor caveat from R109 applies: `total` is key (or pointer) down to the second animation frame after it and is mostly native layout, paint and compositor time; `script` is the Chromium `ScriptDuration` delta of that step (CDP, not available in WebKit; there `js` is key down to a `setTimeout(0)` queued by the same event, which is the handler plus the Svelte flush).

### Where a keystroke goes

- **Typing without a sort is already cheap in script.** Median keystroke at 10k and 50k: 11-13 ms of script out of 55-57 ms total in Chromium; the rest (about 80%) is native style, layout, paint and the compositor frame, which this item does not touch. The filter itself: narrowing a previous result costs 0.01-3 ms at 10k; only a full scan costs 2 ms (10k) or 10-15 ms (50k, first letters and widened queries).
- **A sort multiplies that by 10 to 100.** The sorted list was re-sorted from scratch whenever the filtered set changed (every keystroke, backspace and clear while a sort is set) and whenever a bump swapped the list (every analysed track while sorted). `sortTracks` mapped every row to `{track, value}` (a `WeakMap` lookup per proxy), ran a comparator sort and mapped back. Store level, one thread, real 50k payload: sorted typing "a" 196 ms, clear search 172 ms, one analysed track while sorted by BPM 208 ms; at 10k 36, 34 and 41 ms.
- **Other per-keystroke work** (Svelte flush of 40-50 visible rows, palette groups, `mark_reactions` for `ui.searchText`): 6-10 ms of script at the floor, not reduced.

### Changed (no markup, no CSS, same rows, same order)

- `app/src/lib/sortOrder.ts` (new): `sortedOrder(values, direction)` gives the order the comparator sort gave (no value last in both directions, equal values in list order) as one native numeric sort over `rank * n + position` plus the inverse rank; `sortedSubset` orders any subset of positions by that rank (a numeric typed-array sort, no comparator); `withChangedValues` moves the positions whose value changed with a binary search (up to 64, else it sorts anew).
- `app/src/stores/table-columns.svelte.ts`: `sortTracks` of rows that are `library.tracks` or a `filterTracks` result of it takes the order from a cache of the whole library by `key` and direction (sort values read from the plain objects by position, no per-row `WeakMap` or proxy read) and sorts only the subset by rank. After a bump (`bumpLibraryTracks`) the cached values of the bumped rows are read again and those rows are moved; any other change of the list (load, import, append) sorts anew. Playlists, USB lists, the Only-problems list and the `fav` key keep the old comparator path.
- `app/src/stores/library.svelte.ts`: `filterTracks` keeps up to 16 queries per source list (a `WeakMap` by list, so the palette's `library.tracks` and a playlist table no longer evict each other) and narrows from the smallest earlier result whose terms are all contained in the new terms; a backspace is a map hit. Single-term queries skip the closure. Results of `library.tracks` carry their positions (`libraryPositionsOf`, valid only while the list is the one they came from). After a bump the result of the same query is rebuilt from the earlier one by testing only the bumped rows (`libraryPositionsChangedSince`, new; `libraryRowsChangedSince` is built on it), not by scanning the whole library.
- Tests: `sortOrder.test.ts` (random strings, numbers, ties, blanks, both directions against a comparator sort; subsets; moved values), `search-sort-fuzz.test.ts` (600 random steps of typing, backspacing, edits with bumps and sort changes on a loaded library against a plain reference; results, positions, other source lists), and the env-gated `search-path-real.test.ts` (real payload through `loadState`, 54 scripted queries and sorts plus 4 bump scenarios: step timings and a hash of the full ordered id list of every result). The perf spec `perf-search.spec.ts` (`FF_PERF_STATE=<dump> FF_PERF_RUNS=n [FF_PERF_FRESH=1] npx playwright test perf-search`) types, backspaces, clears and sorts through the real UI, idle and while `analysis-partial` events arrive at 6 per second, and hashes the rows on screen at the top, middle and bottom of the list after each stage.

### Identity

- Store level, whole lists: 58 result lists (queries typed letter by letter, backspaces, clears, five sorts, sorted typing, filtered sorts, four bump scenarios) hashed over every id in order, base (`ov-1028`) against new, on the 10k and the 50k payload: 58 of 58 equal at both sizes (`FF_PERF_STATE=... npx vitest run src/stores/search-path-real.test.ts`; the same test file ran against both trees). Examples at 50k: "g" gives 37,232 rows (`6f22cb7d-1d56bbe3`), sorted by artist 50,000 rows (`c66ddf75-15d2b004`), sorted by artist then "a" `aa0d7f077a24f7f5`; bump while sorted by BPM `ce147c01412a44c3`.
- UI level: the rows on screen (text of every cell, row index) at the top, middle and bottom of the list after each of 11 stages per scenario, hashed in the page: equal between base and new in every run of every browser and size (idle scenarios compared; the analysis scenario shows the same hashes as well, which it need not).
- Fuzz: `search-sort-fuzz.test.ts` compares 600 steps of random operations against a reference that filters and sorts the plain rows with the old comparator; deliberate breaks of the incremental filter and of the sort-order update make it fail (checked).

### Before / after

Store level (script ms, one thread in Node, the real payload through `loadState`, median of 7 repeats, `search-path-real.test.ts`; "sorted type a" etc. are the steps of that script):

| Step (script ms, one thread, median of 7) | 10k base | 10k new | 50k base | 50k new |
|---|---:|---:|---:|---:|
| type 'g' | 7.5 | 2.1 | 10.3 | 10.2 |
| type 'hou' | 2.9 | 3.2 | 15.5 | 14.9 |
| backspace 1 | 2.9 | 0.0 | 13.6 | 0.0 |
| clear | 0.0 | 0.0 | 0.0 | 0.0 |
| sort artist asc | 73.5 | 27.5 | 406.0 | 125.2 |
| sort artist desc | 56.6 | 7.4 | 304.0 | 30.8 |
| sort title asc | 94.2 | 35.0 | 513.6 | 190.3 |
| sort bpm asc | 54.4 | 10.5 | 289.7 | 52.1 |
| sorted type 'a' | 36.4 | 1.7 | 195.7 | 10.4 |
| sorted type 'am' | 11.2 | 5.0 | 57.0 | 22.0 |
| sorted backspace 1 | 4.8 | 0.0 | 24.0 | 0.0 |
| sorted clear | 34.1 | 0.1 | 172.1 | 1.0 |
| sorted house | 9.7 | 0.2 | 48.5 | 1.3 |
| bump, no filter, no sort | 0.7 | 0.7 | 1.3 | 1.3 |
| bump, filter house | 3.5 | 0.7 | 21.0 | 1.9 |
| bump, sorted bpm | 41.3 | 1.2 | 207.7 | 3.2 |
| bump, filter house, sorted bpm | 11.5 | 1.2 | 55.9 | 4.0 |

Browser (`perf-search.spec.ts`; ms, median of the per-run medians with the range of the run medians in brackets; "total worst" = the median over runs of each run's worst step in that group; every key is 120 ms apart; analysis = fast-pass events at 6 per second after "Analyze all unanalyzed"):

#### chromium 10k: runs kept base 10 / new 9, dropped 1 (run median over all steps above 1.5 x 66 ms, the load spikes)
| Mode | Case | total median base / new | total worst base / new | script median base / new |
|---|---|---:|---:|---:|
| idle | typing, no sort | 54 (51-64) / 55 (53-64) | 118 (92-332) / 99 (94-116) | 13 (10-15) / 12 (11-13) |
| idle | typing, sorted by artist | 58 (55-68) / 55 (53-63) | 90 (72-116) / 87 (61-121) | 13 (11-15) / 13 (11-13) |
| idle | backspace, sorted | 65 (59-73) / 57 (51-66) | 81 (70-110) / 79 (64-110) | 18 (14-20) / 10 (8-11) |
| idle | clear search, sorted | 92 (70-103) / 79 (54-106) | 92 (70-103) / 79 (54-106) | 27 (23-36) / 15 (13-28) |
| idle | sort click, all tracks | 90 (82-104) / 74 (73-118) | 117 (104-130) / 103 (82-204) | 36 (30-40) / 26 (24-44) |
| idle | sort click, filtered "a" | 89 (73-176) / 72 (63-116) | 94 (82-266) / 74 (71-127) | 32 (28-114) / 23 (17-29) |
| analysis | typing, no sort | 63 (56-80) / 62 (55-71) | 133 (112-508) / 120 (100-168) | 12 (11-19) / 11 (9-15) |
| analysis | typing, sorted by artist | 63 (57-69) / 67 (61-74) | 101 (85-112) / 106 (84-128) | 12 (10-13) / 13 (12-14) |
| analysis | backspace, sorted | 79 (60-97) / 83 (72-249) | 103 (88-120) / 108 (93-480) | 17 (13-29) / 16 (15-47) |
| analysis | clear search, sorted | 109 (93-153) / 108 (78-536) | 109 (93-153) / 108 (78-536) | 31 (25-44) / 18 (14-74) |
| analysis | sort click, all tracks | 89 (82-103) / 95 (84-103) | 115 (88-135) / 134 (106-343) | 29 (26-36) / 32 (24-36) |
| analysis | sort click, filtered "a" | 98 (89-124) / 100 (81-136) | 104 (89-158) / 103 (88-158) | 34 (29-45) / 38 (27-43) |

#### chromium 50k: runs kept base 3 / new 3, dropped 0 (run median over all steps above 1.5 x 75 ms, the load spikes)
| Mode | Case | total median base / new | total worst base / new | script median base / new |
|---|---|---:|---:|---:|
| idle | typing, no sort | 57 (56-62) / 56 (55-67) | 127 (117-149) / 110 (98-115) | 12 (11-13) / 11 (11-21) |
| idle | typing, sorted by artist | 66 (64-72) / 63 (62-67) | 98 (90-111) / 85 (84-104) | 17 (17-19) / 16 (15-16) |
| idle | backspace, sorted | 87 (84-91) / 61 (58-63) | 102 (101-104) / 82 (71-101) | 37 (37-37) / 10 (9-13) |
| idle | clear search, sorted | 182 (156-184) / 74 (63-90) | 182 (156-184) / 74 (63-90) | 114 (85-119) / 24 (19-35) |
| idle | sort click, all tracks | 172 (171-172) / 137 (124-156) | 204 (202-206) / 220 (219-223) | 121 (105-125) / 77 (69-107) |
| idle | sort click, filtered "a" | 205 (201-210) / 113 (107-114) | 206 (205-232) / 156 (146-156) | 141 (139-152) / 63 (60-63) |
| analysis | typing, no sort | 74 (69-85) / 68 (60-70) | 230 (227-233) / 141 (126-162) | 22 (21-25) / 13 (11-19) |
| analysis | typing, sorted by artist | 80 (65-82) / 70 (62-74) | 134 (103-155) / 107 (101-133) | 17 (15-17) / 13 (13-15) |
| analysis | backspace, sorted | 97 (86-120) / 91 (89-99) | 134 (116-288) / 107 (101-128) | 43 (34-45) / 35 (32-39) |
| analysis | clear search, sorted | 163 (159-284) / 90 (87-122) | 163 (159-284) / 90 (87-122) | 93 (93-214) / 21 (19-27) |
| analysis | sort click, all tracks | 165 (155-187) / 134 (122-138) | 208 (206-400) / 238 (205-240) | 160 (112-195) / 68 (64-79) |
| analysis | sort click, filtered "a" | 230 (222-315) / 195 (189-196) | 270 (238-426) / 222 (218-237) | 165 (157-236) / 124 (120-128) |

#### webkit 10k: runs kept base 5 / new 4, dropped 1 (run median over all steps above 1.5 x 228 ms, the load spikes)
| Mode | Case | total median base / new | total worst base / new | js median base / new |
|---|---|---:|---:|---:|
| idle | typing, no sort | 191 (176-210) / 186 (176-210) | 445 (395-535) / 451 (408-495) | 43 (38-47) / 40 (34-44) |
| idle | typing, sorted by artist | 200 (186-203) / 204 (168-266) | 446 (324-525) / 409 (298-499) | 46 (42-48) / 49 (43-60) |
| idle | backspace, sorted | 215 (181-232) / 190 (178-228) | 235 (189-258) / 250 (203-282) | 52 (45-58) / 44 (40-48) |
| idle | clear search, sorted | 300 (259-323) / 312 (287-469) | 300 (259-323) / 312 (287-469) | 79 (77-97) / 72 (68-96) |
| idle | sort click, all tracks | 230 (224-285) / 212 (199-221) | 281 (262-431) / 294 (264-413) | n/a |
| idle | sort click, filtered "a" | 241 (200-310) / 241 (186-304) | 291 (202-374) / 288 (192-362) | n/a |
| analysis | typing, no sort | 234 (200-314) / 222 (214-251) | 419 (393-600) / 463 (431-476) | 48 (44-52) / 42 (37-46) |
| analysis | typing, sorted by artist | 251 (224-316) / 245 (214-335) | 384 (361-413) / 391 (358-624) | 45 (42-49) / 43 (38-51) |
| analysis | backspace, sorted | 261 (259-348) / 256 (232-281) | 328 (294-444) / 294 (273-385) | 60 (53-64) / 58 (56-60) |
| analysis | clear search, sorted | 398 (315-577) / 425 (384-547) | 398 (315-577) / 425 (384-547) | 107 (66-130) / 73 (56-123) |
| analysis | sort click, all tracks | 252 (250-311) / 248 (231-290) | 331 (308-420) / 291 (287-348) | n/a |
| analysis | sort click, filtered "a" | 338 (281-374) / 290 (248-334) | 388 (281-480) / 312 (259-355) | n/a |

#### webkit 50k: runs kept base 2 / new 2, dropped 0 (run median over all steps above 1.5 x 260 ms, the load spikes)
| Mode | Case | total median base / new | total worst base / new | js median base / new |
|---|---|---:|---:|---:|
| idle | typing, no sort | 230 (226-236) / 206 (188-225) | 570 (559-581) / 736 (427-1046) | 50 (41-58) / 49 (40-57) |
| idle | typing, sorted by artist | 253 (237-270) / 227 (220-233) | 460 (420-501) / 416 (407-425) | 56 (56-58) / 59 (52-65) |
| idle | backspace, sorted | 218 (204-231) / 195 (190-200) | 310 (237-384) / 266 (262-270) | 66 (66-66) / 46 (44-50) |
| idle | clear search, sorted | 357 (356-358) / 296 (285-307) | 357 (356-358) / 296 (285-307) | 158 (140-175) / 76 (75-76) |
| idle | sort click, all tracks | 288 (266-310) / 251 (249-253) | 336 (310-361) / 386 (332-439) | n/a |
| idle | sort click, filtered "a" | 325 (304-345) / 254 (235-273) | 365 (341-389) / 293 (270-316) | n/a |
| analysis | typing, no sort | 274 (254-295) / 256 (246-266) | 530 (458-602) / 449 (397-501) | 76 (72-80) / 50 (47-52) |
| analysis | typing, sorted by artist | 278 (254-301) / 272 (270-272) | 608 (369-846) / 436 (414-458) | 68 (65-72) / 56 (56-56) |
| analysis | backspace, sorted | 290 (280-298) / 268 (243-294) | 344 (304-384) / 303 (294-312) | 87 (74-99) / 80 (78-82) |
| analysis | clear search, sorted | 587 (545-629) / 460 (391-529) | 587 (545-629) / 460 (391-529) | 145 (136-154) / 74 (66-82) |
| analysis | sort click, all tracks | 353 (353-353) / 334 (312-356) | 418 (367-469) / 390 (373-407) | n/a |
| analysis | sort click, filtered "a" | 455 (388-522) / 388 (341-435) | 473 (396-550) / 410 (364-457) | n/a |

Headline row of the budget table, `typing during analysis: worst keystroke` (`perf-bulk-analysis.spec.ts`, 10k synthetic fixture tracks all unanalysed, typing "smoke track 12" during the fast pass; worst of 14 keystrokes; ms):

| | runs | worst keystroke | median keystroke | palette open worst |
| base | 7 | 174 (118-196) | 100 (80-112) | 165 (129-184) |
| new | 7 | 151 (120-676) | 88 (85-117) | 166 (130-408) |

Seven runs each (three pairs and four more pairs later), interleaved, none dropped; the base runs include one at load 37 (idle scroll p95 214 ms) and the 676 ms new run (378 ms long task in the typing window, idle scroll p95 25 ms, did not recur in the other six new runs: 120-175 ms).

### Read with care

- The 100 ms target for the worst keystroke is not met by script changes alone and was not met here: a keystroke without a sort is 55 ms at the median in Chromium at both sizes, 80% of it native, and the worst keystroke of a run (new: 99 ms idle and 120 ms during analysis at 10k, 110 and 141 ms at 50k; base: 118, 133, 127 and 230 ms) is one that changes many visible rows or crosses the viewport size (rows created or destroyed). During analysis at 50k the worst keystroke improved most (230 to 141 ms; script of it 163 to 65 ms), because the bumps no longer rescan the library. The gain is where a sort is active and the result set changes: 50k clear search while sorted 182 ms to 74 ms, backspace while sorted 87 to 61 ms, sort click on a filtered list 205 to 113 ms, and every bump while sorted by BPM (analysis running) from 41 ms (10k) and 208 ms (50k) of script to 1 and 3 ms.
- First sort of a column is still a full sort: 50k by artist 125 ms and by title 190 ms of script at store level (reading 50k values, a sort of 50k distinct strings); it moved from 406 and 514 ms but a click stays at 130-220 ms total at 50k. The second direction of the same column is a new sort (30-52 ms at 50k, not derived from the first one).
- WebKit headless: `total` is 170-300 ms for every step including a bare keystroke (the compositor idle gap of R29), so differences of 10-30 ms between base and new do not show in it; the script side (`js`) moves the same way as Chromium where it matters (50k sorted clear 158 to 76 ms) and is within noise at 10k. One WebKit 10k run hit a load spike of 40 and was dropped by the rule above; three more pairs were run afterwards (5 base and 4 new kept).
- The Chromium outliers of single runs (up to 1.4 s for one keystroke) coincide with load average 39-42 on the box (other workers); they were dropped from the tables by the run-median rule and the count is printed. One of the three first `perf-bulk-analysis` new runs had a worst keystroke of 676 ms and a 378 ms long task in the typing window, with the rest of that run (idle scroll p95 25 ms) normal; four more pairs were run (table above).

### Not done

- The first keystroke after a clear still scans every search text (10 ms at 50k); a joined text with `indexOf` would need rebuilding at every bump while analysis runs, which costs about what it saves.
- `TrackTable` builds `new Set(filteredTracks)` per sorted evaluation (about 5 ms at 50k) for the keep-position-while-scrolling logic; unchanged.
- Layout, paint and the compositor frame (the 80%) are markup and CSS territory and were not touched.

## R114 bundle (2026-11-04, `r114-bundle`)

Branch `r114-bundle` from `ov-1028` (`eb9d84f`, r109 and r111 included). Linux `dev` box, headless Chromium 1217 and Playwright WebKit, `perf-startup.spec.ts` with the 10k fixture's real `load_state` payload (`~/.cache/r95/state10k.json`, backend simulated as 118 ms), load average 12-17 (other workers). Base and new were built from separate copies of the tree and alternated (base, new, base, new ...), 3-4 pairs of 3 runs each; every number is the median of the pair medians, range in brackets. Result: **first real row 25-90 ms earlier at 10k (4-10%), no visible change; the 300 / 330 ms targets are far away, the rest of the start is the first-screen mount and the harness's own wait for the page main thread.**

### Where the start goes (what was measured)

Chromium trace of a warm reload at 10k (`FF_PERF_STARTUP_TRACE`, minified build), page main thread, three traced pairs:

| Phase | base | new |
|---|---:|---:|
| entry script requested / fetched | 80-91 / 139-151 ms (32 / 89 in a fourth run) | 36-93 / 99-147 ms |
| background parse of the entry script (V8 streaming) | 44-59 ms | 39-48 ms |
| module evaluation (imports, `mount(App)`, first effects) | 156-194 ms | 131-170 ms |
| module evaluation ends | 303-334 ms | 267-302 ms |
| `load_state` sent (R109) | 96-129 ms | 106-149 ms |
| `load_state` answered (harness: timer, fetch, parse, then the main thread must be free) | 506-548 ms | 528-562 ms |
| first real row | 670-798 ms | 687-705 ms |

Tracing makes single runs noisy (+-60 ms); the untraced pairs below are the evidence. In the CPU profile of the unminified build, module top-level code outside `mount` is about 13 ms (the stores, shortcut registry, persisters: nothing worth deferring); `mount(App)` is 125 ms: `TrackTable` skeleton rows 54, `Sidebar` rows 29, `Player` 12, `Settings` 9, `GlobalHeader` 8, `AppStatusBar` 6, plus a forced layout of 32 ms from the size bindings. So the bundle matters through the mount of components that are not on the first screen and through parse, not through module evaluation. The skeleton rows, sidebar rows and the first layout are the first screen and stay (identity requirement); they are the rest of the main-thread time before the answer.

### What changed (`app/src`, `app/vite.config.ts`; no markup, CSS or behaviour change)

- **`lib/lazy.svelte.ts`, `lib/LazyMount.svelte`, `lib/afterFirstScreen.ts`, `lib/lazy-views.ts`** (new): `lazyView(() => import(...))` loads a view's chunk on first need (`LazyMount` mounts it; `needed` for views that are always mounted but closed) or, at the latest, one at a time in idle slots after the first screen is shown (`App` marks it when `initLibrary()` has resolved, two animation frames later; fallback 4 s after mount). A loaded view renders synchronously, so a preloaded dialog opens in the same task as before. A failed import is retried on the next request (`lazy-view.test.ts`); a failed preload is silent, the on-demand call reports it.
- Deferred this way: the dialogs `AppDialogs` (Settings with its pages, Rekordbox import and restore, command palette, stick sync, library health, import duplicates, file-blocked tip, stick drop tip; opened by `ui.settingsOpen` / `ui.paletteOpen` immediately), the inspector (`Detail` with `DetailPane`, `CueEditor`), `StickView`, `ExpandedPlayer`, `LibraryActivityMenu`, `TrackKeyPopup` and `TrackBpmPopup`. Their stores stay in the main chunk (shortcuts and context menus use them).
- **Tauri window API on demand** (`services/webview-window.ts`): `getCurrentWebviewWindow` and `PhysicalSize` come in with `import()` from `installMacWindowChrome` and the drag-drop registration, both still started in `onMount` (traffic-light nudge, drop target); 95 kB less to parse in the entry script. Not measurable alone.
- **`vite.config.ts`**: `build.target: 'safari16'` (`minimumSystemVersion` 13.4 in `tauri.conf.json` is Safari 16.5; the default `modules` target lowers class fields with helper calls), design-system `.svelte` components are side-effect free for tree shaking (their `$.delegate(...)` calls kept every unused component of the `$ds` barrel in the entry chunk, `DetailPane` and `CommandPalette` among them), `modulePreload.resolveDependencies: () => []` (Vite's preload links for lazy chunks made WebKit warn "preloaded but not used", a console warning the smoke counts as a failure), and **`cssCodeSplit: false`**.
- **Why one stylesheet**: with per-chunk CSS, WebKit (Playwright) stalled one frame for 150-220 ms every time a lazy chunk's `<link rel=stylesheet>` was added (`longestFrame` 183-224 ms against 19-27 ms in base, `frameInteractive` +430-630 ms), seven times in the first 2 s. Chromium did not show it. Putting all CSS back in one file removed the stall (longestFrame 21-23 ms) at the price of the CSS saving (139.7 to 133.8 kB). Not checked on the Mac.
- `perf-startup.spec.ts`: `FF_PERF_STARTUP_TRACE=<prefix>` (Chromium trace of the last warm reload), `FF_PERF_STARTUP_SNAPSHOT=<prefix>` (writes the page HTML and a screenshot 4 s after the first row, last run).

### Bundle sizes (`npm run build`, minified)

| | base | new |
|---|---:|---:|
| entry JS (loaded before the first row) | 512.50 kB (168.50 gzip) | 370.86 kB (127.21 gzip), -27.6% |
| CSS (one file, render-blocking) | 139.66 kB (20.90 gzip) | 133.77 kB (20.15 gzip) |
| deferred JS, 12 chunks | 0 | 134.4 kB: `AppDialogs` 58.6, `Detail` 22.3, `webviewWindow` 15.7, `StickView` 9.8, `ExpandedPlayer` 9.7, `LibraryActivityMenu` 5.3, `TrackKeyPopup` 2.8, `TrackBpmPopup` 2.6, `alt-key` 2.6, `Popover` 2.3, `dpi` 2.1, `Switch` 0.7 |
| all JS | 512.5 kB | 505.3 kB |

What is left in the entry chunk (rendered, unminified): `waveform-display.js` 59 kB (the strip's display, first screen), the library store 47, `tauri.svelte.ts` 37 (17 of it browser-only mocks, imported at its top and used by `?mock=` and the browser fallback; moving them needs a restructure of that file, not done), `TrackTable` 31, the Svelte runtime about 150, `Sidebar` 22, `OrganizePanel` 17 with `OrganizePicker` and `StickBoard` (the panel is persisted expanded, so its picker is first screen for those users; left alone), context menus 12 + 11.

### Before / after, 10k, Chromium (3 pairs of 3 runs, final build, load 13-17; ms, median of pair medians, base / new; pair diff = median of new minus base in the same pair)

| Case | load_state answered | first real row (range base / new) | pair diff | interactive | pair diff | frameInteractive | longest task |
|---|---:|---:|---:|---:|---:|---:|---:|
| first launch | 568 / 552 | 742 (719-781) / 718 (655-720) | -61, new faster 3/3 | 826 / 802 | -69 | 904 / 887 | 277 / 253 |
| cold | 573 / 558 | 706 (680-758) / 700 (696-738) | +20, 1/3 | 780 / 873 | +36 | 863 / 873 | 254 / 248 |
| warm | 509 / 482 | 667 (626-700) / 604 (576-610) | -91, 3/3 | 718 / 668 | -88 | 783 / 735 | 205 / 196 |

Earlier variants of the same change, 4 pairs each: lazy views only (CSS split): first row -91 / -31 / -12 ms (first launch / cold / warm), interactive -81 / -17 / -53; with the window API deferred, `safari16` and the DS side-effect change (CSS still split): first row -65 / -77 / -26, interactive -60 / -20 / -24 (new faster in 3 or 4 of 4 pairs in every row). The cold row is the weakest and swings with the box; in the final set it is a wash (+20). Main-thread CPU from navigation to 2.5 s after the first row (`cpuMs`, first launch and cold) was 16-62 ms higher in the CSS-split variants and is -3 / +11 in the final one: the work moved after the first row (chunk loads, mounting the closed dialogs, each in its own idle task) is what it costs.

### WebKit (Playwright, 3 pairs of 3 runs, final build, load 12-16)

| Case | first real row (= interactive here) | pair diff | frameInteractive | longestFrame |
|---|---:|---:|---:|---:|
| first launch | 1,191 / 1,148 | -43 (2/3) | 1,490 / 1,384 | 18 / 21 |
| cold | 1,217 / 1,092 | -126 (3/3) | 1,478 / 1,356 | 22 / 23 |
| warm | 739 / 713 | -50 (2/3) | 940 / 937 | 19 / 21 |

`longestTask` and `tbt` are 0 in WebKit (no long-task API).

### Identity

`FF_PERF_STARTUP_SNAPSHOT` on the 10k payload (warm reload, body HTML and screenshot 4 s after the first row, so the lazy chunks have loaded and mounted), base against new, both browsers:

- Chromium: screenshot **byte-identical** (sha256 `27ffd42c...`, 149,472 bytes); body HTML identical (`68912381...`, 3,597 lines) after removing comment anchors and the `svelte-<hash>` class suffixes (they derive from the checkout path; base and new were built in different folders).
- WebKit: screenshot **byte-identical** (`6e561349...`, 571,823 bytes); body HTML identical (`4cfc7d19...`).
- Deferred features: the whole smoke suite opens them (Settings and its pages, palette, Rekordbox import, health dialog, duplicates, stick sync, inspector, expanded player, popovers, activity menu).
- A WebKit smoke run failed once (`files.spec.ts:131`) with `console.error: preload failed: TypeError: Importing a module script failed` (a preload aborted by the test's page navigation; the first version logged preload failures). Preloads are silent now and a failed import is retried when the view is first needed (and reported then). An earlier WebKit failure (`file-drop.spec.ts:77`) was the "preloaded but not used" warning fixed by the preload setting above.

### Looked at, not changed

- `loadEveryFontSubsetNow()` asks for all 11 Geist faces at boot (about 150 kB, 4 ms main thread in the trace); narrowing it could change visible text, not touched.
- Skeleton rows (`TrackTable`, 54 ms), sidebar rows (29 ms) and the first forced layout (32 ms) are what is left before the answer; cutting them changes the first screen (R15 looked at the staged reveal) and `TrackTable` is in r113's lane.
- Moving the browser mocks out of `tauri.svelte.ts` (17 kB rendered, about 7 kB minified) and lazy `OrganizePicker`/`StickBoard` (12 kB rendered): small, and the second changes what users with the panel expanded see at boot.
- Not measured: the Mac (no GUI here). JavaScriptCore parses the whole script on the page thread, so the 140 kB cut from the entry script should count more there than in V8's background streaming; the next Mac night's `page_start` and first-row columns will show it.
## R115 first sort (2026-11-04, `r115-first-sort`)

Branch `r115-first-sort` from `ov-1028` after the r113 merge (`1d9fdb1`). Linux `dev` box, load average 13-22 from other workers during all runs, headless Chromium and Playwright WebKit. Base = `ov-1028` in a second worktree, same spec files, runs interleaved (base, new, base, new ...). Real payloads from `bench_startup dump` (10k and 50k, the r95 dumps). `total` is key or pointer down to the second animation frame after it; `script` is the Chromium `ScriptDuration` delta of the step. In WebKit the `js` number is not meaningful for header clicks (pointerdown), so only `total` is compared there.

### Where the first sort went

R113 left the first sort of a column at 125 ms (artist) and 190 ms (title) of script at 50k. In the browser it split into three parts: reading the sort value of every row (10-15 ms), one `Array.sort` over the distinct strings (5 ms for 7.7k artists, 50 ms for 36k titles) and a numeric sort of `rank * n + position` keys (11 ms), plus building the Map from value to rank. The second direction of a column was a new sort (30-55 ms at 50k).

### Changed (no markup, no CSS, same rows, same order)

- `app/src/lib/sortOrder.ts`: `sortedOrder` now builds value classes (equal values share one class, 0 for the lowest, `count` for no value; one Map pass, distinct values sorted, ranks looked up per distinct value) and turns them into the order with a counting sort (`orderFromClasses`), which gives equal values in position order and "no value last" in both directions without a second sort. The classes serve both directions, so the second direction costs about 3 ms at 50k instead of 30-55 ms. Lists that are not all strings or all numbers (or hold a NaN) keep the old comparator sort. `valueClassesSteps` is the same code as a generator that yields every `chunk` values (and sorts distinct strings as sorted runs of `chunk` merged), so it can run in short slices; `valueClasses` runs it in one piece (`chunk` Infinity, one native sort, as before).
- `app/src/stores/table-columns.svelte.ts`: the library sort cache is now one entry per column (values, classes, orders per direction, the list they were read for) instead of one cache per list generation. An entry for an older list is brought up to date from the bump log (`libraryPositionsChangedSince`): changed rows are read again and moved in each cached order, as in R113; any other list change drops it. Sort values are memoized by raw text per read except for the title and `#` columns.
- Warm-up: `startLibrarySortWarmup(keys)` reads and sorts the visible library columns (not status, fav, wave, cover, `#`) in slices of about 6 ms (2048 rows per read chunk, 4096 strings per sort chunk), starting 1 s after `loadState` or an import (`onTrackListLoaded`, new in `library.svelte.ts`) with `requestIdleCallback` (12 ms gaps with `setTimeout` where it does not exist). A finished column is kept only if nothing built it meanwhile and the list was at most bumped since. The first click on a header finds the order ready; a click before that behaves as before, only faster.
- `app/src/stores/library.svelte.ts`: while the search text is built (already in idle slices since R95) a bit set of the letters a-z and digits 0-9 of each row is kept (`haystackLetters`, `haystackDigits`, refreshed with the row on a bump). A query of one letter or digit on the library list is answered from those sets (exact: the search text holds the character if and only if its bit is set) instead of `includes` over 50k texts; every other query takes the old path.
- Tests: `sortOrder.test.ts` (non-ASCII, astral and combining characters, ties, chunk sizes 1 to 1000 of the steps against the one-piece result, 3000 distinct strings across chunk borders, mixed lists and NaN fall back); `search-sort-fuzz.test.ts` now sorts 7 columns (title, artist, album, genre, bpm, key, time) with non-ASCII words, interleaves warm-ups with random small chunk sizes (several at once, started before bumps and reloads) and has a test of every letter, digit and some other single characters, upper case, and after bumps (deliberate breaks of the bump refresh, of the chunked read and of the bit sets make it fail, checked); `search-path-real.test.ts` sorts every sortable column in both directions and with a filter, and hashes the ids.
- `app/smoke/perf-warmup.spec.ts` (new, Chromium, `FF_PERF_STATE=<dump>`): script time per second and long tasks in the 10 seconds after the first rows.

### Identity

- Store level, real payload through `loadState`, hash of every id of every result list in order: 94 of 94 equal between base and new at 10k and at 50k (90 scripted steps: typing, backspacing, clearing, sorts with and without filters, every column asc and desc and desc with a filter, plus 4 bump scenarios), `FF_PERF_STATE=... npx vitest run src/stores/search-path-real.test.ts` in both trees (the same file).
- Warm against cold: the fuzz compares sorted lists of 900 random steps (with warm-ups, bumps and reloads in between) against a plain reference sort; the real-payload hashes above were taken with the new tree's cold path, the browser runs below with the warmed path.
- UI level: the rows on screen (every cell, 3 scroll positions per checkpoint) after each step of the scenario, hashed in the page: 63 of 63 idle steps identical across all runs of base and new, in Chromium 10k and 50k and WebKit 10k and 50k.

### Before / after (browser, interleaved)

Chromium 50k, 5 pairs, none dropped (a run is dropped when its median over all steps exceeds 1.5 x the pooled median). ms, median with (min-max):

| Step | total base | total new | script base | script new |
|---|---:|---:|---:|---:|
| first sort artist asc | 186 (154-441) | 89 (67-100) | 108 (98-373) | 29 (27-47) |
| artist desc | 119 (105-139) | 85 (70-103) | 57 (45-83) | 26 (20-37) |
| first sort title asc | 228 (205-257) | 87 (70-118) | 161 (149-204) | 30 (22-37) |
| first sort bpm asc | 121 (106-135) | 87 (75-90) | 64 (58-73) | 23 (22-32) |
| first sort key asc | 145 (122-230) | 72 (55-88) | 79 (66-137) | 24 (20-28) |
| title desc, filtered "a" | 157 (141-204) | 73 (64-103) | 106 (96-131) | 20 (18-24) |
| first keystroke "g" | 139 (128-161) | 99 (94-117) | 53 (39-66) | 37 (34-39) |
| first keystroke "h" after a clear | 68 (66-99) | 67 (54-76) | 21 (18-32) | 14 (11-20) |

Chromium 10k, 5 pairs, none dropped:

| Step | total base | total new | script base | script new |
|---|---:|---:|---:|---:|
| first sort artist asc | 101 (75-108) | 73 (58-90) | 47 (34-71) | 21 (18-24) |
| artist desc | 81 (66-98) | 69 (41-74) | 24 (20-39) | 14 (11-18) |
| first sort title asc | 101 (87-108) | 84 (72-106) | 49 (38-57) | 21 (15-30) |
| first sort bpm asc | 87 (74-115) | 80 (72-88) | 26 (20-42) | 19 (16-30) |
| first sort key asc | 88 (73-99) | 84 (61-88) | 26 (24-47) | 15 (13-23) |
| first keystroke "g" | 109 (84-114) | 91 (77-125) | 32 (30-40) | 29 (28-40) |
| first keystroke "h" after a clear | 59 (41-69) | 70 (65-88) | 15 (14-16) | 17 (15-32) |

WebKit (`total` only), 3 pairs each, none dropped:

| Step | 50k base | 50k new | 10k base | 10k new |
|---|---:|---:|---:|---:|
| first sort artist asc | 374 (364-396) | 215 (208-305) | 262 (180-294) | 217 (210-219) |
| artist desc | 297 (280-329) | 306 (302-1616) | 350 (277-398) | 208 (184-222) |
| first sort title asc | 394 (348-634) | 241 (200-914) | 250 (182-281) | 189 (182-228) |
| first sort bpm asc | 260 (228-279) | 268 (204-546) | 246 (218-269) | 192 (181-251) |
| first sort key asc | 316 (257-381) | 238 (182-366) | 237 (217-238) | 180 (179-198) |
| first keystroke "g" | 667 (618-860) | 600 (504-1157) | 551 (546-775) | 536 (497-606) |
| first keystroke "h" after a clear | 341 (287-551) | 361 (275-430) | 307 (262-568) | 277 (267-284) |

Store level (Node, one thread, `search-path-real.test.ts`, median of 3): the sort itself, not the value read, fell: `sortedOrder` of 50k titles 89 to 73 ms, artists 30 to 16 ms, albums 24 to 9 ms, bpm 16 to 5 ms; the second direction of a column 30 to 3 ms. The value read costs 3-5 times more under Vitest than in the browser, so the Node numbers of first sorts (114 vs 132 ms artist) are not comparable to the browser ones.

Cost of the warm-up (`perf-warmup.spec.ts`, Chromium, 4 pairs, script time in the 10 seconds after the first rows): 50k 354 ms (316-362) base, 788 ms (707-807) new, 10k 111 ms (67-128) base, 195 ms (168-216) new; no long task (50 ms) in any run. The added time is spent in slices of at most about 9 ms in the first 4 seconds after the 1 s delay: about 250 ms sorting and reading seven columns, about 100 ms noting the letters and digits of the search text.

### Read with care

- The first sort is fast only when the warm-up had finished (about 4-5 s after the first rows at 50k on this loaded box); a click before that costs what it did before minus the faster sort (50k artist about 115 ms of script). Columns shown later than at load are not warmed.
- The remaining 20-30 ms of script of a click at 50k is the Svelte update of the visible rows and the `TrackTable` bookkeeping; the click-to-painted time is 70-90 ms, mostly layout and paint.
- The first keystroke still carries 33-37 ms of script at 50k: 10 ms were the scan (the letter sets remove it, about 9 ms on later single letters after a clear), the rest is the Svelte update of the result and first-use code.
- WebKit headless adds a compositor gap of 170-300 ms to every step (R29), so differences there are about 40-150 ms in the first sorts and within noise for the keystrokes; one new run at 50k had a spike (1616 and 914 ms) that the median hides.
- Idle script time per column grows with the row count; at 10k the warm-up is about 85 ms in total.

**R69 (`r69-stick-busy`):** `write_usb` and `wipe_usb` are now `#[tauri::command(async)]` too. A per-volume busy guard (`pioneer-test-ui/src/stick_busy.rs`, managed state `BusySticks`, RAII `BusyGuard` on the canonical volume path) is taken by `write_usb`, `sync_usb_many` (all targets up front, all or nothing), `wipe_usb`, `eject_volume` and `stick_add/remove_tracks/playlists`. A second operation on the same stick returns `ErrorKind::StickBusy` ("This stick is busy. Try again when the current sync finishes."). The r67 source test covers `write_usb` and `wipe_usb`.

## R108 throughput (2026-11-04, `r108-throughput`)

Branch `r108-throughput` from `r105-snap` (`903b702`: chunk 500 default, fit fix). Question: what split of parallel tracks, threads per track and beat-model threads gives the most tracks per minute at chunk 500, with identical output, on this box (Xeon Gold 6130, 16 vCPU, one thread per core, 64 GB) and on an 8 performance-core Mac-like budget (`taskset -c 0-7`, plus `FOURFOUR_ANALYSIS_MEMORY_MB` for the memory of the machine). rten on the CPU only: the Mac's CoreML path (2 sessions, chunk 1500) cannot be simulated here.

**Bench set.** The 24 full-length masters of the 10k fixture (`~/.cache/ff-fixture/10k/long.txt`: 5 formats, 304-415 s, 8,816 s of audio); a 12-track half of it for the 8-core runs. Release builds, `nice -n 5`, interleaved configs (rotated order per repetition). The box carried load 15-45 from other workers the whole time.

### How the pipeline uses threads today

- The beat model runs on one pool of **chunk threads shared by all tracks** (`BeatEngine::share_chunk_threads`, `chunk_slots`); each chunk runs on a single-thread rten pool (the setup whose logits match the reference bit for bit), so "model intra-op threads" is always 1 and the parallelism is across chunks. A track queues its chunks, the pool serves tracks in arrival order, and a track whose chunks are taken leaves its threads to the next one.
- **Worker threads** (`plan_pool`: min(threads, memory / 900 MB, tracks left)) each run one track end to end: decode, then in parallel the waveform plus key lane and the beat prefix (resample to 22.05 kHz, mel on a pool of `threads_per_track`), then wait for their chunks, then fit. `threads_per_track` = threads / workers (1 on this box) only widens the mel pool, the key lanes and the fit.
- **Decode of track N+1 during the model on N already happens**: with W workers, W-1 other tracks are in their prefix or their chunk phase while one waits. There is no separate prefetch to add; what matters is whether W is large enough that the chunk queue never runs empty. In the app the model stage is further capped at `max(threads/3, 3)` model jobs (4 on 14 threads); the other in-flight slots take fast passes.

### Measuring on a shared box

Wall-clock tracks/min on this box is biased by the other work: the scheduler gives a process cores in proportion to its runnable threads, so a config with more threads gets a bigger share and looks faster (8 cores, W=8 against W=4: 9.2 against 7.3 tracks/min measured, with the same CPU per track and 1% idle chunk threads in both). Every table therefore has, next to the measured number, a **quiet** estimate: the batch's CPU seconds plus the seconds the chunk threads waited for a chunk (`BeatStatus::chunk_idle_s`, new), spread over the cores the run may use (at most its thread count). It is what a machine with nothing else on it would take if the CPU time stays what it was. `benchmark/scripts/r108-table.py` prints both.

### Sweep, this box (analyze-json, 24 tracks, 3 reps, medians (min-max))

| config (threads x workers) | measured trk/min | quiet trk/min | CPU / track | peak RSS | first result | chunk threads idle |
|---|---:|---:|---:|---:|---:|---:|
| 16 x 14 (default plan) | 10.70 (10.03-11.56) | 19.6 | 48.2 s | 2,533 MB | 15.9 s | 1% |
| 16 x 8 | 10.20 (10.02-11.20) | 19.3 | 49.2 s | 2,152 MB | 14.4 s | 1% |
| 16 x 4 | 10.33 (10.02-10.90) | 19.4 | 49.0 s | 2,081 MB | 11.1 s | 1% |
| 16 x 2 | 8.93 (8.62-9.58) | 14.7 | 50.6 s | 2,077 MB | 11.5 s | 14% |
| 14 x 14 (the app's cores - 2) | 10.35 (9.63-10.99) | 17.1 | 48.6 s | 2,273 MB | 17.1 s | 1% |

### Sweep, 8 cores (`taskset -c 0-7`, 12 tracks, 2 reps)

| config | measured trk/min | quiet trk/min | CPU / track | peak RSS | first result | chunk threads idle |
|---|---:|---:|---:|---:|---:|---:|
| 8 x 8 | 9.16 (9.01-9.32) | 10.0 | 47.2 s | 1,284 MB | 13.0 s | 1% |
| 8 x 4 | 7.26 (6.61-7.91) | 9.9 | 48.0 s | 1,160 MB | 12.6 s | 1% |
| 8 x 2 | 7.17 (7.01-7.34) | 9.9 | 47.4 s | 1,147 MB | 13.0 s | 1% |
| 6 x 6 (the app's cores - 2) | 6.43 (6.37-6.48) | 7.6 | 47.3 s | 1,074 MB | 14.1 s | 1% |

### What the sweep says

- **The beat model is the whole cost and the default already keeps every core on it.** Chunk threads wait 1% of the time at every worker count from the default down to 4 (16 threads) and to 2 (8 threads); CPU per track is 47-49 s whatever the split (0.13 CPU-s per audio-s: 0.118 beat model, 0.013 the rest, as R104). There is no throughput left in the split on a free machine; oversubscription costs nothing measurable (sys time 2% of CPU).
- **Workers: 4 are enough for 16 chunk threads, 2 for 8.** With 2 workers and 16 chunk threads the pool starves 14% of the time (the prefix of a track, about 2.8 CPU-s of decode, resample and mel on one thread, is longer than the chunk phase of the other: 37 CPU-s over 16 threads) and the quiet rate falls 25%. Fewer workers than the default save memory and first-result time at equal rate: 4 instead of 14 is -18% peak RSS and -30% first result. The app already runs only `max(threads/3, 3)` model jobs, so its model stage is in that regime; no change.
- **Threads per track and intra-op threads do not pay.** Giving a track more threads only widens the mel pool, the key lanes and the fit (about 10% of the CPU); running one chunk on several rten threads would trade the efficiency of independent chunks for a lower per-track latency and changes how many chunks hold memory at once. The chunk stays on one thread (it is also what keeps the logits equal to the reference), so this was not swept.
- **The one big lever is the number of cores the app leaves to the UI.** `UI_CORES = 2` takes 12.5% of the quiet rate on 16 cores (17.1 against 19.6) and **24% on 8 cores (7.6 against 10.0)**. Left unchanged: it is a UI-smoothness decision (`analysis_backend::analysis_threads_for`), and the effect on the UI while analysis runs cannot be judged here. If the Mac's QoS (analysis below the UI) is enough, `cores - 1` is worth a Mac test.

### The app scheduler (bench_analysis, `--features dev-bins`)

| 16 cores, 24 tracks, app defaults (14 threads, 14 in flight, 4 model jobs), 2 reps | measured trk/min | CPU / track | peak RSS | first result | chunk threads idle |
|---|---:|---:|---:|---:|---:|
| staged (fast pass, then grid pass; the app) | 8.97 (8.16-9.79) | 52.8 s | 1,936 MB | 16.7 s | 1% |
| `--unstaged` (one whole pass per track) | 8.27 (8.20-8.34) | 52.0 s | 1,380 MB | 14.4 s | 1% |

At full width the staging costs 1.5% CPU (the second decode and waveform of the grid pass) and nothing in throughput; the chunk threads are busy in both. Not touched.

### Change: the memory plan was sized for 1500-frame chunks

`chunk_slots` plans `CHUNK_ACTIVATION_MB = 400` per chunk thread inside a quarter of the machine's memory. That is the 1500-frame chunk (R57); the default is now 500 frames. Peak RSS against the chunk threads (same worker count 4, other tracks the same size: 2,081 MB at 16 threads, 1,160 MB at 8) is **about 115 MB per chunk thread at chunk 500**. On a machine whose quarter-of-memory budget is small, the 400 MB estimate gives fewer threads than there are cores: an 8 GB machine (budget 2 GB) ran **3** chunk threads of the 6 it was given.

`chunk_activation_mb(frames)` now takes 400 MB at 1500 frames (unchanged, so CoreML's chunk size plans as before) and 120 MB at 500, a straight line between; `chunk_slots` reads the chunk size with the new `bt::chunk_frames()`. Tests: `chunk_activation_follows_the_chunk_length`, `short_chunks_leave_room_for_every_thread_on_a_small_machine` (6 threads on an 8 GB machine: 3 slots at 1500 frames, 6 at 500); the old slot tests keep their 400 MB through `chunk_slots_with_ceiling`.

| 8 cores, `FOURFOUR_ANALYSIS_MEMORY_MB=8192` (4 GB available), bench_analysis app defaults (6 threads, 2 workers), 12 tracks, 2 reps interleaved | measured trk/min | CPU / track | peak RSS | first result |
|---|---:|---:|---:|---:|
| base (3 chunk threads) | 3.17 (2.34-4.01) | 56.6 s | 555 MB | 34.8 s |
| new (6 chunk threads) | 6.56 (6.06-7.06) | 53.3 s | 748 MB | 17.1 s |

**2.1x on an 8 GB machine** (3 chunk threads can use 3 cores, 6 use 6; the measured ratio sits at that bound). Nothing changes where memory is not the limit: this box (16 and 8 cores, 26-64 GB) plans the same threads before and after, and a 16 GB machine already had 8 slots. On the same simulated machine with the new binary, 2 reps: 2 workers (the memory-planned count) 7.19 (7.05-7.32) trk/min against 6.92 (6.81-7.03) with 4 workers, so no worker floor is needed; `--unstaged` 7.53 (7.23-7.82) with chunk threads idle 1% against 8% staged (one fast pass and one model job share the 2 in-flight slots), the price of waveforms first on a 2-worker machine, not changed.

### Knobs and instrumentation added (bench only, defaults unchanged)

- `FOURFOUR_ANALYSIS_WORKERS` (`PoolOptions::default`, so the app path and `bench_analysis` read it too) and `analyze-json --workers`: caps the tracks in flight.
- `BeatStatus::chunk_idle_s`: seconds the beat-model threads, added up, waited for a chunk; printed by `analyze-json` (stderr) and `bench_analysis` (`BEAT` line).
- `benchmark/scripts/r108-run.py` (one batch: wall, CPU, peak RSS, first result, per-track finish times, sha256 of the output), `r108-sweep.sh` (interleaved configs, taskset, simulated memory, analyze-json or bench_analysis), `r108-table.py` (medians, spread, quiet rate).

### Proof

`analyze-json` over the 24 tracks, sha256 of stdout (beat grid, bpm, key, waveforms, bar phase): base `r105-snap` and new default both `523dc97f64be81ec8679e613f09ed968791b59d7bdaf4f0cef6ccea4647322cc`; base and new on 8 cores with 8 GB simulated (new: 2 workers, 3 threads per track, 6 chunk threads; base: 3) the same hash each; in those two runs the 24 tracks took 346 s (base) and 190 s (new), one run each, load 20-30. `cargo test --workspace`: 26 `test result: ok` lines, 546 passed, 0 failed.

### Not done, and why

- Mac CoreML path (2 sessions, chunk 1500, ANE/GPU): not simulable on Linux; the sweeps above are the rten CPU path. There the CPU lanes (decode, resample, mel, key, waveform) are what parallel tracks buy, and W has to cover those, not the chunk queue.
- Wall-clock differences between worker counts above 4: none exist on a free machine; the loaded box cannot show them without the share bias described above.
- A separate decode prefetch lane: redundant with 3-4 workers (idle 1%); with 2 workers and 16 threads it would recover part of the 14% idle, but that needs a machine that is both wide and short of memory, and the chunk estimate above removes the shortage that would produce it.
## R118 row layers (2026-11-04, `r118-row-layers`)

Branch `r118-row-layers` from `ov-1028` (`0e01eba`, r112, r113, r114, r115 included). Linux `dev` box, headless Chromium 1217 and Playwright WebKit, 10k fixture payload (`~/.cache/r95/state10k.json`, two thirds of the rows given synthetic 3-band data so rows show real previews, one third left as placeholders), 1600x1000, box load 13-23 (other workers). Result: **the per-frame cost is real and sits in the row canvases, but every way of removing it needs the fenced `Waveform.svelte` (the row preview is that file's `mini` variant, drawn by `TrackRow.svelte`), and the only one that works (rows as `<img>`) is not pixel-identical in Chromium, retains memory the app cannot bound, and is slower on scroll in WebKit. Nothing landed in `app/src`; the proposal and its numbers are below.**

### Where the Layerize time comes from

Layer tree of the expanded, playing player over the 10k table (CDP `LayerTree`, `FF_PERF_R118=1 ... -g 'layer tree'`): 84 layers, 77 drawing: **37 `Canvas` layers (one per row preview, all of them composited whatever the canvas size or `willReadFrequently`) and 37 `Overlap` layers** (the hidden 1 px `.ff-trow__wave-cursor`, `opacity: 0`, paints above its row's canvas, so the row's later content cannot merge back below the canvas layer), 3 others. The table pool keeps its canvases: a 300-frame fling over 25,000 px adds and removes 0 canvases and 0 rows (42 rows, 42 canvases before and after; `FF_PERF_R118=pool`).

Layerize per frame (expanded player playing, 1.5 s traces) by what is hidden, same page, one trace each: base 8.1, row canvases hidden 3.2-3.8, canvases and cursor hidden 2.7, whole table hidden 1.5 (the rest of the page). With no canvases left, the 25 elements of a row still cost about 2 ms: the text spans are about 1.1 ms of it (hide-text 3.5 to 2.4), cells, cover, fav and rating together the rest.

### What was tried in app code (no change, nothing landed)

| Try (CSS injected into the page, expanded player playing, Layerize ms per frame) | Result |
|---|---|
| `will-change: transform` on `.ff-table__scroll` or `.ff-table__body`, `isolation: isolate; contain: paint` on the scroller, `contain: layout paint style` on the body | 6.5-7.4 against 7.0-7.7 base (v6, one trace each): no change |
| `.ff-table__row { contain: none }` | inconclusive: 7.0 (6.6-10.0) against 8.7 (8.1-9.9), 3 traces against 4, inside the spread; not pursued (the row's `contain: layout` is what keeps a text change from relaying out the list) |
| wave cursor `display: none` unless the wave cell is hovered (so the 37 `Overlap` layers go, 84 to 47 layers; written as app CSS in `TrackTable.svelte`) | **worse**: Layerize 10.3 (9.5-11.0) against 7.4 (7.0-8.0), Commit unchanged (2.1 against 2.3); and not byte-identical (fav checkbox corner pixels, see below) |
| `getContext('2d', { willReadFrequently: true })` on the row canvas | still 37 `Canvas` layers |
| `contain`, `content-visibility: auto` on rows, hiding the canvases of overscan rows | already tried in `performance-2026-10.md` (no Layerize change; the overscan protects against blank rows on a compositor-thread fling); not repeated |

### The change that works (proposal, fenced: not landed)

Draw the `mini` preview into an offscreen canvas as before, turn it into a PNG data URL and show it in an `<img class="ff-waveform ff-waveform--mini">`; keep the URL in an 800-entry LRU keyed like the existing bitmap cache (key includes data, size, dpr and palette version; the placeholder bars get their own key); `TrackRow.waveRect` asks for `.ff-waveform` instead of `canvas`. Measured with this diff applied temporarily and reverted (nothing under `prototyping/` is in the tree):

```diff
--- a/prototyping/viewport/design-system/components/TrackRow.svelte
+++ b/prototyping/viewport/design-system/components/TrackRow.svelte
@@ -76,7 +76,7 @@
   function waveRect(e) {
-    return (e.currentTarget.querySelector('canvas') ?? e.currentTarget).getBoundingClientRect();
+    return (e.currentTarget.querySelector('.ff-waveform') ?? e.currentTarget).getBoundingClientRect();
   }
--- a/prototyping/viewport/design-system/components/Waveform.svelte
+++ b/prototyping/viewport/design-system/components/Waveform.svelte
@@ (module)
+  const urlCache = new Map();
+
+  function rememberUrl(key, url) {
+    urlCache.set(key, url);
+    if (urlCache.size > BITMAP_CACHE_LIMIT) urlCache.delete(urlCache.keys().next().value);
+  }
+
+  function recallUrl(key) {
+    const hit = urlCache.get(key);
+    if (hit === undefined) return null;
+    urlCache.delete(key);
+    urlCache.set(key, hit);
+    return hit;
+  }
@@ (instance)
   let canvas;
+  let image;
+  const asImage = variant === 'mini';
@@
+  function pictureKey(dpr, data) {
+    return data ? bitmapKey(dpr, data) : `ph|${variant}|${width}|${height}|${dpr}|${paletteVersion()}|${animating ? 1 : 0}`;
+  }
+
+  function imageUrl(pixelW, pixelH, dpr) {
+    const data = source();
+    const key = pictureKey(dpr, data);
+    let url = recallUrl(key);
+    if (url) return url;
+    const bitmap = document.createElement('canvas');
+    bitmap.width = pixelW;
+    bitmap.height = pixelH;
+    const ctx = bitmap.getContext('2d');
+    if (data) paintData(ctx, data, pixelW, pixelH, dpr);
+    else paintPlaceholder(ctx, dpr, 0);
+    url = bitmap.toDataURL();
+    rememberUrl(key, url);
+    return url;
+  }
+
+  function drawImageElement() {
+    if (!image) return;
+    const dpr = window.devicePixelRatio || 1;
+    const cssW = `${width}px`;
+    const cssH = `${height}px`;
+    if (image.style.width !== cssW) image.style.width = cssW;
+    if (image.style.height !== cssH) image.style.height = cssH;
+    const url = imageUrl(Math.round(width * dpr), Math.round(height * dpr), dpr);
+    if (image.getAttribute('src') !== url) image.setAttribute('src', url);
+  }
+
   function draw(phase) {
+    if (asImage) return drawImageElement();
     if (!canvas) return;
@@ ($effect)
-    if (data && recallBitmap(bitmapKey(window.devicePixelRatio || 1, data))) {
+    if (asImage) {
+      if (urlCache.has(pictureKey(window.devicePixelRatio || 1, data))) {
+        run();
+        return;
+      }
+    } else if (data && recallBitmap(bitmapKey(window.devicePixelRatio || 1, data))) {
@@ (markup)
-<canvas ...></canvas>
+{#if asImage}
+  <img bind:this={image} alt="" draggable="false" class="ff-waveform ff-waveform--{variant}" class:ff-waveform--analysing={animating && !source()} />
+{:else}
+  <canvas ...></canvas>
+{/if}
```

(The measured version computed the placeholder key inline in `imageUrl` and in the effect; `pictureKey` is that line pulled out. The other hunks are as measured.)

Layer tree with it: 16 layers (3 canvases of the player, 5 overlap), 84 before.

The smoke suite with the proposal applied (Chromium): 113 passed, 1 failed: `leaks.spec.ts` "opening and closing the palette, settings and the expanded player leaves no listeners, nodes or callbacks behind" reports DOM nodes 6,791 before and 10,007 after six cycles (listeners and callbacks flat). Base passes it, 2 of 2 runs; the proposal fails it, 2 of 2 runs. Cause not investigated (the open and close of the expanded player resizes the table, which mounts and unmounts pool rows, so the row preview elements are the likely owners).

### Frame cost, Chromium (interleaved base, new, base, new; per-frame medians with the range of the per-trace values)

Expanded player playing, 4 rounds x 3 traces of 1.5 s per side (n = 12), load 19-23:

| ms per frame | base | new | |
|---|---:|---:|---|
| Layerize | 8.40 (7.32-9.45) | 3.24 (2.90-4.10) | -61% |
| Commit | 3.81 (2.99-4.75) | 2.55 (2.32-3.62) | -33% |
| Paint | 0.61 (0.50-1.11) | 0.66 (0.43-1.44) | |
| PrePaint / Layout / Style | 0.29 / 0.33 / 0.24 | 0.28 / 0.27 / 0.27 | |
| main thread busy | 18.9 (16.5-22.2) | 11.4 (10.7-16.8) | -40% |
| frames in 1.5 s | 74 (64-83) | 90 (68-92) | |

Table fling (150 frames at 84 px/frame = 3 rows per frame, `FF_PERF_R118=scroll`), 4 rounds x 3 passes; pass 1 of each load is cold (every row is new to the preview cache, the new code encodes a PNG per row), passes 2 and 3 are warm (rows seen before):

| ms per frame | cold base | cold new | warm base | warm new |
|---|---:|---:|---:|---:|
| Layerize | 8.23 (7.49-8.36) | 3.92 (3.24-3.96) | 7.97 (7.38-8.40) | 3.69 (3.41-3.90) |
| Commit | 2.77 (1.76-2.85) | 1.39 (0.47-1.60) | 1.76 (1.23-2.03) | 0.86 (0.46-1.40) |
| Paint | 1.96 | 2.59 | 1.89 | 2.29 |
| Layout / Style | 1.61 / 1.80 | 1.69 / 1.84 | 1.53 / 1.64 | 1.59 / 1.75 |
| main thread busy | 27.3 (21.9-27.9) | 26.8 (18.8-27.7) | 22.7 (19.6-24.5) | 17.4 (15.3-20.7) |
| script in rAF (trace, 150 frames) | 1,258-1,352 ms | 1,862-1,941 ms | 956 | 686 |
| frames over 20 ms (of 149) | 89 (49-92) | 84 (26-89) | 57 (28-72) | 16 (3-45) |

Cold, the PNG encode (0.45 ms per row at dpr 1, 0.86 ms at dpr 2 on this box) eats the compositor gain: busy 27.3 against 26.8. Warm it is -24% busy and 57 against 16 slow frames.

Typing in the header search (`house`, then five backspaces, 120 ms apart, `FF_PERF_R118=typing`, 3 rounds x 3 passes, load 13-19): keystroke to second frame, median 67-71 ms base against 70-72 new, worst 88-112 base against 91-112 new; Layerize 6.4 against 5.2, Paint 3.2 against 5.1 per frame. No gain and no regression beyond the noise (a keystroke that replaces the 42 visible previews pays the encodes once; the backspaces hit the cache). The `typing during analysis: worst keystroke` budget row was not re-run.

### Frame cost, WebKit (Timeline through the inspector session, 3 rounds x 3 passes, headless software compositing; this build does not report compositor work, so Layerize and Commit have no counterpart)

| per frame | expanded playing base | new | fling warm base | new |
|---|---:|---:|---:|---:|
| Layout | 0.54 | 0.52 | 3.62 | 3.86 |
| Paint | 0.87 | 0.70 | 15.66 | 16.97 |
| Recalculate styles | 1.44 | 1.34 | 3.88 | 3.98 |
| FireAnimationFrame (script) | 2.55 | 2.33 | 9.69 | 13.15 |
| frame gap p50 / p95 ms | 18 / 30 | 17 / 27 | 110 / 142 | 112 / 148 |

In WebKit the fling is compositor-bound (110 ms frames, R29, R34), and setting `img.src` to a data URL costs 0.13 ms the first time and 0.7 ms for a URL it already knows (Chromium: 0.03 and 0.01 ms), so script grows by 3.5 ms per frame in a fling; the PNG encode is 0.65 ms (dpr 1) and 1.4-1.8 ms (dpr 2). A URL seen for the first time is not loaded until a later task (`img.complete` is false in the same task and after the microtask, true after `setTimeout(0)`; Chromium is loaded by the microtask), so a recycled row can show its previous preview for a frame. Playing steady state improves slightly (-20% Paint), scrolling is slower. No Mac measurement (no Mac here); the per-canvas CALayer cost there is the unknown that could change this.

### Pixel identity

`FF_PERF_R118=identity`: screenshot of the table area (`.ff-table__scroll`, animations disabled) in 12 states at device pixel ratio 1 and 2: top, row 100, row 5,000, the end of the list, a 111-step scroll back and forth, hover on a row, hover on a wave cell (cursor shown), one selected row, a selected range, the playing row, "Analyze all unanalyzed" running (pulsing placeholders), and the same scrolled to row 100. Rows with data and rows without (placeholders) are both on screen.

- **WebKit: 24 of 24 screenshots byte-identical, base against new**, and base against base (control) 24 of 24.
- **Chromium: 3 of 24 byte-identical, 21 differ by at most 1 to 4 levels of 255**, in two places: (1) at dpr 1, 290-350 pixels per screenshot at the corners of the fav checkbox (x 266-267 and 282-283, one level, up to 3 in four screenshots); the same pixels differ with only the cursor CSS above applied, so this comes from the `Overlap` layers disappearing (the box is rastered in another layer before), not from the preview; the preview pixels at dpr 1 are identical; (2) at dpr 2, the preview of highlighted rows (selected, range, hover, playing: 130-11,757 pixels, 1-2 levels): the translucent row tint blends under the image in the page's layer instead of under a composited canvas, and 8-bit rounding differs. Control, base against base in Chromium: 23 of 24 byte-identical, one screenshot differs in 2 pixels by 4 levels (the fav corner again), so the harness itself is not perfectly stable there. In a toy page (canvas against the same PNG as `<img>`, rest and tinted row, dpr 1 and 2) the PNG route was byte-identical in Chromium and WebKit; an SVG path instead of the PNG differed (4-253 pixels, 1-21 levels) and was dropped.

So Chromium identity to the byte is not possible with any change that removes the layers; WebKit (the Mac webview) is exact.

### Memory

- JS heap after a forced GC is the same (52 MB base and new after two full sweeps of the 10k list).
- The app's cache is smaller and bounded: 800 data URLs of 1.6 KB (dpr 2) or 0.8 KB (dpr 1), at most 1.3 MB, against up to 800 canvases of 27 KB (dpr 2, 21.5 MB) or 6.7 KB (dpr 1, 5.4 MB) now.
- **The browser retains the images the app lets go of.** Browser process RSS (all Chromium processes, `FF_PERF_R118=memory`: scroll the whole 10k list twice at 20 rows per frame, then force GC and wait 8 s), 2 runs each: base +64 and +40 MB at dpr 2, +44 and +59 MB at dpr 1; new +152 and +145 MB at dpr 2, +128 and +131 MB at dpr 1. A page that only sets `img.src` to 2k, 5k, 10k, 12k and 40k distinct data URLs on 60 images grows by +74, +90, +115, +117 and +309 MB, GC or not: about 5.6 KB per distinct image, with no plateau seen, so a 50k library scrolled end to end would hold about 280 MB more in headless Chromium. Blob URLs revoked after 800 grew more (+154 MB for 10k). Whether WebKit or a GPU-raster Chromium keeps them is not measured. This fails the "row image cache must be bounded" bar as the proposal stands; a hard cap on distinct images ever created (and canvas for the rows past it) would bound it.

### Recommendation

1. Take the R112 proposal first (compositor-driven playhead): it takes Layerize in this very scenario from 658 ms to 27 ms per 1.5 s with rows untouched, which is more than the 61% here and needs no identity trade-off.
2. Do not take rows-as-images now: Layerize and Commit fall by 61% and 33% while a track plays and the fling gets 24% cheaper when warm, but Chromium is not byte-identical, memory grows without a bound the app controls, an existing leak smoke test fails with it, WebKit scrolls slower, and the Mac effect is unmeasured. If it is wanted after a Mac measurement, lift the fence for `Waveform.svelte` and `TrackRow.svelte` and add (a) a cap on distinct images created, (b) conversion only after the table has been idle, so scrolling and typing keep today's cost.
3. App-only levers on rows (will-change, contain, isolation, cursor visibility, willReadFrequently) do not help, see the table above.

### Tools added

`app/smoke/perf-rowlayers.spec.ts` (all tests skip unless `FF_PERF_R118` is set): `FF_PERF_R118=1` layer tree by compositing reason; `FF_PERF_R118=frames` per-frame trace of the expanded player playing, with CSS variants (`FF_PERF_VARIANTS=base,no-row-canvas,...`); `identity` screenshots of the table in twelve states (`FF_PERF_DPRS`, `FF_PERF_R118_OUT=dir/prefix`); `scroll` and `typing` traces with frame-gap statistics; `memory` (RSS of the browser's processes and JS heap, `FF_PERF_DPR`); `webkit-frames` (WebKit Timeline per frame, `FF_PERF_SCENARIO=expanded|scroll`); `pool` (canvas and row churn during a fling). Per-frame numbers come from summing `Layerize`, `Commit`, `Paint`, `Layout`, `UpdateLayoutTree` on the main thread of a `devtools.timeline` trace and dividing by the `Commit` count (`scripts/perf-trace-phases.mjs` has the bucketing). Raw and the interleaving scripts were in `~/.cache/r118-work/` and are deleted after this item.


## R120 state decode (2026-11-04, `r120-state-decode`)

Branch `r120-state-decode` from `ov-1028` (`13dee70`). Linux `dev` box (16 cores, load average 10-25 from other workers, everything under `nice`), headless Chromium 1217 and Playwright WebKit, the 10k and 50k `ff-fixture` libraries (the r95 `bench_startup dump` payloads: 1.70 and 8.42 MB of JSON, the same sha256 as in R111) and two more per size: the realistic-folder copies of R111 (295 and 500 album folders) and, at 10k, an edge-data copy (`benchmark/scripts/r120-edge-variant.py`). Result: **at 50k the page spent 160-215 ms between the bytes and the track objects, so `load_state` now answers with one binary buffer. Page-side that is 30-65% less (Chromium 214 to 126 ms, WebKit 182 to 71 ms at 50k; 10k 59 to 40 and 42 to 21 ms), the backend encode 1.5-1.8x faster, first row at 50k 120-280 ms earlier in the boot harness; at 10k the first row did not move (inside the noise). The data in the page is the same, row by row and key by key.**

### Measured first (the JSON path, `load_state` answer as bytes to `TrackInfo[]`, fresh page, 15 runs, base and new alternated, median with min-max)

Bytes to objects in the page, as `Response.json()` does it for the JSON answer (UTF-8 decode of the text, `JSON.parse`, `decodeTrackTable`) and as the new code does it (`decodeStateBytes` on the buffer):

| Library | Browser | JSON: text decode | parse | build (`decodeTrackTable`) | JSON total | Binary |
|---|---|---:|---:|---:|---:|---:|
| 10k fixture | Chromium | 15 (13-20) | 21 (18-28) | 23 (18-29) | 59 (53-74) | **40 (37-65)** |
| 10k fixture | WebKit | 13 (10-17) | 19 (18-29) | 9 (8-16) | 42 (37-59) | **21 (18-37)** |
| 10k, 295 folders | Chromium | 14 (11-16) | 17 (16-25) | 20 (17-23) | 51 (45-60) | **41 (32-57)** |
| 10k, 295 folders | WebKit | 12 (8-16) | 16 (14-29) | 7 (6-15) | 36 (29-58) | **18 (15-36)** |
| 50k fixture | Chromium | 67 (64-77) | 87 (84-107) | 59 (50-68) | 214 (201-243) | **126 (112-152)** |
| 50k fixture | WebKit | 65 (62-89) | 83 (80-126) | 33 (28-42) | 182 (172-253) | **71 (65-103)** |
| 50k, 500 folders | Chromium | 59 (55-63) | 77 (74-111) | 48 (39-64) | 184 (171-219) | **103 (94-118)** |
| 50k, 500 folders | WebKit | 57 (53-65) | 66 (64-87) | 36 (30-48) | 159 (151-198) | **56 (51-81)** |

That is far over the 20 ms bar at 50k, so the change was made. What the numbers say: the floor is allocating about 150,000 strings and 50,000 objects (a V8 `slice` or `split` of an arena costs the same as `JSON.parse` creating the strings; a fused loop that cuts the title and file slices inside the object loop was not faster; the object loop alone is 60 ms cold in Chromium), so the gain comes from what the binary form does not have to do: decode 8.4 MB of UTF-8 text into a string, tokenise and validate it, and parse 9 numbers per row (the numeric columns are typed-array views over the buffer, no copy).

Backend encode (`bench_startup rep`, in one process, JSON and binary measured in the same loop on the same state, median of 3 rounds of 10-15 runs):

| Library | JSON (`serde_json::to_vec`) | Binary (`to_bytes`) | Bytes JSON / binary |
|---|---:|---:|---|
| 10k fixture | 7.9 ms (6.0-13.4) | **4.4 ms** (4.0-11.0) | 1.70 / 1.72 MB |
| 10k, 295 folders | 6.9 (5.2-13.2) | **3.9** (3.4-5.5) | 1.50 / 1.52 MB |
| 50k fixture | 29.2 (28.5-50.5) | **18.9** (18.1-27.0) | 8.42 / 8.32 MB |
| 50k, 500 folders | 27.8 (24.8-89.7) | **15.7** (15.4-60.7) | 7.32 / 7.35 MB |

In the single-thread encoder at 50k about 15 ms go to the four text columns (154,000 texts; the per-text work is not the whole of it, see below), 2 ms to copy the arenas and 2.6 ms to write the number columns. A version with one thread per text column and a version that copies ASCII runs in bulk instead of per text both produced the same bytes in the same time (19-20 ms and 18 ms at 50k, the fixture), so neither stayed; the rest looks like page faults of the fresh 8 MB output in this VM. The size on the wire is the same: titles and file names are most of it in both forms. Transfer through the Tauri IPC was not measured (no GUI here): the buffer goes through the same custom protocol as `get_waveform_detail`, without the JSON text decode on the other side.

### Changed (no markup, no CSS, same rows)

- **`dto.rs`: `LoadedState::to_bytes`** writes the table and the playlists as one little-endian buffer (header of 10 words, playlists as JSON, a plain-ASCII text arena and a "wide" UTF-8 arena, one end offset per text in UTF-16 units with a top bit for the wide arena, `f64` durations and sizes, `u32` columns, flags; every section starts on a multiple of 8 so the page reads it as typed arrays in place). The layout is documented on the function. The split of the text in two arenas is what keeps the page's strings one-byte: with one arena a single CJK title makes `TextDecoder` return one two-byte string for all 4.7 MB (decode 40 ms instead of 21 ms at 50k, and twice the heap behind every slice). `file_size` goes as `f64`, which is what `JSON.parse` gives for the same integer, so even sizes above 2^53 come out equal.
- **`main.rs`: `load_state` returns `tauri::ipc::Response`** (the pattern of `get_waveform_detail`); `load_state_encoded` perf mark after `load_state_built`.
- **`track-table.ts`: `decodeStateBytes`** builds the same `TrackInfo` objects (same keys in the same order, `rating` and `color` added last) from the buffer; `tauri.svelte.ts` `loadState` takes an `ArrayBuffer` or the old object (browser mocks, `perf-q50`, smoke mock and tests keep sending JSON, and `decodeTrackTable` stays), and sets `ff:state-answer` and `ff:state-decoded` marks (`library.svelte.ts` `ff:state-rows`, after the rows are set) that the boot harness reads.
- Not changed on purpose: `mapTrackInfoToLocal` and the row proxies (47-97 ms wall at 50k in the harness, same before and after), the early call in `index.html`, the JSON form of `LoadedState` (still the logical payload: `bench_startup dump` writes it and its sha256 below is unchanged).

### Boot harness (`perf-startup.spec.ts`, `FF_PERF_STARTUP=<n>=<state.json|state.bin>==<backend ms>,...`, same app build, JSON payload against binary payload alternated run by run, 7 runs each, backend simulated as 45 ms at 10k and 200 ms at 50k, load 12-20; ms, median (min-max); pair diff = median of bin minus JSON in the same run)

Chromium, first real row / interactive:

| Library | Case | JSON first row | binary first row | pair diff | bin lower in | JSON interactive | binary interactive | pair diff |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| 10k | first launch | 749 (636-5508) | 694 (610-6818) | -26 | 4 of 7 | 771 | 764 | +15 |
| 10k | cold | 687 (601-4020) | 729 (695-4275) | +42 | 2 of 7 | 761 | 816 | +68 |
| 10k | warm | 608 (528-4505) | 631 (561-3616) | -3 | 4 of 7 | 676 | 703 | -23 |
| 50k | first launch | 1,055 (1,023-5,895) | 882 (835-3,646) | -212 | 7 of 7 | 1,223 | 1,029 | -209 |
| 50k | cold | 1,241 (1,091-5,145) | 967 (934-2,261) | -282 | 7 of 7 | 1,248 | 967 | -306 |
| 50k | warm | 1,075 (957-2,732) | 852 (790-3,258) | -210 | 6 of 7 | 1,178 | 880 | -231 |

WebKit (first row is interactive there):

| Library | Case | JSON | binary | pair diff | bin lower in |
|---|---|---:|---:|---:|---:|
| 10k | first launch | 1,118 (1,093-1,335) | 1,046 (1,022-1,247) | -71 | 6 of 7 |
| 10k | cold | 1,089 (1,075-1,267) | 1,116 (1,043-1,160) | +20 | 3 of 7 |
| 10k | warm | 698 (655-768) | 705 (588-743) | +14 | 3 of 7 |
| 50k | first launch | 1,779 (1,535-1,901) | 1,488 (1,448-1,929) | -263 | 6 of 7 |
| 50k | cold | 1,674 (1,622-1,790) | 1,619 (1,507-1,712) | -118 | 6 of 7 |
| 50k | warm | 1,187 (1,115-1,313) | 1,040 (964-1,194) | -162 | 7 of 7 |

The pieces of the same runs (Chromium 50k first launch / cold / warm, JSON then binary): bytes to answer in the harness (fetch plus `text()` or `arrayBuffer()`) 333 / 371 / 371 against 155 / 177 / 171; `JSON.parse` 81 / 94 / 67 against 0; `decodeTrackTable` or `decodeStateBytes` (`ff:state-answer` to `ff:state-decoded`) 61 / 28 / 28 against 134 / 129 / 128; row mapping 47 / 91 / 81 against 74 / 77 / 77; `load_state` answered 812 / 1,042 / 918 against 566 / 714 / 595. WebKit 50k first launch: transfer 395 / 248, parse 84 / 0, decode 31 / 108, map 78 / 78, answered 1,347 / 1,022. At 10k the pieces are within 10-20 ms of each other and the first row is the same, so **at 10k only the backend's 3-4 ms and WebKit's 10 ms of page work are saved; the first screen does not come earlier.** Read with care: the in-boot decode of the binary form is as large as parse plus build of the JSON form (the main thread is warm for neither, and the boot is 100-300 ms of other tasks); what the JSON form pays on top is the text decode of the 8.4 MB response, which in this harness happens inside `fetch().text()` on the busy page main thread (the 130-200 ms of `transferMs` difference), and the webview's `Response.json()` should do the same work there. The size of the first-row gain in the real app is not measured (no Mac, no GUI); the in-isolation table above is the part that does not depend on the harness. JS heap after boot (forced GC) 51.7 to 48.2 MB at 50k, 16.0 to 15.4 MB at 10k.

Same harness, realistic folders (295 and 500 album folders; load 13-24, so the ranges are wider), first real row, median (min-max), pair diff, binary lower in:

| Browser | Library | Case | JSON | binary | pair diff | bin lower in |
|---|---|---|---:|---:|---:|---:|
| Chromium | 10k | first launch | 688 (604-6380) | 631 (593-3347) | +5 | 3 of 7 |
| Chromium | 10k | cold | 713 (614-5010) | 733 (584-3676) | -46 | 5 of 7 |
| Chromium | 10k | warm | 706 (558-3666) | 616 (566-4392) | +10 | 2 of 7 |
| Chromium | 50k | first launch | 1,008 (904-6,582) | 897 (753-5,299) | -189 | 6 of 7 |
| Chromium | 50k | cold | 1,160 (1,082-7,630) | 939 (836-10,646) | -256 | 6 of 7 |
| Chromium | 50k | warm | 1,030 (930-8,123) | 1,137 (726-6,663) | -139 | 6 of 7 |
| WebKit | 10k | first launch | 1,176 (1,052-1,230) | 1,059 (974-1,377) | -46 | 5 of 7 |
| WebKit | 10k | cold | 1,166 (1,077-1,302) | 1,175 (999-1,408) | -82 | 5 of 7 |
| WebKit | 10k | warm | 685 (575-893) | 784 (619-826) | +92 | 2 of 7 |
| WebKit | 50k | first launch | 1,617 (1,371-1,637) | 1,518 (1,323-1,589) | -48 | 6 of 7 |
| WebKit | 50k | cold | 1,731 (1,549-1,878) | 1,536 (1,426-1,737) | -141 | 7 of 7 |
| WebKit | 50k | warm | 1,143 (1,057-1,294) | 987 (924-1,123) | -187 | 6 of 7 |

The 50k rows move the same way as with the fixture (the Chromium 50k warm row is the one that disagrees with itself: median 1,030 against 1,137, yet lower in 6 of 7 pairs, pair diff -139; the load swung during that run); at 10k the sign flips from row to row. `decodeMs` of the binary form (Chromium 10k 32-57 ms, 50k 92-152 ms; WebKit 17-19 and 55-93 ms) is above the JSON form's `decodeMs` and below its parse plus decode plus text decode, as in the fixture runs.


### Identity

- **Logical payload unchanged.** `bench_startup dump` (`serde_json::to_vec` of `loaded_state`) on the new code gives the R111 sha256: 10k `38fe2965bc4aa7d3218fbb616669960f466e297f282bac861d8dd0dcd2249eb0` (1,699,963 bytes), 50k `297cb41d20496dc9c9a74e93c71f3e4f248c6fca76e0a855225339bdd41c0dc9`, 10k with 295 folders `1817f860d6c7a7137ecb36b3a67899a2091bb0adfd61461ff93ea8e9614231cf`, 50k with 500 folders `6b7ceabd09beffdf34a64ba8a773a3b04a6b39061b34b74b5969f42bda448b81`, and for the new edge copy `4d2bfc155860f3f2feab9b88b36fab97209e3635a1b11194dfd84d7ef12eb719` (no base binary of this one: `loaded_state` and the JSON form were not touched, the other four equal R111's).
- **Page level, every row, every field, key order.** For each library `decodeStateBytes(state.bin)` against `decodeTrackTable(JSON.parse(state.json))`: `JSON.stringify` of each row (so key order counts, `rating`/`color` position included) hashed with sha256 over all rows, plus the playlists compared as JSON, in Chromium and WebKit (`crypto.subtle`, in the page) and in node (vitest `state-bytes-real.test.ts`, `FF_STATE_PAIRS=<state.json>=<state.bin>,...`). All 10 page runs and the 5 node runs: identical, first differing row none.

| Library | Rows | sha256 of the rows (JSON path = binary path, Chromium = WebKit) |
|---|---:|---|
| 10k fixture | 10,000 | `9d2c2115772ebd4f62f5d9b0ece86b245e1373bf682a8c210d4ba99dbf1265cb` |
| 50k fixture | 50,000 | `e015b861aeb97a46ae92290d88c708f5d574f9436258180c48b8688007a7cf64` |
| 10k with edge data | 10,000 | `baf4b5869063ab3cc052bf5ff895c6b5c07eea5817feaad9ed6db03e8242c322` |
| 10k, 295 folders | 10,000 | `540a1bc5e09ac75d31170975e6073d6cd53613e3df7f2263b60c3e800538cdfb` |
| 50k, 500 folders | 50,000 | `31ed1ad3802f2fcee771e08e8e99b34333e000769aab040949eca5dccffb4e5e` |

- The edge copy has an empty first and every 7th, 11th, 13th and 19th artist, album, genre and title, empty keys, quotes and backslashes in titles and genres, non-ASCII titles, artists and albums (accents, Japanese, an astral emoji, a leading U+FEFF that `TextDecoder` would drop without `ignoreBOM`), Windows (`C:\...`), bare and Japanese paths, tempo 0, bpm 127.995 and 128.005, analyses without a key or row preview, stages 3 and a fifth of the covers deleted.
- **Golden files, both sides.** `dto::tests::state_bytes_match_the_golden_files_the_app_decodes` builds a 5-row edge table (empty strings, quotes, backslash and Windows paths, BOM, astral and CJK text, a NUL, `u32::MAX` id, `u64::MAX` size, 1e21 duration, rating 0 and empty colour, playlists with quotes and an empty folder) and an empty state and compares the bytes and the JSON with `app/src/services/fixtures/state-{edge,empty}.{bin,json}` (`FF_UPDATE_FIXTURES=1` rewrites them); `track-table.test.ts` decodes the `.bin` and expects `toStrictEqual` and equal key lists against the decoded `.json`, and `early-load-state.test.ts` does it through `loadState()`.
- One difference by construction, not by data: a non-finite `duration_secs` would be `null` in JSON and NaN in the buffer; the backend never produces one (it is `as_secs_f64` of a decoded length).

### Not done, and why

- **Lazy or chunked object creation** (decode `TrackInfo` per 2,500 rows inside `mapTracksInTaskSizedChunks`): every row's fields are read at once by `mapTrackInfoToLocal` and `raw` must stay a `TrackInfo` (it is written to, `track.raw = {...}`), so it moves the same work into smaller tasks; the longest task of the boot (230-310 ms) is the shell mount, not the decode. Not done.
- **Rows without the intermediate `TrackInfo`** or lazy strings: the way to get below the allocation floor above, but it changes `library.svelte.ts` and what `raw` is; a question for the orchestrator if the 50k first row still matters.
- **Mac / real IPC**: untested here; the expectation is the larger gain on WebKit (JavaScriptCore's `JSON.parse` plus text decode is slower than V8's: 182 ms against 71 ms above) and no extra cost in the transfer (the waveform detail uses the same response type).

### Tools added

`bench_startup dump-bytes <dir> <out>` (the binary answer) and `binary_ms` / `binary_bytes` in `rep`; `perf-startup.spec.ts`: a `.bin` payload is served as an `ArrayBuffer`, `decodeMs` and `mapMs` per boot (from the three marks), payload kinds are interleaved run by run, `FF_PERF_STARTUP_RAW=<file>` writes every run; `benchmark/scripts/r120-edge-variant.py`; `app/src/services/state-bytes-real.test.ts` (skips without `FF_STATE_PAIRS`).

## R121 mount (2026-11-04, `r121-mount`)

Branch `r121-mount` cut from `ov-1028` at `13dee70` (r114 to r118 included; everything below was measured on that base and on that base plus this change), rebased onto `7056104` (r120) afterwards. Linux `dev` box, headless Chromium 1217 and Playwright WebKit, 10k fixture payload (`~/.cache/r95/state10k.json`, backend simulated as 118 ms), 1600x1000, box load 12-20 (other workers, one of them running Playwright too). Base and new were built from separate copies of the tree and run alternately (base, new, base, new ...), 3 runs per side per pair. Result: **`mount(App)` 106 to 65 ms and 36 ms less main-thread CPU from the start of the mount to the first paint (172 to 136 ms, -21%); first real row 35-50 ms earlier in Chromium on first launch and warm, a wash cold and in WebKit (below); the first screen is byte-identical in both browsers.**

### Where the mount went (what was measured)

Chromium trace of a warm reload (`FF_PERF_STARTUP_TRACE`, minified build, `performance.mark` around `mount(App)` added to `main.ts` in the scratch copies only), 5 interleaved rounds, ms, median (range), CPU = `tdur` of the main-thread tasks:

| Phase, mount start to first commit | base | new |
|---|---:|---:|
| `mount(App)` wall | 105.8 (94-138) | 65.3 (55-73) |
| `mount(App)` main-thread CPU | 94.4 (86-129) | 56.6 (53-62) |
| effects flushed after the mount (JS, includes the first `WaveformDisplay` transport) | 40.4 (33-48) | 33.7 (30-38) |
| style recalculation (5 passes) | 18.1 (17-19) | 14.3 (13-20) |
| layout (4 passes) | 18.6 (17-22) | 17.7 (16-21) |
| paint, pre-paint, layerize, commit | 7.6 | 7.6 |
| **CPU, mount start to first commit** | **172.4 (166-215)** | **136.1 (123-142)** |
| wall, mount start to first commit | 199.9 (186-232) | 154.4 (131-202) |

Per component (CPU profile of the unminified build, one profile per side, inclusive ms inside `mount`): `TrackTable` 47.2 to 11.3 (24 `TrackRow` components 31.7 to 0), whole `mount` 107.5 to 72.2; `Sidebar` 25.2 to 26.4 (its cold-code cost did not move: the shared runtime paths the skeleton rows warmed are now first run by the sidebar), `Player` 11.3 to 10.3, `GlobalHeader` 5.0, `AppStatusBar` 4.2-4.7, `ColumnHeader` 3.9-4.1. Settings and the other dialogs are not in the first screen any more (r114); what was still eager and closed was four `ContextMenu` components, two `Dialog`s (about 2-4 ms together).

The forced layout of r114 (32 ms) is not extra work. The first read of a layout-dependent value in the effects after the mount (the compact player's first `setTransport`, `WaveformDisplay._canvasSize`, then `getBoundingClientRect`) makes the engine lay out the page there instead of in the frame; the style and layout passes cost the same wherever they run (see "Tried" below). A second full-document layout (1,046 layout objects, 8-10 ms, in the task of the `load` event) is the font swap after the 11 faces requested by `loadEveryFontSubsetNow()` finish; it is also unchanged.

### What changed (`app/src`, no visible change)

- **Skeleton rows are one HTML string** (`lib/skeletonRows.ts`, `TrackTable.svelte`). The 24 placeholder rows were 24 mounted `TrackRow` components with an empty track: 13 cells each, a `Waveform` canvas with its effects and drawing per row, five rating stars, all hidden by the skeleton CSS but built and laid out (577 elements, 337 of them rows in the skeleton alone). Now each row is built from a string with the same wrapper, `.ff-trow`, cell classes, `data-col-key` and per-column layout style (`columnCellStyle`, the function `TrackRow` uses), minus the children the skeleton CSS hides (`.ff-trow__text`, wave canvas, stars, fav box); only the cover thumbnail keeps its child. 337 elements for the whole skeleton, 571 for the page (811 before). The skeleton CSS selectors moved under `.ff-table__skeleton :global(...)` because the injected rows do not carry Svelte's scoping class; the same rules, one class more of specificity in both of the places that matter. `skeletonRows.test.ts`: row count and the text-width cycle, cell classes and layout style equal to the `TrackRow` plan for every column kind, escaping of a hostile column key.
- **Closed menus and dialogs mount when they open** (`{#if menu.open}`): the three `ContextMenu`s of `TrackTable`, the one of `Sidebar`, the board menu of `OrganizePanel`, the remove-favourite `Dialog` of `Sidebar` and the analyse prompt of `App`. Same props, same open behaviour (`ContextMenu` and `Dialog` render nothing when closed and have no exit transition); their item builders run on open instead of at every prop change.
- `perf-first-screen.spec.ts` (new, env-gated, below).

### Before / after, 10k

Chromium, 6 pairs of 3 runs (n = 18 per side, one pair is a median of 3), ms, median (range); pair diff = median over the pairs of the pair medians, new minus base:

| Case | first real row base / new | pair diff | interactive base / new | pair diff | longest task base / new | pair diff |
|---|---:|---:|---:|---:|---:|---:|
| first launch | 737 (602-2055) / 679 (590-785) | -51, new faster 5/6 | 816 / 758 | -46, 5/6 | 274 / 214 | -47, 6/6 |
| cold | 674 (610-1056) / 665 (605-750) | +13, 3/6 | 835 / 758 | +17, 3/6 | 245 / 206 | -49, 6/6 |
| warm | 641 (518-723) / 597 (514-730) | -49, 4/6 | 707 / 668 | -21, 4/6 | 186 / 147 | -37, 6/6 |

`frameInteractive`: 879 / 829 (-43), 830 / 797 (-8), 779 / 743 (-53). The cold row is the weak one again (as in R114); the pooled medians move by 9-77 ms in the new build's favour but the pair medians of first row and interactive are inside the noise there. The longest task falls in every pair (6/6) in all three cases, which is the mount.

WebKit (Playwright, 5 pairs of 3 runs, n = 15 per side): first real row 1,152 (1,077-2,063) / 1,110 (1,031-1,756) first launch, 1,170 (1,046-4,642) / 1,116 (991-1,912) cold, 726 (614-948) / 667 (587-947) warm; pair-median diffs +14, -41, -13 ms with new faster in 2, 3 and 3 of 5 pairs; `frameInteractive` -8, +6, -28; `longestFrame` 23-25 ms on both sides. Headless WebKit adds the compositor gap of R29 and the box load swings the single runs by hundreds of ms, so this is "not worse, probably 40-60 ms better", not a measured win. Not measured: the Mac (JavaScriptCore compiles on first call on the page thread, so the 36 ms of CPU should count at least as much there).

### Identity

`app/smoke/perf-first-screen.spec.ts` (`FF_PERF_FIRST_SCREEN=<state.json> FF_PERF_FIRST_SCREEN_OUT=<prefix> [FF_PERF_FIRST_SCREEN_BOOT=1] npx playwright test perf-first-screen`, `FF_SMOKE_BROWSER=webkit` for WebKit): captures four states of the same page, with the CSS animations paused at 700 ms and fonts ready, as a screenshot (sha256 prefix) and the normalised `body.outerHTML` (comment anchors and `svelte-<hash>` classes removed): `skeleton` (the answer never comes), `boot` (a 20,000+ track payload with the boot snapshot of the last run), `row` (first real rows, `get_row_previews` held so the state is stable, 1.5 s after the first row) and `late` (4 s after the first row, normal previews). Base and new built from separate folders, 10k payload:

| | Chromium base / new | WebKit base / new |
|---|---|---|
| skeleton, screenshot | `3751aaa37576` / `3751aaa37576` (43,457 B) | `07f7d4c7f9f1` / `07f7d4c7f9f1` (135,915 B) |
| first rows, screenshot | `27ffd42c6b36` / `27ffd42c6b36` (149,472 B) | `6e5613496435` / `6e5613496435` (571,823 B) |
| first rows, body HTML | `b9558933c5e9` / `b9558933c5e9` | `da09626953d8` / `da09626953d8` |
| 4 s later, screenshot and HTML | same as first rows, both sides | same as first rows, both sides |

The screenshot hashes of the first rows and of the settled screen are the ones R114 recorded (`27ffd42c...` and `6e561349...`), so nothing changed since then either. Only the skeleton's own HTML differs (fewer elements: `87c36072dcfc` / `13dbb499f505` Chromium, `e9f4020a4d27` / `a5b04f57ff82` WebKit); its pixels are identical. Control: base against base twice gave the same hashes in Chromium. 50k payload with boot rows (Chromium): skeleton screenshot identical, boot rows `d4d83b4c363c` / `d4d83b4c363c` (131,659 B) with the same HTML `442b43705...`, first rows and later `650195cc537b` / `650195cc537b` with the same HTML `6e2edc6f7e35`. The whole smoke suite (both browsers) opens every menu and dialog that is mounted on open now.

### Tried, not landed

| Try | Result |
|---|---|
| `bind:clientHeight` of the table and the organize rail as a `ResizeObserver` attachment (no read at mount; `TrackTable`, `OrganizePanel`) | the layout is forced one step later by `WaveformDisplay._canvasSize` (fenced design-system component, `setTransport` of the compact player at mount); main-thread CPU mount start to first commit 153 ms against 146 ms (one trace each, load 26 vs 14): no saving, so reverted |
| `<link rel=preload>` for the two latin font files in `index.html` | the full relayout at the `load` event stays (the nine other subsets still finish there); CPU 121-136 against 127-132 ms: nothing |
| `font-family: sans-serif` on the skeleton (no text in it, so no font dependence) | the font-swap relayout touches 277 layout objects instead of 1,046 but takes 8.5 against 10.7 ms (-2 ms): not worth a hack |
| `Icon` as a cached `<template>` clone instead of `{@html}` (proposal for the fenced `Icon.svelte`; 109 `<svg>` on the settled screen, `html()` 7 ms self in the profile) | no difference beyond the noise: mount start to first commit 145-188 against 133-203 ms CPU, 4 rounds each at load 20-24; not proposed |

### What is left, and why it is not in this lane

`mount(App)` is about 65 ms wall, `Sidebar` 26 ms (cold code; its playlist rows come with the answer), `TrackTable` 11 ms (`ColumnHeader`, derived state), `Player` 10 ms, header and status bar 5 ms each. What is left is the design-system components' first run (`SidebarRow`, `Icon`, `ResizeHandle`, `PlayerCompact` internals, `TrackRow` for the real rows: `set_style` per cell is 7.6 of its 71 ms in the profile), the 40 ms of module evaluation before the mount (trace; the profile shows 12 ms of JS in it), style recalculation of 570 elements (14 ms) and the layouts. After the answer the first real rows cost about 39 ms (`TrackRow`), the 82 sidebar rows 32 ms and `sidebarData` 7 ms (`sidebarTreeRows` over `$state` playlist proxies, in `library.svelte.ts`, r120's file). Deferring the sidebar's playlist rows or the rows below the fold would cut the first-row time but changes the DOM at the first-row moment, which this item may not do.

After the rebase onto `7056104`: `npx vitest run` 940 passed (2 skipped), `npm run check` 0 errors, `npm run build` ok, smoke 114 passed and 0 failed in Chromium and in WebKit, and all the screenshot and HTML hashes above are the same against a fresh build of `7056104` in both browsers. The timings above were not repeated on the rebased tree (r120 changes the answer path, not the mount).

Scratch (`~/.cache/r121-work`, with the scripts that read the traces and profiles) is deleted after this item.

## R124 playlists (2026-11-06, `r124-playlists`)

Goal: a playlist view as fast as All Tracks at 50k: switching to it, sorting, searching in it and adding, removing and reordering tracks, for playlists of 1k, 5k and 20k tracks, with the same rows in the same order. R113 and R115 made sort and search fast for `library.tracks` only; a playlist took the old comparator sort, the old filter and rebuilt its rows from a `$state` proxy of its ids on every visit. Branch `r124-playlists` (work commits from the first worker, continued as `r124-playlists-cont`, rebased onto `ov-1028` `e51d9e7`). Linux `dev` box, headless Chromium and Playwright WebKit, box load 11-44 (other workers). Base = `ov-1028` in a separate copy of the tree with the same spec and test files; base and new runs interleaved. Payload: the r95 50k dump plus three playlists of 1,000, 5,000 and 20,000 distinct library ids in random order, each with two ids that are not in the library (`benchmark/scripts/r124-playlist-variant.py`; 998, 4,998 and 19,998 rows show).

### Where the time went (base)

- **The ids were a `$state` proxy.** Svelte wraps every plain array in `library.playlists` in a proxy with one signal per element it reads: reading a 20k-id playlist once cost 100-200 ms and left 20,000 signals, and `JSON.stringify` of the playlists for `save_state` walked the proxies: 100-1,000 ms per save at 50k (store level, every save of the scripted edits).
- **Rows were rebuilt on every visit** (`track_ids.map(libraryTrackById).filter(Boolean)` through that proxy): 250 ms script for the second visit of a 20k list.
- **Sort and search of a playlist took the pre-R113 paths:** a comparator sort over `{track, value}` per row (225-450 ms at 20k per click), a filter that looked up each row's haystack by id.
- **Edits were quadratic:** `addTracksToPlaylist` filtered with `track_ids.includes` per new id (40 s to add 500 tracks to the 20k list at store level), `moveTracksToPlaylist` and `removeTracks` likewise.

### Changed (no markup, no CSS, same rows, same order)

- `app/src/lib/playlistIds.ts` (new): playlist ids are arrays with a marker prototype (`rawIds`), which Svelte's `$state` leaves unproxied. Every writer in `library.svelte.ts` replaces the array instead of changing it in place (`addTracksToPlaylist`, `moveTracksToPlaylist`, `removeTracksFromPlaylist`, `reorderPlaylistTracks`, favourites, duplicate, clear, remove tracks, `loadState`), so `track_ids` stays the one signal that follows a playlist. Ids that are not marked (a playlist from anywhere else) take the old path.
- `tracksForSidebarSource` for a playlist: the rows are kept per ids array and library list, built once through a position table by id (`Int32Array`, a `Map` only for sparse or huge ids), and keep the positions of their members in `library.tracks`. After a bump the same rows are reused (copied only when a member was bumped), so a sorted or filtered view follows field changes as before.
- `filterTracks` on playlist rows takes the haystacks and the letter/digit masks of R113 by member position; its results carry their member positions too.
- `sortTracks` on playlist rows whose members are all different: the order comes from the whole-column rank cache of R113/R115 (built when it does not exist yet only for a list of at least 1/8 of the library), ties in playlist order; duplicates, `fav` and small lists keep the comparator. The sort warm-up drops stale column entries before it starts.
- Edits use `Set` membership instead of `includes`.
- Tests: `playlist-sort-fuzz.test.ts` (random switching, typing, edits, bumps, sort changes and reloads against a plain reference; raw ids, a table view following edits, dense, sparse and huge ids), the env-gated `playlist-path-real.test.ts` (real payload, 65 scripted steps per playlist, timing and a hash of every result list, every save serialization timed) and `playlist-view-probe.svelte.ts` (the table's derived chain for tests). Perf spec `perf-playlist.spec.ts` (`FF_PERF_STATE=<state> FF_PERF_RUNS=n [FF_PERF_SCENARIO=columns] npx playwright test perf-playlist`), with `save_state` arguments serialized as Tauri's IPC does.

### Before / after, store level (script ms, one thread in Node, real 50k payload through `loadState`, warmed column sorts, 3 interleaved pairs, median (range), the awaited save included)

| Step | 1k base | 1k new | 5k base | 5k new | 20k base | 20k new |
|---|---:|---:|---:|---:|---:|---:|
| first switch | 191 | 30 | 369 | 25 | 582 (433-1501) | 63 (29-86) |
| switch again | 92 | 2.8 | 114 | 9.8 | 254 (252-317) | 6.8 (1.7-7.2) |
| type 'h' | 54 | 2.0 | 29 | 17 | 79 | 19 |
| backspace | 0.7 | 0.3 | 2.1 | 0.3 | 8.6 | 0.4 |
| sort title asc | 23 | 5.6 | 129 | 11 | 375 (276-706) | 39 (23-64) |
| sort artist asc | 11 | 3.0 | 52 | 15 | 294 | 26 |
| sorted clear search | 5.4 | 1.2 | 42 | 3.5 | 221 | 9.8 |
| sorted switch back | 245 | 2.3 | 170 | 8.2 | 627 | 22 |
| 5 tracks analysed while sorted by BPM | 152 | 37 | 217 | 30 | 503 | 52 |
| add 10 | 229 | 10 | 318 | 14 | 860 | 38 |
| add 500 | 2,667 | 13 | 11,814 | 20 | 39,765 | 57 |
| remove 10 | 372 | 8.6 | 374 | 29 | 518 | 33 |
| reorder 10 | 174 | 17 | 291 | 143 | 454 | 153 (15-251) |
| one `save_state` serialization | 184-326 | 6-9 | 128-475 | 10-65 | 235-995 | 6-20 |

All 24 sorts (12 columns, both directions) at 20k: base 45-452 ms, new 10-144 ms (the first click of a column whose order is not cached yet is the slow one: index, album, genre, rating, colour 61-144 ms). For reference, showing All Tracks is 3-5 ms in the same test.

### Before / after, page level (`perf-playlist.spec.ts`, 50k)

Chromium, 6 interleaved pairs (4 at load 10-44 on the first worker's tree, 2 at load 12-28 after the rebase), ms, median over runs of the per-run median (range); `total` = pointer or key down to the second animation frame after it, `script` = Chromium `ScriptDuration` of the step:

| Step | size | total base | total new | script base | script new |
|---|---|---:|---:|---:|---:|
| switch from All Tracks | 1k | 189 (111-1312) | 102 (84-158) | 95 | 30 |
| | 5k | 158 (116-1142) | 77 (66-93) | 81 | 19 |
| | 20k | 285 (206-862) | 82 (66-144) | 216 | 16 |
| switch, list sorted | 20k | 315 (251-2177) | 104 (77-152) | 250 | 38 |
| first sort click (title asc) | 5k | 113 (88-2251) | 72 (68-103) | 46 | 21 |
| | 20k | 143 (132-6517) | 87 (84-116) | 75 | 31 |
| other sort clicks | 20k | 124 (108-1199) | 72 (68-88) | 66 | 25 |
| search keystroke | 20k | 81 (66-166) | 58 (52-64) | 32 | 14 |
| search keystroke, sorted | 20k | 84 (70-751) | 57 (52-65) | 36 | 13 |
| backspace | 20k | 87 (64-355) | 54 (49-54) | 38 | 9 |
| clear search | 20k | 90 (70-146) | 65 (60-87) | 16 | 13 |
| add 10 rows (drag to sidebar) | 5k | 143 (134-1002) | 75 (58-121) | 87 | 29 |
| | 20k | 165 (154-2572) | 63 (55-85) | 112 | 21 |
| reorder 10 rows (drag in list) | 5k | 188 (170-1546) | 101 (88-159) | 117 | 31 |
| | 20k | 530 (444-2437) | 130 (90-1339) | 469 | 70 |
| remove 10 rows (menu) | 5k | 179 (152-1607) | 87 (76-124) | 112 | 28 |
| | 20k | 325 (305-1229) | 86 (78-3686) | 270 | 34 |

At 1k search and sort were already near the floor (search keystroke 70 vs 74 total, 25 vs 23 script; sort clicks 85 vs 76). Reference, switch back to All Tracks in the same runs: 70 (62-150) / 68 (61-72) total, 13 / 11 script; a 20k playlist now switches in 82 total and 16 script, a sorted one in 104 and 38. The column sweep (every sortable column, both directions, 28 clicks per list, one pair): 20k median 108 to 90 total (p90 238 to 136), 52 to 27 script.

WebKit, 5 interleaved pairs (3 + 2 as above), `total` only (no `ScriptDuration`; headless WebKit adds a compositor floor of about 160-200 ms to every step, R29):

| Step | 1k base / new | 5k base / new | 20k base / new |
|---|---:|---:|---:|
| switch from All Tracks | 347 / 300 | 290 / 234 | 357 / 256 |
| switch, list sorted | 258 / 200 | 291 / 194 | 390 / 256 |
| first sort click | 209 / 215 | 247 / 193 | 294 / 293 |
| other sort clicks | 189 / 200 | 215 / 182 | 238 / 215 |
| search keystroke | 247 / 237 | 229 / 254 | 232 / 230 |
| backspace | 218 / 220 | 210 / 198 | 243 / 198 |
| add 10 rows | - | 332 / 289 | 412 / 417 |
| reorder 10 rows | - | 446 / 368 | 733 / 452 |
| remove 10 rows | - | 383 / 335 | 561 / 370 |

All Tracks switch in WebKit: 263 / 252. Key down to the handler's `setTimeout(0)` (`js`) for the switch at 20k: 137 to 48 ms, sorted 263 to 13; remove 10 rows at 20k 302 to 94. Keystrokes and the 1k list are inside the noise of the compositor floor in WebKit. Not measured: the Mac (JavaScriptCore on the page thread; the script savings above should count at least as much there).

### Identity

- Store level: a hash of every id in order of all 195 result lists (65 steps x 3 playlists: switch, switch away and back, "house dub" typed letter by letter, 4 backspaces, clear, "a", all 12 sortable columns of the store in both directions, sorted typing, backspace, clear, switch away and back while sorted, 5 analysed tracks while sorted, unsort, add 10, add 500, insert 10 at 100, remove 10, remove 500, reorder 10, sorted add and remove; edits also hash the playlist's stored ids): **195 of 195 equal** between base and new, every repeat. Examples: 20k switch `19998/c3f2f6a3-3429cd94`, 20k "house" `3917/ea54a20e-73f586ab`, 20k after reorder `20008/4e1e3fec4be6072c`.
- Page level, Chromium and WebKit: the rows on screen (every cell's text and row index) after each of 79 steps, and at the top, middle and bottom of the whole list at 30 checkpoints (open, typed, every sort, sorted search, after add, reorder and remove): **79 of 79 and 30 of 30 equal** in every run of both builds, in both browsers. Column sweep (`FF_PERF_SCENARIO=columns`, the playlist layout seeded with all 14 sortable columns: status, fav, #, title, artist, album, label, genre, year, rating, colour, BPM, key, time): **87 of 87 checkpoints and 90 of 90 on-screen states equal** in Chromium and in WebKit (61 distinct views).
- Fuzz: `playlist-sort-fuzz.test.ts` against a plain reference, in the normal `vitest` run.

### Read with care

- Reordering 10 tracks inside the 20k list is still 70 ms script in Chromium (store level 15-251 ms): a reorder replaces the ids, so the rows and their member positions are built again from the new ids, and the drag end and the save follow. Not profiled further.
- The first sort of a column whose order is not cached costs what R115's first sort costs for the whole library (the playlist uses the library order); the warm-up of R115 builds the columns visible in the library layout after start (by default title, artist, label, BPM, key and time), so those are warm.

Scratch (`~/.cache/r124-work`) is deleted after this item.

## R126 rescan (2026-11-06, `r126-rescan`)

Branch `r126-rescan` from `ov-1028` (`d77ce38`). Linux `dev` box, release `bench_startup rescan`, warm page cache (a cold cache cannot be dropped here), box load 15-60 the whole time (other workers), so read the base-against-new pairs, not the absolute values. Base and new are built from the same tree (the bench commit alone against the bench plus the fix), run interleaved, 5 rounds each, median. Raw and scripts: `~/.cache/r126/`.

**What a rescan is.** Importing a folder that is already in the library: `plan_import` (known paths, one folder walk, tag reads of unknown files only, duplicate check) and `commit_import`. R6 already made a known path cost no tag read, so the no-change case was a folder walk plus bookkeeping. A file that changed on disk under a known path is not read again (before and after); no stat, mtime or size is compared, and adding that would change results, so it is not done. Removed files stay in the library (an import never removes).

**Datasets.** Hard-link trees of the R13 fixtures (`cp -al`, nothing copied, fixtures untouched) with libraries built by the same scan, covers stripped (`DELETE FROM artwork` + `VACUUM`; the rescan never reads them): 10k (9.4k folders), 25k, 50k (`copy1` + `copy2` of the 25k tree: 2 top folders, 18.8k folders). Scenario A = nothing changed. Scenario B = 1% changed: 0.33% added (hard links of other fixture files, new to the library), 0.33% removed (unlinked in the tree), 0.33% rewritten (copy with a later mtime). `bench_startup rescan <folder> <library.db>` does what the two commands do (known paths, walk, scan, plan, take, commit), per stage, plus process CPU.

### Before and after (backend, median of 5, ms; "rows" = until `commit_import` returns)

| | rows before | rows after | known paths | walk | scan | plan | commit | CPU user+sys before | after |
|---|---:|---:|---|---|---|---|---|---:|---:|
| 10k, A (no change) | 320 | 236 | 50 / 36 | 150 / 144 | 24 / 25 | 37 / 24 | 50 / 0 | 0.36 s | 0.33 s |
| 25k, A | 598 | 430 | 84 / 40 | 299 / 295 | 55 / 58 | 79 / 39 | 78 / 0 | 0.83 s | 0.76 s |
| 50k, A | 1,438 | 723 | 125 / 86 | 984 / 502 | 83 / 72 | 109 / 45 | 125 / 0 | 1.70 s | 1.50 s |
| 10k, B (33 added) | 521 | 410 | 44 / 19 | 119 / 118 | 77 / 69 | 190 / 133 | 50 / 48 | 0.59 s | 0.48 s |
| 25k, B (83 added) | 969 | 864 | 57 / 50 | 265 / 318 | 92 / 89 | 390 / 292 | 111 / 126 | 1.39 s | 1.13 s |
| 50k, B (166 added) | 1,602 | 926 | 104 / 73 | 737 / 258 | 70 / 69 | 509 / 363 | 197 / 170 | 2.83 s | 2.58 s |

(stage columns: before / after.) Spread of the "rows" runs under load: 50k A 1,311-1,597 before, 608-864 after; 50k B 1,503-2,164 before, 924-1,451 after. The stages are summed from separate medians, so they do not add up to the row median.

Calls into libc per run (`LD_PRELOAD` counter, `~/.cache/r126/shim`; no `strace` or `perf` here): the walk makes no per-file `stat` (3 `statx`, a dozen `stat`/`lstat` in A at every size; 630 `statx` at 50k B are the 166 tag reads, 3 per file), `readdir` 47.8k at 10k and 229.5k at 50k (one per entry plus end), `fsync` 0 in A and 5 in B. DB writes in A: none beyond opening the file (8 `pwrite` before and after; the empty commit wrote no rows, took the paths again and queued a WAL checkpoint). B: the 166 new rows, 1,617 `pwrite` and 5 `fsync` before and after (same rows). Remaining cost in A is the kernel listing 19k folders: 0.8-0.9 s of system time at 50k, unchanged by this work (a plain `find -printf` over the same tree takes 0.45 s single-threaded).

### What changed (`pioneer-test-ui/src/import_plan.rs`, `main.rs`, `pioneer-library/src/duplicates.rs`, `queries.rs`)

- **The walk takes folders from one shared queue.** The folders below the dropped one were handed to 8 threads by the dropped folder's direct subfolders only: the 50k tree has two, so two threads walked it. Now every thread takes one folder, lists it and pushes its subfolders (`WalkQueue`); symlinked folders are still followed inside the listing thread with the same loop rule. 50k: 984 to 502 ms in A, 737 to 258 ms in B. 10k and 25k have more top folders and do not change (the 25k B pair in the table is noise: 265 / 318). 16 threads measured the same as 8 on the warm cache and were not taken; cold storage was not measurable here.
- **No allocation or hashing per file in the walk.** The extension is compared case-insensitively without a lowercase copy for ASCII extensions (others keep the old lowercase path), the audio path moves into the file list instead of being cloned twice, and the distinct-file count is made once after the walk instead of one set per thread merged serially. User CPU 0.86 s to 0.66-0.70 s at 50k A.
- **An import with nothing to add does not touch the library.** `add_scanned_tracks` returns when the plan resolved to no tracks: no second read of all paths, no empty write transaction with its pragmas and a checkpoint thread (50k: 125 ms to 0). The result is the same empty list.
- **Known paths and the dedupe set are sized up front** (`track_paths` and `plan_import` no longer grow and rehash 17 times): 125 to 86 ms for the paths, 109 to 45 ms for the plan at 50k A.
- **The duplicate index build (first plan with new files after start) is 2-3x cheaper.** It was rebuilt from every track on the first import with something new (the cache lives for the process): the three key maps were grown from empty and the Unicode folding ran on every ASCII string. The maps are reserved, and pure ASCII text takes a byte loop (`collapse_ascii`, `lowercase_nfc`) with the same result; anything else takes the old code. `DuplicateIndex::new` over the 50k library: 650-800 ms to 180-230 ms wall, CPU 0.85 s to 0.50 s (profile with the bench, 3 runs each). In the table that is the plan column in B (50k: 509 to 363 ms with the read of 50k rows, 100 ms, and the finds for the new files left).

### Identity

Same inputs, base build against new build, plan JSON (counts, duplicate rows with reasons and ids, skipped counts; token only differs in value and is blanked) and every table of the resulting `library.db` (`tracks` all 20 columns, `pending_artwork`, `analyses`, `playlists`, `sticks` and the rest: row count and SHA-256 of all rows in rowid order, plus the schema text), after the run:

| input | result |
|---|---|
| A at 10k, 25k, 50k (all known, nothing written) | plan identical, db dump identical (10,000 / 25,000 / 50,000 `tracks` rows) |
| B at 10k, 25k, 50k (33 / 83 / 166 added) | plan identical, db dump identical (10,033 / 25,083 / 50,166 rows, `pending_artwork` 33 / 83 / 166) |
| the 10k tree under another path against the 10k library (nothing known by path, 10,000 `same_file` duplicates) | plan identical, db dump identical |
| the 10k fixture against the 25k library (3 `same_file`, 8 `same_tags`) | plan identical, db dump identical |

No timestamp had to be ignored: the `tracks` table has none that the scan writes, and no column differed. Unit tests: `ascii_fast_paths_match_the_unicode_paths` (20k random ASCII strings and every single ASCII byte through the old and the new folding), `non_ascii_text_still_goes_through_unicode_folding`, `survey_lists_every_depth_of_one_top_folder_and_counts_each_file_once`, `file_kind_ignores_case_and_keeps_non_ascii_extensions_unsupported`.

### One difference, only with symlinks

Two symlinks to the same folder: which of the two paths the survey keeps depends on which thread gets there first. That was already racy between the dropped folder's subfolders (8 threads); now it also holds between deeper folders. Symlink loops and the file set are handled as before (`robustness_tests` and the survey tests pass).

### Not done

- **Skipping unchanged folders by their mtime** would make the no-change walk almost free, but needs a stored mtime per folder (library schema, needs Merlin) and the plan's counts of unsupported and already-in-library files would then have to be stored too.
- **Re-reading a file that changed under a known path** (mtime and size) is a behaviour change, not a speed fix.
- `Scanned.known` still clones each known path (about 30-60 ms at 50k); the `COUNT(*)` that sizes the path set is one more index scan (a few thousand page reads at 50k, not visible in the times).
- Cold-cache and Mac (APFS, `getattrlistbulk`) walks were not measured.

Reproduce: build `bench_startup` (`--features pioneer-test-ui/dev-bins`), `bench_startup import <tree> <work>` for a library, then `bench_startup rescan <tree> <library.db copy>`; `FF_IMPORT_PLAN_OUT=<file>` writes the plan.

## R129 budget (2026-11-07, `r129-budget`)

Read-mostly audit: where the time goes now, to pick the next speed levers. No app code changed, no harness added. Branch `r129-budget` from `ov-1028` (`8dbf8f0`, r126 included; r127 and r128 are not merged and their numbers are cited, not repeated). Linux `dev` box, release `bench_startup` and `bench_analysis` built into `~/.cache/r129-target`, headless Chromium 1217 and Playwright WebKit, the `ff-fixture` 10k and 50k libraries (read only; `bench_startup dump` payloads for the page). **Box load was 9 to 33 for the whole session and swap was full, so single runs swing by 2-10x (a 50k scroll run at load 13-32 showed p95 231 ms, the same run at load 9 showed 25 ms). Read the minimum and the median, not a single run; where a number is from a loaded run it says so.**

Commands (all existing specs, run one at a time, ports 5471-5477): `bench_startup run|rep|load|import|rescan`, `bench_analysis --profile 3 --threads 4` and a 6-track staged run, `perf-q50` (startup, track load, import), `perf-startup` (`FF_PERF_STARTUP=10000=<state>==70,50000=<state>==380`, 7 runs, backend simulated at the quiet `load_state`), `perf-search` (real payload, 3 runs), `perf10k` (synthetic rows, scroll, inspector, bulk), `perf-warmup` (script ms per second after the first rows).

### Budget by flow (Chromium unless noted; `x / y` = 10k / 50k)

| Flow | Current total | Top 3 cost centres | Class |
|---|---|---|---|
| Cold open to first row | 603-626 ms / 1,031-1,147 (warm 538 / 982); WebKit 1,100-1,173 / 1,632-1,722 (warm 777 / 1,227) | 1. page main thread until the first rows: bundle evaluation 40 ms, `mount(App)` 65, effects 34, style and layout 32, first `TrackRow`s 39, `Sidebar` 32 (R121 trace; not re-traced, the box was too loaded) | b (design-system `TrackRow`, `Sidebar` rows, `Icon`) and a (app glue) |
| | | 2. `load_state` backend 50 ms (min 34) / 210 (min 165), of which the SQLite row scan 29 / 157 | a (`pioneer-library`, no schema change) |
| | | 3. answer to track objects in the page 40 / 126 ms (Chromium; the answer is 1.7 / 8.3 MB) and the webview start itself | a (done in R120) and c (webview) |
| Cold open, disk cold | first library query 1.5 s (10k) and 4.1 s (50k) after `posix_fadvise(DONTNEED)` on a contended disk (R109 saw 490 ms at 10k on a quieter one) | the rows query reading `tracks` and `analyses` pages from storage | c (storage); a for hiding it behind the webview start |
| Cold open to interactive | 672-692 ms / 1,098-1,147 (warm 600 / 993); longest task 191-202 / 195-256 ms, total blocking 221-255 / 392-454 | the same as first row plus the long task of the mount | b and a |
| First search keystroke | 100 ms / 234 (script 29 / 120, layout 17 / 18); median key 51 / 79; with analysis running 100 / 122 | 1. compositor and paint floor (about 35-40 ms of every step in headless Chromium) 2. layout and paint of the 42 rows 3. at 50k the sort-column warm-up still running in the typing window (`warmColumn` 410 ms of script, `durationSeconds` 169 ms self, `readLibraryValues` 353 ms inclusive, profile of the synthetic 50k page at load 32) | c, b, a |
| First sort click | 63-73 ms / 72-74 (script 12-19 / 24-27, layout 7-9); **with analysis running, 50k: 89-154 ms (script 41-90)** | 1. floor 2. `sortTracks` and the column rank (cached after R115) 3. under analysis: bumped rows make the cached orders move (`withChangedValues`) before the click | c, a, a |
| Scroll a full screen | frame p50 17-19 ms and p95 25-26 at both sizes (flat in library size); fling p95 41-52 | row canvas layers and layerize (37 canvases + 37 overlap layers per 42 rows, R118), style and layout 3 ms, `flushRowPreviews` | b (fenced `Waveform` / `TrackRow`) |
| Open inspector (select a row) | 12.4 ms median, 33 worst / 13.3, 45 worst | store update, inspector bindings; markers data from the answer after 0.7 ms in the backend | floor |
| Load a track into the player | backend `get_analysis_data` 0.65 ms median (p95 29 / 73 ms when a page is cold), parse 0.2, JSON 0.1; page overview painted 35-39 ms after the answer; expand to detail drawn 90 / 116 ms; first-time detail of a track without a stored series about 850 ms (R109) | 1. page paint after the answer 2. detail first time: decode 543 ms of 413 s of audio plus `DetailMeter` 3. expand draw | a, c (symphonia) and b (`DetailMeter` in `pioneer-usb-writer/src`) |
| Analysis start and first result, fresh import | first result 29.4 s at 4 threads (R3: 17-19 s at 12 threads); 2.75 trk/min at 4 threads on a loaded box (R3: 8.5 at 12); 584 CPU-s for 6 tracks of 343-413 s; peak RSS 1.9 GB | stage profile per audio minute at 4 threads: beat model 3,762 ms of 4,181 (90%), mel 181 (4.3%), resample 175 (4.2%), waveform 141 (3.4%), decode 79, key 27, fit 12, write 13 | c (model), c (rubato), b (`Meter::push`) |
| Import 10k files | rows after 0.74-0.90 s (plan 370-541 incl. scan 255-343, commit 290-360); UI click to rows 47 ms / 93 (50k); the cover pass runs after: 7.3-10.3 s for 8,132 covers (1.5 GB of blobs) | 1. tag scan (open and first page per file) 2. commit 3. cover pass | c (storage), a, b (schema for cover dedupe) |
| Rescan (nothing changed) | 90-99 ms at 10k (R126: 723 ms at 50k) | the folder walk (kernel readdir), known paths | c |
| Idle CPU, page open | script ms per second after the first rows: 10k 121, 32, 0, 0, 12, 0, 0, 0, 0, 94 (260 in 10 s, one 94 ms task at second 9); 50k 244, 351, 117, 0, 10, 0, 0, 0, 0, 10 (731 in 10 s); about 0 after 4 s | the sort and search warm-up in idle chunks (first 3 s) | a |
| Memory after open | JS heap 16.7 MB / 59 after boot, 31 / 122 at the end of the interaction script; backend bench RSS 24 MB / 74 (real app RSS needs a window: not measured, R23 and R42 hold the soak numbers) | the track table (150,000 strings at 50k) | a |

### Findings that are not in the table

- **Analysing a selection freezes the page: 373-393 ms long task for 2,000 rows at 10k and 50k, and the "script" metric sees only 19 ms.** CPU profile (profiler on, so inflated about 7x; read the shares): of 2.6 s, 2.14 s is `analyzeTrackIds` to `markTracksPendingAnalysis` to `bumpLibraryTracks` to `copyRowFieldsToPlainTracks` to `plainValue` to `$state.snapshot` to `structuredClone` (757 ms self in `structuredClone`, 444 ms GC). Every bulk edit that calls `bumpLibraryTracks` takes the same path. The cost is per row and independent of the library size, so selecting all 50k rows would be about 10 s of main thread (linear extrapolation, not measured).
- **The first 3 s after the first rows are busy at 50k (about 700 ms of script, in chunks, no long task) and the first keystroke lands in them.** That is the idle warm-up of R113 and R115, and its biggest parts are string work that the binary table already holds as numbers (`durationSeconds` parses "m:ss" for 50k rows).
- **Box swing.** Idle load 9, other workers 20-33: the same page run gives 25 or 231 ms p95 scroll, 70 or 320 ms warm `load_state` at 10k. Budget `now` values should stay as they are (R109 reason).
- Cold disk: the first rows query was 1.5 s at 10k here against 490 ms in R109. A cold start is the only flow where one stage (storage) is bigger than all of the page.
- WebKit adds 240-600 ms to every start step in headless software compositing (R29); only the ratios carry over to the Mac.

### The 5 best next levers of kind (a)

Ranked by gain per effort. "Proof" is how to show the same rows, the same order, the same bytes.

| # | Lever | Expected gain | Effort | Proof of identity |
|---|---|---|---|---|
| 1 | **Make `copyRowFieldsToPlainTracks` copy values, not `$state.snapshot` clones** (`app/src/stores/library.svelte.ts`): copy scalar fields directly, and for object fields (the `raw` record, preview arrays) copy only what the bump changed or share the frozen value | analyze 2k selected 380 ms to under 30 ms (estimate: the clone is 80% of the task); same for rating, colour, tag and bulk edits; select-all at 50k about 10 s to under 1 s | S | vitest: random bump sequences, `plainTracks` deep-equal to a reference that still snapshots; haystack and column-rank hashes after each step equal; `perf10k` "analyze 2k selected" long task before and after; smoke in both browsers |
| 2 | **Warm the sort columns from the table's numeric columns** (`readLibraryValues`, `sortValueOfRaw`, `durationSeconds` in `table-columns.svelte.ts`): read tempo, duration and key ranks from the typed arrays the decoder already builds instead of formatting and parsing text per row | 50k: warm-up script in the first 3 s 700 ms to about 200 ms, first keystroke 234 to about 100 ms; 10k: about 100 ms less in the first 2 s | M | R113 and R115 method: hash of every sorted order (all sortable columns, both directions, with and without search) equal to base on the 10k and 50k payloads and the edge variant; `playlist-sort-fuzz.test.ts`; on-screen row hashes at checkpoints |
| 3 | **Compute the `load_state` answer at process setup, in parallel with the webview start**, and let the command take it (same function, same bytes; recompute when the library changed, guarded by a revision counter) | removes the backend from the path to the first row: 50k about 150-200 ms (210 ms backend, the page is ready for the answer at about 300 ms after its invoke), cold start up to the whole first-query time (1.5-4 s on a cold disk here; likely 0.2-0.5 s on a Mac SSD, not measured) | S-M | sha256 of the `load_state` payload from the early path equals the on-demand path for the 10k, 50k, 295-folder and edge libraries (the R111 hashes are the reference); test: an import or edit between setup and the command returns the new state; `bench_startup rep` shows the unchanged cost of the compute itself |
| 4 | **Profile and fix sort and search under analysis at 50k** (`columnSortOf` / `withChangedValues` after bumps, `scheduleSearchIndexWarmup` restarts on every bump): sort click 89-154 ms against 73 idle, script 41-90 against 25 | about 60-80 ms per sort click while analysis runs (the long state of a fresh library); fewer restarts of the haystack warm-up | M (profile first; the cause above is read from the code path, not yet measured) | same hashes as #2 but with scripted `analysis-partial` events between steps (the `perf-search` "analysis" mode already does this); `libraryRowsChangedSince` results equal |
| 5 | **Linux analysis runs: set the three allocator variables for batch runs** (`MALLOC_ARENA_MAX=1 MALLOC_MMAP_MAX_=0 MALLOC_TRIM_THRESHOLD_=1073741824`, R106) in `scripts/` and the CI or dev-box benchmark entry points, not in the app | -22% analysis CPU and -24% wall on the box (sys time 17% to 0), none on the Mac; costs +10% peak RSS, so not for the shipped app | S | `analyze-json` sha256 equal with and without the variables (R106 did this for 12 tracks, 4 tracks and a 46 min file); no app change |

Not on the list, and why: the beat model (90% of analysis CPU: chunk 500 changes the output, quantisation too; rten buffer reuse is upstream), `Meter::push` (740 ms per track, `pioneer-usb-writer/src`, 4x headroom measured in R104), rubato resampling (external, output depends on its chunking), the row canvas layers (R118, fenced `Waveform.svelte`), the cover pass (1.5 GB of blobs in one table: storage-bound, and per-album dedupe needs the schema change that `r84-path-keys` holds), the folder walk (kernel time, R126), the mount (design-system components, R121), Mac-only startup (`mbp` night, not run here).

Scratch (`~/.cache/r129-work`, `~/.cache/r129-target`) is deleted after this item.

## R130 bulk bump (2026-11-06, `r130b-bulk-bump`)

Bulk row bumps (analyze selection, analysis results, rating/colour, genre/raw, relink) no longer snapshot every row. Branch from `ov-1028`. Linux `dev` box at load 40-55, real r95 payloads, base = `ov-1028` in a separate tree, runs one after the other.

### Change

- `copyRowFieldsToPlainTracks` (`app/src/stores/library.svelte.ts`) copies without `$state.snapshot` / `structuredClone`. Each row keeps the keys written through its proxy since the last bump and the plain container fields (like `raw`) it has handed out; a bump copies only those, with a plain recursive copy. Rows never written or read for a container are skipped. `refreshHaystacks` runs only for rows whose search text fields changed.
- R129 profile blamed `structuredClone` for 80%; unprofiled it was about a quarter, the rest was proxy reads through `Object.keys` / `ownKeys`.

### Before and after (Chromium, base to new)

| Step (bump only unless noted) | 2k | 10k | 50k |
|---|---|---|---|
| Page, analyze selection, long task (ms) | 580 to 206 | 2,915 to 794 | 9,364 to 2,416 |
| Store, analyze selected, sync part (ms) | 912 to 323 | 4,463 to 1,447 | 23,049 to 7,681 |
| Store, analysis results bump | 628 to 93 | 2,920 to 352 | 14,202 to 2,493 |
| Store, rating + colour bump | 494 to 46 | 2,792 to 180 | 14,050 to 986 |
| Store, genre + raw bump (raw handed out, copied) | 507 to 222 | 2,815 to 1,126 | 14,255 to 6,068 |
| Store, relink bump | 565 to 138 | 2,782 to 728 | 14,622 to 4,064 |

The store numbers are Node in dev mode under load, so only the ratios carry. What is left in the page analyze step at 50k (about 2.4 s) is the field writes through the row proxies (about 1.3 s, Svelte `set`, row state creation), the plain copy (about 0.3 s) and the search text (about 0.25 s).

### Identity proof

- Store level, real payloads (`bulk-bump-real.test.ts`, needs `FF_PERF_STATE`): after each of 5 bulk steps, hashes of every plain row, of 24 sorted orders and of 43 search results are equal between base and new at 2k, 10k and 50k.
- Page level (`perf-bulk-bump.spec.ts`): rows on screen (top, middle, bottom, playlist and All Tracks) equal base and new after analyze at 2k, 10k, 50k and after results at 2k, 10k, in Chromium and in WebKit (WebKit run again in this round, all view hashes equal base and new).
- `bulk-bump-fuzz.test.ts`: random writes (scalars, typed arrays, nested `raw`, held nested references, added and deleted keys) and bumps against a reference that still snapshots; `toStrictEqual` on plain rows and search results after each step. Passes on base and new; two deliberate breaks fail it. Default is 1 seed; `FF_FUZZ_FULL=1` runs 4 seeds.
- Known difference: key order of plain rows for keys added by writes follows first write, not first read. Deep-equal, not observed anywhere.

### Validation

- `npx vitest run`: 948 passed, 4 skipped (the real-payload test skips without `FF_PERF_STATE`); about 31 s wall; the bulk-bump tests went from 17 s to 6 s with the 1-seed default.
- `npm run check`: 0 errors (the two bulk-bump test files needed `node:inspector` types in `src/node-fs.d.ts` and no `Buffer`).
- Chromium smoke: 114 passed, 28 skipped.
- WebKit smoke: 113 passed, 1 failed (`activity.spec.ts:137`, Import folder and files) at load 51; the same test passes alone. Not related to the bump.
- WebKit `perf-bulk-bump`: WebKit has no long-task or CDP timings, so only the identity hashes are compared, and they are equal at 2k, 10k, 50k (`FF_PERF_ARRIVE=0` at 50k). The 50k results-arrival step in WebKit did not finish within 20 minutes on base either (stopped by hand), so that hang is not caused by this change.
## R128 track load (2026-11-06, `r128b-track-load`)

Branch `r128-track-load` finished as `r128b-track-load`, based on `ov-1028` at 8dbf8f0, no push. Measured on the shared dev box (load 10-55), headless Chromium and Playwright WebKit, 1k and 50k libraries, 6 minute real tracks and a 65 minute FLAC; the real `get_analysis_data` answer and detail series were dumped and replayed.

**Trace.** Double-click, `loadPlayerTrack`: `playback_stop`, `get_analysis_data` (cached when the selecting click or the neighbour prefetch already read it), `get_waveform_detail` (47 MB for 65 min), then `playback_play` after the stop. Audio start is not a lever (first packet 0.3-1.3 ms). Base `get_analysis_data` parsed the ANLZ files twice (`get_analysis` plus `get_cues`) and sent about 2 MB of base64 in JSON for a long track.

**Changed (all output-identical)**
- `get_analysis_data` answers with a binary frame (`FFAN`, JSON head, band bytes); the page decodes it (`decodeAnalysisBytes`); no base64, no atob loop.
- One ANLZ parse per load (`LocalLibrary::get_cues_with_analysis`).
- Peak-hold downsample without per-column divisions; detail cache check without copying the 47 MB series.
- Zoom series requested after the first paint (pending state set at once); `bandsFromInterleavedBytes` gain table; `libraryTrackById` instead of a 50k `find`.

**Backend (`bench_startup trackload`, base to new, ms)**

| Track | Analysis answer | Cached detail | Answer bytes |
|---|---|---|---|
| 6 min | 5.1 to 2.8 | 3.0 to 1.9 | |
| 65 min | 68 to 23 | 124 to 48 | 2.23 to 1.73 MB |

**Page (Chromium, medians of 15 distinct rows, ready / painted, ms)**

| Case | Base | New |
|---|---|---|
| 50k, 65 min, cold | 150 / 196 | 40 / 68 |
| 50k, 65 min, warm | 55 / 151 | 21 / 48 |
| 50k, 6 min, cold | 32 / 57 | 20 / 44 |

1k numbers were noisy (load swings). WebKit: ready 20-60% lower, painted 15-30% lower (frames of 200 ms dominate); the zoom series arrives 80-110 ms later there (deferral).

**Identity proof**
- Logical payload sha256 per track equal base vs new (24 mid, 1 long, 30 fixture tracks), plus detail bytes.
- `player.track` hashes equal for 25 real answers (`player-load-real.test.ts`); first 10 s of decoded audio (start and 30 s seek) equal for 30 files.
- Golden `.bin/.json` pairs in `app/src/services/fixtures/analysis-*` (`analysis-bytes.test.ts`).
- Canvas hashes equal in Chromium and WebKit.
- Screenshot hashes, final run (Chromium, 28 mid tracks plus a 44 minute FLAC, tracks at rows 3, 5 and the long one, compact view and three seeks each = 12 snapshots, all canvases and the `.ff-xp` screenshot): 12 of 12 equal in two runs of the new build and one run of the base build (base fed the same answers re-encoded as base64 JSON, `app/smoke/perf-trackload.spec.ts` with `FF_R128_SETTLE=1`).

**The differing screenshot hashes.** Not data and not the playhead. Canvas hashes were equal in every run; only the element screenshot of `.ff-xp` varied, and the same build differed from itself between runs (5 of 12 in one pair, 4 of 12 in another). A pixel diff of two runs of the new build showed 8 x 6 and 9 x 3 pixel boxes at the pencil icon and the collapse chevron in the header: icon colour mid-transition (hover/CSS transition still running at screenshot time). With transitions and animations turned off (`animations: 'disabled'`) 2 of 12 still varied; additionally parking the mouse and injecting `transition: none` gave 12 of 12 equal across both builds. Product code did not change; the harness settles the page before it hashes.

**Validation.** `cargo test -p pioneer-library -p pioneer-test-ui` (120 and 288 tests passed), `cargo check -p pioneer-test-ui --features dev-bins --bins`, `npx vitest run` (962 passed), `npm run check` (0 errors), `npm run smoke` Chromium and WebKit (114 passed, 30 skipped each). `perf-budgets.json` not refreshed. Harness: `app/smoke/perf-trackload.spec.ts` (`FF_R128_MODE=timing|profile|identity`, `FF_R128_INDEX`, `FF_R128_SETTLE`, `FF_R128_SHOTS`), bench `trackload`, `dump-load`, `long-lib`.

## R133 Mac first launch (2026-11-06, `r133-mac-startup`)

Mac night 3 (below) reported a first-launch regression in item 1 and a failed item 2. Both were looked at on `mbp` (on AC, scratch HOMEs under `~/dev/build/fourfour-r133` and the night directory, `/Applications` and the real app data untouched).

**Item 1: not a code regression, the night machine was loaded.** Night 3 measured 1405 ms to first row and 6990 ms to interactive (5f77028), against 868 / 910 ms in R89 (c2bd463). Rerun on a quiet Mac (1-minute load about 4), each launch a fresh HOME with a copy of the 10k library and the night 3 item 1 probe:

| build | launches | first row, ms (median, range) | interactive, ms (median) |
|---|---:|---:|---:|
| R89 code (23534cc = c2bd463 + libc fix, + boot probe) | 8 | 631 (594-748) | 677 |
| night 3 app (5f77028, the binary night 3 measured; the first launch after idle left out, below) | 8 | 617 (588-660) | 666 |
| ov-1028 head (7f2325e, + boot probe), order new, base, n3 | 3 | 609 (596-692) | 657 |
| night 3 item 1 rerun with the fixed runner (ov-1028 head, first launch right after the app build) | 1 | 886 | 959 |

All three builds are within noise of each other and faster than R89's 868 ms. In the first interleaved block (base then new, 5 pairs) ov-1028 head was 764 ms against 625 ms, but the whole gap sat before the page (Rust `setup_done` 405 ms against 290 ms), there is no Rust change between 5f77028 and 7f2325e, and the gap was gone when the order changed; read it as machine state in that window (`setup` opens the CoreAudio device and the library, unverified which one varied). The page part (page start to first row) was 175-192 ms on ov-1028 head against 196-223 ms on the R89 code. The one slow launch outside night 3 was the first launch after about 10 idle minutes: 1222 ms, with `setup_done` at 710 ms.

Night 3 itself: item 1 started two minutes after the night began, right after four builds; the 5-minute load at the start was 11.6, and item 3 a minute later saw load 12-19 while the CPU was 87-90% idle (I/O or scheduler pressure, not CPU). In the item 1 log the page took 763 ms from page start to first row (quiet: about 190), 92 frames passed before 10 frames in a row were under 50 ms (quiet: 10), the longest frame was 205 ms, and the first Settings open timed out at 3 s ("settings did not open"; quiet runs: no failure, 853-857 ms per cycle). A starved WebContent process explains all of it; nothing in the code between c2bd463 and 5f77028 does.

**Item 2 failed on a runner bug.** `item2` checked for the night 3 scratch marker in `perf/home-1`, a HOME that `perf-night.sh` creates (and deletes) itself, so it always stopped with "was not created by night3"; the dry run skips that check, so it never showed. It now only checks that the path is inside the night directory (the same guard as preflight).

**Runner changes (`scripts/mac/night3.sh`).** Item 2 fix above. Items 1 and 2 wait up to 300 s for the 1-minute load to fall below a third of the cores before launching the app and log the load (or say they launched loaded), so a launch is no longer read against R89 while the machine is still busy from the builds.

**Probe change (`perf_probe.js`).** After `interactive`, a `boot_timeline` mark lists the long frames (over 50 ms) between first row and interactive, the page's `performance` marks (`ff:state-answer`, `ff:state-decoded`, `ff:state-rows`) and resource timings. Only active with `FF_PERF_LOG`. Together with `setup_done` and `page_start.time_origin` this splits a launch into Rust setup, page load and page work, which is how the window effect above was found.

**Item 2 rerun (fixed runner, ov-1028 head 7f2325e + boot probe, quiet start at load 4.8): it runs now; the first rerun missed one gate.**

| metric | R89 | night 3 rerun | gate |
|---|---:|---:|---|
| cold first row / interactive, ms | 868 / 910 | 673 / 730 | pass |
| warm first row (median of runs 2-3), ms | 628 | 702 | pass (within 15%) |
| warm interactive, ms | 645 | 807 | FAIL |
| first expand, longest frame (median of 5), ms | 34 | 26 | pass |
| scroll during analysis, frame p95, ms | 18 | 18 | pass |
| app load_state, ms | 53-67 | 81 | |

A second item 2 run on the same head right after (runner 41e5eb2): cold first row / interactive 694 / 746 ms, first expand 27 ms, scroll during analysis p95 17 ms, warm first row 579 and 586 ms (median 583), warm interactive 629 and 646 ms (median 638, passes against 645), no frame over 50 ms in any of the 8 startup launches (longest 33-45 ms). Every item 2 gate passed in that run.

The warm interactive miss is one frame. Run 3 had a single 70 ms frame about 75 ms after the first row (expand runs 1 and 2: 73 and 55 ms at the same spot), and one frame over 50 ms restarts the probe's 10-frame count, so that run's interactive moves by about 90 ms; with two warm runs the median follows. The same frame shows up for the R89 code on today's Mac: longest frame before interactive 32-44 ms for 23534cc and 33-47 ms for ov-1028 head over the quiet A/B launches (R89 itself saw 17-31 ms in its warm runs), so the frame grew with the machine, not the code, and it sits just around the 50 ms threshold. Tried and dropped: delaying the first lazy-view preload to 500 ms after the first screen when there is no `requestIdleCallback` (WebKit). In 6 interleaved pairs the 55 ms frame still came at the same spot (page 246 ms) in one launch, so the preload is not that frame; no app change was kept. What the frame is was not pinned down (WebKit has no long-task API; candidates are the row previews and covers that arrive right after the first rows).
## R134 sort warm-up (2026-11-06, `r134-sort-warmup`)

R129 lever 2 (warm the sort columns from the table's numeric columns) was measured and **not applied: the premise does not hold, and the two changes that were tried gave no gain. No product code changed.** Branch `r134-sort-warmup` from `ov-1028` (`43ee68f`). Linux `dev` box at load 13-33 (other workers), headless Chromium, `ff-fixture` 50k r95 payload. Added: `app/smoke/perf-sort-warmup.spec.ts` (`FF_PERF_MODE=warm|key|sort`, `FF_PERF_STATE`, `FF_PERF_KEY_AT`, `FF_PERF_SORT_AT`; a `requestAnimationFrame` loop keeps frames running in key and sort mode) and `app/src/stores/sort-warmup-real.test.ts` (env-gated: hash of every sorted order of 11 columns, both directions, with "", "house" and "a", cold and after the warm-up, and CPU time of the warm-up).

### Where the 700 ms goes

- Script in the first 3 s after the first rows at 50k, base: median 739 ms (674-1,084, 8 runs). With the sort warm-up switched off it was 415 ms (one run), so the sort warm-up is about 300 ms of it; the rest is the search text build and the mount.
- Per column in the page (one run, load about 20, ms read / ms value classes): title 27 / 143, artist 20 / 29, label 7 / 3, bpm 13 / 7, key 11 / 4, time 9 / 5. **The three numeric columns together are about 50 ms of about 280. Even a free read of tempo, duration and key would not take 700 ms to 200 ms.** Half of the cost is the 36k distinct lowercase titles (a Map pass and one string sort).
- `durationSeconds` runs once per distinct text (the read memoizes by raw text), not per row; the "169 ms self" of R129 was profiler inflation.
- Reading from `raw.tempo` / `raw.duration_secs` would not give the same orders: `track-edits.svelte.ts` and `library.svelte.ts` (relink, analysis results) write the `bpm` text and `bpmExact` and not `raw.tempo`, so the text is the source of truth.

### Tried, not kept

- Reading all warmed columns in one pass per chunk of rows (cache theory): CPU of six columns 380 ms against 355-395 (Node, min of 7), no change.
- Sorting the distinct strings with native numeric sorts of two-code-unit prefix keys (radix by two characters, no comparator): same classes on the 50k titles, 57 ms against 67 for title and 12.8 against 14.0 for artist (Node). Too little for the code.
- Starting the sort warm-up only after the search text is built (`whenSearchIndexBuilt`): warm-up script in the first 3 s 739 to 574 ms, but over 10 s 761 to 884 ms, first title sort 125 to 171 ms total (medians of 8, Chromium, 50k), first key unchanged (169 to 159 ms). The work only moves later.

### The first key

R129's 234 ms first key is a harness effect. In headless Chromium without frames `requestIdleCallback` fires only at its timeout, the search text build lags, and a key at 0.8-1.5 s completes it synchronously (script 216-304 ms; the same page also left the title sort cold at 7 s: 189-470 ms). With frames running (as in a window), base at 1.5 s after the first rows: script median 54 ms (36-78), total 169 ms (114-206), 8 runs. WKWebView has no `requestIdleCallback` (timer fallback).

### Identity and validation

No product file changed, so every order is the base order by construction. `sort-warmup-real.test.ts` on the 50k payload (base tree): cold and warmed hashes of all 66 orders equal. `npx vitest run`: 963 passed, 6 skipped. `npm run check`: 0 errors. Chromium smoke (port 5511) 114 passed, 32 skipped; WebKit smoke (port 5512) 114 passed, 32 skipped. Not run: 10k and WebKit before/after, the edge payload (nothing to compare).

## R135 early state (2026-11-06, `r135-early-state`)

Branch `r135-early-state` from `ov-1028`; lever 3 of R129. Linux `dev` box, release `bench_startup` in `~/.cache/r135-target`, headless Chromium and Playwright WebKit, copies of the `ff-fixture` 10k and 50k `library.db` (anlz linked) plus the R120 edge copy of the 10k. Box load 14-41 the whole time (other workers).

### What changed

- `setup` starts a thread (`spawn_early_state`) that locks the library, builds the `load_state` answer (same `loaded_state` and `to_bytes`) and parks the bytes in `EarlyState` (`early_state.rs`). `load_state` locks the library like before and asks `encoded_state`: it gets the parked bytes when they are still current, else builds on demand. The slot is handed out once (a second `load_state`, such as a page reload, builds on demand), and a request that arrives before the thread has the lock builds on demand and the thread then skips.
- **Staleness guard.** The parked answer carries the `LibraryRevision` taken right after the build and the labels map it was built from. It is served only when the library's revision is unchanged and `read_cached()` of the labels equals the stored map; both checks and the reply happen with the library locked, so no write lands in between. `LibraryRevision` (`pioneer-library/src/revision.rs`, no schema change) is the library object (a counter per `LocalLibrary`, so `change_library_path` and `reset_library` can never match an old answer), `SELECT total_changes()` of its connection (every row any write path changed, so no write path has to remember to bump anything), `PRAGMA data_version` (commits of other connections) and `PRAGMA schema_version`. The labels compare covers ratings and colours, which live in sidecar files and not in the database.
- Tests: `early_state::tests` (an import, an analysis result, a playlist edit and a removal between setup and the command each return the new bytes, equal to on-demand; a write through another connection; a rating set after setup; a replaced library; once-only; untouched library gets the early answer and equal bytes) and `revision::tests`. `load_state_built` mark stays (now with `early`), plus `early_state_ready`.
- `bench_startup early <dir> [request ms list] [out.bin]`: runs the thread and a request arriving `ms` after it started, prints request-to-answer, the on-demand build, and the sha256 of both.

### Backend (`bench_startup early`, request arrives N ms after the thread started; request_to_answer = what the page waits for after its invoke)

| Library | on-demand build | early, request at 150 / 300 ms | cost of the check |
|---|---:|---:|---:|
| 10k warm | 61-77 ms | 5 / 4 ms | 2-4 ms |
| 50k warm | 190-270 ms | 118 / 22 ms (thread done at 250-290 ms) | 17-22 ms (label cache read and compare) |
| 10k cold file cache | 60-77 ms (first query 230 ms) | 36 / 4 ms | |
| 50k cold file cache | 200-270 ms (first query 640 ms) | 422 / 293 ms (thread done at 555-575 ms) | |

The compute itself is unchanged (`rep` numbers); what moves is when it runs. How much a real launch gains depends on the head start: the time from `setup` to the page's invoke (webview creation, HTML parse), which a headless run does not have; the Mac R133 launches had `setup_done` at 290-405 ms. A request that comes after the thread finished costs only the check (2-22 ms).

### Identity

`bench_startup early` compares the early bytes with a fresh on-demand build in the same process on each library: **equal**, sha256 of the binary answer 10k `189c1084156ec70db43f1005134df010ae92b5cadde3e1f6d57074c6d86b36ad` (1,715,152 bytes), 50k `893a4522aa3716187953bf68054de31c73768fe174ae242c904bbf299fcccbe9` (8,319,832), edge 10k `c24f015ae53a7a1f0333903581210f512a12acef4e99f3777cee09c0cfc824da`. The logical JSON payload of the unchanged code (`bench_startup dump`) still gives the R111/R120 hashes: 10k `38fe2965...49eb0` (1,699,963 bytes), 50k `297cb41d...c0dc9` (8,416,233), edge `4d2bfc15...2eb719`.

### Page harness (`perf-startup.spec.ts`, binary payloads, 5 runs per case, 2 alternated rounds, median of the two round medians; the harness backend is a timer, so base = the measured on-demand time and early = the measured request-to-answer for a request after the head start: warm 10k 62 / 4 ms, 50k 230 / 21 ms)

| Browser | Library | Case | `load_state` answered base / early | first real row base / early |
|---|---|---|---:|---:|
| Chromium | 10k | first launch | 529 / 105 | 741 / 592 |
| Chromium | 10k | cold | 488 / 110 | 681 / 610 |
| Chromium | 10k | warm | 405 / 142 | 621 / 557 |
| Chromium | 50k | first launch | 568 / 467 | 869 / 826 |
| Chromium | 50k | cold | 706 / 607 | 978 / 947 |
| Chromium | 50k | warm | 682 / 558 | 992 / 932 |
| WebKit | 10k | first launch | 859 / 741 | 1,219 / 1,087 |
| WebKit | 10k | warm | 416 / 370 | 711 / 686 |
| WebKit | 50k | first launch | 1,056 / 979 | 1,530 / 1,483 |
| WebKit | 50k | warm | 716 / 696 | 1,052 / 1,011 |

Read with care: load 14-41 and two rounds only; per-round values moved by 100-500 ms (Chromium 50k warm first row: base 853 and 992, early 932 and 860). The direction is the same in every row (answer earlier, first row 25-150 ms earlier) but the first-row gain at 50k is inside the noise of this harness: the page main thread, not the backend, is the gate for the first row here (R109), and the harness backend is simulated. A cold-disk run in the harness (base 230 / 640 ms against early 4 / 293 ms for 10k / 50k) was dominated by load spikes and is not reported as a result. The saving that does not depend on the harness is the backend table above: about 200 ms at 50k and about 60 ms at 10k leave the path once the page asks later than the thread finishes; on a cold disk everything up to the first-query time overlaps the webview start. Not measured: a real launch on the Mac (no window here).

### Validation

`cargo test -p pioneer-library` and `-p pioneer-test-ui` (294 passed), `cargo check -p pioneer-test-ui --features dev-bins --bins`, vitest (963 passed), `npm run check` (0 errors), smoke in Chromium and WebKit (114 passed each). Not done: the 50k edge library (the edge script copies a 7.5 GB database).
## R136 cover batch (2026-11-06, `r136-cover-batch`)

Branch `r136-cover-batch` from `parked-all-v12` (build 1292, with r132's scroll prefetch). Linux `dev` box (16 cores, load average 13-25 from other workers), headless Chromium and Playwright WebKit. Result: **list thumbnails now come from one backend call per batch of 16 rows (`get_track_artworks`, one binary buffer), three batches in flight. In r132's scroll-fill harness the empty-cover frames fell from 15.7% to 4.7% of the frames at 84 px/frame and from 70% to 4.7% at 280 px/frame (Chromium, 10k and 50k alike); WebKit from 3.5-10% to 0.8-1.2%. Every thumbnail is byte-identical to the single-fetch path (sha256 equal on 1,500 covers, and a Rust test).**

### Measured first: the old baseline was partly a warm-up artefact

r132's harness measured the first speed (84 px/frame) cold: in Chromium the first 50-55 frames of that run were 100% empty and then 0% (`coverSeries`: `100 x9, 0 x12`), which is the mock encoding its first JPEG per size and the first image decodes, not the fetch rate. Its "66 -> 47%" at 84 px/frame was mostly that start. The harness now runs one unmeasured pass at 84 px/frame first (`WARMUP_SPEED`), so both speeds measure the steady fetch rate. Both columns below use this harness; the old store was run in it again for the "before" numbers (the 84 px/frame cold figure of the old harness was 47-51% before and 41-43% after, which shows why it could not tell the two apart).

### Page numbers (`perf-scrollfill.spec.ts`, `FF_PERF_SCROLLFILL=1 FF_PERF_ROW_BANDS=1 FF_SOAK_ARTWORK=1 FF_PERF_TRACKS=<10000|50000>`, mock IPC delay 10 ms per call as in r132, 120 frames per speed, two runs each, mean share of the visible rows without a cover per frame, `framesAffected` in brackets, requests = cover IPC calls during the scroll)

| Browser | Library | Speed px/frame | Before (single fetch, 6 in flight) | After (batch 16, 3 in flight) | Requests before / after |
|---|---|---:|---|---|---|
| Chromium | 10k | 84 | 15.7% (21), 15.7% (21) | **4.5% (6), 4.9% (6)** | 379 / 89 |
| Chromium | 10k | 280 | 70.2% (120), 71.4% (120) | **4.7% (6), 4.7% (6)** | 480 / 128 |
| Chromium | 50k | 84 | 15.1% (20), 15.7% (21) | **4.9% (6), 4.9% (6)** | 380 / 89 |
| Chromium | 50k | 280 | 68.5% (120), 68.9% (120) | **4.7% (6), 4.7% (6)** | 503 / 128 |
| WebKit | 10k | 84 | 4.5% (7), 3.5% (5) | **0.8% (1), 0.8% (1)** | 425 / 93 |
| WebKit | 10k | 280 | 9.3% (24), 8.5% (30) | **0.8% (1), 0.8% (1)** | 1,225 / 132 |
| WebKit | 50k | 84 | 3.8% (5), 4.1% (6) | **1.2% (2), 0.8% (1)** | 425 / 93 |
| WebKit | 50k | 280 | 5.3% (11), 10.1% (28) | **0.8% (1), 0.8% (1)** | 1,255 / 120 |

The 5-6 frames that stay at 100% are the first frames of a run: the first batch has not come back yet (one mock delay plus the page work). Settle time after the scroll stopped is the same (30-80 ms Chromium, 140-220 ms WebKit). Frames in the box are 30 ms or longer under the load, so the scripted scroll is gentler than a real 60 Hz fling; the ratios carry over, the absolute percentages do not.

Batch size and requests in flight (Chromium 10k, two runs each, empty-cover mean at 84 / 280): batch 4 x 6 in flight 8.0 / 7.7%; 8 x 4 6.4 / 5.8%; 16 x 3 4.1-4.9 / 3.9-5.0%; 32 x 2 4.1 / 3.0%. 16 x 3 kept: the 32 batch is a little better at 280 px/frame but makes the first visible rows wait for 32 thumbnails (about 140 ms of backend work cold against 75 ms for 16).

### Backend (`bench_covers`, release, 300 distinct 600 px JPEG covers of 175 KB from a library on disk, 64 px row thumbnails, covers per second, median of 5 rounds, load 23, so read the ratios; cold = fresh thumbnail cache, warm = the same ids again)

| Path | Cold | Warm |
|---|---:|---:|
| single call, 1 in flight | 69 | 4,343 |
| single call, 6 in flight (the old page) | 384 | 5,228 |
| batch 8, 4 threads | 206 | 4,215 |
| batch 16, 1 / 2 / 4 / 8 threads | 66 / 91 / 214 / 312 | 2,656 / 2,417 / 3,777 / 2,960 |
| batch 32, 1 / 2 / 4 / 8 threads | 71 / 132 / 231 / 320 | 2,335 / 2,715 / 3,105 / 2,264 |
| batch 64, 1 / 2 / 4 / 8 threads | 69 / 107 / 169 / 280 | 2,132 / 2,310 / 2,026 / 2,655 |

The backend is not what the batch speeds up: a cold thumbnail is one JPEG decode and resize, 14 ms on this loaded box, in every variant (one thread: 66-71 covers/s however it is called). The batch matches the old path only because it decodes on 4 threads per call (`BATCH_THUMBNAIL_THREADS`) and the page keeps 3 calls in flight; a cold batch of 16 takes about 75 ms. Warm calls are 20-50% slower per cover than the single path (thread start and one output buffer per call, 0.25-0.4 ms per cover), which is far below one frame for the 30 covers a frame needs. What the batch removes is the per-call cost: IPC round trip, a `setTimeout`-sized wait per call in the mock, one blob and one task per cover in the answer path, and the library lock taken once per batch instead of once per cover. Scrolling 10k rows cold now needs 15% of the calls (89 against 379).

Not done: a decoded-thumbnail cache that survives the 400-entry memory cache (that is storage, a schema or file layout change) and a faster decode (DCT scaling changes pixels).

### Identity

`bench_covers identity` writes the 64 px thumbnail of each of 1,500 distinct covers through the single path (`cached_artwork_thumbnail`, what `get_track_artwork` calls) and through `thumbnail_batch` (batches of 32, 4 threads), then each file is hashed with sha256: **1,500 of 1,500 equal** (digest of the digests `baeb61887e0765ed70094a5f456b7f65ab41287a55be5a22db4e6a7ba9cccef5`). Unit tests: `artwork_thumbs::tests::batch_entries_are_the_bytes_of_the_single_track_path` (JPEG and PNG covers above and below the box, garbage bytes, a track without a cover, an unknown id; 1, 3 and 16 threads; cached second call) and `decodeArtworkBatch` round trips in the page. The page makes a blob of the same bytes it made before.

### Changed

- `pioneer-test-ui/src/artwork_thumbs.rs` (new; the thumbnail cache and `artwork_thumbnail` moved out of `main.rs` unchanged so the bench can share them): `stored_covers` (reads the covers under one library lock), `thumbnail_batch` (the thumbnails on scoped threads, one buffer: `u32` count, one `u32` length per track with 0 for none, the images in request order, little-endian).
- `main.rs`: command `get_track_artworks(track_ids, max_px)` (async, `spawn_blocking`, binary `tauri::ipc::Response`). `get_track_artwork` is unchanged and still serves the 512 px covers.
- `app`: `getTrackArtworks` in `tauri.svelte.ts`, `lib/artworkBatch.ts` (decode), `artwork.svelte.ts` pumps the row queue in batches of 16, 3 in flight, most urgent first as r132 ordered it (the focus, queue limit and drop rules are untouched). Smoke mock answers `get_track_artworks`; `perf-scrollfill.spec.ts` counts both commands and warms up.
- `bench_covers` dev bin (`gen`, `run`, `identity`).

Validation: `cargo test -p pioneer-test-ui` 293 passed, 11 ignored; `npx vitest run` 1,017 passed, 3 skipped; `npm run check` 0 errors; smoke Chromium 161 passed, 28 skipped; smoke WebKit 161 passed, 28 skipped. Scratch (`~/.cache/r136*`) is deleted after this item.

## R137 sort during analysis (2026-11-06, `r137-sort-during-analysis`)

R129 lever 4: a sort click at 50k while analysis runs took 89-154 ms against 72-74 ms idle. Branch from `ov-1028`. Linux `dev` box at load 13-26 (other workers), headless Chromium and Playwright WebKit, real r95 payloads, base = `ov-1028` in a separate copy of the tree (same spec and test files), runs interleaved. New harness `app/smoke/perf-sort-analysis.spec.ts` (`FF_PERF_STATE`, `FF_PERF_MODE=idle|analysis`, `FF_PERF_AGE` seconds of analysis before the first click, `FF_PERF_STOP=1` stops the events before the clicks, `FF_PERF_PROFILE=1` writes a CPU profile of the clicks); a `requestAnimationFrame` loop keeps frames running (R134), `analysis-partial` events arrive at 6 per second (the app applies them in batches, at most 4 bumps per second), then 12 steps: seven first sorts of columns (artist, title, bpm, key, time, label, both directions of artist), sorted typing and clearing, two more sorts.

### Cause (measured)

- Two parts of the R129 guess hold, one does not.
- **Holds: stale column orders were thrown away.** `bumpLog` kept the last 64 bumps. A cached column order (R115 warm-up) is brought up to date lazily when that column is sorted, from the bumps since it was built. After 64 bumps (about 16 s of analysis at 4 bumps per second) every cached column that was not the one being sorted had no usable history, `dropUnusableColumnSorts` deleted all of them and the first click on each rebuilt it from scratch (read 50k values, value classes, sort): title 100-120 ms of script, the other columns 15-40 ms more. Even inside the log, `withChangedValues` gave up past 64 moved rows and sorted anew.
- **Does not hold: the search warm-up restart.** `scheduleSearchIndexWarmup` does restart on every bump, but typing during analysis is not slower than idle in the same harness (sorted type 'a': idle 100 ms total, analysis 91 ms, base, 50k).
- **Left as is: per-bump work while a sort is active** (the derived chain: `sameMembers` over 50k rows about 1.2 ms, `withChangedValues` and the rank array about 1.5 ms, GC, in all about 8-10 ms per bump in a profile of the sorted phase). A click that lands next to a bump pays part of it: that is the remaining 20 ms of script over idle. It is not specific to clicks and was not touched.

### Changed (no markup, no CSS, same rows, same order)

- `app/src/stores/library.svelte.ts`: the bump log keeps up to 4096 bumps and 16,384 ids (was 64 bumps), so a column order built before a long stretch of analysis is moved to the current list instead of dropped; `libraryPositionsChangedSince` users (filters, sort values, playlists) get the same longer memory.
- `app/src/lib/sortOrder.ts`: `withChangedValues` removes the changed positions, sorts them by value and position and merges them into the rest in one pass (a binary search per moved row, then block copies) instead of one `copyWithin` per row; `MAX_REPOSITIONED` 64 to 4096, counted after removing repeats (a stale column sees the same row many times).
- Tests: `sortOrder.test.ts` (hundreds of moved rows with repeats, ties, blanks, both directions), `sort-analysis-real.test.ts` (env-gated, below).

### Before / after

Chromium 50k, analysis running (6 events per second, 20 s before the first click), 4 interleaved pairs, ms, median over runs:

| Step | total base | total new | script base | script new |
|---|---:|---:|---:|---:|
| sort title asc | 168 | 92 | 121 | 52 |
| sort artist asc | 114 | 134 | 62 | 56 |
| sort artist desc | 102 | 100 | 59 | 42 |
| sort bpm asc | 96 | 192 | 46 | 52 |
| sort key, time, label asc | 96, 92, 88 | 128, 88, 90 | 50, 44, 41 | 50, 42, 47 |
| all 7 first sorts, median per run | 95-121 | 88-100 | 37-57 | 43-53 |
| sorted type 'a' / clear | 91 / 84 | 90 / 84 | 27 / 27 | 34 / 32 |

Same harness with the events stopped 1.5 s before the clicks (3 pairs; isolates the stale-column cost from the bumps landing in the click window): title asc total 154 to 88, script 101 to 31; bpm script 42 to 28; every other step equal. The idle run of the same spec: sort clicks 64-144 total (median about 82), script 16-32 (median about 24). The totals of single steps swing by 100-300 ms on this box (the 192 and 378-407 ms new values in single runs are load, the script column did not move); read the script column and the stopped-events table.

Chromium 10k, 3 pairs: sort-click median per run total 88.5-90 to 72-82, script 26-35 to 24-28. WebKit 50k, 2 pairs (`total` only, compositor floor of R29): 236 and 248 to 216 and 260, inside the noise.

So the 89-154 ms of R129 is not fully gone: the cold-column rebuild (the part with a code cause) is, the click next to a bump costs about 20 ms of script more than idle.

### Identity

- Store level (`sort-analysis-real.test.ts`, `FF_PERF_STATE=<dump> npx vitest run src/stores/sort-analysis-real.test.ts`, the same file in both trees): after a warm-up of 11 columns, hashes of the ordered ids of every column, both directions, with the queries "", "house" and "a" (66 views) after each of 6 scripted bump steps (1 bump, 3 bumps of 2 rows, 70 bumps, 130 bumps of 3 rows, 500 bumps, one bump of 5,000 rows; rows get new BPM and key, some a new artist, title, genre or rating), and the stale case (reload, warm-up, then 80 bumps before each column is sorted, two rounds over all 11 columns): **601 of 601 hashes equal base and new at 10k and at 50k** (172-174 distinct fresh views).
- `sortOrder.test.ts` compares moved orders with a comparator sort; `search-sort-fuzz.test.ts`, `playlist-sort-fuzz.test.ts` and `bulk-bump-fuzz.test.ts` pass unchanged.
- Page level: `perf-sort-analysis` records a hash of the rows on screen after every step; the runs with the events stopped are equal between base and new, the runs with events change with the event timing by design.

### Not done

- Per-bump cost while a sort is active (`sameMembers` over the whole list, the order and rank rebuild per bump): about 1 ms each at 50k, left for a later item.
- Memory of the log: at most 16,384 ids plus 4,096 small arrays.

Scratch (`~/.cache/r137`) is deleted after this item.

## R138 allocator env (2026-11-06, `r138-alloc-env`)

R106's three glibc variables (`MALLOC_ARENA_MAX=1 MALLOC_MMAP_MAX_=0 MALLOC_TRIM_THRESHOLD_=1073741824`) are set for the Linux `bench_analysis` runs in `scripts/perf.sh` only, through a `bench_analysis` wrapper that applies them when `uname -s` is Linux. The app, the macOS path and the startup and load steps are unchanged. `benchmark/scripts/` has no Linux `analyze-json` or `bench_analysis` caller (its Python batch runs `fourfour-analyze`). `fourfour-analysis-rs/scripts/parity_run.py` runs `analyze-json` but is outside this scope and is not changed.

Identity and cost: `analyze-json --threads 2` on 10 tracks (every 63rd file of `benchmark/datasets/rekordbox-testdata-2025`: 4 MP3, 4 WAV, 2 FLAC), one run each, base then env, box load average 16-37 from other workers.

| | user | sys | CPU | wall | output sha256 |
|---|---:|---:|---:|---:|---|
| base (default glibc) | 715.7 s | 145.2 s | 860.9 s | 709.1 s | `2b9c235b55f5625bcd1c35dd1f0f157bc30f50ed4fcfb9c51a7879487bf8fa12` |
| env (R106 variables) | 689.5 s | 4.4 s | 693.9 s | 627.9 s | `2b9c235b55f5625bcd1c35dd1f0f157bc30f50ed4fcfb9c51a7879487bf8fa12` |
| change | -3.7% | -97% | **-19.4%** | -11.4% | equal |

Same as R106 in direction: sys time goes to near zero and CPU drops about a fifth. Wall moved less than in R106 (-11% against -24%) under heavier, uneven load, and only one pair ran, so read the wall number as a rough figure. Peak RSS was not measured in this run (R106: +10%).
## R139 bump cost (2026-11-06, `r139-bump-cost`)

R137 left the work every analysis bump does on the whole library (4 bumps per second while analysis runs): `sameMembers` over every row, a rebuild of the sort order and rank of both directions, the array of filtered rows. Branch from `ov-1028` (has R137). Linux `dev` box at load 15-40, real r95 payloads, base = `ov-1028` in a separate tree with the same spec and test files, runs interleaved.

### Change (no markup, no CSS, same rows, same order)

- `app/src/modules/TrackTable.svelte`: "same rows as the last sort?" asks `sameLibraryMembers` first (new, `library.svelte.ts`): two lists of library rows (`library.tracks`, a `filterTracks` result, a playlist's rows) are the same when both sit in one unbroken bump chain and have the same positions, an integer compare or a reference compare. Only when that cannot be told is the `Set` built, lazily, as before.
- `app/src/lib/sortOrder.ts`: `moveChangedValues` moves up to 16 changed rows in `order` itself (`indexOf`, a binary search and one block move per row; no new arrays). More rows still go through `withChangedValues`. `SortedOrder` is a class whose `rank` is built when first read and dropped when `order` moves, so a bump no longer rebuilds 50k ranks nobody reads; `sortedSubset` without a built rank takes a mask pass over `order` instead of ranking and sorting the subset.
- `app/src/stores/table-columns.svelte.ts`: each direction of a column order keeps the list it is valid for and is brought up to date when that direction is sorted (was: both directions on every bump, `columnSortOf`).
- `app/src/stores/library.svelte.ts` `withChangedMatches`: when the bumped rows match the query exactly as before, the result indices stay the same array and the row array is a `slice` of the earlier one (was: a filter and a merge over every match, then a map over all of them).
- Tests: `sortOrder.test.ts` (in-place moves over many bumps, rank dropped and rebuilt, subset with and without rank), `search-sort-fuzz.test.ts` and `playlist-sort-fuzz.test.ts` (every `sameLibraryMembers` claim must be the same rows in the same order, over 900 and the playlist steps), `bump-cost-real.test.ts` (env-gated, the measurement below), `perf-sort-analysis.spec.ts` (`FF_PERF_QUIET=<seconds>`: script and task time of a window of analysis with no input, after the clicks).

### Before / after, store level (`bump-cost-real.test.ts`, Node, real payload, a bump of 2 rows then the view the table computes, 400 bumps, median of 3 interleaved runs, ms per bump; "bump" = write the fields and `bumpLibraryTracks`)

| Case | 10k base to new | 50k base to new |
|---|---|---|
| sorted by artist, whole view | 1.21 to 0.46 | 8.83 to 2.38 |
| sorted by bpm, whole view | 1.15 to 0.29 | 6.17 to 2.10 |
| sorted by artist, query "house", whole view | 1.09 to 0.73 | 4.68 to 2.46 |
| of which same-rows check (artist) | 0.61 to 0.03 | 5.58 to 0.03 |
| of which sort (artist) | 0.75 to 0.33 | 2.83 to 1.44 |
| of which filter (query "house") | 0.42 to 0.11 | 1.26 to 0.12 |
| GC per bump (artist / bpm / query) | 0.15 to 0.10 / 0.04 to 0.06 / 0.45 to 0.26 | 0.37 to 0.25 / 0.34 to 0.36 / 0.55 to 0.30 |

The `Set` check was the largest part at 50k (Set lookups of 50k row proxies, 5.6 ms in Node dev mode; the R137 estimate in Chromium was 1.2 ms). What is left per bump at 50k is the array of rows the sort hands to the table (about 1.4 ms, a new 50k array each bump, needed because the table compares the old and the new one) and `trackList.slice()` in the bump itself (about 0.9 ms); both are copies of 50k pointers and stay linear.

### Before / after, page level (`perf-sort-analysis.spec.ts`, Chromium, 6 events per second, 20 s before the first click, median over runs; base n=5, new n=6 at 50k)

- Script time of a 12 s window of analysis with the table sorted and no input: 34.9 to 30.5 ms per second at 50k (noisy, one set of three pairs 34 to 22); 10k (2 pairs) 24.6 to 23.6. Per bump that is about 8 ms of script at 50k that is now mostly the table (effects, rows), not the library size. Task time (includes GC and layout) did not move beyond the noise.
- Sort click, script ms, all sort steps pooled: 43 to 28 at 50k with analysis (idle, 2 pairs: 29 to 18, the lazy rank also saves the rank build of a first sort); 10k 29 to 28. Totals (click to second frame): 89 to 82 at 50k, inside the 70-250 ms swing of this box.
- Typing in the sorted table, script ms: 33 to 38 (type 'a' 26 to 31, 'm' 64 to 64, clear 31 to 25), no change beyond noise.
- WebKit 50k analysis, 2 pairs: sort click totals 267 to 279 ms, typing 355 to 371 (no script metric in WebKit; the compositor floor of R29 dominates), no difference.

### Identity

- Store level (`sort-analysis-real.test.ts`, the R137 test, same file in both trees): **601 of 601 hashes equal base and new at 10k and at 50k** (242 and 240 distinct views).
- `search-sort-fuzz.test.ts` (with the new soundness check), `playlist-sort-fuzz.test.ts` (same), `bulk-bump-fuzz.test.ts` and `sortOrder.test.ts` pass.
- Page level, events stopped before the clicks (`FF_PERF_STOP=1`, `FF_PERF_AGE=15`): the rows on screen after each of the 12 steps are equal between base and new at 50k and 10k (12 of 12 each).

### Validation

- `npx vitest run`: 109 files passed, 9 skipped; 967 tests passed, 8 skipped.
- `npm run check`: 0 errors, 0 warnings.
- Chromium smoke (`FF_SMOKE_PORT=5551`): 114 passed, 33 skipped (perf specs without a payload).
- WebKit smoke (`FF_SMOKE_BROWSER=webkit FF_SMOKE_PORT=5552`): 114 passed, 33 skipped.

### Not done

- The array of rows handed to the table is still built per bump (linear, about 1.4 ms at 50k); patching the previous array would save about half of it and needs the moves reported out of `moveChangedValues`.
- The bump itself (`trackList.slice()`, version log) and what the table does per bump (effects, row updates) are not touched.
- A stale direction (not sorted for more than 16 rows of bumps) still goes through `withChangedValues`, linear, as in R137.

Scratch (`~/.cache/r139*`) is deleted after this item.

## R143 budget (2026-11-07, `r143-budget`)

Read-mostly audit after R124-R142: where the time goes now, to rank the next speed levers. Branch `r143-budget` from `parked-all-v14` (build 1327, `e63aa0f`). No product code changed. Linux `dev` box, release `bench_startup` and `bench_analysis` in `~/.cache/r143-target`, headless Chromium 1217 and Playwright WebKit, the `ff-fixture` 10k and 50k libraries (read only; payloads from `bench_startup dump` and `dump-bytes`, same sizes as R135: 1,715,152 and 8,319,832 bytes). **The box was at load 50-62 for the first hour (my own release build plus other workers), 6-15 for the rest.** Everything from the load 50+ phase (a first `perf-startup` run: 10k first row 1.7 s, 50k 2.3 s; a first `perf10k` run) was thrown away and rerun at load 6-15 (the WebKit startup run was at load 31 and is noted below). Frames run in every harness (R134): `perf-startup` has them, `perf-sort-warmup`, `perf-sort-analysis` and `perf-import` run a `requestAnimationFrame` loop, `perf-warmup` got one (`FF_PERF_FRAMES=1`). The backend in `perf-startup` is simulated as the R135 early answer (5 ms at 10k, 22 ms at 50k).

Harness added: `app/smoke/perf-import.spec.ts` (a drop of a 1,000-file folder in 10 subfolders on the sidebar, the real 10k or 50k payload loaded, mock `plan_import` held, then released; `FF_PERF_STATE`, `FF_PERF_IMPORT_IDLE` (ms before the first sort and key, 3000 or 300), `FF_PERF_IMPORT_FILES`, `FF_PERF_PROFILE_JSON`, `FF_PERF_TRACE`); `perf-warmup.spec.ts` got `FF_PERF_FRAMES`. The 1k folder is hard links of fixture files under `~/.cache/r143-import`; the backend side is `bench_startup import`.

### Budget by flow (Chromium unless noted; `x / y` = 10k / 50k; medians, load 6-15 unless noted)

| Flow | Current total | Top 3 cost centres | Class |
|---|---|---|---|
| Cold open to first row | 543-610 / 838-920 ms (R129 603-626 / 1,031-1,147; R135 592 / 826); WebKit 1,364-2,220 / 2,053-2,582 at load 31, software-compositing floor of R29 | 1. mount of `App`, effects, first `TrackRow`s, `Sidebar` rows (R121: 65 + 34 + 39 + 32 ms; profile 285 ms inclusive at 10k) 2. the search text build starts at once: `trackHaystack` + `markCharacters` + `fillSearchIndex` 182 ms self (10k) / 496 (50k) in the first seconds 3. GC 450 / 905 ms over the first seconds (150k row strings at 50k; plain rows, `$state` rows and haystacks) | b, a, a |
| Cold open to interactive | 601-697 / 879-922 ms; longest task 196-266 / 209-265, TBT 264-314 / 271-340 | same as first row; the long task is the mount | b, a |
| First search key (frames running, key 1.5 s after the rows) | 100-113 / 128-148 ms total, script 29-33 / 38-51; `perf10k` first key 76 / 85, median 45 / 59; WebKit 620-757 / 523-845 | 1. compositor and paint floor (about 70 ms of any step) 2. layout and paint of 42 rows 3. filter plus the haystack warm-up still running at 50k | c, b, a |
| First sort click (8 s after the rows) | 99-111 / 130-186 ms, script 20-22 / 26-37; `perf10k` 72 / 72; WebKit 392-504 / 172-222 | 1. floor 2. sort of the column (cached after the warm-up) 3. paint of the rows | c, a, b |
| Sort click and keystroke during analysis (6 `analysis-partial` per s, 20 s old) | sort 67-100 / 72-100 ms (title asc at 50k 171 once), script 20-36 / 24-36; typing 'a' in the sorted view 81 / 86; **'m' (second key): script 35 / 58-74 against 19-27 / 28 for the first key** | 1. floor 2. per-bump work in the sorted view (R139: 2.4 ms per bump at 50k) 3. the second key of a query in a sorted view at 50k | c, a, a |
| Fast scroll fill (`perf-scrollfill`, 10 ms mock IPC) | rows missing 0.8%, previews empty 0.9-1.7%, covers empty 4.2-5.1% (6-7 frames, the first ones of a run) at both speeds and both sizes; settle 48-93 ms; 89 / 128 cover calls; frame p50 17-20, p95 27-31 ms, flat in library size (`perf10k`) | 1. first batch not back yet (one mock delay) 2. row canvas layers (R118) 3. batch decode 75 ms cold per 16 | c, b, a |
| Open inspector (select a row) | 14.6 / 14.2 ms median, 40 worst (`perf10k`) | store update, bindings | floor |
| Load a track into the player, expand | backend `get_analysis_data` 1.2 ms median (p95 2.8), analysis plus detail in parallel 2.0 ms; page overview painted 42 / 91 ms after the answer; expand to detail drawn 97-108 / 101 ms, longest task 50 ms (`perf-q50`) | 1. page paint after the answer 2. expand draw (compositor-bound) 3. first-time detail of a track without a stored series (decode, R128/R109) | a, c, b |
| Import 1k folder, new import flow, backend | `survey` 4-6 ms, scan 45-183 ms, plan 11-141 ms (141 with 1,000 duplicates into the 10k library), commit 60-84 ms: **rows after 144-261 ms** (empty library), 287 ms (10k, all duplicates); covers 2.0-2.6 s for 818 covers afterwards (`bench_startup import`, warm) | 1. tag scan 2. duplicate plan 3. cover pass (about 3 ms per cover) | c, a, b (per-album dedupe needs schema) |
| Import 1k, page: drop to placeholder rows | **53-55 ms to the DOM, 79-84 ms to the frame** at both sizes (WebKit 77-125 ms to the DOM, 226-297 to the frame) | `survey_import` answer, playlist layout, 1,000 pending names | floor |
| Import 1k, page: commit answer to real rows | **186 ms (10k) / 560 ms (50k) in one long task** (237 / 616 ms long task), frame 334 / 700 ms; WebKit 196-252 / 567-797 (frame 506-619 / 904-1,308); the older harness saw the same: `perf-q50` "rows after commit answer" 67-88 / 307 ms | 1. `importedTrackIds` to `trackIdsUnderPaths`: `normalizePath` and `isInside` over every library row (300 ms of 763 busy ms in the profile at 50k) 2. `fillPlannedPlaylists` to `trackIdsForFiles`: a path map over every library row, `rowEntryOf` creates row state for all of them (149 ms, 106 of it `createRowState`) 3. `landImportedTracks`: `concat` of two 50k arrays, `resetSearchIndex` | a, a, a |
| After the import: next 3 s, first sort, first key | idle script 129 / 486 ms in the 3 s (steady state is 3-4 ms per s); first sort of a column right after: script 15 / 116 (50k: 84-139 against 16 when the 3 s are waited); first key 24 / 41 | `resetSearchIndex` drops all haystacks and `scheduleSearchIndexWarmup` rebuilds all 10k / 50k; column orders are rebuilt on first sort | a |
| First analysis result after import (backend, 6 long tracks of 343-413 s, 4 workers, load 12) | first full result 16.1 s, all waveforms 6.8 s, all keys 27.3 s, all grids 67.8 s; 5.3 tracks per min; CPU 320 s (53 per track); peak RSS 697 MB. With the 10 s fixture clips: first result 1.7 s | beat model 90% (R129, R104), mel and resample 8%, `Meter::push` 3.4% | c, c, b |
| Idle CPU, page open (`perf-warmup`, frames on) | script ms per s: 10k 130-149, 14, 4, 4, 14-19, 4, 4, 3, 3, 12-15 (201-206 in 10 s); 50k 477-484, 186-217, 4, 4, 13-17, then 3-4, 11-16 at second 10 (709-755 in 10 s); no long task | the search text build and the sort warm-up in idle chunks; a 12-19 ms task every 5 s | a |
| Memory after open | JS heap 15.5-16.9 MB / 48-59.5 after boot, 33.9 / 124 at the end of the `perf10k` script (synthetic rows) | the track table: per-row strings and `$state` rows | a |

Not repeated: cold-disk start (R129: 1.5 s / 4.1 s for the first query on a contended disk; R135 hides up to the head start), Mac start (R133: quiet Mac 609 / 657 ms), WebKit scroll fill (R136: 0.8-1.2%).

### Findings that are not in the table

- **`ScriptDuration` does not see promise continuations.** The import commit task showed script 102-136 ms and a 560-616 ms long task, with the CPU profile pointing at JS in microtasks (`importDroppedTracks`, `fillPlannedPlaylists`). Budget rows that say "script" under-count async store work; use long task and task time for those.
- **Both halves of the import flow are different**: the placeholder phase is cheap and flat in library size (53 ms), everything after the commit answer scales with the library (186 to 560 ms for the same 1,000 files), because three steps walk the whole library to find 1,000 rows.
- **The R129 levers now**: 1 (bulk bump) landed as R130 (analyze 2k selected: long task 140-183 ms; 50k select-all about 2.4 s, from 9.4 s); 2 (sort warm-up from typed columns) was refuted by R134; 3 (early state) landed as R135; 4 (sort under analysis) landed as R137 and R139 (sort click 67-100 ms, near the 72 ms floor); 5 (allocator env) landed as R138 for scripts.
- The box swing is still 2-3x between load 6 and load 50: a scroll p95 of 65 ms at 10k and 35 at 50k in the same session was load, not size.

### The 5 best next levers of kind (a)

Ranked by gain per effort. "Proof" is how to show the same rows, the same order, the same bytes. Gains marked est. are read from profile shares, not yet measured as a change.

| # | Lever | Expected gain | Effort | Proof of identity |
|---|---|---|---|---|
| 1 | **Find the imported tracks without walking the library** (`importedTrackIds` / `trackIdsUnderPaths` in `library.svelte.ts` and `lib/fileDrop.ts`, `trackIdsForFiles` in `lib/importPlaylists.ts`, `fillPlannedPlaylists` in `stores/file-drop.svelte.ts`): one path index of the library built once (or only the added rows plus the skipped-duplicate matches, plus a prefix lookup for dropped folders), `normalizePath` once per distinct path, no row state created for rows nobody reads | the commit-answer long task after a 1k import 560-616 ms to about 150-250 ms (est.) at 50k and 237 to about 100 ms at 10k; it scales with the library today and would scale with the import | S | vitest: random libraries and dropped paths (trailing slashes, case, nested folders, files and folders mixed, duplicate matches, ids out of library order), `importedTrackIds` and `trackIdsForFiles` equal to the current functions; saved playlist `track_ids` equal after the drop in the smoke specs; `perf-import` on-screen row hash after the import, base against new, both sizes, both browsers |
| 2 | **Extend, do not rebuild, the search text and the column orders after an import** (`resetSearchIndex` in `landImportedTracks`; `appendLibraryTracks`): keep `haystackList` and the character masks and fill only the appended rows (they sit at the end; the typed arrays grow); bring each cached column order up to date with the appended rows the way `withChangedValues` does for bumps | 50k: the 486 ms of script in the 3 s after an import to about 20 ms, the first sort of a column right after 84-139 to about 16 ms of script, the first key 41 to 24; 10k 129 to about 10 ms. Applies to every import and every add | M (index S, orders M) | hashes of every haystack string and of every sorted order (11 columns, both directions, queries "", "house", "a") after the import equal to a full rebuild, on the 10k and 50k payloads: the R137/R139 `sort-analysis-real.test.ts` pattern with an append step, plus `search-sort-fuzz.test.ts` with appends |
| 3 | **Bulk field writes without a proxy `set` per field** (`markTracksPendingAnalysis` / `bumpLibraryTracks` after R130): 1.3 s of the 2.4 s that select-all-and-analyze still takes at 50k is field writes through the row proxies (Svelte `set`, row state creation), 0.3 s the plain copy, 0.25 s the search text | analyze 2k selected 140-183 ms to about 80 ms (est.); 50k select-all 2.4 s to about 1.2 s (est.); the same path serves rating, colour, tags and bulk edits | M | the R130 hashes: plain rows, 24 sort orders, 43 searches, on-screen rows after each bulk step at 2k, 10k, 50k, plus `bulk-bump-fuzz.test.ts` (`FF_FUZZ_FULL=1`) |
| 4 | **Cheaper search text** (`trackHaystack`, `markCharacters`): about 500 ms self at 50k and 182 ms at 10k in the profile of the first seconds, with the GC it feeds (905 ms at 50k): ten concatenations and one `toLowerCase` of the whole string per row; memoise the lowercase of each distinct title, artist, album, genre, label and key, and share the strings across rows | 50k first-seconds script 700 to about 450 ms (est.) and less GC; 10k 200 to about 140 ms; idle CPU only, the first row does not move (R134: work that is only moved later changes nothing for the first key) | S-M | haystack strings of every row equal to base on the 10k and 50k payloads and the edge payload (hash of the list), and the 66 order hashes of `sort-warmup-real.test.ts` |
| 5 | **Second key of a query in a sorted view at 50k** (`withChangedMatches`, `sortedSubset`, the filter history in `library.svelte.ts` and `sortOrder.ts`): typing 'm' after 'a' costs 58-74 ms of script against 28 for the first key (10k: 27 against 22), total 135-143 ms against 99 | about 30-40 ms per further key at 50k (est.: 135 to about 100); profile first, the cause is read from the numbers, not from a trace | M | the R113 method: on-screen rows and the ordered ids of every step of a typing script (sorted by each column, both directions) equal to base, `search-sort-fuzz.test.ts` |

Looked at, not on the list: a 12-19 ms task every 5 s (launch analysis delay, 3 ms per second of the 10 s window); opening a 2k playlist 144-153 ms against a 72 ms floor (script 19-33, layout 8-10; the rest is not attributed yet: profile first, up to about 70 ms); the first-row path (design-system rows, class b below); the `load_state` backend 69-81 / 332 ms warm is hidden by R135 behind the webview start.

### Best (b) items, for the fences

| Fence | Item | Gain (as measured by the item named) |
|---|---|---|
| `prototyping/` (`waveform-display.js`) | Compositor-driven playhead (R112 proposal: a paused `el.animate` instead of `style.transform` every frame) | with the expanded player open and a track playing: Layerize 658 to 27 ms per 1.5 s, about 7 ms of every 17 ms frame (40% of the main thread in headless Chromium); canvases identical (72 of 72 in Chromium; the WebKit cost is unmeasured) |
| `prototyping/` (`TrackRow`, `SidebarRow`, `Icon` first run) | A cheaper first run of the design-system components; the mount of the real rows | the largest piece of the 540-610 ms cold first row at 10k that is not floor: mount 65 + first `TrackRow`s 39 + `Sidebar` 32 ms (R121); a deferral changes the DOM at the first-row moment, so it needs Merlin |
| `prototyping/` (`Waveform.svelte`, `TrackRow.svelte`) | Row waveforms as images (R118) | fling warm -24% main thread busy, Layerize -61% while playing; Chromium not byte-identical, memory +130-150 MB at 10k; not recommended before a Mac measurement |
| schema (`r84-path-keys`) | Per-album cover dedupe | cover pass after an import: 2.0-2.6 s for 818 covers at 1k, 7-10 s at 10k (1.5 GB of blobs); bound by the distinct albums, so up to several times fewer decodes (not measured) |
| `pioneer-usb-writer/src` (`Meter::push`) | 4x faster waveform meter (R104: 4x headroom) | 3.4% of analysis CPU (about 140 ms per audio minute in the stage profile), so about 3% of throughput |
| Mac-only | CoreML analysis, WebKit start and frames | R26/R30 numbers; the headless WebKit rows above are the software-compositing floor (R29) |

Class c, for the record: the 72 ms floor of any sort or key step (compositor and paint in headless), webview start, the beat model (90% of analysis CPU), rubato, the folder walk, a cold disk.

Scratch (`~/.cache/r143*`, the release target) is deleted after this item.

## R144 import commit (2026-11-07, `r144-import-commit`)

R143 levers 1 and 2. Branch from `r143-budget` (build 1327). Linux `dev` box at load 9-17 (other workers), headless Chromium and Playwright WebKit, the r95 10k and 50k payloads, harness `perf-import.spec.ts` (1,000 files in 10 folders dropped on the sidebar, frames running; it now also records a hash of the rows on screen and of the saved playlists' `track_ids`). Base = `r143-budget` copied to a second tree with the same spec; runs: 3 per case in Chromium, 2 in WebKit, base and new alternated by case.

### Changed (no markup, no CSS, same rows, same order)

- `lib/fileDrop.ts` `trackIdsUnderPaths`, `lib/importPlaylists.ts` `trackIdsForFiles`: read the plain rows (`libraryPathRows`, new) so no row state is created for rows nobody reads (106 of the 149 ms of `fillPlannedPlaylists`); `normalizePath` returns the string at once when it has no backslash and no trailing slash; the folder prefixes are built once; `trackIdsForFiles` takes the files it will be asked about and keeps only those paths. One pass over the library remains (a string compare per row, a few ms at 50k); a path index was not built because `filePath` changes on relink.
- `stores/library.svelte.ts`: `landImportedTracks` extends the search text instead of resetting it (`extendSearchIndex`: the haystacks and the letter and digit masks built so far stay, the typed arrays grow, the warm-up fills the new rows). An append is recorded in the bump log (`appended`); `libraryChangesSince` returns the changed positions and where the appended rows start, `libraryPositionsChangedSince` still returns `null` across an append, so playlist views, filters and member checks rebuild as before.
- `lib/sortOrder.ts` `withChangedValues` accepts appended values (positions from the order's length on) and merges them with the changed rows in one pass; `stores/table-columns.svelte.ts` keeps each column's values and both directions across an append (values read for the new rows only, each direction moved when it is next sorted).

### Before / after (Chromium, ms, median (min-max) of 3; `idle` = wait after the rows before the first sort and key)

| Metric | 10k base | 10k new | 50k base | 50k new |
|---|---|---|---|---|
| release to real rows | 205 (190-226) | 124 (116-138) | 568 (566-569) | 107 (103-110) |
| release to frame | 339 | 279 | 691 | 230 |
| script in the 3 s after the import | 149 (143-158) | 37 (36-43) | 456 (452-538) | 30 (28-33) |
| first sort title script, idle 3 s | 19 | 30 | 17 | 33 |
| first sort title script, idle 0.3 s | 17 | 27 | 113 (111-133) | 32 (28-38) |
| first key script, idle 3 s / 0.3 s | 29 / 26 | 24 / 25 | 25 / 25 | 22 / 24 |
| script in 0.3 s idle window | 52 | 6 | 88 | 7 |

WebKit (release to rows / to frame, 2 runs): 10k 302 / 760 to 169 / 516; 50k 615 / 1,072 to 195 / 572.

Read with care: the first sort now pays a lazy move of the 1,000 rows in the order (about 15 ms at 50k in Node, 10-15 ms more script in the page than a column that was rebuilt in idle), so with a 3 s wait it is a little slower than base (17 to 33 at 50k) and with a 0.3 s wait much faster (113 to 32). The longest task in the window stays 70-87 ms in both (the measure sees continuation tasks only partly, see R143). The 16 ms estimate of R143 was not reached.

### Identity

- `fileDrop.test.ts` and `importPlaylists.test.ts`: the new `trackIdsUnderPaths` and `trackIdsForFiles` against the old walks on 400 and 300 random libraries (trailing slashes, backslashes, nested folders, duplicate paths, ids out of library order, matches to rows that are not in the library): equal ids and order.
- `import-append-fuzz.test.ts` (40 random scripts of appends, bumps and checks, columns warmed first or not) and `import-append-real.test.ts` (`FF_PERF_STATE=<dump>`, 1,000 + 1 + 250 rows appended with bumps between, 2 checks): every haystack (one hash of the list) and 12 columns x 2 directions x 4 queries plus the unsorted view equal to the same library loaded whole. 10k and 50k: no mismatch. Breaking the appended read, the tie order or the index extension each makes the fuzz fail (checked). `sortOrder.test.ts` has a random append test.
- Page level: the hash of the rows on screen and of the saved playlists' `track_ids` after the import is equal between base and new in every run, both sizes, both browsers (10k `bcab9ba63cb8` / `bc34bf672e3e`, 50k `1952cd408dff` / `c5d0858d1bac`).
- `search-sort-fuzz`, `playlist-sort-fuzz`, `bulk-bump-fuzz` (`FF_FUZZ_FULL=1`) green.

### Validation

- `npx vitest run`: 118 files passed, 10 skipped; 1,044 tests passed, 9 skipped.
- `npm run check`: 0 errors, 0 warnings.
- Chromium smoke (`FF_SMOKE_PORT=5561`): 161 passed, 35 skipped. WebKit smoke (`FF_SMOKE_BROWSER=webkit FF_SMOKE_PORT=5562`): 161 passed, 35 skipped.

### Not done

- The library is still walked once per import (path compare, a few ms at 50k); a path index needs invalidation on relink.
- The lazy order move could run in the sort warm-up to take the 10-15 ms off the first click.

Scratch (`~/.cache/r144*`) is deleted after this item.
## R145 bulk writes (2026-11-07, `r145-bulk-writes`)

R143 lever 3, from `ov-1028`. Linux `dev` box (load 13-38), headless Chromium and Playwright WebKit, r95 50k payload. Base and new runs back to back (new, base, base, new).

### Change

- `markTracksPendingAnalysis` (analyze selection, analyze all) writes the plain object of a row directly when that row has no `$state` entry yet (`rowEntries`), and goes through the row proxy as before when it has one. A row without an entry has had nothing but `id` read through it, so no view, effect or derived can show the old value; the entry that is created later starts from the plain object. Rows that were written directly are handed to `bumpLibraryTracks(ids, writtenToPlain)` for the search text; the copy step skips them anyway (no entry).
- The reads of that function (`analysisPending`, `bpmLocked`, `keyLocked`) also come from the plain object, so they no longer create the entry. `knownTrackIds` checks `plainTracks` instead of `libraryTrackById(id) !== undefined`: in dev builds Svelte compiles `!==` to `strict_equals`, which reads through the row proxy and created every entry (the store tests run in dev mode and showed no gain until this).
- Rows that already have an entry (shown rows, rows edited earlier, all rows after analysis results arrived) still take the proxy path, so a second analyze of the same rows in one session costs what it did. Results arrival, rating, colour, genre and relink writes are unchanged (no gain measured or claimed).

### Before and after

| Step | 2k | 10k | 50k |
|---|---|---|---|
| Page, analyze selection, task ms (Chromium, base to new) | 162 to 68 | 426 to 202 | 2,387 to 487 |
| Page, long task ms | 136 to none | 400 to 159 | 2,359 to 448 |
| Store (dev mode), analyze selected, sync part ms | 395 to 149 | 1,411 to 335 | 7,999 to 1,561 |

Other store steps (results, rating + colour, genre + raw, relink) are equal within noise (for example 50k results 2,540 to 2,648, rating + colour 896 to 859).

### Identity proof

- `bulk-bump-real.test.ts` (r95 payload, 2k, 10k, 50k): plain row, 24-order and 43-search hashes after each of the 5 steps equal base and new.
- `perf-bulk-bump.spec.ts`: on-screen rows before analyze, after analyze (playlist and All Tracks view), after results (2k, 10k) equal base and new in Chromium at 2k, 10k, 50k; WebKit new gives the same view hashes at 2k, 10k, 50k.
- `bulk-bump-fuzz.test.ts` with `FF_FUZZ_FULL=1`: 4 passed.
- Reactivity: `library-rows.test.ts` "shows a bulk analyze start on mounted rows and on rows nobody has read yet": a watched row (proxy path) shows the new values, a row first read after the analyze shows them and follows a later write, search and the plain rows agree.

### Validation

- `npx vitest run`: 968 passed, 8 skipped. `npm run check`: 0 errors.
- Chromium smoke: 114 passed, 33 skipped. WebKit smoke: 114 passed, 33 skipped.

## R146 search text (2026-11-07, `r146-haystack`)

R143 lever 4, from `ov-1028` (with R145). Linux `dev` box, load 10-60 the whole time (other workers), r95 50k and 10k payloads, headless Chromium. **Result: no product change kept.** The gain in a microbench is real but small; in the page it is inside the noise.

### What was tried

One fused function replacing `trackHaystack` plus `markCharacters`: lower-case the pieces instead of the joined text, memoise lower text and letter/digit bits per distinct artist, album, label, genre, key, tempo, time (and the raw fields when they differ), scan only title, path and index per row, build the string with `Array.join` (a flat string; plain `+` leaves a cons string that retains every piece, 40 MB against 22.5 MB for 50k rows, and flattens at the first search). Tried and dropped: a directory-prefix memo for the path (slower, the hash of the slice costs what the scan saves; the fixture has 2.6 tracks per folder), `TextEncoder.encodeInto` plus a byte scan (no gain), 64K character tables (slower).

Unicode: per-piece lower case equals whole-string lower case because every piece is space-delimited and a space ends the look-behind and look-ahead of the final-sigma rule, the only context-dependent mapping in `toLowerCase`. No ASCII fast path was needed: the native `toLowerCase` is the fast path, and half of the fixture rows carry non-ASCII text (20% non-Latin-1), so a JS ASCII loop would not have been taken.

### Measurements

| | base | fused |
|---|---|---|
| Node 22, 50k, build + bits, median / min of 25 | 295-352 / 283-309 ms | 245-270 / 207-243 ms (-15 to -25%) |
| Chromium microbench, 50k, median / min of 9 | 219-252 / 213-227 ms | 198-219 / 175-178 ms (-10 to -18%) |
| Page, `fillSearchIndex` wall total in the first 10 s, 50k (9 base, 8 new runs, loaded box) | median 424 ms (333-652) | median 470 ms (279-1,533) |
| Page, script in the first 10 s (`perf-warmup`, frames on), quiet runs | 726 / 727 / 734 / 767 | 650 / 780 / 868 / 875 / 906 / 913 / 930 / 951 / 971 |

The microbench saving (about 40-50 ms at 50k, 6-7% of the 700 ms the first 10 s cost) is smaller than the run-to-run spread of the page. Of the 700 ms, the search text is about 250 ms; the rest is decode, row mapping and mount.

### Identity (of the discarded change)

Haystack strings and letter and digit sets equal the base for every row of the 10k and 50k payloads (SHA-256 of all strings and masks equal, 3c3a59a7... and 818b2eaa...), on 5 seeds of 4,000 random rows (ASCII, Latin-1, Greek capital sigma next to separators and slashes, dotted I, combining marks, joiners, surrogate pairs and lone surrogates, null and numeric fields, empty fields, missing `raw`) and on edge rows (5,000-character fields). The change was then removed; the store tests were not rerun because no product code remains.

### Where a real gain would be

Not in the build's string work: the allocation volume per row (path, title, joined text) is the floor, and the memo only removes about a quarter of it. A larger step would be to avoid the stored string altogether (per-field search with the letter masks as the only per-row state), which changes the filter and needs its own item.

Scratch (`~/.cache/r146*`) deleted after this item.

## R147 sorted typing (2026-11-07, `r147-sorted-typing`)

R143 lever 5: at 50k the second key of a query in a sorted view cost 58-74 ms of script against 28 ms for the first key. Branch from `ov-1028`. Linux `dev` box (load 8-35, other workers), headless Chromium and Playwright WebKit, r95 10k and 50k payloads, base = `ov-1028` in a separate tree with the same spec and test files, runs interleaved. New harness `app/smoke/perf-sorted-typing.spec.ts` (`FF_PERF_STATE`, `FF_PERF_SORTS=none,title,artist,bpm`, `FF_PERF_ANALYSIS=1` for 6 `analysis-partial` events per second 10 s before the keys, `FF_PERF_PROFILE=1 FF_PERF_PROFILE_EACH=<dir>` for one CPU profile per key; frames run, R134): per sort, typing "house", backspacing to empty, typing "a b", backspacing to empty; per key script, total and a hash of the rows on screen. `app/src/stores/sorted-typing-real.test.ts` (env-gated): filter and sort time of 24 queries for 9 sorts, with and without a bump of 2 rows before every key, and the hash of every ordered result.

### Cause (measured)

- **The second key is the first key after a bump.** `filterHistory` is per list array and a bump makes a new array, so after every analysis bump (up to 4 per second) the history of earlier queries was gone: any query that was not the exact one asked before did a full scan of every search text. "a" is answered from the letter sets (8 ms at 50k in Node); "am", "hou", "a b" are `includes` over 37-50k texts (14-22 ms in Node, 50k, each bump in between). Without bumps between keys the old code narrowed from the earlier key and was as fast as the new code. Store level, sorted by title, bump before each key, ms (filter + sort): "hous" 16.7 to 4.4, "hou" back from "hous" 17.2 to 1.8, "house" 17.7 to 4.3, "house dub" 16.7 to 5.3, "am" after "a" and "a b" after "a" unchanged (the fixture's "a" matches all 50k rows, nothing to narrow).
- **The sort is not the cause.** Sorting 50k by title, artist or bpm adds 0.5-2 ms to a key at store level ("a b" asc 6.5 once for the rank); in the page the sorted and the unsorted view cost the same per key after the change (50k analysis script sum over 24 keys: none 252, title 252, artist 234, bpm 195 ms).
- **Keys that change the rows on screen cost script in any view**: Svelte updates of 40-50 rows, 20-35 ms against 8-12 ms for a key that leaves the visible rows alone ("hou" after "ho", "a b", backspace to "ho" and "a "). That is the remaining difference between a first and a second key; it is not sort or filter work and not touched.
- `matches` for a query of several terms ran `includes` for every term, also for one-letter terms ("a b"), though the letter and digit sets answer those exactly (R115).

### Changed (no markup, no CSS, same rows, same order)

- `app/src/stores/library.svelte.ts` `filterTracks`: the history of the library list is carried over to the new list array after a bump (`new Map(libraryFilters.history)` when the bump log reaches back to it); each entry knows the list it was computed for (`FilterEntry.list`). A query that narrows an older entry (`narrows`, all old terms inside new terms) starts from that entry's rows plus the rows bumped since (`libraryPositionsChangedSince`, those not already among the candidates are tested separately and merged in order), which is the full-scan result by construction: an unchanged row that matches the new terms matches the old ones. The same query asked before still takes `withChangedMatches`. A list replaced by anything but bumps drops the carried entries (as before the history was empty).
- `singleCharacterBits` takes several terms: when every term is one letter or digit, a row matches when all their bits are set in its letter and digit sets (one term: the same test as before). "a b" no longer calls `includes` (not separable in the store run: Svelte's dev build turns every `===` into a call, so the Node timing of that loop is not the browser's; the browser total above includes it).
- Tests: `filter-after-bump.test.ts` (rows that move into and out of the earlier result by edits and bumps, longer queries typed after them, several one-character terms in any order; breaking the merge of outside rows makes it fail, checked), `search-sort-fuzz.test.ts` fails with the merge or the bit test broken (checked), `sorted-typing-real.test.ts`, `perf-sorted-typing.spec.ts`.

### Before / after

Chromium 50k, analysis running (10 s old), 3 interleaved pairs, ms, median over runs (script / total):

| Key (sorted by title) | base | new |
|---|---|---|
| "ho" | 31 / 88 | 11 / 61 |
| "hou" | 45 / 98 | 36 / 94 |
| back to "hous" | 28 / 60 | 16 / 59 |
| back to "hou" | 26 / 62 | 10 / 60 |
| back to "ho" | 28 / 178 | 21 / 105 |
| "a b" | 46 / 112 | 36 / 104 |

| Script sum over the 24 keys, ms | none | title | artist | bpm |
|---|---|---|---|---|
| base | 281 | 321 | 293 | 295 |
| new | 252 | 252 | 234 | 195 |

Totals (key to second frame), mean per key: title 77 to 70, artist 73 to 65, bpm 70 to 65, none 69 to 71. Idle (no analysis, 2 pairs): title 365 to 244, artist 235 to 237, bpm 228 to 194, none 340 to 296 (noisy; there is no bump to lose the history). 10k analysis (2 pairs): 273/236/232/258 to 240/232/231/235, inside the noise (the scan is 3-4 ms). WebKit 50k analysis (2 pairs, `js` = key to a `setTimeout(0)`; compositor floor of R29 in `total`): none 868 to 914, title 815 to 794, artist 1022 to 827, bpm 868 to 1934 (one new run 1797, the other 977), totals 230-286 to 238-485 with that one run; no WebKit gain claimed.

### Identity

- Store level, hash of every ordered id of every result, base against new, 10k and 50k: `sorted-typing-real.test.ts` 432 of 432 at both sizes (9 sorts, 24 queries typed, backspaced and widened, with and without a bump of 2 rows before each key); `search-path-real.test.ts` (R113 script) 94 of 94; `sort-analysis-real.test.ts` (R137/R139) 601 of 601.
- Page level: the rows on screen after every key (hash, 3 sorts and unsorted, typing and backspacing "house" and "a b") equal between base and new in every run, also the analysis runs (one view-hash sequence in all 6 runs at 50k, all 4 at 10k, all 4 in WebKit).
- Fuzz: `FF_FUZZ_FULL=1` `bulk-bump-fuzz`, `search-sort-fuzz`, `playlist-sort-fuzz` 12 of 12.

### Validation

- `npx vitest run`: 110 files passed, 10 skipped; 970 tests passed, 9 skipped. `npm run check`: 0 errors, 0 warnings.
- Chromium smoke (`FF_SMOKE_PORT=5591`): 114 passed, 34 skipped. WebKit smoke (`FF_SMOKE_BROWSER=webkit FF_SMOKE_PORT=5592`): 114 passed, 34 skipped.

### Not done

- A key that scans (a second character on a set that did not shrink, "am" after "a" when "a" matches every row) still costs one `includes` per row, about 12-20 ms at 50k: per-call overhead of `includes` on 150-200 character texts; a per-row bigram set would pay for it with more search-text build time (R146: the build is the floor).
- The Svelte update of the visible rows after a key that changes them (20-35 ms script) is markup territory.
- Carried entries keep their old list arrays alive (at most 16 entries, each a 50k-pointer array).

Scratch (`~/.cache/r147`) is deleted after this item.

## R148 row updates (2026-11-07, `r148-row-updates`)

R147 said a key that changes the rows on screen costs 20-35 ms of script at 50k, mostly Svelte updates of the visible rows. Branch from `ov-1028`. Linux `dev` box (load 10-30), headless Chromium and Playwright WebKit, r95 50k payload, base = `ov-1028` in a separate tree with the same spec files, runs interleaved.

### Cause (measured)

- CPU profile per key (`FF_PERF_PROFILE_EACH`, 50k, sorted by title, no analysis; "hou", "a b", back to "ho"): of 91 ms flush time over the three keys, 65 ms is derived values (`filterTracks` 25, `currentTracks` self 14, `sortTracks` 12) and 22 ms is effects (`update_effect`, the row updates, 5-8 ms per key; `create_item` inside it 2-4 ms, `set_text` 1-3 ms). The rows are 40-50 recycled slots (R95), each a `TrackRow` whose cells only change text and classes. The row update is about a fifth of a key, not most of it.
- The visible-row cost is in `prototyping/viewport/design-system/components/TrackRow.svelte` and `Waveform.svelte` (AGENTS fence, not touched). No change there was needed to find a lever: nothing in them stood out above 1-3 ms per key.
- Outside the rows, `currentTracks` in `app/src/modules/TrackTable.svelte` built `new Set(previous.rows)` (up to 50k entries, 8-12 ms self) on every key whose result had a different size from the previous one, only to have `sameMembers` return false on the length check straight after.

### Changed (no markup, no CSS, same rows, same order)

- `TrackTable.svelte` `currentTracks`: `previous.rows.length === filteredTracks.length` is tested before `sameLibraryMembers` and the lazy member Set. A different length can never be the same rows, so `sameRows` is the same value as before; the Set is only built when the lengths are equal.

### Before / after

Chromium 50k, sorts none, title, bpm, 16 keys each, mean script ms over the keys that change the rows on screen (13 per run), one value per run. Pairs are base and new back to back, order flipped every pair (a stray earlier run left an extra new run in the analysis series):

| | base | new |
|---|---|---|
| analysis running, 3 pairs | 30.0, 25.3, 26.8 | 24.9, 26.2 (21.0, 25.8), 21.2 |
| idle, 3 pairs | 27.7, 24.9, 33.0 | 26.2, 20.1, 19.9 |

Mean over runs: analysis 27.4 to 23.8, idle 28.5 to 22.1. New is lower in 5 of 6 pairs, but the ranges overlap (base 24.9-33.0, new 19.9-26.2), so the gain, about 4-6 ms on a row-changing key, is small and only just outside the box noise; not a large claim. Keys that leave the rows alone (median 12-15 ms) are unchanged. Total (key to second frame) for row-changing keys, median over keys: analysis 92 to 86, idle 95 to 90. WebKit: not timed (no script counter; identity only).

### Identity

- Page level, `perf-sorted-typing.spec.ts` row hash per key: one view-hash sequence in every base and new run, equal across the two.
- New `app/smoke/perf-row-dom.spec.ts`: after every key (7 queries typed and backspaced, "house", "hou", "a b", "dub", "x", "a", "mix remix", for unsorted, title and bpm; 150 steps) the `outerHTML` of every visible row (canvas attributes removed) and the pixel data of every row canvas are hashed. Base against new, Chromium and WebKit: 150 of 150 identical in both, 0 canvas differences (23 distinct row sets in each run).

### Not done

- The remaining 5-8 ms per key of row effects and the 25 ms of `filterTracks` per scanning key stay. Cutting row effects further means changing `TrackRow` and `Waveform` in `prototyping/` (fenced); nothing there showed a lever above a few ms.

### Validation

- `npx vitest run`: 110 files passed, 10 skipped; 970 tests passed, 9 skipped. `npm run check`: 0 errors, 0 warnings.
- Chromium smoke (`FF_SMOKE_PORT=5595`): 113 passed, 35 skipped, 1 failed (`leaks.spec.ts:110` at load average 35); that spec alone passes on rerun. WebKit smoke (`FF_SMOKE_BROWSER=webkit FF_SMOKE_PORT=5596`): 114 passed, 35 skipped.

Scratch (`~/.cache/r148`, base worktree) is deleted after this item.

## R149 filter scan (2026-11-07, `r149-filter-scan`)

R147 and R148 left a scanning key (a second character on a set that did not shrink, "am" after "a") at about 25 ms in `filterTracks` at 50k. Branch from `ov-1028`. Linux `dev` box (load 11-22), Node 22 (V8 as in Chromium) on the real r95 50k search text (`haystackListFor()` dumped once: 50,000 rows, 211.6 characters on average), best of 15. **Result: no product change kept.** Every variant that is clearly faster on a scanning key costs more build time than it saves in realistic use.

### Measured

Cold store keys after "a" (`filterTracks`, 50k, ms, 3 runs): am 25, an 20, ou 26-28, th 17, er 16 (all rows), mix 16, ng 18, on 20, in 14, lo 16, zz 7, xq 9. Plain `includes` over the 50k texts is 15-20 ms of that; the rest (push, row array) is 3-5 ms.

| Variant (50k, "am" / "ou" / "th" / "amo") | scan | build | memory |
|---|---|---|---|
| base, `includes` per row | 19.6 / 19.4 / 15.7 / 21.5 | none | none |
| one concatenated haystack, `indexOf` jumping from hit to hit, row from a start-offset table | 17.6 / 18.3 / 13.8 / n.m. (about -10%, "zz" 6.9 to 3.7) | 55 ms join, plus a copy of the text (about 10-20 MB) | +10-20 MB |
| hashed bigram bits per row (1,024 bits, test the bits, then `includes` to confirm) | 6.7 / 6.6 / 6.8 / 3.7 (-60 to -80%) | 120 ms standalone; piggybacked on `markCharacters`: 84-134 to 121-213 ms, so +37-80 ms | 6.4 MB |
| the same with 512 bits | 8.1 / 9.0 / 9.2 / 3.3 | same | 3.2 MB |

Both give the same rows as the base for every term tried (am, an, ou, th, zz, xq, er, mix, house, amo).

### Why none is kept

- The bigram bits cut a scanning key by about 12 ms (60%) but add 40-120 ms to a search text build that is already 250 ms (R146: the build is the floor) and 3-6 MB of heap. A full scan is also rare: a key that extends the previous one narrows from its rows (R147), and a bump no longer loses the history. Break-even is about 4-10 scanning keys per session; building the bits lazily on the first scanning key would stall that key by 80-120 ms to save 12 on the later ones.
- The concatenated haystack saves 2-3 ms and needs a rebuild or a stale-row check after every bump (`refreshHaystacks` edits single rows, offsets would move).
- Off the main thread: `filterTracks` is a synchronous `$derived` input to the sort and the row window; a worker would need its own copy of the text (10-20 MB), and the result would arrive a frame later with rows that do not match the query for that frame. For 15-25 ms at 50k that is not worth a changed contract.
- Scanning only candidates in chunks is what R147 already does when the set shrinks; when it does not (the letter "a" matches every row) the candidate list is the whole list.
- The per-call overhead is not the cost: `indexOf` over one 10M-character string runs at the same 1.7 ns per character as 50k separate `includes` calls. The scan is bound by V8's string search on 10-20 MB of text.

### Identity / validation

No product code changed, so the identity tests and smoke runs were not repeated; the variants were compared with the base only on the term list above (equal rows). Scratch (`~/.cache/r149`) deleted.

## R150 model ops (2026-11-07, `r150-model-ops`)

Question: are there CPU gains inside the beat model run, at chunk 500, for the fp32 and the int8 model, with output that stays the same to the last bit? Branch `r150-model-ops` from `ov-1028` (f327a90): this section and a profiling example. One lever pays, and only for int8, so it lives on a side branch from the parked int8 branch: `r150-int8-prepack` from `r97-int8-500` (1b3a972). Linux `dev` box (Xeon Gold 6130, 16 vCPU, AVX-512 without VNNI), release builds, at most 2 analysis threads. **Result: rten weight prepacking cuts int8 CPU by about 4% with byte-identical output; for fp32 it gives nothing and costs 90-130 MB RSS. Nothing else found is both identical and faster.**

### Op profile (`fourfour-analysis-rs/examples/model_ops.rs`, one 500 frame chunk, 1 thread, rten `RunOptions` timing)

| op | fp32 ms (share) | int8 ms (share) | what it is |
|---|---:|---:|---|
| FusedMatMul (MatMul + bias) | 285 (29.2%) | - | feed-forward and frontend weight GEMMs |
| MatMulIntegerToFloat | - | 372 (38.1%) | the same weight GEMMs in int8 |
| MatMul | 282 (28.8%) | 157 (16.0%) | qkv/out projections (fp32 only, about 150 ms) and attention QK^T and AV (about 130-210 ms) |
| Softmax | 83 (8.5%) | 83 (8.5%) | attention |
| Concat, Split, Reshape, Transpose, TransformInputs(Split), Slice, Expand | 189 (19.3%) | 188 (19.2%) | data movement; most of it is the rotary embedding (`rotate_every_two`) in all 24 attention blocks |
| Conv | 39 (4.0%) | 43 (4.4%) | frontend |
| Gelu, Mul, Add, Div, ReduceL2, Neg | 94 (9.6%) | 101 (10.3%) | elementwise |
| DynamicQuantizeLinear | - | 25 (2.6%) | int8 only |
| total | 977 | 977 | 2,777 / 2,802 ops; max activation 39 MB |

By shape, the 6 big transformer layers are about 60% of the time: the feed-forward GEMMs [500, 512] x [512, 2048] and back (21 + 19 ms each), the qkv projection (15 ms) and the 16-head attention (9 + 5 ms + 6 ms softmax). One odd entry: the first frontend time attention (`blocks.0/partial/attnT/attend/MatMul`, [32, 1, 500, 32] x [32, 1, 32, 500]) takes 50 ms where blocks 1 and 2 do the same 32 matrices of the same size in 12 ms; same graph pattern, both inputs contiguous. Not explained; inside rten.

### Levers

| lever | identical? | CPU | kept? |
|---|---|---|---|
| Prepack weights at load (`ModelOptions::prepack_weights`, rten packs MatMul, FusedMatMul and MatMulInteger B matrices once) | yes: logits and the full `analyze-json` output byte-equal, both models | fp32: none (user 374.2 s base vs 375.2 s new, mean of 2); int8: **-4.4%** (user 346.9 s base vs 331.6 s new, mean of 3) | int8 only, side branch |
| 2 threads per chunk instead of 1 (`ThreadPool` per chunk) | yes (same logits at 2 threads) | +12% CPU per chunk (wall -28%) | no; 1 thread per chunk with one chunk thread per analysis thread stays the cheapest per track |
| ONNX rewrite of the rotary `Split / Neg / Concat / Reshape` as `Gather(perm) * sign` | yes (same logits: only copies and an exact sign flip) | +9% (rten `Gather` 124 ms) | no |
| the same as `Slice(step -1) * [-1, 1]` | yes | +10% (rten negative-step `Slice` 98 ms) | no |
| our code around `Model::run` (`extract_chunk` 256 KB copy, `RunOptions` clone, logits `into_data`, stitching) | - | about 0.03 ms of a 1,000 ms chunk | nothing to gain |
| rten cargo features | - | - | none apply: the GEMM kernels (AVX2/FMA, AVX-512, VNNI on x86; NEON and the i8 dot product on aarch64) are picked at run time in 0.24, the only features are file formats, `fft`, `random`, `mmap` |
| graph constant folding and fusion at load | - | - | rten already does it (`FusedMatMul`, `Gelu`, `AddSoftmax` and others); a further fused rotary or attention op would be an rten change |
| a buffer pool kept across runs | - | - | not reachable from outside in 0.24 (R106); at chunk 500 the per-run pool costs about 0 sys time (max activation 39 MB, no blocks above the mmap threshold) |

Why prepacking helps int8 and not fp32: without it rten packs every weight matrix on every call. For fp32 that is a copy of K x N floats against 500 x K x N multiply-adds, about 0.2%. The int8 packing also reorders into the dot-product layout and computes the column sums for the zero point, which is a larger share next to the int8 kernel. The fp32 RSS cost is the packed second copy of the 83 MB of weights; for int8 the copy is about 23 MB.

### Before / after (`analyze-json --threads 2`, 8 tracks: 3 MP3, 3 FLAC, 2 WAV from `rekordbox-testdata-2025`, chunk 500 (r97 default), interleaved base, new, new, base, plus one more new, base pair for int8)

| model | build | user s | sys s | CPU per track | wall s | peak RSS MB | load at start |
|---|---|---:|---:|---:|---:|---:|---|
| fp32 | base | 374.8 / 373.6 | 2.3 / 2.1 | 47.1 s | 186 / 172 | 483 / 488 | 11 / 12 |
| fp32 | prepack | 378.6 / 371.8 | 3.5 / 2.9 | 47.3 s | 247 / 192 | 611 / 571 | 23 / 33 |
| int8 | base | 346.5 / 340.1 / 354.1 | 1.9 / 1.7 / 3.1 | 43.6 s | 158 / 155 / 237 | 442 / 455 / 481 | 6 / 9 / 28 |
| int8 | prepack | 331.7 / 334.1 / 328.8 | 1.7 / 1.7 / 2.0 | 41.7 s (**-4.4%**) | 152 / 152 / 173 | 475 / 475 / 475 | 6 / 7 / 7 |

Every int8 prepack run is below every int8 base run (329-334 against 340-354 s user; the 354 s base ran at load 28, the two quiet ones were 340 and 346). fp32 is within noise both ways. RSS: int8 +20-33 MB (+5-7%) against the two quiet base runs, fp32 +90-130 MB (+18-27%).

**Identity.** `cmp`-equal output for every run of a model: fp32 8,952,482 bytes, sha256 prefix `097fba2ee30b5b8a` (4 runs, both builds); int8 8,952,495 bytes, `bccdf469f94b44c7` (6 runs, both builds). `model_ops` gives the same logit hashes with and without prepacking for both models (fp32 beat `2e33ac0a8585c976`, int8 beat `cb2294a4618d881b`).

### What changed

- `r150-model-ops` (from `ov-1028`): `examples/model_ops.rs` (per-op timing, `BY_SHAPE=1` for shapes, `PREPACK`, `POOL_THREADS`, `CHUNK_FRAMES`, `MODEL_FILE`, logit hash) and this section. No product change: on `ov-1028` the model is fp32, where prepacking only costs memory.
- `r150-int8-prepack` (from `r97-int8-500`, e779648, not merged anywhere): `Analyzer::load` prepacks the weights when the beat model is not the default fp32 file (`prepacks_weights`, the int8 file chosen with `FOURFOUR_BEAT_MODEL`). If int8 ships, land this with it. Its own binary was checked once more: int8 user 333.7 s, RSS 474 MB, sha `bccdf469f94b44c7`; fp32 user 368.8 s, RSS 483 MB (no prepack, as base), sha `097fba2ee30b5b8a`.

Validation: `cargo test --workspace` on `r150-model-ops`: 26 `test result: ok` lines, 570 passed, 0 failed. `cargo test -p fourfour-analysis-rs` on `r150-int8-prepack`: all ok (24 unit tests, new `only_a_non_default_beat_model_prepacks_its_weights`). Scratch (`~/.cache/r150*`, the int8 model regenerated with the r97 script) deleted.

### Not measured

- The Mac. int8 runs on rten's aarch64 CPU path there (i8 dot product kernels); the same per-call packing exists, so a similar or larger gain is likely, but it needs a Mac pair (add `r150-int8-prepack` to the next Mac night next to `r98-mac-pair`).
- Chunk 1500 on `ov-1028` (the current default): prepacking was not measured there; the weight-packing share is smaller at 1500 than at 500.
- The rotary data movement (about 12% of the run) and the slow first frontend attention are the largest identical-output targets left, both inside rten (a fused rotary op or a faster strided copy for an innermost axis of 2).

## R151 first attention MatMul (2026-11-07, `r151-first-matmul`)

Question: R150 saw the first frontend time attention (`blocks.0/partial/attnT/attend/MatMul`, [32, 1, 500, 32] x [32, 1, 32, 500]) take about 50 ms in a 500 frame chunk, where blocks 1 and 2 do the same 32 matrices in about 12 ms. Why, and can it be cut with identical output? Branch `r151-first-matmul` from `ov-1028` (a807485). Linux `dev` box, release build, 1 thread, `model_ops` (now also prints minor page faults per run). **Result: it is not the MatMul. It is first-touch page faults on a fresh 32 MB output buffer, and at chunk 500 only in the first 3 runs of a thread. From the 4th run on the op takes 12.5 ms, like blocks 1 and 2. Nothing to cut in our code; doc only, no product change.**

### Cause

- The QK^T output (the attention scores) is 32 x 500 x 500 floats = 32,000,000 bytes, the largest buffer of the run up to that point. Blocks 1 and 2 produce the same element count ([16, 2, 500, 500], [8, 4, 500, 500]), so rten's `BufferPool` hands them block 0's buffer again once Softmax and the AV MatMul are done with it. Only block 0 gets fresh memory from malloc, and the first write to it faults in about 7,800 pages. The kernel, the layout and the shapes are the same in all three blocks.
- rten 0.24 creates a new `BufferPool` in every `Model::run` (`graph.rs`, R106), so each chunk starts with an empty pool. Whether the 32 MB then costs faults depends on glibc: in the first runs the block is `mmap`ed (dynamic mmap threshold still low) or lands in fresh arena heap pages; once the threshold has risen and the arena heap has its final size, the freed 32 MB stays mapped and is reused by the next run without faults.
- Evidence (`model_ops`, 500 frames, 1 thread, minor faults per run): runs 1 to 4 fault 22,188 / 12,308 / 7,698 / 1 pages (four repeats, same sequence each time, then 1-245 per run up to run 8). Run 3 faults 7,698 pages, which is the 32 MB score buffer (7,813 pages). The op time follows: 53-54 ms when the timed run is run 2 or 3, 12.3-12.9 ms when it is run 4, 5 or 8.
- Cross-checks: with `MALLOC_ARENA_MAX=1 MALLOC_MMAP_MAX_=0 MALLOC_TRIM_THRESHOLD_=1G` (R106) the op takes 13.8 ms already in run 3. With `RTEN_USE_POOL=0` (no buffer reuse inside the run) blocks 1 and 2 would have to fault too; in run 3 all three took 12.3-12.6 ms because the warm-up had already happened in runs 1 and 2 (186k and 154k faults there). Logits are the same in every run and setting (beat `2e33ac0a8585c976`, down `082582f6ea64f9f4`).
- R150's 50 ms figure matches such a warm-up reading: 53-54 ms came back here whenever the timed run was run 2 or 3, never from run 4 on.

### What it costs in the app

- The beat chunk threads are long-lived (`ChunkExecutor`, one 1-thread rten pool each), so each thread pays the warm-up once for its first 3 chunks: about 42k faults, about 150-200 ms of system time per chunk thread per process. Per track that is nothing; R150's 8-track `analyze-json` at chunk 500 had 2.1-2.3 s sys against 374 s user in total (0.6%), an upper bound for all faults.
- So the "4% of a chunk" does not exist in steady state, for fp32 or int8 (same graph, same buffer).

### Chunk 1500 (the `ov-1028` default)

Here the slow first MatMul is real in every run, but it is R106: blocks above 32 MiB are always `mmap`ed by glibc and unmapped on free, so the 288 MB score buffer (and the 144 MB ones of the 6 transformer layers) fault every chunk. `model_ops`, 1500 frames, 1 thread:

| | blocks.0 QK^T | blocks.1 / 2 QK^T | transformer QK^T (mean of 6) | user / sys ms per run | faults per run |
|---|---:|---:|---:|---:|---:|
| default glibc | 535 ms | 149 / 158 ms | 265 ms | 5,130-5,420 / 1,120-1,320 | 257k-276k (runs 1-3) |
| R106 env vars (run 3) | 134 ms | 133 / 134 ms | 68 ms | 4,470 / 10 | 27 |

Without faults every attention QK^T runs at the same 4.2 ms per 1500 x 1500 matrix, so the kernel is fine there too. Same logits both ways (beat `91f79fb92dd29326`). The levers are the ones R106 already weighed: the three malloc env vars (-22% CPU, +10-23% peak RSS, not default), chunk 500 (r96, faults go away), or an rten change that keeps the `BufferPool` across runs.

### Levers checked

| lever | result |
|---|---|
| operand layout, transposed K, batch shape [32, 1] vs [16, 2] / [8, 4] | not the cause: without faults all three blocks take 12.3-13.8 ms at 500 and 133-134 ms at 1500 |
| a load-time graph tweak so block 0 reuses an earlier buffer | nothing earlier in the run is 32 MB or larger; an extra early allocation would only move the faults |
| our chunk tensor (`extract_chunk`) | 256 KB, already contiguous [1, 500, 128]; not involved |
| `mallopt` at startup to skip the warm-up | would save about 150-200 ms of sys per chunk thread per process at 500; not worth the global allocator change. At 1500 it is R106 with its RSS cost |
| keep the pool across runs | not reachable from outside rten 0.24 (no pool in `RunOptions`); an upstream change |

### What changed

`fourfour-analysis-rs/examples/model_ops.rs` prints minor page faults per run next to user and sys ms, plus this section. No library or app code changed, so `analyze-json` output is the same binary path as `ov-1028`. Validation: `cargo test --workspace`: 26 `test result: ok` lines, 570 passed, 0 failed. Not measured: the Mac (a different allocator, so a different warm-up; not checked), and `analyze-json` timing, since there is no product change to compare.

## R152 budget (2026-11-07, `r152-budget`)

Measurement only, no product change. Branch `r152-budget` from `ov-1028`. New harness `app/smoke/perf-budget.spec.ts` (boot, flows, import, analysis, memory; one JSON line per run). Linux `dev` box (16 cores), headless Chromium 1217 and Playwright WebKit, the r95 `bench_startup dump` payloads of the `ff-fixture` 10k and 50k libraries (read only, `~/.cache/r95/state{10k,50k}.json`, same schema the page decodes today). `bench_startup` was not rebuilt, so the backend share is cited from R129 (`load_state` 50 / 210 ms), not re-measured.

Commands (one at a time, 3 runs unless noted, load average recorded in each line):

```
FF_PERF_STATE=<state.json> FF_PERF_BUDGET=boot,flows,import,analysis FF_PERF_RUNS=3 FF_PERF_LABEL=x FF_PERF_BUDGET_OUT=out.jsonl npx playwright test smoke/perf-budget.spec.ts
FF_PERF_BUDGET=memory FF_PERF_SOAK_MIN=10 ...            # heap: open, 2 full scrolls, 10 min idle with 1 analysis event per second
FF_PERF_PROFILE=1 ...                                    # unminified build plus a CDP sampling profile per step (top 3 self-time functions)
FF_SMOKE_BROWSER=webkit ...                              # WebKit: wall time only (no CDP: no script/layout split, no profile)
FF_PERF_STARTUP=10000=<state>==50,50000=<state>==210 FF_PERF_RUNS=5 npx playwright test smoke/perf-startup.spec.ts   # cold open (backend simulated at 50 / 210 ms)
```

Read with care:

- **Load.** Quiet runs had load 5.7-6.1 (flows 50k, cold open); the first batch ran at 8-11 and the 10k analysis and import runs at 16-37. Numbers below come from the quiet runs where there is one; 10k analysis numbers are not usable (load 16-36) and are left out. Profile runs (load 30) show shares, not times.
- **Click timing** = hardware timestamp of the click until the screen shows the result plus two animation frames (about 35-40 ms of that is the headless compositor floor, as in R129). "rest" in the split is main-thread task time that is not script, layout or style (paint, layerize, commit, GC); it cannot be split further without a trace.
- **Harness costs removed.** The mock's `get_waveform_detail` builds a 4.8 MB buffer in about 316 ms on the page's main thread; the first version of the track-load step carried it (700 ms). The final step answers it from a cache filled before the click. The mock also hands the 50k payload to the page as init-script JSON, which delays the first row by about 1.2 s; cold open therefore comes from `perf-startup.spec.ts` (payload by fetch, backend simulated), not from the new spec.
- **WebKit** is headless software compositing (R29): 2-6x slower on every step, scroll 120-170 ms per frame. Only the ratios carry to the Mac.

### Budget table, ranked by user-felt ms (Chromium, 50k, quiet; `x / y` = 10k / 50k where both exist)

| # | Flow | Felt ms | Script / layout+style / rest | Top 3 (CPU profile, self ms) | Top cause | Class | Fenced | Lever idea |
|---|---|---|---|---|---|---|---|---|
| 1 | Cold open to first rows, then interactive | 628 / 1,035 first rows, interactive 691 / 1,035 (warm 573 / 910); WebKit cold 898 / 1,410; backend inside this (50 / 210 simulated) | to first rows 320 / 406, 118 / 113, rest not split (mock-inflated); longest task 196 / 280, TBT 234 / 332 | GC 674, `trackHaystack` 220 + `markCharacters` 101 (search text), `valueClassesSteps` 136 + `sortDistinctStrings` 79 + `orderFromClasses` 52 (sort warm-up); profile at load 31, shares only | mount (design-system rows, sidebar) plus the idle index warm-up that starts with the first rows (R121, R129) | a for the warm-up and `load_state`, b for the mount | partly (mount: `TrackRow`, `Sidebar` rows) | start the `load_state` compute at process setup (R129 lever 3); defer the warm-up past the first click window |
| 2 | Import 1k folder into the 50k library, after the rows appear | 555-653 ms of script in the 3.5 s after the rows (10k: 217-338), worst frame 169 / 191 ms; UI click to rows 30 / 53 ms with the mock | script 555-653, layout 6, rest 164-219 | `trackHaystack` 194, `markCharacters` 85, `sortDistinctStrings` 82 | adding 1,000 rows throws away and rebuilds the search text and sort classes of all 51,000 rows | a | no (`library.svelte.ts`, `table-columns.svelte.ts`) | append the new rows to the search index and sort classes instead of rebuilding |
| 3 | Fast fling over the library | 49 ms per frame p50, p95 63, max 89; 89 of 90 frames over 25 ms, 32 over 50 ms; 18-31 long tasks, 1.0-1.8 s of 4.6 s; wheel fling 46 / 62 / 69 | per 90 frames: script 1,214, layout+style 1,123, rest 2,156 | `handle` 230, `get` 173, `(anon)` 136 (Svelte row updates and bindings; no source for `handle`) | every frame replaces all 42 row components (R118: 37 canvases and 37 overlap layers per 42 rows); steady scroll (90 px per frame) is 23 p50, 28 p95, 32 of 120 frames over 25 ms | b | yes (`TrackRow`, `Waveform` row canvases) | no per-row canvas work while a fling is faster than N rows per frame (changes what is shown) |
| 4 | Track double click to player ready | 293 ms (10k: 243-634 loaded) | 46 / 15 / 229 | `colorBandsFromPayload` 20, anon 42, anon 19 (load 31, shares) | player overview paint and the answer path; includes the mock answer through Playwright (not app time) | a, b | partly (overview in `Waveform`) | reuse the decoded bands across a double click and the inspector load |
| 5 | First row select (inspector opens) | 275 ms (10k 340 loaded; R129 measured 12 ms for later selects) | 43 / 19 / 95 | anon 59 (`index`), `set_attribute` 13, `Detail` chunk | first select mounts the lazily loaded `Detail` inspector (chunk, component tree, bindings); later selects are cheap | a | no (`Detail.svelte`) | preload the `Detail` chunk and mount it hidden after the first rows |
| 6 | First click right after the first rows (sort by title) | 227 ms at 50k (10k 131); a later first click on a settled page is 105 (second click 91) | 191 / 13 / 73 (boot click) against 24 / 9 / 57 settled | `sortDistinctStrings` 54, `valueClassesSteps` 53, `sortValueOfRaw` 26 | the sort-column warm-up is still running (about 800 ms of main thread after the first rows, 400-500 ms of TBT) | a | no (`table-columns.svelte.ts`) | R129 lever 2: rank from the typed arrays the decoder already built, not from text |
| 7 | Expand the big player | 180 ms (10k 187); one 70 ms task | 77 / 22 / 146 | `fillRect` 17, `reduceBands` 14, `measureText` 9 (load 30) | first draw of the detail canvases and the expanded layout | b | yes (`waveform-display.js`, `Waveform`) | draw the detail canvas in two slices, label metrics measured once |
| 8 | Analysis running, list visible | main thread busy 61 ms/s at 1 event/s (idle page 7), 248 ms/s at 6 events/s (worst second 324); about 50 ms per event | script 10-14 ms/s; the rest is GC and rendering | GC 60-98 ms/s, `get` 27-39, `is_dirty` 27 (Svelte reactive reads) | each `analysis-partial` bumps rows through the store, and GC follows (R129 finding: `structuredClone` of row fields) | a | no (`library.svelte.ts`) | R129 lever 1: copy fields instead of `$state.snapshot` clones; batch events per frame |
| 9 | Player playing (strip) | idle 13 ms/s (worst second 105-180); big player 6 ms/s; with analysis 1 event/s 56 / 52 ms/s | script 2 ms/s | GC 100 ms in a 15 s window, anon, `walk` | a periodic GC second in the strip; analysis cost is the same as row 8 (no extra from the player) | a | no | none needed beyond row 8 |
| 10 | Clicks that open something: playlist (10,000 tracks), analysis panel, settings, sort | 106, 101, 117, 105 ms | playlist 31 / 12 / 51; panel 9 / 11 / 120; settings 16 / 18 / 117 | `positionsByIdTable` 11 (playlist), `focus` 73 ms in the loaded profile (settings), `lastCheckText` 18 | compositor floor plus one forced layout (`focus`) in the settings sheet | c, a | partly (sheet is design-system) | focus the sheet after the first paint |

Also measured:

| Item | Chromium | WebKit (software) |
|---|---|---|
| Sort click (first / second), 10k | 103 / 90 ms | 212 / 178 |
| Playlist, 10k (2,000 tracks) | 91 ms | 200 |
| Inspector first select, 10k | 340 ms (loaded) | 581 |
| Track load, 10k | 590 ms (loaded; 243 in the quiet run) | 711 |
| Expand, 10k | 187 ms | 461 |
| Settings, 10k | 165 ms (loaded) | 476 |
| Import 1k, UI to rows (mock), 10k / 50k | 53 / 30 ms | 41 / 44 ms (worst frame 409) |
| Scroll steady, p50 / p95, 50k | 23 / 28 ms | 148 / 191 |
| Scroll steady, p50 / p95, 10k | 25 / 41 ms (loaded) | 122 / 167 |

### Memory (Chromium, 50k, forced GC before each reading, `Runtime.getHeapUsage`)

| Moment | JS heap MB | DOM nodes |
|---|---|---|
| after open (6 s settled) | 98 | 6,402 |
| after two full scrolls | 107 | 6,401 |
| idle with analysis 1 event/s, minute 1 / 5 / 10 | 119 / 120 / 120 | 6,433 |

Heap rises 21 MB in the first minute of analysis events and then stays flat for 10 minutes (no leak seen), DOM nodes are flat, and the idle page costs 52-56 ms of main thread per second the whole time (that is row 8, not a leak). 10k: 25 MB after open, 43 after a scroll and playlist walk. Real app RSS needs a window and was not measured (R23, R42).

### What the list says

- The search text and the sort classes are the biggest repeated main-thread work at 50k: they run once in the first seconds (row 1, 6), again after an import (row 2), and the same rows are bumped by every analysis event (row 8). One incremental index (append and patch, never rebuild) would touch rows 1, 2, 6 and 8 with identical results.
- The felt costs of single clicks are at the floor (100-117 ms of which about 40 is compositor); only the first select (275) and the first click inside the warm-up window (227) stand out.
- Fling scroll is the one flow that is slow in every phase, and its hot code is in `prototyping/` (fenced).
- Not measured: real Tauri window start, real backend `load_state`, cold-disk first query (R129: 1.5 s at 10k, 4.1 s at 50k), real audio playback, a Mac.

### Top 5 next items

1. **Incremental search index and sort classes** (`library.svelte.ts`, `table-columns.svelte.ts`): append and patch on import and bumps, never rebuild all rows. Identical result, can land. Targets rows 2 and 8 and part of 1 and 6 (about 600 ms after an import, about 270 ms of boot warm-up).
2. **Analysis events: copy row fields, batch per frame** (R129 lever 1, plus one flush per frame): about 50 ms of main thread per event to a few ms. Identical result, can land.
3. **Sort warm-up from the decoder's typed arrays and later than the first click window** (R129 lever 2): first click right after the first rows 227 to about 110 ms, and 400-500 ms less blocking after open. Identical result, can land.
4. **`load_state` computed at process setup** (R129 lever 3): takes the 210 ms backend (50k) off the path to the first row, and the storage wait on a cold disk. Identical bytes, can land (Rust, `pioneer-test-ui`).
5. **Fling scroll: no per-row canvas work while rows pass faster than they can be read** (R118 proposal): 49 ms per frame to near the steady 23. Changes what is shown during a fling and the hot code is fenced, so it needs Merlin and a prototyping change first.

Scratch (`~/.cache/r152*`) is deleted after this item.

## R154 early load_state (2026-11-07, `r154-early-load-state`)

Branch `r154-early-load-state` from `ov-1028`. R135 already builds the answer on a thread at setup and hands the bytes to the first `load_state`, so that is not redone. What R135 left: the thread starts inside the `setup` hook, and Tauri 2.10.3 builds the config windows (`WebviewWindowBuilder::build`) before it calls the hook (`tauri::app::setup`), so the webview and page start loading first. The library open (`read_library_path`, `create_dir_all`, `LocalLibrary::open`, about 2 ms warm here) also ran on the main thread inside the hook, before the thread was spawned.

### What changed

- `early_state::open_and_prepare` runs the library open and the answer build on one thread. `main` calls it right after `Builder::build` (the `App` and its path resolver exist, no window yet) and before `run`. The setup hook takes the opened library from a channel (`StartupLibrary`) instead of opening it itself and keeps the rest of its work (staging sweep, file health, cover fill, playback) as it was. The thread holds the library lock before it sends the library, so the answer is built before the health check or cover fill can take the lock, as in R135.
- Same functions and checks: `read_library_path`, `open_startup_library` (unchanged), `EarlyState::prepare`, `encoded_state` with the revision and labels guard. An open error is sent unchanged and nothing is built; the setup manages `LibraryOpenFailure` as before and `load_state` returns the same error (the recovery path).
- `EarlyState` is now managed at start (before, only after a successful open); `load_state` checks the library state first, so a failed open still answers with the failure.
- New mark (with `FF_PERF_LOG`): `library_opened`; `early_state_ready` now comes from the thread.
- Tests: `early_state::tests` (the thread hands back the library and the first request gets the early answer with equal bytes; a write after the build returns the new state; an open error is returned unchanged and nothing is built) and in `main` (a damaged `library.db` and a missing library path through the thread give the same error, kind, path and untouched file as the direct open; a missing `library.db` is created and answered from the thread).
- `bench_startup boot <dir> base|new <setup ms> <request ms> [out.bin]`: one simulated launch with the real functions. `base` is the R135 flow (open in the hook at `setup ms`, build thread after it), `new` is this one (open and build from time 0, the hook only waits for the library).

### Identity

`bench_startup boot` prints the sha256 of the answer in both modes: 10k `189c1084156ec70db43f1005134df010ae92b5cadde3e1f6d57074c6d86b36ad` (1,715,152 bytes), 50k `893a4522aa3716187953bf68054de31c73768fe174ae242c904bbf299fcccbe9` (8,319,832). Equal in `base` and `new`, and equal to the R135 values.

### Backend, process start to `load_state` answered (simulation, warm file cache)

The time from `build` to the hook cannot be measured without a window. The table takes the hook at 100 ms (an assumption; the Mac R133 launches had `setup_done` at 290-405 ms, which includes playback) and the page's request at 250 or 400 ms. The shared fixtures had been migrated to schema 8 by another branch, so the runs use copies downgraded to schema 7 (v7 cover table put back; the answer hashes match R135). 4 interleaved base/new pairs per cell, box load 37-38 the whole time (other workers), so read the direction, not the digits.

| Library | Request at | Answered at, base / new (median, ms) | Request to answer, base / new (median, ms) | New faster in |
|---|---:|---:|---:|---:|
| 10k | 250 | 253 / 255 | 3 / 5 | no change (answer ready before the request either way) |
| 10k | 400 | 405 / 403 | 5 / 3 | no change |
| 50k | 250 | 571 / 394 | 322 / 144 | 4 of 4 pairs |
| 50k | 400 | 534 / 428 | 134 / 28 | 4 of 4 pairs |

Raw 50k answered-at, base against new: request 250: 587, 606, 483, 556 against 354, 444, 395, 393; request 400: 596, 509, 510, 558 against 423, 473, 433, 422. The gain is the head start itself (the assumed 100 ms before the hook, more under load), so it is as large as the time Tauri takes from `build` to the hook and only shows where the build is longer than the page's wait: 50k. At 10k the 60 ms answer was already ready.

### What this means on the Mac (not measured)

A Mac launch has a real head start (R133: `setup_done` at 290-405 ms), and the library open is a few ms there. The saving is the time between `build` and the hook (window creation) that the build now overlaps. At 10k expect nothing visible, since the answer was already ready before the page asked in R135. At 50k expect up to the same 100-150 ms as above, only if the page asks within the build time. On a cold disk (first query 1.5-4 s on the box) it matters most: the first read now starts before the window is created.

### Not done

No real window run (rules), so the time from process start to `build` and from `build` to the hook is assumed, not measured. The cold-disk case was not run (the page cache cannot be dropped here without root).
## R153 import index (2026-11-07, `r153-import-index`)

R152 top item 1 for the import path (budget row 2). Branch from `ov-1028`. Linux `dev` box, load 16-54 the whole time (other workers), r95 50k payload, headless Chromium, `app/smoke/perf-budget.spec.ts` with `FF_PERF_BUDGET=import` (1,000 tracks into 50,000, 3 s window after the rows appear). Base = `ov-1028` in a separate tree, same spec, runs interleaved (3 rounds of base then new, 2 runs each).

### Cause

`landImportedTracks` called `resetSearchIndex()` (search text of all rows rebuilt) and the new `library.tracks` array had no bump history, so every cached column sort (`columnSorts`) was dropped and rebuilt (`valueClassesSteps` plus `sortDistinctStrings` over 51,000 values per column).

### Changed (no markup, same rows, same order)

- `library.svelte.ts`: an import no longer resets the search index. The search text list and the letter and digit sets are extended from the cursor (`fillSearchIndex` grows the typed arrays); only the new rows are built. The change log of the track list (`recordTrackListBump`) records an append (`appended`), `libraryChangesSince` reports `grown` plus the bumped positions, and `libraryPositionsChangedSince` still answers `null` across an append, so every other consumer (filter history, playlist members, sort order moves) behaves as before.
- `sortOrder.ts`: `extendValueClasses(held, values, from)` folds appended values into the held classes: new distinct values are sorted (a few), binary-searched into the held sorted distinct list, old classes are remapped by a shift table (one typed-array pass). Gives up (`undefined`, full rebuild) on NaN, values of the other kind, or no held distinct values. The sorted distinct list is kept in a `WeakMap` next to the classes, so `ValueClasses` is unchanged.
- `table-columns.svelte.ts`: `columnSortOf` reads only the appended rows (and bumped ones) and folds the classes (bumped rows plus an append falls back to a full class build); the sort warm-up refreshes stale columns (`refreshColumn`, rebuilding the orders that were held by counting sort from the folded classes) instead of building them from scratch. Loads, removals, renames that go through `loadState` are untouched (full rebuild as before).

### Before / after (Chromium 50k, import 1k, ms)

| | base (6 runs) | new (6 runs) |
|---|---|---|
| Script in the 3 s after the rows | median 713 (563-802) | median 153 (114-253) |
| Main-thread rest (paint, GC) | 180-243 | 99-146 |
| Click to rows (mock) | 31-92 | 37-107 |
| Worst frame | median 315 (177-400) | median 254 (196-591) |

Per pair (load in brackets) the script was 563/653 (17) against 157/148 (20-26), 740/686 (38) against 114/120 (38), 775/802 (49-54) against 253/196 (53). The script gain holds in every pair, also at load above 50. The worst frame is not a gain outside noise: it moved with the load (177-400 base, 196-591 new) and is not in the profile of the index work (profile of the new build: top self time `columnSortOf` 30 ms, `orderFromClasses` 15-18, `extendValueClasses` 15-35, no `trackHaystack`; run at load 50). The frame spike belongs to landing the rows themselves (row mapping, Svelte updates), not to the index.

### Identity

- New `import-index.test.ts`: imports of none, 1 and 1,000 rows (also into an empty library), copies of existing titles, artists, paths and values, renamed and moved rows bumped before and after an import, tracks removed and the library reloaded, then imports again; after every step the hash (count and ordered ids) of 12 sorts (title, artist, album, genre, bpm, key, time, label, both directions where noted, none) times 20 queries equals the one of a full `loadState` rebuild (240 of 240 each time). Warm-up refresh runs in some steps, not in others. Breaking the shift table makes it fail (checked).
- New `sortOrder-extend.test.ts`: folded classes equal `valueClasses` of the whole list (120 random splits, strings and numbers, repeats, no value, zero and negative zero, a second fold).
- Store level, base against new, hash of every ordered id, 10k and 50k: `sorted-typing-real` 432 of 432, `search-path-real` 94 of 94, `sort-analysis-real` 601 of 601 (these load, not import, so they confirm the load path is unchanged). `FF_FUZZ_FULL=1` `bulk-bump-fuzz`, `search-sort-fuzz`, `playlist-sort-fuzz`: 12 of 12.

### Validation

- `npx vitest run`: 112 files passed, 10 skipped; 978 tests passed, 9 skipped. `npm run check`: 0 errors, 0 warnings.
- Chromium smoke (`FF_SMOKE_PORT=5601`): 114 passed, 40 skipped. WebKit smoke (`FF_SMOKE_BROWSER=webkit FF_SMOKE_PORT=5602`): 114 passed, 40 skipped.

### Not done

- First sort click right after an import on a column that was never built still pays the full build (as before).
- After an import the filter history of the old list is dropped (the first query scans once, 14-22 ms at 50k).
- Bumped rows during an import (analysis running) make `columnSortOf` rebuild the classes of that column in full.

Scratch (`~/.cache/r153*`) deleted after this item.

## R156 analysis events (2026-11-07, `r156-analysis-events`)

Row 8 of R152 (analysis running, list visible: 61 ms/s at 1 event/s, 248 ms/s at 6 events/s). Branch from `ov-1028`. Linux `dev` box (load 10-53, other workers), headless Chromium, r95 50k payload, base = `ov-1028` in a separate tree with the same spec file, runs interleaved (base, new, new, base, base, new, new, base).

### Finding 1: most of the R152 number was the harness

- `perf-budget.spec.ts` answered every `get_analysis_data` from the page through a Playwright `exposeFunction`. A trace (CDP `Tracing`, main thread) of the 6 events/s window shows 51 `Receive mojo message` entries on `blink.mojom.DevToolsSession` of 52-74 KB each, 20-47 ms apiece: 1,100-1,500 ms of main thread per 8 s, about 170 ms/s. That is the DevTools channel delivering the mock payload, not app work, and the real app gets its payload over Tauri IPC. Blanking the three big waveform fields did not remove it (206-279 ms/s with the payload at 60 KB).
- The harness now fills a page-side cache with the payloads of the first 250 emitted events before the window (`startAnalysis`), and `__smokeAnalysis` answers from that cache. Base on the new harness: 17 ms/s at 1 event/s and 71-114 ms/s at 6 events/s (R152: 61 and 248).
- New part `FF_PERF_BUDGET=analysis-list` (list only, 1 and 6 events/s, `FF_PERF_WINDOW` seconds each, busy per second, script, layout+style, GC share from a 1 ms CPU profile, load). `FF_PERF_TRACE=<prefix>` writes the main-thread trace, `FF_PERF_DUMP_PROFILE=<prefix>` the CPU profile.

### Finding 2: what was left, and the change

- Events already arrive batched: `queueAnalysisRefresh` collects ids and `applyQueuedAnalysis` applies them at most every 250 ms with one `bumpLibraryTracks`. R130 already copies only the touched fields. So the two ideas from R152 (copy fields, batch per frame) were in the code; nothing batches coarser or finer now, and event timing is unchanged.
- CPU profile of the 6 events/s window (self + callees): the Svelte flush after each bump re-ran `library.sidebarData`, one getter that builds all four sidebar sections and also reads `this.tracks.length`. A bump replaces `library.tracks` (new array, same length), so every bump rebuilt favorites, the playlist tree (`sidebarTreeRows`, 80 ms in 10 s) and the USB rows, and the sidebar compared new objects. Flush time dropped from 345 to 98 ms in the same 10 s window, GC from 127 to 13 ms.
- Change (`app/src/stores/library.svelte.ts`): `trackListLength = $derived(trackList.length)`; `library.trackCount` and the "All Tracks" count read it. A bump leaves the number unchanged, so nothing that reads only the count (sidebar, status bar, settings) is marked dirty. Values on screen are the same; the count still follows every replacement or append of the list.

### Before / after (Chromium, 50k, interleaved pairs, run = 12 s window, mean busy ms/s over the window, load = 1-minute average at the end of the run)

| Metric | base, 4 runs | new, 4 runs | Pairs (base to new, same round) |
|---|---|---|---|
| 6 events/s, busy ms/s | 71.2, 108.9, 114.4, 72.0 (median 90) | 57.0, 65.4, 65.7, 53.3 (median 61) | 71 to 57, 109 to 65, 114 to 66, 72 to 53 |
| 6 events/s, script ms/s | 35.7, 52.0, 64.6, 35.1 | 14.2, 13.2, 23.3, 15.2 | script ranges do not overlap |
| 6 events/s, layout+style ms/s | 4.6, 4.2, 3.2, 4.7 | 3.7, 4.0, 2.7, 4.4 | no change |
| 6 events/s, GC ms/s (1 ms profile) | 1.6, 14.4, 5.0, 1.7 | 0.8, 13.7, 1.8, 2.2 | inside noise (GC share follows load) |
| 1 event/s, busy ms/s | 40.5, 52.1, 86.5, 32.4 | 40.1, 32.2, 79.5, 35.2 | inside noise |
| 1 event/s, script ms/s | 15.2, 18.6, 28.1, 11.1 | 6.6, 5.7, 12.9, 10.3 | lower in 4 of 4 pairs |
| idle list (no analysis), busy ms/s | 2.9, 3.9, 22.6, 2.7 | 3.8, 17.4, 3.3, 26.0 | noise (load 10-53) |
| load (base / new) | 10.3, 30.4, 50.9, 9.9 | 30.1, 32.4, 53.0, 35.5 | new runs followed a base run, so their load is higher by the leftover |

Claimed: script time at both rates is down by more than the noise (at 6 events/s 47 to 17 ms/s on average), and main-thread busy time at 6 events/s is lower in every pair (about 30 percent at the median, 90 to 61). Not claimed: busy time at 1 event/s (a few ms, hidden in the swing of the box), GC share, layout and style, idle. What is left at 6 events/s is about 10 ms per event: a 50k-pointer `slice` per bump (about 0.4 ms), Svelte marking and re-checking the table and row deriveds, and the 1 ms profiler of the harness itself.

### Identity

- Store level, hash of every ordered id of every result, base against new, 10k and 50k: `sorted-typing-real.test.ts` 432 of 432, `search-path-real.test.ts` 94 of 94, `sort-analysis-real.test.ts` 601 of 601 (all six runs equal).
- `FF_FUZZ_FULL=1` `bulk-bump-fuzz`, `search-sort-fuzz`, `playlist-sort-fuzz` and `import-index.test.ts`: 17 passed.
- New `analysis-events.test.ts`: the same event stream (4 frames, 13 events, partial and finished, some ids in several frames) applied together per frame and one event per apply interval; after every frame the plain rows (bpm, key, analysed, pending, loaded, preview length), the analysing ids, the track count, the sidebar count, 5 search results and 6 sorted views (3 columns, with and without a query) are `toStrictEqual`. Second test: the track count and the sidebar "All Tracks" count equal the list length across a bump and a replacement.
- Timing: the events are applied exactly when they were before (same 250 ms apply interval, one bump per batch). An event can land at most one apply interval later than the same event sent alone, as before this change; nothing in R156 moves it.

### Validation

- `npx vitest run`: 113 files passed, 10 skipped; 980 tests passed, 9 skipped. `npm run check`: 0 errors, 0 warnings.
- Chromium smoke (`FF_SMOKE_PORT=5621`): 114 passed, 41 skipped. WebKit smoke (`FF_SMOKE_BROWSER=webkit FF_SMOKE_PORT=5622`): 114 passed, 41 skipped.

### Not done

- The array copy in `bumpLibraryTracks` and the table-side effects are as in R139.
- The harness cost is about 30 ms per event on the page thread for a 60 KB payload through DevTools; real Tauri IPC cost per payload was not measured here (no window).

Scratch (`~/.cache/r156*`) is deleted after this item.

## R157 first clicks (2026-11-07, `r157-first-clicks`)

Branch from `ov-1028`. Linux `dev` box, load 15-43 from other workers during every run, headless Chromium 50k (r95 payload) and Playwright WebKit. Base = `ov-1028` in a second worktree, same spec file, runs interleaved (base, new, base, new, ...), 3 pairs for Chromium, 2 for WebKit.

### Findings

- The R152 cause was only half right. The warm-up never blocks a click: it runs in 6 ms slices and no long task came from it (`tasks` part, 6 s after the first rows: the only long tasks were the search text build in the first 170 ms). The 227 ms first click is the clicked column being built cold, because the title warm-up had not reached it yet (it started 1,000 ms after the list loaded). Clicks at +0 to +1,000 ms after the first rows cost 190-240 ms of script in one 160 ms task; from +1,500 ms on 25-50 ms.
- Harness inflation: in headless Chromium without frames `requestIdleCallback` only fires at its timeout (R134), so the warm-up looked far slower than in a window; the new `early` part therefore runs an empty `requestAnimationFrame` loop (`FF_PERF_FRAMES=1`). The first select in `flows` also waited for the 60 KB analysis payload delivered through DevTools (R156): the harness now answers it from a page-side template (`answerAnalysisLocally`).
- The `Detail` chunk was already prefetched after the first screen (`preloadAfterFirstScreen`, in `lazy.svelte.ts`); it was loaded by +300 ms in every run. What a first select costs is the mount (about 120-150 ms on an idle box, script 25-115 ms, GC and layout), which the task forbids prefetching. Only change: `inspector` is first in the prefetch queue (it was second, behind `AppDialogs`), so the chunk is in at +0.

### Changed (no visible change)

- `table-columns.svelte.ts`: sort warm-up delay 1,000 to 250 ms (the title column is warmed first, as before); the warm-up pauses 400 ms after a pointerdown, keydown or click and stops a slice when `navigator.scheduling.isInputPending()` is true (Chromium; WKWebView has only the quiet time); a click on a column whose warm-up job is in flight finishes that job instead of building the column again (`finishColumn`, not re-entrant while the job runs).
- `lazy-views.ts`: `inspector` before `appDialogs`.
- `perf-budget.spec.ts`: parts `early` (sort or select at fixed delays after the first rows, `FF_PERF_CLICK_AT`, `FF_PERF_ACTIONS`) and `tasks`, `FF_PERF_FRAMES`, `FF_PERF_TOP`, local analysis answers.

### Before / after (Chromium, 50k, `early`, sort title; ms click to painted, 3 interleaved pairs, medians; script ms in brackets)

| Click at | base | new | Verdict |
|---|---|---|---|
| +0 | 507 (332-513) | 400 (235-432) | cold either way, not claimed |
| +300 | 608 (312-566) | 304 (126-308) | lower in 3 of 3 pairs (an earlier batch, 3 pairs: 276 to 143) |
| +600 | 587 (305-742) | 204 (51-204) | lower in 3 of 3 pairs (earlier batch 256 to 194) |
| +1000 | 211 | 268 | not better, ranges overlap, not claimed (earlier batch 134 to 192, worse) |
| +1500, +3000 | 209 / 184 | 225 / 195 | no change |

Both batches ran at load 15-43, base values in the first early2 pairs are inflated by load (script is the safer column). First select (`select`, same delays): 131-247 base, 121-240 new, no difference, not claimed. Cold open to first rows (`boot`, 3 interleaved pairs, harness payload inflates it): 2,451 / 2,533 / 2,661 base, 2,444 / 2,661 / 2,570 new, same; TBT 537-609 base, 526-680 new, same; click at +0 297 / 315 / 272 base, 282 / 333 / 277 new (cold build both). WebKit (2 pairs, wall time only, load 21-32): sort +600 398 / 351 base, 255 / 261 new; +0 and +1500 mixed (+1500 worse in new, 523 against 357); select mixed; nothing claimed.

Net: the first click is faster only when it lands about 250-700 ms after the first rows. Before 250 ms the column must be built cold and nothing can fix that without doing the work before the click; the +1000 ms window is not better and may be a little worse (the warm-up job in flight is finished by the click, but the box was saturated). The select goal is not met: the prefetch already existed.

### Identity

Store level, hash of every result, base against new, `FF_PERF_STATE` r95 payloads: `sorted-typing-real` 432 of 432 (10k and 50k), `search-path-real` 90 of 90 (10k and 50k, one repeat), `sort-analysis-real` top-level digests 3 of 3 (10k and 50k). `FF_FUZZ_FULL=1` fuzz tests (`search-sort-fuzz`, `playlist-sort-fuzz`, `bulk-bump-fuzz`), `import-index.test.ts`, `analysis-events.test.ts`: 19 passed. Orders cannot differ by construction: the same generators build the same entries, only the start time and who drives the generator changed. Not done: page-level row hashes after a sort click (the existing R115 UI hash spec was not re-run).

### Validation

`npx vitest run` with `FF_FUZZ_FULL=1` and the 50k payload: 989 passed, 2 skipped, 1 failed (`playlist-path-real`, needs a playlist name of another payload; not related). `npm run check`: 0 errors. Chromium smoke: 114 passed, 43 skipped. WebKit smoke: 114 passed, 43 skipped.

Scratch (`~/.cache/r157*`) is deleted after this item.
## R158 Mac analysis ship (2026-11-07, `r158-mac-analysis`)

Merlin's decision (2026-11-07, Waiting 43 and 44: "1 yes"): beat chunk 500 on every build, the Mac too, and the int8 beat model with weight prepacking on the Mac. x86 keeps fp32. Branch `r158-mac-analysis` from `ov-1028` (7e00d28).

**What went in.** `r108-throughput` (1ee34d2, which carries `r96-chunk500`, the R79 fold peak, R101 with the half-time band off, R102 switches and `r105-snap` 903b702) as one merge; then `r98-mac-pair` (87548eb, with `r97-int8-500`: the quantize script and docs); then `r150-int8-prepack` (e779648). Conflicts: `service.rs` tests (kept ov-1028's R93 ceiling test, gave it r108's activation argument) and this doc (both sides kept, R108 placed after R106).

**Code on top** (`fourfour-analysis-rs/src/bt.rs`, `service.rs`):

- `DEFAULT_CHUNK_SIZE` is 500 everywhere; the CoreML-only 1500 is gone. `FOURFOUR_BEAT_CHUNK` still overrides.
- `DEFAULT_BEAT_MODEL` is `beat_this_q8sc.onnx` on macOS (`cfg!(target_os = "macos")`), `beat_this.onnx` elsewhere. `FOURFOUR_BEAT_MODEL` still overrides. Prepacking follows the file (`prepacks_weights`: any file but fp32), so the Mac gets R150's prepack and x86 does not.
- Runtime: in an `ort`/`coreml` build the beat model goes to onnxruntime when `FOURFOUR_BEAT_RUNTIME` names anything but `rten`, or when it is unset and the model is the fp32 file. The int8 model runs on rten unless asked otherwise. So the Mac app (still built with `--features coreml`) runs int8 on rten by default, and `FOURFOUR_BEAT_MODEL=beat_this.onnx` alone brings back CoreML (at chunk 500; add `FOURFOUR_BEAT_CHUNK=1500` for the old path) if the fp32 file is in the models dir.
- `bench_analysis`'s per-stage profile loads the default model (it loaded fp32 by name before).

**The int8 file.** Not in git (like the fp32 weights). `scripts/mac/int8-model.sh [models dir]` makes it from the sha-checked `beat_this.onnx` with `fourfour-analysis-rs/scripts/quantize_beat_int8.py --per-channel` (R97) in a throwaway venv (`~/.cache/fourfour-quant-venv`, pinned onnx 1.23.2, onnxruntime 1.31.1, numpy 2.5.4) and checks the result: sha256 `f315e4684715999897fea9d9c7581e53b43fa73dc32f96aabee0cc58d9b2fe78`, 23,905,596 bytes. On dev it reproduces that hash exactly, from the R97 venv and from a fresh venv with the pins. If the Mac's wheels give other bytes it stops and says so; `FOURFOUR_INT8_MODEL=<file>` then copies a known file (dev: `~/.cache/r97-models/beat_this_q8sc.onnx`) and still checks it. `scripts/mac/build-dmg.sh` and `fourfour-analysis-rs/scripts/bundle-app.sh` (on Darwin) run it, then bundle with `pioneer-test-ui/tauri.bundle-weights-mac.conf.json`: int8, mel, `LICENSE-beat-this.txt`, `LICENSE-key-model.txt` (the fp32 file is not bundled on the Mac: 83 MB the app never loads). `check-dmg.sh` checks the int8 file. Licence: the int8 file is the MIT Beat This! weights, quantized; the same `LICENSE-beat-this.txt` ships next to it, and the notices entry (`scripts/release/third-party-extras.json`, `pioneer-test-ui/resources/THIRD-PARTY-NOTICES.txt`) names it.

**Battery.** No branch had a "fewer workers on battery" cut in code; r98 only recommended one because rten int8 used 2.7x the CPU of CoreML 1500 (988 vs 368 s). Mac night 3 measured int8 at 500 at 614 CPU s against 644 for CoreML 1500, so the reason is gone. Not added.

**Acceptance on x86 (dev box, fp32, chunk 500, at most 2 analysis threads).**

- Output identity with `r108-throughput`: `analyze-json --threads 2 --workers 1` over 8 tracks of `rekordbox-testdata-2025` from the test split (3 MP3, 3 FLAC, 2 WAV; full app result: grid, BPM, key, waveforms) is byte-identical, 8,276,547 bytes, sha256 `974a7f995d45b03e63e01c97b7cd442c13339dfa01f4f92ec74ace30dbc0e90b`, both builds, 8 of 8 analysed. CPU 366 s (r158) and 370 s (r108).
- Scores (`benchmark/scripts/r94-run.sh` with this branch's `ffa`, `WORKERS=1 THREADS=2`, fp32, `o3-score.py`): test 205: grid 20 ms **171**, BPM 0.1 **185**, BPM 1 193, bar right 142 of all. These are exactly the R105 chunk-500 numbers that R108 carried (171 / 185 / 193 / 142; gate 171 = 171 passes, BPM 0.1 equals R108's 185, one under the 1500 reference as before). 46.1 CPU s per track, 1.97 tracks/min at load 15-50, peak RSS 678 MB. The dev split was not rerun (timebox; the box was at load 50 for half the run); the byte identity above covers the code path.

Validation: `cargo test --workspace`: 26 `test result: ok` lines, 587 passed, 0 failed (new: `every_build_uses_the_500_frame_chunk`, `only_the_fp32_model_goes_to_onnxruntime_by_default`, `the_mac_defaults_to_the_int8_beat_model_and_the_rest_to_fp32`). `cargo check -p fourfour-analysis-rs --features ort --tests` clean. `app`: `npx vitest run` 980 passed, 9 skipped; `npm run check` 0 errors, 0 warnings (app code unchanged). Not checked: `aarch64-apple-darwin` (target not installed on dev), the Mac build and the `coreml` feature on the Mac.

**Mac checks for Merlin** (not run here: no Mac access, no `aarch64-apple-darwin` target on dev). See the commands in the report: int8 file, `build-dmg.sh`, `check-dmg.sh`, a 49-track `bench_analysis` against night 3's int8 row, and a look at `beat_runtime` in the app's analysis status (should read `rten`).

## Mac night 3 (prepared 2026-11-05, `r125-mac-night3`, not run)

One entry runs every pending automatic Mac check and writes one summary. Nothing in it needs a click. Every app launch uses `HOME` inside the night directory with a copy of the 10k fixture library; the app is built into the night directory and never installed to `/Applications`. Before the first launch the script refuses any scratch path outside `build/mac/night3-<date>/` or pointing at Application Support, `~/Library/Pioneer` or rekordbox, and checks the library path in the scratch `config.json`. The real `Pioneer`, `com.pioneer.test-ui` and rekordbox folders are hashed (find and stat, read only) before and after; a change fails the run.

**Go (from the dev box, after Merlin's go):**

```bash
cd /home/merlin/dev/projects/fourfour/.claude/worktrees/agent-a1492114039f883d9 && scripts/mac/night3.sh --remote
```

It bundles the branches, copies them and the int8 model to `mbp` (the `ship-dmg.sh` method: `git bundle` + rsync, GitHub never touched), starts the run there under `caffeinate -i` with `nohup`, waits, and copies `summary.md` and the per-item results back to `build/mac/night3-<date>/` on the box. If the ssh session drops the Mac run goes on; `scripts/mac/night3.sh --remote --collect` waits for it and fetches. `--only 3,4` reruns single items. `--dry-run` prints every command (nothing runs).

**mac-queue does not carry this.** `mac-queue add` knows Swift/Xcode job kinds and rejects arbitrary commands, and it has no fourfour profile. Earlier fourfour nights (R89, R26) ran by `ssh mbp` on Merlin's go; this does the same with one command. A `night` job kind in lab-infra would be the clean fix, and is not part of this task.

| # | Item | How | Gate |
|---|---|---|---|
| 1 | ov-1028 (r109 early load_state, r111, r113-r115, r114 lazy dialogs, r120 binary load_state, r121 mount): build, scratch-HOME launch, rows on screen, first row, interactive, first Settings open | `cargo tauri build --features coreml --bundles app` from a worktree of `ov-1028`; one launch with the perf probe, soak action `settings` (cycle 1 is the first, lazy open) | opens, first row and interactive marks, first row within 15% of the last cold Mac number (868 ms, R89) |
| 2 | `perf-night.sh --size 10k --runs 3 --expand-runs 5` on that app | the existing script, from the worktree, scratch output under the night directory | warm first row, warm interactive, cold first row within 15% of R89 (628 / 645 / 868 ms), first expand median under 50 ms, scroll during analysis p95 under 25 ms |
| 3 | r110 first result on a quiet machine | `scripts/mac/night3-first.py` (macOS port of `r110-first.py`), rten (the burst is rten only), 24 masters, 6 interleaved rounds, a trial starts at 80% idle or after 60 s | r110 median below ov-1028, and at least 3 quiet trials per build |
| 4 | Chunk 500 on CoreML (Waiting 43) and int8 rten at 500 (Waiting 44) against CoreML 1500 | `bench_analysis` of `r98-mac-pair` (the only build with `FOURFOUR_BEAT_CHUNK` / `FOURFOUR_BEAT_MODEL`), 200 fixture tracks, 3 interleaved rounds, plus the ov-1028 binary as a drift row; `time -l` for RSS and CPU | tracks/min at least 95% of CoreML 1500 and peak RSS not higher (the r98 gate). Quality is not scored here |
| 5 | cores-1 against cores-2 with the UI busy (Waiting 29) | `bench_analysis` of ov-1028, `--threads cores-1` / `cores-2`, `--ui-busy 2000/6000`, 24 masters, 3 interleaved rounds | runs clean; the throughput gain is the answer (r108 predicted +13% on 16 cores, +24% on 8) |
| 6 | Ears and eyes | not automated, see below | Merlin |

**Expected duration:** about 2.5 hours on `mbp` (cold builds of four targets in one shared cargo target about 25 min, items 1-2 about 35 min, item 3 about 15 min, item 4 about 25 min, item 5 about 6 min, plus waits for quiet). A window opens for each app launch in items 1-2 (about 25 min in all); leave the Mac alone while items 2-5 run, the numbers want a quiet machine. **Free disk needed:** 35 GB at the start (the script refuses below that; `NIGHT3_MIN_FREE_GB`): one shared cargo target 15-20 GB, three worktrees with `node_modules` about 3 GB, app and bench binaries 0.5 GB, a copy of the 10k library for one launch, models 0.5 GB. The 10k fixture (`~/.cache/ff-fixture/10k`), the beat models (`~/.cache/fourfour-models`) and an ssh path from the box are assumed present, as in R89. `--clean` removes the worktrees and scratch at the end; the cargo target stays (delete `build/mac/night3-target` by hand when done, see "Prune worker caches").

**Reading the summary.** `summary.md` has one row per item (PASS, FAIL, or SKIP with the numbers), the protected-paths result, then one table per item. FAIL means a gate above was missed, not that the script broke; a broken step says so in the row and in `night.log`. Item 3 prints FAIL when the machine was not quiet, so the number is not read as a verdict.

**Things this does not cover.** The Settings number is a proxy (a whole soak cycle; the probe has no mark for the first dialog open, adding one is a change in `pioneer-test-ui/src/perf_probe.js`, outside this task). Item 4 has no quality score (the o3 reference set is not on the Mac); the r96/r98 scores stand. r123 is not in ov-1028, so the r123 check below needs its own install.

### Merlin's ears and eyes (phone checklist, not automated)

1. **r123 audio (branch `r123-idle-cpu`, needs a Mac install of that branch first, ask for it).** Long track (over 6 min). Set the buffer to 32, then 64 frames. Play 2 min: any click or dropout? Seek 5 times (bar jumps, scrub): any glitch? Minimise the window for 30 s while playing, bring it back: sound unbroken, meters alive again at once.
2. **Waiting 16, bar 1 (b15-bar1 e717de0).** Open the ten tracks listed in `docs/analysis/rust-status-2026-10-28.md` ("the last five are the regressions to listen to"). For each: does bar 1 sit on the first downbeat as Rekordbox has it? Verdict per track: ok / wrong. Land the rule if the five regressions are ok.
3. **Waiting 39, user docs marks (branch `r53-user-docs-v2` 56f7625, `docs/user/`).** 20 "(to confirm)" marks: faq 10, usb 3, known-issues 4, install 1, quick-start 1, README 1 (`grep -n "(to confirm)" docs/user/*.md`). Read on the phone, confirm each on the Mac: right, or what the app really does. Send back the list of answers.
