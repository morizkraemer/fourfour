#!/usr/bin/env bash
set -euo pipefail
DEST="${1:-$(cd "$(dirname "$0")/../.." && pwd)/prototyping/analysis-packaging/models}"
RAW=https://raw.githubusercontent.com/danigb/beat-this-rs/main/models
REL=https://github.com/danigb/beat-this-rs/releases/download/model-large
mkdir -p "$DEST"
curl -fsSL -o "$DEST/mel_spectrogram.onnx" "$RAW/mel_spectrogram.onnx"
curl -fsSL -o "$DEST/beat_this.onnx" "$REL/beat_this.onnx"
curl -fsSL -o "$DEST/beat_this.onnx.sha256" "$REL/beat_this.onnx.sha256"
(cd "$DEST" && sha256sum -c beat_this.onnx.sha256 2>/dev/null || shasum -a 256 -c beat_this.onnx.sha256)
echo "model weights installed in $DEST"
