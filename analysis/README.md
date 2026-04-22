# fourfour-analysis CLI

LLM entrypoint for the Python analysis and benchmark sidecar in `fourfour`.

Start here when you clone the repo and need to understand or use the CLI without rediscovering the architecture.

## What This Is

`fourfour-analysis` is a Python package with two console commands:

- `fourfour-analyze`: analyze one audio file.
- `fourfour-benchmark`: build corpora, run backend comparisons, and score output against ground truth.

The package is a sidecar for the Rust Pioneer USB tooling. `fourfour-analyze` now uses the final production stack and emits the complete single-track analysis contract used for testing.

Current validated scope:

- BPM, key, energy, cue, and waveform output types.
- Benchmark manifest generation from tagged audio files.
- Cached backend runs.
- Objective scoring against known BPM/key tags.
- Key detection benchmark against the Beatport EDM Key Dataset.

Not complete here:

- Rekordbox-equivalent beatgrid / first-beat quality.
- Final waveform analyzer integration.
- Phrase analysis.
- Production packaging decision for the Python sidecar.

## Read These Docs First

Use this order:

1. `analysis/README.md` - this file, CLI and architecture entrypoint.
2. `../docs/key-detection-benchmark-findings.md` - current key detector decision and benchmark evidence.
3. `../docs/analysis-pipeline-handoff.md` - broader analysis pipeline notes.
4. `../docs/experimentation-path.md` - project roadmap and remaining validation.
5. `../docs/tech-stack-reference.md` - library tradeoffs and dependency notes.

Reusable benchmark scripts, manifests, and baselines live under `../benchmark/`. Local datasets, logs, caches, and run outputs under that directory are gitignored.

## Setup

From the repo root:

```bash
cd analysis
uv venv .venv
uv pip install --python .venv/bin/python -e ".[dev]"
```

DeepRhythm, librosa, torch, and Essentia are normal dependencies because `fourfour-analyze` uses them in the final stack.

## Commands

Show resolved benchmark directories:

```bash
cd analysis
.venv/bin/fourfour-benchmark config-dirs
```

Analyze one track:

```bash
cd analysis
.venv/bin/fourfour-analyze /path/to/track.mp3 --json
```

`fourfour-analyze --json` returns one JSON object with BPM, key, energy, classic waveform arrays, and Pioneer waveform arrays:

```text
path
bpm
key
energy
beats
cue_points
waveform_preview
waveform_color
waveform_peaks
pioneer_3band_detail
pioneer_3band_overview
errors
elapsed_seconds
```

`beats` and `cue_points` are currently present but empty. DeepRhythm provides the global BPM number, not a beat grid. Beatgrid / first-beat analysis is a separate integration track.

The compatibility command below returns the same per-track objects wrapped in a list because the current Tauri test UI already calls this shape:

```bash
cd analysis
.venv/bin/python -m fourfour_analysis analyze /path/to/track.mp3 --json
```

Build a manifest from tagged files:

```bash
cd analysis
.venv/bin/fourfour-benchmark init /path/to/audio-folder --name my-corpus
```

Run a key-only benchmark:

```bash
cd analysis
.venv/bin/fourfour-benchmark run \
  --corpus ../benchmark/manifests/my-corpus.corpus.json \
  --variants essentia_key_bgate \
  --features key \
  --parallel 1
```

Show a finished run:

```bash
cd analysis
.venv/bin/fourfour-benchmark show run-YYYYMMDDTHHMMSSZ
```

Compare two runs:

```bash
cd analysis
.venv/bin/fourfour-benchmark compare run-old run-new
```

List runs:

```bash
cd analysis
.venv/bin/fourfour-benchmark list
```

Black-box test the public analysis CLI over a folder:

```bash
cd ..
analysis/.venv/bin/python benchmark/scripts/cli_batch_analyze.py /path/to/audio --tmux
```

For the local Beatport dataset:

```bash
analysis/.venv/bin/python benchmark/scripts/cli_batch_analyze.py \
  benchmark/datasets/beatport-edm-key/audio \
  --chunk-size 25 \
  --tmux
```

## Current Backend Variants

Backends are registered in `src/fourfour_analysis/backends/registry.py`.

| Variant | Purpose | Dependencies | Status |
|---|---|---|---|
| `final_stack` | DeepRhythm BPM + librosa energy + Essentia `bgate` key | base deps | Current production path |
| `lexicon_port` | Python port of Lexicon-style BPM/key/energy/cue/waveform logic | base deps | Benchmark baseline |
| `python_deeprhythm` | DeepRhythm BPM + librosa key/energy | base deps | Benchmark baseline |
| `stratum_dsp` | Rust subprocess wrapper | Rust binary | Benchmark target |
| `essentia_key_bgate` | Essentia KeyExtractor `bgate` profile | base deps | Key benchmark winner |

`fourfour-analyze` always uses the full orchestrator: `final_stack` for DeepRhythm BPM, librosa energy, and Essentia `bgate` key plus the newer Pioneer waveform analyzer from `waveform.py`. The benchmark CLI still exposes the underlying variants for comparison runs.

## Current Key Decision

Use `final_stack` for normal analysis. Use `essentia_key_bgate` inside benchmarks when you want to isolate key detection.

Beatport EDM Key benchmark summary:

| System | Ground truth | Exact | Exact + adjacent |
|---|---|---:|---:|
| Rekordbox | Beatport labels | 47% | 55% |
| Essentia `bgate` | Beatport labels | 54.0% | 68.9% |

This satisfies the current requirement: key detection should at least match Rekordbox on external labels.

Details and caveats are in `../docs/key-detection-benchmark-findings.md`.

## Architecture

High-level flow:

```text
audio files
  -> fourfour-benchmark init
  -> benchmark/manifests/*.corpus.json
  -> fourfour-benchmark run
  -> backend registry
  -> backend analyze_track_cached()
  -> benchmark/results/<run_id>/raw/*.json
  -> compare.py
  -> benchmark/results/<run_id>/comparisons.json
  -> scoring.py
  -> benchmark/results/<run_id>/scoring.json
```

Important modules:

| File | Responsibility |
|---|---|
| `src/fourfour_analysis/cli.py` | Console commands and argument parsing |
| `src/fourfour_analysis/config.py` | Project-root and benchmark path resolution |
| `src/fourfour_analysis/manifest.py` | Corpus creation and loading |
| `src/fourfour_analysis/groundtruth.py` | Tag extraction and key normalization |
| `src/fourfour_analysis/runner.py` | Benchmark orchestration |
| `src/fourfour_analysis/compare.py` | BPM/key/energy comparison logic |
| `src/fourfour_analysis/scoring.py` | Aggregate scores and recommendations |
| `src/fourfour_analysis/cache.py` | Content/config-addressed result cache |
| `src/fourfour_analysis/types.py` | Shared dataclasses |
| `src/fourfour_analysis/backends/base.py` | Backend interface and caching wrapper |
| `src/fourfour_analysis/analyze.py` | Full single-track orchestrator used by `fourfour-analyze` and the module compatibility command |
| `src/fourfour_analysis/backends/final_stack.py` | Production DeepRhythm BPM, librosa energy, and Essentia `bgate` key stack |
| `src/fourfour_analysis/backends/registry.py` | Public backend variants |
| `src/fourfour_analysis/backends/essentia_key.py` | Essentia `bgate` key backend |
| `src/fourfour_analysis/backends/lexicon_port.py` | Lexicon-style full backend |
| `src/fourfour_analysis/backends/python_stack.py` | DeepRhythm/librosa backend |
| `src/fourfour_analysis/backends/stratum_dsp.py` | Rust subprocess backend |

## Artifact Layout

Reusable benchmark config and generated benchmark data are under `../benchmark/`:

```text
benchmark/
  README.md           tracked benchmark directory guide
  scripts/            tracked reusable benchmark/smoke scripts
  manifests/          tracked small corpus manifests
  baselines/          tracked small baseline references
  results/<run_id>/   generated outputs, ignored
  cache/              generated backend cache, ignored
  logs/               generated tmux or shell logs, ignored
  datasets/           local datasets and archives, ignored
```

Do not commit local datasets, result JSON, cache files, or logs unless there is an explicit reason to publish a small fixture.

## Working Rules For Agents

- Prefer `fourfour-benchmark run --features key` when testing key detection.
- Prefer `benchmark/scripts/cli_batch_analyze.py` when testing the public `fourfour-analyze` CLI contract over real files.
- Keep benchmark runs deterministic by using manifest files, not ad hoc directory scans.
- Cache keys include backend config. If backend behavior changes, update `BackendMetadata.config_hash`.
- Use `groundtruth.normalize_key()` for all key normalization. Do not add duplicate Camelot mappings.
- Do not add new public backend variants just for experiments. Benchmark internally first, then expose only a chosen backend.
- Generated files under `benchmark/` are ignored and should stay local.

## Verification

Run the test suite:

```bash
cd analysis
.venv/bin/pytest -q
```

Check CLI registration:

```bash
cd analysis
.venv/bin/fourfour-benchmark run --help
```

Compile changed Python files when touching CLI or backend code:

```bash
cd analysis
.venv/bin/python -m py_compile \
  src/fourfour_analysis/cli.py \
  src/fourfour_analysis/backends/registry.py \
  src/fourfour_analysis/backends/essentia_key.py
```
