# Analysis run: quit, crash and restart (2026-11)

Scope: headless on the dev box. Queue logic tested with the stub engine; crash tests use a real file-backed library (160 tracks, real files). No real 10k run with the Rust engine (no weights or music on the box).

## How a run works

- The queue lives in memory only (`AnalysisCoordinator`).
- Persisted per finished pass: `analyses.stages` bits plus the ANLZ files in `anlz/<id>/`.
- A store is staged: files go to `.staging-<pid>-<n>/`, are renamed into place, then the row is written in one transaction. A crash leaves the previous stage.
- At launch `remove_stale_staging` runs. After 5 s `resume_analysis` queues incomplete tracks first, then never-analysed ones if the setting is on, and runs only the missing passes.

## Cases (before -> after)

| # | Case | Before | After |
|---|---|---|---|
| 1 | Quit or cancel mid-run | Finished tracks kept their results, resume ran only the rest. Cancelled tracks counted as failures ("Analysis failed for all N") | Cancelled tracks are not failures |
| 1b | Cancel while a fast pass finishes | Grid pass queued after cancel, run never ended; `running` stuck true and later requests merged into a dead run | Cancelled run queues nothing and ends |
| 2 | kill -9 mid-run (6 kills, 160 tracks) | Already safe: `integrity_check` ok, nothing finished redone, resumed library byte-identical to an uninterrupted run, staging cleaned at launch | Same, kept as a regression test |
| 2b | Plain store (grid/cue edits, fallback after a refused commit) | DAT/EXT/2EX truncated and rewritten in place (11,801 short reads in 150 stores) | Each file moves in whole |
| 3a | Work merged while a run closes (played track, "Analyze" on a selection) | Lost into a dead queue, track pending until next launch | Waits for the run to end, starts the next one |
| 3b | Merge during the fast-to-grid handoff | Grid pass ran twice | Runs once |
| 3c | Start/stop storms | No duplicate runs, but see 1b and 3a | Back-to-back cancel/start and 3 simultaneous runs: each track ran once |
| 3d | Run or priority request fails to start (no weights, no Python) | No `analysis-complete`: store stuck "analyzing" | Backend sends `analysis-complete` |
| 3e | Launch resume answered after its run already ended | Store set `analyzing=true` after complete: stuck spinner | Ignored |
| 4a | Track removed while queued | Dropped, nothing left behind | Same |
| 4b | File moved/relinked while queued, or track id reused (`INTEGER PRIMARY KEY` without AUTOINCREMENT) | Stale-path result stored on the track | Not stored; resume reads the new path |
| 5 | Order after restart | Waveform+key-done first, then never analysed, by id. `resume_work` quadratic: 30k half-analysed tracks 17.3 s under the library lock (debug, loaded box) | Same order, 2.2 s |

## Tests added

- `pioneer-library/tests/crash_resume.rs`
- a test in `pioneer-library/src/analysis_write.rs`
- 12 tests in `analysis_run_tests.rs` (pioneer-test-ui)
- `app/src/stores/launch-analysis-race.test.ts`

## Needs Merlin

- Analysis failures are not stored, so undecodable or missing files retry at every launch. Needs a per-track failure marker (schema).
- Queue order is not persisted, so resume cannot restore selection-first or visible-first. Options: a side file in the app data dir with the last run's order (no schema; rec), a "recently added first" rule, or leave it.

## Open items

- A graceful quit does not cancel in-flight jobs; they die with the process like kill -9 (covered).
- `.part` files in the waveform-detail cache left by a kill are never cleaned; a startup sweep of other pids' files would fix it.
- After Cancel, rows that did not run stay pending with a "-" BPM until the next load (UI, unchanged).
- Cases 3a, 3b and 4b are only partly exercised by direct tests of the grid/whole paths; the fast-pass handoff is tested and shares the code.
