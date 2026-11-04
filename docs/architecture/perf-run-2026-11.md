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

**Guards added.** `smoke/leaks.spec.ts` (in the normal smoke run) opens and closes the palette, settings (three pages) and the expanded player 6 times and asserts listeners (+2), registered callbacks (0) and DOM nodes (+40) after a GC. No leak was found there, so it passes before and after; it pins what was verified.

**What is left**

- `artwork.cache` (the 512 px covers of the inspector and player) is still not capped: one blob per track selected, about 40-60 KB each. Capping it needs to know which cover the player and inspector still show (an evicted URL under a visible image would blank it), so it is left for a change that may touch the UI.
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
