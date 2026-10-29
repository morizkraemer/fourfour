# Grid offset: why the grid looks "always a little off" (2026-10-29)

Branch `fb-grid` (from `ov-1028`). Report: Merlin on the Mac, "even simple 4/4 tracks with clear transients are always a little bit off."

## Result

The grid itself was not off against the audio for most files. The time bases were:

1. **Waveform and audio disagreed for 79% of the measured mp3s (59 of 75: every file with a LAME/Lavc/Lavf Info tag).** The analyzer decodes with symphonia 0.6, whose decoder trims the encoder delay by default (`AudioDecoderOptions::gapless = true`, 1105 samples = 25.1 ms at 44.1 kHz). The player and the zoomed-waveform decode use symphonia 0.5 with `enable_gapless = false` (untrimmed). The grid was written in the untrimmed (Rekordbox) timeline. So the overview/colour waveform was drawn 25 ms before the audio, the playhead and the grid. Median grid minus drawn transient on mp3: **+27.3 ms** (wav/flac: +2.8 / +4.4 ms).
2. **26 ms late against the audio on mp3s whose Info frame Rekordbox plays as a frame** (LAME 3.99r files here, 16 of 75 measured test mp3s, 21% of them). Rekordbox's decoder outputs that frame as 1152 silent samples, so its whole timeline sits one frame later; our grid copied Rekordbox's timeline but our player skips the frame. Median grid minus heard transient there: **+31.2 ms** (waveform: +45.1 ms).
3. The tag duration (lofty) that maps waveform columns, grid and playhead to pixels equals the untrimmed length, not the trimmed one the waveform covered, so the waveform was also slightly stretched.

Not a cause: Beat This! frame quantisation (the fit already refines each beat with a probability centroid over +-2 frames and then fits one comb over the whole track, so the line is sub-millisecond), the resampler, and any constant in the fit or the display. The model does fire about 10 ms after the attack, but the existing -10 ms shift already compensates it. Output latency is not compensated (below), but it is the same for every track and is not what was measured.

**Fix:** one time base for analysis, waveforms, playback and the stored grid, equal to Rekordbox's (so exported grids and imported Rekordbox grids also line up):

- `decode_mono` decodes untrimmed (`gapless(false)`) and prepends 1152 zero samples when the Info frame counts (`mp3::leading_silence_frames`, the existing cyanbox frame-map rule `header_counts`).
- The player (`playback::run_decode`) and the zoom decode (`band_analysis::decode_blocks_from`) play the same lead, including after seeks.
- The analysis payload's `duration_ms` adds the lead, so waveform columns span the decoded audio.
- The model and the key detector still get the gapless samples (`Mono::gapless`, the slice a trimming decoder would return; checked bit-identical on 6 files), and their beats are moved into the decoded time base by the slice start. So BPM, key and the model's logits are unchanged. Feeding the model the untrimmed audio instead was tried first: the 25 to 51 ms input shift changed the logits enough to flip the tempo of 3 of 88 test mp3s (ids 33, 525, 612; one other gained), net 163/205 instead of 165.
- The per-encoder mp3 rules (`enc+header+l3100`) become one rule for every file: the -10 ms model lead (`DelayRule::ModelLead`, `MODEL_LEAD_MS`). The old "Lame3.100 is one frame early" rule (Q30, "mechanism not known") is explained: symphonia only trims tags starting with `LAME`, `Lavf` or `Lavc`, and `Lame3.100` is lower-case, so those files were never trimmed.

AIFF and M4A: there are none in the benchmark (test: 88 mp3, 75 wav, 42 flac). PCM containers have no delay. For AAC, neither symphonia version handles the 2112-sample priming (no edit list or iTunSMPB support in `symphonia-format-isomp4` 0.5 or 0.6), so analysis and playback share the untrimmed timeline; whether Rekordbox trims it is unmeasured.

## Method

`fourfour-analysis-rs/examples/grid_offset.rs` (re-fits the dumped logits with the app's `FitParams::default()`, then decodes the audio):

- Onsets: log-magnitude spectral flux, 512-sample Hann window, 32-sample hop, parabolic peak, strongest peak within +-50 ms of each grid beat, kept when it is 4x the track's median flux. Per track the median signed offset over all such beats.
- Self-test on synthetic kicks (`grid_offset selftest`): the flux peak sits **4.1 ms before** the true attack start. So "+4 ms against the flux onset" means "on the attack".
- "Heard" = the player's timeline (before: untrimmed; after: the decoded timeline, which the player now matches). "Wave" = the timeline of the decode that made the waveform (before: trimmed).
- Test split as in rust-status-2026-10-28.md: 205 tracks, logits from `benchmark/results/o3/dump`. After the fix the model input is bit-identical, so the same logits are re-fitted with the new rule (`rebased` mode); a fresh `analyze()` run on 4 mp3s (ids 15, 33, 525, 612) gave byte-identical logits and the same grid. wav/flac are unchanged by the fix (no delay, no lead, same -10 ms).

## Before (test, 184 tracks with BPM within 0.5 of Rekordbox)

Median / mean of the per-track median offset, ms. Positive = grid later than the audio.

| Group | n | model beat vs flux onset | grid vs heard | grid vs drawn waveform | Rekordbox vs untrimmed audio | grid vs Rekordbox |
|---|---:|---:|---:|---:|---:|---:|
| all | 184 | +13.8 / +13.5 | +3.7 / +5.2 | +5.4 / +11.1 | +2.8 / +5.0 | +0.8 / +1.1 |
| wav | 67 | +12.8 / +11.6 | +2.8 / +1.7 | +2.8 / +1.7 | +2.6 / +2.6 | +0.4 / -2.0 |
| flac | 42 | +14.4 / +14.5 | +4.4 / +4.5 | +4.4 / +4.5 | +2.9 / +3.6 | +1.9 / +1.0 |
| mp3 | 75 | +15.0 / +14.7 | +5.1 / +8.7 | **+27.3 / +23.8** | +3.0 / +8.1 | +1.6 / +3.8 |
| mp3 LAME3.99r, Info frame counts | 16 | +17.1 / +16.1 | **+31.2 / +30.1** | **+45.1 / +37.6** | +28.9 / +29.8 | +0.5 / -15.8 |
| mp3 Lavc/Lavf (trimmed by symphonia) | 43 | +14 to +20 | +1 to +8 | **+25 to +33** | +2 to +4 | -1 to +7 |
| mp3 Lame3.100 (not trimmed) | 13 | +14.6 / +14.0 | +1.5 / +1.0 | +1.5 / +1.0 | +2.7 / +2.6 | +0.7 / +18.3 |

Reading: Rekordbox's grid sits 2.6 to 3.0 ms after the flux onset on every container, i.e. on the attack (flux leads the attack by 4 ms), in the untrimmed timeline, plus exactly one frame (+26 ms, per track 23 to 39 ms) for the 16 "Info frame counts" files. The model fires about 11 ms after Rekordbox. The grid matched Rekordbox (+0.8 ms median), so it also matched the heard audio, except for the 16 Info-frame files; but it was drawn 25 to 45 ms late on the waveform of 59 of the 75 mp3s.

Worst per track before (grid vs drawn waveform): 594 Pe Apa Zambetei +50.0 (heard +28.1), 615 Mamute +49.3 (+30.5), 597 I Feel Furrr U +49.1 (+33.4), 33 Alberto Balsalm +47.1 (+37.1), 15 New Forms +47.1 (+25.6), 567 Fire Guard +46.8 (+21.8), 516 Springing Daisy's +45.1 (+25.0), 609 Creatures +45.0 (+20.2), 531 Work Together +43.9 (+23.6), all LAME3.99r with a counting Info frame; then 543 Sumback (Lavf) +42.7.

## After

Same tracks, same columns. "Heard" and "drawn waveform" are now the same timeline.

| Group | n | grid vs heard | grid vs drawn waveform | Rekordbox vs decoded audio | grid vs Rekordbox |
|---|---:|---:|---:|---:|---:|
| all | 184 | +3.9 / +3.6 (was +3.7 / +5.2) | **+3.9 / +3.6** (was +5.4 / +11.1) | +2.7 / +2.9 | +1.3 / +1.7 |
| wav | 67 | +2.8 / +1.7 | +2.8 / +1.7 | +2.6 / +2.6 | +0.4 / -2.0 |
| flac | 42 | +4.4 / +4.5 | +4.4 / +4.5 | +2.9 / +3.6 | +1.9 / +1.0 |
| mp3 | 75 | +5.0 / +4.8 | **+5.0 / +4.8** (was +27.3 / +23.8) | +2.6 / +2.8 | +3.6 / +5.5 |
| mp3 LAME3.99r, Info frame counts | 16 | **+7.1 / +6.1** (was +31.2) | **+7.1 / +6.1** (was +45.1) | +2.8 / +4.7 (was +28.9) | +2.5 / -13.8 |
| mp3 Lame3.100 | 13 | +4.6 / +4.0 | +4.6 / +4.0 | +2.7 / +2.6 | +3.7 / +21.1 |

With the flux onset 4 ms ahead of the attack, the grid now sits about 0 to 3 ms after the attack on every container, against both the sound and the drawn waveform, and Rekordbox's grid lands on our decoded audio the same way for every encoder (+2.6 to +2.9 ms). The mean "grid vs Rekordbox" values with large magnitude are single tracks with a wrong tempo or half-beat phase, not offsets.

Worst tracks after (BPM right, by |grid vs waveform|): 411 CAN - The Empress And The Ukraine King (wav) -23.0, 543 Sumback (mp3 Lavf) +19.6, 555 Besides (mp3 Lavc) +18.9, 468 Black Paint (mp3 Lavc) +18.4, 117 Moodymanc - Black Paint (flac, same track) +18.3, 183 Star Butchers (flac) +17.6. In each the model's own beat sits 25 to 30 ms after the attack (or 13 ms before it on 411), across containers, so these are per-track model placement, not time base; Rekordbox agrees with the attack better on them (grid vs Rekordbox +8 to +17).

## Accuracy against Rekordbox (rust-status metric: whole grid within +-20 ms)

| | Before (app today) | After |
|---|---:|---:|
| Grid within +-20 ms, test (n=205) | 165 = 80.5% | **167 = 81.5%** |
| Grid within +-20 ms, dev (n=49) | 44 | 44 |
| BPM within +-0.1 / +-0.5 / +-1, test | 180 / 184 / 190 | 180 / 184 / 190 |
| Key | unchanged by construction (same input samples) | |

Tracks that changed (test): +2 (id 531 LAME3.99r 21.8 -> 19.8 ms worst beat; id 534 mp3 without Info tag 23.6 -> 13.6 ms), none lost. The "before" run reproduces rust-status-2026-10-28.md exactly (165/205, dev 44/49). The metric barely moves because the old per-encoder rules already approximated Rekordbox's timeline within 1 to 2 ms; what changed is that the waveform and the player now share it.

## Display side

- Grid, playhead and waveform are all placed by `time / durationMs` (`prototyping/viewport/design-system/waveform-display.js`); waveform columns by `index / len`, so the waveform is only right when `durationMs` equals its length at 150 columns/s. `durationMs` is the lofty tag duration, which equals the untrimmed decode to 0.4 ms (checked on wav, flac, Lavc, Lame3.100 and LAME3.99r files with `grid_offset durations`); the Info-frame files are 26 ms longer after the fix, hence the payload change.
- **Output latency is not compensated.** The playhead is `start + frames_played / rate` (`playback.rs::clock_position_ms`), counted when the callback hands frames to CoreAudio, not when they are heard. With the default buffer this is a few ms plus the device's own latency, so the playhead leads the sound by that amount for every track. Bluetooth outputs make this 100+ ms. Not changed here (it is display/transport, not analysis); cpal's `OutputCallbackInfo::timestamp()` gives `playback - callback` to subtract.

## Existing libraries

Tracks analysed before this change keep their old waveform (25 ms early on most mp3s) and, for the Info-frame files, a grid 26 ms late against the new player. Re-analysis fixes both. Hand-set grid offsets on those files were set against the old picture.

## Reproduce

Paths relative to the repo root; results under `benchmark/results/fb-grid` (gitignored). `test-ids.txt` / `dev-ids.txt` are the ids of `o3/test.txt` / `o3/dev.txt` with a dump.

```bash
B=benchmark/results; E=target/release/examples/grid_offset
cargo build --release -p fourfour-analysis-rs --example grid_offset
$E selftest
$E $B/o3/ref.json $B/o3/dump $B/fb-grid/test-ids.txt $B/fb-grid/after-test.jsonl 4 4.0 model rebased
python3 -I benchmark/scripts/fb-grid-offsets.py $B/fb-grid/after-test.jsonl --worst 10
$E durations $B/fb-grid/timebase-list.txt
FOURFOUR_MODELS_DIR=~/.cache/fourfour-models $E run $B/o3/ref.json $B/fb-grid/check-ids.txt $B/fb-grid/dump-check 4
FOURFOUR_TIMEBASE_LIST=$B/fb-grid/timebase-list.txt cargo test --release -p pioneer-test-ui playback_and_analysis_share_the_time_base -- --nocapture
```

The "before" table was measured on the code before this branch (`... before-test.jsonl 4 4.0 enc+header+l3100 untrimmed`, trimmed decode). The grid-vs-Rekordbox columns of that line do not depend on the decode and still reproduce on the new code; the onset columns do.
