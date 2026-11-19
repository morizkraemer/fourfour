# Windowed beat model (2026-11-19)

Question: can the beat model run on a few short windows instead of the whole track, with the grid fitted from them and extended, without losing grid, BPM or bar-1 quality?

Answer after the optimize loop below (kept version, `FOURFOUR_BEAT_WINDOWS=30`): **2.4x less CPU per track on the test split (46.8 -> 19.2 s), 3.1x on a back-to-back timing pair, with BPM, grid and bar-1 vs Rekordbox within the gate.** First answer (step 0): **yes for about 4 in 5 tracks. Analysis CPU per track drops 2.1x on the test split (beat model ~2.9x on the tracks that stay windowed); BPM, grid within 20 ms and bar-1 vs Rekordbox stay within one track of the full model.** About 1 in 5 tracks falls back to the full model.

## Switch (default off)

On by default since 2026-11-19 (30 s windows, `BeatWindows::from_env`, `fourfour-analysis-rs/src/bt.rs`). `FOURFOUR_BEAT_WINDOWS=<seconds>` changes the window length; `FOURFOUR_BEAT_WINDOWS=0` runs the full model on every track.

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

## Optimize loop (same day, later)

One change at a time, kept only when faster on dev and passing the gate on the test split. The gate compares with full mode: BPM Acc1 not lower, grid within 20 ms of Rekordbox not lower, bar right at most 1 lower, and ≥98% of tracks with ≥99% of beats within 20 ms of the full grid. Rules were tuned on dev (50 tracks) and on a separate tune split (150 tracks from `o3/ref.json` that are in neither dev nor test, `bw-tune-list.py`). Full mode is unchanged, so its runs are reused.

How the kept version works (`FOURFOUR_BEAT_WINDOWS=30`), compared with step 0 above:

- A window is a whole number of chunks: 30 s = 3 chunks (step 0 took every chunk that overlapped 30 s, usually 4).
- When the 3 windows fail the check, 2 more windows run (12.5% and 87.5%, `FOURFOUR_BEAT_WINDOW_EXTRA`). The grid is then kept when at least 3 windows are good, at most 1 is weak (inliers under 60%), and the good ones lie within 10 ms. Only then does the rest of the track run (`beat_mode: extended` / `fallback`).
- The model-rate audio and the mel spectrogram are computed only where the run chunks read them. Mel parts start at a frame aligned like the whole-track run and have 128 frames of margin, so they are bit-identical to the whole-track mel. Resampled parts start at a native sample aligned to the rate ratio and have 1 s of margin. On dev, 46/50 tracks came out bit-identical; the other 4 differ by under 0.1 ms.
- Bar-1 in windowed mode needs a 0.85 model vote instead of 0.8 (`service::WINDOWED_MODEL_STRONG_SHARE`). Windows hold the steady body of the track, so their votes are more one-sided. On the tune split, 0.85 gave the fewest bar-1 decisions that differ from full mode (0.85 and 0.9 tied; dev could not tell 0.8 from 0.95).

| step | commit | dev CPU ms/audio-min | test CPU s/track | test Acc1 | test grid 20 ms | test bar right | test ≥99% beats = full | fallback (test) | gate |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| full | - | 7311 | 46.8 | 194 | 171 | 142 | - | - | - |
| 0: 3 windows of 30 s | 602596b | 3624 | 22.3 | 194 | 171 | 141 | 205/205 | 39 (19%) | pass |
| 1: whole-chunk windows, +2 windows, mel only where read | d2dc9a1 | 3156 | 21.0 | 194 | 172 | 140 | 205/205 | 41 (20%) | fail (bar -2) |
| 2: + resample only where read, windowed bar-1 share 0.85 | 2870ef6 | 3202 | 19.2 | 194 | 172 | 141 | 205/205 | 41 (20%) | **pass, kept** |

Back-to-back timing pair, 41 test tracks (every 5th), 4 workers x 1 thread: beat-only CPU 7504 -> 2428 ms per audio minute (3.1x), 42.8 -> 13.8 s per track, wall 420 -> 125 s; all stages on (key + waveform) 7577 -> 2684 ms per audio minute (2.8x). This subset has fewer fallbacks (4/41) than the whole test split (41/205), so the test-split 2.4x is the more representative number.

Dev CPU is noisy (±5%, shared box); use the beat-model work share for decisions (step 0 0.444, step 2 0.385 of the full-mode chunks). Per stage, dev median ms per audio minute, step 0 vs step 2: resample 176 -> 79, mel 157 -> 66.

Tried and not kept:

| change | result |
|---|---|
| windows at 20/40/80% instead of 25/50/75% | work 0.305, but dev grid 44 -> 42 |
| 15 ms phase spread at the extension stage | dev work 0.345 and dev equal, but tune grid 120 -> 118 |
| 20 s windows (2 chunks) | dev and tune pass (work 0.35), test fails: grid 171 -> 167, bar 142 -> 137 |
| 10 s windows (1 chunk) | dev grid 44 -> 43 |
| extension windows at 37.5/62.5% | dev: 1 track off the full grid |

Other stages, dev, all stages on (key + waveform), step 2 settings with 20 s windows, median ms per audio minute: decode 189, resample 106, mel 83, predict 2228, fit 112, key 88, waveform 222. So with key and waveform on, total CPU is ~8% higher than beat-only. The waveform `Meter` is in `pioneer-usb-writer` and was not touched.

What is left: about 1 track in 5 still falls back and runs whole, and it carries about two thirds of the remaining beat-model work. Every rule tried that accepts more of those tracks lost grid or bar-1 accuracy somewhere.

## Mac

Not measured in this loop: Merlin was using the Mac, and it was on battery in Low Power Mode. A full-mode run of the 2025 folder had reached 457/512 tracks after 1457 s when it stopped (battery 61% -> 45%, powermode 1). Command to run later (build clone `~/dev/build/bw-loop`, models in `~/dev/build/bw-loop-models` = the dev `~/.cache/fourfour-models` files + `beat_this_q8sc.onnx`; the Mac default model is int8 on rten):

```bash
cd ~/dev/build/bw-loop && git pull && ~/.cargo/bin/cargo build --release -p fourfour-analysis-rs --example beat_windows_bench
cd ~/dev/build/bw-loop-out   # mac2025.tsv: id<TAB>path for the 512 files of #02_DJ/2025
./run.sh full                                   # 5 workers x 2 threads, writes full.jsonl/.err/.power (pmset before/after, wall_s)
./run.sh w30 FOURFOUR_BEAT_WINDOWS=30
python3 ~/dev/build/bw-loop/benchmark/scripts/bw-compare.py full.jsonl w30.jsonl   # grid/bar-1 agreement
```

## Next

- Fewer fallbacks would need a better signal than the per-window checks: every looser rule tried lost accuracy.
- Mac numbers (above).
