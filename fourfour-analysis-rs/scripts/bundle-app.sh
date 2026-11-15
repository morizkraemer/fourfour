#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
MODELS="$ROOT/prototyping/analysis-packaging/models"
REQUIRED="beat_this.onnx mel_spectrogram.onnx LICENSE-beat-this.txt"
CONFIG=tauri.bundle-weights.conf.json
FEATURES=()
if [ "$(uname -s)" = "Darwin" ]; then
  CONFIG=tauri.bundle-weights-mac.conf.json
  FEATURES=(--features coreml)
fi
missing() {
  for f in $REQUIRED; do
    [ -s "$MODELS/$f" ] || return 0
  done
  return 1
}
if missing; then
  "$ROOT/fourfour-analysis-rs/scripts/fetch-models.sh" "$MODELS"
fi
if [ "$(uname -s)" = "Darwin" ]; then
  "$ROOT/scripts/mac/int8-model.sh" "$MODELS"
fi
for f in $REQUIRED; do
  [ -s "$MODELS/$f" ] || { echo "bundle-app: $MODELS/$f missing or empty; refusing to bundle without model weights" >&2; exit 1; }
done
cd "$ROOT/pioneer-test-ui"
exec cargo tauri build --config "$CONFIG" ${FEATURES[@]+"${FEATURES[@]}"} "$@"
