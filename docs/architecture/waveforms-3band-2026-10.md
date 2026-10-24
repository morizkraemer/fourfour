# 3-band waveforms (2026-10)

Two halves, plus the integration: the app (Q35, `q35-app-waveforms`), the USB export (Q34, `q34-export-waveforms`) and how they meet (Q36, `q36-waveforms`).

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


## Export (Q34)

Status: written and tested in software on the `dev` box. Not tried on any player. The open hardware checks are listed at the end and are the brother's to run.

Every colour waveform in the export now comes from one source: 3-band bytes (low, mid, high; 0..127; 150 columns per second). Nothing is faked from the mono preview any more. An analysis without band data gets neutral colour sections and no `.2EX` file.

### Files and sections

One folder per track, `PIONEER/USBANLZ/P{xxx}/{hash}/`, three files:

| File | Sections in order | Notes |
|---|---|---|
| `ANLZ0000.DAT` | `PPTH`, `PVBR`, `PQTZ`, `PWAV`, `PWV2`, `PCOB`, `PCOB` | `PWV2` is new; `PWAV` header word 4 is now `0x00010000` as Rekordbox writes it (was 0) |
| `ANLZ0000.EXT` | `PPTH`, `PWV3`, `PCOB`, `PCOB`, `PCO2`, `PCO2`, `PQT2`, `PWV5`, `PWV4`, `PVB2` | `PWV7` and `PWV6` no longer live here |
| `ANLZ0000.2EX` | `PPTH`, `PWV7`, `PWV6`, `PWVC` | new; only written for tracks with band data; a stale file is deleted on sync when the bands go away |

Section order follows the Rekordbox 7.2 files in Merlin's library (1374 sets) and the notes in `docs/research/clones-2026-10-23.md`. All three files start with the same 28-byte `PMAI` header (`1, 0x10000, 0x10000, 0` after the length).

Layouts (all big-endian; every section starts `tag, header_len, section_len`):

| Section | header_len | Header after `section_len` | Entry |
|---|---|---|---|
| `PWAV` | 20 | `400`, `0x00010000` | 400 bytes: bits 7-5 whiteness, bits 4-0 height (2..25) |
| `PWV2` | 20 | `100`, `0x00010000` | 100 bytes: height 1..15 |
| `PWV3` | 24 | `1`, `entries`, `0x00960000` | 1 byte per column: bits 7-5 whiteness, bits 4-0 height |
| `PWV5` | 24 | `2`, `entries`, `0x00960305` | u16 per column: `rrrgggbbbhhhhh00` (3-bit low, mid, high level; 5-bit height) |
| `PWV4` | 24 | `6`, `1200`, `0` | 6 bytes per column: `b0 b1 b2` colour/height, `b3 b4 b5` low, mid, high (all at most 127 except `b1`) |
| `PWV7` | 24 | `3`, `entries`, `0x00960000` | 3 bytes per column: low, mid, high, at most 127 |
| `PWV6` | 20 | `3`, `1200` | 3 bytes per column: low, mid, high |
| `PWVC` | 14 | (total 20) | `u16 0`, then low, mid, high gain as three u16 |

`entries = round(duration_secs * 150)`; `PWV3`, `PWV5` and `PWV7` always have the same count.

The `PWV5` colour levels, the `PWV3` whiteness, `PWAV`, `PWV2` and `PWV4` are all derived from the same `PWV7` bytes (`Waveforms::from_bands`), so a player drawing any of them shows the same track.

### Band computation

Shared module: `pioneer-usb-writer/src/waveform/mod.rs` (no dependencies beyond serde; the export and the app backend both already link the writer crate).

```text
Meter::new(rate, channels)  .push(&[f32] interleaved)  ...  .finish() -> Measured
analyze(&Measured) -> Waveforms { pwv3, pwv5, pwv7, gains, pwav, pwv2, pwv6, pwv4 }
Waveforms::color_waveform() -> ColorWaveform { detail: pwv7, overview: pwv6 }   // for AnalysisResult.color_waveform
Waveforms::preview()        -> WaveformPreview { data: pwav }                    // for AnalysisResult.waveform
pioneer_usb_writer::writer::anlz::export_waveforms(duration_secs, &AnalysisResult) -> Waveforms   // the exact stick bytes
```

Per column of 1/150 s of the mono mix `(L + R) / 2` (column `i` covers samples `round(i*rate/150)..round((i+1)*rate/150)`):

- Three second-order Butterworth (RBJ, Q = 1/sqrt 2) band filters: low = low-pass, mid = high-pass then low-pass, high = high-pass. Band value = peak absolute output in the column.
- Band envelope = max(column value, previous envelope times a per-band release).
- Per band gain = `clamp(round(target / max envelope), floor, cap)`; byte = `round(band_scale * gain * envelope)`, at most 127. The gains are what `PWVC` carries.
- `PWV6` = mean of `PWV7` over 1200 column groups times a per-band scale; `PWV4` bytes 3..5 are `PWV6` times a scale, bytes 0..2 follow baken's fitted rule.
- Heights and whiteness (`PWV3`) from the column peak and a 150 Hz low-pass peak; `PWAV` and `PWV2` from RMS over column groups.

`Tuning` holds every constant (crossovers, release, gain targets, scales). `Derivation` holds the constants for the sections that are derived from `PWV7` alone, which is the case in the export (see below).

How the export gets its bands: `AnalysisResult.color_waveform.detail` is the `PWV7` byte array. The Python analysis already fills it (calibrated filter bank, 0..127). The Rust analysis fills it with `analyze(..).color_waveform()`. The writer clamps to 127, stretches it to `round(duration * 150)` columns in 64-bit index math, and derives the rest. `color_waveform.overview` is no longer read by the writer; `PWV6` and `PWV4` are derived from `detail`.

### The overflow bug (writer-bugs doc, bug 2)

`(i * src_len / entry_count) as usize` multiplied two `u32` values and wrapped once `entry_count` passed 65,536 (tracks over 7:17). Fixed first, on its own commit, with `u64` math at all four sites. The detail resampling now lives in `waveform::resample_columns` (also `u64`). The q27 repro tests are normal tests now (5, 10, 20 and 60 minute tracks, plus stretched detail). The PDB 192-track bug was not touched and its tests stay `#[ignore]`.

### Fit against Rekordbox

Ground truth: the 1374 `.DAT` / `.EXT` / `.2EX` sets Rekordbox 7.2 wrote for Merlin's library, matched to the audio in `benchmark/datasets/rekordbox-testdata-2025` through `master.db` (read on a copy; the SQLCipher key is the one already in `reader/masterdb.rs`).

Tool: `pioneer-library/examples/waveform_compare.rs` (release build; dev-dependencies `symphonia` and `image`, both already in the workspace through `pioneer-test-ui` and the writer).

```text
cargo run --release -p pioneer-library --example waveform_compare -- <command> --library <dir with master.db copy + share/> --audio <audio root>
  list | score [--set tune|test|all] [--tuning t.json] [--derivation d.json] | fit | fit-derive | scan | curve | lag | gainstat | png <dir>
```

Selection: tracks of 90-420 s that exist in the audio folder and have all three Rekordbox files, sorted by path, spread over mp3 / wav / flac. 40 tracks, alternating: 20 tune, 20 held-out (held-out half: 6 mp3, 7 wav, 7 flac). The tool decodes with symphonia (mono mix, no resampling, no encoder-delay trimming: Rekordbox's column grid lined up at lag 0 for mp3, wav and flac).

What the fit found:

- Baken's crossovers were already near the optimum for low (2nd-order low-pass 300 Hz, peak, release 0.97: mean per-track correlation 0.977 against Rekordbox `PWV7`) and mid (high-pass 250 Hz + low-pass 1000-1500 Hz, release 0.93: 0.961). Steeper filters (two or three cascaded stages) did not help the high band: it tops out near 0.875 whatever the filter, so the remaining high-band error is not the filter shape.
- Rekordbox's byte scale is linear in the band envelope (checked with binned means), `byte = band_scale * gain * envelope`, with `band_scale` about 1.265 (low) and 1.17 (mid). The gain is `max(80, target / max envelope)` for most tracks, but the exact rule is not recovered: 49 % of the 1343 tracks with a `.2EX` have low gain exactly 80 (floor), the others cluster at a track maximum of about 126, and our own maximum envelope disagrees with Rekordbox's by 5-15 %, which shows up as a per-track scale error (the gain columns in `score`). That is the main reason `PWV7` is not byte-exact.
- Fitted (coordinate descent on the tune half, loss = `PWV7` MAE of the three bands + `PWV6` MAE + `PWV5` height MAE): mid high-cut 1200 Hz, high-pass 4746 Hz with release 0.765, gain targets 72.8 / 104 / 78.2, high band scale 0.853, `PWV6` high scale 2.531. Everything is in `Tuning::default()`.
- `Derivation` (sections that follow from `PWV7` only) was fitted on Rekordbox's own `PWV7` as input: loudness weights 3.254 / 0.505 / 0.964 with power 1.10, bass share gain 1.10, level weights 1.69 / 0.35 / 2.96. The weights undo the per-band gain differences.
- `PWAV` heights in Rekordbox start at 2 and top out at 25 (`2 + round(23 * rms / max)`); `PWV2` runs 1..15. Both are baked into the preview step (this also matches the cyanbox minimum-height note).
- `PWV6` and `PWV4` are not simple functions of Rekordbox's own `PWV7` (best aggregate, the column-group mean, correlates 0.90 / 0.83 / 0.77 with its `PWV6`; `PWV4` bands 0.76 / 0.52 / 0.60 with its `PWV6`). Rekordbox seems to compute the overview from a separate pass, so these two stay approximations.

Held-out half (20 tracks, about 970,000 detail columns, 24,000 overview columns). "Direct" is `analyze` (every section from the audio); "export" is what the writer produces from the `PWV7` bytes alone, which is all an `AnalysisResult` carries:

| Section | exact % | within 2 % | MAE | correlation |
|---|---|---|---|---|
| `PWV7` low (direct = export) | 12.0 | 43.8 | 5.08 | 0.960 |
| `PWV7` mid | 26.5 | 69.8 | 2.40 | 0.977 |
| `PWV7` high | 13.8 | 54.3 | 4.51 | 0.888 |
| `PWV6` low / mid / high | 9.4 / 7.3 / 8.2 | 38 / 32 / 34 | 4.41 / 5.10 / 5.86 | 0.864 / 0.735 / 0.788 |
| `PWV5` height, direct | 78.9 | 91.8 | 0.65 | 0.966 |
| `PWV5` height, export | 18.6 | 52.1 | 3.34 | 0.806 |
| `PWV5` low / mid / high level, export | 67.5 / 15.4 / 25.2 | 77 / 92 / 85 | 1.25 / 1.39 / 1.43 | 0.42 / 0.62 / 0.74 |
| `PWV4` band bytes b3 / b4 / b5 | 3.1 / 3.8 / 6.0 | 10.7 / 15.4 / 24.2 | 15.2 / 11.2 / 9.5 | 0.88 / 0.61 / 0.64 |
| `PWV3` height, direct | 99.7 | 100 | 0.00 | 1.000 |
| `PWV3` height, export | 18.6 | 51.9 | 3.35 | 0.804 |
| `PWV3` whiteness, direct / export | 99.8 / 25.8 | 100 / 90.1 | 0.00 / 1.20 | 0.999 / 0.708 |
| `PWAV` height, direct / export | 17.5 / 16.8 | 57.7 / 55.6 | 3.10 / 3.39 | 0.879 / 0.865 |
| `PWV2`, direct / export | 44.1 / 46.0 | 80.5 / 82.0 | 1.39 / 1.30 | 0.790 / 0.828 |

Tune half for `PWV7` (direct): low 13.5 % exact, MAE 5.01, r 0.959; mid 22.3 %, 3.32, 0.958; high 14.9 %, 4.80, 0.862. Baken's own constants on the held-out half: low 8.9 % exact, MAE 5.43; mid 17.3 %, 2.87; high 3.7 %, 6.84, so the fit mostly helped the high band.

Reading the numbers:

- "Exact" is harsh for 0..127 amplitude bytes; correlation and the images are the better guide. The images (ours on top, Rekordbox below, low blue, mid amber, high white) are in `/tmp/claude-1000/-home-merlin-dev-projects-fourfour/5e0c999f-66b8-4690-ac43-edb78b690bb0/scratchpad/waveforms/export/` (not committed), four held-out tracks, each as a full-track view and a 1200-column zoom. At full resolution the match is close; the visible differences are slightly stronger high band in places and some bass notes a little lower or higher.
- The export path loses the per-column peak, so `PWV3` / `PWV5` heights are an estimate from the bands (r 0.81) instead of the near-exact mono peak (99.7 % exact on `PWV3`). Getting those back needs the peak and whiteness columns (and the real gains for `PWVC`) carried next to the bands, which means one extra field on `ColorWaveform` or `AnalysisResult`: about 16 mechanical literal edits, 4 of them in `pioneer-test-ui` bins and `main.rs`, which were outside this task's write list. Recommended follow-up if the exact `PWV3`/`PWV5` matter on hardware.
- `PWVC` gains are fixed at the population medians (85 / 134 / 135) because they cannot be recovered from the band bytes.
- `PWV6`: 1306 of Rekordbox's 1374 tracks (95 %) have a byte above 45, the largest is 115. The Booth note says 45 was enough for a CDJ-3000; Rekordbox itself writes more. The export follows Rekordbox (cap 127, `waveform::PWV6_MAX`) and the round-trip tests assert that constant instead of 45. Lowering it to 45 is a one-line change if the hardware disagrees.


### Open hardware checks (brother, CDJ-3000 / 3000X / older players)

Nothing here was tried on a player.

1. CDJ-3000 / 3000X: do tracks exported with `.2EX` show the 3-band waveform in the scrolling view and the overview? The research doc says wrong sections (`PWV7` / `PWV6` / `PWVC` in `.EXT`) hid the colour waveform on a 3000X, which is why they moved.
2. CDJ-3000 / 3000X: any difference between a track exported without band data (no `.2EX`, neutral `PWV5`) and one with? Does the player fall back to `PWV5` colour?
3. `PWV6` above 45: compare a track whose overview exceeds 45 with the same track capped (`PWV6_MAX = 45`). Rekordbox writes values up to 115 on 95 % of tracks.
4. `PWVC`: the gains are fixed (85 / 134 / 135). Check whether the player rescales the bands with them; if it does, the real gains need to travel with the bands.
5. Older players (CDJ-2000NXS2, XDJ-1000MK2, CDJ-900NXS): the old `PWV5` writer put `[amplitude, 0x80]` in the two bytes; the real layout is `rrrgggbbbhhhhh00`. Check the colour waveform and the 400-pixel overview (`PWAV` now has height at least 2, `PWV2` is new). `PWV4` (6-byte overview) is the weakest fit: check it looks sane.
6. Rekordbox 7.2 also writes an `ANLZ0000.3EX` (about 3.5 KB, header only in the samples). Not written here; check whether a 3000X wants it.
7. `PVB2` is still appended to `.EXT` (zero-filled, untouched); the Rekordbox 7.2 `.EXT` files in the sample end at `PWV4`.
8. A 60-minute track (540,000 detail columns, 2.7 MB `.2EX`): scrolling and loading time.
9. Silent track: `PWAV` and `PWV2` come out all zero (as Rekordbox writes them). Nexus players reportedly reject zero columns (cyanbox note).

### Licences

- `waveform/mod.rs` is derived from baken `crates/baken-export/src/anlz/generate/waveform.rs` (MIT, Copyright (c) 2025 Masanari Higashi). The licence header is in the file; the full text is in `NOTICE` at the repository root.
- Format facts (section order, `.2EX` contents, value ranges) come from the Rekordbox files in Merlin's library, Booth and cyanbox notes; no Booth, cyanbox or rbxport code was copied.
- The comparison tool reads Merlin's Rekordbox library read-only (it works on a copy of `master.db`); no library data is committed.
