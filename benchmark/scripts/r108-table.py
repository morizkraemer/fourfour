#!/usr/bin/env python3
"""R108: summarise r108-run.py result lines: median and spread per label.

usage: r108-table.py results.jsonl [label-prefix]
Columns: runs, tracks/min median (min-max) as measured, steady tracks/min (middle half of the batch), CPU per track,
peak RSS, first result, idle share of the beat-model threads, load at start, and "quiet" tracks/min.

The box runs other work (load 20-45 on 16 cores), and the scheduler hands a process cores in proportion to its runnable
threads, so a config with more threads gets a bigger share and looks faster than it would alone. "quiet" removes that:
the batch's CPU seconds plus the beat-model threads' idle seconds, spread over the cores the run may use (at most its thread count)
(a machine with nothing else running finishes in that time), as tracks per minute.
"""
import json
import statistics as st
import sys


def steady(row):
    done = row.get("done_at_s")
    if not done or len(done) < 8:
        return None
    n = len(done)
    lo, hi = n // 4, (3 * n) // 4
    return (hi - lo) / (done[hi - 1] - done[lo - 1]) * 60


def cores_of(row):
    cores = row.get("cores")
    allowed = 16
    if cores:
        lo, _, hi = cores.partition("-")
        allowed = int(hi) - int(lo) + 1 if hi else 1
    cmd = row["cmd"]
    threads = int(cmd.split("--threads ")[1].split()[0]) if "--threads " in cmd else allowed
    return min(allowed, threads)


def quiet(row):
    if row.get("chunk_idle_s") is None:
        return None
    return row["tracks"] * 60 / ((row["user_s"] + row["sys_s"] + row["chunk_idle_s"]) / cores_of(row))


def span(values, digits=1):
    values = [v for v in values if v is not None]
    if not values:
        return "-"
    med = st.median(values)
    return f"{med:.{digits}f} ({min(values):.{digits}f}-{max(values):.{digits}f})"


rows = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
prefix = sys.argv[2] if len(sys.argv) > 2 else ""
labels = []
for r in rows:
    if r["label"].startswith(prefix) and r["label"] not in labels:
        labels.append(r["label"])
print(f"{'config':<14}{'n':>3}  {'trk/min med (min-max)':<24}{'steady trk/min':<22}{'cpu/trk s':<18}{'rss MB':<18}{'first s':<14}{'idle %':<8}{'load':<6}{'quiet trk/min'}")
for label in labels:
    mine = [r for r in rows if r["label"] == label and r["rc"] == 0]
    if not mine:
        continue
    threads = None
    idle = []
    for r in mine:
        if r.get("chunk_idle_s") is not None and r.get("starting"):
            t = int(r["cmd"].split("--threads ")[1].split()[0]) if "--threads " in r["cmd"] else 16
            idle.append(100 * r["chunk_idle_s"] / (t * r["wall_s"]))
    print(
        f"{label:<14}{len(mine):>3}  {span([r['tracks_per_min'] for r in mine], 2):<24}{span([steady(r) for r in mine], 2):<22}"
        f"{span([r['cpu_per_track_s'] for r in mine]):<18}{span([r['maxrss_mb'] for r in mine], 0):<18}"
        f"{span([r.get('first_s') for r in mine]):<14}{span(idle, 0) if idle else '-':<8}{st.median([r['load_before'] for r in mine]):<6.0f}{span([quiet(r) for r in mine], 2)}"
    )
