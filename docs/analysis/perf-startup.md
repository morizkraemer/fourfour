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
| 6 | q50 harness (orchestrator decision, option 1): the mock no longer carries the payload in its init script. `perf-q50.spec.ts` serves it from a local HTTP server; the mock fetches it when the page starts (standing in for the early-state thread) and answers `load_state` with the dumped table JSON, which the app decodes as it decodes the IPC answer. Row previews come from the same server per call; the decoded track list is fetched only for the stick commands that read it | yes (measurement change, no app change) | see the two tables below | | Same app build, old harness against new, 10k, 3 rounds x 1 run, load 4-6: cold first row 735 / 775 / 766 to 593 / 527 / 620 ms, warm 366 / 392 / 344 to 331 / 347 / 365 ms; import click to rows 38 / 59 / 30 to 45 / 40 / 55 ms (rows after the commit answer 19-31 to 31-38 ms, so about 10 ms more, cause not pinned down) |

25k against base (steps 1 and 2 together, 3 rounds x 3 runs, load 11-21): cold first row 1,222 to 1,177 ms, warm 623 to 551 ms; interactive unchanged (1,405 / 1,413 cold, 753 / 758 warm). 50k (2 rounds x 3 runs, load 7-10): cold 1,894 / 1,963 to 1,793 / 1,888 ms, warm 907 / 982 to 911 / 911 ms, no regression.

The realistic harness (`perf-startup.spec.ts`: payload fetched over HTTP when the page asks, backend simulated at 77 ms) shows no change from steps 1-2 at 10k (first row 590-620 ms either way): there the answer arrives after the skeleton is already painted. The gain is for the case where the answer is in hand when the app mounts, which is the q50 harness and the real app at 10k-25k with the early state.

## Final numbers (`scripts/perf.sh --size`, head)

`FF_PERF_SKIP=load,import,analysis,playback scripts/perf.sh --size 10k|25k`, load 3-8 (a quiet window). Old harness on `034ef3e`; new harness on `504da61`. Proposed `now` values are the new-harness column (not written to `scripts/perf-budgets.json`):

| Metric | size | budget-file now | old harness | **new harness (proposed now)** | target | left to target |
|---|---|---:|---:|---:|---:|---:|
| cold first row | 10k | 1,085 | 857 | **616** | 500 | 116 |
| cold interactive | 10k | 1,319 | 1,021 | **794** | 550 | 244 |
| warm first row | 10k | 694 | 448 | **379** | 300 | 79 |
| warm interactive | 10k | 748 | 633 | **550** | 330 | 220 |
| backend warm | 10k | 108 | 100 | **65** | 60 | 5 |
| cold first row | 25k | 2,212 | 1,310 | **843** | 900 | met |
| cold interactive | 25k | 2,815 | 1,537 | **1,037** | 1,000 | 37 |
| warm first row | 25k | 1,660 | 637 | **475** | 600 | met |
| warm interactive | 25k | 1,748 | 818 | **640** | 650 | met |
| backend warm | 25k | 248 | 134 | **107** | 150 | met |

The old-harness column was measured at load 10-18, the new one at load 3-8, so part of the gap between those two columns is the quieter box; the same-build A/B in step 6 is the clean harness effect (about 140-250 ms cold, 0-35 ms warm at 10k). The `import UI click to rows` budget (now 38.7 ms at 10k) fails with the new harness (62-74 ms in the perf.sh runs, 40-55 in the A/B; target 100 is met) and needs the same rebaseline.

### What is left, and who owns it (10k)

- **First row.** The composite adds the backend serially: cold 109 + frontend 506 ms, warm 65 + 314 ms. In the real app the early-state thread builds the answer while the webview starts, so at 10k the backend is mostly off the path; the frontend alone is at or near the targets (cold 506 against 500, warm 314 against 300). What the frontend still spends: bundle evaluation and mount (cold compile 140-170 ms), the first table rows (`TrackRow` x 33, fenced) and the first-row frame (50-90 ms, about 20 ms of it the compositor commit of the row canvases). No invisible lever of more than about 20 ms was found there.
- **Interactive.** 170-180 ms after the first row, all app-owned: the first-row frame (50-90 ms), the overscan rows (about 30 ms) and the sidebar playlist reveal (88-106 ms: 30-41 ms for 82 `SidebarRow`s, the rest layout, paint and a 20 ms commit). On a cold start one more relayout (about 45 ms) follows when the non-Latin font subsets arrive. The reveal is the task that ends "interactive"; cutting it means showing the off-screen playlist rows a few frames later (option 3 below), which is a visible change, so no further A/B step was taken.
- **Harness left in the window:** Playwright's injected script (`locator().waitFor()`, 30-50 ms), and the mock's HTTP fetch and JSON parse of the answer (the real app decodes a 1.7 MB binary answer instead).

## Options that need a decision

1. **Rebaseline `scripts/perf-budgets.json`** with the new-harness column above (done in step 6, decided by the orchestrator). Not done: signalling the first row through an exposed binding instead of `locator().waitFor()` (30-50 ms of Playwright's injected script in the window).
2. **Fonts before the first layout** (step 4): about 25 ms cold, and the first cold frame paints in Geist instead of the fallback font. Visible (no font swap any more) and holds the mount up to 50 ms.
3. **Sidebar playlist rows and overscan rows in smaller pieces after the first rows.** The 96-124 ms reveal task is what ends "interactive" now; splitting it means the off-screen part of the playlist list (and the off-screen table rows) appear a few frames later. R163 found a frame-by-frame reveal slow in WebKit, so it needs a WebKit check.

## What needs the Mac

- The real launch: R133 measured Rust `setup_done` at 290-405 ms and the page part at about 190 ms; which part of the setup hook takes the time is not known (`PlaybackEngine::new` no longer opens the device, it is lazy now). A launch with `FF_PERF_LOG` and a mark per setup step would show it.
- Audio device opens at mount: `App.svelte` calls `syncPlaybackAudioConfig()` in `onMount`, which sends `playback_set_output_device` and then `playback_configure`. Both are sync Tauri commands, so they run on the main thread, and each one drops and reopens the CoreAudio output stream (`open_output`). That is two device opens on the main thread while the page waits for its first IPC answers. Lead: one open instead of two, and the commands as `#[tauri::command(async)]`, if cpal allows the open off the main thread on macOS. Not measurable here (no audio device on the box, so the open fails fast).
- Whether steps 1-2 show on the Mac: the early answer reaches the page during or right after the mount there too (R154), so they should, but it is not measured.

## Validation

`cargo test --workspace`: all 20 test binaries ok (before the bench placement amend; after it `cargo check -p pioneer-test-ui --features dev-bins --bins` and the `bench_startup` tests, 90 passed). `npx vitest run`: 125 files passed, 11 skipped; 1,091 tests passed, 10 skipped. `npx svelte-check`: 0 errors, 0 warnings. Chromium smoke (`FF_SMOKE_PORT=5491`): 193 passed, 48 skipped. WebKit smoke not run.

Scratch (`~/.cache/perf-startup-work`, `~/.cache/perf-startup-target`) is deleted after this item.
