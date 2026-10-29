#!/usr/bin/env python3
"""Summarises grid_offset rows: median and mean of the per-track median offsets by container and mp3 encoder.
usage: fb-grid-offsets.py <rows.jsonl> [--worst N] [--bpm-tol 0.5]"""
import json
import statistics
import sys

FIELDS = ["model_vs_onset_ms", "app_vs_heard_ms", "app_vs_wave_ms", "rb_vs_untrimmed_ms", "rb_vs_trimmed_ms", "app_vs_rb_ms"]


def group(r):
    if r["ext"] != "mp3":
        return r["ext"]
    enc = (r.get("encoder") or "none")[:9]
    return f"mp3:{enc}" + ("+hdr" if r.get("header_counts") else "")


def main():
    rows = [json.loads(l) for l in open(sys.argv[1])]
    rows = [r for r in rows if "error" not in r]
    worst = int(sys.argv[sys.argv.index("--worst") + 1]) if "--worst" in sys.argv else 10
    tol = float(sys.argv[sys.argv.index("--bpm-tol") + 1]) if "--bpm-tol" in sys.argv else 0.5
    n = len(rows)
    hit = sum(r["max_dev_ms"] <= 20 for r in rows)
    print(f"n {n}; grid within 20 ms of Rekordbox: {hit} = {100 * hit / n:.1f}%")
    ok = [r for r in rows if r["ref_bpm"] is not None and abs(r["bpm"] - r["ref_bpm"]) <= tol]
    print(f"offset stats on {len(ok)} tracks with BPM within {tol} (median / mean of per-track medians, ms)")
    groups = {}
    for r in ok:
        groups.setdefault(r["ext"], []).append(r)
        if r["ext"] == "mp3":
            groups.setdefault(group(r), []).append(r)
    groups["all"] = ok
    print(f"{'group':28} {'n':>4} " + " ".join(f"{f[:-3]:>18}" for f in FIELDS))
    for g in sorted(groups):
        rs = groups[g]
        cells = []
        for f in FIELDS:
            v = [r[f] for r in rs if r.get(f) is not None]
            cells.append(f"{statistics.median(v):+6.1f} / {statistics.mean(v):+6.1f}" if v else "n/a")
        print(f"{g:28} {len(rs):>4} " + " ".join(f"{c:>18}" for c in cells))
    print(f"\nworst {worst} by |app_vs_wave| (BPM within {tol}):")
    fmt = lambda v: f"{v:+6.1f}" if v is not None else "   n/a"
    measured = [r for r in ok if r.get("app_vs_wave_ms") is not None]
    for r in sorted(measured, key=lambda r: -abs(r["app_vs_wave_ms"]))[:worst]:
        name = r["path"].rsplit("/", 1)[-1][:50]
        print(f"  {r['id']:>4} {group(r):24} heard {fmt(r['app_vs_heard_ms'])} wave {fmt(r['app_vs_wave_ms'])} model {fmt(r['model_vs_onset_ms'])} vsRB {fmt(r['app_vs_rb_ms'])}  {name}")


main()
