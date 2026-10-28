#!/usr/bin/env bash
# usage: o3-fit.sh <label> <dump dir> <split dev|test> [grid args...]   re-fits cached logits with the app's settings (extra args override) and prints the summary
set -u
here="$(cd "$(dirname "$0")/../.." && pwd)"
label="$1"; dump="$2"; split="$3"
shift 3
"${GRID:-$here/prototyping/analysis-packaging/target/release/grid}" "$here/benchmark/results/o3/ref.json" "$dump" --split "$split" --lo 75 --comb \
  --mp3-rule enc+header+l3100 --snap-tol 0.05 --snap-wide 1.5 --keep-native 150 185 --snap-ratio 0.97 --scan-pct 0.3 "$@" \
  --out "$here/benchmark/results/o3/v-$label.json" 2>&1 | sed -n '1,6p' | sed "s/^/$label: /"
