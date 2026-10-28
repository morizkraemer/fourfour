# Rust analyzer: status and accuracy against the ship bar (O3), 2026-10-28

Branch `ov-analysis` (from `ov-1028`). Reference is Rekordbox throughout (master.db BPM, PQTZ grid and bar positions), so every number measures compatibility with Rekordbox, not musical correctness.

## Summary

| Metric | Bar | Rust (app path) before | Rust after this branch | Python backend |
|---|---|---:|---:|---|
| Grid within +-20 ms of Rekordbox, test split (n=205) | none set; AGENTS.md target line still says "47% now" | 161 = 78.5% | **165 = 80.5%** | no Rekordbox-aligned grid (phase 3 to 100 ms off, Q33 parity) |
| Grid within +-20 ms, constant-tempo references (n=189) | | 161 = 85.2% | 165 = 87.3% | |
| BPM within +-0.1 / +-1 (test) | | 87.8% / 92.7% | 87.8% / 92.7% | DeepRhythm 38.7% / 90.8% (Q9b, 573 entries, not paired) |
| Bar position matches Rekordbox (test, 175 beat-aligned tracks) | | 108 = 61.7% | unchanged | counts from beat 0 (see "Downbeat") |
| Key exact on Beatport test (n=199) | **60%** | 58.3% (cyanbox port) | unchanged | Essentia about the same (Q33) |
| Cues / energy | | none | none | 1 to 5 memory cues, energy 2 or 7 |

- The grid/BPM part of the ship bar ("Rust analysis meets the grid bar") has no number in the docs beyond the stale 47% line; at 80.5% (87.3% on constant grids) the Rust path is well past where Q13 started. The key bar is not met (58.3% vs 60%); key licensing is Merlin's call (key-licence-2026-10-27.md) and was not touched.
- The app's stored result equals the prototype re-fit: `ffa` output (the same `analyze_controlled` the app calls) scored 161/205, identical to Q30's published 161/205 and to re-fitting the dumped logits.
- The largest unmeasured gap is **bar position**: Rekordbox's bar 1 agrees with the model's downbeats on only 61.7% of beat-aligned test tracks, while "grid beat 0 is bar 1" agrees on 78.9%. That is a product decision, not a bug fix (below).

## 1. Backend status

- **Default**: Rust when model weights are found (`$FOURFOUR_MODELS_DIR`, `<app data>/models`, `prototyping/analysis-packaging/models`), otherwise Python (`pioneer-test-ui/src/analysis_backend.rs::choose_backend`). Setting `analysis_backend` or `FOURFOUR_ANALYSIS_BACKEND` overrides; a failed weight load falls back to Python with a log line.
- **Rust path complete for BPM, grid, bar positions, key and waveforms**: no `todo!`, `unimplemented!`, TODO or WIP markers in `fourfour-analysis-rs/` or the app's analysis modules; staged queue (waveform, key, grid) and the now-lane are merged.
- **Gaps**:
  1. **No cues, no energy** (`to_app_result` writes `cue_points: []`, `energy: None`). Python's versions are Lexicon ports and wait on a licence decision (Q33).
  2. **Weights are not bundled.** No `bundle.resources` entry; a fresh install has no weights and silently runs Python.
  3. **The Python fallback only works in a dev checkout.** `resolve_python` looks for `analysis/.venv` next to the source tree (`CARGO_MANIFEST_DIR`), else `python3` with no packages. On `dev` neither the main checkout nor this worktree has the venv (one exists in another worktree), so a weight-less install analyses nothing.
  4. **Truncated files fail the whole analysis.** `decode_mono` returns `unexpected end of file` from `next_packet()?` and throws away all audio decoded so far. Reproduced on cut copies of a wav and a flac (both fail; a cut mp3 decodes). In the dataset: 1 of 50 dev tracks (Boswick Gates - You Are The Reason.wav) and id 172 (mp3, Q13). Rekordbox and the player handle such files; the analyzer should keep what it decoded.
  5. No UI for the backend setting or cancel (commands exist).
  6. Downbeat choice disagrees with Rekordbox (below).

## 2. Accuracy (test split, manifest id % 3 == 0, 205 tracks with verified audio)

Run: `ffa --workers 4 --threads 1 --no-key --no-waveform` (beats and fit as in the app), 2.25 tracks/min at load average 45 to 50, peak RSS 2.7 GB. Logits dumped to `benchmark/results/o3/dump` (gitignored) for re-fits.

| Metric | Before (app today) | After |
|---|---:|---:|
| BPM within +-0.1 / +-0.5 / +-1 | 180 / 184 / 190 (87.8 / 89.8 / 92.7%) | 180 / 184 / 190 |
| Octave errors x2 / x0.5 / other | 0 / 2 / 13 | 0 / 2 / 13 |
| F-measure at 70 ms | 0.895 | 0.895 |
| Median offset, signed / absolute | +0.7 / 3.9 ms | +0.9 / 3.9 ms |
| **Grid within +-20 ms (every Rekordbox beat)** | **161 = 78.5%** | **165 = 80.5%** |
| Constant-tempo refs only (189) | 161 = 85.2% | 165 = 87.3% |
| Fit stage, median per track | 192 ms | 195 ms |

Remaining test misses after the fix (40): 15 with BPM beyond +-1 (broken beat and 3:2 pulses, two above 150 BPM reported at half, see Q30 table), 25 with BPM right: half-beat 7, outliers (dynamic or edited reference grids) 8, drift 5 (was 9), constant offset 4, offset + wobble 1.

Dev check (50 tracks, manifest id % 12 == 4, never used for tuning before; 49 scored, one truncated wav failed to decode): grid within 20 ms 43 -> 44 of 49, BPM within +-0.1 46 of 49 and +-1 49 of 49, unchanged. The `ffa` run there used the new code, and its stored grid scores the same 44/49 as the re-fit.

Key (cyanbox port as shipped, `cyankey` bin, 598 Beatport tracks): test 116/199 = **58.3%** exact, 72.9% exact + relative + fifth; dev 62.7%; all 61.2%. Same as Q46. Median detect 377 ms per track.

### Python backend

Not re-run. Its venv is not in this checkout, and the run limit (4 analysis threads, another worker building) was taken by the Rust runs. Earlier numbers: DeepRhythm BPM 38.7% within 0.1 and 90.8% within 1 (Q9b, integer BPM only); grid phase 3 to 100 ms off Rekordbox's on the 10-track parity set (Q33), so its 20 ms grid share is far below Rust's; it counts bars from beat 0.

## 3. Fix: integer snap must keep the comb score

**Gap.** The fit snaps the BPM to the nearest integer in two ways: within 0.05 BPM always, and up to 1.5 BPM if the whole-track comb score stays at 97% of the unsnapped fit. The first rule ignored the score. Rekordbox's BPM is not an integer on 23% of references (128.01, 130.04, 130.98, ...), and for those the unsnapped fit is right and the forced integer drifts 25 to 400 ms over the track. Comb ratios of the forced snaps on test: 0.84 (128.01), 0.51 (130.04), 0.64 (124.02); real integer tracks keep 0.97 to 1.00.

**Change** (`fourfour-analysis-rs/src/fit.rs`): every snap candidate needs `snap_ratio` (0.97, unchanged) of the comb score. `FitParams::snap_tol_unchecked` (default false) and `grid --snap-tol-unchecked` restore the old rule for reproduction; older reproduce commands in bpm-grid-2026-10.md now give the new behaviour unless that flag is added. No new parameter value was tuned: the ratio sweep 0.90 / 0.95 / 0.97 / 0.99 gave the same result on the first 106 test tracks, so 0.97 stays.

| | Before | After | Tracks changed |
|---|---:|---:|---|
| Test, grid within 20 ms | 161 / 205 | 165 / 205 | +4 (ids 21: 128.01, 72: 130.04, 477: 130.98, 621: 132.01), 0 lost |
| Dev, grid within 20 ms | 43 / 49 | 44 / 49 | +1 (id 472: 114.99), 0 lost |
| BPM within +-0.1 | unchanged | unchanged | |
| Fit time per track (median, test) | 192 ms | 195 ms | model + decode is about 95 s per track at this load, so throughput is unchanged |

Caveat: the mechanism was found on the first 106 test tracks (the snap debug ratios above), then confirmed on all 205 and on 49 dev tracks. There was no unused dev logits cache on the box (Q30's caches are gone), so the held-out split was looked at before the change. The change removes a rule rather than tuning one.

Unit tests in `fit.rs`: synthetic 128.00 still snaps to 128.0; synthetic 128.02 stays within 0.003; with `snap_tol_unchecked` it snaps (the old behaviour).

## Downbeat (bar position): decision needed

Scored on tracks whose grid matches Rekordbox beat for beat (BPM within 0.5, one bar phase covers 90% of matched beats). `o3-bar-rules.py`:

| Rule | Test (175) | Dev (45) |
|---|---:|---:|
| Model downbeat vote (app today, `service::bar_phase`) | 108 = 61.7% | 26 = 57.8% |
| Grid beat 0 is bar 1 (what Python does) | 138 = 78.9% | 40 = 88.9% |
| Beat 0 unless the model's top phase holds at least 80% of the votes | 140 = 80.0% | 40 = 88.9% |

Rekordbox's bar 1 sits on the first grid beat in about 80% of these tracks, so it mostly numbers bars from the start of the track. The disagreements with the model are not noise: on e.g. "Don't Sleep" (flac) all 226 model downbeats fall on one phase and Rekordbox is one beat off it. Either the model is musically right and Rekordbox (or a hand edit) is wrong, or the model misses the bar. Which one is right needs ears on the Mac; the code choice (Q33) was deliberate. Not changed here.

## Reproduce

```bash
prototyping/analysis-packaging/target/release/ffa --workers 4 --threads 1 --no-key --no-waveform \
  --reference benchmark/results/o3/ref.json --dump benchmark/results/o3/dump \
  --list benchmark/results/o3/test.txt --out benchmark/results/o3/test-before.jsonl
python3 -I benchmark/scripts/o3-score.py benchmark/results/o3/ref.json benchmark/results/o3/test-before.jsonl
python3 -I benchmark/scripts/o3-bar-rules.py benchmark/results/o3/ref.json benchmark/results/o3/test-before.jsonl
benchmark/scripts/o3-fit.sh test-before benchmark/results/o3/dump test --snap-tol-unchecked
benchmark/scripts/o3-fit.sh test-after benchmark/results/o3/dump test
python3 -I benchmark/scripts/o3-diff.py benchmark/results/o3/v-test-before.json benchmark/results/o3/v-test-after.json
prototyping/analysis-packaging/target/release/cyankey benchmark/manifests/beatport-edm-key-keyonly-clean-full.corpus.json \
  benchmark/datasets/beatport-edm-key/audio 4 benchmark/results/o3/key-rows.json
```

`ref.json` is the Q13 reference (`benchmark/results/q13/rb-grid-reference.json`); `test.txt` is the 205 fingerprint-verified test paths, `dev.txt` the 50 dev paths with id % 12 == 4. Weights: `fourfour-analysis-rs/scripts/fetch-models.sh`.
