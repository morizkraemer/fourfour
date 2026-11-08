# Permissive key detector (Q12), 2026-10

Status: prototype in Rust, not wired into the app. Bar for the beta: at least 60% exact on Beatport labels. Result: **57.3% exact on a held-out test split of 199 tracks; 60.9% exact in 5-fold cross-validation over all 598 tracks.** On the test split the bar is not clearly reached (the 95% interval is roughly +-7 points); the 598-track figure reaches it, but its hyper-parameters were tuned on the same tracks' 399-track subset, so treat it as optimistic.

## Data and labels

- Corpus: `benchmark/manifests/beatport-edm-key-keyonly-clean-full.corpus.json`, 598 tracks, 120 s Beatport preview clips, single `X major` / `X minor` Beatport label converted to Camelot. Audio read-only from `benchmark/datasets/beatport-edm-key/audio`. Beatport labels are external ground truth, so these are accuracies.
- Split (fixed, deterministic): test = manifest `id % 3 == 0` (199 tracks), dev/train = the other 399. All tuning (front-end parameters, regularisation) used 5-fold CV inside the 399 only. The test split was scored once with the final settings (a second time identically when the weights were exported). No result below is tuned on test.
- Possible leak: the same track can appear in several versions; duplicates were not removed.

## Results

Relations (all denominators are the row's `n`): relative = same Camelot number, other letter; parallel = same tonic, other mode; fifth = Camelot neighbour (same mode, +-7 semitones tonic); E+R+F = exact + relative + fifth.

| System | Eval set | n | Exact | Relative | Parallel | Fifth | E+R+F |
|---|---|---:|---:|---:|---:|---:|---:|
| stratum-dsp 1.0.0 (default config) | all | 598 | 26.8% | 4.7% | 21.2% | 18.6% | 50.0% |
| stratum-dsp | test split | 199 | 25.6% | 3.5% | 25.6% | 22.6% | 51.8% |
| Essentia `bgate` (reference only, AGPL; from `benchmark-2026-10.md`) | all | 598 | 54.0% | 5.5% | 16.7% | 9.4% | 68.9% |
| Rust chroma + Temperley template (no training) | all | 598 | 34.4% | 8.9% | 13.4% | 13.5% | 56.9% |
| Rust chroma + Temperley template | test split | 199 | 32.7% | 9.5% | 14.1% | 13.6% | 55.8% |
| **Rust chroma + learned profile (this work)** | **test split (held out)** | 199 | **57.3%** | 3.5% | 12.6% | 9.5% | **70.4%** |
| same, 5-fold CV on dev only | dev | 399 | 63.7% | 4.5% | 8.0% | 11.3% | 79.4% |
| same, 5-fold out-of-fold over all | all | 598 | 60.9% | 4.3% | 10.4% | 10.5% | 75.8% |

Essentia `bgate` has no held-out split in the benchmark doc (it is untrained on this set); its test-split numbers were not computed. Krumhansl profile with the same front end: 32.1% exact (worse than Temperley). The 598-track CV row uses models that never saw the scored track, but hyper-parameters were picked on the dev 399 (which are 2/3 of those 598). The shipped weights (`weights.rs`) are trained on all 598 and therefore cannot be scored honestly; the test-split row uses the train-only fit.

Mode confusion (relative + parallel + other swap): 22.6% on test (Essentia `bgate`: about 33%).

## Method

1. Decode (symphonia via `beat-this`), mono, FIR low-pass and decimate to 11.025 kHz.
2. STFT 8192 / hop 2048, Hann; magnitudes mapped to a log-frequency axis, 3 bins per semitone, MIDI 24 to 96.
3. Per frame: `ln(1 + 10 * mag / track mean)`, harmonic summation (8 harmonics, weight 0.9^(h-1)) so overtones support their fundamental, spectral whitening (subtract +-6 semitone local mean, clamp at 0).
4. Tuning: the 1/3-semitone grid phase with the most energy is chosen per track; fold into 12 pitch classes in two bands, bass (MIDI 28 to 51) and mid (48 to 89), per-frame L2 normalised, summed over the track.
5. Classifier: transposition-equivariant multinomial logistic regression. One 2x12 weight vector per mode (24 weights per mode over the two bands) plus a mode bias, shared across all 12 tonics. 50 numbers in total. Adam, 300 epochs, L2 = 0.001, trained on track-level vectors.

Ablations on dev CV (5-fold within 399, exact): no harmonic summation 59.4%; with 4 harmonics 61.7%; 6 harmonics 62.2%; 8 harmonics (chosen) 63.7%; no whitening 42.1%; HPSS (median filter, time 9 / freq 45) 62.7% vs 62.2%, no gain and about 10x the feature cost, so off; per-segment (8 s) training and voting no gain; stronger regularisation (L2 0.1) 44.6%.

Code: `prototyping/analysis-packaging/src/bin/keydet/`. No new crates: it uses `beat-this` (decode), `rustfft`, `serde_json` already in the workspace.

## Runtime

`keydet detect` on 120 s clips, single thread, box load average about 7 of 16 cores: decode 350 ms, spectrum 450 ms, classify 5 ms, about 0.8 s per track (0.007x realtime). Essentia `bgate` measured 1.17 s per track in the Q9 run (Python process). stratum-dsp key came from the full `analyze_audio` and cost 11.6 s per track here under load (4 parallel, load average about 14), not comparable. Feature extraction for the experiments cached spectra at about 1.4 s per track (4 threads, loaded box, before a FIR speed-up).

## Licences (verified)

| Piece | Licence | Source checked |
|---|---|---|
| keydet code | MIT OR Apache-2.0 (workspace `Cargo.toml`), own code | this repo |
| stratum-dsp 1.0.0 (baseline only) | MIT OR Apache-2.0 | crate `Cargo.toml` in the registry |
| beat-this 1.1.0 (decoder only) | MIT | crate `Cargo.toml` |
| rustfft 6.4.1, serde_json, rten 0.24 | MIT OR Apache-2.0 | `cargo metadata` |
| symphonia 0.5.5 / 0.6.1 (decoder, via beat-this) | MPL-2.0 | `cargo metadata` |
| rubato 3.0.0 | MIT | `cargo metadata` |
| full dependency tree | 60 MIT OR Apache-2.0, 32 MPL-2.0 (symphonia family), 6 MIT, others MIT/Apache/Zlib/Unlicense; no GPL/AGPL | `cargo metadata --offline` |
| Temperley and Krumhansl-Kessler profile numbers (template baseline only, not shipped) | published values from the literature (Temperley 2005, Krumhansl 1990); no code taken | not verifiable as a licence; they are numeric constants from papers |
| Learned weights (`weights.rs`, 50 floats) | own work, fit by `keydet export` | see below |
| Training labels / audio: Beatport EDM Key dataset | CC BY-SA 4.0 | Zenodo API record 1101082, `metadata.license.id = cc-by-sa-4.0` |

Open point for Merlin: the weights are a 50-number statistical summary fit on CC BY-SA 4.0 data (labels and audio features). Whether that makes the weights a derivative needing share-alike is a legal question I did not resolve; the audio is not redistributed. Essentia `bgate`/EDMA profile values were deliberately not copied (they live in AGPL source); the learned profile replaces them.

## Reproduce

```bash
cd prototyping/analysis-packaging
cargo build --release --bin keydet
M=../../benchmark/manifests/beatport-edm-key-keyonly-clean-full.corpus.json
./target/release/keydet extract $M /home/merlin/dev/projects/fourfour/benchmark/datasets/beatport-edm-key/audio ../../benchmark/results/keydet-spec threads=4
./keydet-eval.sh method=linear split=test          # held-out 199
./keydet-eval.sh method=linear split=cvall         # 5-fold OOF over 598
./keydet-eval.sh method=linear split=cv            # dev CV (399)
./keydet-eval.sh method=template split=test profile=temperley
./target/release/keydet export $M ../../benchmark/results/keydet-spec src/bin/keydet/weights.rs split=all   # regenerate shipped weights
./target/release/keydet detect some.mp3
python3 key-subset.py 1 4                          # stratum on all 598 (about 20 min loaded)
python3 ../../benchmark/scripts/score_key_rows.py results/key-subset.json test
```

`keydet-eval.sh method=shipped split=test` scores the embedded weights; after `export split=all` that number is contaminated (trained on test). To reproduce the 57.3% run `export ... split=train` first. `keydet-sweep.sh <file>` runs one eval config per line.

## Next steps (if more accuracy is wanted)

- The test split is small (199, +-7 points). Re-score on a fresh Beatport-labelled set, or use a nested CV, before claiming 60%.
- More training data is the likely lever (not measured here). Other label sets (GiantSteps key, MTG) have their own licences, check before use.
- Richer small model: circular-convolution CNN or MLP over per-segment chroma plus a bass-note feature; mode confusion (parallel 12.6%) is still the biggest error.
- Check label noise: Beatport labels disagree with ear/Rekordbox on many tracks (see `benchmark-2026-10.md`).

## Q16, 2026-10-23

Goal: at least 60% exact on the held-out test split (manifest `id % 3 == 0`). Result: **not reached. Best is 57.3% exact, 70.9% exact+relative+fifth on the same 199 tracks; round 1 was 57.3% and 70.4%.** The split, denominators and labels are unchanged from round 1. Tuning used 5-fold CV inside the 399 dev tracks only; the test split was scored once with the final settings (and a second time by reloading the saved weights, identical).

### Test split (n = 199, held out)

| System | Exact | Relative | Parallel | Fifth | E+R+F |
|---|---:|---:|---:|---:|---:|
| Round 1: chroma + learned profile (50 weights) | 57.3% | 3.5% | 12.6% | 9.5% | 70.4% |
| **Q16 final: same 2 channels, 3-seed ensemble, segment-pool augmentation** | **57.3%** | 3.5% | 12.6% | 10.1% | **70.9%** |
| stratum-dsp 1.0.0 | 25.6% | 3.5% | 25.6% | 22.6% | 51.8% |
| Essentia `bgate` (reference, AGPL) | not computed on the test split; all 598: 54.0% exact, 5.5% relative, 16.7% parallel, 9.4% fifth, 68.9% E+R+F | | | | |

One more test track lands on fifth, none on exact: the difference is noise (the 95% interval is about +-7 points at n = 199).

### What was tried (dev 5-fold CV, n = 399, exact; round-1 baseline 63.7%)

| Change | Exact |
|---|---:|
| Two extra channels: sharpened bass-note chroma (MIDI 28 to 45, power 4) and sharpened mid chroma (power 2), linear model | 60.2% (worse) |
| Same, with only the bass-note channel / only the sharpened mid channel added; also one run with L2 0.01 and four channels | 58.4% to 62.2% |
| Equivariant MLP (shared across the 12 tonics, 8 or 16 ReLU units, plus linear skip), 2 channels | 61.9% to 62.7% (worse) |
| MLP 16 units, 4 channels, 8 segment-pool augmentations | 63.4% |
| Linear + augmentation (mean of a random 60% of the 8 s segments, 4 copies per track per epoch) | 64.2% |
| Same, segment voting at predict time | 63.7% |
| Linear + augmentation x8 (70% keep) + 3-seed ensemble (chosen) | 63.9% |
| Front-end sweep, linear: gamma 3 / 30, whitening 12 / 24 / 30, harmonic decay 0.8 / 0.95, 12 harmonics, mid band 40 to 78 / 55 to 96 | 61.4% to 63.4% |
| Bass band top at MIDI 46 / 58 (vs 52) | 57.9% / 59.4% |

Everything that does not remove information lands within +-1.5 points of 63.7%, which is inside the CV noise (about +-2.4 points). What helped: nothing measurable. What hurt: more channels, more capacity, and moving the bass band edge. Reading: with 399 training tracks and Beatport label noise, the model class is not the limit; the 2-channel, 50-weight model is already near the ceiling of this feature set. Per-segment voting and attention-like pooling were not separately built; only voting and random segment pooling were tested. No nested CV was run.

### Runtime

Unchanged from round 1: decode about 355 ms, spectrum about 450 ms, classify about 6 ms per 120 s clip (about 0.8 s per track, single thread, box load average about 20, measured on two clips with `keydet detect`). The final model is the same linear scorer; the 3-net ensemble averages three copies of 50 weights. The two extra channels cost a little extra feature time that was not measured separately.

### Licences

No new crates. New code is `nn.rs` (own code, MIT OR Apache-2.0). The trained weights are a 3 x 50-number summary fit on Beatport EDM Key (CC BY-SA 4.0) for measurement only. They were not embedded or committed: the file is `benchmark/results/q16/final-net.json` (gitignored) and is loaded at eval time with `load=`. `weights.rs` still holds the round-1 weights. Shipping any trained weights waits on the licence decision from round 1.

### Next steps

- Label noise and test size are the likelier limits, not the model: measure agreement between Beatport labels and a second source on the 199 test tracks before more modelling.
- More labelled data with a licence that allows training is the lever the data suggests; repeated CV (several fold assignments) would resolve effects of about 1 point.
- Parallel confusion (12.6%) needs a feature that separates minor from major with the same tonic, for example chord-aware third detection; plain chroma channels did not provide it.

### Reproduce

```bash
cd prototyping/analysis-packaging
cargo build --release --bin keydet
M=../../benchmark/manifests/beatport-edm-key-keyonly-clean-full.corpus.json
./target/release/keydet extract $M /home/merlin/dev/projects/fourfour/benchmark/datasets/beatport-edm-key/audio ../../benchmark/results/keydet-spec threads=4
./keydet-eval.sh method=net split=cv chan=1100 hidden=0 aug=8 keep=0.7 lr=0.05 seeds=3
./keydet-eval.sh method=net split=test chan=1100 hidden=0 aug=8 keep=0.7 lr=0.05 seeds=3 save=../../benchmark/results/q16/final-net.json
./keydet-eval.sh method=net split=test chan=1100 hidden=0 aug=8 keep=0.7 lr=0.05 seeds=3 load=../../benchmark/results/q16/final-net.json
./keydet-sweep.sh sweep.txt     # one eval config per line
```

## R162, 2026-11-08

Goal: at least 60% exact on the Beatport test split (199 tracks, manifest `id % 3 == 0`) with licence-clean methods, every choice made on the 399 dev tracks. Branch `r162-key-accuracy` from `ov-1028`, with b14 (`CLEAN_FSL_FMA`, commit 11784f5) cherry-picked because it is the best clean model so far.

**Result: not reached. The best honest clean model scores 58.3% exact on test (116 of 199).** Shipped on `ov-1028` (`CLEAN_FSL`) scores 56.3% (112), b14 57.3% (114). The +2 / +4 tracks are inside the noise of n = 199 (about +-7 points). The new model is `cyankey::CLEAN_FSL_FMA_SEG` and is `SHIPPED` on this branch.

### Final pick vs shipped (Beatport EDM Key)

E+R+F = exact + relative + fifth. MIREX = exact + 0.5 fifth + 0.3 relative + 0.2 parallel.

| Model | Split | Exact | Relative | Parallel | Fifth | E+R+F | MIREX |
|---|---|---:|---:|---:|---:|---:|---:|
| `clean-fsl` (shipped on ov-1028) | test (199) | 112 = 56.3% | 11 | 22 | 23 | 73.4% | 65.9 |
| `clean-fsl-fma` (b14) | test | 114 = 57.3% | 12 | 19 | 19 | 72.9% | 65.8 |
| **`clean-fsl-fma-seg` (R162 pick)** | test | **116 = 58.3%** | 9 | 19 | 21 | 73.4% | 66.8 |
| `clean-fsl` | dev (399) | 237 = 59.4% | 18 | 50 | 41 | 74.2% | 68.4 |
| `clean-fsl-fma` | dev | 246 = 61.7% | 16 | 37 | 42 | 76.2% | 70.0 |
| **`clean-fsl-fma-seg`** | dev | **248 = 62.2%** | 16 | 34 | 41 | 76.4% | 70.2 |
| `clean-fsl` | all (598) | 349 = 58.4% | 29 | 72 | 64 | 73.9% | 67.6 |
| `clean-fsl-fma` | all | 360 = 60.2% | 28 | 56 | 61 | 75.1% | 68.6 |
| **`clean-fsl-fma-seg`** | all | **364 = 60.9%** | 25 | 53 | 62 | 75.4% | 69.1 |

No number of these models is fit on Beatport labels, so their 598-track score is already out of sample. The only Beatport-dependent step is choosing the variant among the clean candidates. Doing that choice inside 5-fold CV (pick on 4/5, score the held-out 1/5) gives **61.2% on dev (244 / 399)** and **60.7% over all 598 (363)**. The pick was the same in 8 of 10 folds.

Mode errors (relative + parallel) on test: 28 for the pick, 33 for `clean-fsl`.

### What changed

The b14 recipe, with one change in the self-training step: the 2,150 unlabelled FMA clips (30 s) are cut into 10 s segments (27 frames, 3 per clip), the FSL teacher labels each segment, and the student is trained on FSL loops plus the 6,450 labelled segments (each segment weight 1, same as one FSL loop). Major bias frozen at the FSL value (-1.0626) as in b14. Same front end, same 72 weights + bias form, so it is a drop-in template swap. Confidence calibration `CAL_A` / `CAL_C` refit out-of-fold on FSL (0.98842 / 0.24367): mean confidence 0.52 vs accuracy 0.62 on dev, 0.50 vs 0.58 on test, Brier 0.23 / 0.22 (slightly under-confident, as before).

### Every variant (dev, n = 399, exact; nothing fit on Beatport unless marked)

| Idea | Variant | Dev exact |
|---|---|---:|
| baseline | `clean-fsl` / `clean-fsl-fma` (b14 recipe reproduced) | 237 / 246 |
| major bias offset (1 scalar, picked by inner 5-fold CV on dev) | on `clean-fsl` / on `clean-fsl-fma` | 239 / 242 |
| more unlabelled data? | b14 recipe with 500 / 1000 / 1500 / 2150 FMA clips | 242 / 241 / 240 / 246 (flat) |
| pseudo-label filter | keep top 70 / 50 / 30% teacher confidence | 237 / 238 / 233 |
| | keep only clips whose two 10 s halves agree with the clip label (943) | 245 |
| soft pseudo-labels | teacher softmax at temperature x0.5 / x1 / x2 | 238 / 234 / 238 |
| unlabelled weight | 0.5 / 2 | 244 / 242 |
| segments as training examples | 5 s segments, weight raw / normalised 1 / 0.5 | 243 / 244 / 238 |
| | **10 s segments, raw (pick)** / 1 / 0.5 | **248** / 246 / 242 |
| | 20 s segments, raw / 1 / 0.5 | 246 / 246 / 244 |
| segment voting at predict time | 20 s segments, probability sum, energy weight 0 / 1 | 240 / 240 |
| second front end (keydet-like: log, 8-harmonic sum, whitening, bass + mid chroma, computed from the cached semitone bands) | trained on FSL / FSL + FMA student | 199-211 / 221-222 |
| ensemble of the two clean front ends | weight 0.3 / 0.5 / 0.7 on the keydet-like model | 233-239 / 227-232 / 222-226 |
| co-training (pseudo-labels where both teachers agree, 1,686 clips; or summed teacher scores) | cyan student / ensemble 0.2-0.5 | 238-239 / 216-234 |
| joint 5-band linear model (both front ends) | FSL / student | 218 / 216 |
| mode re-ranker (choose among predicted key, relative, parallel; logistic on rotated chroma + score; fit on FSL out-of-fold) | on `clean-fsl`, L2 1e-3 / 1e-2 / 1e-1 | 240 / 240 / 236 |
| | on `clean-fsl-fma`, L2 1e-3 / 1e-2 / 1e-1 | 243 / 246 / 247 |

Reference, **not licence-clean** (fit on Beatport labels, CC BY-SA 4.0; measured because the brief asked for the ensemble with the R12 learned profile). R12 itself needs the keydet front end over the audio; a keydet-like front end from the cached bands stands in for it here.

| Model (5-fold CV) | Dev (399) | All (598) |
|---|---:|---:|
| keydet-like 2-band, Beatport-fit | 229 = 57.4% | 350 = 58.5% |
| cyan 3-band, Beatport-fit | 239 = 59.9% | 351 = 58.7% |
| `clean-fsl-fma` + keydet-like Beatport-fit, weight by inner CV | 242 = 60.7% | 365 = 61.0% |
| `clean-fsl-fma` + cyan Beatport-fit, weight by inner CV | 239 = 59.9% | 360 = 60.2% |

Reading: even with Beatport labels the ensemble lands around 60-61% in CV, the same as the clean model. Everything that does not remove information lands within about +-2 points of 61.7% dev (CV noise is about +-2.4). The FMA data curve is flat from 500 to 2,150 clips, so more clips of the same kind will not move it. The keydet-like front end is much weaker when trained on loops (FSL is often one instrument), so the ensemble the brief proposed only helps when that model is fit on in-domain labels, which is not clean. The linear template model on this front end is at its ceiling; a clear step needs a few hundred to 1,500 permissively labelled full EDM tracks, which do not exist as far as Q46, b8 and b14 found.

### Rekordbox agreement (library set)

The findings doc's Rekordbox number is on Beatport audio, but no per-track Rekordbox keys for it are stored. The per-track Rekordbox set on this box is Merlin's library playlist (`benchmark/manifests/rekordbox-testdata-2025.corpus.json`, keys from `master.db`): 622 of 652 tracks have a key and decode (one file fails in symphonia). Full-length tracks; 86% of them are minor per Rekordbox. Agreement with Rekordbox (not accuracy):

| Model | Exact agree | Relative | Parallel | Fifth | E+R+F | Predicted minor |
|---|---:|---:|---:|---:|---:|---:|
| `clean-fsl` (shipped) | 269 = 43.2% | 18 | 20 | 102 | 62.5% | 90% |
| `clean-fsl-fma` | 271 = 43.6% | 16 | 18 | 101 | 62.4% | 95% |
| **`clean-fsl-fma-seg`** | **274 = 44.1%** | 18 | 15 | 100 | 63.0% | 96% |

It does not move away from Rekordbox (+5 tracks). Many disagreements are a fifth apart (100 tracks), where Rekordbox and this detector often hear the dominant differently.

### Runtime and size

Same front end and the same 73 numbers plus 2 calibration constants, so runtime and binary size are unchanged: `cyankey` median detect 125 ms per 120 s clip (decode excluded, single thread, box load average 20 to 40; b14 measured 234 ms for the same code path under a different load, so the difference is load). The Rust bin reproduces the lab predictions on all 598 tracks (0 mismatches). No new dependencies.

### Licences

| Piece | Source | Licence | Used for |
|---|---|---|---|
| FSL10K loops (671, CC0 / CC BY 3.0) and annotations | Zenodo 3967852 | CC BY 4.0 dataset, per-loop CC0 / CC BY 3.0 | labelled training (as b8) |
| FMA clips (2,150, Electronic, CC BY / CC0 / PDM) | `fma_large`, per-track licence from `tracks.csv` | CC BY / CC0 / PDM | unlabelled audio; labels are our own model's (as b14) |
| Krumhansl-Kessler profile | Krumhansl 1990 (paper numbers) | n/a | L2 prior |
| Beatport EDM Key | Zenodo 1101082 | CC BY-SA 4.0 | evaluation and variant choice only (about 40 variants compared on dev); the Beatport-fit rows above are reference only and are not shipped |
| code (`key-lab/r162*.py`, `cyankey.rs`) | own | MIT OR Apache-2.0 | |

No label in the shipped model comes from a non-permissive source. The caveats from b8 and b14 remain: x^0.75 compression and the variant were chosen on Beatport dev (selection, not fitting), and the front-end constants come from the BSD-2 cyanbox port. Attribution files (`scripts/mac/LICENSE-key-model.txt`, `NOTICE`, `benchmark/manifests/fma-electronic-cc-unlabelled.json`) come with the b14 cherry-pick and cover the same FMA clips.

### Reproduce

```bash
# band caches from b8 / b14 (keybands bin): beatbands, fslbands, fmabands
cd benchmark/scripts/key-lab
export KEY_BANDS=<beatbands> FSL_BANDS=<fslbands> FSL_MANIFEST=<fsl.corpus.json> UNL_BANDS=<fmabands>
uv run --no-project --with numpy python r162_e1.py    # baselines, bias offset, keydet-like front end, ensembles
uv run --no-project --with numpy python r162_e2.py    # self-training variants, segment voting
uv run --no-project --with numpy python r162_e3.py    # co-training, joint model, mode re-ranker
uv run --no-project --with numpy python r162_e4.py    # Beatport-fit reference, segment students (writes cands.npz)
uv run --no-project --with numpy python r162_e5.py    # variant choice by inner CV (SHOW_TEST=1 adds all 598)
SHOW_TEST=1 uv run --no-project --with numpy python r162_final.py
SHOW_TEST=1 uv run --no-project --with numpy python r162_export.py   # templates, bias, calibration for cyankey.rs
KEY_BANDS=<rekordbox bands> uv run --no-project --with numpy python r162_rekordbox.py
cyankey <beatport manifest> <beatport audio> 4 rows.txt clean-fsl-fma-seg
```

Caches go to `~/.cache/r162-work` (`R162_CACHE`).
