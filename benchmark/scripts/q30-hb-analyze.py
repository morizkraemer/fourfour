#!/usr/bin/env python3
"""Half-beat analysis from a `grid --onset-dir ... --out` file. usage: q30-hb-analyze.py <grid out json>
A track is 'half-off' when its median offset is within 70 ms of +-P/2 (the grid sits on the other half-beat).
Prints, per band, the off/on energy ratio of half-off tracks against all other tracks with BPM ok, and the best threshold."""
import json, sys

tr = json.load(open(sys.argv[1]))["tracks"]
ok = [t for t in tr if abs(t["est_bpm"] - t["ref_bpm"]) <= 0.5 and t["ref_constant"] and t.get("hb")]
for t in ok:
    h = t["period_ms"] / 2
    t["half"] = abs(abs(t["med_offset_ms"]) - h) < 70
half = [t for t in ok if t["half"]]
rest = [t for t in ok if not t["half"]]
print(f"bpm ok & constant: {len(ok)}, half-off: {len(half)}")
names = ["low", "mid", "high", "beatact", "downact", "lownarrow"]
for b in range(6):
    print(f"-- band {names[b]}")
    print("   half-off ratios:", sorted(round(t["hb"][b], 2) for t in half))
    r = sorted(t["hb"][b] for t in rest)
    print("   others: median %.2f p90 %.2f p95 %.2f max %.2f" % (r[len(r) // 2], r[int(len(r) * .9)], r[int(len(r) * .95)], r[-1]))
    for thr in (0.8, 1.0, 1.2, 1.5, 2.0, 3.0):
        tp = sum(t["hb"][b] > thr for t in half)
        fp = sum(t["hb"][b] > thr for t in rest)
        print(f"   thr {thr}: fixes {tp}/{len(half)}, breaks {fp}/{len(rest)}")
for t in sorted(half, key=lambda t: t["id"]):
    print(t["id"], round(t["med_offset_ms"]), round(t["period_ms"]), [round(x, 2) for x in t["hb"]], t["container"])
