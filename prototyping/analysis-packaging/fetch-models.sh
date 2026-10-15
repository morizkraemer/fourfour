#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p models
RAW=https://raw.githubusercontent.com/danigb/beat-this-rs/main/models
REL=https://github.com/danigb/beat-this-rs/releases/download/model-large
curl -fsSL -o models/mel_spectrogram.onnx "$RAW/mel_spectrogram.onnx"
curl -fsSL -o models/beat_this_small.onnx "$RAW/beat_this_small.onnx"
curl -fsSL -o models/beat_this.onnx "$REL/beat_this.onnx"
curl -fsSL -o models/beat_this.onnx.sha256 "$REL/beat_this.onnx.sha256"
(cd models && sha256sum -c beat_this.onnx.sha256)
