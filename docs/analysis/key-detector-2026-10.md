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
