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

## 2026-04-22 Analysis CLI README

- [x] Add an LLM-oriented entrypoint README for the analysis CLI.
- [x] Document setup, commands, backend variants, architecture, artifact layout, and verification.
- [x] Commit and push the README update.

## 2026-04-22 Merge Analysis CLI Into Master

- [x] Merge `origin/feat/analysis-cli` into `master`.
- [x] Preserve the newer master waveform/Pioneer analysis stack.
- [x] Route compatibility analysis through the final stack: Lexicon-style BPM/energy plus Essentia bgate key.
- [x] Keep `python -m fourfour_analysis analyze ... --json` compatible with the Tauri caller.
- [x] Verify Python tests, CLI smoke checks, and Rust workspace compile.

## Merge Review

- `fourfour-analyze` uses the final production stack.
- `fourfour-benchmark` keeps key-only benchmarking and `--no-waveform` controls.
- `python -m fourfour_analysis analyze` returns Pioneer waveform fields required by `pioneer-test-ui`.
- Waveform implementation stays on the newer master stack.
