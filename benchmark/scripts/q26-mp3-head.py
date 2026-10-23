#!/usr/bin/env python3
"""Dump the first frame header and Info/Xing payload start of mp3 tracks by id. usage: q26-mp3-head.py <reference json> <id>..."""
import json
import sys

ref = json.load(open(sys.argv[1]))
for id_ in sys.argv[2:]:
    path = ref[id_]["path"]
    raw = open(path, "rb").read(400000)
    id3 = 10 + ((raw[6] << 21) | (raw[7] << 14) | (raw[8] << 7) | raw[9]) if raw[:3] == b"ID3" else 0
    b = raw[id3:]
    gap = 0
    while gap < len(b) - 4 and not (b[gap] == 0xFF and (b[gap + 1] & 0xE0) == 0xE0):
        gap += 1
    h = b[gap : gap + 4]
    k = b.find(b"Info", gap, gap + 60)
    print(id_, "id3", id3, "gap", gap, "hdr", h.hex(), "info@", k - gap if k >= 0 else None, "payload", b[k : k + 24].hex() if k >= 0 else "-", b[k + 120 : k + 156] if k >= 0 else b"")
