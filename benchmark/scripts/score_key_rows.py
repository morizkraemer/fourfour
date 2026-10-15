#!/usr/bin/env python3
import json
import sys

MAJOR = ["8B", "3B", "10B", "5B", "12B", "7B", "2B", "9B", "4B", "11B", "6B", "1B"]
MINOR = ["5A", "12A", "7A", "2A", "9A", "4A", "11A", "6A", "1A", "8A", "3A", "10A"]


def parse(code):
    if code in MAJOR:
        return MAJOR.index(code), False
    return MINOR.index(code), True


def relation(pred, truth):
    (pt, pm), (tt, tm) = parse(pred), parse(truth)
    if (pt, pm) == (tt, tm):
        return "exact"
    if pm != tm and pt == tt:
        return "parallel"
    if pm != tm and ((not pm and pt == (tt + 3) % 12) or (pm and pt == (tt + 9) % 12)):
        return "relative"
    if pm == tm and (pt - tt) % 12 in (5, 7):
        return "fifth"
    return "other_mode" if pm != tm else "other_same_mode"


def main():
    rows = json.load(open(sys.argv[1]))
    split = sys.argv[2] if len(sys.argv) > 2 else "all"
    counts = {}
    n = 0
    for r in rows:
        if "error" in r:
            continue
        if split == "test" and int(r["id"]) % 3 != 0:
            continue
        n += 1
        k = relation(r["camelot"], r["truth"])
        counts[k] = counts.get(k, 0) + 1
    pct = lambda k: 100 * counts.get(k, 0) / n
    print(
        f"split={split} n={n} exact={pct('exact'):.1f}% relative={pct('relative'):.1f}% parallel={pct('parallel'):.1f}% "
        f"fifth={pct('fifth'):.1f}% exact+rel+fifth={pct('exact') + pct('relative') + pct('fifth'):.1f}%"
    )


main()
