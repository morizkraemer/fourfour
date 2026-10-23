#!/usr/bin/env bash
# usage: q21-variants.sh <list file> <threads> <variant>...   runs ffa once per variant (in parallel) with that beat model
# variants: fp32 (beat_this.onnx), small, or a suffix such as q8mm (beat_this_q8mm.onnx). Outputs in benchmark/results/q21/var-<variant>.*
set -u
here="$(cd "$(dirname "$0")/../.." && pwd)"
ffa="$here/prototyping/analysis-packaging/target/release/ffa"
out="$here/benchmark/results/q21"
list="$1"
threads="$2"
shift 2
for v in "$@"; do
  case "$v" in
    fp32) model=beat_this.onnx ;;
    small) model=beat_this_small.onnx ;;
    *) model="beat_this_$v.onnx" ;;
  esac
  "$here/benchmark/scripts/q21-timed.sh" "$out/var-$v" "$ffa" --threads "$threads" --brief --no-key --no-waveform \
    --beat-model "$model" --reference "$out/ref50.json" --dump "$out/dump-$v" --list "$list" > "$out/var-$v.sum" 2>&1 &
done
wait
for v in "$@"; do
  echo "== $v"
  cat "$out/var-$v.sum"
  grep -o '"predict":[0-9.]*' "$out/var-$v.out" | head -50 | awk -F: '{s+=$2; n++} END {printf "predict mean %.0f ms over %d tracks\n", s/n, n}'
done
