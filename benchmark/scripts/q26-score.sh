#!/usr/bin/env bash
# usage: q26-score.sh <dump dir> [split dev|test] [extra grid args...]   re-fits from the logits and prints the grid summary
set -u
here="$(cd "$(dirname "$0")/../.." && pwd)"
dump="$1"
split="${2:-test}"
shift 2 2>/dev/null || shift $#
"${GRID:-$here/prototyping/analysis-packaging/target/release/grid}" "$here/benchmark/results/q26/${REF:-ref.json}" "$dump" --split "$split" --lo 75 --comb \
  --mp3-rule enc+header --snap-tol 0.05 --snap-wide 0.5 --snap-ratio 0.97 --scan-pct 0.3 "$@" 2>&1 | tail -${TAILN:-12}
