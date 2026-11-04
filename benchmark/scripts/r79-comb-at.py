import json
import math
import struct
import sys

FPS = 50.0


def comb(act, bpm):
    p = 60.0 / bpm
    n = len(act)
    best = (-1.0, 0.0)
    for j in range(int(p / 0.002)):
        s = 0.0
        t = j * 0.002
        while t * FPS + 1 < n:
            x = t * FPS
            i = int(x)
            fr = x - i
            s += act[i] * (1 - fr) + act[i + 1] * fr
            t += p
        if s > best[0]:
            best = (s, j * 0.002)
    return best


dump, ref = sys.argv[1], json.load(open(sys.argv[2]))
for spec in sys.argv[3:]:
    tid, *bpms = spec.split(":")
    doc = json.load(open(f"{dump}/{tid}.json"))
    raw = open(f"{dump}/{tid}.logits", "rb").read()
    frames = doc["frames"]
    logits = struct.unpack(f"<{frames}f", raw[: 4 * frames])
    act = [1 / (1 + math.exp(-x)) for x in logits]
    out = []
    for b in [float(x) for x in bpms] + [ref[tid]["bpm_masterdb"]]:
        s, ph = comb(act, b)
        out.append(f"{b:.3f}: {s:.1f} (phase {ph:.3f})")
    print(tid, " | ".join(out))
