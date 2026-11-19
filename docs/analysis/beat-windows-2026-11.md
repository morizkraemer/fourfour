# Windowed beat model (2026-11-19)

Question: can the beat model run on a few short windows instead of the whole track, with the grid fitted from them and extended, without losing grid, BPM or bar-1 quality?

Answer: **yes for about 4 in 5 tracks. Analysis CPU per track drops 2.1x on the test split (beat model ~2.9x on the tracks that stay windowed); BPM, grid within 20 ms and bar-1 vs Rekordbox stay within one track of the full model.** About 1 in 5 tracks falls back to the full model.

## Switch (default off)

`FOURFOUR_BEAT_WINDOWS=<seconds>` (e.g. `30`) turns it on (`BeatWindows::from_env`, `fourfour-analysis-rs/src/bt.rs`). Unset, every path is the old one.

How it works:

- The mel spectrogram is still computed for the whole track (cheap: ~2% of the full-mode time).
- Only the beat-model chunks (500 frames, 10 s) that overlap 3 windows centred at 25%, 50% and 75% of the track run. A 30 s window takes 4 chunks, so about 2 minutes of audio are modelled. A track where the windows would cover 70% or more of the chunks runs whole (`beat_mode: full`).
- The normal `fit_track` runs on the stitched logits; frames outside the windows are silent, so the comb fits one constant tempo and phase across all windows (the long baseline between windows pins the tempo).
- Check per window (`fit::check_windows`, `fit::windows_agree`): refined model beats within 35 ms of the grid are inliers. A window with fewer peaks than 25% of the expected beats is beatless and ignored. The grid is kept when at least 2 windows have beats, each has inliers for 60% of its expected beats, each window's median offset is within 15 ms, and the medians of the windows lie within 10 ms of each other.
- Otherwise the remaining chunks run (the window chunks are reused), and the full-track fit is used (`beat_mode: fallback`). The result is then identical to full mode.
- Bar-1: unchanged rule (`BAR_ONE_RULE`, first beat unless the model vote is strong). In windowed mode the vote only sees downbeats inside the windows.

`to_json` adds `beat_mode` and `window_checks` when the switch is on.

## Method

- Box `dev` (x86, 16 cores, load 9-15 from other work), `fourfour-analysis-rs/examples/beat_windows_bench.rs`, release, 4 workers x 1 thread, key and waveform off, fp32 model (x86 default), chunk 500.
- Truth: Rekordbox grids `benchmark/results/o3/ref.json`, splits `o3/dev.txt` (50) and `o3/test.txt` (205), real library tracks (test: median 6.1 min, p10 3.9, p90 8.2, max 15.2). Scored with `benchmark/scripts/o3-score.py` (as R177/O3).
- Windowed vs full on the same tracks: `benchmark/scripts/bw-compare.py`.
- Rules were tuned on dev. The 60% inlier share was raised from 50% after a test track came out half a beat off (off-beat hi-hat peaks). Results for the final rule are an exact replay of the 50% run (`bw-simulate.py`; a stricter rule only moves tracks to fallback, and fallback equals full mode; windowed runs repeat bit for bit, checked on 160 tracks).

## Results

Test split, 205 tracks:

| | full | windowed 30 s |
|---|---:|---:|
| CPU per track | 46.8 s | 22.3 s (2.1x less; measured with the 50% rule, the 1 extra fallback adds ~0.15 s) |
| CPU per audio minute | 7.5 s | 3.6 s |
| BPM Acc1 (4%) | 194 (94.6%) | 194 |
| BPM within 0.1 | 185 (90.2%) | 185 |
| grid within 20 ms of Rekordbox | 171 (83.4%) | 171 |
| bar position right | 142 (69.3%) | 141 |
| beats within 20 ms of the full grid | - | 100% of tracks have >=99% |
| bar-1 same as full | - | 202 / 205 |
| windows kept / fallback / short track run whole | - | 159 / 39 / 7 (78% / 19% / 3%) |

Dev split, 50 tracks: grid 44 -> 43, bar 42 -> 43, BPM equal, fallback 11/50.

Back-to-back timing, 41 test tracks (every 5th), nothing else of this run on the box: CPU 41.8 -> 19.5 s per track (2.15x), 7.3 -> 3.4 s per audio minute, wall 407 -> 171 s. Per-track wall per audio minute (median): kept windows 2.5 s, fallback 5.8 s, full 7.1 s.

Where the windowed fit differs from full:

- Bar-1 (3 on test, 1 on dev): the windows hold the steady body of the track, so the model vote is more often strong (>= 80%) and overrides "first beat". Right on 2 tracks, wrong on 2.
- 1 track on test: BPM 121.011 vs 121.000, max deviation from Rekordbox 16 ms (still within 20 ms).

Fallback reasons on test (39): 24 low inlier share in some window (weak or changing beats), 15 phase spread over 10 ms (slight tempo drift or a fit the windows do not pin).

## Next

- Fewer fallbacks: on a failed check, add one more window instead of running the rest of the track.
- Bar-1 in windowed mode: the 0.8 vote threshold was set on full-track votes; check it on more tracks before turning the switch on.
- Mac (int8 + prepack): the same 41-track pair with the int8 model on x86 gives CPU 38.8 -> 18.3 s per track (2.1x), 7 fallbacks instead of 5; the Mac itself is not measured yet.
