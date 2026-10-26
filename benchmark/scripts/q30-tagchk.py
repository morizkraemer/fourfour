#!/usr/bin/env python3
"""Shows the raw Xing/Info tag bytes for mp3 tracks grouped as 'mp3' (no encoder found) or tagged LAME3.100.
usage: q30-tagchk.py <ref.json> <grid out json>"""
import json, sys

ref = json.load(open(sys.argv[1]))
last = {t["id"]: t for t in json.load(open(sys.argv[2]))["tracks"]}


def tag(path):
    with open(path, "rb") as f:
        head = f.read(10)
        start = 0
        if head[:3] == b"ID3":
            s = (head[6] & 0x7F) << 21 | (head[7] & 0x7F) << 14 | (head[8] & 0x7F) << 7 | (head[9] & 0x7F)
            start = 10 + s
        f.seek(start)
        d = f.read(4096)
    out = []
    for key in (b"Xing", b"Info"):
        i = d.find(key)
        if 0 <= i < 200:
            out.append((key, d[i + 100 : i + 140]))
    return out


for i, t in last.items():
    p = ref[str(i)]["path"]
    if t["container"] == "mp3" or (p.lower().endswith(".mp3") and "LAME3.100" in str(tag(p))):
        print(i, t["container"], round(t["med_offset_ms"], 1), tag(p))
