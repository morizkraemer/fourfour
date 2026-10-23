#!/usr/bin/env bash
# usage: q26-ort-matrix.sh <python with onnxruntime>   runs the chunk bench over the Q26 variant matrix
set -u
here="$(cd "$(dirname "$0")/../.." && pwd)"
py="$1"
b="$here/benchmark/scripts/q26-ort-chunk-bench.py"
m="$here/prototyping/analysis-packaging/models"
while read -r model threads batch runs opt; do
  "$py" "$b" "$m/$model.onnx" "$threads" "$batch" "$runs" "$opt" 2>&1 | grep -v -i warn
done <<'EOF'
beat_this 1 1 3 all
beat_this 1 4 1 all
beat_this 1 1 3 basic
beat_this_q8mm 1 1 3 all
beat_this_q8all 1 1 3 all
beat_this 4 1 3 all
beat_this 1 1 3 all
EOF
