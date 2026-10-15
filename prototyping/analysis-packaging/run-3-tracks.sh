#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
AUDIO="${AUDIO:-/home/merlin/dev/projects/fourfour/benchmark/datasets/beatport-edm-key/audio}"
MODEL_FLAG="${MODEL_FLAG:-}"
mkdir -p results
python3 -I measure.py results/three.jsonl results/three.err.txt target/release/ffproto $MODEL_FLAG \
  "$AUDIO/1389968 Cecile - Talk Talk (Jr. Blender Remix).mp3" \
  "$AUDIO/1393305 Zoom - Five To Eleven (Original Mix).mp3" \
  "$AUDIO/1397802 Taco & Da Mofos - Miracle Whip (Original Mix).mp3"
cat results/three.err.txt
python3 -I - <<'PY'
import json
for l in open("results/three.jsonl"):
    r = json.loads(l)
    print(r["path"].split("/")[-1][:40], round(r["duration_s"]), "s", {k: round(v) for k, v in r["ms"].items()},
          "stratum", round(r["stratum"]["bpm"], 2), r["stratum"]["camelot"], "beats", r["stratum"]["beats"],
          "| beat_this", r["beat_this"] and (round(r["beat_this"]["bpm"] or 0, 2), r["beat_this"]["beats"], round(r["beat_this"]["ms"])))
PY
