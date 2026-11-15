#!/usr/bin/env bash
# usage: int8-model.sh [models dir]    FOURFOUR_INT8_MODEL=<file> copies that file instead of quantizing
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
MODELS="${1:-$ROOT/prototyping/analysis-packaging/models}"
OUT="$MODELS/beat_this_q8sc.onnx"
FP32_SHA=5f810debe53459b559127fb55bbad40035bb47cc567b20e501670f968c770f02
INT8_SHA=f315e4684715999897fea9d9c7581e53b43fa73dc32f96aabee0cc58d9b2fe78
VENV="${FOURFOUR_QUANT_VENV:-$HOME/.cache/fourfour-quant-venv}"
PINS=(onnx==1.23.2 onnxruntime==1.31.1 numpy==2.5.4 protobuf==7.36.2 ml_dtypes==0.6.0 flatbuffers==25.12.19 packaging==26.3)

sha() { if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi; }

if [ -s "$OUT" ] && [ "$(sha "$OUT")" = "$INT8_SHA" ]; then
  echo "int8 beat model ok: $OUT"
  exit 0
fi

if [ -n "${FOURFOUR_INT8_MODEL:-}" ]; then
  cp "$FOURFOUR_INT8_MODEL" "$OUT.tmp"
else
  [ -s "$MODELS/beat_this.onnx" ] || { echo "int8-model: $MODELS/beat_this.onnx missing (run the fetch-models script first)" >&2; exit 1; }
  got="$(sha "$MODELS/beat_this.onnx")"
  [ "$got" = "$FP32_SHA" ] || { echo "int8-model: beat_this.onnx sha256 $got, expected $FP32_SHA" >&2; exit 1; }
  if [ ! -x "$VENV/bin/python" ]; then
    python3 -m venv "$VENV"
    "$VENV/bin/pip" install --quiet "${PINS[@]}"
  fi
  "$VENV/bin/python" -I "$ROOT/fourfour-analysis-rs/scripts/quantize_beat_int8.py" "$MODELS/beat_this.onnx" "$OUT.tmp" --per-channel
fi

got="$(sha "$OUT.tmp")"
if [ "$got" != "$INT8_SHA" ]; then
  echo "int8-model: result sha256 $got, expected $INT8_SHA; left at $OUT.tmp" >&2
  echo "int8-model: copy the reference file from dev (~/.cache/r97-models/beat_this_q8sc.onnx) and rerun with FOURFOUR_INT8_MODEL=<file>" >&2
  exit 1
fi
mv "$OUT.tmp" "$OUT"
echo "int8 beat model written: $OUT"
