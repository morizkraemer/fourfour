#!/usr/bin/env bash
# usage: q30-score.sh <dump dir> <split dev|test> [extra grid args...]   re-fits from cached logits (benchmark/results/q30) and prints the grid summary
set -u
here="$(cd "$(dirname "$0")/../.." && pwd)"
dump="$1"
split="$2"
shift 2
"${GRID:-$here/prototyping/analysis-packaging/target/release/grid}" "$here/benchmark/results/q30/ref.json" "$dump" --split "$split" --lo 75 --comb \
  --mp3-rule enc+header+l3100 --snap-tol 0.05 --snap-wide 1.5 --keep-native 150 185 --snap-ratio 0.97 --scan-pct 0.3 "$@" 2>&1 | tail -${TAILN:-12}
