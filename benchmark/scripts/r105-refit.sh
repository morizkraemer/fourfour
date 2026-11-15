#!/usr/bin/env bash
# usage: r105-refit.sh <label> <dump dir> <dev|test|all> [VAR=value ...]
# Re-fits every track of a split from saved logits (grid_refit, R105_* and R79_* switches as env) and scores the rows with o3-score.py.
# Writes $R105/res/<split>-<label>.jsonl, prints the o3-score lines.
set -euo pipefail
here="$(cd "$(dirname "$0")/../.." && pwd)"
work="${R105:-$HOME/.cache/r105-work}"
o3="${O3:-/home/merlin/dev/projects/fourfour/benchmark/results/o3}"
refit="${GRID_REFIT:-$work/grid_refit}"
label="$1"; dump="$2"; split="$3"; shift 3
mkdir -p "$work/res"
out="$work/res/$split-$label.jsonl"
env "$@" "$refit" "$o3/ref.json" "$dump" "$work/$split.ids" "$out" "${WORKERS:-2}" 2> "$work/res/$split-$label.err"
echo "$split $label: $(python3 -I "$here/benchmark/scripts/o3-score.py" "$o3/ref.json" "$out" | tr '\n' ' ')"
