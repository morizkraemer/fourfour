#!/usr/bin/env python3
"""Classify tracks that miss the 20 ms grid test. usage: q26-fail-classes.py <grid --diag --out json>"""
import json
import sys

tracks = json.load(open(sys.argv[1]))["tracks"]
fails = [t for t in tracks if abs(t["est_bpm"] - t["ref_bpm"]) <= 0.1 and t["max_dev_ms"] > 20]
bpm_bad = [t for t in tracks if abs(t["est_bpm"] - t["ref_bpm"]) > 0.1]
classes = {"phase >100 ms": [], "median offset 20-100 ms": [], "median within 20 ms (drift or outlier beats)": []}
for t in fails:
    m = abs(t["med_offset_ms"])
    key = "phase >100 ms" if m > 100 else "median offset 20-100 ms" if m > 20 else "median within 20 ms (drift or outlier beats)"
    classes[key].append(t)
print("tracks", len(tracks), "bpm miss", len(bpm_bad), "bpm ok but grid miss", len(fails))
for k, v in classes.items():
    print(f"{k}: {len(v)}  ref_constant {sum(t['ref_constant'] for t in v)}")
    for t in v:
        print("   id", t["id"], "med %.1f max %.1f drift %.1f detrended %.1f" % (t["med_offset_ms"], t["max_dev_ms"], t["drift_ms"], t["detrended_max_ms"]))
print("bpm misses with reference constant:", sum(t["ref_constant"] for t in bpm_bad))
