# Key Detection Benchmark Findings

Date: 2026-04-22

## Decision

Use `essentia_key_bgate` as the current recommended key-detection backend.

It is the first tested open-source backend that meets the practical requirement: match or beat Rekordbox key detection on the Beatport EDM key dataset while staying simple enough to ship as a Python sidecar.

## Dataset

- Source: Beatport EDM Key Dataset from Zenodo.
- Local audio tested: `benchmark/datasets/beatport-edm-key/audio/`.
- Ground truth: `benchmark/datasets/beatport-edm-key/keys/*.txt`.
- Clean evaluation subset: 598 tracks with simple `X major` / `X minor` labels.
- Excluded labels: unknown, modal, multi-key, and other labels that cannot be represented as one Camelot key.

## Rekordbox Baseline

The same local Beatport dataset was analyzed in Rekordbox and scored against the Beatport labels:

| Metric | Result |
|---|---:|
| Tracks scored | 698 |
| Exact match | 331 / 698 = 47% |
| Adjacent compatible | 58 / 698 = 8% |
| Exact + adjacent | 389 / 698 = 55% |
| No ground truth | 59 |

The main Rekordbox failure mode was mode confusion: major/minor errors and same-mode wrong-key errors.

## Backend Shootout

Command shape:

```bash
cd analysis
.venv/bin/fourfour-benchmark run \
  --corpus ../benchmark/manifests/beatport-edm-key-keyonly-clean-full.corpus.json \
  --variants essentia_key_bgate essentia_key_edma essentia_key_edmm essentia_key_shaath essentia_key_krumhansl essentia_key_temperley lexicon_port python_deeprhythm \
  --features key \
  --parallel 1
```

Artifacts:

- Run directory: `benchmark/results/run-20260421T174620Z`
- Log: `benchmark/logs/beatport-edm-key-essentia-shootout-run-20260421T174620Z.log`
- Corpus: `benchmark/manifests/beatport-edm-key-keyonly-clean-full.corpus.json`

Results:

| Backend | Exact | Exact + adjacent | Mean seconds / track |
|---|---:|---:|---:|
| `essentia_key_bgate` | 54.0% | 68.9% | 0.114 |
| `essentia_key_edmm` | 49.3% | 70.4% | 0.115 |
| `essentia_key_edma` | 48.0% | 62.7% | 0.114 |
| `essentia_key_shaath` | 44.3% | 59.4% | 0.116 |
| `essentia_key_krumhansl` | 40.0% | 54.5% | 0.116 |
| `essentia_key_temperley` | 39.1% | 62.2% | 0.116 |
| `lexicon_port` | 29.9% | 42.5% | 0.186 |
| `python_deeprhythm` | 21.6% | 39.3% | 0.520 |

## Interpretation

`essentia_key_bgate` is better than the Rekordbox baseline on exact key match in the available test:

| System | Ground truth | Exact | Exact + adjacent |
|---|---|---:|---:|
| Rekordbox | Beatport labels | 47% | 55% |
| Essentia `bgate` | Beatport labels | 54.0% | 68.9% |

The comparison is not perfectly denominator-matched because Rekordbox was scored on the broader 698-track set and `bgate` was scored on the 598 clean simple-key subset. That is still the right subset for a one-key Camelot detector. The evidence is good enough for the project requirement: key detection should at least match Rekordbox.

## Setup

Install the optional key dependency into the analysis venv:

```bash
cd analysis
uv pip install --python .venv/bin/python essentia
```

Run a key-only benchmark:

```bash
cd analysis
.venv/bin/fourfour-benchmark run \
  --corpus ../benchmark/manifests/beatport-edm-key-keyonly-clean-full.corpus.json \
  --variants essentia_key_bgate \
  --features key \
  --parallel 1
```

Show a run:

```bash
cd analysis
.venv/bin/fourfour-benchmark show run-20260421T174620Z
```

## Rejected Candidates

- `key-cnn`: not available on PyPI, stale repository, old Python/TensorFlow assumptions. Not suitable as a maintained project dependency.
- `keyfinder-py`: package was not available in the local package index.
- `keyfinder`: package existed but failed to build because the native `keyfinder/constants.h` header was missing. This can be revisited only if we explicitly vendor or install libKeyFinder.
- `madmom`: not tested here. It remains a possible research backend, but maintenance and licensing risk make it less attractive than Essentia for the current goal.

## Manual Validation

Manual spot checks should focus on tracks where `bgate` disagrees with Beatport:

1. Sample 20 to 30 exact mismatches from `benchmark/results/run-20260421T174620Z/comparisons.json`.
2. Prioritize `relative`, `parallel`, and `fifth` error types because these are musically plausible and common in DJ tools.
3. Confirm by ear or with a keyboard/drone whether Beatport or `bgate` is more useful for harmonic mixing.
4. If many Beatport-disagreement cases still mix well, prefer `Exact + adjacent` as the operational metric.
