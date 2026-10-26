#!/usr/bin/env python3
"""Lists tracks whose BPM is off by more than 1 with the ratio and a category. usage: q30-bpm-misses.py <grid out json> <ref.json> [dev|test]"""
import json, sys, collections

tr = json.load(open(sys.argv[1]))["tracks"]
ref = json.load(open(sys.argv[2]))
cats = collections.Counter()


def cat(est, r):
    q = est / r
    if abs(est - r) <= 1:
        return "ok"
    for name, x in (("x2", 2), ("x0.5", 0.5), ("x1.5", 1.5), ("x2/3", 2 / 3), ("x4/3", 4 / 3), ("x3/4", 3 / 4)):
        if abs(q - x) < 0.02 * x:
            return name
    return "other"


for t in sorted(tr, key=lambda t: t["id"]):
    c = cat(t["est_bpm"], t["ref_bpm"])
    cats[c] += 1
    if c != "ok":
        v = ref[str(t["id"])]
        print(f"{t['id']:>4} est {t['est_bpm']:7.2f} ref {t['ref_bpm']:7.2f} ratio {t['est_bpm'] / t['ref_bpm']:.3f} {c:6} const={t['ref_constant']} {v.get('artist', '')[:22]} - {v.get('title', '')[:32]}")
print(dict(cats))
