#!/usr/bin/env bash
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
while read -r line; do
  [ -z "$line" ] && continue
  "$here/keydet-eval.sh" $line | cut -c1-300
done < "$1"
