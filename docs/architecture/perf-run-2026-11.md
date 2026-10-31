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
