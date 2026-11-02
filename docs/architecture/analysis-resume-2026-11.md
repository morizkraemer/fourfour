# Analysis run: quit, crash and restart (2026-11)

Scope: headless on the dev box. Queue logic tested with the stub engine; crash tests use a real file-backed library (160 tracks, real files). No real 10k run with the Rust engine (no weights or music on the box).

## How a run works

- The queue lives in `AnalysisCoordinator`; its order is also kept in a side file (see Resume order).
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
| 5 | Order after restart | Waveform+key-done first, then never analysed, by id. `resume_work` quadratic: 30k half-analysed tracks 17.3 s under the library lock (debug, loaded box) | Same default order, 2.2 s; the saved run order comes first when the side file has one (see Resume order) |

## Tests added

- `pioneer-library/tests/crash_resume.rs`
- a test in `pioneer-library/src/analysis_write.rs`
- 12 tests in `analysis_run_tests.rs` (pioneer-test-ui)
- `app/src/stores/launch-analysis-race.test.ts`
- `analysis_order.rs` unit tests, 4 resume/side-file tests in `analysis_run_tests.rs`, 2 `.part` sweep tests in `waveform_detail.rs`

## Needs Merlin

- Nothing open for failures: they are remembered in a side file (see Failure memo). A per-track column would still be the cleaner home if the schema is opened for other reasons; the UI does not show the memo yet.

## Resume order

- A run writes its pending order (in-flight tracks, then the queue, then the grid queue) to `analysis-order.json` in the app data dir. No schema change.
- Writes are coalesced: the first one at run start, then at most one per 3 s and only when the order changed. Each write goes to a temp file and is renamed into place.
- `resume_analysis` puts the tracks the file lists first, in its order, then the rest in the old order (incomplete first, then never analysed, by id). Ids that are removed, already finished or not in today's work are skipped.
- A missing, stale or corrupt file gives the old order. A run that finishes or is cancelled deletes the file. A run that ends in an error leaves it.
- A new run overwrites the file with its own order, so a short "Analyze" after a kill replaces the old list; the rest falls back to the old order.
- At launch, `.part` files older than 10 minutes in `waveform-detail/` that belong to another pid are removed. Files of this process are never touched.

## Failure memo

- A track whose analysis fails is remembered in `analysis-failures.json` in the app data dir, so launch resume and "Analyze all" stop retrying it. No schema change; same temp-file-then-rename write as the order file.
- Per track id: the file path, size, mtime (ms), the error kind (`not_found`, `permission_denied`, `decode_failed`, `analysis_failed`, the `ErrorKind` names of r39), the attempt count and the time of the last attempt. The file also carries the app version (`pioneer_usb_writer::VERSION`); entries written by another version are ignored, so an analyzer fix retries everything once.
- Kind: the file is gone (`not_found`) or unreadable (`permission_denied`) by stat; else `decode_failed` when the message names decoding, audio, a codec or an unsupported format; else `analysis_failed` (timeouts and anything else).
- Skip rule: `resume_analysis` and `analyze_tracks` drop a track when its entry has 2 or more attempts and the track's path, size and mtime still equal the entry's. N is 2: one failure can be a hiccup (a file still being copied, a timeout on a loaded box, an unplugged drive), the second one in a later run is the file, and a hard failure costs at most two retries. A missing file stays skipped until it appears.
- Cleared when: the file changed (size or mtime), the track points at another path (relink, relocate, id reuse), the track is gone, or the track analyses successfully. Those are checked when work is built, so a relink needs no hook. A failure after a change starts counting from 1.
- Always retried: `analyze_track_ids` (Analyze selection, re-analyze), `prioritize_analysis` (play), `enqueue_analysis`. A failure there still counts, so a track that failed again stays skipped; a success clears it.
- Not counted: cancels, a priority request that preempts a background job, and results that could not be stored (track removed or relinked while running).
- Writes: the first failure of a run is written at once, then at most one write per 3 s; the run (and the now lane) flush what is left when they end. A kill -9 can lose the last 3 s of failures, which only means one more retry.
- A missing, corrupt or wrong-version file, or no app data dir, is today's behaviour (nothing skipped); the next change rewrites it. Removed tracks are dropped the next time work is built.
- Resume logs how many tracks it left out. There is no count on the status data yet: `resume_analysis` returns only the queued number, and changing that return is a UI contract change, so it is left for the UI task that renders it (`FailureMemo::skip_known` already returns the count).
- Library switch: ids are per library, but entries are matched on path and file, so another library's entry for the same id is dropped, not applied.
- Tests: `analysis_failures.rs` (skip after N, retry on changed size or mtime, missing file, relink, removed tracks, success, corrupt files, version, coalescing) and five in `analysis_run_tests.rs` (resume after two failed runs, explicit and priority requests retry, store failures and cancels do not count).

## Open items

- A graceful quit does not cancel in-flight jobs; they die with the process like kill -9 (covered).
- After Cancel, rows that did not run stay pending with a "-" BPM until the next load (UI, unchanged).
- Cases 3a, 3b and 4b are only partly exercised by direct tests of the grid/whole paths; the fast-pass handoff is tested and shares the code.
