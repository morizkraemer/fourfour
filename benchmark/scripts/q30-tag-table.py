#!/usr/bin/env python3
"""Tag x offset table from `grid --out` files. usage: q30-tag-table.py <grid out json> [more json...]
Only tracks with BPM within 0.5 of Rekordbox and a constant reference grid are used for the offset columns."""
import json, sys, statistics, collections

groups = collections.defaultdict(list)
for p in sys.argv[1:]:
    for t in json.load(open(p))["tracks"]:
        groups[t["container"]].append(t)

print(f"{'container:encoder':<20}{'n':>4}{'bpm ok':>8}{'med offset':>12}{'min':>8}{'max':>8}{'within 20ms':>13}")
for k in sorted(groups, key=lambda k: (k.split(":")[0], -len(groups[k]))):
    ts = groups[k]
    ok = [t for t in ts if abs(t["est_bpm"] - t["ref_bpm"]) <= 0.5 and t["ref_constant"]]
    offs = [t["med_offset_ms"] for t in ok]
    g20 = sum(t["max_dev_ms"] <= 20 for t in ts)
    if offs:
        print(f"{k:<20}{len(ts):>4}{len(ok):>8}{statistics.median(offs):>12.1f}{min(offs):>8.1f}{max(offs):>8.1f}{g20:>9}/{len(ts)}")
    else:
        print(f"{k:<20}{len(ts):>4}{0:>8}{'-':>12}{'-':>8}{'-':>8}{g20:>9}/{len(ts)}")
