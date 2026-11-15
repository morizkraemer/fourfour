#!/bin/bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
STAMP="$(date +%Y%m%d-%H%M)"
OUT="$ROOT/build/mac/dmg-$STAMP"
MODELS="$ROOT/prototyping/analysis-packaging/models"
DEV_INSTALL_MODELS="$HOME/dev/build/fourfour-mac/prototyping/analysis-packaging/models"

mkdir -p "$OUT"
exec > >(tee "$OUT/build.log") 2>&1
echo "build-dmg start $(date) root=$ROOT"

unset APPLE_SIGNING_IDENTITY APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID
export CI=true

mkdir -p "$MODELS"
for f in beat_this.onnx beat_this_q8sc.onnx mel_spectrogram.onnx; do
  if [ ! -f "$MODELS/$f" ] && [ -f "$DEV_INSTALL_MODELS/$f" ]; then
    cp "$DEV_INSTALL_MODELS/$f" "$MODELS/$f"
  fi
done
if [ ! -f "$MODELS/beat_this.onnx" ] || [ ! -f "$MODELS/mel_spectrogram.onnx" ]; then
  "$ROOT/prototyping/analysis-packaging/fetch-models.sh"
fi
"$ROOT/scripts/mac/int8-model.sh" "$MODELS"
cp "$ROOT/scripts/mac/LICENSE-beat-this.txt" "$MODELS/LICENSE-beat-this.txt"
cp "$ROOT/scripts/mac/LICENSE-key-model.txt" "$MODELS/LICENSE-key-model.txt"
(cd "$MODELS" && shasum -a 256 beat_this_q8sc.onnx mel_spectrogram.onnx LICENSE-beat-this.txt)

(cd "$ROOT/app" && npm ci)

cd "$ROOT/pioneer-test-ui"
caffeinate -i cargo tauri build --config tauri.bundle-weights-mac.conf.json --features coreml --bundles dmg

DMG="$(ls -t "$ROOT"/target/release/bundle/dmg/*.dmg | head -1)"
[ -f "$DMG" ] || { echo "FAIL no dmg produced"; exit 1; }
cp "$DMG" "$OUT/"
NAME="$(basename "$DMG")"
(cd "$OUT" && shasum -a 256 "$NAME" > "$NAME.sha256")
ls -l "$OUT/$NAME"
cat "$OUT/$NAME.sha256"
echo "$OUT/$NAME" > "$OUT/dmg-path.txt"
echo "build-dmg done $(date)"
