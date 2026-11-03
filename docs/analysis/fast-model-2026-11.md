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

## Next (to reach 2x+)

- Chunk 500 leaves the model at about 85% of analysis CPU. Small1 at chunk 500 would be about 3x the base model speed but carries the small-model quality loss; not recommended.
- Remaining model cost is mostly the weight MatMuls, which int8 could cut (r16 measured -11% per chunk at 1500 frames, where attention was a bigger share; at 500 frames the weight share is larger, so int8 may now pay more). Worth one re-measure at chunk 500 with quality scored.
- First-result latency: shorter chunks already halve it; ordering visible tracks first is the scheduling worker's lane.

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
