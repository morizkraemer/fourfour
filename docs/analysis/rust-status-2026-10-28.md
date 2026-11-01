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

## O9: weights in the bundle, no silent empty analysis (branch `ov-weights`)

**Runtime.** Weight lookup order (`service::model_search_dirs`): `$FOURFOUR_MODELS_DIR`, `<app data>/models`, `<resource dir>/models` (the bundled copy), the dev `prototyping/analysis-packaging/models`. `BackendEnv::locate_weights` feeds it the Tauri resource dir. When no weights load and Python cannot `import fourfour_analysis`, `resolve_runner` returns `NO_BACKEND_ERROR`; the run command returns it as its `Err`, which the existing analysis status UI already shows.

**Bundling.** `pioneer-test-ui/tauri.bundle-weights.conf.json` adds `bundle.resources` (beat_this.onnx, mel_spectrogram.onnx, LICENSE-beat-this.txt into `models/`). It is a separate config on purpose: Tauri checks resource paths at compile time, so putting them in `tauri.conf.json` would break `cargo build` for everyone without the weights. Explicit paths (no globs) make a missing file fail the bundle. `fourfour-analysis-rs/scripts/bundle-app.sh [tauri build args]` fetches the weights if absent (sha256 checked for beat_this), refuses to continue if any file is missing or empty, then runs `cargo tauri build --config tauri.bundle-weights.conf.json`. The .onnx files stay gitignored (`prototyping/analysis-packaging/.gitignore`).

**Mac .dmg build.** `fourfour-analysis-rs/scripts/bundle-app.sh --bundles dmg` on the Mac (needs curl, cargo-tauri 2, node; if the CI flow adds `.github/tauri-ci-bundle.json`, merge the `bundle.resources` object into it or pass both configs). Installed app then finds the weights in `fourfour.app/Contents/Resources/models/`. Size impact: about +83.4 MB (83.16 MB beat_this + 0.27 MB mel), before dmg compression (ONNX float weights compress little).

**Licence.** Beat This! (CPJKU/beat_this README, "License"): "The code and the published model weights are released under the MIT license. Note that some of the training files are fully copyrighted or under limited Creative Commons licenses, and it is up to the user to assess whether this may impact their use case." The ONNX conversion (danigb/beat-this-rs) is MIT and retains both copyright notices; MIT requires shipping the notice, so `LICENSE-beat-this.txt` is bundled next to the weights. Open risk: the training-data note (weights trained on partly restricted data) is the authors' disclaimer, not a restriction on the weights.

## B15: bar 1 rule switch (branch `b15-bar1`, analysis target, not landed)

**Change.** `fourfour-analysis-rs/src/service.rs`: `enum BarOneRule { ModelVote, FirstBeat, FirstBeatUnlessModelStrong }`, selected by `pub const BAR_ONE_RULE` (now `FirstBeatUnlessModelStrong`) with `MODEL_STRONG_SHARE = 0.8`. `bar_phase` uses the constant; `bar_phase_by(rule, ...)` takes the rule explicitly. The old behaviour is one line: `BAR_ONE_RULE = BarOneRule::ModelVote`. The hybrid is the rule from "Downbeat" above: grid beat 0 is bar 1, unless the model's top downbeat phase holds at least 80% of the downbeat votes, then the model phase wins. Nothing else moves: BPM and grid come from the fit, the rule only picks which beat is numbered 1.

Why 0.8: the threshold sweep on test is flat from 0.7 to 0.9 (79.4 / 80.0 / 79.4%) and on dev 88.9 / 88.9 / 91.1%, so 0.8 is the middle of a plateau, not a tuned peak. Plain "first beat" is within one track of it on test (78.9%) and equal on dev (88.9%), so the hybrid's gain is small: it only keeps the model where it is nearly unanimous.

**Eval plumbing.** `ffa` JSONL (`Analysis::to_json`) now writes `bar_phase` (active rule) and `bar_phase_model_vote` (old rule), both from the Rust code. `o3-score.py` scores the app's real phase (`--bar-field bar_phase_model_vote` gives the old rule). `fourfour-analysis-rs/examples/bar_replay.rs` re-applies both Rust rules to an older ffa JSONL (beats, downbeats and mp3 delay are the inputs, so the result is exactly what the new code would write for the same grid). `benchmark/scripts/b15-bar-listen.py` makes the listen list.

### Results (Rekordbox as reference, `o3-score.py`)

| Run | Bar 1 right, old (model vote) | Bar 1 right, new (hybrid) | Grid within 20 ms, old / new | BPM within 0.1 / 1.0, old / new |
|---|---:|---:|---:|---:|
| Test, 205 tracks, ffa JSONL of the 10-28 run replayed through the Rust rules; 174 beat-aligned | 107 = 61.5% | **138 = 79.3%** | 161 / 161 (78.5%) | 180 / 190, same |
| Same, `o3-bar-rules.py` filter (175 aligned) | 108 = 61.7% | **140 = 80.0%** | | |
| Dev, 49 scored (`dev-after.jsonl` replayed); 45 aligned | 26 = 57.8% | **40 = 88.9%** | 44 / 44 (89.8%) | 46 / 49, same |
| Dev, fresh `ffa` end to end with this binary, first 26 of 50 tracks (stopped at the timebox); 25 aligned | 16 = 64.0% | **24 = 96.0%** | 24 / 24 of 26 | 25 / 26, same |

- Bar agreement of the whole set: test 52.2% -> 67.3% of all 205 tracks; grid within 20 ms AND bar right 97 -> 130 (47.3% -> 63.4%).
- Grid and BPM do not move because the rule runs after the fit. The fresh dev run confirms it: BPM equal and beat counts equal to the earlier run on all 25 shared tracks, 18 of them bit-identical, 7 within 2 to 3 ms (one-sample resample/decode differences from the perf branches since), and the same 24 tracks inside 20 ms. The 26th track (id 172, the truncated mp3 that failed before) now decodes and is one grid miss; it is not in the earlier 49.
- The test JSONL is the pre-snap-fix run (161 grid). With the snap fix the app is at 165 = 80.5% (see Summary); the bar rule does not touch it. The full 205-track fresh `ffa` run was not done: at load average 40 to 60 on `dev` one track took about 3 minutes of wall clock (0.35 tracks/min with 4 workers), so test would have taken 10 hours. The replay is exact for the rule, not for the 4 tracks the snap fix changed.
- Dev run speed was this slow because of box load; a single track's CPU cost is unchanged.

### Listen list (old vs new bar 1, for the Mac)

72 beat-aligned tracks (test + dev) get a different bar 1 from the two rules: the new rule matches Rekordbox on 59 of them, the old one on 13, none on neither. Times are the first beat each rule numbers 1, in the stored grid (seconds from file start); "Rekordbox" is its first bar-1 beat. Bar numbers repeat every 4 beats, so a time several beats off is the same bar position when the difference is a multiple of 4 beats.

| id | file | old bar 1 (s) | new bar 1 (s) | Rekordbox bar 1 (s) | matches Rekordbox |
|---:|---|---:|---:|---:|---|
| 39 | Messenger Vox.mp3 | 1.153 | 0.250 | 0.250 | new |
| 66 | 08. Nicolas Barnes - Shut It Down.flac | 0.578 | 0.094 | 0.094 | new |
| 81 | 80. Tripmastaz - Dreezy Benz Dubb.flac | 1.176 | 0.208 | 0.202 | new |
| 114 | 54. Guillaume & the Coutu Dumonts - Ubiquitous Gaze.flac | 1.001 | 0.049 | 0.047 | new |
| 123 | 03. Ilario Liburni - Tiny Little Squares (Original Mix).flac | 0.987 | 0.004 | 0.000 | new |
| 21 | Brooklyn Haunt.mp3 | 0.536 | 0.068 | 0.550 | old |
| 30 | She's A Replicant - Afriqua Remix.mp3 | 0.566 | 0.090 | 0.563 | old |
| 27 | And That Was The Way - Echospace's Shinjuku Sedative.mp3 | 1.975 | 0.359 | 1.972 | old |
| 57 | 04. Wouter de Moor - Afternoon Thing.flac | 0.932 | 0.000 | 0.936 | old |
| 72 | 39. Silverlining - Don't Forget Your Ambdo Knyper.flac | 1.793 | 0.409 | 0.004 | old |

In the first five the old rule puts bar 1 later than Rekordbox, which numbers it at the first beat; the last five are the regressions to listen to (the old model phase matches Rekordbox and the hybrid falls back to beat 0 because the vote is split). For id 72 our grid starts 0.405 s (about one beat) after Rekordbox's first beat, so "beat 0" is one beat off Rekordbox's bar 1 there; the model phase lines up modulo 4 beats. The question for ears: is the model's bar 1 the musical downbeat in the last five, and is Rekordbox's in the first five?

### Validation

`cargo build` clean; `timeout 40m cargo test --workspace`: 510 passed, 0 failed across all crates (new unit test `rules_pick_the_expected_phase_for_a_split_and_a_unanimous_vote` in `service.rs`; the existing bar-position tests pass unchanged).

```bash
python3 -I benchmark/scripts/o3-score.py benchmark/results/o3/ref.json new.jsonl --bar-field bar_phase_model_vote   # old rule
python3 -I benchmark/scripts/o3-score.py benchmark/results/o3/ref.json new.jsonl                                      # active rule
cargo run -p fourfour-analysis-rs --example bar_replay -- old-ffa.jsonl > replayed.jsonl
python3 -I benchmark/scripts/b15-bar-listen.py benchmark/results/o3/ref.json replayed.jsonl --max 10
```
