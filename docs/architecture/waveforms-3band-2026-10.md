# 3-band waveforms, 2026-10

## App

Branch `q35-app-waveforms` (from `q32-search`, master merged). Goal: every waveform in the app is the same 3-band picture (low, mid, high), like Rekordbox's 3-band mode, from one renderer at every size. The export side (real 3-band data, ANLZ PWV7/PWV6) is a separate section written on `q34-export-waveforms`.

### Views, before and after

| View | Before | After |
|---|---|---|
| List row preview (120 x 14) | Mono grey bars (2 px + 1 px gap) from the 400-byte PWAV height; whiteness bits dropped | 3-band, built from PWAV height and whiteness |
| Detail pane strip (300 x 36) | Mono bars, `--ff-text-mid` | 3-band, same data as rows, cue dots kept |
| Compact player strip | 3-band polygons, but drawn bass, mid, high, so high (white) covered everything; hard-coded colours and background; no played dimming | 3-band, low in front; tokens; played part dimmed |
| Expanded overview (48 px) | Different design: bottom-anchored stacked bars (one height split by band weights) | Same symmetric 3-band as everywhere; played part dimmed |
| Expanded zoom | Same polygon engine as the strip, same layer order problem, hard-coded `#0d0d0d`, white grid | Same renderer; played part left of the playhead dimmed; grid and playhead colours from tokens |
| Organize panel | No waveform | Unchanged |

Overview decision: symmetric, not top-only. The old stacked overview lost band independence (one height shared by three weights) and looked like a different product from the strip next to it. A symmetric overview reads the same as the compact strip and the zoom view, and 24 px per side is enough for the three layers.

### Renderer

- `prototyping/viewport/design-system/waveform-bands.js`: DOM-free math. `reduceBands` turns columns into one display height per pixel column (peak of the covered columns when zoomed out, linear interpolation when zoomed in, 0 outside the track, floor inside), `smoothEnvelope` is the peak-hold follower, `halfHeight` applies the per-band gain.
- `prototyping/viewport/design-system/waveform-render.js`: `drawBandRange` (reduce, smooth, paint), `paintBands`, `wavePalette` (tokens, cached, invalidated on theme change), `dimPlayed`, `ink`. Works in device pixels, so one output pixel is one reduced column at any devicePixelRatio. Layer order high, mid, low (low in front); each band has its own envelope, symmetric about the centre line. Display curve: `0.045 + 0.955 * v^0.65` per band, band gains low 0.72, mid 0.9, high 1 so mid and high stay visible behind low.
- Scrolling (zoom view): the sample grid is aligned to whole device pixels and the sub-pixel remainder is a paint offset, so peak buckets do not re-bucket while the view slides. Bucket edges carry an epsilon so float noise cannot flip a column between two buckets (found by a unit test: it caused one-pixel flicker at exact bucket boundaries).
- Painting: tall canvases (zoom view) use one `fillRect` per column and band; a jagged tall polygon costs 5 to 20 times more to rasterise (first version measured 176 ms per frame in the expanded wheel-scroll scenario against 29 ms before). Short canvases (rows, strip, overview, compact; 100 device px or less) fill one stepped path per band, which gives the same whole-pixel edges and records faster. Row canvases draw 2 device px columns (`columnPx`), roughly the old 2 px bar grid.
- Played dimming: zoom and overview canvases paint a translucent ground-colour rectangle over the played part after the bands; the compact strip, which does not repaint per tick, moves a DOM mask with the same transform as its playhead.
- Components: `Waveform.svelte` (rows, detail; keeps the bitmap cache, the rAF batch flush and the placeholder animation), `WaveformView.svelte` + `waveform-display.js` (players), `waveform.js` (vanilla canvas artboards). The theme is observed (`class`, `style`, `data-theme`), repaints and cache keys follow it.

### Tokens

In `tokens.css`, read through `getComputedStyle` by `wavePalette()`:

| Token | Dark | Light |
|---|---|---|
| `--ff-wave-bg` | `#0d0d0d` | `#f1f2f5` |
| `--ff-wave-low` | `#2f7bff` | `#1f5fe6` |
| `--ff-wave-mid` | `#ffa224` | `#d96c00` |
| `--ff-wave-high` | `#eef2f8` | `#7d8a9b` |
| `--ff-wave-dim` | `rgba(13,13,13,.58)` | `rgba(241,242,245,.62)` |
| `--ff-wave-ink` (grid, labels, as an rgb triplet) | `255, 255, 255` | `20, 24, 32` |
| `--ff-wave-playhead` | `#ffffff` | `#111111` |

Each band is at least 3:1 against its background (unit test over `tokens.css`). The app has no light theme yet (`docs/ui/design-system.md` defers it): `:root[data-theme='light']` only carries the waveform tokens. Light screenshots set `data-theme='light'` and inject a throwaway set of base tokens for the rest of the page.

### Data contract

The renderer takes `BandColumns`: `{ low, mid, high }`, three equally long arrays (`Float32Array`, `number[]`, any array-like) of per-column amplitudes, 0 to 1, evenly spaced over the whole track. Each band is its own envelope.

`app/src/services/waveform-bands.ts` is the only adapter:

| Input | Function |
|---|---|
| Colour waveform from `get_analysis_data` (`{len, calibration, bands}` base64 low/mid/high bytes, or the old `{amp, r, g, b}[]`) | `colorBandsFromPayload` (memoised per payload), then `balanceBands` |
| 400-byte PWAV preview (rows, detail, and the players when there is no colour waveform) | `bandsFromPreviewBytes`, `rowBandsFor` (small LRU) |
| Real 3-band bytes (PWV7, filter bank) | `bandsFromInterleavedBytes(bytes, fullScale, calibration)`: plug the new data in here |

`balanceBands` scales each band so its 98th percentile lands near the top (gain bounded to 0.6 to 2.5). Without it the loudest band of every column, usually the low end, always reaches full height and hides the bands behind it. It is a no-op on data that is already balanced per band. The preview has only a height and a 3-bit whiteness, so rows and the detail strip show an approximation (whiteness moves weight from low through mid to high); they become exact when a small 3-band preview ships in `load_state`.

Tracks in the list carry `preview` (a `Uint8Array(400)`, replaces the `peaks` number arrays: 400 bytes instead of 3.2 KB per track) and a `bands` getter. `player.track` carries `bands`, `bandsDetail`, `bandsOverview`.

Mocks: `app/src/mocks/synthetic-bands.ts` generates a dance track as three band envelopes (kick and sidechained bass, claps, stabs, pads and vocal phrases, hats and risers; intro, build, breakdown, two drops, outro), encodes it in the real wire format, and derives the PWAV preview from it. `?mock=N` rows use 128 distinct previews; `?mock=N` analysis payloads use 4 variants of the same size as before (so the Q23 to Q29 load and scroll numbers stay comparable).

### Tests

- `app/src/services/waveform-math.test.ts` (vitest): display curve, peak and interpolating reduction, no re-bucketing under scroll, envelope, half heights, painter layer order and symmetry (recording context, both painters), token contrast.
- `app/src/services/waveform-bands.test.ts` (vitest): adapter equals the old decode for every column, balance bounds, preview mapping, LRU, synthetic data shape.
- `app/perf/waveform-pixels.mjs` (headless Chromium, `PW_DIR` set like the other perf scripts): renders a known band pattern (low only, mid only, high only, all three stacked) into an `OffscreenCanvas` at dpr 1 and 2, dark and light tokens, and checks heights and colours at sample columns, symmetry, layering and the ground colour. 60 checks, all pass.
- `app/perf/waveform-shots.mjs` and `waveform-compare.mjs`: the screenshots below.
- Walkthrough (`app/perf/walkthrough.mjs`): no console errors.

### Performance

Headless Chromium on the shared `dev` box, `?mock=30000`, software compositing, `app/perf/waveform-perf.mjs`, median of 5 alternating runs of the Q28 head-equivalent build (before) and this branch (after). Main-thread CPU per frame.

| Scenario | Before | After |
|---|---|---|
| Fast scroll (3 s, +90 px/frame) | 16.9 ms | 16.7 ms |
| Scroll 120 steps | 17.0 ms | 17.5 ms |
| Fling to the end (20 frames) | 73.6 ms | 80.6 ms (+9.5%) |
| Scroll then fling | 43.4 ms | 43.8 ms |
| Compact player playing | 5.6 ms | 6.1 ms |
| Expanded player playing | 15.7 ms | 16.3 ms |
| Expanded zoom in/out | 14.3 ms | 15.0 ms |
| Expanded wheel scroll | 31.4 ms | 27.9 ms |

Frame p50 is identical in every scenario. The one measurable cost is the fling: every row is a first draw (no bitmap yet) and a row now reduces and paints three bands instead of 40 bars, about 0.2 ms more per row mount. Left open: a cheaper first draw for rows during a fling (for example the unreduced 60-column path precomputed with the preview). Not measured: a real WebView with GPU canvas.

### Screenshots

`/tmp/claude-1000/-home-merlin-dev-projects-fourfour/5e0c999f-66b8-4690-ac43-edb78b690bb0/scratchpad/waveforms/app/` (`before/`, `after/`, `compare/`): list, detail, compact player, expanded (seek 37%), expanded zoomed, expanded fit; 1440 x 900 at dpr 1 and 2, dark and light; each with a full frame and a crop, plus one before/after PNG per view in `compare/`. Not looked at by Merlin yet: the tuning (curve, band gains, row smoothing) was judged on these screenshots only.
