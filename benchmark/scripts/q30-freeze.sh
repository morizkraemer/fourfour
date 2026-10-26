#!/usr/bin/env bash
# usage: q30-freeze.sh <dump dir> <frozen dir>   symlinks the tracks dumped so far, so comparisons run on a fixed set while a batch is still running
set -eu
mkdir -p "$2"
for j in "$1"/*.json; do
  id=$(basename "$j" .json)
  [ -e "$1/$id.logits" ] && ln -sf "$(cd "$1" && pwd)/$id.json" "$2/$id.json" && ln -sf "$(cd "$1" && pwd)/$id.logits" "$2/$id.logits"
done
ls "$2"/*.json | wc -l
