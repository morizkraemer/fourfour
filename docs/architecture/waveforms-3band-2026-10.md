# 3-band waveforms (2026-10)

Two halves, plus the integration: the app (Q35, `q35-app-waveforms`), the USB export (Q34, `q34-export-waveforms`) and how they meet (Q36, `q36-waveforms`).

## App

Branch `q35-app-waveforms` (from `q32-search`, master merged). Goal: every waveform in the app is the same 3-band picture (low, mid, high), like Rekordbox's 3-band mode, from one renderer at every size. The export side (real 3-band data, ANLZ PWV7/PWV6) is a separate section written on `q34-export-waveforms`.

### Views, before and after

| View | Before | After |
|---|---|---|
| List row preview (120 x 14) | Mono grey bars (2 px + 1 px gap) from the 400-byte PWAV height; whiteness bits dropped | Simple loudness envelope (see Row preview decision) |
| Detail pane strip (300 x 36) | Mono bars, `--ff-text-mid` | Same loudness envelope as rows, cue dots kept |
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

### Row preview decision

Merlin looked at the 3-band rows and the detail strip in the browser and found them busy; three scaling variants of the 3-band picture did not fix it (per-track gain, log curve, Rekordbox-style). He then chose a simple loudness envelope for rows and the detail strip, because rows are for spotting breakdowns and overall energy, not band detail. `waveform-simple.js` draws one smooth mirrored area (RMS of the three bands times a fixed gain, smoothed over about 3 columns twice, curve `level^0.65`, 4% floor) in one muted ink colour, sampled at one column per device pixel. Scaling is absolute, so loud tracks look loud and quiet ones look quiet. The 3-band renderer stays in the players. The mock has four loudness profiles (dance, dynamic, loud, ambient) so the differences show.

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

## Integration (Q36)

Branch `q36-waveforms` (q35 + q34 merged, master merged). One waveform truth from analysis to app to stick. Not tried on a player, not looked at by Merlin.

### Data flow

```text
audio file --symphonia decode--> waveform::Meter/analyze (pioneer-test-ui/src/band_analysis.rs, after the Python analysis)
   --> AnalysisResult.color_waveform { detail = PWV7, overview = PWV6, exact = PWV3 / PWV5 / PWVC values }
   --> library.set_analysis --> anlz/{id}/ANLZ0000.DAT / .EXT / .2EX      (the same writer the USB export uses)
   --> get_analysis_data: PWV7 bytes (base64, flag "pwv7", calibration 1) --> bandsFromStickBytes = byte / 127 --> renderer
   --> load_state: 128-column row preview from PWV6 (base64 low, mid, high) --> rows and detail pane
USB export: library read-back (PWV7 + exact PWV3/PWV5/PWVC) --> export_waveforms --> same bytes on the stick
```

- Python stays the live backend for beats, key, BPM and cues. After it returns, the Rust backend decodes the file again and replaces `color_waveform`, so the colour data no longer depends on Python's filter bank. If the decode fails the analyzer's own colour data is kept and the log says so.
- `get_analysis_data` colour fields (`waveform_color` peak-held to 2048 columns, `waveform_color_detail` the full PWV7, `waveform_overview` PWV6 as stored, `waveform_row_bands`) are built by one function, `waveform_fields`, shared with the fixture tool. A track whose stored files carry the `.2EX` triple (PWV3 + PWV5 + PWVC alongside PWV7) is flagged `pwv7`; the adapter then draws `byte / 127` per band, no PWAV calibration, no balancing. Tracks stored before the 3-band export (no `.2EX`) keep the old calibrated and balanced path. A Python-measured track stored by the q34 writer also counts as `pwv7`: its bytes are what the stick carries, which is the point.
- `ColorWaveform.exact` (`ExactColumns`: PWV3 bytes, PWV5 words, PWVC gains) is written by `Waveforms::color_waveform()`, read back from the library's `.EXT` / `.2EX` by the ANLZ reader, and used by `export_waveforms` instead of the bands-only derivation. `PWVC` now carries the measured gains of our bytes instead of the fixed 85 / 134 / 135.

### Exact PWV3 / PWV5 heights

Held-out half of the q34 fit (20 tracks, about 970,000 columns), export path against Rekordbox, before (bands only) and after (exact columns):

| Section | before: exact % / MAE / r | after: exact % / MAE / r |
|---|---|---|
| `PWV3` height | 18.6 / 3.35 / 0.804 | 99.7 / 0.00 / 1.000 |
| `PWV3` whiteness | 25.8 / 1.20 / 0.708 | 99.8 / 0.00 / 0.999 |
| `PWV5` height | 18.6 / 3.34 / 0.806 | 78.9 / 0.65 / 0.966 |
| `PWV5` low / mid / high level (exact %) | 67.5 / 15.4 / 25.2 | 67.1 / 3.1 / 30.2 |

The export now equals `analyze` direct for `PWV3` (same bytes). `PWV5` heights and `PWV5` levels were always the direct path's values; the levels against Rekordbox did not improve (they were never exact). Reproduce: `waveform_compare score --set test` (set `WAVEFORM_COMPARE_BANDS_ONLY=1` for the old bands-only export). The edits: `ColorWaveform` got one field, 9 literals in 3 crates updated (library, test-ui bins and `main.rs`, writer tests), no PDB or `playback.rs` change.

### Payload sizes (`bench_startup`, release, 30k fixtures of 240 s tracks)

| | before (q35 payload shape, old fixture) | after (new fixture) |
|---|---|---|
| `load_state` JSON, 30k tracks | 25,648,980 bytes | 24,868,980 bytes (-3.0 %) |
| `load_state` JSON, 1k tracks | 858,254 | 832,254 |
| per row | 400-byte PWAV, 536 base64 characters | 384-byte band preview, 512 characters |
| `get_analysis_data` per track | colour fields as before | + 512 characters (`waveform_row_bands`), colour bytes unchanged |

The row preview replaces the PWAV in `load_state` (tracks without band data still send the PWAV). The 400-byte PWAV is still in `get_analysis_data` for the player's fallback.

Backend `load_state` at 30k: `build_track_infos` 330 ms to 760 ms and total 480 ms to 910 ms (median of three warm runs). The extra is one more file per track (the tail of the `.2EX`, 3.6 KB, read in a single call) over 30,000 tracks. Not recovered. A sidecar or DB column for the row preview would remove it but is a library schema change (must ask). `load_state` is an async command on `spawn_blocking`, so the window thread is not blocked.

### Analysis cost

`waveform::Meter` plus decode, release build, `bench_startup bands` on the six side-by-side tracks (278 to 417 s of audio, mp3 / wav / flac): 1.0 to 2.0 s per track, about 4 ms per second of audio, on the shared `dev` box. It runs on `spawn_blocking` beside the Python analyzer in the same task (at most 3 tracks in flight).

### App performance

Headless Chromium on the shared `dev` box, `?mock=30000`, production builds of `q32-search` and this branch, median of 5 alternating runs (`app/perf/waveform-perf.mjs`, main-thread CPU per frame; `frontend-perf.mjs` and `search-perf.mjs` for load):

| Scenario | q32-search | q36 |
|---|---|---|
| Fast scroll | 22.7 ms | 22.9 (+1.0 %) |
| Scroll 120 steps | 22.6 | 23.1 (+2.1 %) |
| Fling to the end | 49.9 | 48.3 (-3.2 %) |
| Scroll then fling | 32.9 | 33.0 (+0.6 %) |
| Compact player playing | 10.7 | 10.7 (+0.5 %) |
| Expanded player playing | 16.0 | 13.6 (-15.0 %) |
| Expanded zoom | 15.2 | 13.9 (-8.3 %) |
| Expanded wheel scroll | 35.9 | 25.3 (-29.5 %) |

Frame p50 is identical everywhere. Q35's +10 % fling cost is gone (-3.2 % against q32): a row now builds three bands from 128 real columns instead of reducing a 400-byte approximation, and the load path no longer builds 400-float preview arrays.

Load (`frontend-perf`, 30k, median of 5): first contentful paint 2,712 ms to 1,100 ms, interactive 2,718 to 1,064 ms, heap 186 MB to 68 MB (the old preview arrays). Search keystrokes unchanged (64 ms to 70 ms median, first keystroke CPU 78 ms to 78 ms). Selecting a row (detail pane) went from 19 ms to 42 ms median, a regression: the first two clicks are cold (82 and 64 ms) and the last two match q32 (20 and 18 ms). Q35's detail strip is already a 3-band canvas; not chased further here.

Pixel checks (`app/perf/waveform-pixels.mjs`): 60 of 60.

### Side by side with Rekordbox

`/tmp/claude-1000/-home-merlin-dev-projects-fourfour/5e0c999f-66b8-4690-ac43-edb78b690bb0/scratchpad/waveforms/final/` (one PNG per track, README.txt there). Six held-out tracks of the q34 fit (2 mp3, 2 wav, 2 flac). Top half: our analysis measured by the shared module, stored in a temporary library and read back through the app's payload code. Bottom half: Rekordbox 7.2's own PWV7 and PWV6 for the same track through the same payload code. Both through the same app renderer: expanded view (overview + zoom), list row and detail strip.

| Track | PWV7 correlation low / mid / high | Verdict |
|---|---|---|
| Vetiver (mp3) | .988 / .975 / .825 | same structure; Rekordbox has a wider orange fringe and more white in the overview |
| Darkside, Vocal Remix (mp3) | .970 / .993 / .941 | nearly identical zoom; overview a little flatter on ours |
| Neptune (wav) | .995 / .982 / .918 | closest match |
| Revision (wav) | .992 / .987 / .924 | same body; Rekordbox's white hi-hat spikes taller |
| Shimmer (flac) | .980 / .960 / .843 | same shape; Rekordbox has a clearly stronger white rim in the overview |
| Be You (flac) | .986 / .986 / .948 | same shape; white transients taller on Rekordbox |

The renderer produces the Rekordbox 3-band look from Rekordbox's own bytes (blue body, orange mid, white high in front). The remaining visible difference is the data: our high band is weaker than Rekordbox's (the weakest correlation everywhere, as the q34 fit found) and the `PWV6` overview differs more than `PWV7` (r .63 to .94). The rows (120 x 14) and strips come from the same bands and read as the same track in both halves.

Tools: `waveform_compare fixtures` (Rekordbox bytes and beat grid for picked tracks), `pioneer-test-ui` bin `waveform_fixtures` (the app payloads for both sides), `app/compare.html` plus `app/src/compare-harness.ts` (dev-only page), `app/perf/waveform-final.mjs` (headless screenshots).

### Tests

- Rust: `get_analysis_data` band bytes equal the PWV7 bytes in the `.2EX` (and PWV6, and the analysis' own bytes), `analysis_payload.rs`; exact columns reach `.EXT` / `.2EX` and survive a read back, bands-only derivation differs, PWV6 tail read and scan fallback, `waveform_export.rs`; the existing round-trip tests; `band_analysis` decode test.
- App: `bandsFromStickBytes` / `pwv7` payloads map to `byte / 127` with no calibration or balancing, row preview decode, fallback path unchanged (`waveform-bands.test.ts`).

### Still needs hardware or Merlin's eye

- Everything in "Open hardware checks" above, plus: do players use the real `PWVC` gains (they are now our measured gains, not 85 / 134 / 135)?
- The row preview and the high-band weakness in the screenshots: taste (the band gains in the renderer were tuned on balanced data in Q35 and now run on raw PWV7).
- Row preview cost at 30k (+0.4 s backend) and the schema-change option.
- A real WebView (WKWebView) with GPU canvas: all numbers here are headless Chromium with software compositing.
- Re-analysing an existing library: tracks analysed before Q36 keep their stored colour data until re-analysed (Python filter bank, derived PWV3 / PWV5).

## Polish (Q37)

Branch `q37-waveforms` (from `q36-waveforms`, master merged). Three fixes: the high band, the `load_state` cost, the row-select cost. Not tried on a player, not looked at by Merlin.

**Needs Merlin's sign-off:** the library schema change below (v3 to v4, a new nullable column). It lives only on this branch. The high-band retune changes what users see and what the stick carries (`PWV7` high bytes), so it also needs a look.

### High band

Tool: `waveform_compare hiband` (new): a search over the high-pass chain, feature (peak, rms), release, gain target and curve on the tune half. Loss is `1 - correlation` plus the log ratio of mean, p75, p90 and p99 of the byte distribution against Rekordbox, so it rewards matching spike height, not only the mean error. The old tuning had p99 of 47 against Rekordbox's 84, which is the "spikes too short" in the side-by-sides.

Change (`Tuning::default`): high-pass 4746 Hz to 3000 Hz, release 0.765 to 0.90, gain target 78.2 to 140, band scale 0.853 to 1.0, plus one new constant, `curve` (`byte = 127 * (value / 127)^curve`, high band 2.08, others 1.0, serde default so old tuning files still load). The envelope attack was already instant (`max(column peak, decayed previous)`), so "fast attack" was not the lever; the slower release and the expanding curve are. `PWV6` is built from the values before the curve (otherwise the overview collapses), and its high scale went from 2.531 to 1.2.

Held-out half (20 tracks, 969,980 columns), `waveform_compare score --set test`, before to after:

| `PWV7` high | exact % | within 2 % | MAE | correlation |
|---|---|---|---|---|
| held-out | 13.8 to 41.7 | 54.3 to 73.2 | 4.51 to 2.78 | 0.888 to 0.931 |
| tune (for reference) | 14.9 to 42.7 | | 4.80 to 2.68 | 0.862 to 0.911 |

Low (12.0 / 43.8 / 5.08 / 0.960) and mid (26.5 / 69.8 / 2.40 / 0.977) are byte for byte unchanged. Side effects on the held-out half: `PWV6` high MAE 5.86 to 9.56 and correlation 0.788 to 0.767 (a deliberate trade, so the overview keeps a white rim; scale 0.9 would give MAE 6.33), `PWV4` b5 MAE 9.47 to 10.91, `PWV5` high level correlation 0.852 to 0.873.

### Side by side, v2

`/tmp/claude-1000/-home-merlin-dev-projects-fourfour/5e0c999f-66b8-4690-ac43-edb78b690bb0/scratchpad/waveforms/final-v2/` (same six tracks and layout as `final/`, README.txt there). `PWV7` high correlation before to after and verdict (only 01, 04 and 05 were looked at; the others are from the numbers):

| Track | high corr | Verdict |
|---|---|---|
| Vetiver | .825 to .866 | overview now matches in white and structure; zoom mid fringe still thinner than Rekordbox |
| Darkside | .941 to .992 | best fit of the set (numbers only) |
| Neptune | .918 to .928 | was already closest (numbers only) |
| Revision | .924 to .947 | white hi-hat spikes now as tall as Rekordbox's |
| Shimmer | .843 to .935 | zoom spikes match, the missing white overview rim is back (a bit thinner) |
| Be You | .948 to .971 | numbers only |

### `load_state` at 30k: 850 ms to 350 ms

Cause: the row preview was read from the tail of each track's `.2EX` (one file per track).

Schema change (v3 to v4, `pioneer-library/src/schema.rs`): `ALTER TABLE analyses ADD COLUMN row_preview BLOB`, nullable, additive. A v1 or v2 library now walks the migrations in order. An older build ignores the column (it refuses a version above its own, so a library opened by this branch is not opened by an older build).

- Content: 384 bytes (128 columns of low, mid, high) for tracks with band data, 400 bytes (mono PWAV) for tracks without, an empty blob for "checked, nothing to show".
- Filled by `set_analysis`, from the files just written (so it equals what the old path read).
- Old rows stay `NULL` and are filled once, lazily, by `build_track_infos` (`LocalLibrary::backfill_row_previews`, parallel read, one transaction); later loads cost one query (`get_row_previews`).
- The preview code (`row_bands`, `effective_preview`) moved from `pioneer-test-ui` into `pioneer-library/src/row_preview.rs`; the old paths re-export it. JSON payload is unchanged (24,868,980 bytes at 30k).
- Tests: stored preview equals the file read; a v3 library migrates and backfills (`lib.rs`), blob round trip.

`bench_startup run`, release, 30k fixtures of 240 s tracks (Q20 method, warm runs), `load_state` total = `build_track_infos` + playlists + JSON:

| | before (q36) | after |
|---|---|---|
| `build_track_infos` | 685 ms | 200 to 230 ms |
| `load_state` total | 850 ms | 337 to 367 ms (target 520) |
| first run after migration (backfill, cold) | 3,949 ms (cold files) | 1,310 ms, once |

Run on a copy of the q36 `library.db` with the `anlz` folder linked (so the migration path was measured); a library generated by the new code (1k tracks, `set_analysis` path) loads with no backfill (5.6 ms).

### Row select: 42 ms to 20 ms

`?mock=30000`, `frontend-perf.mjs` (click then two animation frames; 4 clicks, median). Profile (`profile-action.mjs`, plus a per-click profile): a warm click is 19 to 22 ms, which is the two-frame floor; the cold clicks were 82 and 64 ms because the perf mock built its four analysis variants (3-band synthesis, 36,000 columns each, a slow base64 loop) on first use. That is mock work, not app work (the real backend does it in Rust). Fixes:

- Mock: one synthesis, variants by rotation, fast base64, all four built on the first answer. First click cold 90 ms (q32: 62), the rest warm.
- App: `applyWaveformAndCuesFromAnalysis` keeps the track's row preview object when the analysis sends the same bytes, so the strip is not repainted from fresh bands and a missing bitmap. (The mock sends different bands for rows and analysis, so the harness still draws twice; the real app no longer does.)
- Renderer: `drawBandRange` keeps the reduced and smoothed columns per bands object and width (6 entries), so redraws skip the reduction.
- The strip already used the row preview bands (no full data needed).

### Perf against `q32-search`

Headless Chromium on the shared `dev` box, `?mock=30000`, production builds, median of 5 alternating runs (`waveform-perf.mjs`, `frontend-perf.mjs`, `search-perf.mjs`):

| Scenario | q32-search | q37 |
|---|---|---|
| Fast scroll | 22.2 ms | 22.2 (-0.3 %) |
| Scroll 120 steps | 23.2 | 22.6 (-2.6 %) |
| Fling to the end | 48.6 | 50.6 (+4.1 %) |
| Scroll then fling | 32.6 | 31.7 (-2.7 %) |
| Compact player playing | 10.6 | 10.9 (+3.4 %) |
| Expanded player playing | 15.9 | 14.7 (-7.8 %) |
| Expanded zoom | 15.2 | 14.5 (-5.1 %) |
| Expanded wheel scroll | 33.6 | 24.7 (-26.6 %) |
| Row select, median of 4 clicks | 20.3 | 20.7 |
| Shift-range click | 22.9 | 22.9 |
| Search keystroke, median | 69.5 | 65.2 |
| First contentful paint | 2,688 | 1,076 |
| Interactive | 2,645 | 1,037 |
| Heap | 186 MB | 69 MB |

Noise is a few percent between runs (the fling and compact-player rows are inside it). Pixel checks (`waveform-pixels.mjs`): 60 of 60.

### App screenshots

`/tmp/claude-1000/-home-merlin-dev-projects-fourfour/5e0c999f-66b8-4690-ac43-edb78b690bb0/scratchpad/waveforms/app/after-v2/{dark,light}-dpr{1,2}/` (list, detail, compact and expanded player; the app renders them from the mock's synthetic bands, so they show the renderer, not the new tuning). Not looked at.

### Still open

- Schema v4 sign-off (above), and the high band on real tracks in the app.
- `PWV6` high is now a worse numeric fit than before (white rim chosen over MAE); `PWV4` b5 likewise slightly.
- Cold first select click is 90 ms against q32's 62 in the mock (first-use synthesis plus the first 3-band strip paint).

## Q39 fix

Branch `q39-wave-fix` (from `row-wave-variants`). Merlin ran the real app on his tracks: the compact strip was a flat saturated blue block, the expanded player flashed while scrolling, the detail strip was grey. Rows keep the grey loudness envelope (Row preview decision); everything else is the 3-band picture. Not tried in the real WKWebView, not looked at by Merlin.

### Cause of the flat block

Not saturated bytes. Real PWV7 low bytes top out at 93 of 127 (Rekordbox's own: 124); the overview PWV6 bytes top out at 36 to 49. Three things made the strip flat, in order of weight:

1. **The compact strip was fed `waveform_color`, the full detail peak-held to 2048 columns** (`downsample_3band`, max per band over about 24 columns, 160 ms, one kick period), and then peak-pooled again to one value per pixel. A max over a kick period is the kick level everywhere there is a kick, so a drop is a plateau. Byte percentiles (p5 / p50 / p95), full detail against the 2048-column strip data, `byte-histograms.txt` in the q39 folder has all of it:

   | Track | Low, ours | Low, Rekordbox | Mid, ours | High, ours |
   |---|---|---|---|---|
   | Revision | 36 / 61 / 85 to 64 / 83 / 90 | 29 / 59 / 84 to 62 / 81 / 88 | 13 / 27 / 77 to 25 / 73 / 101 | 0 / 3 / 58 to 1 / 36 / 123 |
   | Be You | 3 / 36 / 78 to 4 / 72 / 87 | 3 / 40 / 105 to 5 / 96 / 118 | 5 / 25 / 63 to 15 / 52 / 93 | 0 / 3 / 54 to 3 / 45 / 116 |
   | Oyea | 14 / 41 / 62 to 24 / 53 / 73 | 14 / 51 / 79 to 27 / 68 / 93 | 11 / 27 / 73 to 27 / 51 / 102 | 0 / 2 / 39 to 0 / 20 / 76 |

   The median of every band moves up to near its peak and the spread collapses (Revision low: p5 to p95 spans 49 bytes in the detail, 26 in the strip data; Rekordbox's own bytes do the same). Rekordbox's 2048-column equivalent is just as flat through our renderer, so it is the pooling, not our bytes.
2. **Display curve and stacking.** `0.045 + 0.955 * v^0.65` lifts every quiet value, and low was drawn in front, so the flat low plateau covered mid and high. The Rekordbox references stack the other way (low outer and back, mid in front, high the thin core on top).
3. **The legacy payload path** (tracks stored before Q36, no `pwv7` flag) runs `balanceBands`, which puts every band's 98th percentile at 0.95. On pooled data that is every column near full height (the `before-legacy/` PNGs reproduce the screenshot: solid blue, thin amber fringe, white specks). Left in place for those tracks; the strip no longer pools.
4. The expanded overview showed PWV6 at `byte / 127`, so it was thin on the pwv7 path (bytes of 15 to 49) and full on the legacy path: two different looks.

### Fix

- `waveform-bands.js`: `BAND_ORDER` is low, mid, high (low outer, high on top). Pixel buckets blend peak and mean (`BUCKET_PEAK_WEIGHT` low 0.2, mid 0.3, high 0.4) instead of the pure peak. Curve `0.02 + 0.98 * v^0.85` (a gentle expansion: quiet stays quiet, silence is a thin line). Global per-band gains against one reference for every track and view (`BAND_GAIN` low 1.15, mid 0.75, high 0.5), no per-track normalisation. `BandColumns.scale` (per band) for data whose full scale is below 1.
- The compact strip, the expanded overview and the zoom view all draw the full-resolution `waveform_color_detail` (`player.track.bands` and `bandsOverview` are the detail bands). One data source, one look. The 2048-column field stays in the payload (tests, fallback) but the strip no longer uses it.
- Detail strip: `Waveform.svelte` got `look` (`envelope` default for rows, `bands` for the detail pane). The 128-column row preview carries `scale { low 2.4, mid 2.4, high 1.4 }` (PWV6 bytes top out near 50 of 127); the row envelope ignores it.

### Cause of the flashing

Headless frame capture (70 to 90 consecutive frames at dpr 1 and 2, mock and real bytes, `app/perf/waveform-flicker.mjs`) shows no blank, half-drawn or cleared frame, no canvas resize, no DOM rebuild and one canvas element throughout, so none of clear-then-draw, bitmap eviction, per-frame resize or a double-buffer swap exists in this code. What does exist: **the zoom view scrolls by translating the 1 px column rects by the fractional pixel remainder (Q28)**. Every column is then anti-aliased across two device pixels and neighbouring columns composite at coverage a and 1 - a, so the background bleeds through (opacity 1 - a(1 - a), down to 75 %). The bleed changes with the sub-pixel phase, i.e. every frame while scrolling. Measured on a constant waveform scrolled in 40 sub-pixel steps: brightness swings 22 % (blue channel 197 to 254) at dpr 1 and 2. On real content the body brightness of the zoom view swung 10.6 % across 70 consecutive frames. Beat, cue and playhead lines had the same problem (a 1 px line at a fractional x is crisp or smeared depending on phase).

Fix: the zoom view scrolls in whole device pixels (no fractional offset), and lines land on whole device pixels (`_lineAt`). Constant waveform: 0.0 % swing at every phase; real content over 64 to 70 frames: 1.9 % at dpr 1, 2.2 % at dpr 2 (what the content itself changes), frame-to-frame content residual 0.7 % median. Pixel check `waveform-pixels.mjs` has the constant-waveform check (64 checks, all pass).

Not reproduced: a GPU-canvas or compositor-level flash that only exists in WKWebView. If Merlin still sees a flash on the Mac, the next suspects are the per-frame cost of about 6,000 `fillRect`s at dpr 2 and the opaque bar-label boxes that clip the waveform top and jump sides at the playhead; neither is blank-frame behaviour, so they are not changed here.

### Tests and checks

- `waveform-math.test.ts`: curve, bucket blend, a drop and a breakdown with the same kick peak stay apart (the flat-block regression), one global reference (the same level is the same height whatever else is in the track), per-band scale, painter layer order (low, mid, high; rect and path painters), whole-pixel scrolled columns.
- `waveform-bands.test.ts`: row preview carries the display scale.
- `app/perf/waveform-pixels.mjs`: heights stack high < mid < low, high is the front core, mid ring, low ring, ground outside; constant waveform keeps its brightness at every sub-pixel phase.
- `app/perf/waveform-flicker.mjs` (frame capture): per frame, ink coverage of the centre rows of the zoom and overview canvases, left and right halves, transparent pixels, canvas width / height assignments, DOM mutations under the player, canvas identity, frame-to-frame content residual, body brightness swing. `FIXTURE=<fixture.json>` runs it on real bytes through `compare.html?play=1`; without it, on the `?mock=N` app.

### Screenshots and numbers

`/tmp/claude-1000/-home-merlin-dev-projects-fourfour/5e0c999f-66b8-4690-ac43-edb78b690bb0/scratchpad/waveforms/q39/` (`before/`, `before-legacy/`, `after/`, `README.txt`, `byte-histograms.txt`, `perf.json`): four tracks (Oyea, Revision, Be You, Darkside), compact strip + expanded overview + zoom + row + detail strip, ours and Rekordbox's bytes through the same renderer.

Perf (`waveform-perf.mjs`, headless Chromium, shared `dev` box under load from other jobs, `?mock=3000`, software compositing, median of 5 alternating runs, main-thread CPU per frame; before is the `row-wave-variants` head build):

| Scenario | before | after |
|---|---|---|
| Compact player playing | 6.54 ms | 6.18 ms |
| Expanded player playing | 17.28 | 17.08 |
| Expanded zoom in/out | 16.75 | 16.53 |
| Expanded wheel scroll | 29.36 | 28.49 |

CPU per frame is equal or lower everywhere. Frame p50 is 16.7 ms in all of them. Frames over 33 ms per 4 s window (median of 5): expanded playing 8 to 17, zoom 7 to 9, wheel scroll 20 to 15, compact 0 to 0; the expanded-playing difference sits next to unchanged CPU on a loaded box and was not chased (rerun on an idle machine before reading anything into it).

## Q40 Rekordbox match

Branch `q40-rb-match` (from `q39-wave-fix`). Goal: the 3-band waveforms look like Rekordbox's, judged 1:1 on the same tracks (Dubby, Gate). Two separate steps: first the renderer alone (Rekordbox's own bytes through our renderer against its screenshots), then our bytes against Rekordbox's. Not looked at by Merlin, not tried in the real WKWebView.

PNGs, README and the metric files: `/tmp/claude-1000/-home-merlin-dev-projects-fourfour/5e0c999f-66b8-4690-ac43-edb78b690bb0/scratchpad/waveforms/q40/` (`dubby-zoom.png`, `gate-zoom.png`, `dubby-overview.png`, `gate-overview.png`; each is a labelled stack: Rekordbox screenshot, our renderer on Rekordbox's bytes, our renderer on our bytes; the overviews also have Rekordbox's PWV6 and the app path). The compact strip draws the overview picture through the same code.

### Method

- Rekordbox's bytes for both tracks come from the fresh `master.db` copy (`rekordbox-library-2026-10-25`) through `waveform_compare track` (new: `WAVEFORM_TRACKS="Dubby=<audio>,..."`, writes PWV7, PWV6, PWV4, PWV5, PWV3, PWAV, gains, beats and our measured bytes as JSON). `waveform_compare shape` (new) prints pooled byte distribution, attack (mean rise per column) and decay (mean fall ratio) for ours against Rekordbox over a half of the selection.
- `app/rbmatch.html` + `app/perf/rbmatch.mjs` draw the real `WaveformDisplay` from raw PWV7/PWV6 bytes at an exact pixels-per-second and left edge. Screenshot geometry: Rekordbox's crops are at Dubby 1:26.0 (bar 51.1) and Gate 1:55.8 (bar 69.2); the "2:29 to 2:33" in the brief is where our own screenshot was. Pixel scale from the red downbeat markers: 241.65 px/s (Dubby, 140 BPM) and 246.4 px/s (Gate, 142 BPM), 1931 px window, centre line at row 78.
- Pixel comparison: every column is classified by colour (blue, bright amber, dark amber, white, cream, grey-white) and the extents of the outermost colour, of amber-or-white and of white are compared (MAE in px of the 77 px half height, correlation).
- Rekordbox's waveform in the screenshots sits 8 ms (Dubby) and 2 ms (Gate) later than its PWV7 bytes at the alignment the grid markers give (minimum extent error); applied to the rows that are compared with the screenshot.

### What made the renderer match (top 3)

1. **Hard attacks, linear heights.** Each source column is held over its pixels (no interpolation) and nothing is smoothed along the pixel axis at one pixel per column or finer (the bytes already carry Rekordbox's release; the old follower added a second tail). Heights are linear in the byte (curve exponent 1.0 instead of 0.85), with band gains fitted against Rekordbox's column extents: low 0.80, mid 0.82, high 0.89 (was 1.15, 0.75, 0.5 with the blended-bucket curve). Dubby column extents, outer MAE 17.3 px to 2.2 px, blue 23.7 to 1.75.
2. **Rekordbox's layer tones.** Rekordbox draws low (blue, back), mid (amber), high (white), every band from the centre line with its own height, and tones the front bands by what is behind them: amber inside the blue is dark amber `#b2660a`, bright amber `#fda602` only where it sticks out; white is `#fdfdfd` outside, `#ece8e5` inside blue, cream `#f6e4d2` inside amber. Colours now `#0053e2` / `#fda602` / `#fdfdfd` (sampled from the screenshots), three new tokens (`--ff-wave-mid-in-low`, `-high-in-low`, `-high-in-mid`). Layers that a taller later layer covers are not painted.
3. **Stacked, bottom-anchored overview.** Rekordbox's overview is a stack from a baseline: blue body, amber on top of it, white on top of that, from its 1200-column PWV6. The overview and the compact strip now reduce the detail columns to at most 1200 mean buckets (`OVERVIEW_COLUMNS`), scale them to PWV6 level (`OVERVIEW_SCALE` 0.6 / 0.75 / 2.0, from the PWV6 to mean-of-PWV7 ratios of Rekordbox's own files) and stack with `STACK_GAIN` 1.08 / 0.7 / 1.35 (high exponent 1.7, so hat accents stand out of the body). Stack-top MAE against the screenshots: Dubby 2.6 px, Gate 3.9 px from Rekordbox's PWV7 through the app path (2.5 / 3.5 from its PWV6).

### Renderer results

| View, bytes | Dubby | Gate |
|---|---|---|
| Zoom, Rekordbox bytes: outer / blue / amber / white MAE (px) | 2.2 / 1.75 / 2.8 / 2.1 | 2.7 / 0.9 / 3.1 / 2.2 |
| Zoom, our bytes (same audio) | 4.4 / 8.1 / 6.6 / 6.6 | 5.1 / 2.9 / 4.5 / 3.5 |
| Overview, Rekordbox PWV6: stack / blue MAE | 2.5 / 0.6 | 3.5 / 1.4 |
| Overview, Rekordbox PWV7 via the app path | 2.6 / 0.55 | 3.9 / 2.2 |
| Overview, our bytes | 2.8 / 0.8 | 6.6 / 2.5 |

Dubby, our bytes in the zoom row: the extents are worse only because our columns sit 4 columns (27 ms) earlier than Rekordbox's on this mp3 (lag scan: best lag +4 for all three bands; Gate lag 0). Cause not verified; our timeline is the playback timeline, so it is not "fixed" in the bytes.

Honest verdict: zoom matches closely on both tracks (kick and hat spikes with decay tails, white core flash, amber attacks, blue body). Rekordbox's screenshots are slightly blurred; ours are crisper and our cream core is about 1 px wider. The overview reproduces structure and level (breakdown dips, amber fringe, white rim); Rekordbox's tall white hat spikes show only from its own PWV6 bytes: they are not in the mean of its PWV7 (a higher peak weight on the high band raised the level too much, mean 12 % peak made Dubby's stack 2.5 px too tall).

### Our bytes against Rekordbox's

`Tuning::default` after the pooled shape comparison (`waveform_compare shape`, 20 tune and 20 held-out tracks): the high band already matched (p99 82 / 91 against 84 / 83, attack and decay within 3 %); low and mid released slightly too slowly and low peaked slightly low. Changes: low release 0.97 to 0.965, mid release 0.93 to 0.925, low gain target 72.8 to 77.0. A second candidate (low release 0.96, target 82) matched the level distribution better (low p99 99 against 100) but lost exact matches and MAE, so it was not taken.

Held-out half (20 tracks, 969,980 columns), `waveform_compare score --set test`, before to after (Q37 numbers are the "before"):

| `PWV7` | exact % | MAE | correlation |
|---|---|---|---|
| low | 12.0 to 10.8 | 5.08 to 5.10 | 0.960 to 0.963 |
| mid | 26.5 to 27.0 | 2.40 to 2.12 | 0.977 to 0.981 |
| high (unchanged) | 41.7 | 2.78 | 0.931 |

Tune half: low r 0.959 to 0.965 (MAE 5.01 to 4.68), mid 0.958 to 0.958 (MAE 3.32 to 3.13), high 0.911 unchanged. Held-out correlations are at least Q37's (low .960, mid .977, high .931). Pooled shape, held-out: low mean 48.5 (Rekordbox) / 47.1 (ours) after, rise mean 9.74 / 8.71, fall ratio 0.965 / 0.965; mid p99 91 / 89; high p99 83 / 91.

Not solved: the per-track level. Rekordbox's byte scale per track (its `PWVC` gain) does not follow `max(80, target / max envelope)`: Gate's low gain is 94 in Rekordbox, ours falls to the floor of 80, so our low bytes are about 13 % lower on that track (p50 54 against 63) and the stack in the overview is 6 px lower. Dubby: gains 80 / 121 / 107 (Rekordbox) against 84 / 127 / 127. Recovering that rule is a separate piece of work (needs the per-track gains of the whole library against some track statistic).

### Performance

`waveform-perf.mjs`, headless Chromium on the loaded `dev` box, `?mock=3000`, median of 5 alternating runs, main-thread CPU per frame (ms), before is the `q39-wave-fix` build:

| Scenario | before | after |
|---|---|---|
| Compact player playing | 6.24 | 6.60 (earlier full runs of the same build: +1.0 %, +1.3 %, +2.4 %) |
| Expanded player playing | 16.49 | 12.80 |
| Expanded zoom | 16.38 | 12.71 |
| Expanded wheel (zoom out) | 29.61 | 27.06 |

The extra layers cost more at first (+15 to +30 % in the first run); fixed by painting layer by layer (one fill style per layer), skipping layers a taller later layer covers, and an incremental scroll for views at one pixel per column or finer: the zoom canvas moves the last frame by the scroll distance and paints only the new columns (`drawScrolledColumns`; the pixel check compares a scrolled sequence with a fresh paint, 0 differing pixels). `perf/paint-bench.mjs` compares the full-repaint cost of two design-system folders: 17.5 ms to 13.7 ms at 0.3 columns per pixel, 6.2 to 5.4 ms at 8. The compact figure is within the box's noise (spread between runs about 5 %); the code path for it is the static overview bitmap, which did not change per frame.

### Tests and checks

- `waveform-math.test.ts`: held columns (a hard step stays a hard step), overview mean reduction, layer order and tones, hidden-layer pruning, stacked painter and `stackHeights`.
- `waveform-pixels.mjs` (84 checks, all pass): tones in the stack, a hard step stays hard when zoomed in, the stacked overview is anchored at the bottom, scrolling by moving the last frame equals a fresh paint.
- `waveform-flicker.mjs`: 0 bad frames at dpr 1 and 2 (body brightness swing 2.7 % and 2.4 %); the overview is sampled at the baseline now that it is bottom-anchored.
- The 2.6 to 2.7 % swing is the content, as in Q39.

### Open

- The list-row simple envelope (Merlin's choice) and the detail strip (centred bands, 128-column row preview) are not changed except for the new tokens.
- Pure black ground: Rekordbox's waveform panel is `#000`; ours is `--ff-wave-bg` `#0d0d0d`, left as is.
- The stack constants come from two tracks; a third and fourth track of different style would show how far the per-track level (above) matters for the overview.

### Q40 round 2: the white band

Merlin on the Mac: "still off, mostly the high end"; "the high end has different energy levels across the board, in the overview and in the zoom; mid and low seem good". Low and mid were not touched. PNGs, white-only diff images and `white-stats.txt`: `.../waveforms/q40/round2/`. (Some of my first scans of this round ran against a dev server that no longer saw `sed -i` edits and are redone; all numbers below are from the redone runs.)

**Dynamic range of the high band, ours against Rekordbox** (RMS of PWV7 high in 6 s windows over the whole track; `hisec.py`):

| Set | section-level correlation | p90 / p10 of the section levels, Rekordbox / ours | CV, Rekordbox / ours |
|---|---|---|---|
| held-out, 20 tracks | 0.957 | 2.32 / 2.20 | 0.352 / 0.361 |
| tune, 20 tracks | 0.970 | 2.74 / 3.16 | 0.467 / 0.607 |
| Dubby | 0.99 | 15.6 / 15.3 | |
| Gate | 0.99 | 1.3 / 1.3 | |

Nothing is flat or compressed: the section-to-section variation is the same as Rekordbox's (the pooled byte distribution too: p50 / p90 / p99 on the held-out half 1 / 25 / 83 against 2 / 26 / 91). What differs is the **level per track**: ours / Rekordbox of the section level is 0.77 (Dubby) and 0.71 (Gate), and over the 40 tracks the top-decile level of the high bytes is off by 20 % on average (mean absolute log ratio 0.203 tune, 0.186 held-out; signed mean about 0). Clipping at the top is not the cause: columns at 120 or more are 0.14 % in ours against 0.03 % in Rekordbox's. The gain rule has the right form: Rekordbox's high gain is `K / max envelope` of our envelope (slope -1.18 free, -1 fixed, residual 14 % in log; low: -1.02, 10.7 %), smoothed maxima and percentiles are worse (residual 19 to 27 %), so the 14 % is our maximum differing from Rekordbox's (a spike of a slightly different filter), which the exponent 2.08 of the high curve turns into about 30 % in the bytes. Things tried that did not reduce it (all on the tune half, `waveform_compare hiband` now has a per-track level term in its loss, `stereo` and `refdump` are new):

- Filter, feature, release, gain target and curve search around the current setting: the current one (3000 Hz, peak, release 0.90, target 140, curve 2.08) stays best (level 0.203); the nearest alternative (target 143.5, curve 2.13) is 0.210.
- Channel handling instead of the mono sum (louder channel, rms of the channels, mean of |L| and |R|): level error 0.227 / 0.206 / 0.200 against 0.203 (tune), 0.208 / 0.196 / 0.191 against 0.186 (held-out); correlation not better.
- A regression of the level error on statistics of our own envelope and gain (leave-one-out over the 40 tracks): best two-feature model 0.33 against 0.415 without; fitted on the tune half it gives 0.346 against 0.421 on the held-out half and 0.156 on the tune half itself, i.e. over-fitted, so not adopted. Across tracks our level is a little too spread out (sd of log p99 0.72 against 0.62 for Rekordbox, correlation 0.93), the same thing seen from the other side.

No data change for the high band therefore; the held-out numbers are Q40 round 1's (`PWV7` high correlation 0.931, MAE 2.78, exact 41.7 %). The per-track level of the white band stays about 20 % off on average and about 30 % low on Dubby and Gate; recovering Rekordbox's envelope maximum (and so its gain) needs its actual filter, which the 40 tracks do not give.

**Overview and compact strip.** Pooling the high band with a peak share (0.3 to 1.0 of the bucket maximum, scale refitted) was measured against the screenshots: the median of the stack top rises (22 to 25 against Rekordbox's 20 on Dubby) while the tail stays short (26 to 32 against 33), so spikes are not recovered and the stack is too tall; pure mean stays best (Dubby stack top p50 / p90 / p97 / p99.5: 20 / 24 / 27 / 30 against Rekordbox 20 / 27 / 33 / 37; Gate 27 / 34 / 37 / 38 against 28 / 34 / 36 / 37). Why: Rekordbox's PWV6 high is *less* variable than any power mean of its own PWV7 high (CV ratio 0.45 to 0.71 for exponents 0.35 to 5, best per-track correlation 0.80 at exponent 0.5); the tall white hat spikes of its overview come from the PWV6 pass itself (its PWV6 white is not in its PWV7). Our own PWV6 high correlates 0.77 with Rekordbox's PWV6 high, no better than the mean of its PWV7 (0.77), so shipping it in the payload would not help either. Unchanged: mean pooling, `OVERVIEW_SCALE` high 2.0, `STACK_GAIN` high 1.35, exponent 1.7. White pixel area against the screenshots through the app path: Dubby 1.15 times Rekordbox's, Gate 1.66; from our own bytes 0.81 and 0.88 (the track-level difference above again).

**Zoom.** Per-hat comparison with column profiles (`hats.mjs`; 28 and 34 spikes of at least 22 px in Rekordbox's crops, 28 and 32 found within 4 px in ours, position offset 0 px): Rekordbox's bytes through our renderer had the white spikes 8 % too tall (Dubby median 41 against 45 px, mean +3.4 px; Gate 35 against 36, +3.3 px) and 0.2 to 0.8 px too narrow at half height (9 against 8, 12 against 12). `BAND_GAIN.high` 0.89 to 0.83: mean spike height difference +0.5 px on both (Dubby 41 / 42, Gate 35 / 33), widths unchanged. White extents against the screenshot: Dubby MAE 2.10 to 2.02 (r .958 to .957), Gate 2.15 to 1.76 (r .973); the cream core is 0.6 px wider on average (bias). White-only diff, row 2 of the zoom PNGs: Dubby IoU 0.74, Gate 0.81. With our bytes on Dubby, drawn 27 ms later to take out the mp3 lag, the spikes are 14 % shorter (35 against 41 px) and 1.5 px narrower, white correlation 0.78: the level difference above, nothing in the renderer.

Checks: `waveform-pixels.mjs` 84 / 0 failures, `waveform-flicker.mjs` 0 bad frames at dpr 1 and 2 (swing 2.6 % / 2.5 %), `npm test` 169 passed, `npm run check` 0 errors, `npm run build` ok, `cargo build` and `cargo test --workspace` exit 0, no failures. Perf (median of 5 alternating runs, CPU ms per frame, before is `q39-wave-fix`): compact playing 6.46 to 6.54, expanded playing 18.12 to 13.26, zoom 16.89 to 12.74, wheel 28.46 to 28.13.

Honest verdict on the white band per view: zoom with Rekordbox's bytes: matches (heights within 0.5 px, widths within 0.6 px, IoU 0.74 to 0.81); zoom with our bytes: right shape and variation, level 20 to 30 % low on Dubby and Gate (spikes 14 % short on Dubby), Dubby also 27 ms early; overview and compact from Rekordbox's bytes: right rim and structure, tall spikes only from its PWV6; from our bytes: variation right, level low (stack 6 px low on Gate).

## Q40 round 3: the zoomed-out views from a PWV6 series

Merlin on the Mac: "zoomed in looks better, the zoomed-out view isn't right yet". Round 2 had shown that the look of Rekordbox's overview (blue body, thin amber fringe, tall white hat spikes, breakdowns dropping) is in its separate `PWV6` pass and not in the mean of its `PWV7`. Round 3 builds our `PWV6` like Rekordbox's, draws the expanded overview and the compact strip from it, and writes it to the stick. Zoom view, low and mid detail data and list rows are untouched. PNGs, diff images and `stats.txt`: `/tmp/claude-1000/-home-merlin-dev-projects-fourfour/5e0c999f-66b8-4690-ac43-edb78b690bb0/scratchpad/waveforms/q40/round3/`.

### How Rekordbox's PWV6 relates to the audio

Fitted on the raw per-column band peaks of our analysis (`Measured::band_columns`: the peak of each band's filter output per 1/150 s, before release and gain; `waveform_compare refdump` now writes them) against Rekordbox's `PWV6` of the 40-track set, tuned on 20 tracks, checked on the other 20 plus Dubby and Gate:

- **Aggregate.** Each of the 1200 columns is a plain mean of the raw peaks over a trailing window that ends with the column's span and is longer than the span: 2.2 columns' width for low, 1.8 for mid, 1.4 for high (a column is `1 / 1200` of the track, so the window scales with the track, not with seconds: fixed-seconds windows correlate 0.971 to 0.991 against 0.988 to 0.995 and fail on long tracks). A two-tap fit of the bucket means shows the same: weights about 0.5 / 0.5 for low and mid, 0.63 / 0.28 for high. Median per-track correlation of that mean with Rekordbox's column: 0.995 / 0.992 / 0.988 (the plain bucket mean gave 0.90 / 0.88 / 0.90, the mean of the `PWV7` bytes 0.87 / 0.75 / 0.80). Peak, rms, percentiles, power means and a max share were all worse (for the high band a 10 % max share drops the correlation from 0.99 to 0.955 and the spikiness).
- **Level.** Per band `byte = scale * mean^gamma / p99^delta`, p99 being the 99th percentile of the band's 1200 means: low 51.7 / 1.0 / 0.63, mid 36.4 / 0.91 / 0.92, high 93.2 / 1.15 / 0.77. Rekordbox normalises its overview per track (the byte maxima per band have a spread of 0.16 to 0.31 in log across tracks), but not by the maximum; p90 to p99.9 and max as the normaliser were tried, p99 gives the best level error for mid and high. The high exponent 1.15 (the best fit for the mean error was 1.04) buys the spikiness: held-out p90/p10 of the high series 6.15 against Rekordbox's 5.96 (5.18 with 1.04).

Implementation: `waveform::overview_from_columns` (`Tuning::overview_window`, `overview_scale`, `overview_gamma`, `overview_delta`), used by `analyze`; `PWV4` still follows `PWV6`.

### Held-out numbers (20 tracks, 24,000 overview columns each band)

Per-track medians against Rekordbox's `PWV6`, before (mean of the `PWV7` bytes) to after:

| Band | correlation | level error (mean abs log of the track mean) | variation (CV ours / Rekordbox) | p90 / p10, Rekordbox / ours |
|---|---|---|---|---|
| low | 0.873 to 0.996 | 0.098 to 0.076 | 0.99 to 0.96 | 2.25 / 2.21 to 2.16 |
| mid | 0.752 to 0.990 | 0.150 to 0.164 | 0.98 to 0.90 | 2.36 / 2.48 to 2.09 |
| high | 0.804 to 0.990 | 0.282 to 0.156 | 0.99 to 1.04 | 5.96 / 4.68 to 6.15 |

Tune half: correlation 0.912 / 0.817 / 0.837 to 0.995 / 0.991 / 0.987, high level 0.330 to 0.120. Dubby and Gate (not in the 40): correlation 0.847 / 0.629 / 0.847 to 0.998 / 0.982 / 0.985, level error 0.076 / 0.055 / 0.079, high p90/p10 11.30 against ours 11.76. Pooled over all held-out columns (`waveform_compare score --set test`, the export path): `PWV6` low / mid / high correlation 0.864 / 0.735 / 0.767 (Q37 table) to 0.972 / 0.858 / 0.938, MAE 4.41 / 5.10 / 9.56 to 2.07 / 3.54 / 3.05 (the pooled mid number is limited by the track level, per-track the shape correlation is 0.99). What is left is the track level (mid 16 %, high 16 % mean abs log), the same unrecovered per-track normalisation as in round 2, now much smaller for the overview because the p99 normalisation carries it.

### Data path

`ColorWaveform::overview` already was the analysis' `PWV6`. The writer now uses it (and the `PWV4` that follows) when the analysis carries the measured columns (`exact` present and 1200 entries); without them (Python analysis, older stored tracks) `PWV6` is still derived from the `PWV7` bytes. **The export improves too**: the `PWV6` section (and `PWV4`) on the stick is the new overview; an analysis read back from a stick carries Rekordbox's own `PWV6` and writes it back unchanged. The payload already sends `waveform_overview` (stick values only for tracks with `pwv7`); the app now draws it (`player.track.bandsOverview`, `overviewBandsFromPayload`) in the expanded overview and, through `PlayerCompact`, the compact strip. Tracks stored before this change keep their old overview bytes (mean of the `PWV7`, about the right level) until they are analysed again; tracks without stick values draw from the detail columns as before. The renderer draws overview-level data as it is (`BandColumns.overview`), detail data is still reduced and scaled down for the fallback.

### Renderer against the screenshots (Rekordbox's own `PWV6` through our renderer, then ours)

The stack gains were refit with Rekordbox's `PWV6` as input on both screenshots at their widths (1814 and 1847 px): `STACK_GAIN` 1.08 / 0.62 / 1.69, high exponent 1.72. Stack-top percentiles p50 / p90 / p97 / p99.5 (px of a 38 px strip):

| | screenshot | our renderer, Rekordbox's PWV6 | our renderer, our PWV6 |
|---|---|---|---|
| Dubby | 20 / 27 / 33 / 37 | 21 / 28 / 33 / 36 | 21 / 26 / 31 / 36 |
| Gate | 28 / 34 / 36 / 37 | 26 / 31 / 32 / 34 | 24 / 30 / 32 / 35 |

Column errors against the screenshot (MAE px, stack top / blue top): Dubby 2.5 / 0.6 from Rekordbox's PWV6, 2.3 / 0.8 from ours; Gate 3.3 / 1.4 and 4.2 / 2.4. Silhouette overlap of the ink (any pixel that is not ground): Dubby 0.89 (Rekordbox's bytes) and 0.90 (ours), Gate 0.88 and 0.84. The white-only overlap is low (0.32 to 0.44) because the white spikes are one to two pixels wide and the screenshot is a blurred downscale: our white covers 1.4 to 1.8 times the pixels above the threshold.

Honest verdict: Dubby: breakdowns, blue body, amber fringe and the white hat spikes appear at the same places and heights from our own `PWV6`; the stack is right to within the percentiles above. Gate: structure and spike positions match; our stack is about 4 px lower at the median and the spikes are 1 to 4 px shorter (Rekordbox's Gate `PWV6` itself gives 26 against its screenshot's 28 through the same renderer, so about half of it is the display of that track, not our bytes).

### Checks

`waveform-pixels.mjs` 84 checks, 0 failures; `waveform-flicker.mjs` 0 bad frames at dpr 1 and 2 (swing 2.6 % / 2.4 %); `npm test` 172 passed, `npm run check` 0 errors, `npm run build` ok; `cargo test --workspace` exit 0 (new tests: `overview_keeps_spikes_and_breakdowns_of_the_audio`, `the_stick_carries_the_overview_measured_from_the_audio`, the overview payload and renderer tests). Perf (median of 5 alternating runs, CPU ms per frame, before is `q39-wave-fix`): compact playing 6.35 to 6.22, expanded playing 16.36 to 13.12, zoom 16.52 to 12.84, wheel scroll 28.97 to 26.52.
