# Fast beat-model path (r94, 2026-11-03)

Question: can a cheaper beat-model path cut analysis time 2x+ on CPU while keeping grid/BPM quality?

Answer: **shorter chunks (500 frames = 10 s instead of 1500 = 30 s) give 1.8x tracks/min and 1.75x less CPU per track, with half the peak memory, and no measurable grid/BPM/bar loss.** The Beat This! small checkpoints are about as fast but lose 6-13 grid points. Not quite the 2x target alone; 2x+ needs chunk 500 plus something else (see "Next").

## Switches (branch `r94-fast-model`, default unchanged)

| Env | Default | Effect |
|---|---|---|
| `FOURFOUR_BEAT_CHUNK` | 1500 | frames per beat-model chunk (border stays 6 frames); also the CoreML session shape |
| `FOURFOUR_BEAT_SKIP_SILENCE` | off | `1`: a chunk whose mel is all zero gets -1000 logits without running the model |
| `FOURFOUR_BEAT_MODEL` | `beat_this.onnx` | beat model file in the models dir (app path, `service.rs`); `ffa --beat-model` does the same |

With the switches unset every constant is the old one. Evidence: dev split base with this build is 44/50 grid, 47/50 BPM 0.1, 41/46 bar, exactly R3/R24's numbers; `cargo test -p fourfour-analysis-rs` passes.

## Method

- Box `dev`, 16 cores, load 17-50 from other workers during every run (noted per row). `ffa` release, `--workers 4 --threads 4` (4 shared chunk threads; `--threads 1` gives only 1 chunk thread in total since the shared executor), `--no-key --no-waveform`, `nice 5`.
- Speed: CPU seconds per track (`/usr/bin/time` user+sys / tracks) is the load-robust number; wall tracks/min only from the back-to-back pair runs (8 dev tracks, base, c500, c500, base).
- Quality: `benchmark/scripts/o3-score.py` on ffa JSONL. Test split: the full base run was stopped at 109 tracks to save time, so test numbers are **paired** on the tracks both runs finished (`benchmark/scripts/r94-pair.py`). Dev split: all 50.
- Model weights: `~/.cache/r94-models` (not in git). small0/1/2 exported from the CPJKU checkpoints with the same `torch.onnx.export` settings as danigb's `ckpt2onnx.py` (opset 17, dynamic time axis), venv `~/.cache/r94-venv`.
- Licence: Beat This! code and published weights are MIT ("The code and the published model weights are released under the MIT license", CPJKU/beat_this README), same as the current weights.

## Results

Per-chunk microbench (`fourfour-analysis-rs/examples/chunk_bench.rs`, 1 thread, random input, median of 6 interleaved rounds, CPU ms per 1000 frames):

| Chunk frames | large | small1 |
|---:|---:|---:|
| 1500 | 4033 | 2247 |
| 1000 | 3140 (-22%) | 1740 |
| 750 | 2733 (-32%) | 1427 |
| 500 | 2200 (-45%) | 1220 |
| 300 | 1967 (-51%) | 1033 |

Per-frame cost falls with chunk length because self-attention is quadratic in the chunk length. Below 500 the gain flattens.

Dev split (50 tracks, all variants full):

| Variant | CPU s/track | Peak RSS | Grid 20 ms | BPM 0.1 | BPM 1 | Bar right / aligned | BPM changed | Load |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| base (1500) | 87.0 | 2598 MB | 44 | 47 | 50 | 41/46 | - | 17-34 |
| chunk 1000 | 70.9 (-19%) | 1796 MB | 42 | 45 | 50 | 40/46 | 3 | 29-37 |
| chunk 750 | 61.7 (-29%) | 1515 MB | 43 | 46 | 50 | 41/45 | 1 | 26-37 |
| **chunk 500** | **50.0 (-43%)** | **1349 MB** | **43** | **47** | **50** | **41/45** | 1 | 32-49 |
| chunk 300 | 44.9 (-48%) | 1189 MB | 42 | 45 | 49 | 40/44 | 3 | 27-49 |
| small1 | 50.1 (-42%) | 2536 MB | 38 | 44 | 50 | 36/43 | 4 | 26-34 |
| small0 | 49.2 (-43%) | 2562 MB | 31 | 45 | 50 | 36/41 | 3 | 25-35 |
| small2 | not run (small0/1 already lose 6-13 grid points) | | | | | | | |

Test split, paired:

| Variant | Paired tracks | Grid 20 ms base -> variant | BPM 0.1 | BPM 1 | Bar right |
|---|---:|---|---|---|---|
| chunk 500 | 100 (first 100 of test.txt) | 80 -> **81** | 87 -> 87 | 89 -> 89 | 64 -> 64 |
| chunk 750 | 75 (stopped at timebox) | 62 -> 60 | 68 -> 65 | 69 -> 68 | 46 -> 45 |
| small1 | 109 | 88 -> **77** | 96 -> 94 | 98 -> 100 | 72 -> 69 |

Speed pairs (8 dev tracks, back to back, load 26-42):

| Run | Wall tracks/min | CPU s/track | First result | Peak RSS |
|---|---:|---:|---:|---:|
| base 1 | 1.73 | 86.6 | 39.5 s | 2345 MB |
| chunk 500 1 | 3.23 | 49.5 | 22.9 s | 1096 MB |
| chunk 500 2 | 3.16 | 49.2 | 25.6 s | 1072 MB |
| base 2 | 1.72 | 85.4 | 44.1 s | 2269 MB |

Chunk 500: **1.86x tracks/min**, 1.75x less CPU, first result 1.7x sooner, peak RSS -53%. The 100-track test run alone: 3.01 tracks/min, 48.9 CPU s/track.

Digital-silence skip: implemented behind the switch, run stopped unmeasured (the queue ran out of time). It can only matter for chunks that are exactly zero for the whole chunk; with 10 s chunks that is more likely than with 30 s, but DJ tracks rarely have 10 s of digital silence. Expect low single-digit percent at best; not a priority.

## Why c750 looks worse than c500

On dev c750 and c500 are equal; on test c750 lost 2 grid / 3 BPM on 75 tracks while c500 gained 1 on 100. Seven to ten tracks change BPM by more than 0.01 for every chunk length, and the within-tolerance counts move by one to three tracks either way. Treat +-3 on 100 tracks as noise; c500 is not proven better than c750, only not worse than base.

## Recommendation

1. Make 500-frame chunks the default after one confirmation run: the full 205-track test split for base and c500 on a quieter box (or the Mac), interleaved, plus the `ort`/CoreML path with `FOURFOUR_BEAT_CHUNK=500` (the CoreML session compiles for the chunk length; its cache key already includes it). Gate: grid within 20 ms and BPM 0.1 on test not lower than base by more than 2 tracks. Bump `ANALYSIS_VERSION` when the default changes, so cached analyses are redone.
2. Drop the small checkpoints: same speed as chunk 500 with 6-13 grid points lost (as in the October test).
3. Do not go below 500: 300 frames gains only 5% more and starts losing BPM.

## Confirmation (r96, 2026-11-03)

Branch `r96-chunk500`. Gate (grid and BPM 0.1 each within 2 tracks of base on test, no worse than -2 on dev): **passed**.

Quality, `o3-score.py`, ffa `--workers 2 --threads 2` on the box (base and c500 ran at the same time, load 13-25, test base split in two halves because of the run limits):

| Split | Variant | Grid 20 ms | BPM 0.1 | BPM 1 | Bar right | Grid + bar | CPU s/track |
|---|---|---:|---:|---:|---:|---:|---:|
| test 205 | base 1500 | 167 | 180 | 190 | 138 | 133 | 80.3 |
| test 205 | chunk 500 | **167** | **180** | 192 | 138 | 135 | 45.8 |
| dev 50 | base 1500 | 44 | 47 | 50 | 41/46 | 40 | 81.2 |
| dev 50 | chunk 500 | 43 | 47 | 50 | 41/45 | 40 | 46.8 |

BPM differs by more than 0.01 on 22 of the 205 test tracks, but the tolerance counts do not move. Dev matches r94 (44 -> 43).

Mac `mbp` (M5 Pro, 24 GB, AC, headless, `nice -n 10`, HOME scratch, no Pioneer data touched), dev split, same code:

| App path (`bench_analysis`, 48 unique tracks, 6 workers) | Tracks/min | First result | Peak RSS |
|---|---:|---:|---:|
| CoreML 1500 | 31.5 | 5.2 s | 6.3 GB |
| CoreML 500 | 25.9 (-18%) | 3.6 s | 2.7 GB |
| rten 1500 | 15.9 | 5.8 s | 6.1 GB |
| rten 500 | 24.9 (+57%) | 3.9 s | 2.3 GB |

`ffa` (2 workers) agrees: CoreML 31.0/30.2 -> 22.1/22.9 tracks/min (RSS 4.9/4.4 -> 2.1/2.2 GB), rten 12.7/13.0 -> 19.7/19.4 (3.1/3.2 -> 1.7/1.3 GB).

- CoreML output equals rten output at the same chunk length: 49/49 tracks, every beat time identical, BPM within 0.01 (R24 criterion holds at 500 too). Mac dev scores: 1500 grid 44, BPM 47; 500 grid 43, BPM 47, equal on CoreML and rten.
- **CoreML is slower at 500**: three times as many calls, and its per-call cost does not fall with the chunk length the way rten's does. It trades 18% speed for 57% less memory and a 30% sooner first result.

Decision in code: `DEFAULT_CHUNK_SIZE` is 500, except 1500 in builds with the `coreml` feature on macOS (the shipped Mac app), so the Mac path is unchanged. `FOURFOUR_BEAT_CHUNK` still overrides. Making the Mac 500 as well is one constant (`COREML_CHUNK_SIZE` in `bt.rs`) and is a speed against memory call for Merlin.

Superseded by R158 (2026-11-07, Merlin: ship): 500 on every build, and the Mac defaults to the int8 model on rten with prepacked weights. See "R158 Mac analysis ship" in `docs/architecture/perf-run-2026-11.md`.

`ANALYSIS_VERSION` 1 -> 2. It only feeds `analysis_key`, which keys `analysis-failures.json` (the memo of tracks that failed analysis): after the bump the memo is dropped once and failed tracks are retried. It does not re-analyse the library and does not touch stored results, so existing tracks keep their 1500-chunk analyses (grid differences are within the 1-3 track noise above). The earlier "cached analyses are redone" note was wrong. Re-analysis of existing tracks stays the user's action.

## Next (to reach 2x+)

- Chunk 500 leaves the model at about 85% of analysis CPU. Small1 at chunk 500 would be about 3x the base model speed but carries the small-model quality loss; not recommended.
- Remaining model cost is mostly the weight MatMuls, which int8 could cut (r16 measured -11% per chunk at 1500 frames, where attention was a bigger share; at 500 frames the weight share is larger, so int8 may now pay more). Worth one re-measure at chunk 500 with quality scored.
- First-result latency: shorter chunks already halve it; ordering visible tracks first is the scheduling worker's lane.

## int8 at 500 (r97, 2026-11-03)

Branch `r97-int8-500` (from `r96-chunk500`). Question: does a dynamic-int8 beat model give a real gain at chunk 500 on rten, with no quality loss? Gate: test grid 20 ms and BPM 0.1 not lower than fp32@500, dev grid at most -1, gain >= 15% tracks/min.

**Answer: platform dependent. On the x86 box: no real gain (-10% CPU in `ffa`, about 0% in the app bench) and -2 grid / -2 BPM on test, so the gate fails. On the Mac (M5 Pro, rten): +43% to +60% tracks/min, -37% CPU, and test grid +1 / BPM +1, so the gate passes there. Default stays unchanged; int8 is opt-in through `FOURFOUR_BEAT_MODEL`.**

### The model

- Tool: `fourfour-analysis-rs/scripts/quantize_beat_int8.py` (onnxruntime `quantize_dynamic`, MIT; offline only, never in the app). It quantizes the 62 MatMuls with constant weights to int8 (per-channel, s8); attention (activation x activation), Conv and everything else stay f32. 83 MB -> 23 MB.
- Files live in `~/.cache/r97-models/` (not committed): `beat_this_q8sc.onnx` is the scored one; `q8s` (per-tensor) and `q8all` (MatMul + Conv) were only microbenched.
- Regenerate (venv `~/.cache/r97-venv` with `onnx onnxruntime numpy`):

```bash
~/.cache/r97-venv/bin/python fourfour-analysis-rs/scripts/quantize_beat_int8.py ~/.cache/fourfour-models/beat_this.onnx ~/.cache/r97-models/beat_this_q8sc.onnx --per-channel
FOURFOUR_BEAT_MODEL=beat_this_q8sc.onnx   # file next to mel_spectrogram.onnx; ffa: --beat-model beat_this_q8sc.onnx
```

- rten gotchas found: u8 weights (`--weight-type u8`) produce no output (every track fails); the default `quant_pre_process` optimizer renames a Split output to the node's own name and rten then fails with `Missing input "/model/task_heads/Split"`, so the script passes `skip_optimization=True`.

### Per-chunk microbench (`chunk_bench`, 1 thread, random input, median of 6)

| Host | Frames | fp32 | q8 per-tensor | q8 per-channel | q8 + Conv |
|---|---:|---:|---:|---:|---:|
| box (Xeon Gold 6130, AVX512, no VNNI), CPU ms | 500 | 990 | 950 (-4%) | 930 (-6%) | 910 (-8%) |
| box | 1500 | 4670 | 4610 (-1%) | 4460 (-4%) | - |
| Mac M5 Pro, wall ms (load about 6) | 500 | 331 | 199 (-40%) | 199 (-40%) | - |
| Mac | 1500 | 1444 | 1041 (-28%) | 1039 (-28%) | - |

The weight share is larger at 500 as predicted, but the x86 int8 kernel in rten barely beats its f32 one on a CPU without VNNI; the Mac has the int8 dot-product units. r16's -11% at 1500 does not reproduce on the box (-4%, same direction, smaller).

### Quality (`o3-score.py`, chunk 500, rten)

| Host | Split | Variant | Grid 20 ms | BPM 0.1 | BPM 1 | Bar right | Grid + bar |
|---|---|---|---:|---:|---:|---:|---:|
| box | test 205 | fp32 | 167 | 180 | 192 | 138 | 135 |
| box | test 205 | int8 | 165 (-2) | 178 (-2) | 192 | 136 | 133 |
| box | dev 50 | fp32 | 43 | 47 | 50 | 41 | 40 |
| box | dev 50 | int8 | 43 | 46 | 50 | 41 | 40 |
| Mac | test 205 | fp32 | 167 | 180 | 192 | 138 | 135 |
| Mac | test 205 | int8 | 168 (+1) | 181 (+1) | 192 | 139 | 136 |
| Mac | dev 50 | fp32 | 43 | 47 | 50 | 41 | 40 |
| Mac | dev 50 | int8 | 43 | 47 | 50 | 41 | 40 |

fp32 numbers equal r96 on both hosts. The same int8 file scores differently on x86 and ARM (the int8 arithmetic is not bit-identical). The x86 -2/-2 on 205 tracks is at the +-3 noise level from r94 but below the "not lower" gate.

### Speed

Box, `ffa --workers 2 --threads 2`, fp32 and int8 run at the same time, load 8-47 (the int8 run finished earlier, so its load was lower at the end):

| Split | Variant | CPU s/track | Tracks/min | First result | Peak RSS |
|---|---|---:|---:|---:|---:|
| test 205 | fp32 | 46.3 | 2.15 | 12.8 s | 810 MB |
| test 205 | int8 | 41.5 (-10%) | 2.38 (+11%) | 16.7 s | 769 MB |
| dev 50 | fp32 | 46.1 | 2.48 | 24.5 s | 735 MB |
| dev 50 | int8 | 42.1 (-9%) | 2.75 (+11%) | 22.1 s | 666 MB |

Box, `bench_analysis --limit 8 --threads 2`, fp32 and int8 simultaneously, two rounds (app path, load 17-75): fp32 2.31 / 2.44 tracks/min, CPU 491 / 467 s; int8 2.37 / 2.43, CPU 470 / 461 s; first result 31-35 s for both; RSS 437-535 MB. In the app path the box gain is within noise (-3% CPU).

Mac, `ffa --workers 2 --threads 6`, rten, chunk 500, fp32 then int8 back to back (shared Mac, load 3-12):

| Split | Variant | Tracks/min | CPU s (user+sys) | First result | Peak RSS |
|---|---|---:|---:|---:|---:|
| dev 50 | fp32 | 20.75 | 893 | 3.3 s | 1.59 GB |
| dev 50 | int8 | 34.29 (+65%) | 554 (-38%) | 2.3 s | 1.32 GB |
| test 205 | fp32 | 19.46 | 3918 | 2.0 s | 1.43 GB |
| test 205 | int8 | 31.13 (+60%) | 2501 (-36%) | 1.3 s | 1.96 GB |

Mac, `bench_analysis` (48 unique dev tracks, 6 workers, `FOURFOUR_BEAT_RUNTIME=rten`, chunk 500), 5 fp32 and 5 int8 runs interleaved: fp32 30.6 / 23.4 / 27.4 / 20.7 / 23.5 tracks/min, int8 30.8 / 30.0 / 46.6 / 33.6 / 35.5. Medians 23.5 -> 33.6 (+43%), user CPU 1498 -> 950 s (-37%), first result about 4.2 -> 3.1 s, peak RSS about 2.4 GB for both. The Mac was shared with other work, so the spread is wide; the worst int8 run ties the best fp32 run.

### Decision

- Wiring: nothing new needed. `FOURFOUR_BEAT_MODEL` (r94) already selects the file; default stays `beat_this.onnx`. `analysis_key` already includes the beat model file size.
- Linux/Windows x86 (no VNNI): do not ship int8; no real gain, small quality loss.
- Mac: worth a decision. rten int8 at 500 on the Mac runs about as fast as CoreML at 1500 did in r96 (31.5 tracks/min in the app bench, 4.9-6.3 GB RSS) at about 2.4 GB, with a 23 MB model instead of 83 MB; CoreML at 500 was slower (25.9). The comparison across sessions is noisy: a same-session Mac pair (CoreML 1500 vs rten int8 500, `ffa` and `bench_analysis`, test-split scores) is the next step before changing the shipped Mac default. Shipping it also needs the model in the app bundle and the models fetch script, and a per-platform choice (x86 keeps fp32).
- Not checked: Linux Arm and Windows; only beat/BPM quality was scored, not key.

## r98 Mac pair (2026-11-04)

Branch `r98-mac-pair` (from `r97-int8-500`, no code change). Same-session Mac pair on `mbp` (M5 Pro, 24 GB, AC): (a) the shipped Mac path, `coreml` feature build, chunk 1500, fp32; (b) the same binary with `FOURFOUR_BEAT_RUNTIME=rten FOURFOUR_BEAT_CHUNK=500 FOURFOUR_BEAT_MODEL=beat_this_q8sc.onnx` (r97 int8). `bench_analysis --list dev` (50 paths, 49 analysed: one wav fails tag read in both), default 6 workers, `nice -n 10`, `/usr/bin/time -l` for CPU and RSS (the bench's own RSS and CPU fields read 0 and NaN on macOS), five runs each, interleaved a b a b. Scoring: `ffa --workers 2 --threads 6` and `o3-score.py` on the same builds.

Load on the Mac: 3-4 before the first run (Chrome, WindowServer, Tailscale; someone was using the machine), 6-29 during (the benchmark's own threads count in that; run 1 started at about 7). An audio copy from the box to the Mac ran in the background during the first runs (about 1 MB/s through a Tailscale relay, negligible CPU). Run 1 was the quietest and has the biggest gap; runs 2-5 are consistent with each other.

| App path, `bench_analysis`, 5 runs, median | Tracks/min | Min / max | CPU s (user+sys) | First result | Peak RSS |
|---|---:|---|---:|---:|---:|
| (a) CoreML fp32, chunk 1500 | 26.6 | 26.1 / 31.8 | 368 | 6.7 s | 6.87 GB |
| (b) rten int8, chunk 500 | 32.6 (+22%) | 32.5 / 40.7 | 988 (2.7x) | 4.2 s | 2.44 GB |

Per round (b / a): 1.28, 1.20, 1.22, 1.23, 1.25. CoreML runs ran 627 chunks on CoreML and 40 on rten (the short tail chunks), no fallback.

| Quality (`o3-score.py`) | Grid 20 ms | BPM 0.1 | BPM 1 | Bar right |
|---|---:|---:|---:|---:|
| dev 50, (a) CoreML 1500 | 44 | 47 | 50 | 41 |
| dev 50, (b) rten int8 500 | 43 (-1) | 47 | 50 | 41 |
| test 126 of 205, (a) | 94 | 105 | 112 | 72 |
| test 126 of 205, (b) | 95 (+1) | 106 (+1) | 114 | 72 |

`ffa --workers 2 --threads 6` (same session, same load): dev (a) 32.1 tracks/min, 236 CPU s, first 6.1 s, 4.8 GB; (b) 22.4, 812 CPU s, 2.9 s, 1.4 GB. test-126 (a) 27.4, 510 CPU s, 3.9 s, 5.1 GB; (b) 28.4, 1676 CPU s, 2.6 s, 1.9 GB. With only 2 track workers rten does not keep the cores busy the way CoreML does, so the ffa speed gap does not follow the app path.

Caveats:
- **Test is a 126-track subset, not 205.** The box to Mac link is a Tailscale relay at about 1 MB/s; the full 8.7 GB did not fit the timebox. The subset is the 126 smallest test files (shorter tracks), the same tracks for both variants. Dev 50 is complete.
- **CPU is 2.7x.** `user+sys` for CoreML leaves out the Neural Engine and GPU work, so this is not an energy number, but rten int8 loads all cores for the whole run (about 11 cores busy against about 4 for CoreML). On a Mac that is also showing the UI this may cost more in fan, battery and UI headroom than the RSS saving buys.
- Both variants differ from the shipped app only by the runtime switch; the chunk and model are env overrides, not the code defaults.

Gate: tracks/min >= CoreML minus 5% (pass, +22%); test grid 20 ms and BPM 0.1 not lower (pass on the 126-track subset, +1/+1; dev grid -1, same as the chunk-500 effect in r96); RSS lower (pass, 2.44 against 6.87 GB).

Verdict: int8-rten-500 passes the Mac-default gate (+22% tracks/min, 64% less RSS, 37% sooner first result, quality equal within 1 track), with two open points before shipping: the full test-205 score, and the 2.7x CPU load (decide against battery and UI feel, or run the app with fewer analysis workers on the Mac).

## Reproduce

```bash
CARGO_TARGET_DIR=~/.cache/r94-target cargo build --release --manifest-path prototyping/analysis-packaging/Cargo.toml --bin ffa
benchmark/scripts/r94-run.sh dev-base dev
benchmark/scripts/r94-run.sh dev-c500 dev FOURFOUR_BEAT_CHUNK=500
benchmark/scripts/r94-run.sh dev-small1 dev MODEL=beat_this_small1.onnx
python3 -I benchmark/scripts/r94-pair.py benchmark/results/o3/ref.json benchmark/results/r94/test-base.jsonl benchmark/results/r94/test100-c500.jsonl
CARGO_TARGET_DIR=~/.cache/r94-target cargo build --release -p fourfour-analysis-rs --example chunk_bench
~/.cache/r94-target/release/examples/chunk_bench 6 1500,1000,750,500,300 beat_this.onnx beat_this_small1.onnx
```

Results JSONL in `benchmark/results/r94/` (gitignored).

## r102 chunk stitching (2026-11-04)

Question: can a cheaper stitch win back the BPM results that chunk 500 loses against chunk 1500 (r101: tracks 144, 231 and 528 go wrong; 42 goes right), keeping at least 30% less CPU per track?

Answer: **no variant passes the gate.** Better stitching removes the chunk-border error completely, but that error is only about a tenth of the difference between the 500 and 1500 logits; the rest comes from the shorter context and no overlap can recover it. The best variant (overlap 72 with a linear cross-fade) brings two of the three tracks back, loses another one, and ends where chunk 500 was: test BPM 0.1 184 (1500: 186), grid 168 (171). The default stays 500 frames, border 6, no overlap (Mac CoreML 1500, as before).

Baseline: branch `r102-chunk-stitch` = `r101-bpm-misses` with `HALF_TIME_BAND = (0.0, 0.0)` (the half-time rule is parked; the unit test now sets the band explicitly). The 1500 reference is the October full-model dump (`benchmark/results/o3/dump`, 254 of the 255 test + dev tracks; dev track 172 has none) re-fitted with the current fit (`grid_refit`): test 171 grid / 186 BPM 0.1 / 194 BPM 1, which are exactly the r79 numbers in the r101 doc; dev (49 tracks) 44 / 46 / 49.

### Switches (new, all optional)

| Env | Default | Effect |
|---|---|---|
| `FOURFOUR_BEAT_BORDER` | 6 | frames dropped at each end of a chunk (valid region = chunk - 2 x border) |
| `FOURFOUR_BEAT_OVERLAP` | 0 | extra frames the valid regions of neighbouring chunks overlap (the chunk step shrinks by this) |
| `FOURFOUR_BEAT_BLEND` | 1 if overlap > 0 | cross-fade the overlap linearly (weights ramp over the overlap) instead of letting the earlier chunk win |

With the three unset the logits are the r101 ones; unit tests check that every stitching mode returns position-consistent chunk logits unchanged for track lengths from 100 to 13790 frames, and that the cross-fade rises monotonically across the overlap. A hard-cut overlap on a track shorter than the step exposed a negative start for the last chunk (the original geometry cannot reach it); the last start is now clamped to the first chunk's.

### Why 144, 231 and 528 flip

Beat logits of the chunk-500 run against the 1500 dump on 39 tracks (the 3 flips plus 36 spread over test and dev; 727k frames), mean absolute difference by distance to the nearest 500-chunk border:

| Frames from a border | 0-6 | 6-25 | 25-60 | 60-120 | over 120 | All |
|---|---:|---:|---:|---:|---:|---:|
| chunk 500, border 6 | 1.70 | 1.48 | 1.23 | 1.01 | 0.91 | 1.043 |
| chunk 500, border 25 | 1.42 | 1.30 | 1.14 | 0.98 | 0.90 | 1.005 |
| chunk 500, overlap 72, blend | 0.97 | 1.00 | 1.03 | 0.99 | 0.91 | 0.963 |
| chunk 500, overlap 88, blend | 0.98 | 0.96 | 1.00 | 0.97 | 0.91 | 0.950 |
| chunk 640, border 6 | 1.60 | 1.44 | 1.18 | 0.96 | 0.82 | 0.938 |

- The border does hurt: the logits within 25 frames of a border are 60 to 90% further from the 1500 logits than the chunk middle. A cross-fade of 72 or more frames flattens that completely.
- It does not matter much for the result: the middle of a chunk is still about 0.9 logits away from 1500 (0.82 at 640), so the 3.5% of beat peaks that exist in only one of the two runs hardly move (1098 only in 1500 for chunk 500, 1055 with the cross-fade, 1081 with overlap 88). Beat This! reads the whole chunk through self-attention; a 10 s window simply sees less context than a 30 s one, and the 1500 border artefacts (every 1488 frames) are in the reference too.
- The three tracks sit on thin margins. 231 and 528 (reference 128.00 and 118.02): the snap and fold search takes a neighbouring comb peak on slightly different logits (as r101 found); 528 is 15 ms off at 1500 and 294 ms at chunk 500. 144 (reference 127.40, a 3:2 track): chunk 1500 gives raw 129.47 which the fit pulls to 127.386; border 6, border 25 and chunk 640 land on another tempo (170, 113, 172) and only the overlap-72 and overlap-88 cross-fades keep it (127.388, 127.394), so it is a coin flip on the model's tempo, not a stitch defect. Which tracks flip changes with every variant (201, 507 with border 25; 477 with chunk 640; 540 and 525 with overlap 72): each variant moves 1 to 3 right/wrong calls per 40 tracks in both directions.

### Variants

Subset S39 = tracks 144, 231, 528 + every 7th test track + every 6th dev track (31 test + 8 dev), all variants through `ffa` (2 workers x 2 threads, base and variant runs at the same time, box load 14 to 37). 1500 on the same 39: grid 33, BPM 0.1 36, BPM 1 36. CPU is user + sys per track from `/usr/bin/time`; the "8-track pair" column is the first 8 tracks of S39 run back to back (1500 and overlap 72 together, then 500 and 640 together), the better speed number (the two rounds ran at load 14 to 18; the reductions use the 1500 run of the first round); the S39 column was measured at different loads and is only good to about 10%.

| Variant (chunk / border / overlap) | CPU s/track S39 | CPU 8-track pair | Grid 20 ms | BPM 0.1 | BPM 1 | Bar right | Right -> wrong vs 1500 (BPM 0.1) | Wrong -> right | Mean abs logit diff |
|---|---:|---:|---:|---:|---:|---:|---|---|---:|
| 1500 / 6 / 0 (reference) | 80.3 (r96) | 69.2 | 33 | 36 | 36 | - | - | - | - |
| 500 / 6 / 0 (current default) | 47.6 | 39.7 (-43%) | 31 | 33 | 35 | 26 | 3: 144, 231, 528 | 0 | 1.043 |
| 500 / 25 / 0 | 51.2 | - | 31 | 34 | 35 | 25 | 2: 144, 201 | 0 | 1.005 |
| 500 / 6 / 72 blend | 52.9 (full run) | 45.0 (-35%) | 32 | 35 | 36 | - | 1: 231 | 0 | 0.963 |
| 500 / 6 / 88 blend | 56.3 | - | 32 | 35 | 37 | 26 | 1: 528 | 0 | 0.950 |
| 640 / 6 / 0 | 49.2 | 41.6 (-40%) | 31 | 33 | 36 | 27 | 3: 144, 231, 477 | 0 | 0.938 |

Chunk 500 with border 50 and 100, an overlap of 200, chunk 800 and 1000 were planned for a 3-track screen and not run: the box load made one 3-track run take 4.5 minutes, and a stride at or below 400 (border 50 and up) costs 22% or more per track, which already leaves too little of the 30% CPU margin. Border 36 (stride 416, the largest border the margin allows) was started and stopped for the full-set run below.

### Full test and dev for the best trade-off (500 / 6 / 72, cross-fade)

Full 205 + 50 through `ffa` (two halves at the same time, 2 workers x 2 threads each, 52.9 CPU s/track at load 15). Against the r101 chunk-500 numbers and the 1500 re-fit:

| Split | Variant | Grid 20 ms | BPM 0.1 | BPM 1 | Bar right (of all) |
|---|---|---:|---:|---:|---:|
| test 205 | 1500 (r79) | 171 | 186 | 194 | - |
| test 205 | 500 / 6 / 0 (r101) | 169 | 183 | 193 | 140 |
| test 205 | **500 / 6 / 72 blend** | **168** | **184** | **194** | 138 |
| dev 50 | 1500 (49 tracks) | 44 | 46 | 49 | - |
| dev 50 | 500 / 6 / 0 (r101) | 44 | 47 | 50 | 42 |
| dev 50 | **500 / 6 / 72 blend** | 44 | 47 | 50 | 42 |

Right -> wrong against 1500: BPM 0.1 on test 231 (128.123 against 128.000) and 540 (134.329 against 134.000); grid on test 231, 525 (16 -> 27 ms) and 540; none on dev (49 tracks), none at BPM 1. 144 (127.388) and 528 (118.016) are back; 42 stays right.

Gate: test BPM 0.1 >= 186 and grid >= 171 **fail** (184, 168), dev not lower passes, CPU at least 30% below 1500 passes (-35% in the pair), zero right -> wrong flips fails (2 on BPM 0.1, 3 on grid). The cross-fade variant is within one or two tracks of chunk 500 in every column, so it is not a win worth 13% more CPU.

### Decision and what is left

- Default unchanged: chunk 500, border 6, no overlap. The switches stay for experiments.
- Chunk 640 (-40% CPU) and the cross-fades are no better than 500 on the quality columns; border 25 is +8% CPU for nothing measurable.
- What would have to change is not the stitch. The flips come from the beat model seeing 10 s instead of 30 s; to hold the 1500 result the fit would have to be less sensitive to a 0.9-logit wobble, the next candidate in the r101 doc (a stricter snap acceptance), or the chunk would have to be longer than the 30% CPU margin allows (750 and up).
- Not verified: bar position for the 1500 reference (not run), the Mac CoreML path (unchanged, 1500, no overlap by default), the overlap with CoreML shapes (the last chunks are shorter than the session shape and run on rten, as before).

### Reproduce

```bash
CARGO_TARGET_DIR=~/.cache/r102-target cargo build --release --manifest-path prototyping/analysis-packaging/Cargo.toml --bin ffa
CARGO_TARGET_DIR=~/.cache/r102-target cargo build --release -p fourfour-analysis-rs --example grid_refit
grid_refit benchmark/results/o3/ref.json benchmark/results/o3/dump <test+dev ids> ref1500.jsonl 2
FFA=~/.cache/r102-target/release/ffa WORKERS=2 THREADS=2 DUMP=~/.cache/r102-dump/bl72 benchmark/scripts/r94-run.sh bl72 <list> FOURFOUR_BEAT_CHUNK=500 FOURFOUR_BEAT_OVERLAP=72 FOURFOUR_BEAT_BLEND=1
python3 -I benchmark/scripts/o3-score.py benchmark/results/o3/ref.json benchmark/results/r94/bl72.jsonl --rows bl72.rows.json
python3 -I benchmark/scripts/r102-flips.py ref1500.jsonl bl72.rows.json          # right -> wrong lists
python3 -I benchmark/scripts/r102-chunkdiff.py benchmark/results/o3/dump ~/.cache/r102-dump/bl72 <ids> 416 --peaks   # logit difference by distance to a border
python3 -I benchmark/scripts/r102-bpms.py benchmark/results/o3/ref.json benchmark/results/r94/bl72.jsonl
```
