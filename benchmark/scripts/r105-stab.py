#!/usr/bin/env python3
"""Scores one grid_refit JSONL (500 logits) against another (1500 logits) and against the reference.
usage: r105-stab.py <ref.json> <a.jsonl> <b.jsonl> [--base base.jsonl] [--ids ids.txt]
Agreement of a with b on a track: BPM within 0.05 and median grid offset within 15 ms.
Right = BPM within 0.1 / within 1 / grid within 20 ms / bar >= 90% (as o3-score.py). --base lists right -> wrong and wrong -> right of a against base."""
import bisect
import json
import statistics
import sys


def load(path):
    return {str(json.loads(line)["id"]): json.loads(line) for line in open(path)}


def grid_offset_ms(a, b):
    xs = b["final_beats_s"]
    diffs = []
    for t in a["final_beats_s"]:
        j = bisect.bisect_left(xs, t)
        c = min((xs[k] - t for k in (j - 1, j) if 0 <= k < len(xs)), key=abs, default=None)
        if c is not None:
            diffs.append(c * 1000.0)
    return statistics.median(diffs) if diffs else 1e9


def agree(a, b):
    return abs(a["bpm"] - b["bpm"]) <= 0.05 and abs(grid_offset_ms(a, b)) <= 15.0


def bar_right(r, ref):
    rr = ref[str(r["id"])]
    est = [round(t * 1000.0) for t in r["final_beats_s"]]
    ebar = [(i + 4 - r["bar_phase"]) % 4 + 1 for i in range(len(est))]
    same = matched = 0
    for t, b in zip(rr["beats_ms"], rr["bar_pos"]):
        j = bisect.bisect_left(est, t)
        c = min((k for k in (j - 1, j) if 0 <= k < len(est)), key=lambda k: abs(est[k] - t), default=None)
        if c is not None and abs(est[c] - t) <= 35:
            matched += 1
            same += ebar[c] == b
    aligned = abs(r["bpm"] - r["ref_bpm"]) <= 0.5 and abs(r["med_offset_ms"]) < 35 and matched > 0
    return aligned and same / matched >= 0.9


CRIT = {
    "bpm0.1": lambda r, ref: abs(r["bpm"] - r["ref_bpm"]) <= 0.1,
    "bpm1": lambda r, ref: abs(r["bpm"] - r["ref_bpm"]) <= 1.0,
    "grid20": lambda r, ref: r["max_dev_ms"] <= 20,
    "bar": bar_right,
}


def main():
    ref = json.load(open(sys.argv[1]))
    a, b = load(sys.argv[2]), load(sys.argv[3])
    base = load(sys.argv[sys.argv.index("--base") + 1]) if "--base" in sys.argv else None
    ids = set(a) & set(b)
    if "--ids" in sys.argv:
        ids &= {l.strip() for l in open(sys.argv[sys.argv.index("--ids") + 1]) if l.strip()}
    ids = sorted(ids, key=int)
    cnt = lambda rows, f: sum(f(rows[i], ref) for i in ids)
    summary = " ".join(f"{k} {cnt(a, f)}/{cnt(b, f)}" for k, f in CRIT.items())
    dis = [i for i in ids if not agree(a[i], b[i])]
    print(f"n {len(ids)}  a/b: {summary}  agree {len(ids) - len(dis)}/{len(ids)}")
    if "--list" in sys.argv:
        print("  disagree:", " ".join(f"{i}({a[i]['bpm']:.3f}|{b[i]['bpm']:.3f}|ref {a[i]['ref_bpm']})" for i in dis))
    if base:
        for k, f in CRIT.items():
            lost = [i for i in ids if i in base and f(base[i], ref) and not f(a[i], ref)]
            won = [i for i in ids if i in base and not f(base[i], ref) and f(a[i], ref)]
            print(f"  vs base {k}: right->wrong {len(lost)} {lost}  wrong->right {len(won)} {won}")


main()
