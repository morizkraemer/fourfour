# perf-startup: startup at 10k and 25k (2026-11-21)

Branch `perf-startup` from `master` (`eb236d3`). Linux `dev` box (16 cores), headless Chromium, release `bench_startup`, the `ff-fixture` 10k, 25k and 50k libraries (read only). Other workers ran the whole time (load 7-43); every comparison below is interleaved A/B, base and new served side by side from prebuilt dists (`vite preview`), `perf-q50.spec.ts` with `FF_PERF_Q50_ONLY=startup` and the fixture's real `load_state` payload, so it measures the same frontend rows as `scripts/perf.sh --size`.

## Where the time goes (10k, traces of the head build)

Chromium trace of a cold load and a warm reload (scratch spec, same mock and payload as `perf-q50`), main thread:

| Part | cold | warm | whose |
|---|---:|---:|---|
| Mock init script carrying the whole payload as a literal (`LocalWindowProxy::Initialize`) | 250-620 ms | 38-68 ms | harness |
| `load_state` answered by the mock from the inline script in `index.html` | 28-55 ms | 29-53 ms | harness (stands in for the IPC) |
| Bundle evaluation, mount, answer, row mapping and the first rows, one task after this branch | 400-440 ms | 170-200 ms | app |
| of which `evaluateModule` (compile on a cold load) | 140-170 ms | 40-80 ms | app |
| of which the first table rows (`TrackRow` x 33, fenced) | about 45-60 ms | about 45 ms | app |
| First-row frame (commit about 25 ms: the row canvases) | 55-95 ms | 55-80 ms | app |
| Playwright's injected script (`page.locator().waitFor()`) | 35-124 ms | 35-50 ms | harness |
| After the first row: overscan rows 33-43 ms, sidebar playlist rows 96-124 ms (JS 35, commit 24), GC | | | app |

Before this branch the three map yields sat between the answer and the first rows: each let a skeleton frame (layout 35-70 ms), the injected script or a GC run first, so the first rows came 250-400 ms after the answer although mapping 10k rows takes 10-25 ms.

Backend: the app's `load_state` is `loaded_state` + `to_bytes` (binary answer, parallel table scan). As the first query of a fresh process (what the app's early-state thread sees) it takes 68-89 ms at 10k on this box with a warm page cache and 284-296 ms after an eviction (`library.db` is 1.5 GB with the covers): labels and playlists 6-12 ms, the table scan 57-74 ms. The three scan threads open their own connection and take 38-54 ms each for a quarter of the rows while the calling thread does its quarter in 17-28 ms, so the four-way split gains little here (one thread: 68-81 ms in the same runs). The bundled SQLite is built with `SQLITE_ENABLE_MEMORY_MANAGEMENT` (libsqlite3-sys), which puts every connection's page cache behind one global mutex; switching `SQLITE_CONFIG_MEMSTATUS` off brought the threads to 24-38 ms but the whole call only to 67-75 ms. In the real app this time is hidden at 10k: the early-state thread has the answer ready before the page asks (R135/R154).

## Steps

| # | Step | Kept | Before | After | How measured |
|---|---|---|---|---|---|
| 1 | Map up to 10k tracks in one task (`TRACK_MAP_CHUNK` 2,500 to 10,000) | yes | 10k cold first row 1,190, interactive 1,420; warm 716 / 986 ms | 785 / 973; 444 / 648 ms | 10k, 3 rounds x 5 runs, load 19-32, median of round medians; new lower in 3/3 rounds on all four |
| 2 | Map up to 25k in one task (10,000 to 25,000) | yes | 25k (after step 1) cold first row 1,393, interactive 1,613; warm 784 / 968 ms | 1,286 / 1,563; 554 / 733 ms | 25k, 3 rounds x 3 runs, load 9-16; lower in 3/3 on all four |
| 3 | `bench_startup run`: `load_state_ms` times the app's call (`loaded_state` + `to_bytes`) instead of the single-thread list + JSON encode, at the same place in the run (after the row query that warms the page cache); the old sum stays as `json_path_ms` | yes (measurement fix) | warm 76 ms, cold 244 ms (old sum) | warm 78-83 ms, cold 188 ms (the real call, same runs) | 10k, fresh processes, load 6-15. A first version timed the call first after the open; after a page-cache eviction that is 284-296 ms (the old first query, the row list, 355-376 ms) and once 12.5 s while other workers hit the disk, so it would have moved the cold composite by more than the app changed; kept at the old place so the `now` values stay comparable |
| 4 | Preload the two Latin font files and wait for them (at most 50 ms) before mounting | no | 10k cold first row 726 / 734 / 688, warm 361 / 359 / 369 ms | 693 / 714 / 664; 354 / 356 / 364 ms | 3 rounds x 5 runs, load 7-24; 3/3 lower but about 25 ms cold, 5 ms warm; it also removes the fallback-font flash of a cold start (the fonts arrive after the long mount task today) and can hold the mount up to 50 ms, so it is a visible change and listed below |
| 5 | Scan thread count 1 / 2 / 4 / 6 / 8, `SQLITE_CONFIG_MEMSTATUS` off (scratch builds) | no | 4 threads 72-89 ms | 1 thread 68-81, 8 threads 90-102, memstatus off 67-75 ms | 10k and 25k fresh processes, load 12-25; no clear win, global SQLite config, not kept |

25k against base (steps 1 and 2 together, 3 rounds x 3 runs, load 11-21): cold first row 1,222 to 1,177 ms, warm 623 to 551 ms; interactive unchanged (1,405 / 1,413 cold, 753 / 758 warm). 50k (2 rounds x 3 runs, load 7-10): cold 1,894 / 1,963 to 1,793 / 1,888 ms, warm 907 / 982 to 911 / 911 ms, no regression.

The realistic harness (`perf-startup.spec.ts`: payload fetched over HTTP when the page asks, backend simulated at 77 ms) shows no change from steps 1-2 at 10k (first row 590-620 ms either way): there the answer arrives after the skeleton is already painted. The gain is for the case where the answer is in hand when the app mounts, which is the q50 harness and the real app at 10k-25k with the early state.

## Final numbers (`scripts/perf.sh --size`, head)

`FF_PERF_SKIP=load,import,analysis,playback scripts/perf.sh --size 10k|25k` on `034ef3e` (load 10-18). Proposed `now` values (not written to `scripts/perf-budgets.json`):

| Metric | 10k now (budget file) | 10k measured | 10k target | 25k now | 25k measured | 25k target |
|---|---:|---:|---:|---:|---:|---:|
| startup_cold_first_row | 1,085 | 857 | 500 | 2,212 | 1,310 | 900 |
| startup_cold_interactive | 1,319 | 1,021 | 550 | 2,815 | 1,537 | 1,000 |
| startup_warm_first_row | 694 | 448 | 300 | 1,660 | 637 | 600 |
| startup_warm_interactive | 748 | 633 | 330 | 1,748 | 818 | 650 |
| startup_backend_warm | 108 | 100 | 60 | 248 | 134 | 150 (met) |

Frontend parts of those rows: 10k cold first row 712 / interactive 876, warm 348 / 533; 25k cold 1,078 / 1,305, warm 504 / 684. Backend cold (after the eviction): 145 ms at 10k, 232 ms at 25k. The `now` column is from 2026-10-31; most of the drop since then comes from the startup items already on `master` (R95-R163). What this branch adds is the A/B above: about 400 ms off the 10k cold first row and 270 ms off the warm one, 45-70 ms at 25k. One earlier 25k run had the cold backend at 3.4 s (and a 10k run 12.5 s) while other workers loaded the disk; the reruns after an eviction gave 259-260 ms at 25k, so those were the disk, not the code.

## Options that need a decision

1. **Make the q50 startup harness deliver the payload like the IPC does.** Today the mock carries the whole decoded state as a literal in its init script (250-620 ms of a cold load at 10k, more at 25k) and Playwright's injected script lands in the measured window (35-124 ms). Fetching the payload when `load_state` is called (as `perf-startup.spec.ts` does) and signalling the first row through an exposed binding would take roughly 0.3-0.7 s off the cold composite without any app change. It changes what the `now` values mean, so the budget file would need a fresh baseline.
2. **Fonts before the first layout** (step 4): about 25 ms cold, and the first cold frame paints in Geist instead of the fallback font. Visible (no font swap any more) and holds the mount up to 50 ms.
3. **Sidebar playlist rows and overscan rows in smaller pieces after the first rows.** The 96-124 ms reveal task is what ends "interactive" now; splitting it means the off-screen part of the playlist list (and the off-screen table rows) appear a few frames later. R163 found a frame-by-frame reveal slow in WebKit, so it needs a WebKit check.

## What needs the Mac

- The real launch: R133 measured Rust `setup_done` at 290-405 ms and the page part at about 190 ms; which part of the setup hook takes the time is not known (`PlaybackEngine::new` no longer opens the device, it is lazy now). A launch with `FF_PERF_LOG` and a mark per setup step would show it.
- Audio device opens at mount: `App.svelte` calls `syncPlaybackAudioConfig()` in `onMount`, which sends `playback_set_output_device` and then `playback_configure`. Both are sync Tauri commands, so they run on the main thread, and each one drops and reopens the CoreAudio output stream (`open_output`). That is two device opens on the main thread while the page waits for its first IPC answers. Lead: one open instead of two, and the commands as `#[tauri::command(async)]`, if cpal allows the open off the main thread on macOS. Not measurable here (no audio device on the box, so the open fails fast).
- Whether steps 1-2 show on the Mac: the early answer reaches the page during or right after the mount there too (R154), so they should, but it is not measured.

## Validation

`cargo test --workspace`: all 20 test binaries ok (before the bench placement amend; after it `cargo check -p pioneer-test-ui --features dev-bins --bins` and the `bench_startup` tests, 90 passed). `npx vitest run`: 125 files passed, 11 skipped; 1,091 tests passed, 10 skipped. `npx svelte-check`: 0 errors, 0 warnings. Chromium smoke (`FF_SMOKE_PORT=5491`): 193 passed, 48 skipped. WebKit smoke not run.

Scratch (`~/.cache/perf-startup-work`, `~/.cache/perf-startup-target`) is deleted after this item.
