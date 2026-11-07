#!/usr/bin/env python3
"""Compares two grid_refit or ffa-derived JSONL runs per split and lists every track whose BPM or grid changed.
usage: r101-compare.py <dir> <label-a> <label-b> [split ...]   (files <dir>/<split>-<label>.jsonl)"""
import json
import sys

d, la, lb = sys.argv[1:4]
splits = sys.argv[4:] or ["test", "dev", "train", "train2"]


def load(path):
    return {json.loads(line)["id"]: json.loads(line) for line in open(path)}


for s in splits:
    a, b = load(f"{d}/{s}-{la}.jsonl"), load(f"{d}/{s}-{lb}.jsonl")
    count = lambda rows, tol: sum(abs(r["bpm"] - r["ref_bpm"]) <= tol for r in rows.values())
    grid = lambda rows: sum(r["max_dev_ms"] <= 20 for r in rows.values())
    print(f"{s} n={len(a)}/{len(b)}  bpm0.1 {count(a, 0.1)} -> {count(b, 0.1)}  bpm1 {count(a, 1)} -> {count(b, 1)}  grid20 {grid(a)} -> {grid(b)}")
    for i in sorted(a):
        if i not in b:
            continue
        x, y = a[i], b[i]
        if abs(x["bpm"] - y["bpm"]) > 1e-6 or abs(x["max_dev_ms"] - y["max_dev_ms"]) > 0.5:
            print(f"  changed {i}: bpm {x['bpm']:.3f} -> {y['bpm']:.3f} (ref {x['ref_bpm']}), max_dev {x['max_dev_ms']:.0f} -> {y['max_dev_ms']:.0f} ms")
