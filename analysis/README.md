# fourfour-analysis CLI

LLM entrypoint for the Python analysis and benchmark sidecar in `fourfour`.

Start here when you clone the repo and need to understand or use the CLI without rediscovering the architecture.

## What This Is

`fourfour-analysis` is a Python package with two console commands:

- `fourfour-analyze`: analyze one audio file.
- `fourfour-benchmark`: build corpora, run backend comparisons, and score output against ground truth.

The package is a sidecar for the Rust Pioneer USB tooling. It is used for analysis experiments and for backends that are easier to run in Python than Rust.

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

Generated benchmark artifacts live under `../benchmark/` and are gitignored.

## Setup

From the repo root:

```bash
cd analysis
uv venv .venv
uv pip install --python .venv/bin/python -e ".[dev]"
```

Optional backends:

```bash
# Existing ML stack: DeepRhythm + librosa.
uv pip install --python .venv/bin/python -e ".[ml]"

# Current recommended key backend: Essentia KeyExtractor bgate.
uv pip install --python .venv/bin/python -e ".[key]"
```

If the venv already exists, just install the extras needed for the backend you are using.

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

Analyze one track with the current key backend:

```bash
cd analysis
.venv/bin/fourfour-analyze /path/to/track.mp3 --backend essentia_key_bgate --json
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

## Current Backend Variants

Backends are registered in `src/fourfour_analysis/backends/registry.py`.

| Variant | Purpose | Dependencies | Status |
|---|---|---|---|
| `lexicon_port` | Python port of Lexicon-style BPM/key/energy/cue/waveform logic | base deps | Useful baseline |
| `python_deeprhythm` | DeepRhythm BPM + librosa key/energy | `[ml]` extra | Heavy baseline |
| `stratum_dsp` | Rust subprocess wrapper | Rust binary | Integration target, not fully validated here |
| `essentia_key_bgate` | Essentia KeyExtractor `bgate` profile | `[key]` extra | Current recommended key detector |

Only `essentia_key_bgate` is exposed for Essentia. Other Essentia profiles were benchmarked historically, but `bgate` won exact-match accuracy and is the only public variant.

## Current Key Decision

Use `essentia_key_bgate` for key detection.

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
| `src/fourfour_analysis/backends/registry.py` | Public backend variants |
| `src/fourfour_analysis/backends/essentia_key.py` | Essentia `bgate` key backend |
| `src/fourfour_analysis/backends/lexicon_port.py` | Lexicon-style full backend |
| `src/fourfour_analysis/backends/python_stack.py` | DeepRhythm/librosa backend |
| `src/fourfour_analysis/backends/stratum_dsp.py` | Rust subprocess backend |

## Artifact Layout

All generated benchmark data is under `../benchmark/`:

```text
benchmark/
  manifests/          input corpora
  results/<run_id>/   raw backend output, comparisons, scoring, metadata
  cache/              content/config-addressed backend cache
  logs/               tmux or shell logs for long runs
  datasets/           local datasets, not committed
```

The repo-level `.gitignore` ignores `benchmark/`. Do not commit local datasets, result JSON, cache files, or logs unless there is an explicit reason to publish a small fixture.

## Working Rules For Agents

- Prefer `fourfour-benchmark run --features key` when testing key detection. It avoids waveform cost and stale comparisons.
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

