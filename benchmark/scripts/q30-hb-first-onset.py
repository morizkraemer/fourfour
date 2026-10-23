#!/usr/bin/env python3
"""Does the phase of the first strong onset relative to the fitted grid tell half-off tracks apart?
usage: q30-hb-first-onset.py <grid out json> <onsets dir>"""
import json, sys, struct, array

tr = json.load(open(sys.argv[1]))["tracks"]
odir = sys.argv[2]


def load(i):
    a = array.array("f")
    with open(f"{odir}/{i}.onsets", "rb") as f:
        a.frombytes(f.read())
    n = (len(a) - 2) // 3
    return a[0], a[1], [a[2:2 + n], a[2 + n:2 + 2 * n], a[2 + 2 * n:]]


rows = []
for t in tr:
    if abs(t["est_bpm"] - t["ref_bpm"]) > 0.5 or not t["ref_constant"] or not t.get("hb"):
        continue
    P = t["period_ms"] / 1000
    half = abs(abs(t["med_offset_ms"]) - P * 500) < 70
    try:
        hop, centre, bands = load(t["id"])
    except FileNotFoundError:
        continue
    row = {"id": t["id"], "half": half}
    for b in (0, 1, 2):
        env = sorted(bands[b])
        p95 = env[int(len(env) * 0.95)]
        for frac in (0.3, 0.5, 0.8):
            idx = next((i for i, v in enumerate(bands[b]) if v > frac * p95), None)
            if idx is None:
                row[(b, frac)] = None
                continue
            ton = idx * hop + centre
            ph = ((ton - t["fit_first_s"]) / P) % 1.0
            row[(b, frac)] = min(ph, 1 - ph)
    rows.append(row)

print(len(rows), "tracks;", sum(r["half"] for r in rows), "half-off")
for key in sorted(k for k in rows[0] if isinstance(k, tuple)):
    h = sorted(round(r[key], 2) for r in rows if r["half"] and r[key] is not None)
    for thr in (0.25, 0.3, 0.35, 0.4):
        fixes = sum(r[key] is not None and r[key] > thr for r in rows if r["half"])
        breaks = sum(r[key] is not None and r[key] > thr for r in rows if not r["half"])
        print(key, "thr", thr, f"fixes {fixes}/{sum(r['half'] for r in rows)} breaks {breaks}/{sum(not r['half'] for r in rows)}")
    print("   half-off phases:", h)
