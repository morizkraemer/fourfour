# Benchmark Fixtures And CLI Batch Tests

This directory separates reusable benchmark tooling from local/generated data.

## Tracked

- `scripts/`: reusable benchmark and smoke-test scripts.
- `manifests/`: small corpus manifests used to reproduce benchmark runs.
- `baselines/`: small baseline JSON files worth preserving.

## Ignored

- `datasets/`: local audio datasets, archives, Rekordbox exports, and third-party metadata.
- `results/`: generated benchmark outputs.
- `logs/`: tmux/shell logs from long runs.
- `cache/`: backend result cache.
- `*.zip`: downloaded dataset archives.

## Main Script

Use `scripts/cli_batch_analyze.py` to black-box test the public analysis CLI against a folder of audio files.

It calls the public `fourfour-analyze ... --json` CLI in chunks so it validates the same contract the Rust integration will consume later without paying model startup cost for every file.

### Beatport Example

Run a small sample in the current terminal:

```bash
analysis/.venv/bin/python benchmark/scripts/cli_batch_analyze.py \
  benchmark/datasets/beatport-edm-key/audio \
  --chunk-size 25 \
  --limit 50
```

Run the full local Beatport audio folder in a detached tmux pane:

```bash
analysis/.venv/bin/python benchmark/scripts/cli_batch_analyze.py \
  benchmark/datasets/beatport-edm-key/audio \
  --chunk-size 25 \
  --tmux
```

The tmux pane closes when the script finishes. The command prints:

- `tmux_pane`
- `run_id`
- `log`
- `results`

### Generic Folder Example

```bash
analysis/.venv/bin/python benchmark/scripts/cli_batch_analyze.py /path/to/audio --tmux
```

The script records output shape, runtime, CLI errors, energy, BPM, key, and waveform lengths.

## Outputs

Each run writes:

- `benchmark/results/<run_id>/results.json`: one row per file.
- `benchmark/results/<run_id>/summary.json`: aggregate metrics.
- `benchmark/logs/<run_id>.log`: only when launched with `--tmux`.
