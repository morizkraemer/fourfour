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
