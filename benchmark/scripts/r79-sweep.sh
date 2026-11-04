#!/usr/bin/env bash
# usage: r79-sweep.sh   re-fits R79_SPLITS (default dev train train2) with the old tempo search and the global peak, then diffs them
set -euo pipefail
here="$(cd "$(dirname "$0")/../.." && pwd)"
out="${R79_OUT:-/home/merlin/dev/projects/fourfour/benchmark/results/r79}"
export R79_SPLITS="${R79_SPLITS:-dev train train2}" R79_WORKERS="${R79_WORKERS:-3}"
R79_TEMPO_SEARCH=line "$here/benchmark/scripts/r79-run.sh" old
R79_TEMPO_SEARCH=global "$here/benchmark/scripts/r79-run.sh" global
for s in $R79_SPLITS; do
  echo "== $s"
  python3 -I "$here/benchmark/scripts/r79-diff.py" "$out/$s-old.jsonl" "$out/$s-global.jsonl"
  python3 -I "$here/benchmark/scripts/r77-compare.py" "$out/$s-old.jsonl" "$out/$s-global.jsonl" | grep -E '^(base|new)'
done
