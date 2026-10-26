#!/usr/bin/env python3
"""Count tracks within 20 ms per container for a grid --diag --out json.

usage: q26-shift-by-ext.py <grid json> <reference json>
"""
import collections
import json
import sys

tracks = json.load(open(sys.argv[1]))["tracks"]
ref = json.load(open(sys.argv[2]))
n = collections.Counter()
ok = collections.Counter()
for t in tracks:
    ext = ref[str(t["id"])]["path"].rsplit(".", 1)[-1].lower()
    if abs(t["est_bpm"] - t["ref_bpm"]) > 0.1:
        continue
    n[ext] += 1
    ok[ext] += t["max_dev_ms"] <= 20
print(" ".join(f"{e} {ok[e]}/{n[e]}={100 * ok[e] / n[e]:.0f}%" for e in sorted(n)), "| all", sum(ok.values()), "/", sum(n.values()))
