#!/usr/bin/env bash
# usage: r105-sweep.sh <dump dir> <dump label> <variant label> [VAR=value ...]
# Re-fits dev and test from <dump dir> with the switches and prints one scoring line per split.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
dump="$1"; dl="$2"; vl="$3"; shift 3
for split in test dev; do
  "$here/r105-refit.sh" "$vl-$dl" "$dump" "$split" "$@" | sed -E 's/bpm within 0.5: [^b]*//; s/beat-aligned.*(bar position)/\1/; s/bar-wrong.*//'
done
