#!/usr/bin/env python3
"""Compares per-track results of two runs and lists right -> wrong flips.
usage: r102-flips.py <base> <variant> [--ids ids.txt]
Each input is either an o3-score --rows JSON or a grid_refit JSONL (fields id, bpm, ref_bpm, max_dev_ms).
Criteria: BPM within 0.1, BPM within 1, grid within 20 ms."""
import json
import sys


def load(path):
    text = open(path).read()
    try:
        rows = json.loads(text)
    except json.JSONDecodeError:
        rows = [json.loads(line) for line in text.splitlines() if line.strip()]
    return {str(r["id"]): r for r in rows}


CRITERIA = {
    "bpm0.1": lambda r: abs(r["bpm"] - r["ref_bpm"]) <= 0.1,
    "bpm1": lambda r: abs(r["bpm"] - r["ref_bpm"]) <= 1.0,
    "grid20": lambda r: r["max_dev_ms"] <= 20,
}


def main():
    base, var = load(sys.argv[1]), load(sys.argv[2])
    ids = set(base) & set(var)
    if "--ids" in sys.argv:
        ids &= {l.strip() for l in open(sys.argv[sys.argv.index("--ids") + 1]) if l.strip()}
    print(f"tracks {len(ids)}")
    for name, ok in CRITERIA.items():
        b = sum(ok(base[i]) for i in ids)
        v = sum(ok(var[i]) for i in ids)
        lost = sorted((i for i in ids if ok(base[i]) and not ok(var[i])), key=int)
        won = sorted((i for i in ids if not ok(base[i]) and ok(var[i])), key=int)
        print(f"{name}: base {b} variant {v}; right->wrong {len(lost)} {lost}; wrong->right {len(won)} {won}")
        for i in lost:
            print(f"   {i}: ref {base[i]['ref_bpm']:.3f} base {base[i]['bpm']:.3f} variant {var[i]['bpm']:.3f} dev {base[i]['max_dev_ms']:.0f}->{var[i]['max_dev_ms']:.0f} ms")


main()
