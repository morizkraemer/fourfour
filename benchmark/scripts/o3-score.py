#!/usr/bin/env python3
"""Scores ffa JSONL output (the app's final grid) against the Rekordbox reference: BPM, whole-grid +-20 ms, and bar position (downbeat).
usage: o3-score.py <ref.json> <ffa.jsonl> [--rows out.json]
Bar position uses the same vote as fourfour_analysis::service::bar_phase. A track's downbeat counts as right when at least 90% of the
Rekordbox beats matched within 35 ms carry the same bar position (1..4) in both grids; scored only on tracks with BPM within 0.5 and
median |offset| under 35 ms (beat-level agreement), because bar numbers are meaningless otherwise."""
import bisect
import json
import statistics
import sys


def bar_phase(beats, downbeats, delay):
    if len(beats) < 2:
        return 0
    period = (beats[-1] - beats[0]) / (len(beats) - 1)
    votes = [0, 0, 0, 0]
    for d in downbeats:
        t = d + delay
        after = bisect.bisect_left(beats, t)
        cands = [c for c in (after - 1, after) if 0 <= c < len(beats)]
        nearest = min(cands, key=lambda c: (abs(beats[c] - t), c))
        if abs(beats[nearest] - t) < period / 2:
            votes[nearest % 4] += 1
    return max(range(4), key=lambda p: (votes[p], -p))


def main():
    ref = json.load(open(sys.argv[1]))
    rows_out = sys.argv[sys.argv.index("--rows") + 1] if "--rows" in sys.argv else None
    rows = []
    for line in open(sys.argv[2]):
        a = json.loads(line)
        if a.get("id") is None or a.get("bpm") is None:
            continue
        r = ref[str(a["id"])]
        rb = r["beats_ms"]
        rbar = r["bar_pos"]
        est = [round(t * 1000.0) for t in a["final_beats_s"]]
        phase = bar_phase(a["final_beats_s"], a["downbeats_s"], a["mp3_delay_ms"] / 1000.0)
        ebar = [(i + 4 - phase) % 4 + 1 for i in range(len(est))]
        offs, same, matched = [], 0, 0
        for t, b in zip(rb, rbar):
            j = bisect.bisect_left(est, t)
            c = min((k for k in (j - 1, j) if 0 <= k < len(est)), key=lambda k: abs(est[k] - t))
            d = est[c] - t
            offs.append(d)
            if abs(d) <= 35:
                matched += 1
                same += ebar[c] == b
        med = statistics.median(offs)
        rows.append({
            "id": a["id"], "path": r["path"], "ref_bpm": r["bpm_masterdb"], "bpm": a["bpm"],
            "max_dev_ms": max(abs(x) for x in offs), "med_offset_ms": med,
            "bar_agree": same / matched if matched else None, "matched": matched / len(rb),
        })
    n = len(rows)
    pct = lambda c: f"{c} = {100 * c / n:.1f}%"
    print(f"n {n}")
    for tol in (0.1, 0.5, 1.0):
        print(f"bpm within {tol}: {pct(sum(abs(x['bpm'] - x['ref_bpm']) <= tol for x in rows))}")
    print(f"grid within 20 ms: {pct(sum(x['max_dev_ms'] <= 20 for x in rows))}")
    beat_ok = [x for x in rows if abs(x["bpm"] - x["ref_bpm"]) <= 0.5 and abs(x["med_offset_ms"]) < 35 and x["bar_agree"] is not None]
    bar_ok = [x for x in beat_ok if x["bar_agree"] >= 0.9]
    bar_off = {}
    for x in beat_ok:
        if x["bar_agree"] < 0.9:
            bar_off[round(x["bar_agree"], 1)] = bar_off.get(round(x["bar_agree"], 1), 0) + 1
    print(f"beat-aligned tracks: {len(beat_ok)}; bar position right (>=90% of beats): {len(bar_ok)} = {100 * len(bar_ok) / max(1, len(beat_ok)):.1f}% of aligned, {pct(len(bar_ok))} of all")
    print(f"bar-wrong agreement histogram: {dict(sorted(bar_off.items()))}")
    g20 = [x for x in rows if x["max_dev_ms"] <= 20]
    print(f"grid within 20 ms AND bar right: {pct(sum(1 for x in g20 if x['bar_agree'] is not None and x['bar_agree'] >= 0.9))}")
    if rows_out:
        json.dump(rows, open(rows_out, "w"), indent=0)


main()
