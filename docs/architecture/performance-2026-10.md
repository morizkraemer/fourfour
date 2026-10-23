# Startup and UI performance (2026-10, Q20)

Status: measured on the `dev` box (Linux, no display). Baseline commit `d188f28`, fixes on branch `q20-speed`.

Headline at 30k tracks: backend `load_state` 24.7 s → 0.5 s, browser first paint 188 s → 3.3 s, rows in the DOM 30,000 → 39.

## Method

Two layers, measured separately because the box has no Tauri window.

**Backend** (`pioneer-test-ui/src/bin/bench_startup.rs`, release build):
- `gen` writes a temp library through `pioneer-library`: N tracks, one analysis each (240 s tracks, 90-169 BPM beat grid, 400-byte PWAV, 36k-sample color waveform), 20 playlists. 1k = 235 MB, 30k = 7.0 GB on disk.
- `run` opens the library and times each `load_state` step: `open`, the track query, `build_track_infos` (includes the track query), playlists, JSON serialisation (`serde_json`, same as the Tauri IPC payload). It uses the real `track_infos.rs` and `dto.rs` from the app via `#[path]`.
- Warm page cache, median of 3 runs. Cold-disk start is not measured (no root to drop caches).

**Frontend** (`app/perf/frontend-perf.mjs`, headless Chromium 1440x900 via Playwright, production `vite build`):
- `?mock=N` makes the browser build serve N synthetic tracks (90% analyzed, with 400-byte previews, 3 default playlists). Without the parameter the 3 original mock tracks are used.
- Metrics: first contentful paint, first `[data-track-row]` in the DOM, "interactive" (end of the last long task after the first row), total blocking time, DOM node count, JS heap (CDP `Performance.getMetrics`), scroll frame times (rAF loop scrolling 90 px per frame for 3 s), search keystroke latency (keydown to second rAF, typing `track 12`), selection click latency (4 clicks), shift-click range latency.
- One run per size. Headless software rendering runs at ~30 fps under load, so scroll p50 of 33 ms is the floor of this setup, not a regression signal.
- Numbers at 30k include ~1.9 s of the page generating the mock data itself (`syntheticMockTrack`, CPU profile); that part does not exist in the app. Floor with 3 tracks: FCP 375 ms.
- `app/perf/profile-action.mjs` takes a CDP CPU profile of load / select / scroll / search.
- `app/perf/walkthrough.mjs` clicks through the main screens and fails on console errors.

## Results: backend `load_state` (release, ms)

| Tracks | Step | Before | After |
|---|---|---|---|
| 1k | track query | 12 | 8 |
| 1k | `build_track_infos` | 841 | 21 |
| 1k | JSON | 8 | 7 |
| 1k | **total** | **851** | **31** |
| 1k | payload | 1.75 MB | 0.86 MB |
| 10k | track query | 120 | 46 |
| 10k | `build_track_infos` | 8,061 | 117 |
| 10k | JSON | 91 | 59 |
| 10k | **total** | **8,262** | **178** |
| 10k | payload | 17.5 MB | 8.5 MB |
| 30k | track query | 354 | 150 |
| 30k | `build_track_infos` | 24,453 | 363 |
| 30k | JSON | 260 | 151 |
| 30k | **total** | **24,714** | **516** |
| 30k | payload | 52.5 MB | 25.6 MB |

`open` is 1-3 ms at every size (schema check only). The first run after a rebuild of the 30k library was 1,074 ms (file cache), the other two 472 and 516 ms.

## Results: browser build (ms unless noted)

| Metric | 1k before | 1k after | 10k before | 10k after | 30k before | 30k after |
|---|---|---|---|---|---|---|
| First contentful paint | 6,685 | 812 | 39,626 | 1,753 | 188,321 | 3,281 |
| First row in DOM | 3,140 | 595 | 19,092 | 1,523 | 119,704 | 2,934 |
| Interactive | 8,645 | 734 | 48,368 | 1,747 | 219,430 | 3,281 |
| Total blocking time | 8,096 | 501 | 47,338 | 1,431 | 214,348 | 2,960 |
| Rows in DOM | 1,000 | 39 | 10,000 | 39 | 30,000 | 39 |
| DOM nodes | 22,261 | 1,120 | 220,261 | 1,120 | 660,261 | 1,120 |
| JS heap (MB) | 51 | 13 | 450 | 60 | 1,314 | 175 |
| Scroll frame p95 | 117 | 50 | 250 | 67 | 800 | 50 |
| Scroll frame max | 150 | 67 | 333 | 83 | 800 | 67 |
| Search keystroke, worst | 4,674 | 140 | 28,736 | 420 | 141,195 | 1,120 |
| Search keystroke, typical (median of 9) | 262 | 58 | 795 | 105 | 2,850 | 140 |
| Select click | 215-463 | 46-78 | 1,162-3,052 | 54-89 | 3,321-9,178 | 86-108 |
| Shift-click range | 298 | 50 | 799 | 55 | 2,144 | 42 |
| Bundle JS (gzip) | 224 KB (72 KB) | 226 KB (73 KB) | | | | |

The worst search keystroke after the fix is always the first one (builds the search text for every track once, 1.1 s at 30k); later keystrokes are 85-225 ms.

## What was slow, what changed

1. **`build_track_infos` read every track's ANLZ files one by one** (`get_analysis` per track: one SQL query, then `read_anlz` parsed the whole DAT plus the 100+ KB EXT color waveform), only to keep the 400-byte preview. 0.8 ms per track, single-threaded.
   Fix: `LocalLibrary::get_waveform_analyses()` — one SQL query for the analyzed ids, then a parallel read (one thread per core) of just the PWAV section of each DAT. The EXT/color waveform is read only when the PWAV preview is flat (the existing fallback). Output is identical; covered by `waveform_analyses_match_per_track_reads`.
2. **The track table rendered every row** (`{#each currentTracks}`), each with a canvas and two `getComputedStyle` calls. 22 DOM nodes and one canvas per track, so first paint, selection and search all scaled linearly with a large constant.
   Fix: windowed rendering in `TrackTable.svelte`: fixed 28 px rows, 12 rows overscan, spacer divs for the rest. Rows carry `data-row-index`; marquee select, drop-line position, drag drop index (`dnd.svelte.ts`) and column auto-fit (now measures text over all tracks, as before) use it instead of DOM position.
3. **Search rebuilt a 16-field search string for every track, three times per keystroke** (header count, list panel, table). Fix: `filterTracks` caches the search text per source array; the array identity changes with every `loadState`/`bumpLibraryTracks`, which is the only time track text changes.
4. **The sidebar's "Recently Added" count filtered all tracks through Svelte proxies** on every library change (~0.8 s at 30k). Now counted once from the plain payload in `loadState`.
5. **Payload size**: the preview went over IPC as a JSON number array (~1.4 KB per track). Now base64 (536 bytes), decoded in `mapTrackInfoToLocal`. Halves the payload (52 MB → 26 MB at 30k) and the JSON time.
6. `select_all_tracks_with_flags` reads columns by index instead of by name (-55% query time).

Not changed: DB indexes. Every lookup in the load path is on a primary key (`tracks.id`, `analyses.track_id`, `artwork.track_id`, `playlist_tracks(playlist_id, track_id)`); there are no missing indexes and no schema change was needed. (`idx_tracks_source_path` duplicates the `UNIQUE` index; harmless, left alone.)

## What is left

- **Scroll jank under fast scrolling**: mounting ~3 rows per frame still costs a canvas draw plus two `getComputedStyle` calls each (`Waveform.svelte` in the design system, outside this task's write scope). Caching the two token colors there would remove the forced style recalcs.
- **`load_state` is a synchronous Tauri command**, so it runs on the main thread. At 30k it blocks for ~0.5 s. Making it `async` with `spawn_blocking` would keep the window responsive; not done (changes the command contract, needs a window to verify).
- **Payload at 30k is still 25.6 MB.** Lazy previews (a `get_waveform_previews(ids)` command for rows in the window) would cut it to ~7 MB at the price of a visible blank mini-waveform on fast scroll; skipped to keep behaviour identical.
- **Frontend load at 30k is ~1.4 s of app work**: normalising 30k previews to 400 floats each (~0.4 s) and mapping tracks through Svelte proxies. Could be made lazy per visible row.
- **First search keystroke at 30k is 1.1 s** (search text built through proxies once per array).
- **Every selection with a not-yet-loaded analysis calls `bumpLibraryTracks()`**, which copies the 30k-element array and re-evaluates every derived list. It is cheap now (≈90 ms at 30k) but is the next O(N) in the click path.
- **Not measured**: a real Tauri window (WebKit/WKWebView, IPC transfer time), debug-profile Rust, cold disk cache, color-waveform scrolling in the player. Browser numbers come from Chromium; macOS WebKit will differ.
- **Not verified visually**: this run has no display. The headless walkthrough (select, shift range, scroll to end, sort, column menu, search, marquee, row drag, organize panel, settings) ran with no console errors at 12 and 2,000 tracks; drop-line position and drag auto-scroll still need a human look in the app.

## Reproduce

Backend:

```bash
cargo build --release --bin bench_startup
for n in 1000 10000 30000; do
  target/release/bench_startup gen ~/.cache/q20-bench/lib$n $n
  target/release/bench_startup run ~/.cache/q20-bench/lib$n
done
```

Frontend (needs Playwright installed somewhere; `PW_DIR` is the folder containing its `node_modules`):

```bash
cd app && npm ci && npm run build
npx vite preview --host 127.0.0.1 --port 4311 &
export PW_DIR=/path/to/playwright-dir
for n in 1000 10000 30000; do node perf/frontend-perf.mjs http://127.0.0.1:4311/ $n; done
node perf/walkthrough.mjs http://127.0.0.1:4311/ 2000
node perf/profile-action.mjs http://127.0.0.1:4311/ 30000 select   # or load | scroll | search
```

To reproduce the "before" numbers, check out `d188f28` and apply the `?mock=N` block from `app/src/services/tauri.svelte.ts` (`syntheticMockTrack`) to it; the old code takes minutes at 10k and about 4 minutes at 30k. For the old backend path, run `bench_startup` from this branch against `d188f28`'s `track_infos`/`get_analysis` loop (the old `build_track_infos` body is in `git show d188f28:pioneer-test-ui/src/main.rs`).


# Q23, 2026-10-23: track load (select and load-in-player)

Branch `q23-load` (from `q20-speed`, master merged). Scope: selecting a track (detail pane) and loading it into the player (colour waveform, beat grid, cues). Headline: backend `get_analysis_data` 137 ms -> 3.2 ms and 3.1 MB -> 177 KB per track; select -> detail ready ~160 ms -> ~45 ms; load -> waveform drawn ~310 ms -> ~85 ms; expanded player open 530 ms -> 120 ms.

## Method

**Backend** (`bench_startup load <dir> [samples]`, release build, Q20 fixtures at `~/.cache/q20-bench`, warm page cache): 60 tracks spread over the library, second pass timed. Each track is a 240 s fixture with a 36k-column colour waveform, 600 beats. Reported: `get_analysis` (ANLZ parse), `build_analysis_json` (the whole command body, from `analysis_payload.rs`), `serde_json::to_string` of the result (the IPC payload), median / p95. Results are the same at 10k and 30k: the cost is per track, not per library size.

**Frontend** (`app/perf/load-perf.mjs`, headless Chromium 1440x900, production build, `?mock=N`): in mock mode the analysis call returns a payload of real size (`app/src/mocks/analysis-perf.ts`: 240 s, 150 columns/s, 2048 + 36,000 + 1200 colour columns, 512 beats, 2 cues) after a `JSON.parse` of the text, as Tauri does with the IPC reply. `?legacy` makes it the old JSON-object shape, used for the "before" runs on the pre-change frontend code. Timers start at the dispatched click (select) or double-click (load in player) and stop at the second `requestAnimationFrame` after the app's `performance.mark` (`ff:detail-ready`, `ff:player-ready`). Five tracks per run, median of the five (first one is cold JIT: 100-185 ms), two runs per size for "after". Headless software rendering paints at ~30 fps, so about 35-65 ms of every figure is the two-frame floor of this setup.

Not measured: a real Tauri window (WebKit, real IPC), Rust debug profile (the app runs its own code unoptimised in dev, so the backend win is larger there), cold disk.

## Results

Backend per track (release):

| Step | Before | After |
|---|---|---|
| ANLZ parse (`get_analysis`) | 0.9 ms | 0.9 ms |
| Build payload | 116 ms | 3.2 ms |
| JSON serialise | 21 ms | 0.6 ms |
| **Total** | **137 ms** | **3.8 ms** |
| IPC payload | 3.1 MB | 177 KB |
| Artwork 200 KB over IPC | 800 KB JSON number array | 200 KB binary `ArrayBuffer` |

Browser (ms, median of 5 tracks; backend time not included, add the rows above):

| Path | 10k before | 10k after | 30k before | 30k after |
|---|---|---|---|---|
| Select -> detail ready (mark) | 113 | 4-6 | 130 | 3-11 |
| Select -> detail painted | 155 | 44-47 | 164 | 44-51 |
| Load in player -> waveform painted | 301 | 89-103 | 325 | 74-80 |
| Open expanded player (36k-column detail) | 518 | 130-150 | 542 | 115-120 |
| First search keystroke (30k) | | | 1,120 | 457 |
| Select click, `frontend-perf` (30k) | | | 86-108 | 28-93 (first, cold: 93; then 28-43) |

Full chain at 30k in release, select -> detail painted: about 137 + 3 + 164 = ~300 ms before, 4 + 3 + 45 = ~50 ms after (IPC transfer of 3.1 MB vs 177 KB not included on either side, so the real gap is larger).

## What was slow, what changed

1. **`get_analysis_data` built the colour waveform as `serde_json::json!` objects** (`{amp, r, g, b}` as f64, 36k of them for the expanded player, plus 2048 and 1200). 116 ms of allocation, 21 ms to serialise, 3.1 MB to ship, then the same again in `JSON.parse` and in the frontend's per-sample validation. The ANLZ parse itself is 0.9 ms. Fix: the three colour arrays now go over as `{len, calibration, bands}` (base64 of the raw low/mid/high bytes plus the PWAV calibration factor). `decodeColorWaveform` in `app/src/services/analysis-data.ts` rebuilds `{amp, r, g, b}` with the same arithmetic as before (`amp = min(max/127 * calibration, 1)`, `r = low * (1/max)`), memoised per payload. Downsampling and calibration are unchanged on the Rust side, so the drawn waveform is the same. Browser mocks that pass plain arrays still work.
2. **Detail pane and player shared one command but not one cost.** The detail pane only needs preview, cues, bpm and key, yet paid for the colour data. With the colour data base64 (~150 KB) and decoded only when the player applies it, the detail path no longer touches it.
3. **`player.track` was a deep `$state` proxy.** The waveform views read 36k samples through Svelte proxies (`has`/`get` traps, 115 ms, plus 143 ms GC when expanding). `player.track` is now `$state.raw` behind the same `player.track` accessor (the track is only ever replaced whole, never mutated). Expanded player 530 -> 120 ms, compact load 300 -> ~90 ms.
4. **Analysis cache was unbounded and had no in-flight dedupe.** Now an LRU of 48 payloads (~10 MB), concurrent requests for the same track share one call, and selecting a row prefetches the next and previous analysed rows (120 ms after the click, one at a time). Selecting a neighbour or double-clicking it is then a cache hit.
5. **Selection copied the 30k-element library array** (`bumpLibraryTracks`) after every first-time analysis load. Now it only happens when the loaded analysis changes the row's bpm, key or analysed flag (never for tracks that arrived analysed in `load_state`).
6. **`load_state`, `get_analysis_data`, `get_track_artwork` are `async` commands** that run on `spawn_blocking`. The window thread no longer blocks for the ~0.5 s of `load_state` at 30k, nor holds on the library mutex during ANLZ file reads.
7. **Artwork** is returned as a binary IPC response (`tauri::ipc::Response`, an `ArrayBuffer`) instead of a JSON number array (4x the bytes, plus a parse and a `Uint8Array` copy), and the object URL is built from it without another copy. Frontend side of this is not measurable in the browser (no Tauri IPC); the Rust side is in the table.
8. **First search keystroke (Q20 leftover):** the search text was built by reading 16 fields per track through reactive proxies. It is now built from the plain track objects (`plainTracks` in `library.svelte.ts`; the proxies wrap the same objects, so they see every mutation). 1,120 -> 457 ms at 30k; later keystrokes unchanged (80-190 ms).

Dev tooling added: `bench_startup load`, `app/perf/load-perf.mjs`, `app/perf/profile-action.mjs player|expand`, `performance.mark` calls `ff:detail-ready` and `ff:player-ready` in the stores.

## What is left

- **Artwork thumbnails at display size** were not done: they need an image-decoding dependency in `pioneer-test-ui` (the `image` crate is only a dependency of the format library), which the unattended rules forbid without a BLOCKED report. The binary transfer is in; a downscaled JPEG (or a one-time cache of 240 px thumbnails in the library) would cut the browser's decode of large covers.
- **Waveform drawing** (offscreen canvas / `ImageBitmap` once per track) lives in the design-system `WaveformView`/`Waveform.svelte`, outside this task's scope. Of the remaining ~85 ms to "waveform painted", about 45 ms is the headless two-frame floor; the rest is `_renderLexiconWaveform`, `getBoundingClientRect` and the overview cache in the design system.
- **Beat grid and cues** still go as JSON objects (600 beats, ~25 KB); measurable only at the microsecond level, left alone.
- **Cold first load** of a track is still the ANLZ file read from disk (not measured; warm cache here). A `prefetch` command that reads ANLZ for neighbours on the Rust side would help on slow disks.
- **Real window:** IPC transfer time, WebKit decode and draw times, and debug-build Rust timings were not measured on this box.
- **Not verified visually:** no display here. The headless walkthrough (select, range, scroll, sort, column menu, search, marquee, row drag, organize, settings) ran with no console errors; the colour waveform pixel output was not compared. The decode uses the same formulas as the old Rust code, and `color_payload` has unit tests, but a human should look at the compact and expanded waveform once in the app.

## Reproduce

```bash
cargo build --release --bin bench_startup
for n in 10000 30000; do target/release/bench_startup load ~/.cache/q20-bench/lib$n 60; done

cd app && npm ci && npm run build && npx vite preview --host 127.0.0.1 --port 4312 &
export PW_DIR=/path/to/playwright-dir
for n in 10000 30000; do node perf/load-perf.mjs http://127.0.0.1:4312/ $n; done
node perf/load-perf.mjs http://127.0.0.1:4312/ 30000 legacy   # old payload shape (only meaningful on the pre-change frontend)
node perf/profile-action.mjs http://127.0.0.1:4312/ 30000 expand
```

The backend "before" numbers came from the original command body (`serde_json::json!` objects per colour column) moved unchanged into `analysis_payload.rs` with the `load` bench added; that state was never committed separately. To see it again, replace `color_payload` with a `Vec<serde_json::Value>` of `{amp, r, g, b}` per column.
