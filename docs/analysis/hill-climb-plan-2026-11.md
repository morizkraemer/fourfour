# Hill-climb delegation plan (2026-11-20)

Status: draft 2 (re-reviewed) at the end of this file is the plan to start from; draft 1 below is kept for reference. Nothing started.

## Findings that shape the plan
- The grid test split `o3/test.txt` (205) was used for decisions in r181: regression gate only from now on.
- The r180 real folder (`#02_DJ/2025`) may overlap `benchmark/datasets/rekordbox-testdata-2025`: fingerprint-check before calling it held out.
- `benchmark/results/o3/ref.json` has 643 Rekordbox-ref tracks; dev 50 + test 205 + tune 150 used, ~200 untouched = the only fresh confirm set. `benchmark/datasets/rekordbox-library*` may hold more (not checked).
- Cached beat logits exist for 254 tracks (`benchmark/results/o3/dump`); fit and bar-1 experiments on cached logits run in seconds (`o3-fit.sh`, `grid_refit.rs`, `bar_replay.rs`). Dumping logits for all refs is the enabler.
- bw split files (`benchmark/results/bw/*.tsv`) live only in worktree `agent-aa3b2485cb3ea9d8f`: copy before removing it.
- No `[profile.release]` in the workspace (no LTO/strip/codegen-units).
- `scripts/perf-budgets.json` "now" values partly stale. Biggest gap: waveform detail first time 800 ms vs 120 target.
- On the Mac (int8) non-model stages are now about half of analysis CPU after windowing (estimate).
- `perf`/`valgrind` installed: instruction counts give a load-independent speed metric.

## 1. Verdict per candidate
- BPM octave: worth it. Acc1 92.4% vs Acc2 97.5% on GiantSteps+; realistic +2 to +3. DnB 87 vs 174 is a product call.
- Grid phase (20 ms): worth it, modest. 83.4%, misses mostly half-beat phase. +2 to +4. Cached logits.
- Bar-1: one bounded loop. Weakest metric; truth is Rekordbox agreement; Waiting 16 open. Only new features can move it. +2 to +5, high overfit risk.
- Key: not a loop (label-noise ceiling). One $15 diagnostic: 49% vs Rekordbox on full tracks vs 78.9% on previews.
- Beat model speed: not a loop on dev (fallback loosening failed 5 ways; x86 fp32 does not transfer to Mac int8). CoreML is Mac-only.
- Non-model stages CPU + peak RSS: worth it, best pure hill-climb, bit-identical gate. ~-10% dev, maybe -20% Mac.
- Duplicate reuse: one-shot build, ~-28% work on real libraries. Branch only.
- Startup/import/scroll: mostly done; keep backend metrics only (waveform detail cold 6.7x off, load_state warm 1.8x).
- Bundle/DMG: one-shot release profile. Frontend JS is 0.5 MB of 42 MB; model file is the bulk.
- Test/CI health: small. Known load flakes (leaks.spec:110, duplicates.spec:45, playhead).
- Robustness fuzz: worth it (strangers' folders; symphonia panics seen).
- Phrase, energy, loudness: drop (no labels).

## 2. Chosen loops, ranked
L0 Setup (one-shot, blocks L1/L2). sonnet medium, $15, 3 h.
- Dump logits for all ref.json tracks with audio and GiantSteps+ (601) to `benchmark/results/hc/dump`. Freeze splits in `benchmark/results/hc/`: TUNE = old dev 50 + tune 150; CONFIRM = untouched remainder (fingerprint-deduped); GATE = old test 205. GiantSteps+: fixed seed 300 tune / 301 confirm. `benchmark/scripts/hc-score.sh <split>` prints Acc1, BPM 0.1, grid 20 ms, bar right. Baseline in `docs/analysis/hill-climb-2026-11.md`.
- May write: benchmark/scripts, benchmark/results, docs/analysis. Lands: scripts + doc.

L1 BPM octave + grid phase. opus high, $120, 4 days, max 40 steps.
- Metric: `hc-score.sh tune` (Acc1 + grid 20 ms). Refit via `grid_refit` on cached logits.
- Gates: no metric lower on TUNE; GATE Acc1 >= 194, grid >= 172, bar >= 140; GiantSteps+ tune Acc1 not lower; `cargo test -p fourfour-analysis-rs`.
- CONFIRM run at most 3 times (day 2, day 4, final), never to pick variants.
- May write: `fit.rs`, tempo parts of `service.rs`, `mp3.rs`, examples, benchmark/scripts. Bump ANALYSIS_VERSION at the end.
- Stop: 8 rejected in a row, < 2 tracks over the last 10 steps, or budget. Branch `hc-grid`, nothing lands.

L2 Bar-1 rule. opus high, $70, 2.5 days, max 25 steps, after L1.
- Metric: bar right on TUNE (`bar_replay` + `o3-score.py`). Grid/BPM bit-identical. Must beat baseline on CONFIRM by >= 3 tracks at the single final check. Listen list via `b15-bar-listen.py`. Branch `hc-bar1`, needs Merlin's ears.

L3 Non-model stage CPU + peak RSS. sonnet high (opus when stuck), $80, 4 days, max 40 steps.
- Metric: `perf stat -e instructions:u` on `stage_cpu` over 20 fixed TUNE tracks; secondary peak RSS (`stage_rss`). Wall CPU only as final quiet-box A/B.
- Gate: JSONL byte-identical on 50 dev tracks; `cargo test --workspace`; `cargo check -p pioneer-test-ui --features dev-bins --bins`.
- May write: `fourfour-analysis-rs/src/{decode,bt,onset,cyankey,analyze,waveform,keymap}.rs`, examples. Keep a step at >= 1% instructions. Stop: 6 rejected in a row or < 1% over 8 steps. Branch `hc-stages`.

L4 Duplicate reuse. opus high, one-shot, $35, 1.5 days. Step 1 measure only (pairs within 5 ms after format delay correction); build only if >= 20% reuse. Schema change = BLOCKED. Branch `hc-dup-reuse`.

L5 Fuzz robustness. sonnet medium, $45, 3 days. 200 seeds x 50 mutations through decode + tag scan + import under timeout; metric panics + hangs + >2 GB allocations. pioneer-usb-writer bugs: repro test only.

L6 Backend perf (waveform detail cold, load_state warm). sonnet high, $40, 2 days. Refresh perf-budgets "now" first. Gate identical output bytes. Stop if cost is in pioneer-usb-writer Meter::push. Branch `hc-backend-perf`.

L7 Release profile (one-shot). sonnet medium, $10, 3 h. LTO, codegen-units=1, strip, panic=abort (check Tauri). Branch `hc-release-profile`; DMG size needs the Mac.

L8 Smoke flake hunt. sonnet medium, $30, 2 days. Failures in 20 smoke runs under `load-run.sh`. May write `app/tests` and helpers only.

L9 Key gap diagnostic (one-shot). sonnet medium, $15, 3 h. Report only.

Total ~$460 before attrition; hold $20-30 back. Fable as a final read-only review per kept branch (~$3).

Worker rules: own worktree + branch off master; one change per commit with before/after metric; revert rejected steps; row in `docs/analysis/hill-climb-2026-11.md`; `CARGO_TARGET_DIR=~/.cache/hc-<loop>`; nice 10; max 4 analysis processes box-wide; no spawn under 1500 MB available; no new deps; no Tauri window, audio, USB write; prototyping/ off limits; datasets read-only; no push.

## 3. Schedule (max 3 writers, disjoint paths)
- Day 1 (Nov 20): L0, L7, L9. ~$40.
- Days 2-5: L1 + L3 + L5 (L5 tests/pioneer-library only until L3 ends). ~$55/day.
- Days 6-8: L2 + L4 + L8; L5 decode fixes day 6. ~$45/day.
- Days 9-10: L6, final CONFIRM runs, Fable reviews, merge checks, write-up. ~$30/day.
- Disk: delete `~/.cache/hc-<loop>` when a loop ends; main target 16 GB, agent-aa3b target 8.3 GB (copy bw results first); stale worktrees under /tmp scratchpad.
- Mac: one night queue run near day 8 with Merlin's OK.

## 4. Risks and decisions for Merlin
- Truth is thin: ~200-track confirm set, ±3 tracks is noise; < 2 points = "no gain".
- Bar-1 goal: Rekordbox agreement or his ears (Waiting 16 first)?
- DnB/half-time convention (174 vs 87).
- May L3-type fixes touch `pioneer-usb-writer` waveform Meter (4x, bit-identical, ~222 ms/audio-min)?
- One Mac night run; raise run limit to 3 workers for these 10 days.
- Use `rekordbox-library*` refs as extra confirm data?
- Timing loops next to accuracy loops (mitigated by instruction counts).
- 10 branches to review at the end: each loop ends with a 10-line result and one recommended action.
- Not verified: r180 vs ref.json overlap, size of the untouched remainder, Mac int8 stage shares.

# Draft 2: second review (Fable, 2026-11-20) - the plan to start from

## Corrections to draft 1
- CONFIRM: 217 untouched refs have audio, but 78 are copies (same artist+title) of dev/test/tune tracks: truly independent ~127-139. 21 refs have no audio.
- The r180 folder IS the ref.json folder (all 622 paths are `rekordbox-testdata-2025/#02_DJ/2025`): not held out.
- `perf stat` refused (perf_event_paranoid=3); use callgrind.
- `grid_refit` fits the full track; the shipped default is windowed (~80% of tracks): L0 must add windowed emulation.
- `o3-fit.sh` and `bgdump` live in `prototyping/` (fence): dump from the product crate instead.
- Smoke lives in `app/smoke`, not `app/tests`.
- panic=abort excluded: 14+ catch_unwind sites would turn caught decoder panics into crashes.
- L3 climbable share is ~10% of total CPU without the waveform Meter: realistic -3 to -5% (or ~-10% with the Meter).
- Schedule: 11 days; draft 1 days 2-5 exceeded $60/day and the 2-worker run limit.
- L2 must branch off hc-grid. One result file per loop (`docs/analysis/hc-<loop>.md`). Splits by absolute path in the main checkout (benchmark/results is gitignored).

## Revised plan
| loop | goal metric | harness | gates | may write | model | $ | timebox | start | branch | lands |
|---|---|---|---|---|---|---|---|---|---|---|
| L0 setup | splits + scorer + dumps + window emulation | new hc-score.sh, grid_refit, beat_windows_bench | emulation = bench on GATE (>=99% tracks equal) | benchmark/scripts, examples, results/hc, docs/analysis | sonnet high | 25 | 4 h | Nov 20 | hc-setup | scripts + doc |
| L9 key gap | explain 49% vs 78.9% (check notation/mapping bug first) | r180 compare.py, key_error_breakdown.py | report only | docs/analysis | sonnet medium | 15 | 3 h | Nov 20 | none | doc |
| L7 release profile | binary size, CPU/audio-min | cargo build --release, stage_cpu | cargo test --workspace, no panic=abort | Cargo.toml | sonnet medium | 10 | 3 h | Nov 20 | hc-release-profile | no |
| L1 grid + BPM | grid 20 ms + Acc1 on TUNE (windowed) | hc-score.sh tune | TUNE not lower; GATE Acc1>=194, grid>=172, bar>=140; GiantSteps tune not lower; cargo test; CONFIRM 3 looks, +4 tracks to count; DnB 87/174 untouched | fit.rs, service.rs tempo, mp3.rs, examples | opus high | 110 | 4 d, 40 steps | Nov 21 | hc-grid | no |
| L3 stages (+L6) | instructions on 6 tracks, peak RSS, waveform_detail_cold | callgrind on stage_cpu, stage_rss, perf.sh --size 10k | HASH identical all formats; JSONL identical dev 50; cargo test; dev-bins check | decode, bt (mel), onset, cyankey, analyze, examples; Meter only with OK | sonnet high | 50 (25 w/o Meter) | 3 d, 30 steps | Nov 21 | hc-stages | no |
| L5 fuzz | panics + hangs + >2 GB | new harness from tests/messy_audio.rs, truncated_audio.rs; 200 seeds x 50 | cargo test --workspace | tests first; decode.rs only after L3 ends | sonnet medium | 30 | 2 d | Nov 24 | hc-fuzz | no |
| L4 dup measure | reuse share, grid agreement | r180 data + ref.json | report only | docs/analysis | sonnet medium | 10 | 3 h | Nov 24 | none | doc |
| L2 bar-1 probe | bar right on TUNE | bar_replay + o3-score.py | grid/BPM bit-identical; CONFIRM +5 once | service.rs bar rule, examples | opus high | 30 | 1.5 d, 12 steps | after L1 | hc-bar1 off hc-grid | no |
| L10 Mac run | Mac CPU/audio-min, int8 GATE | run.sh full / w30, bw-compare.py | none | docs/analysis | sonnet medium | 15 | 1 night | Nov 25-27 | none | doc |
| reviews | one read-only pass per kept branch | - | - | - | fable | 20 | - | Nov 28-30 | - | - |

Total $315; reserve $135 (up to +$50 L1, +$25 L3 only if their last 10 steps still gained; +$25 L4 build if wanted). Cut: L8 flake hunt, standalone L6, full L2.
Rules: max 2 workers at once unless Merlin raises it; splits and scorer read-only with checksums; day cap $60 enforced by the orchestrator.

## Decisions for Merlin
1. Copy ~300 more tracks with Rekordbox grids (another folder) to dev as a real held-out set? rec: yes.
2. May L3 touch the waveform Meter in pioneer-usb-writer (branch only, bit-identical, tests)? rec: yes, else L3 $25.
3. Raise the run limit from 2 to 3 workers? rec: no.
4. One Mac night run around Nov 25-27 (on power, unlocked)? rec: yes.
5. Will you listen to a bar-1 list this month? rec: if not, drop L2.
6. Allow `sudo sysctl kernel.perf_event_paranoid=1` on dev? rec: no, use callgrind.
7. Under-spend OK? rec: yes.

## Merlin's answers (2026-11-20) and the final split
- Credits: cap this project at $150 (other projects use the API too). Seat subagents may be used as well.
- 1 yes (300 more held-out tracks). 2 yes: only USB sync and CDJ behaviour are Mo's; the waveform meter and the rest may change (AGENTS.md lanes updated). 3 three workers when it is faster. 4 Mac night run OK. 5 bar-1 listen list OK if quick. 6 orchestrator's call: callgrind, no sysctl change. 7 see credits.
- Credit runs ($150): L1 grid + BPM $75 (opus high), L3 stages incl. waveform meter $45 (sonnet high), L5 fuzz $30 (sonnet medium).
- Seat subagents: L0 setup, L9 key gap, L7 release profile, L4 dup measure, L2 bar-1 probe, L10 Mac run, final Fable reviews.
- Release builds: serialize with `flock ~/.cache/hc-release.lock`.

## Rebalance (Merlin 2026-11-20: "use more credits and less claude plan ... or just spread out the work more")
All new workers run as credit runs within the $150 cap; no new seat workers. L0 and L7 (already on the seat) finish as they are. At most 2 credit workers at once.
| loop | model | $ |
|---|---|---|
| L3 stages (running) | sonnet high | 45 |
| L1 grid + BPM | opus high | 55 |
| L5 fuzz | sonnet medium | 20 |
| L2 bar-1 probe | sonnet high | 12 |
| L4 dup measure | sonnet medium | 5 |
| L10 Mac night run | sonnet medium | 5 |
| final reviews | sonnet medium | 8 |
