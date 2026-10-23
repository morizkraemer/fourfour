#!/usr/bin/env python3
"""Mean seconds per stage from ffa --brief output lines. usage: q26-stage-means.py <ffa .out>..."""
import json
import sys

keys = ["decode", "resample", "mel", "predict", "fit", "key", "waveform", "total"]
for path in sys.argv[1:]:
    rows = [json.loads(line) for line in open(path)]
    means = {k: sum(r["ms"][k] for r in rows) / len(rows) / 1000 for k in keys}
    print(path.rsplit("/", 1)[-1], len(rows), {k: round(v, 2) for k, v in means.items()})
