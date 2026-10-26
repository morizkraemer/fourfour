#!/usr/bin/env python3
"""Group per-track median offsets of the mp3 tracks by Xing/Info and LAME-tag flags.

usage: q26-mp3-groups.py <grid --diag --out json> <grid --mp3-diag stdout text>
"""
import collections
import json
import re
import statistics as st
import sys

tracks = {t["id"]: t for t in json.load(open(sys.argv[1]))["tracks"]}
groups = collections.defaultdict(list)
for line in open(sys.argv[2]):
    m = re.match(r"MP3 (\d+) header_frame (\w+) header_counts (\w+) lame_delay (\S+)", line)
    if not m:
        continue
    t = tracks[int(m[1])]
    if abs(t["est_bpm"] - t["ref_bpm"]) > 0.1:
        continue
    groups[(m[2], m[3], m[4])].append((t["med_offset_ms"], t["max_dev_ms"]))
for key, v in sorted(groups.items()):
    offs = [x[0] for x in v]
    q = st.quantiles(offs, n=4) if len(offs) > 3 else [0, 0, 0]
    ok = sum(x[1] <= 20 for x in v)
    print(key, len(v), "median %.1f q1 %.1f q3 %.1f" % (st.median(offs), q[0], q[2]), "within20 %d" % ok, sorted(round(x) for x in offs))
