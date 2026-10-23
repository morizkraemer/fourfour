#!/usr/bin/env bash
# usage: q30-sum.sh <label> <dump dir> <split> [grid args...]   one-line fitted summary of q30-score.sh (extra args override the defaults)
set -u
here="$(cd "$(dirname "$0")" && pwd)"
label="$1"; shift
TAILN=200 "$here/q30-score.sh" "$@" --out "$here/../results/q30/last-$label.json" | awk -v l="$label" '/fitted"/ && !done {getline a; getline b; getline c; getline d; print l ": " a " | " b " | " d; done=1}' | tr -s ' ' | cut -c1-400
