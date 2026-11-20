# hc-L3: CPU of the non-model analysis stages

Worker L3 of the 2026-11 hill climb (`hill-climb-plan-2026-11.md`). Branch `drain/hc-L3-20261120`. Output must stay bit-identical.

## Method

- Metric: instructions under `valgrind --tool=callgrind` (`perf stat` is not allowed here), example `stage_cpu`, `STAGE_REPS=1 STAGE_THREADS=1`, release build with `CARGO_PROFILE_RELEASE_DEBUG=1` (env only).
- Fixtures: six 30 s clips cut once with ffmpeg from `benchmark/datasets/rekordbox-testdata-2025/#02_DJ/2025` (no m4a or aiff exists in the datasets, so those two are re-encoded from the same material). Kept in `~/.cache/hc-L3-fixtures`.

| clip | source | codec |
|---|---|---|
| t1.mp3 | `Aug-Okt/2562/Burn.mp3`, from 30 s | mp3 192k 44.1 kHz stereo (stream copy) |
| t2.mp3 | `Feb-Apr/soothing set/Lightman - Nava.zeta.mp3`, from 60 s | mp3 128k 44.1 kHz stereo (stream copy) |
| t3.flac | `Nov-Jan/house/78. Dj Swap - Consequence (Original Mix).flac`, from 20 s | flac 44.1 kHz stereo |
| t4.wav | `Feb-Apr/3/U.N.I.T. Untitled.wav`, from 20 s | pcm s16 44.1 kHz stereo |
| t5.m4a | the flac above, from 40 s, resampled | aac 256k 48 kHz stereo (exercises resample44) |
| t6.aiff | the wav above, from 0 s | pcm s16 be 44.1 kHz stereo |

- Gates per kept step: `HASH` lines of `stage_cpu` equal to the baseline on all six clips (samples, key, r22, m44, plus new `wf` = PWV checksum and `wfcols` = raw band columns); full analysis JSON of the 50 dev tracks (`benchmark/results/hc/bw-splits/dev.tsv`, `analyze-json --threads 1 --workers 1`, keys sorted) byte-identical to the baseline; unit tests of the touched crate.
- Baseline commit: `test(analysis): stage_cpu hashes waveform columns` (adds `wf`/`wfcols` hashes and `STAGE_REPS`).

## Baseline

6,164,851,821 instructions over the six clips. By stage (inclusive, summed): resample22 2.75 G (all inside rubato in the `beat-this` crate, not ours), waveform `Meter::push` 1.56 G, decode 1.27 G (symphonia, almost all inside the codecs), key 0.157 G, waveform columns 0.04 G.

Instructions undervalue the Meter: it is a serial float recurrence (low IPC), the resampler is AVX (high IPC). Wall numbers are in the final A/B.

## Steps

| # | change | instructions before | after | saved | kept |
|---|---|---|---|---|---|
