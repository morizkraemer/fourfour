# 3-band waveforms (2026-10)

Two halves: how the USB export writes them (this section, Q34, branch `q34-export-waveforms`) and how the app draws them (the app-rendering worker adds its section here).

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
