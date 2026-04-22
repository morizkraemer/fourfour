# Lessons

## 2026-04-22 Key Detection Benchmarking

- Do not treat web research summaries as dependency truth. Verify package availability, maintenance state, installability, and licensing before implementing a backend.
- When comparing key detectors, state the denominator and label filter explicitly. A broad dataset and a clean single-key subset are not identical, even when they come from the same source.
- For this project, a useful key benchmark needs Beatport-style labels or another external label source. Rekordbox agreement alone measures compatibility with Rekordbox, not correctness.

## 2026-04-22 Analysis CLI Contract

- If a CLI is for manual testing, make the obvious command emit the same contract the app integration cares about. Hidden compatibility commands are fine, but they should not be the only way to inspect complete output.
- A complete JSON shape does not imply analysis quality. Validate each signal separately; key accuracy results do not validate BPM or beat-grid quality.
- Before calling a stack "final", check it against the documented per-signal decisions. In this project, the production CLI should use DeepRhythm for BPM, Essentia `bgate` for key, librosa feature fusion for energy, and the current waveform analyzer. Lexicon-style code is benchmark/reference code unless explicitly re-selected.
