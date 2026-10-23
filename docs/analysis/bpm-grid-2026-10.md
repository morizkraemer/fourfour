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

# Q15, 2026-10-23: grid phase and drift

Status: measured on the same held-out test split (manifest id % 3 == 0). Prototype only. Branch `q15-grid`. Settings were tuned on the dev split only; test numbers are for the final settings.

## Result (test split, 205 tracks)

| Metric | Q13 (window, above) | Q15 (full track, final) |
|---|---:|---:|
| BPM within +-0.1 | 177 = 86.3% | 180 = 87.8% |
| BPM within +-1 | 190 = 92.7% | 190 = 92.7% |
| Octave errors x2 / x0.5 / x1.5 | 0 / 2 / 1 | 0 / 3 / 0 |
| Other miss (more than 1 BPM, not an octave ratio) | 12 | 12 |
| Beat F-measure at 70 ms (mean) | 0.875 | 0.893 |
| Median per-track offset vs Rekordbox, signed (est minus ref) | 0.0 ms | +6.7 ms |
| Median per-track offset, absolute | 8.3 ms | 11.3 ms |
| **Whole grid within +-20 ms of Rekordbox** | **97 = 47.3%** | **148 = 72.2%** |
| Whole grid within 10 / 30 / 50 ms | 54 / 123 / 141 | 50 / 163 / 168 |
| Constant-grid references only (189): grid within 20 ms | 97 = 51.3% | 148 = 78.3% |
| Constant-grid references only: BPM within +-0.1 | 174 = 92.1% | 178 = 94.2% |

The median offset got worse on purpose: the global -7 ms shift is gone (see "Phase"), and the metric that counts is the worst beat, not the median. The x0.5 octave errors are the three tracks above 150 BPM (outside the 75 to 150 prior, same as before); one Q13 x1.5 miss is now counted under "other". By container (test): mp3 52 of 88 within 20 ms, flac 34 of 42, wav 62 of 75.

Of the 179 constant-grid tracks within +-0.5 BPM, 31 still fail 20 ms: 21 have a median offset above 20 ms, 14 drift more than 15 ms, 4 have irregular beats (groups overlap). The remaining misses are mostly tracks where the model's tempo wanders by 0.1 to 0.8 BPM or sits half a beat off.

## What helped (dev split, 104 tracks, full-track logits; grid within 20 ms)

| Step (cumulative) | BPM +-0.1 | F70 | Grid within 20 ms |
|---|---:|---:|---:|
| Q13 settings (comb, `lo` 75, shift -7 ms), run on full-track logits | 83.7% | 0.863 | 52.9% (55) |
| shift 0 + MP3 encoder-delay correction | 83.7% | 0.872 | 64.4% (67) |
| + snap to integer BPM when within 0.05 | 83.7% | 0.874 | 66.3% (69) |
| + exhaustive tempo scan of the whole-track comb (+-0.3%) | 90.4% | 0.919 | 70.2% (73) |
| + wide integer snap (within 0.5 BPM if the comb score keeps 97%) = final | 91.3% | 0.925 | **71.2% (74)** |

1. **MP3 encoder delay (largest effect).** Rekordbox's grid for an mp3 sits later than the decoded audio by the LAME delay: ffprobe reports a 25.057 ms start for these files (1105 samples at 44.1 kHz = delay field 576 + 529 decoder delay). The Q13 offset by container (test, tracks with BPM within 0.5): mp3 median -20.0 ms (est earlier than Rekordbox), flac +3.9 ms, wav +3.5 ms. Mp3 files without a LAME header (start 0.000) show about 0. The fix reads the LAME header from the file (`mp3_reference_delay_s`, 20 lines, no crate) and adds `(delay + 529) / sample_rate` to the grid; the global -7 ms shift, which was a compromise between mp3 and the rest, is not needed (dev: shift 0 beats -3 and +3 and -7). This probably also explains why the Q13 audio-onset alignment looked worse than the model: it moved mp3 grids toward the audio, away from Rekordbox. So the reference is not wrong; it is a timeline that includes the encoder delay. Whether an app writing grids for CDJ playback should apply the same offset for mp3 is not settled here and needs a hardware check.
2. **Whole-track tempo.** Beat This! on the full track instead of a 120 s window, then an exhaustive scan of the comb score over tempo (+-0.3% in steps of 1e-5, phase in 4 ms steps, pivot at the track middle) before the local search. The old local search got stuck on 5 to 8 dev tracks. On dev: BPM +-0.1 83.7% to 90.4%, grid +4 points. Cost: about 1.1 s per track (median, unoptimised) on top of the model.
3. **Integer BPM prior.** 496 of the 643 reference BPMs (77%) are exact integers, and for those the Q13 estimate is within 0.03 of the integer in 143 of 153 cases. Snapping removes drift for those tracks. The wide variant (up to 0.5 BPM away, accepted only if the whole-track comb score stays at or above 97% of the unsnapped score) adds one dev track; treat it as marginal.
4. **Grid edges (2 fixes, test +2.9 points on the Q13 logits).** Estimated beats now run to the audio end plus 30 ms (a Rekordbox beat 0.4 ms past the audio duration counted as a 460 ms miss), and a first beat that wraps from slightly negative to a full period late is kept at 0 instead. With only these two fixes and the old settings, the Q13 window logits score 103 of 205 = 50.2% on test (checked once as a reproduction; the "before" column keeps the published 47.3%).

## What did not help

- **Audio-onset phase alignment** (a spectral-flux envelope, best offset within +-40 ms, gated by peak contrast; `--audio-q/--audio-calib-ms/--audio-min-ms`). Median |offset| drops from 11 to about 5 to 7 ms on dev, but the grid within 20 ms falls: 71.2% to 70.2% (only large corrections, gate 15 ms, contrast 4) and 58.7% (contrast 2.5). The worst beat decides the metric, and Rekordbox's own grid is not on the audio's transients to that precision. Same conclusion as Q13, now with the mp3 delay removed as a confound.
- **Least-squares line through all beats instead of the comb** (comb off, shift 0, mp3 delay, snap 0.05): 67.3% against 71.4% with the comb, measured on the first 49 dev tracks. The line is only the start point of the comb in the final pipeline.
- **A global phase shift on dev** (-7, -4, -3, 0, +2, +3, +4, +6 ms, measured on 49 to 92 tracks): 0 is best, +2 to +4 is flat, the others lose 3 to 7 points.
- **Downbeat (bar position) alignment**: not scored here, and not needed for the +-20 ms test; the Q13 downbeat head is still used only to choose the phase when the tempo is halved.
- Wider integer snap alone (`--snap-tol 0.1`): 69.2%, lower than 0.05.

## Denominators and caveats

- Test: the same 205 verified-audio tracks as Q13 (212 reference grids; 7 without verified audio). Dev: 104 tracks analysed with the full-track model (Q13 used 58), of the 407 verified; they are the first 104 in the bgdump order (ids with id % 12 equal to 4 or 7, then a few more), not a random sample. The test split was run once with the final settings.
- The dev set was extended and then frozen at 104 before the last tuning steps; the first rows in the dev table were also measured while the set was at 49 to 92 tracks, but the table above is all 104.
- Dev gains do not equal test gains: grid 52.9% to 71.2% on dev, 47.3% (50.2% with the edge fixes) to 72.2% on test.
- Reference remains Rekordbox, so all of this measures compatibility with it. The mp3 delay rule is derived from 2 containers' worth of offsets and the LAME header, not from a Rekordbox spec; other encoders (no LAME tag, VBR headers without it) are treated as zero delay.

## Runtime per track

Beat This! on the full track: 29 to 41 s per 345 to 465 s track with 4 processes at 3 threads each on a box at load average 20 to 25 (about 0.1 x realtime of the track; the idle box measured 0.055 x realtime in Q13, so about 20 s for a 6 minute track). Decode 1.5 to 2.5 s. Tempo scan and fit 0.8 to 2 s (median 1.1 s on test; unoptimised scalar loops). Total about 25 s per track on an idle box, roughly double the 120 s window; a 5,000-track library is about 35 hours sequentially or 9 hours on 4 processes.

## Reproduce

```bash
cd prototyping/analysis-packaging
./fetch-models.sh
cargo build --release
cd ../..

for i in 0 1 2 3; do
  RTEN_NUM_THREADS=3 prototyping/analysis-packaging/target/release/bgdump \
    --reference benchmark/results/rb-grid-reference.json \
    --out benchmark/results/q15/full --shard $i/4 &
done; wait

prototyping/analysis-packaging/target/release/grid benchmark/results/rb-grid-reference.json \
  benchmark/results/q15/full --split test --lo 75 --comb --shift-ms 0 --mp3-delay \
  --snap-tol 0.05 --snap-wide 0.5 --snap-ratio 0.97 --scan-pct 0.3 \
  --out benchmark/results/q15/test-final.json
```

`bgdump` without `--window` runs the whole track; it writes dev tracks with id % 12 == 4 first, then the test split, then id % 12 == 7 and the rest, and skips tracks already dumped, so it can be stopped at any time. Dev rows: replace `--split test` with `--split dev` and the flags as in the table (`--shift-ms -7 --comb --lo 75` for the Q13 row). Negative results: `--audio-q 4 --audio-calib-ms 2 --audio-min-ms 15`. `--diag` prints the per-track audio-flux offsets that led to the mp3 finding; `GRID_DEBUG_ID=<id>` prints the worst beat of one track. The reference JSON used here sat next to the Q13 logits (`benchmark/results/q13/rb-grid-reference.json`).

# Q30, 2026-10-24: grid round 3 (Lame3.100, PCM shift, octave and half-beat)

Status: measured. Prototype only. Branch `q30-grid` (from `q26-analysis`, master merged). Tuned on the dev split (manifest id % 3 != 0) only; the test split (id % 3 == 0, n=205) was scored once with the final settings. Reference remains Rekordbox, so all of this measures compatibility with it.

## Result (test split, n=205, same tracks and logits as Q26)

| Metric | Q26 (pass 2) | Q30 (final) |
|---|---:|---:|
| **Whole grid within +-20 ms of Rekordbox** | 148 = 72.2% | **161 = 78.5%** |
| Constant-grid references only (189): grid within 20 ms | 148 = 78.3% | 161 = 85.2% |
| BPM within +-0.1 | 180 = 87.8% | **180 = 87.8%** (no change, no regression) |
| BPM within +-0.5 / +-1 | 184 / 190 | 184 / 190 |
| Octave errors x0.5 | 3 | 2 |
| Beat F-measure at 70 ms | 0.894 | 0.895 |
| Median per-track offset, signed / absolute | +7.9 / 10.2 ms | +0.7 / 3.9 ms |
| Grid within 20 ms by container: mp3 / flac / wav | 52/88, 34/42, 62/75 | 61/88, 37/42, 63/75 |

The 13 new grid hits are 9 mp3 (Lame3.100 files: 1 of 14 to 10 of 14), 3 flac, 1 wav. BPM did not move because none of the three changes touches the tempo of a track that was already within 0.1; the keep-native rule turned one x0.5 miss (Rio Corrente, 82.5 vs 162.2) into a 1.8% miss (165.0). Test misses left, 205 tracks: 15 beyond +-1 BPM, 25 beyond +-0.1.

## Result (dev split, 412 tracks with audio, cumulative)

Dev = the 412 of 431 dev reference grids whose audio is on the box (the brief says 407 verified; I filtered on the file existing and did not chase the 5 difference). 104 of them are the Q15 full-track logits, 308 were dumped this run with `ffa --dump` (7 workers x 1 thread, 72 minutes, 4.28 tracks per minute at load 20 to 30); one damaged wav (Boswick Gates - You Are The Reason, "unexpected end of file") failed to decode. mp3 first, ordered by encoder tag.

| Step (cumulative) | Grid within 20 ms | BPM +-0.1 | BPM +-1 |
|---|---:|---:|---:|
| Q26 settings (`enc+header` mp3 rule, no PCM shift) | 289 = 70.1% | 370 = 89.8% | 383 = 93.0% |
| + Lame3.100 timeline one frame (26.1 ms) earlier | 306 = 74.3% | 370 | 383 |
| + PCM (wav, flac) timeline shift of -10 ms | 330 = 80.1% | 370 | 383 |
| + keep the model's native tempo when it is 150 to 185 BPM | 331 = 80.3% | 372 = 90.3% | 385 = 93.4% |
| + integer snap reaches 1.5 BPM (was 0.5) = **final** | **331 = 80.3%** | **374 = 90.8%** | **386 = 93.7%** |

By container (dev, grid within 20 ms, before to after): mp3 113/180 to 130/180, flac 55/81 to 71/81, wav 121/151 to 130/151. Test split gains were smaller on wav (62 to 63 of 75) because 11 of its 75 wav tracks have a wrong BPM.

## Tag x offset table

Median of the per-track median offset (estimate minus Rekordbox) for tracks with BPM within 0.5 and a constant reference grid, in ms. "Before" is the Q26 rule (`enc+header`, no PCM shift); "after" is final. Encoder = the 9 bytes of the LAME/Lavc/Lavf string in the Xing/Info frame. Groups under 5 tracks are folded into "other mp3 tags". "n/a" = the Q26 test run printed no per-tag table for that group (only the final numbers are available).

| Container : encoder | Dev n | Dev median offset before | after | Dev within 20 ms before to after | Test n | Test median before | after | Test within 20 ms before to after |
|---|---:|---:|---:|---|---:|---:|---:|---|
| mp3 : Lame3.100 | 31 | +23.9 | **-1.0** | 5/31 to **22/31** | 14 | +25.5 | **+1.3** | 1/14 to **10/14** |
| mp3 : LAME3.99r | 37 | -0.6 | -0.6 | 28/37 | 18 | +0.5 | +0.5 | 13/18 |
| mp3 : Lavc61.19 | 34 | -1.2 | -1.2 | 18/34 | 16 | n/a | -0.3 | 9/16 |
| mp3 : Lavc57.24 | 22 | +0.7 | +1.0 | 17/22 | 9 | n/a | -0.9 | 7/9 |
| mp3 : Lavc57.48 | 9 | +2.3 | +2.3 | 7/9 | 12 | n/a | +3.1 | 9/12 |
| mp3 : Lavf56.9. | 8 | +1.1 | +1.1 | 8/8 | 2 | n/a | -6.1 | 2/2 |
| mp3 : Lavc56.19 | 7 | +1.3 | +1.3 | 6/7 | 4 | n/a | +7.0 | 3/4 |
| mp3 : no Xing/Info header | 4 | +20.2 | +20.2 | 1/4 | 4 | +13.3 | +13.3 | 2/4 |
| mp3 : other tags (LAME3.98r, Lavf, Lavc, 21 tracks) | 21 | -7.4 to +3.9 | same | 17/21 | 8 | n/a | -2.3 to +15.1 | 6/8 |
| flac | 81 | +12.3 | **+2.3** | 55/81 to **71/81** | 42 | +10.9 | **+1.3** | 34/42 to 37/42 |
| wav | 151 | +9.6 | **-0.4** | 121/151 to **130/151** | 75 | +10.3 | **+0.3** | 62/75 to 63/75 |

Only Lame3.100 gets an encoder-specific rule (45 files, 31 dev and 14 test; the tag is the mixed-case string `Lame3.100`, `LAME3.99r` is a separate group). The 4 dev and 4 test mp3 files with no Xing/Info header at all have no encoder string and a delay of 0; their median offsets are +20 ms (dev, 2 of the 4 have the right BPM) and +13 ms (test).

## What helped

1. **Lame3.100 timeline (+17 dev tracks, +9 test).** Pass 2 guessed the sign wrong: the hypothesis said "add one frame", but the Lame3.100 estimates are *late* by 14 to 38 ms (median +23.9, all 31 dev files have delay field 576 and `header_counts` false). Adding a frame doubled the offset (+50, 0 of 31 within 20 ms). Subtracting one MPEG frame (1152 / 44100 = 26.1 ms) from the `enc+header` delay gives median -1.0 ms and 22 of 31 within 20 ms; of the 9 dev misses left, 2 are half-beat phase misses (ids 608, 628), 3 are constant offsets of 25 to 57 ms (635, 646, 650), the rest drift or have a wrong BPM. The rule does not touch any other encoder (LAME3.99r, Lavc, Lavf unchanged to the digit). Mechanism not known; it is empirical (Rekordbox's timeline for these files does not match the cyanbox frame map).
2. **PCM timeline shift of -10 ms (+24 dev tracks; on test +3 flac and +1 wav together with the other changes).** The wav and flac offsets were +9.6 and +12.3 ms on dev (test +10.3 and +10.9 in the same table, and in pass 2). Constant offsets of 21 to 30 ms on 6 flac and wav tracks were the visible tail. Shifting every non-mp3 grid by -10 ms, dev sweep (grid within 20 ms of 412, Lame3.100 rule on): 0 ms 306, -6 326, -8 329, -10 330, -12 330, -14 329. A plateau from -8 to -14, so -10. (On the first 249 dev tracks: +5 ms 173, 0 ms 179, -5 186, -10 188, -15 187.) This does not contradict the Q15 finding "a global shift of 0 is best": that sweep moved the mp3 grids too, which already get their own delay.
3. **Keep native tempo of 150 to 185 BPM (+2 BPM, +1 grid on dev).** If the model's own median-IBI tempo is in 150 to 185 it is no longer folded (Amir Bresler - Fish, 175.0, was 87.5). The fold to 75 to 150 stays for everything else. Dev: 0 tracks harmed; test: Rio Corrente now x1.02 instead of x0.5.
4. **Integer snap reach 1.5 BPM (+2 BPM +-0.1 on dev, grid unchanged).** Candidates are tried nearest first and accepted at 97% of the unsnapped comb score, as before. Marginal: 2 of 412.
5. Runtime: no change. The shifts are constants; the snap loop adds up to 2 more candidates only on tracks where the nearest integer is rejected (fit stage median 189 ms before, 195 ms after, 205 test tracks); the onset envelopes used for the half-beat study are not part of `ffa`. An `ffa` run on 4 tracks reproduces the grid-refit numbers exactly (BPM 4/4 within 0.1 and the same final beats).

## What did not help (negative results)

- **Half-beat phase (12 dev tracks and 7 test tracks whose median offset is within 70 ms of half a beat).** Nothing separated them from the 362 good dev tracks. Off-beat over on-beat energy (`onset.rs`: spectral flux at 11 kHz, bands 21 to 215 Hz, 215 Hz to 1.4 kHz and 1.4 to 5.5 kHz, peak within +-30 ms): half-off ratios 0.4 to 2.0 against a median of 0.64 for the rest; moving the grid by half a beat whenever the ratio is above 1.0 fixes 4 of 12 and breaks 26 of 362 (low band), 5 of 12 against 19 of 362 (mid band); no threshold has more fixes than breaks. The Beat This! beat and downbeat activations at the half-beat position are near zero for both groups (median 0.00 for the good tracks; 0.0 to 0.8 for the half-off ones). A rule built on the first strong onset of the track (its phase against the model's grid) separates nothing either: 4 of 12 fixed against 15 of 362 broken at the best threshold, and in the first 204 dev tracks the first strong low-band onset sat on the model's own grid in 6 of the 7 half-off tracks found by then, so Rekordbox put its grid on the off-beat of the audible pulse there. No rule found; no change made. Tools: `q30-hb-analyze.py`, `q30-hb-first-onset.py`, `grid --onset-dir`.
- **Wider fold (`lo` other than 75).** Dev, all fixes applied: `lo` 70 gives 79 x0.5 errors and BPM +-0.1 of 72.3% (tracks at 140 to 150 fold down), `lo` 72 gives 85.4%, `lo` 80 gives 89.8% (equal to 75) with 4 x2 errors and one more grid hit (332 against 331 at that point). No advantage. The learned preference (periodicity strength at T against 2T) was not built: dev has no x2 errors and one x0.5 error, and the 5 dev tracks with a reference above 150 are 1 fixed by keep-native and 4 missed at ratios 0.5 to 0.8 of the reference (159 vs 80, 154 vs 102, 167 vs 125, 169 vs 135: the model's tempo is not an octave of Rekordbox's). The beat activation at the half-beat position was tried as the cue on the first 249 dev tracks (Fish 0.52, tracks at 80 to 81 BPM 0.04 to 0.97): no separation.
- **Integer snap beyond 1.5 BPM** or a lower ratio (0.9, 0.95, 0.93): identical to 1.5 at 0.97 on dev.

## Categories of the remaining wrong-BPM tracks (test, 25 beyond +-0.1, 15 beyond +-1)

| Category | Test tracks (id: estimate vs reference) |
|---|---|
| Above the 150 prior, model reports half (kept at x0.5; keep-native does not apply because the model's native tempo is already 85 or 91) | 240: 85.0 vs 170.1; 423: 91.0 vs 181.8 |
| Above 150, now octave-correct (not within 1) | 246: 165.0 vs 162.2 |
| Ratio 1.3 to 1.7 (3:2 and 4:3, broken beat, polyrhythm) | 36: 131.9 vs 78.5 (1.68); 9: 145.0 vs 99.0 (1.46); 168: 136.0 vs 95.2 (1.43); 243: 140.0 vs 106.5 (1.31); 6: 111.3 vs 88.2 (1.26) |
| Ratio 1.1 to 1.14 (different pulse) | 12: 93.6 vs 82.2; 255: 144.0 vs 130.0 |
| Within 4% (tempo wobble, or Rekordbox's tempo is itself an average of a varying tempo) | 144, 273, 402, 501, 507 (ratio 0.98 to 1.04) |

Dev (26 beyond +-1: 24 other, 1 x0.5, 1 x2/3): 18 of the 26 are within 4% of the reference (the same tempo-wobble class), 4 are references above 150 that the model reports at 0.5 to 0.8 of the value, 4 sit at ratio 1.2 to 1.8. No x2 error on either split, so a better octave decision has little to find; the remaining misses are broken-beat material and tempo wobble that a prior does not fix (the Q13 list: Falaise, Shopping List, Primes, Detroit People Mover and Arctic Horizon come back unchanged).

## Grid misses left (BPM within 1, grid beyond 20 ms)

Test: 29 tracks, by cause: drift 9, outliers 8, half-beat 7, constant offset 4, offset + wobble 1. Dev: 55 of 386 tracks with BPM within 1: constant offset 18, drift 14, half-beat 12, outliers 7, offset + wobble 4 (the mp3 files without Xing/Info header are among the offsets). Next candidates: the no-header mp3 files (4 dev, 4 test; offsets +30, +10, +3, +11 on dev), drift (the model's tempo is off by 0.1 to 0.8 BPM on 14 dev tracks), and a half-beat rule that needs a signal other than onset energy.

## Caveats

- Rekordbox agreement only. The -10 ms PCM shift and the Lame3.100 frame are empirical fits to Rekordbox's timeline, not derived from a spec; other encoders without an Info frame (4 dev, 4 test tracks) are still treated as zero delay.
- The dev set is not a random sample: it is the tracks that have audio, 104 of them were also used for the Q15 tuning of the snap and scan settings.
- The test split was scored once with the final settings. Q26's logits (`benchmark/results/q26/dump205`) were re-fitted, not re-run through the model; with 44.1 kHz sources the logits do not change, and an `ffa` check of 4 tracks gives the same result as the re-fit.

## Reproduce

All paths relative to the repo root (logits cache: `benchmark/results/q30/`, gitignored: `ref.json` = the Q13 reference, `dump/` dev logits and Q15 test logits, `test/` the Q26 test dumps).

```bash
cd prototyping/analysis-packaging && ./fetch-models.sh && cargo build --release && cd ../..
S=benchmark/scripts; R=benchmark/results/q30
python3 -I $S/q30-inventory.py $R/ref.json $R/dev-todo.txt --have $R/dump
target/release/ffa is at prototyping/analysis-packaging/target/release/ffa:
prototyping/analysis-packaging/target/release/ffa --workers 7 --threads 1 --no-key --no-waveform --brief \
  --reference $R/ref.json --dump $R/dump --list $R/dev-todo.txt --out $R/devrun.out
$S/q30-freeze.sh $R/dump $R/frz2
$S/q30-sum.sh F2 $R/frz2 dev                       # Q30 final settings (defaults of q30-score.sh)
$S/q30-sum.sh F0 $R/frz2 dev --mp3-rule enc+header --pcm-shift-ms 0 --keep-native 0 0 --snap-wide 0.5   # Q26 settings
$S/q30-sum.sh test $R/test test                    # test, final settings
python3 -I $S/q30-tag-table.py $R/last-F2.json     # tag x offset
python3 -I $S/q30-bpm-misses.py $R/last-test.json $R/ref.json
$S/q30-sum.sh hb $R/frz2 dev --onset-dir $R/onsets && python3 -I $S/q30-hb-analyze.py $R/last-hb.json
```

`q30-sum.sh` writes `last-<label>.json` next to the cache. The extra flags of `grid` added in Q30: `--pcm-shift-ms`, `--keep-native LO HI`, `--onset-dir`, `--mp3-rule enc+header+l3100`.
