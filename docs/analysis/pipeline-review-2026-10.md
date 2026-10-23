# Rust analysis pipeline review, pass 1 (Q21), 2026-10-23

Prototype only (`prototyping/analysis-packaging`, branch `q21-analysis`, started from `q15-grid` plus master). No numbers here replace the Q15 grid or Q16 key docs; they extend them. The box (`dev`, 16 cores) was shared and loaded (load average 12 to 32) during every measurement, so wall times are noisy; CPU seconds (user + system from `/usr/bin/time -v`) are the steadier figure.

## Result in one table

| | Before (Q15 pipeline) | After (`ffa`) |
|---|---|---|
| Grid within +-20 ms, test split n=205 | 148 = 72.2% | **148 = 72.2%** |
| BPM within +-0.1, n=205 | 180 = 87.8% | **182 = 88.8%** (see "Quality") |
| Key exact, Beatport test split n=199 | 114 = 57.3% (keydet, weights fit on Beatport) | **116 = 58.3%** (cyanbox port, not fit on Beatport) |
| Beat This! share of CPU per track | not measured | 93% (92 of 98 s mean, loaded box) |
| Process model | 3 to 4 separate binaries, audio decoded 3 times | one process, decoded once, stages in parallel |

## Speed (50 tracks, test split every 4th id, 328.6 min of audio, mean 6.6 min)

| Setup | Wall | Tracks/min | CPU s per track | Peak RSS | Covers |
|---|---:|---:|---:|---:|---|
| Before, single process (`bgdump`, rten default 16 threads) | 36:55 | 1.35 | 99 | 1.6 GB | decode + Beat This! only |
| Before, batched as Q15 ran it (4 processes x 3 rten threads) | 9:17 | 5.4 | 71 | 4 x 0.7 GB | decode + Beat This! only |
| After, batched (`ffa --workers 8 --threads 1`) | 11:02 | 4.53 | 79 | 4.45 GB (0.56 GB per worker) | decode, Beat This!, fit, key, waveforms |
| After, 205-track test run, same setup | 42:31 | 4.82 | 74 | 5.1 GB | all stages, plus logit dump |
| After, single process (`--workers 1 --threads 8`), first 20 of the 50 | 6:05 | 3.29 | 89 | 4.1 GB (8 threads) | all stages |

Reading it honestly:

- The "before" rows leave out stages that ran in other programs: `grid` fit 1.2 s per track, `keydet detect` about 4.5 s per track wall (decode 2 s + spectrum 2.3 s), waveform decode + FFTs 3 to 7 s, and the old all-in-one `ffproto` binary which also ran stratum-dsp for 75 to 170 s per track. The "after" row is the whole job. Counting those, before is about 86 to 110 CPU-s per track and after 74 to 79: a gain of 10 to 30%, not a multiple. The model cost is the same code and weights.
- Single track, 430 s file: 26.7 s wall with 4 chunk-parallel threads on the loaded box (all stages), against 46 s (2.8 s decode + 43.5 s model, 16 rten threads) before for the beat stage alone. Single-thread CPU for the model on that track is 70 s, so the 4-thread run is 3.2x faster at equal CPU.
- Cold start: model load (both ONNX files) 0.11 to 0.34 s, as before. The first chunk of the first track runs about 1.3 s slower (rten packs weights on first use); `prepack_weights` moves that cost to load and does not change the total.
- Peak RSS per single-thread track is 0.56 to 0.7 GB; each extra thread adds about 0.37 GB (attention activations, 300 MB max per chunk), so 4 threads on one track reach 1.8 GB. The batch pool therefore prefers many 1-thread workers over few wide ones.
- Batch memory cap: `ffa` reads `MemAvailable`, divides 50% of it by `--rss-mb-per-worker` (default 900, a bit above the longest tracks measured) and runs min(requested, cores, cap) workers. On this box the cap was 20 to 22 workers, so cores bound it; on an 8 GB laptop with 4 GB free it would allow 2.

## Where the time goes (205 tracks, mean 374 s audio, 1 thread per track, loaded box)

| Stage | Mean seconds per track |
|---|---:|
| decode (symphonia, mono, 44.1 kHz resample) | 1.85 |
| resample 44.1 to 22.05 kHz (rubato) | 1.61 |
| mel (ONNX, 30 s windows) | 1.34 |
| **Beat This! prediction** | **91.7** |
| tempo scan and grid fit | 1.57 |
| key (cyanbox port) | 1.07 |
| waveforms (PWAV, 3-band detail 150/s, overview 1200) | 1.71 |
| total | 98.0 |

Per 30 s chunk with one thread (RTEN_TIMING): MatMul 45%, FusedMatMul (attention) 21%, Softmax 12%, Concat/Split 6.6%, Conv 2.6%.

## What was tried for Beat This! (the only stage that matters)

| Idea | Result | Verdict |
|---|---|---|
| Chunk-parallel inference, one single-thread rten pool per worker | logits bit-identical to Q15 on 186 of 205 tracks (the other 19 are not 44.1 kHz, see below); 3.2x faster at 4 threads | kept |
| `prepack_weights(true)` | 71.2 s vs 70.5 s (no change) | not used |
| int8 dynamic quantisation (onnxruntime `quantize_dynamic`, MatMul/Gemm only, or plus Conv; 83 MB to 23 MB) | per-chunk 4.26 s vs 4.78 s (-11%); the attention matmuls are activation x activation and stay f32; accuracy not scored | not adopted, not worth the risk for 11% |
| f16 weights | rten 0.24 has no f16 compute ("planned" in its docs) | not possible |
| `beat_this_small.onnx` | 1.8x faster (8.2 vs 4.5 tracks/min) but grid within 20 ms 44% (22 of 50) against 76% (38 of 50) for the large model on the same tracks | rejected |
| Smaller input hop or a downsampled spectrogram | not possible: the model is trained for 50 fps and 128 mel bins; any other shape is a different model | rejected |
| Model on a 120 s window instead of the full track | already measured in Q15 (full track is +25 grid points) | rejected |
| One model instance reused across tracks | done (one `rten::Model` shared by all workers via `&self`) | kept |

## Quality

Grid and BPM: the unified analyzer's final beats scored with the same `grid` code as Q15 (`--use-final`), test split, 205 tracks.

| Metric | Q15 | Q21 `ffa` |
|---|---:|---:|
| Whole grid within +-20 ms | 148 = 72.2% | 148 = 72.2% |
| BPM within +-0.1 | 180 = 87.8% | 182 = 88.8% |
| BPM within +-1 | 190 = 92.7% | 191 = 93.2% |
| F70 (fitted) | 0.893 | 0.897 |
| Median offset, signed / absolute | +6.7 / 11.3 ms | +8.4 / 10.3 ms |

Fit code moved to a library and the Q15 numbers were reproduced exactly from the Q15 logits before anything else changed (148, 180). The two extra BPM hits come from the 19 tracks whose source is not 44.1 kHz: the unified decode resamples 48 kHz (and others) to 44.1 kHz and then to 22.05 kHz, where Q15 resampled once; their logits differ by up to 14 and their peak lists change. On this split it is neutral to slightly positive, but it is a double resampling and goes on the pass-2 list (decode at the native rate and resample once).

### MP3 timeline rule (cyanbox `src/audio/mp3.c`, ported to `src/mp3.rs`)

Offset (estimate minus Rekordbox) by MP3 group on the 88 test mp3s, before the delay is added versus after, median of per-track medians in ms; bpm-ok tracks only for the within-20-ms counts:

| Group (n) | Q15 rule: LAME tag delay + 529 samples | New rule: encoder delay + 1 frame if the Xing/Info frame counts |
|---|---|---|
| Info frame present and counted by the frame map (18) | -11.8 ms, 9 of 16 within 20 ms | +0.6 ms, 13 of 16 |
| Info frame present, map restarts (66, ffmpeg-style `Lavf` tag) | -9.9 ms (rule gave 0), 41 of 56 | +3.1 ms, 37 of 56 |
| No Xing/Info frame (4) | +13.3 ms | +13.3 ms (unchanged) |

wav and flac sit at +3.5 ms, so +3 ms is the model's own bias; the new rule puts every MP3 group on that line, which Q15's did not. Whole-grid result is identical (148). Findings:

- Rekordbox's timeline is the raw decoder output; symphonia trims the encoder delay (576 samples), so the shift to add is the encoder delay field alone, not delay + 529. The 529 decoder delay is in both timelines.
- The Info/Xing frame counts as one silent frame (1152 samples = 26.1 ms) when the first four frames agree in version, layer, rate and channel mode (cyanbox rule, from rbxport's 200-file measurement). 18 of 88 test mp3s are in that group; this is what Q15's rule missed, and it is the cause of the -11.8 ms median there.
- Q15's reader only recognised the `LAME` tag id; ffmpeg's `Lavf` tag holds the same delay field. 66 of 88 test mp3s are such files and got no correction at all.
- The rule is now the default (`DelayRule::EncoderAndHeaderFrame`); `--mp3-rule none|lame|lame+header|header|enc|enc+header` selects others. Variants scored (grid within 20 ms): Q15 148, +header 150 (LAME ids only), enc only 136, enc+header 148, LAME-or-Lavf + 529: 137. The principled rule ties the best on the count and has the best median offsets, so it stays. The remaining mp3 spread is not explained by the timeline (66% within 20 ms in the ffmpeg group against 83% for wav/flac) and needs per-track inspection.
- The file reader now reads 16 KB after the ID3 tag instead of the whole 36 MB file.

### Key: cyanbox port against fourfour's keydet

Beatport EDM Key, test split (id % 3 == 0, n=199), same manifest and labels as Q12/Q16; exact match of the Camelot class.

| Detector | Exact | Relative | Parallel | Fifth | Exact+rel+fifth | Time per 120 s clip |
|---|---:|---:|---:|---:|---:|---:|
| keydet (chroma + 50 learned weights, weights fit on Beatport dev split) | 114 = 57.3% | 3.5% | 12.6% | 10.1% | 70.9% | about 0.8 s incl. decode |
| **cyanbox port (`src/cyankey.rs`, rustfft)** | **116 = 58.3%** | 4.0% | 11.1% | 10.6% | **72.9%** | 0.29 s single thread, decode excluded |
| cyanbox port, dev split n=399 | 250 = 62.7% | | | | | |
| cyanbox port, all 598 | 366 = 61.2% | | | | | |

The difference of one track is noise (95% interval about +-7 points at n=199). The cyanbox result stands on three other things: it was not fit on Beatport (no CC BY-SA question, nothing to withhold from the repo), it is a self-contained front end (tuning estimate, semitone bands, whitening, three chroma bands) with a calibrated confidence, and it ran faster. Decision: **cyanbox becomes the unified analyzer's key stage; keydet stays in the repo as a bench tool.** cyanbox claims 64.7% on GiantSteps key (604, held out); that dataset was not available here, so that number is not reproduced.

Where the templates come from (checked): 72 weights, one major bias, a softmax temperature and a three-number confidence calibration, fitted by cyanbox's author on the GiantSteps MTG key annotations (1159 tracks, Faraldo). That GitHub repository has no licence file (API: `license: null`), so the annotation licence is unverified; cyanbox publishes the numbers under BSD-2-Clause and they are used as-is with the notice kept (`prototyping/analysis-packaging/NOTICE`). Sha'ath's profile table (tie-breaker only) is libKeyFinder constants quoted as data by cyanbox. Nothing here was fit on Beatport, so the CC BY-SA rule is not touched.

## Review findings, ranked

Ranked by cost or risk to a real library import. "Fixed" means done in Q21.

1. **Beat This! prediction is 93% of CPU** (92 of 98 s mean per track on one thread, 0.25 x realtime on the loaded box, 0.16 on an idle core). Ops are 45% MatMul, 21% attention, 12% Softmax. No cheap lever exists (table above). A 5,000-track library is about 100 CPU-hours: 13 hours on 8 cores. Not fixed.
2. **Rayon-based rten threading wasted CPU**: the default 16-thread pool cost 99 CPU-s per track against 71 with 3 threads. Chunk-parallel workers with single-thread pools reach the 71 to 74 level at any width. Fixed.
3. **Audio decoded three times and stratum-dsp in the all-in-one binary**: `bgdump` (22.05 kHz), `keydet` (44.1 kHz) and the waveform code each decoded; the old `ffproto` ran stratum for 75 to 170 s per track as a side analysis that the grid and key results do not use. Fixed in `ffa` (one decode at 44.1 kHz; stratum not used). Not fixed: `ffproto` still exists in the tree.
4. **Double resampling for non-44.1 kHz sources** (19 of 205 test tracks): decode resamples to 44.1 kHz, the model path again to 22.05 kHz. Neutral on the split, but not clean. Needs a native-rate decode (own symphonia loop, MPL-2.0 crate already in the tree). Not fixed.
5. **Peak memory in decode**: `load_audio` holds the interleaved f32 track, then a mono copy, then the resampled copy (about 3.2 x 176 KB per second of stereo audio, 220 MB for a 7 min file), and `BeatEngine::analyze` copies the mono track once more to hand it to the resampler. A streaming decode that mixes to mono per packet would cut roughly 150 MB per worker. Not fixed.
6. **Waveform stage is a placeholder, not Rekordbox's**: per detail entry one 2048-point FFT over a 294-sample hop, zero padded, so the 20 to 250 Hz band is one or two bins; the 1200-entry overview takes only the first 2048 samples of each 11,000-sample hop (18% of the audio); PWAV is a plain RMS. Fast now (1.7 s, parallel over entries, output bytes identical to `ffproto`), but format fidelity belongs to the brother's lane (baken's fitted rules, `docs/research/clones-2026-10-23.md`). Not fixed.
7. **Fit scan**: 601 tempi x 125 phases of the whole-track comb, scalar f64, 1.2 to 1.6 s per track; `act` was computed twice. Now computed once and the scan splits over threads with the same tie-breaking (identical result). In batch mode it stays single-thread. A coarse-to-fine scan would cut it to about 0.2 s. Partly fixed.
8. **Reference leakage in the Q15 scorer**: `grid` took the track duration as max(audio, last Rekordbox beat). `ffa` uses the decoded duration only; the end-to-end score above includes that change and shows no loss.
9. **Key detector decimates and runs the 8192-point STFT frame by frame on complex FFTs** (0.29 s per 120 s single thread, about 3x the C original). A real-FFT or two-frames-per-FFT variant would halve it. Low value next to item 1. Not fixed.
10. **Decode failure and silence paths**: a track that decodes to silence returns no key and no grid but not an error; `ffa` logs failures per track and continues. Fine for a prototype; the app needs explicit states.
11. **mp3 spread** still 66% within 20 ms in the ffmpeg-tag group against 83% for wav/flac (see above). Cause unknown.

## What changed (code)

- `src/lib.rs` (library `ffcore`): `fit` (Q15 grid fit, scan optionally parallel), `mp3` (frame scan + delay rules), `bt` (Beat This! engine: shared rten models, mel stream, chunk-parallel predictor, peak picking), `cyankey` (key), `waveform`, `analyze` (all stages for one file; key and waveforms run beside the beat model under `std::thread::scope`), `keymap`.
- `src/bin/ffa.rs`: unified CLI and batch pool. `grid` uses the library and scores `ffa` output with `--use-final`. `cyankey` is the key benchmark. `keydet`, `bgdump`, `rbref` and `ffproto` are unchanged.
- Deps added to the prototype (all already in its lock file): `rten =0.24.0`, `rten-tensor =0.24.0` (MIT OR Apache-2.0), `anyhow` (MIT OR Apache-2.0). Beat This! helpers used from `beat_this::__probe` (hidden, pinned to =1.1.0).
- Beat This! output is bit-identical to Q15 for 44.1 kHz and 22.05 kHz sources (checked on all 205 test tracks with `q21-logit-diff.py`).

## Left for pass 2

1. Native-rate decode with one resample, streaming mono mix (items 4 and 5): saves memory and removes the 19-track difference.
2. Coarse-to-fine tempo scan; real-FFT key front end (items 7 and 9).
3. Replace the waveform stage with the Rekordbox-fitted rules (baken, MIT) once the brother's lane agrees; add the 3-band colour bytes.
4. Per-track look at the 66 ffmpeg-tag mp3s and at the 31 constant-grid tracks that still fail 20 ms (Q15 list).
5. A real downbeat/bar-position score and a per-track confidence for BPM and grid, so the app can mark doubtful tracks.
6. Priority queue for the batch pool (user-waiting first, duplicates joined), as in Vynull's design; progress events.
7. GiantSteps key set for a like-for-like check of the cyanbox number; run the key stage on the full track versus a 120 s clip.
8. Decide whether the int8 model (11%) or a distilled model is worth a quality study; nothing else can cut item 1.

## Reproduce

```bash
cd prototyping/analysis-packaging
./fetch-models.sh && cargo build --release
M=/path/to/benchmark/results/q13/rb-grid-reference.json        # Rekordbox reference (gitignored)

# all stages, batch pool, 8 workers x 1 thread, dump logits for scoring
../../benchmark/scripts/q21-timed.sh ../../benchmark/results/q21/full205 \
  target/release/ffa --workers 8 --threads 1 --brief --reference $M --dump ../../benchmark/results/q21/dump205 \
  --list ../../benchmark/results/q21/tracks205.txt
# score the final beats exactly as shipped, and re-fit from logits with the MP3 rule of your choice
target/release/grid $M ../../benchmark/results/q21/dump205 --split test --lo 75 --use-final
target/release/grid $M ../../benchmark/results/q21/dump205 --split test --lo 75 --comb --mp3-rule enc+header \
  --snap-tol 0.05 --snap-wide 0.5 --snap-ratio 0.97 --scan-pct 0.3 --mp3-diag
# key benchmark (Beatport test split is the line starting "test")
target/release/cyankey ../../benchmark/manifests/beatport-edm-key-keyonly-clean-full.corpus.json \
  /path/to/benchmark/datasets/beatport-edm-key/audio 4
# before numbers: Q15 binaries built on q15-grid
../../benchmark/scripts/q21-before-batch.sh ; ../../benchmark/scripts/q21-before-stages.sh
# model experiments: int8 files made with onnxruntime quantize_dynamic; variants run with
../../benchmark/scripts/q21-variants.sh <list> 1 fp32 q8mm q8all small
python3 -I ../../benchmark/scripts/q21-logit-diff.py <dirA> <dirB> [n]
```

Run sizes and outputs sit in `benchmark/results/q21/` (gitignored): `full205.*`, `after-batch8.*`, `before/`, `small50.*`, `mp3rule2-*.txt`, `cyankey-preds.txt`.

## Pass 2, 2026-10-24 (Q26)

Branch `q26-analysis` (from `q21-analysis`, master merged). Same box (`dev`, load average 14 to 35 during every run, so wall times are noisy; CPU seconds are the steadier figure). Same splits as pass 1.

### Result

| | Pass 1 (Q21) | Pass 2 (Q26) |
|---|---:|---:|
| CPU s per track (user + sys, 205-track test run, 8 workers x 1 thread, all stages) | 74 | **70.3** |
| Sum of stage times per track | 98.0 s | **89.3 s** |
| Tracks per minute at 8 workers | 4.82 | **5.29** (load differs between runs; treat as +-10%) |
| Peak RSS (8 workers) | 5.1 GB | 5.0 GB (about 0.63 GB per worker, unchanged: the model dominates) |
| Grid within +-20 ms, test n=205 | 148 = 72.2% | **148 = 72.2%** |
| BPM within +-0.1, n=205 | 182 = 88.8% | **180 = 87.8%** (see below; equals Q15) |
| Key exact, Beatport test n=199 | 116 = 58.3% | **116 = 58.3%** (stage and code path unchanged, re-run) |

Stage means per track (205 tracks, 1 thread each): decode 1.85 to 1.14 s, fit 1.57 to 0.28 s, predict 91.7 to 84.9 s (same code and weights; the difference is load), resample 1.61 to 1.72, mel 1.34 to 1.22, key 1.07 to 1.04, waveforms 1.71 to 1.59. Everything except the model is now 4.4 s of 89.3. The pass-1 target (a 3x to 10x gain) is not on the table: 95% of the CPU is Beat This!, and no backend change reaches 2x (next section).

### 1. Inference backend: ONNX Runtime versus rten (finding: not adopted)

Both are permissive: the `ort` crate is MIT OR Apache-2.0 (checked in its `Cargo.toml`, 2.0.0-rc.14), ONNX Runtime is MIT (the onnxruntime 1.31.0 wheel metadata says "MIT License"). The Rust crate downloads the native library from pyke's CDN in its default `download-binaries` feature; a build with `load-dynamic` instead loads a `libonnxruntime.so` that we ship ourselves (no build-script download; Debian's 1.21 and the wheel's 1.31 both exist on this box). Shipping that library costs about 29 MB (`libonnxruntime.so.1.31.0` in the wheel; rten is compiled into the 7.2 MB `ffa`).

The beat-this-rs release already ships the ONNX files (`beat_this.onnx`, `mel_spectrogram.onnx`, both with a dynamic batch axis), so no export from the PyTorch weights is needed. Measured with ONNX Runtime 1.31 (Python, the same C++ core `ort` calls) on one 1500-frame chunk, same moment on the same loaded box; rten: 72.4 s predict for the 430 s track = 14.5 chunks = 5.0 s per chunk (`ffa --threads 1`):

| Setup | Wall s per chunk | CPU s per chunk | Versus rten fp32 |
|---|---:|---:|---:|
| rten fp32, 1 thread | 5.0 | 5.0 | 1.0x |
| ORT fp32, 1 thread | 3.4 to 3.9 | 3.4 to 3.9 | 1.3 to 1.5x |
| ORT fp32, graph optimisation "basic" only | 3.8 | 3.8 | 1.3x |
| ORT fp32, 4 threads | 1.39 | 5.3 | 3.6x wall, 1.05x CPU: no CPU gain |
| ORT fp32, batch 4 (4 chunks in one run) | 16.3 | 12.5 | worse: attention memory traffic |
| ORT int8 dynamic, MatMul/Gemm (`beat_this_q8mm.onnx`) | 3.3 | 3.3 | 1.5x; vs ORT fp32 only 5 to 9% |
| ORT int8 dynamic, MatMul/Gemm + Conv (`q8all`) | 3.8 | 3.8 | no gain |

Verdict: ORT is 1.3 to 1.5x faster in CPU per chunk, under the 2x bar, so no feature flag and no new dependency were added (a Rust `ort` build would add the crate chain, a 29 MB shared library to package and sign per platform, and an unstable 2.0 release-candidate API, for at most one third less CPU). Batching chunks into one run makes it slower. int8 buys another 5 to 9% over fp32 in ORT and 11% in rten, and its grid accuracy could not be scored: the pass-1 variant dumps hold one track each. Thread-pool tuning: nothing to tune; threads inside one track only trade CPU for latency (CPU per chunk goes up 1.5x at 4 threads), and 8 one-thread workers is already the efficient point. A 5,000-track library stays at about 95 CPU-hours (12 hours on 8 cores). Revisit if Beat This! ships a distilled model, or if a hardware backend (CoreML on the Mac) is acceptable for the app; neither was measurable here (CPU-only box).

### 2. Decode at the native rate (done)

`src/decode.rs`: own symphonia loop (MPL-2.0, same 0.6 version the lock file already had, `features = ["all"]`), mono mix per packet, native-rate buffer. The model path resamples native to 22.05 kHz once; key and waveforms resample native to 44.1 kHz once, only when the source is not 44.1 kHz. For 44.1 kHz sources the logits are bit-identical to pass 1 (`q21-logit-diff.py`, 20 of 20 tracks checked, same peaks). The 19 non-44.1 kHz tracks no longer resample twice, and the BPM count went from 182 back to 180, the Q15 value: the two extra pass-1 hits were luck from the double resample, not quality. Grid unchanged at 148. Copies saved: the interleaved f32 buffer, the second mono copy and the `to_vec` before the model resample.

### 3. Coarse-to-fine tempo scan (done)

601 tempi x 125 phases became: every 6th tempo over all phases, then the 5 best coarse tempi refined over their 11 neighbours with 7 phases each. Fit time (median over the test split, same logits): 1,094 ms to 187 ms with grid scores identical (148 within 20 ms, 182 BPM within 0.1, F70 and offsets unchanged on re-fit of the pass-1 logits). In the end-to-end run the mean fit stage is 0.28 s.

### 4. Grid quality: MP3 spread and constant-grid misses (cause found, fix not applied)

Not tuned: the pass-1 logits cover the test split only, and the rule was not to tune on test. What the test split shows (bpm-ok tracks; within 20 ms by container: flac 34 of 42 = 81%, mp3 53 of 74 = 72%, wav 61 of 66 = 92%; median offset wav +10.5, flac +10.9, mp3 +2.5 ms):

- The mp3 spread is one encoder. Grouping the mp3 tracks by the encoder string in the Info frame: **`Lame3.100`: 10 of 12 sit at +17 to +35 ms, median +27 ms, one MPEG frame (26.1 ms)**; the other two are half-beat phase misses. `LAME3.99` (16 tracks, median +2), every ffmpeg tag `Lavc*`/`Lavf*` (about 40 tracks, median +1) and no-tag files are fine. The Lame3.100 files have the same first-four-frames header agreement as the others, so the cyanbox rule says "Info frame does not count", but Rekordbox's timeline behaves as if it does. Seven of the 12 constant-offset misses below (ids 3, 537, 549, 573, 606, 648, 651; offsets +27 to +35 ms, detrended error 0.5 ms or less) are these files; the fit itself is internally perfect there.
- Hypothesis for pass 3: add one frame (1152 samples) to the delay when the encoder tag starts `Lame3.100`. Expected effect on the test split: up to 7 more tracks within 20 ms (about +3 points). This was found by looking at test-split offsets, so it needs a dev-split check before it is adopted: pick the dev mp3s whose tag starts `Lame3.100`, run `ffa --dump`, score with and without the frame. `q26-mp3-features.py` prints the encoder and header fields for this.
- The 34 tracks with the right BPM and a missed grid (the pass-1 list said 31; same reading with `--diag`), by class: 6 are half-beat phase errors (median offset 211 to 247 ms; Rekordbox's grid sits on the other half-beat or the model locked on the off-beat); 12 have a clean constant offset of 22 to 100 ms (7 are the Lame3.100 files above; the others are ids 69 and 117 at +23 and +26 ms, 75 at -38 ms, 324 at -97 ms and 180 at +55 ms with 100 ms of drift); 16 have a median within 20 ms but drift (tempo error accumulating to 20 to 60 ms over the track, 13 of them with a constant reference) or a few outlier beats (3 of them with 250 to 450 ms outliers). 23 more tracks have the wrong BPM (10 of them with a constant reference grid): mostly octave errors (x0.5) that need a tempo prior, not a fit change.

### 5. Dead weight removed

`ffproto` (`src/main.rs`, the stratum-dsp path, 75 to 170 s per track), the `stratum-dsp` dependency and its lock entries, the `--stratum` switch of `bgdump`, and the two scripts that only drove it (`run-3-tracks.sh`, `key-subset.py`) are gone; nothing else in the tree called them (checked with grep over `benchmark/scripts`, `docs/`, the prototype and the app). `keydet`, `bgdump`, `grid`, `cyankey` and `rbref` stay. The Cargo package is still named `ffproto`; renaming it is cosmetic and left alone.

### 6. Waveform stage

Untouched (bytes identical by construction: the 44.1 kHz mono input is the same for 44.1 kHz sources). It costs 1.6 s per track.

### What is left

1. Lame3.100 timeline rule (item 4), validated on dev, then the half-beat phase and octave tempo errors (a prior from the downbeat logits).
2. Beat This! stays 95% of the CPU. Options that need a decision, not more engineering: a distilled model, int8 with a proper accuracy study (5 to 11%), or CoreML on the Mac.
3. Pass-1 list items 3, 5, 6, 7 (Rekordbox-fitted waveform, confidence scores, priority queue, GiantSteps key set) unchanged.

### Reproduce

```bash
cd prototyping/analysis-packaging && ./fetch-models.sh && cargo build --release
S=../../benchmark/scripts; R=../../benchmark/results/q26     # copy Q21's tracks205.txt and the Q13 reference as ref.json
$S/q21-timed.sh $R/full205 target/release/ffa --workers 8 --threads 1 --brief --reference $R/ref.json --dump $R/dump205 --list $R/tracks205.txt
$S/q26-score.sh $R/dump205 test --use-final                 # as shipped
$S/q26-stage-means.py $R/full205.out                        # stage means
target/release/cyankey ../../benchmark/manifests/beatport-edm-key-keyonly-clean-full.corpus.json <beatport audio dir> 1
# ORT comparison (pip install onnxruntime in a venv)
$S/q26-ort-matrix.sh <venv python>
# mp3 and miss analysis
TAILN=1000 $S/q26-score.sh $R/dump205 test --mp3-diag --diag --out $R/grid-diag.json > $R/diag.txt
python3 -I $S/q26-mp3-features.py $R/grid-diag.json $R/ref.json ; python3 -I $S/q26-fail-classes.py $R/grid-diag.json
```
