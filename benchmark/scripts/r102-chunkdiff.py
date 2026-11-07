#!/usr/bin/env python3
"""How far the beat logits of a chunked run are from a reference run (ffa --dump dirs), by distance to the nearest chunk border.
usage: r102-chunkdiff.py <ref dump> <run dump[,run dump...]> <ids file | id> <stride> [--peaks]
<stride> is the frame distance between valid regions of the run (chunk - 2 * border - overlap). Prints, over all tracks, the mean absolute
beat-logit difference in bins of distance to the nearest border, and (with --peaks) the beat peaks (logit > 0, local maximum over 7 frames)
that only one side has."""
import json
import os
import struct
import sys

BINS = [(0, 6), (6, 25), (25, 60), (60, 120), (120, 10**9)]


def load(d, i):
    doc = json.load(open(f"{d}/{i}.json"))
    raw = open(f"{d}/{i}.logits", "rb").read()
    n = doc["frames"]
    return struct.unpack(f"<{n}f", raw[: 4 * n])


def peaks(x):
    return [i for i, v in enumerate(x) if v > 0 and all(v >= w for w in x[max(0, i - 3):i + 4])]


def main():
    ref, runs, ids_arg, stride = sys.argv[1], sys.argv[2].split(","), sys.argv[3], int(sys.argv[4])
    ids = open(ids_arg).read().split() if os.path.exists(ids_arg) else [ids_arg]
    sums = [0.0] * len(BINS)
    counts = [0] * len(BINS)
    total_abs = 0.0
    total_n = 0
    only_ref = only_run = peaks_ref = peaks_run = 0
    used = 0
    for i in ids:
        run = next((d for d in runs if os.path.exists(f"{d}/{i}.json")), None)
        if run is None or not os.path.exists(f"{ref}/{i}.json"):
            continue
        used += 1
        a, b = load(ref, i), load(run, i)
        n = min(len(a), len(b))
        for k in range(n):
            d = abs(a[k] - b[k])
            pos = k % stride
            edge = min(pos, stride - 1 - pos)
            for j, (lo, hi) in enumerate(BINS):
                if lo <= edge < hi:
                    sums[j] += d
                    counts[j] += 1
                    break
            total_abs += d
            total_n += 1
        if "--peaks" in sys.argv:
            pa, pb = peaks(a[:n]), peaks(b[:n])
            sa, sb = set(pa), set(pb)
            only_ref += sum(1 for p in pa if not any(abs(p - q) <= 2 for q in sb))
            only_run += sum(1 for p in pb if not any(abs(p - q) <= 2 for q in sa))
            peaks_ref += len(pa)
            peaks_run += len(pb)
    print(f"tracks {used}, frames {total_n}, mean |diff| {total_abs / max(1, total_n):.4f}")
    print("distance to chunk border (frames): " + "  ".join(f"{lo}-{hi if hi < 10**9 else 'inf'}: {sums[j] / max(1, counts[j]):.4f}" for j, (lo, hi) in enumerate(BINS)))
    if "--peaks" in sys.argv:
        print(f"peaks ref {peaks_ref} run {peaks_run}; only ref {only_ref}, only run {only_run}")


main()
