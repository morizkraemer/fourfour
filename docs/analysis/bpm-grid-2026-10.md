# BPM and beat grid in Rust vs Rekordbox, 2026-10 (Q13)

Status: measured on a held-out test split. Prototype only; nothing is wired into the app. Branch `q13-bpm-grid`.

## Read this first

- The reference is Rekordbox's own analysis (master.db BPM and the PQTZ beat grid in its ANLZ files). Agreement is compatibility with Rekordbox, not correctness (docs/process/lessons.md). Some grids may have been hand-edited by Merlin; that is not known.
- Rust-native, permissive licences only: Beat This! (full model, via `rten`) plus our own constant-tempo fit. No Python, no Essentia, no DeepRhythm.
- Beat This! ran on one **120 s window from the middle of each track**, not the whole track (compute budget, see "Runtime"). The fitted constant grid is then extended over the whole track and scored against Rekordbox's whole-track grid. A 3-track check with full-track analysis gave the same numbers within noise, so the window is not the limiting factor.
- Headline: BPM is now fractional and close to Rekordbox (86% within +-0.1 on the test split). The grid agrees on the beat level (F-measure 0.875 at 70 ms) but only 47% of tracks stay within +-20 ms of Rekordbox over the whole track. Two causes, both small: a per-track phase offset of 10 to 30 ms between the model and Rekordbox, and tempo error below 0.01 BPM that accumulates over a whole track (0.005 BPM is 11 ms over 5 minutes at 125 BPM). See "Why the grid share is 47%".
- Stratum was not scored (see "What was not done").

## Grid ground truth

Source: Merlin's Rekordbox library copy at `benchmark/datasets/rekordbox-library/` (read-only): `master.db` plus `share/PIONEER/USBANLZ/**/ANLZ0000.DAT`. The prototype bin `rbref` opens `master.db` (SQLCipher, same key the repo's reader uses) and reads `FolderPath`, `AnalysisDataPath`, `BPM`, `Length`, title and artist from `djmdContent`; each `.DAT` is parsed with `pioneer_usb_writer::reader::anlz::read_anlz` (path dependency, not edited). `benchmark/scripts/make_grid_reference.py` joins this to the corpus manifest and writes `benchmark/results/rb-grid-reference.json` (id to beats in ms, bar positions, per-beat tempo, master.db BPM; gitignored, contains Merlin's track list).

| Step | Count |
|---|---:|
| `djmdContent` rows (non-deleted) | 1382 |
| Rows with a PQTZ grid in ANLZ | 1351 |
| Manifest entries (`rekordbox-testdata-2025`) | 652 |
| Manifest entries matched to a library row | 652 (all by exact path after stripping the Mac prefix; the filename+duration fallback was never needed) |
| Of those with a PQTZ grid | **643** (9 rows have none) |
| Of those with the audio on this box | 622 |
| Of those with audio fingerprint-verified (SHA256 of first+last 64 KB + size, same rule as Q9b) | **612** (10 mismatches excluded) |

Properties of the reference grids (643):

- PQTZ beat times are integer milliseconds. First beat: median 87 ms, max 760 ms (the grid starts at the first transient, not at a downbeat).
- 603 of 643 have a single tempo value. By a stricter test (every beat within 5 ms of one straight line) 40 are not constant (dynamic or hand-edited grids). Tables give "all" and "constant only".
- BPM reference for the BPM columns is the master.db `BPM`, the same reference as docs/analysis/benchmark-2026-10.md. It differs from the first PQTZ tempo by more than 0.01 on 39 tracks.

## Splits and denominators

| Split | Rule | Reference grids | Audio verified and analysed |
|---|---|---:|---:|
| Test | manifest id % 3 == 0 | 212 | **205** (unique by artist + title + BPM: 195) |
| Dev | manifest id % 3 != 0 | 431 | 58 analysed (of 407 verified; time budget) |

The 7 test grids not scored have no verified audio on this box. One dev track (id 172) failed to decode (`unexpected end of file`, a damaged mp3). Test tracks were never looked at while tuning. Everything below the "Method" heading that says "tuned" used the 58 dev tracks only.

## Method

1. Decode with symphonia to 22.05 kHz mono (inside `beat-this`), take a 120 s window centred in the track.
2. Beat This! full model (83 MB ONNX, rten) produces per-frame beat and downbeat logits at 50 fps. The bin `bgdump` stores them; `grid` does everything else, so tuning does not re-run the model.
3. Constant-tempo fit (`grid`): refine each model beat to sub-frame time with a probability-weighted centroid over +-2 frames; take the mode of the inter-beat intervals; number the beats by incremental rounding (robust to gaps); least-squares line `t = t0 + k*P` with shrinking outlier gates (50, 30, 20, 15 ms); then a local search on the sigmoid beat activation (comb score over the window, period and phase, +-0.02% / +-4 ms start steps, 4 halvings) to polish BPM and phase. This gives a fractional BPM and a first-beat phase. Cost: 1 to 20 ms per track.
4. Tempo octave: fold the fitted tempo into `[lo, 2*lo)` by doubling or halving. `lo` is the DJ-range prior; tuned on dev to **75**, so the range is 75 to 150 BPM. When the fold halves the tempo, the phase among the candidate beats is chosen by the downbeat activation. No energy or onset-density cue is used: on dev the plain fold already gave 98% to 100% within 1 BPM (lo = 75 was the best of 60 to 100 in steps of 5; below 75 half-speed errors return, above 80 one 150+ BPM track is halved).
5. Grid: first beat = phase modulo the period, plus a global shift of -7 ms (tuned on dev: the median model-minus-Rekordbox offset was +7 ms), then beats every period to the end of the track.

Tried and dropped: moving the grid to the audio's spectral-flux onsets (+-50 ms search). On dev it lowered the within-20-ms share from 44% to 33% (it disagrees with Rekordbox's phase by more than the model does). The code is still in `grid` behind `--audio-phase` for reproduction.

## Results on the test split

Reference: Rekordbox. Denominator: 205 test tracks (189 with a constant-tempo reference grid). Settings: `--lo 75 --comb --shift-ms -7`.

| Metric | All 205 | Constant-grid refs only (189) | Unique tracks (195) |
|---|---:|---:|---:|
| BPM within +-0.1 | 177 = 86.3% | 174 = 92.1% | 169 = 86.7% |
| BPM within +-0.5 | 183 = 89.3% | 178 = 94.2% | 175 = 89.7% |
| BPM within +-1 | 190 = 92.7% | 181 = 95.8% | 181 = 92.8% |
| Octave error x2 | 0 | 0 | 0 |
| Octave error x0.5 | 2 | 1 | 2 |
| Octave error x1.5 | 1 | 0 | 1 |
| Octave error x2/3 | 0 | 0 | 0 |
| Other miss (more than 1 BPM, not an octave ratio within 2%) | 12 | 7 | 11 |
| Beat F-measure at 70 ms (fitted grid over the whole track, mean) | 0.875 | 0.908 | 0.876 |
| Median per-track grid offset vs Rekordbox (signed, est minus ref) | 0.0 ms | 0.1 ms | 0.0 ms |
| Median per-track grid offset, absolute | 8.3 ms | 8.1 ms | 8.3 ms |
| **Whole grid within +-20 ms of Rekordbox** (every Rekordbox beat has an estimated beat within 20 ms) | **97 = 47.3%** | 97 = 51.3% | 93 = 47.7% |
| 95% of beats within +-20 ms | 108 = 52.7% | 107 = 56.6% | 103 = 52.8% |

Octave error: estimated BPM / Rekordbox BPM within 2% of the ratio, and not already within 1 BPM. Whole-grid deviation, all 205 tracks (max over all Rekordbox beats): within 10 ms 54, 20 ms 97, 30 ms 123, 50 ms 141, 70 ms 147. All 97 tracks within 20 ms also have BPM within +-0.1; the other 80 tracks with BPM within +-0.1 miss the 20 ms test for the reasons below.

### Why the grid share is 47%

Constant-grid references only (189 test tracks); per track, the estimated minus Rekordbox offset is fitted with a line over the track. 151 of the 189 are straight to within 10 ms after removing offset and drift (the other 38 have irregular beats in the reference or in the estimate, more than 30 ms off a line). Median absolute drift over the track: 7.9 ms; 146 of 189 drift 20 ms or less. Of the 81 tracks with BPM within +-0.5 that still fail the 20 ms test: 45 have a median offset above 20 ms (phase), 37 drift more than 15 ms (tempo error of a few thousandths of a BPM), 27 have irregular beats; the groups overlap.

Other components (same split):

| Variant | BPM +-0.1 | BPM +-1 | x0.5 octave errors | Grid within +-20 ms |
|---|---:|---:|---:|---:|
| Baseline: median IBI of the model's beats, `lo` 70, no fit, no shift | 70.2% | 77.6% | 34 | 27.3% |
| Median IBI BPM, `lo` 75 (the fold alone) | 45.4% | 92.7% | 2 | n/a |
| Final: constant fit + comb + `lo` 75 + shift | 86.3% | 92.7% | 2 | 47.3% |

So the octave fold fixes the 1 BPM column (77.6% to 92.7%), and the fit fixes the 0.1 BPM column (45.4% to 86.3%).

Dev split (n = 58, tuned on): BPM +-0.1 87.9%, +-1 98.3%, grid within 20 ms 44.8%, F-measure 0.907. The test numbers are lower on +-1 (92.7% vs 98.3%) mainly because the test split holds more tracks outside the prior (below).

### Misses (test)

- Outside the 75 to 150 prior by design: Rio Corrente (Rekordbox 162.17, estimate 82.8), To Rococo Rot - Die Dinge des Lebens (170.07 vs 85.03), U.N.I.T. - Untitled (181.82 vs 91.04). The test reference has 3 of 205 tracks above 150 and 1 of 205 in 75 to 80; in the 407 verified dev tracks 6 are above 150.
- Broken-beat or irregular material where the model and Rekordbox pick different pulses (not octave ratios): Floating Points - Falaise (82.19 vs 143.29), Khotin - Shopping List (78.53 vs 118.38, 1.5x), Polar Inertia - Arctic Horizon (99.02 vs 145.00), Koreless - Primes (95.24 vs 135.26), Squarepusher - Detroit People Mover (106.51 vs 140.83). DeepRhythm failed on Falaise and Shopping List too (143 and 136 vs about 80).
- Small differences: LP - When The Beat Drops (120.00 vs 119.00, dev), Datassette - Mechanical Advantage (130.02 vs 133.90), Mount Kimbie - Zone 3 (127.40 vs 130.05).
- The known slow-track failures of DeepRhythm that sit in the dev split came out right: Four Tet - Two Thousand and Seventeen (75.45, estimate 75.445). Not re-scored on test.

## Comparison with DeepRhythm

DeepRhythm numbers are from docs/analysis/benchmark-2026-10.md (Q9b), whose raw per-track results are not on this box. The denominators differ: DeepRhythm 472 unique tracks or 573 entries across all of the 579 verified Rekordbox entries (no split), this run 205 test tracks (195 unique). The test tracks are a third of the same corpus, so the comparison is indicative, not paired.

| Metric | DeepRhythm (472 unique / 573 entries) | Rust, test (195 unique / 205 entries) |
|---|---:|---:|
| BPM within +-0.1 | 35.6% / 38.7% | 86.7% / 86.3% |
| BPM within +-0.5 | not reported | 89.7% / 89.3% |
| BPM within +-1 | 90.0% / 90.8% | 92.8% / 92.7% |
| Octave errors (strict) | 0 at x2, 3 at x0.5 of 472 | 0 at x2, 2 at x0.5, 1 at x1.5 of 205 |
| Fractional BPM | none (integers only) | yes |
| Beat grid | none (`beats` empty) | yes: F70 0.875, 47.3% within 20 ms |

## Runtime per track

Single process, full model, rten on 16 threads, a box at load average about 8 (not idle; other agents were running):

| Step | 4 test-range tracks, 343 to 406 s long |
|---|---|
| Decode to 22.05 kHz mono | 1.9 to 2.7 s |
| Beat This! on the 120 s window | 7.4 to 7.5 s |
| Window total | about 10 s per track (0.026 x realtime of the track) |
| Beat This! on the whole track (3 tracks) | 17.9 to 22.5 s, about 0.055 x realtime of the track |
| Constant-tempo fit | 1 to 20 ms |

During the batch run (4 processes x 3 threads at load average 30 to 60) the same window cost about 23 to 26 s per track; treat that as an upper bound. A 5,000-track library at about 10 s per track is about 14 hours sequentially, or about 4 hours on 4 processes; whole-track analysis roughly doubles that. Peak RSS was not re-measured (Q10: about 1.4 to 1.6 GB with stratum; Beat This! alone is lower). Apple Silicon not measured.

## Licences

No new crate in the `ffproto` package: `bgdump` and `grid` use only crates already there (`beat-this` 1.1.0 MIT, `rten` 0.24 MIT OR Apache-2.0, `rustfft` MIT OR Apache-2.0, `serde_json` MIT OR Apache-2.0, `stratum-dsp` 1.0.0 MIT OR Apache-2.0). The ffproto tree (105 crates, `cargo metadata --locked`) is MIT, Apache-2.0, Zlib, BSD or MPL-2.0 (32 symphonia crates, file-level), no GPL or AGPL. Models are not committed: `beat_this.onnx` (MIT code and weights; the upstream README warns that some training data has limited licences, which is the same open item as in packaging-decision.md) and the mel model, fetched by `fetch-models.sh`.

`rbref` (new workspace member, an offline dev tool that is never shipped) depends on `pioneer-usb-writer` (path, no licence field in its Cargo.toml; the repo has no LICENSE) and `rusqlite` 0.33 with `bundled-sqlcipher` (MIT; SQLCipher is BSD-style, OpenSSL Apache-2.0), the same stack `pioneer-usb-writer` already uses. Its 151-crate tree has no GPL or AGPL; one crate (`r-efi`) lists `MIT OR Apache-2.0 OR LGPL-2.1-or-later`, so the permissive option applies.

## What was not done

- Stratum-dsp was not scored. `bgdump --stratum` exists but is untested at scale: Q10 measured 9 to 16 s per 120 s clip on a loaded box, about 60 s per full track, and the Beat This! batch used the box. Q10's observation (stratum's beat list is incomplete, its grid can start 25 s late) is unchanged.
- No onset-aware phase correction that agrees with Rekordbox: the one tried made it worse. The 20 ms grid share is limited by phase offset and sub-0.01 BPM tempo error, so the next experiments are a fit over the whole track (the 3-track check did not show a gain), and a phase rule closer to Rekordbox (its own transient rule, or a learned offset).
- No energy or onset-density cue for the octave decision; the dev set did not need one, and the prior is wrong by design for the 1.5% of tracks above 150 BPM.
- Only 58 of 407 verified dev tracks were analysed, so `lo` and the shift are tuned on a small set. The first 36 dev tracks all fell in 75 to 150 BPM.
- Dynamic Rekordbox grids (40 of 643) cannot be matched by a constant grid; they are in the "all" column.
- Downbeat (bar 1) position is not scored.

## Reproduce

All paths relative to the repo root. Data stays read-only.

```bash
cd prototyping/analysis-packaging
./fetch-models.sh
cargo build --release
cargo build --release -p rbref
cd ../..

prototyping/analysis-packaging/target/release/rbref \
  benchmark/datasets/rekordbox-library benchmark/results/rb-library-dump.json

python3 benchmark/scripts/make_grid_reference.py \
  --library-dump benchmark/results/rb-library-dump.json \
  --manifest benchmark/manifests/rekordbox-testdata-2025.corpus.json \
  --dataset-root benchmark/datasets/rekordbox-testdata-2025 \
  --out benchmark/results/rb-grid-reference.json

for i in 0 1 2 3; do
  RTEN_NUM_THREADS=3 prototyping/analysis-packaging/target/release/bgdump --window 120 \
    --reference benchmark/results/rb-grid-reference.json \
    --out benchmark/results/q13-win120 --shard $i/4 &
done; wait

prototyping/analysis-packaging/target/release/grid benchmark/results/rb-grid-reference.json \
  benchmark/results/q13-win120 --split test --lo 75 --comb --shift-ms -7 \
  --out benchmark/results/q13-test-final.json
```

Add `--dedupe` for the unique-track column. Dev tuning: `--split dev --sweep --comb` (octave prior), `--split dev --shift-ms N` (shift). Baseline row: `--split test --lo 70`. The batch run processes dev tracks with id % 12 == 4 first, then the test split; stop it whenever the budget ends and score what exists. Run time for the 205 test tracks plus 58 dev tracks was about 35 minutes on 4 processes at a busy box.
