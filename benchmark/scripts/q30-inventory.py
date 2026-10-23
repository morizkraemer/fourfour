#!/usr/bin/env python3
"""Lists dev tracks (manifest id % 3 != 0) with audio on this box, ordered for analysis: mp3 with LAME/Lavf-style tag first.
usage: q30-inventory.py <ref.json> <out list.txt> [--have DIR]   (skips ids that already have DIR/<id>.json)"""
import json, os, sys, collections

ref = json.load(open(sys.argv[1]))
out = sys.argv[2]
have = set()
if "--have" in sys.argv:
    d = sys.argv[sys.argv.index("--have") + 1]
    have = {f[:-5] for f in os.listdir(d) if f.endswith(".json")}


def tag(path):
    if not path.lower().endswith(".mp3"):
        return None
    with open(path, "rb") as f:
        head = f.read(10)
        start = 0
        if head[:3] == b"ID3":
            s = (head[6] & 0x7F) << 21 | (head[7] & 0x7F) << 14 | (head[8] & 0x7F) << 7 | (head[9] & 0x7F)
            start = 10 + s
        f.seek(start)
        d = f.read(4096)
    for key in (b"Xing", b"Info"):
        i = d.find(key)
        if 0 <= i < 200:
            j = d.find(b"L", i)
            enc = d[i + 100 : i + 400]
            for m in (b"LAME", b"Lame", b"Lavf", b"Lavc"):
                k = enc.find(m)
                if k >= 0:
                    return enc[k : k + 9].decode("latin1")
            return key.decode() + "-notag"
    return "notag"


rows = []
for i, v in ref.items():
    i = int(i)
    p = v["path"]
    if i % 3 == 0 or not p or not os.path.exists(p):
        continue
    ext = os.path.splitext(p)[1].lower()
    t = tag(p)
    prio = 0 if (t and t[:4] in ("LAME", "Lame", "Lavf", "Lavc")) else 1 if ext == ".mp3" else 2
    rows.append((prio, i, p, ext, t))
rows.sort()
print(collections.Counter((r[3], r[4]) for r in rows).most_common(40))
todo = [r for r in rows if str(r[1]) not in have]
open(out, "w").write("".join(r[2] + "\n" for r in todo))
print(len(rows), "dev with audio;", len(todo), "to analyse")
