#!/usr/bin/env python3
"""Lists beat-aligned tracks where the old bar-1 rule (model vote) and the active rule disagree, for listening.
usage: b15-bar-listen.py <ref.json> <ffa.jsonl>... [--max N]
Needs the "bar_phase" and "bar_phase_model_vote" fields that ffa writes. Bar-1 time = time of the first beat the rule numbers as bar 1
(final beat grid plus mp3 delay already applied, as stored). Rekordbox = its first beat with bar position 1. "right" = at least 90% of
the Rekordbox beats matched within 35 ms carry the same bar position under that rule (same test as o3-score.py)."""
import bisect
import json
import sys


def agreement(est_ms, phase, rb, rbar):
    same = matched = 0
    for t, b in zip(rb, rbar):
        j = bisect.bisect_left(est_ms, t)
        c = min((k for k in (j - 1, j) if 0 <= k < len(est_ms)), key=lambda k: abs(est_ms[k] - t))
        if abs(est_ms[c] - t) <= 35:
            matched += 1
            same += (c - phase) % 4 + 1 == b
    return same / matched if matched else None


def main():
    limit = int(sys.argv[sys.argv.index("--max") + 1]) if "--max" in sys.argv else 10
    ref = json.load(open(sys.argv[1]))
    files = [a for i, a in enumerate(sys.argv[2:], 2) if a != "--max" and sys.argv[i - 1] != "--max"]
    rows = []
    for path in files:
        for line in open(path):
            a = json.loads(line)
            if a.get("id") is None or a.get("bpm") is None:
                continue
            r = ref[str(a["id"])]
            beats = a["final_beats_s"]
            est = [round(t * 1000.0) for t in beats]
            if abs(a["bpm"] - r["bpm_masterdb"]) > 0.5 or len(beats) < 8:
                continue
            old, new = a["bar_phase_model_vote"], a["bar_phase"]
            if old == new:
                continue
            ag_old = agreement(est, old, r["beats_ms"], r["bar_pos"])
            ag_new = agreement(est, new, r["beats_ms"], r["bar_pos"])
            if ag_old is None or ag_new is None or max(ag_old, ag_new) < 0.9:
                continue
            rb1 = next(t for t, b in zip(r["beats_ms"], r["bar_pos"]) if b == 1)
            rows.append((a["id"], r["path"].rsplit("/", 1)[-1], est[old] / 1000, est[new] / 1000, rb1 / 1000, ag_old >= 0.9, ag_new >= 0.9))
    new_right = [x for x in rows if x[6] and not x[5]]
    old_right = [x for x in rows if x[5] and not x[6]]
    print(f"disagreeing beat-aligned tracks: {len(rows)}; new right {len(new_right)}, old right {len(old_right)}, neither {len(rows) - len(new_right) - len(old_right)}")
    pick = new_right[: limit - limit // 3] + old_right[: limit // 3]
    print("| id | file | old bar 1 (s) | new bar 1 (s) | Rekordbox bar 1 (s) | right |")
    print("|---:|---|---:|---:|---:|---|")
    for id_, name, o, n, rbt, ok_old, ok_new in pick:
        print(f"| {id_} | {name} | {o:.3f} | {n:.3f} | {rbt:.3f} | {'new' if ok_new else 'old'} |")


main()
