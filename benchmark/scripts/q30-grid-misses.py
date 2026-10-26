#!/usr/bin/env python3
"""Tracks with BPM within 1 and a missed 20 ms grid, grouped by cause. usage: q30-grid-misses.py <grid out json>"""
import json, sys, collections
tr = json.load(open(sys.argv[1]))["tracks"]
cls = collections.defaultdict(list)
for t in tr:
    if abs(t["est_bpm"] - t["ref_bpm"]) > 1 or t["max_dev_ms"] <= 20:
        continue
    h = t["period_ms"] / 2
    off = t["med_offset_ms"]
    if abs(abs(off) - h) < 70:
        c = "half-beat"
    elif abs(off) > 20 and t["detrended_max_ms"] < 20:
        c = "constant offset"
    elif abs(off) > 20:
        c = "offset + wobble"
    elif abs(t["drift_ms"]) > 15 and t["ref_constant"]:
        c = "drift"
    else:
        c = "outliers"
    cls[c].append(t)
for c, v in cls.items():
    print(c, len(v))
    for t in sorted(v, key=lambda t: t["id"]):
        print("   ", t["id"], "off %.0f drift %.0f detr %.0f max %.0f period %.0f ref_const %s %s" % (t["med_offset_ms"], t["drift_ms"], t["detrended_max_ms"], t["max_dev_ms"], t["period_ms"], t["ref_constant"], t["container"]))
