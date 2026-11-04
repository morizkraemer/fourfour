#!/usr/bin/env bash
# usage: r79-run.sh <label>   re-fits dev + train logits into benchmark/results/r79/{dev,train}-<label>.jsonl (env R79_* passes through)
set -euo pipefail
repo="${R79_REPO:-/home/merlin/dev/projects/fourfour}"
refit="${GRID_REFIT:-$HOME/.cache/r79-target/release/examples/grid_refit}"
r77="$repo/benchmark/results/r77"
out="$repo/benchmark/results/r79"
ref="$repo/benchmark/results/o3/ref.json"
label="$1"
splits="${R79_SPLITS:-dev train}"
for s in $splits; do
  case "$s" in
    dev) dump="$repo/benchmark/results/o3/dump"; ids="$r77/dev-ids.txt" ;;
    train) dump="$r77/dump-train"; ids="$r77/train-ids.txt" ;;
    train2) dump="$out/dump-train2"; ids="$out/train2-ids.txt" ;;
    test) dump="$repo/benchmark/results/o3/dump"; ids="$r77/test-ids.txt" ;;
  esac
  "$refit" "$ref" "$dump" "$ids" "$out/$s-$label.jsonl" "${R79_WORKERS:-4}"
done
