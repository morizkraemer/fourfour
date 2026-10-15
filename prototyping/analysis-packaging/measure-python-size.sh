#!/usr/bin/env bash
set -euo pipefail
WORK="${1:?usage: measure-python-size.sh <empty-dir>}"
uv venv "$WORK/venv"
uv pip install --python "$WORK/venv/bin/python" \
  --extra-index-url https://download.pytorch.org/whl/cpu --index-strategy unsafe-best-match \
  numpy scipy soundfile essentia torch librosa deeprhythm mutagen
du -sh "$WORK/venv"
SP="$(echo "$WORK"/venv/lib/python*/site-packages)"
du -sm "$SP"/* | sort -rn | head -15
