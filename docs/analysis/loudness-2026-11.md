# R99, 2026-11-04: loudness (integrated LUFS) and suggested gain

Status: implemented, measured. Branch `r99-lufs` (from `ov-1028`). Off by default (`Config::loudness = false`); every existing output stays byte-identical. Nothing is stored, shown or written to USB yet.

## Result

- `Analysis::loudness: Option<Loudness { lufs, peak_db, gain_db }>`, computed by `fourfour-analysis-rs/src/loudness.rs`.
- Integrated loudness matches ffmpeg's `ebur128` filter on 22 real tracks within **0.013 LU** (mean 0.005 LU); the target was 0.5 LU. True peak within **0.07 dB** (mean 0.02 dB).
- Cost: about **2.2 ms of CPU per second of audio** (0.6 s for a 4.7 minute track), about **1 %** of the pipeline's CPU for the same track.
- Plain Rust, no new dependency.

## Method

ITU-R BS.1770-4 / EBU R128, computed in one pass over the audio the decoder already produces:

1. `decode::decode_mono_measured(path, true)` feeds every decoded packet (interleaved, native sample rate, before the mono downmix) to `loudness::Meter`. The decode itself is unchanged: samples, gapless range and truncation handling are identical with and without the meter (asserted in `tests/loudness_decode.rs`). `decode_mono` is the same call with the meter off.
2. Per channel, a two-stage K-weighting filter: high shelf (about +4 dB above 1.7 kHz), then a 38 Hz high-pass, as f64 biquads. The coefficients are designed analytically for any sample rate (the spec only tabulates 48 kHz); a test pins the 48 kHz result to the spec table.
3. Channel weights: 1.0 for front channels and mono, 1.41 for side and rear channels, 0 for LFE. Channel order is symphonia's canonical position order. Mono is one channel (not doubled); stereo is L + R energy.
4. 400 ms blocks with 75 % overlap (100 ms hop), mean square summed over the weighted channels, loudness `-0.691 + 10 log10(z)`. Only complete blocks count.
5. Two-stage gating: blocks below -70 LUFS are dropped; the mean of the rest minus 10 LU is the relative gate; the result is the energy mean of the blocks above it.
6. Peak: 4x oversampled true peak (49-tap Hann-windowed sinc, 12 taps per phase, in the spirit of BS.1770 Annex 2), maximum over all channels, in dB relative to full scale (dBTP). Windows that cannot exceed the running peak (window maximum times the filter's L1 gain) are skipped. That skip is exact, not a heuristic; a test compares against an exhaustive computation.
7. `gain_db = REFERENCE_LUFS - lufs`, with `REFERENCE_LUFS = -14.0`.

`loudness` is `None` for silence, for tracks shorter than one 400 ms block, and when nothing rises above -70 LUFS.

### Reference level

-14 LUFS is the common streaming normalisation target (Spotify, YouTube and Tidal use values around it). It is a constant (`loudness::REFERENCE_LUFS`), not a setting. All 22 tracks below sit between -5.2 and -12.5 LUFS, so every suggested gain is negative (-1.5 to -8.8 dB): club masters are louder than -14. A different reference (for example around -9 LUFS to match club masters) is a product decision for later. The stored value is `lufs`; `gain_db` is derived from it, so changing the reference never needs a re-analysis.

`gain_db` is deliberately not limited by the peak. A consumer that applies a positive gain must check `peak_db + gain_db <= 0`; 18 of the 22 tracks have a true peak above 0 dBTP (loud masters and decoded mp3 overshoot), so a cap or limiter is the consumer's call.

### How to turn it on

```rust
let analyzer = Analyzer::load(&models, PoolOptions::default())?.with_config(|c| c.loudness = true);
```

or `Config { loudness: true, ..Config::default() }` with `analyze_controlled`. `Analysis::to_json` adds a `"loudness"` object only when it was measured, and `ms.loudness` carries the time spent in the meter. `Analyzer::analyze` returns the writer's `AnalysisResult`, which has no loudness field (that type lives in the format library), so today only `Analysis` consumers see the value; see "How it would surface".

## Accuracy

Reference: ffmpeg `ebur128=peak=true` (`lavfi.r128.I` and `true_peaks_ch*` read through `ametadata`), used only for comparison, never as a dependency. 22 tracks selected with a fixed seed by `benchmark/scripts/r99-loudness.py` from the read-only datasets (7 Beatport EDM Key mp3, 5 library mp3, 4 flac, 3 wav at 44.1 kHz, 1 wav at 48 kHz, each 1 to 10 minutes) plus two derivatives of track 1 made with ffmpeg: a mono flac (row 21) and a 48 kHz flac (row 22). The diff columns use full-precision values (ours minus ffmpeg); the ffmpeg columns are rounded to one decimal here.

| # | format | ffmpeg LUFS | ours LUFS | diff LU | ffmpeg TP dB | ours TP dB | diff dB | suggested gain dB |
|---|---|---|---|---|---|---|---|---|
| 1 | mp3 44k 2ch | -7.6 | -7.57 | +0.00 | 3.0 | 2.99 | -0.00 | -6.43 |
| 2 | mp3 44k 2ch | -5.2 | -5.22 | +0.00 | 4.7 | 4.77 | +0.03 | -8.78 |
| 3 | mp3 44k 2ch | -10.3 | -10.29 | -0.00 | 3.3 | 3.25 | -0.01 | -3.71 |
| 4 | mp3 44k 2ch | -7.3 | -7.26 | +0.01 | 3.0 | 3.04 | -0.00 | -6.74 |
| 5 | mp3 44k 2ch | -10.9 | -10.92 | +0.00 | 3.0 | 2.97 | +0.00 | -3.08 |
| 6 | mp3 44k 2ch | -10.9 | -10.90 | +0.01 | 0.8 | 0.74 | -0.02 | -3.10 |
| 7 | mp3 44k 2ch | -7.4 | -7.37 | +0.00 | 4.8 | 4.79 | +0.01 | -6.63 |
| 8 | mp3 44k 2ch | -10.6 | -10.58 | +0.01 | -0.1 | -0.12 | -0.04 | -3.42 |
| 9 | mp3 44k 2ch | -11.6 | -11.55 | +0.00 | -0.0 | -0.05 | -0.02 | -2.45 |
| 10 | mp3 44k 2ch | -9.9 | -9.90 | +0.01 | 2.1 | 2.07 | +0.01 | -4.10 |
| 11 | mp3 44k 2ch | -11.7 | -11.68 | +0.00 | 1.0 | 0.97 | -0.05 | -2.32 |
| 12 | mp3 44k 2ch | -9.9 | -9.92 | +0.01 | 0.4 | 0.39 | +0.00 | -4.08 |
| 13 | flac 44k 2ch | -8.4 | -8.39 | +0.01 | 0.8 | 0.71 | -0.07 | -5.61 |
| 14 | flac 44k 2ch | -9.9 | -9.93 | +0.00 | 0.2 | 0.17 | -0.03 | -4.07 |
| 15 | flac 44k 2ch | -12.5 | -12.51 | +0.00 | 0.1 | 0.08 | +0.03 | -1.49 |
| 16 | flac 44k 2ch | -11.5 | -11.49 | +0.01 | 0.4 | 0.39 | +0.01 | -2.51 |
| 17 | wav 48k 2ch | -8.9 | -8.87 | +0.00 | 0.6 | 0.57 | -0.04 | -5.13 |
| 18 | wav 44k 2ch | -7.2 | -7.19 | +0.00 | 0.9 | 0.84 | -0.02 | -6.81 |
| 19 | wav 44k 2ch | -10.6 | -10.54 | +0.01 | -0.9 | -0.95 | -0.02 | -3.46 |
| 20 | wav 44k 2ch | -9.0 | -9.02 | +0.01 | -0.0 | -0.02 | +0.00 | -4.98 |
| 21 | flac 44k 1ch | -10.9 | -10.85 | +0.00 | 0.6 | 0.59 | +0.02 | -3.15 |
| 22 | flac 48k 2ch | -7.6 | -7.57 | +0.01 | 0.9 | 0.92 | +0.01 | -6.43 |

Summary: |diff LUFS| mean 0.005, max 0.013 over 22 tracks. |diff TP| mean 0.020, max 0.071 dB (both are band-limited 4x interpolators with different filters). The small remaining LUFS difference is probably block alignment against ffmpeg's frame grid and its decoder's mp3 start handling; it was not investigated further because it is 40 times below the target.

Synthetic checks (unit tests in `loudness.rs`, all pass):

| signal | expected | test |
|---|---|---|
| mono 997 Hz sine at 0 dBFS | -3.01 LUFS (BS.1770 definition of the -0.691 offset) | within 0.02 at 44.1 and 48 kHz |
| stereo 997 Hz sine at -23 dBFS | -23.0 LUFS (EBU Tech 3341 style) | within 0.02 at 44.1, 48 and 96 kHz |
| 20 s at -36 then 20 s at -23 LUFS | -23.0 (quiet half is below the relative gate) | within 0.05 |
| 20 s at -80 then 20 s at -23 | -23.0 (quiet half is below the absolute gate) | within 0.05 |
| 20 s at -26 then 20 s at -20 | energy mean of both (both inside the gate) | within 0.05 |
| 30 dB level step | exactly 30.00 LU | within 0.01 |
| 30 Hz and 10 kHz sines against 997 Hz | K-weighting attenuates lows, lifts highs | sign and size |
| LFE channel, rear channel | LFE ignored; rear +1.49 dB | within 0.01 |
| 12 kHz sine at 48 kHz, phase 45 degrees | sample peak -3.0 dBFS, true peak 0 dBFS | within 0.15 dB |
| silence, 0.3 s clip, -90 dBFS tone | `None` | exact |
| 16-bit wav files through the real decoder | -26.0 LUFS stereo, -29.01 mono, peak -26 dBFS | within 0.05 |

## Cost

Measured on `dev` while it was loaded (load average 17 to 50 on 16 cores), so absolute numbers are inflated; the ratio is the useful part.

- Meter alone, on the 22 decoded tracks: **2.16 ms CPU per audio second** (min 1.76, max 2.87), 591 ms per track on average (mean track 282 s). Thread CPU time, best of 3 runs. The right channel in this measurement is a delayed copy of the left, because the decoder only keeps a mono mix; the cost does not depend on the content except for the true-peak skip.
- Pipeline CPU for four of the same tracks (beat model, key, waveform, fit, 1 thread): 31.4 s (120 s track), 29.0 s (120 s), 89.8 s (424 s), 78.6 s (360 s). The meter is **0.76 %, 0.98 %, 0.96 %, 1.15 %** of those. The pipeline with and without the flag differs by less than its run-to-run noise (-3 % to +4 %).
- Memory: one `f64` per 100 ms of audio (6000 values for a 10 minute track) plus a few small buffers.

The first version updated a per-sample history and was about four times slower. The current version works on de-interleaved blocks and skips windows that cannot set a new peak.

## Byte-identical proof

`analyze-json` built from `ov-1028` (8249974, in a clean `git archive` copy) and from `r99-lufs` with the flag off were run on the same six tracks (3 mp3, 1 flac, 2 wav; `--threads 2`). The 5.2 MB JSON outputs (beat grids, waveforms, keys, BPM) are identical: `cmp` reports no difference and both have sha256 `ff998501c7fb37d5960e68129cfb6d0d909248c837f682e86d34464413511489`.

With the flag on, the decoded samples are unchanged by construction (the meter only reads the decoder's scratch buffer; `tests/loudness_decode.rs` asserts equal samples and gapless range), so beats, key and waveform cannot change either. An `analyze.rs` test checks that the default is off and that the JSON has no `loudness` key unless it was measured.

## How it would surface

Nothing below is decided or built; it lists what each option touches.

- **Analysis result**: `pioneer_usb_writer::models::AnalysisResult` has no loudness field. Carrying `Option<Loudness>` through `service::to_app_result` needs a field there, and that crate is the format library (another lane). An alternative with no change there: a second return value or callback from `Analyzer`.
- **Library**: a nullable `loudness_lufs` and `peak_db` column pair next to the analysis data in `library.db` (a schema change, needs sign-off). `gain_db` should be derived at read time from `lufs` and the chosen reference. An `analysis_key` bump is only needed if the flag becomes default-on.
- **Rekordbox/USB field**: in Rekordbox's desktop `master.db`, per-track gain and peak live in `djmdMixerParam` (`ContentID`, `GainHigh`, `GainLow`, `PeakHigh`, `PeakLow`; listed in `pioneer-usb-writer/reference-code/PIONEER.md`). How `GainHigh`/`GainLow` encode a dB value, and whether the OneLibrary `exportLibrary.db` or the PDB on a USB stick carries an equivalent that CDJs read, is unverified. Checking it needs a Rekordbox-exported library and belongs to the writer's owner.
- **UI**: no decisions made.
- **Turning it on by default**: about 1 % more CPU per track, no extra decode, negligible memory. Existing libraries would need a re-analysis to fill it; a loudness-only pass would need its own decode.

## Limits

- Integrated loudness only. No momentary, short-term or loudness range, and no per-section values (a small extension of the same block list).
- The 100 ms hop starts at the first decoded frame, including mp3 encoder delay samples; at most one hop of misalignment against other meters, which the gating absorbs.
- Layouts beyond mono and stereo use symphonia's position bits. The weights are unit tested, but no real multichannel file was available in the datasets.
- `ms.loudness` is wall time inside the meter and includes waiting when the machine is busy; use the CPU figures above for comparisons.

## Reproduce

```bash
export CARGO_TARGET_DIR=$HOME/.cache/r99-target
cargo build --release -p fourfour-analysis-rs --example loudness_report
python3 benchmark/scripts/r99-loudness.py benchmark/datasets <scratch-dir> $CARGO_TARGET_DIR/release/examples/loudness_report
$CARGO_TARGET_DIR/release/examples/loudness_report --cpu --list <scratch-dir>/tracks.txt
FOURFOUR_MODELS_DIR=<models> $CARGO_TARGET_DIR/release/examples/loudness_report --full 1 --cpu --list <tracks>
cargo test --release -p fourfour-analysis-rs --lib loudness
```
