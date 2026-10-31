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
