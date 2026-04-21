# Todo

## 2026-04-22 Key Detection Benchmark

- [x] Document Beatport/Rekordbox baseline and Essentia benchmark results.
- [x] Document local setup and commands for rerunning key-only benchmarks.
- [x] Check generated benchmark artifacts and ignore rules.
- [x] Keep `essentia` as the only new key-detection dependency.
- [x] Commit and push the feature branch after verification.

## Review

- `essentia_key_bgate` scored 54.0% exact and 68.9% exact-or-adjacent on the 598-track clean Beatport subset.
- Rekordbox scored 47% exact and 55% exact-or-adjacent on the user's broader 698-track Beatport run.
- This satisfies the current requirement: open-source key detection should at least match Rekordbox for this project.
