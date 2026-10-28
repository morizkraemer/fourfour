#!/usr/bin/env python3
"""Per-track grid and BPM changes between two grid --out files. usage: o3-diff.py <before.json> <after.json>"""
import json
import sys

a = {t["id"]: t for t in json.load(open(sys.argv[1]))["tracks"]}
b = {t["id"]: t for t in json.load(open(sys.argv[2]))["tracks"]}
for i in sorted(a.keys() & b.keys()):
    x, y = a[i], b[i]
    gx, gy = x["max_dev_ms"] <= 20, y["max_dev_ms"] <= 20
    bx, by = abs(x["est_bpm"] - x["ref_bpm"]) <= 0.1, abs(y["est_bpm"] - y["ref_bpm"]) <= 0.1
    if gx != gy or bx != by:
        print(f"{i} ref {x['ref_bpm']:.3f} bpm {x['est_bpm']:.3f} -> {y['est_bpm']:.3f} max {x['max_dev_ms']:.0f} -> {y['max_dev_ms']:.0f} grid {'+' if gy and not gx else '-' if gx and not gy else '='}")
