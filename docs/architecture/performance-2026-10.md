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


# Q28, 2026-10-23: fast scrolling and waveform drawing

Branch `q28-waveform-perf` (from `q25-multi-usb`, master merged). Everything below is headless Chromium on the shared `dev` box, `?mock=30000`, software compositing, no display.

## Method

`app/perf/waveform-perf.mjs` (new) drives: fast scroll (+90 px per frame for 3 s), a fixed-length scroll (120 frames of +90 px) and a fixed fling (20 frames jumping to the end), then the compact player playing (mock transport, 20 Hz ticks), the expanded player playing, expanded zoom in/out and wheel scroll. It records rAF frame deltas (p50/p95/p99), long tasks, `Performance.getMetrics` script/layout/style time, and main-thread CPU time of the renderer from `/proc/<pid>/task/<pid>/schedstat`. `trace-scroll.mjs` summarises a Chromium trace (Paint, Layerize, Layout ...); `profile-action.mjs` gained `fling`, `playing` and `playingExpanded`.

**Caveat on the numbers.** The box ran at load average 20 to 33 on 16 cores for the whole session, so wall-clock frame times swing by a factor of two between runs of the same build. The table uses the median of 5 alternating before/after runs (before = the merge commit `3eb99a2` built in a scratch directory, after = this branch). Main-thread CPU time and layout time are the trustworthy columns; p95/p99 frame times are shown but read them as "no worse", not as a measured win. Quieter single runs earlier in the session showed 16.7 / 33.4 / 50 ms (p50/p95/p99) after against 33.3 / 50 / 83 ms before for the same 3 s scroll.

## Results (median of 5)

| scenario | metric | before | after |
|---|---|---|---|
| fixed scroll, 120 frames of +90 px | main-thread CPU per frame | 33.1 ms | 26.2 ms (-21%) |
| same | frame p50 / p95 / p99 (wall, noisy) | 49.9 / 66.7 / 83.4 ms | 33.4 / 66.7 / 83.3 ms |
| 3 s fast scroll | frame p95 / p99 (wall, noisy) | 66.6 / 83.4 ms | 66.8 / 83.4 ms |
| 3 s fast scroll | `getComputedStyle` calls | 512 | 0 |
| fling to the end (20 frames, whole window replaced each frame) | main-thread CPU per frame | 165 ms | 174 ms (same, within noise) |
| compact player playing | layout time | 16.7 ms/s | 0.7 ms/s |
| compact player playing | script time / main-thread CPU | 33.7 ms/s / 34% of a core | 32.1 ms/s / 37% (same, within noise) |
| expanded player playing | layout time | 18.4 ms/s | 0.7 ms/s |
| expanded player playing | script time | 163 ms/s | 154 ms/s |
| expanded zoom / wheel scroll | layout time | 13.8 / 10.9 ms/s | 0.6 / 0.5 ms/s |
| expanded player playing | `getBoundingClientRect` calls per second | ~120 | 0 |

Per playhead tick (20 Hz): layout went from about 0.8 ms to about 0.03 ms in both players. Total main-thread CPU per tick did not measurably change (the compact player spends ~17 ms per second on script and the rest in Paint/Layerize at this page size, see What is left). The compact player's per-tick script cost was already small: the playhead was already a DOM overlay (`WaveformDisplay._updatePlayheadOverlay`) and the full-track waveform is drawn once, so there was no per-tick canvas redraw to remove there.

Trace of the 2 s scroll (main thread): `Paint` averaged 24.3 ms per frame (22 frames) before the row containment change and 3.2 ms per frame (68 frames) after. In the compact-playing trace `Layout` ran 183 times in 3 s before and 3 times after.

## What changed

1. **`Waveform.svelte` (design system):** the two colour tokens are resolved once and cached; the cache is cleared by a `MutationObserver` on the root element (`class`, `style`, `data-theme`) and a `prefers-color-scheme` listener. Colours that do not resolve (stylesheet not loaded yet) are never cached. Row previews are drawn straight onto the row canvas the first time; the second time the same key (peaks identity + length, variant, size, devicePixelRatio, token version, played bars) is drawn, the bars are rendered into a shared bitmap cache (800 entries, LRU) and every later mount just blits it, synchronously, so a revisited row has no blank frame. First draws are not cached on purpose: during a one-way scroll every row is new, and building a cache copy per row measured as pure overhead (an idle-time copy cost 175 ms over a fling). Queued draws from all rows flush in one `requestAnimationFrame`; the canvas backing store is only resized when its size changes; the animated placeholder rows share the same flush. Draw-only-when-visible comes from the table's row windowing (only ~56 rows exist); no `IntersectionObserver` was added.
2. **`waveform-display.js`:** canvas sizes come from a `ResizeObserver` instead of `getBoundingClientRect` on every frame (that forced a layout each frame because the overview playhead's `left` dirtied layout); both playhead overlays move with `transform: translateX()` (rounded to 1/64 px to match layout units) instead of `left: %`; bar-label text widths are cached instead of `measureText` per label per frame. The zoom canvas draw itself is unchanged: in the expanded view the waveform scrolls under a fixed playhead, so it must be redrawn every frame, and caching a strip and blitting it at a sub-pixel offset would resample (blur) the polygon edges.
3. **`TrackTable.svelte`:** `contain: layout paint style` on each row wrapper. This is the biggest scroll win: it stops every new row from invalidating paint of its neighbours.
4. **Artwork thumbnails (Rust):** `get_track_artwork` takes an optional `maxPx`; the app asks for 512 px (the 250 px detail-pane cover at 2x). The backend decodes with the `image` crate, downsizes with `thumbnail()` to a 512 px JPEG (quality 85) and keeps up to 400 thumbnails in memory keyed by track id, size and a hash of the stored bytes (so replaced artwork is not served stale); images already 512 px or smaller and undecodable data pass through unchanged. `image = "0.25.10"` was added to `pioneer-test-ui`: it is the version already locked for `pioneer-usb-writer` (MIT OR Apache-2.0, same default features), so `Cargo.lock` only gained a dependency edge, not a package. Two unit tests cover resize/format and pass-through. Browser effect not measured: the mock library has no artwork, so this is verified by the Rust tests only; thumbnails are not pixel-identical to the browser's own downscale of the full image, by design.

Not done: a pool of recycled row canvases (`getContext` is ~30 ms of a 2 s scroll, not the bottleneck), `ImageBitmap` for the player (the overview already blits an offscreen cache and the compact view is drawn once).

## Pixel identity

`app/perf/shots.mjs` takes 14 screenshots at 1440x900 (dpr 1, and again at dpr 2): track list at top / middle / end / after a fast scroll, compact player paused at four fixed positions, expanded player at fit, zoomed and at three overview seeks. Animations are disabled, the library is the fixed `?mock=30000`, the transport is paused and seeked by click. `app/perf/pixdiff.mjs` compares two folders (max channel difference, differing pixel count, bounding box). Before = the merge commit built separately, after = this branch.

- Before vs before (noise floor): 0 everywhere except the master level meter, bottom right (up to 11/255, 16 pixels), which animates by itself.
- **Every track list screenshot: 0 differing pixels at dpr 1 and dpr 2** (the row mini waveforms, covers and text are identical, including after the bitmap cache has been filled).
- Compact player: waveform and playhead identical (0) at all positions; at dpr 1 only the cue ring shows up to 2/255 (45 pixels) in two shots, plus the level meter noise above.
- Expanded player: waveform, overview, grid and labels identical (0) at the seek, zoom and fit screenshots; the selected row's BPM text in the list above it differs by 1/255 on 54 pixels (compositing of the contained row; dpr 2: 1/255 on 147 pixels).
- **Maximum difference: 4/255 at dpr 1 and 7/255 at dpr 2**, both in a single screenshot (the first expanded frame after expanding), on the 1 px overview playhead line (antialiasing coverage of a line at a fractional x; the same position after any later seek is 0). Threshold used: 8/255, no visible change. A first attempt that promoted the playheads to their own layer (`will-change: transform`) snapped them half a pixel off (up to 128/255) and was dropped.

Not looked at by a person: someone should glance at the compact and expanded waveform and at a few rows with the artwork column enabled once in the app.

## What is left

- **Fling is unchanged**: replacing the whole 56-row window costs ~165 ms of main-thread CPU, ~3 ms per row of Svelte mount (12 cells each) plus style/layout; only fewer cells per row, a leaner `TrackRow`, or recycled row components would move that. `TrackRow.svelte` and `track-row.css` were outside this entry's write scope.
- **Page-level paint/layerize cost at 30k rows**: while playing, most of the compact player's main-thread time is `Paint`/`Layerize`/`Commit` with no script involved (Layerize ~2 to 5 ms per frame even with the row canvases removed). Something about the 840,000 px tall scroller and layer tree is re-layerized every frame; not tracked down. A GPU-backed Chromium or WebKit will behave differently.
- **The expanded view still redraws its zoom canvas every frame** (about 3 to 4 ms of script per frame here, mostly `lineTo` and the envelope arrays); caching it would change pixels. Expanded main-thread CPU per frame was within noise before and after (16 to 18 ms in this environment); do not read the layout-time drop as a CPU win there.
- **Row bitmap cache benefit is untested for scroll-back**: the one-way scroll benchmarks never hit it. It should help sort/filter/scroll-back (remounts of known rows) but there is no number for it.
- **Not measured**: artwork thumbnails in a real window, WebKit canvas/transform behaviour, anything on the `mac` box.

## Reproduce

```bash
cd app && npm ci && npm run build
npx vite preview --host 127.0.0.1 --port 4313 &
export PW_DIR=/path/to/playwright-dir
node perf/waveform-perf.mjs http://127.0.0.1:4313/ 30000 [count]     # `count` adds getComputedStyle/getBoundingClientRect/fillRect counters
node perf/trace-scroll.mjs http://127.0.0.1:4313/ 30000 scroll|playing|expanded
node perf/shots.mjs http://127.0.0.1:4313/ /tmp/after 30000 1         # last arg: deviceScaleFactor
node perf/pixdiff.mjs /tmp/before /tmp/after 8                        # exits 1 above the threshold
```

For "before", extract `3eb99a2` (`git archive 3eb99a2 app prototyping/viewport | tar -x -C /tmp/base`), link `node_modules`, build and preview on another port.


# Q29, 2026-10-23: track row mount cost and page paint at 30k rows

Branch `q29-rows` (from `q28-waveform-perf`, master merged). Headless Chromium on the shared `dev` box, `?mock=30000`, software compositing. Headline: a fling to the end costs 166 ms of main-thread CPU per frame before, 74 ms after (frame p50 200 -> 83 ms wall); the per-frame Layerize cost fell from 5.2 to 3.9 ms; the "paint cost at 30k rows" is not caused by the 30k-tall scroller.

## Method

Same tools as Q28: `waveform-perf.mjs` (new `ONLY=list` env stops after the list scenarios), `trace-scroll.mjs` (new `fling` action; `INJECT_CSS` / `INJECT_JS` env to try a style change on a built page), `walkthrough.mjs`. Before = the Q28 head built in a scratch directory and served on its own port; after = this branch. Medians of 5 alternating before/after runs (load average 20 to 33 the whole time, so CPU time is the figure to read; wall times are shown for context).

## Results (median of 5, alternating)

| scenario | metric | before | after |
|---|---|---|---|
| fling to the end, 20 frames, whole window replaced each frame | main-thread CPU per frame | 166 ms (164 to 168) | 74 ms (72 to 81) |
| same | frame p50 / p95 (wall) | 200 / 283 ms | 83 / 133 ms |
| same | script / layout / style per frame | 99 / 37 / 22 ms | 20 / 28 / 6 ms |
| 120 frames of +90 px scroll | main-thread CPU per frame | 26.2 ms | 19.9 ms |
| compact player playing (3 s trace, 30k rows) | Layerize avg per frame (181 frames) | 5.2 ms | 3.9 ms |
| same | main-thread self time over 3 s | 1756 ms | 1457 ms |
| same | Paint avg (50 paints) | 2.2 ms | 2.4 ms (same, noise) |
| same, 1,000 rows instead of 30,000 | Layerize avg | 4.9 ms | 3.9 ms |
| 2 s scroll trace | Paint avg per frame | 4.2 ms (47 paints) | 3.4 ms (72 paints) |

## What was slow, what changed

1. **Row mount on every scroll step.** Rows were keyed by track id, so each new window mounted ~56 `TrackRow` components (12 cells each) and unmounted the old ones: 99 ms of script per fling frame. `TrackTable.svelte` now keeps a fixed pool of row components (`ceil(viewport / 28) + 26` slots, enough for the visible rows plus 12 rows of overscan each side), keyed by slot. Slot `s` shows row index `first + ((s - first) mod pool)`, so a row keeps its DOM node while it scrolls and only the one slot that moves from the top band to the bottom band changes its props. Rows are `position: absolute` at `top = index * 28` inside a body of fixed height; the two spacer divs are gone. Script per fling frame 99 -> 20 ms; layout/style stay (new text must be shaped); layout is now the largest part (28 ms, all of it text layout of the new cell strings: with `.ff-trow__text` hidden it drops to 1.4 ms).
2. **DOM order is no longer row order.** Anything that read `[data-track-row]` in DOM order broke by design: `updateTrackDropTarget` in `dnd.svelte.ts` now takes the smallest row index whose midpoint is below the pointer (and the largest index + 1 otherwise) instead of the first row in the NodeList. The marquee already read each row's `data-row-index`. Slots without a track (end of a short list) are `display: none` and have no `data-track-row`, so they are invisible to both. Screen-reader order follows slot order, not track order (the list has no row roles, so nothing announces it today).
3. **`TrackRow.svelte`:** the per-column style string is computed once per `columns` change (`cellPlan`), not per cell per row; text cells (everything except tag, fav, wave, cover) no longer clip (`overflow: hidden` moved from the cell to nothing; the inner `.ff-trow__text` span still clips and ellipsises, and the cell is exactly as wide as the span, so nothing changes visually). Selectors that use `.ff-trow__cell` / `.ff-trow__text` (column fit) are unchanged.
4. **Paint / Layerize at 30k rows.** Measured, not what Q28 guessed: the cost does not scale with the row count (Layerize 4.9 ms at 1,000 rows, 5.2 ms at 30,000), so the 840,000 px spacer is not the problem. It is a per-frame cost: while a track plays, the playhead overlay (`.waveform-display` `div`, 60 style mutations per second) changes a paint property every frame and Chromium re-layerizes every paint chunk of the page, about 0.13 ms per visible row. Experiments on the built page (`INJECT_CSS`): hiding all rows 4.6 -> 2.1 ms; removing every `overflow: hidden` inside the rows 4.2 -> 1.75 ms; either the cell clips or the text clips alone 3.5 to 3.7; `contain` on the rows, a sticky header, `will-change` on the body or the scroller, `content-visibility: auto` on the rows, and `will-change: transform` on the playhead: no change. So the lever is the number of clip nodes in visible rows, and the change in (3) removes one of the two per text cell: 5.2 -> 3.9 ms.

## Not done, tested and dropped

- **Deferring cell content of rows in the overscan band** (render the text only for visible rows plus 3, fill the rest 70 ms after the last scroll): measured 61 to 64 ms per fling frame against 67 to 74 ms without it, within noise, for blank overscan rows on a fast scroll in a real (async-scrolling) webview. Dropped; the band is only 12 of 56 rows and the cost is the visible ones.
- **`content-visibility: auto`** on rows: no Layerize change, and it would remove the overscan's protection against blank rows when the webview scrolls on a compositor thread.

## Pixel identity

`shots.mjs` (14 states) before vs after, `pixdiff.mjs` threshold 8/255. dpr 2: max 0 on all 14. dpr 1: all four track-list shots (top, middle, end, after fast scroll) and the 10 player shots are 0 except the master level meter in the bottom right corner (up to 11/255 on 15 to 16 pixels, bbox x 1279..1423, y 794..893), which animates by itself and showed the same 11/255 between two builds of the same code in Q28. Not in the shots: a selected or dragged row (class and CSS unchanged; the walkthrough checks the selection) and the artwork column.

Walkthrough (headless, 2,000 tracks, no console errors): select, shift-range, scroll to end and select the last row, rows match their index and sit at `header + index * 28 - scrollTop` after six scroll jumps (new step), column resize (new step), sort, column menu, search, marquee, row drag onto a playlist, drop line index in a playlist (new step: line at 56 px), organize panel, settings. Both before and after pass every step.

## What is left

- **Fling is now text layout bound** (28 of 74 ms layout, 14 ms paint). Fewer text cells per row or less text would be the only big lever; a column that is always the same text (e.g. a short key) could be a single text node.
- **Layerize while playing** (3.9 ms per frame at ~32 visible rows): the remaining cost is one clip node per text cell plus the page. Removing the clips needs another way to ellipsise (measure and clip only cells that overflow, with canvas `measureText`), which risks edge pixels; or a playhead that does not change paint properties per frame (the Q28 attempt to put it on its own layer shifted it by half a pixel).
- **Not measured:** WebKit/real window, a GPU-backed compositor, scroll-back with a populated bitmap cache, drag with a very long list in a playlist (dnd now scans visible rows only, as before).

## Reproduce

```bash
cd app && npm ci && npm run build   # after
# before: git archive q28-waveform-perf app prototyping/viewport | tar -x -C /tmp/base, link node_modules, build
export PW_DIR=/path/to/playwright-dir
ONLY=list node perf/waveform-perf.mjs http://127.0.0.1:PORT/ 30000     # flingSteps20.totalMs.mainCpu / 20
node perf/trace-scroll.mjs http://127.0.0.1:PORT/ 30000 fling|playing|scroll
INJECT_CSS='.ff-trow__text{overflow:visible!important}' node perf/trace-scroll.mjs http://127.0.0.1:PORT/ 30000 playing
node perf/walkthrough.mjs http://127.0.0.1:PORT/ 2000
node perf/shots.mjs http://127.0.0.1:PORT/ /tmp/after 30000 1 && node perf/pixdiff.mjs /tmp/before /tmp/after 8
```

# Q32, 2026-10-23: first search keystroke and row-window module

## First search keystroke at 30k

Tool: `app/perf/search-perf.mjs <base> <count> <runs> <idleMs>` (fresh headless Chromium per run, 4 s idle after the first row, types `track 12`, wall time = keydown to two animation frames, CPU = CDP `TaskDuration` per keystroke). Medians of 5, `?mock=30000`, load average 25 to 33 the whole time, so the CPU column is the one to trust.

| | before | after |
|---|---|---|
| First keystroke, wall (ms) | 444.9 | 119.7 |
| First keystroke, CPU (ms) | 446.7 | 114.8 |
| Later keystrokes, wall (ms) | 128.3 | 94.0 |
| Later keystrokes, CPU (ms) | 113.9 | 87.3 |

Why the first was slow: the per-track search text (16 fields, lowercased, joined) was built lazily inside the first `filterTracks` call, and then a second 30k pass mapped it onto the `library.tracks` proxy array (also lazy, per source array). Both ran on the first keystroke.

Changes (`library.svelte.ts`, results unchanged; the 20 existing search tests pass):
- The search text is built in idle time after `loadState()` and after `bumpLibraryTracks()`: `requestIdleCallback` chunks of 2,000 tracks, started 250 ms after the change, cancelled by a generation counter when the library changes again. If a keystroke arrives first, `filterTracks` finishes the remaining tracks synchronously, so results never depend on timing.
- When the index is complete, the per-array list for `library.tracks` is built once as well.
- `filterTracks` remembers its last (array, query) result. `GlobalHeader`, `ListPanel` and `TrackTable` each call it with the same input per keystroke, so two of the three calls are now free (later keystrokes 128 -> 94 ms).
- No input debounce was added: after the change nothing on the keystroke path needs it.

Left: every `bumpLibraryTracks()` (a first-time analysis load that changes bpm/key) still rebuilds the whole index in idle time rather than patching one track, because the function does not know which track changed. At 30k that is ~170 ms of idle work spread over chunks.

## Row-window module

The virtual/pooled row maths from `TrackTable.svelte` now lives in `app/src/lib/rowWindow.ts`: `rowWindow` (first row, end row, pool size from scrollTop, viewport, header, row count), `indexForSlot`, `rowTop`, `spacerHeight`, `dropLineTop`, and `scrollTopToReveal` (scroll-to-index; not used by the table yet, which has no keyboard scroll-to-row, but covered by tests so the next feature can use it). Constants `ROW_HEIGHT = 28` and `OVERSCAN_ROWS = 12` moved with it. 20 vitest cases in `rowWindow.test.ts`: top and bottom clamps, list shorter than the viewport, empty list, zero viewport before first measure, viewport resize, jump to the end and back, slot-to-row mapping stays a permutation and moves exactly one slot per row of scroll, reveal up, down and past the end.

## No behaviour or visual change

- `walkthrough.mjs` at 2,000 tracks: 14 steps ok, console errors none.
- `shots.mjs` + `pixdiff.mjs` threshold 8/255, before = `q31-tests` head built in a scratch worktree, after = this branch, 14 states: dpr 2 max 0 on all. dpr 1: max 11/255, only in the four compact-player shots (master meter, bbox x 1279..1424, y 884..893, and a 2/255 wobble at x 106..134). Two builds of the unchanged `q31-tests` code differ the same way (11/255 meter, 2/255), so this is the animated meter, not the change. All track-list shots are 0.


# F1/F6 (2026-10-26): scroll reloads the list, laggy drag

Branch `wa-list-scroll` (from `q41-feedback`). Merlin's report on the Mac with a real library: "scrolling isn't smooth, it reloads the list on scroll every time" (F1) and "dragging is laggy" (F6). Headless Chromium on the shared `dev` box (load average ~24), `?mock=30000`, software compositing; real WebKit is not available here.

## Root causes

**F1.** The row `cover` getter called `artwork.get`, which fired one `get_track_artwork` IPC per track the moment its row entered the pool (visible rows plus 12 overscan rows each side), asking for the 512 px detail-pane thumbnail for an 18 px cell. Nothing was cancelled or limited, so a fast scroll queued hundreds of calls for rows that were already gone. Each call takes the library mutex, reads the stored image, hashes and decodes it on the backend, then ships a ~50 KB JPEG over IPC, which the page turns into a Blob and an object URL (kept for the session) and the webview decodes at 512 px to draw 18 px. That is a burst of main-thread and backend work on every scroll step, and covers pop in late ("the list reloads"). Not the cause, checked: no row remounts (0 elements added or removed during the scroll), no `getBoundingClientRect` / `getComputedStyle`, no store write per scroll (text mutations match the ~3 recycled rows per frame), no `loadState`, the row pool keys are stable, the row preview bands come embedded with `load_state` (no IPC), `bumpLibraryTracks` only runs on analysis events.

**F6.** Every `pointermove` of a drag ran, synchronously: write `pointerX/Y` (ghost `left/top` style, so layout), `elementFromPoint`, then `getBoundingClientRect` on every row in the pool (38 at this window height) to find the drop index. About 39 forced geometry reads per event; WebKit delivers `pointermove` uncoalesced, so on a Mac this runs at the pointer rate.

## Fix

1. `artwork.getRow()` (new): row covers ask for a 64 px thumbnail (`ARTWORK_ROW_PX`, the backend already took `maxPx`), through a newest-first queue: 4 in flight, 64 waiting, older waiters are dropped (and re-requested when their row is shown again). The detail pane and players still use `artwork.get` at 512 px. `TrackRow` reads `track.rowCover ?? track.cover` (the library track got a `rowCover` getter). Row covers are about 1 to 3 KB instead of ~50 KB each, and the object URL cache for 30k tracks stays small.
2. `updateTrackDropTarget` computes the drop index from the body rect and a `data-row-count` attribute (`floor((y - top - 14) / 28) + 1`, clamped to 0..count): one `getBoundingClientRect`, no row scan. Same rule as before (before the first row whose midpoint is below the pointer, else after the last).
3. `TrackTable` stores the latest pointer position per `pointermove` and applies `updatePointer` + `updateTrackDropTarget` once per animation frame; pointerup flushes a pending update first so the drop uses the last position. The ghost is positioned with `transform: translate()` and `will-change: transform` (same pixels, no layout). The scroll pin during an embedded drag only writes `scrollTop` when it moved.

## Results (median of 5 runs, `app/perf/list-scroll-drag.mjs`, 3 s scroll of +90 px per frame, 3 s drag with the mouse; mock backend answers `get_track_artwork` after 12 ms)

| | before | after |
|---|---|---|
| scroll: `get_track_artwork` IPC calls in 3 s | 272 | 12 |
| scroll: blob URLs created | 198 | 10 |
| scroll: main-thread CPU per rAF frame | 26.2 ms | 18.5 ms |
| scroll: elements added or removed (mounts) | 0 | 0 |
| scroll: text / attribute mutations per frame | 29 / 17 | 30 / 16 |
| scroll: canvas draws per frame | 61 | 60 |
| drag: `getBoundingClientRect` calls, 3 s | 4522 (38 per move) | 99 (1 per move) |
| drag: `elementFromPoint` calls | 119 | 99 (one per frame at most) |
| drag: main-thread CPU per rAF frame | 6.0 ms | 3.3 ms |
| drag: layout per frame | 0.12 ms | 0 ms |

Frame counts differ between runs (93 vs 105 scroll frames, 186 vs 190 drag) because the box is loaded; read the call counts and CPU per frame. Store writes are not counted directly (no hook): the DOM mutation counts above show no extra re-render, and the artwork cache writes equal the blob URL count. Covers after a 150-frame scroll: all 26 visible rows have one 1.5 s after the scroll stops in both builds (8 of 26 after 0.5 s now against 15 of 26 before, with the mock's 12 ms latency; a real backend decode is slower, which is the point of not queueing hundreds).

Walkthrough (headless, 2,000 tracks): no errors, including drag onto a playlist and the drop line index. `npm test` (173), `npm run check` (0 errors), `npm run build` green; no Rust change.

## Pixel identity

`shots.mjs` dpr 1, 14 shots: all track list shots 0 differing pixels; the rest 0 except the master level meter (up to 11/255, known). New `drag-shots.mjs` (2,000 rows with artwork, covers and mid-drag with ghost): the ghost and everything else identical; 2 pixels differ by up to 18/255 in one cover (the 18 px cell now shows a 64 px source instead of a 256 px one, so the downscale resamples slightly differently).

## Not measurable here, confirm on the Mac

- Whether scrolling in WKWebView feels smooth with a real artwork library (decode cost of the old 512 px covers, IPC latency). Expected: far fewer and smaller IPC calls, no 512 px decodes per row.
- Drag lag in WKWebView with the ghost on a compositor layer. Check dragging one track and a large selection over the list and over sidebar playlists.
- A cover pops in about 1 frame later than before only when its row enters the window faster than the backend answers; at normal scroll speed it is ready before the row is visible (12 rows of overscan).

Not done: backend still decodes the full stored image per track on first request (cached per size, 400 entries) and hashes it every call; a stored 64 px thumbnail per track would remove that, but touches the library schema.

## Reproduce

```bash
cd app && npm ci && npm run build && npx vite preview --host 127.0.0.1 --port 4412 &
export PW_DIR=/path/to/playwright-dir
node perf/list-scroll-drag.mjs http://127.0.0.1:4412/ 30000 12 5   # tracks, mock artwork latency ms, runs
node perf/drag-shots.mjs http://127.0.0.1:4412/ /tmp/after 1 && node perf/pixdiff.mjs /tmp/before /tmp/after 8
```

# Bulk analysis (2026-10-29, fb-bulk): UI lag while a fresh library is analysed

Report: "the UI lags a lot when analyzing a lot of stuff". Branch `fb-bulk` (from `ov-1028`).

## What the backend sends

`bench_analysis` (59 mp3s, Rust backend, dev box at load ~25): staged run with 15 workers. The fast pass (waveform + key, 1 thread per track) finished all 59 tracks in 27 s, about 2.2 tracks/s, and sends two `analysis-partial` events per track (waveform stored, then waveform + key): about 4–5 events/s here, more on an idle Mac. The grid pass runs at about 4 tracks/min and sends one `analysis-progress` per track. A 10k import spends over an hour in the fast pass, so the event storm is the fast pass.

## Causes (measured)

- Every event fetched the track's analysis with a forced read and swapped the whole track array (`bumpLibraryTracks`). At 10k each event was one long task of 90–140 ms. With a list filter active (the palette leaves its text as the filter) each swap also rebuilt the search text of all 10k tracks (~450 ms of `trackHaystack` per profile).
- The player and the library each read the analysis of the loaded track per event.
- `analyzingTrackIds` was a deep `$state` array of up to 10k ids: each visible row's `includes` read every element through the proxy, and every progress event filtered and re-proxied the array.
- The rows' analysing placeholder redrew its canvas every animation frame (about 100 ms/s of script with no input at all).
- Backend priority: every Rust lane (decode, waveform, key, both rten/ONNX pools, now lane, detail warm-up) already runs inside `run_below_ui`; rten pools are created per call inside those threads, and workers × threads per track ≤ cores − 1, so no oversubscription. The Python fallback did not: its analyzer process and the band pass after it ran at normal priority.

## Fixes

- Events invalidate the cache at once and queue the track; the library applies at most every 250 ms (at once when idle): one read per track, one array swap per batch, the analysing ids filtered once. The read is not forced, so the player's read of the same track is shared; reads invalidated while in flight are no longer cached.
- A batch rebuilds only its tracks' search text.
- `analyzingTrackIds` is raw state behind an accessor (replaced, never mutated).
- The placeholder is drawn once and pulses with a CSS opacity animation (none with `prefers-reduced-motion`). `will-change: opacity` was tried and made headless software compositing much worse, so it is not set.
- The Python analyzer starts at nice 10 (`lower_child`) and its band pass runs in `run_below_ui`.

## Numbers

`app/smoke/perf-bulk-analysis.spec.ts` (production build, smoke Tauri mock): a fresh import (no analysis), one track loaded in the player, "Analyze all unanalyzed" from the palette, then events replayed at 6/s (fast pass) and 0.2/s (grid pass) while the table scrolls 45 px per frame for 6 s, then typing in search and opening the palette during events. Two interleaved before/after runs per size, medians. The dev box was at load 20–46 on 16 cores during these runs: headless software rendering alone took ~900 ms/s of main thread while scrolling, so frame counts and long-task counts are dominated by the box; script time, reads per event and input latency are the readable signals.

| 10k tracks | before | after |
|---|---|---|
| no input, rows analysing: script ms/s | 119 | 13 |
| scroll during fast pass: script ms/s | 220 | 139 |
| fast pass: analysis reads per 35 events | 38 | 22 |
| typing during fast pass: palette open median / worst ms | 422 / 607 | 237 / 440 |
| typing during fast pass: keystroke worst ms | 654 | 603 |
| typing during fast pass: long-task ms in window | 6162 | 4370 |

| 2k tracks | before | after |
|---|---|---|
| no input, rows analysing: script ms/s | 110 | 15 |
| scroll during fast pass: script ms/s | 229 | 142 |
| typing during fast pass: keystroke median / worst ms | 532 / 2176 | 314 / 813 |
| typing during fast pass: palette open median / worst ms | 706 / 1511 | 290 / 691 |
| typing during fast pass: long-task ms in window | 17355 | 8713 |

An earlier pair at 10k with the palette text left as the list filter (load ~25): before, each of 37 fast-pass events was a long task (3275 ms in 6 s, 177 frames lost); after, 14 long tasks (1206 ms), 79 frames lost.

Throughput: no Rust scheduling or thread change (only the Python fallback's priority), so Rust throughput is unchanged by construction. Before: 4.20 tracks/min (59 tracks, load ~25). The after run was stopped at 30 min because the box went to load 35–45; it says nothing either way.

## Not done

- A batch still swaps the whole array (~10 ms at 10k per batch in the profile, plus Svelte's re-derivation of the sorted/filtered list). Per-row versioning would remove that; not needed for the event rate measured here.
- Analysis writes (three ANLZ files and the DB index) happen under the library mutex that `get_analysis_data` also takes, so a UI read can wait for one write. Not measured.
- Check on the Mac: scrolling and typing while a few thousand tracks analyse; the pulsing placeholder look.

## Reproduce

```bash
cd app && npm ci
FF_SMOKE_PORT=5361 FF_PERF_BULK=1 FF_PERF_TRACKS=10000 npx playwright test perf-bulk-analysis
# rates: FF_PERF_FAST_RATE / FF_PERF_GRID_RATE (events/s); profile a window: FF_PERF_PROFILE="scroll during fast pass"
FOURFOUR_MODELS_DIR=~/.cache/fourfour-models ./target/release/bench_analysis --backend rust --list <paths.txt> --limit 60
```

# Player waveform scroll during bulk analysis (2026-10-29, fb-scroll)

Report: "still very laggy (the big player waveform scroll)" while many tracks analyse. Branch `fb-scroll` (from `ov-1028`).

## Method

New scenario in `app/smoke/perf-bulk-analysis.spec.ts` (`FF_PERF_SCENARIO=player`): 10k fresh tracks, one analysed track loaded, the expanded player open with the high-resolution series, the mock transport playing (50 ms ticks, the zoom view scrolls every frame). Windows of 6 s: no analysis; analysing with no events; fast-pass events at 6/s (twice, interleaved with quiet windows); grid-pass events at 0.2/s. Per window: rAF frame deltas (p50/p95/p99, frames lost), the summed cost of all rAF callbacks per frame (the render path, measured by wrapping `requestAnimationFrame` before the app loads), long tasks, main-thread task and script ms/s. `FF_PERF_PROFILE=<window name>` adds a CPU profile (`FF_PERF_TOP` rows).

## Causes (measured)

- **The zoom canvas was repainted in full every frame.** At the default zoom (~5 s visible) the high-resolution series (2400 columns/s) gives ~7.5 columns per device pixel here (~3.75 on a 2x screen), so the column ring that Q28/Q36 added for 1 column per pixel or finer was never used: every frame reduced ~12000 columns and wrote and uploaded a full-canvas `ImageData`. That was 4.5-9 ms of script per frame in the rAF callback (p50; 260-316 ms/s of script with no analysis at all), the steady cost that leaves no room for anything else in the frame. On a Retina Mac the canvas has 4x the pixels.
- **Each library batch is one 10-25 ms task** (here; every 250 ms while events arrive): `bumpLibraryTracks` copies the 10k-element deep `$state` array through the proxy (`get`/`proxy` ~10 ms) and Svelte re-runs the deriveds that read `library.tracks`. This is the hitch every quarter second. The player itself does nothing per event unless the event is for the loaded track (it then reads that track once); it does not refetch, redraw or re-derive beats or bands for other tracks.
- **CPU contention dominates the headless numbers.** With the box at load 15-25 the idle player lost 2-10 frames in 6 s; at load 47-80 (other workers' nice-0 compiles) it lost 30-90 with identical code. Sixteen extra busy processes at nice 10 on top of that load changed nothing measurable (89 vs 95 frames lost, no analysis). A nice-0 run timed out on the box and is not reported. Analysis threads already run below UI priority (fb-wave: QoS utility / nice 10), so no worker throttling was added: there is no evidence here that lower-priority backend load starves the WebView, and it would cost throughput.

## Fix

- `drawBandRange` (`waveform-render.js`) takes the column-ring path whenever no envelope decay applies (`decayPerPixel === 0`), not only at 1 column per pixel or finer. The high-resolution series is drawn without decay, so each device-pixel column depends only on its absolute pixel index at any zoom; a scroll by d pixels now reduces and paints only d columns and blits the ring in two slices. The coarse analysis columns (zoom decay 0.86) still repaint in full. No visual change: `waveform-scroll-identity.test.ts` draws 90 scrolled frames (uneven steps, two zooms at 4-17 columns per pixel) through the ring and through a full repaint into a pixel-exact software canvas and requires 0 differing pixels per frame; it fails if the ring is not used.

## Numbers (10k tracks, production build, headless software rendering)

Interleaved before/after pairs. Render path = summed rAF callback time per frame (p50 / p95 ms).

| window | before (load ~18) | after (load ~25) | before (load ~50-70) | after (load ~50) |
|---|---|---|---|---|
| no analysis: render path | 4.6 / 7.4 | 0.7 / 1.4 | 4.5 / 7.3 | 0.7 / 5.0 |
| no analysis: script ms/s | 316 | 64 | 305 | 81 |
| analysing, no events: script ms/s | 288 / 291 | 51 / 64 | 273 / 284 | 59 / 69 |
| fast pass: render path | 5.1 / 8.6, 4.8 / 7.3 | 1.4 / 3.7, 1.0 / 3.2 | 7.7 / 13.5, 6.0 / 11.6 | 1.6 / 9.2, 1.1 / 6.2 |
| fast pass: script ms/s | 266 / 259 | 74 / 64 | 250 / 247 | 75 / 74 |
| grid pass: script ms/s | 296 | 57 | 283 | 59 |

Frames lost and frame p95/p99 are not readable on this box: main-thread task time was 800-980 ms/s in every window (software paint and compositing of the whole page), and the same build lost 2 to 90 frames in the quiet window depending only on box load. Read the table as: the render path is 4-6x cheaper, script time per second down ~75%, batches unchanged.

## Not done

- **Batch cost** (10-25 ms per 250 ms here): removing it needs the library's track list out of the deep `$state` proxy (raw array plus per-row versions) so a batch does not copy 10k elements and re-run every derived; that is a store refactor, not done in this entry. A Mac measurement with the ring fix should show whether it is still felt.
- **Mac check**: play a track in the expanded player while a few thousand tracks analyse; the waveform should scroll without hitches between batches. Retina should gain more than the 1x numbers above.
- Contention with nice-0 backend load was not isolated cleanly (box too loaded).

## Reproduce

```bash
cd app && npm ci
FF_SMOKE_PORT=5371 FF_PERF_BULK=1 FF_PERF_SCENARIO=player FF_PERF_TRACKS=10000 FF_PERF_OUT=/path/out.json npx playwright test perf-bulk-analysis
# profile a window: FF_PERF_PROFILE="player during fast pass (2)" FF_PERF_TOP=90
# before: set the condition in drawBandRange back to `columnsPerPx <= 1`
npx vitest run src/services/waveform-scroll-identity.test.ts
```
