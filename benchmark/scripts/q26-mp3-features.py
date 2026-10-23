#!/usr/bin/env python3
"""Per-track mp3 header features next to the grid offset, to look for what splits the offsets.

usage: q26-mp3-features.py <grid --diag --out json> <reference json>
"""
import json
import struct
import sys

BR = {
    (3, 1): [0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448],
    (3, 2): [0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384],
    (3, 3): [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320],
}
SR = {3: [44100, 48000, 32000], 2: [22050, 24000, 16000], 0: [11025, 12000, 8000]}


def frame(b, i):
    if i + 4 > len(b) or b[i] != 0xFF or (b[i + 1] & 0xE0) != 0xE0:
        return None
    ver = (b[i + 1] >> 3) & 3
    lay = (b[i + 1] >> 1) & 3
    bri = b[i + 2] >> 4
    sri = (b[i + 2] >> 2) & 3
    pad = (b[i + 2] >> 1) & 1
    mode = b[i + 3] >> 6
    if ver == 1 or lay != 1 or bri in (0, 15) or sri == 3:
        return None
    layer = 4 - lay
    v = 3 if ver == 3 else 2
    br = BR[(3, layer)][bri] if ver == 3 else None
    if br is None:
        return None
    sr = SR[ver][sri]
    size = 144000 * br // sr + pad
    return dict(br=br, sr=sr, mode=mode, size=size, ver=ver, crc=not (b[i + 1] & 1))


tracks = {t["id"]: t for t in json.load(open(sys.argv[1]))["tracks"]}
ref = json.load(open(sys.argv[2]))
rows = []
for id_, t in tracks.items():
    path = ref[str(id_)]["path"]
    if not path.lower().endswith(".mp3") or abs(t["est_bpm"] - t["ref_bpm"]) > 0.1:
        continue
    b = open(path, "rb").read(200000)
    id3 = 0
    if b[:3] == b"ID3":
        id3 = 10 + ((b[6] << 21) | (b[7] << 14) | (b[8] << 7) | b[9])
        b = open(path, "rb").read(id3 + 20000)[id3:]
    i = 0
    while i < len(b) - 4 and frame(b, i) is None:
        i += 1
    f = frame(b, i)
    if f is None:
        continue
    tag = b[i : i + 400]
    kind = "Info" if b"Info" in tag else ("Xing" if b"Xing" in tag else "none")
    import re
    mm = re.search(rb"(Lame|Lavc|Lavf|LAME)[0-9.]*", b[i : i + 400])
    enc = mm.group(0)[:9] if mm else b"?"
    n = 0
    j = i
    while n < 6 and frame(b, j):
        j += frame(b, j)["size"]
        n += 1
    rows.append((round(t["med_offset_ms"]), f["br"], f["sr"], f["mode"], kind, enc.decode(), id3, f["crc"], n, id_, round(t["max_dev_ms"])))
rows.sort()
print("off br sr mode tag enc id3size crc frames_ok id maxdev")
for r in rows:
    print(*r)
