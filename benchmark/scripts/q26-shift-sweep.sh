#!/usr/bin/env bash
# usage: q26-shift-sweep.sh <dump dir> <shift ms>...   grid fit with a global extra shift, within-20ms counts per container
set -u
here="$(cd "$(dirname "$0")/../.." && pwd)"
dump="$1"
shift
for s in "$@"; do
  out="$here/benchmark/results/q26/shift-$s.json"
  TAILN=1 "$here/benchmark/scripts/q26-score.sh" "$dump" test --shift-ms "$s" --diag --out "$out" > /dev/null
  echo -n "shift $s ms: "
  python3 -I "$here/benchmark/scripts/q26-shift-by-ext.py" "$out" "$here/benchmark/results/q26/ref.json"
done
