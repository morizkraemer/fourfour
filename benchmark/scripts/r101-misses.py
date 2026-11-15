#!/usr/bin/env python3
"""Lists tracks more than 1 BPM off the Rekordbox master.db BPM in ffa JSONL output and classifies them.
usage: r101-misses.py <ref.json> <ffa.jsonl> [<dump dir with raw beats>] [--tol 1.0]
Classes: octave x2 / x0.5, meter 3:2 / 4:3 (ratio within 3%), dynamic reference grid (Rekordbox grid is not constant),
wobble (within 4% of the reference), other. With a dump dir it also prints the model's raw median tempo per third of the track."""
import json
import statistics
import sys

RATIOS = [(2.0, "octave x2"), (0.5, "octave x0.5"), (1.5, "meter 3:2"), (2 / 3, "meter 2:3"), (4 / 3, "meter 4:3"), (0.75, "meter 3:4")]


def constant_grid(beats_ms):
    if len(beats_ms) < 3:
        return False
    p = (beats_ms[-1] - beats_ms[0]) / (len(beats_ms) - 1)
    return all(abs(t - (beats_ms[0] + p * i)) <= 5 for i, t in enumerate(beats_ms))


def classify(bpm, e):
    ref = e["bpm_masterdb"]
    dyn = not constant_grid(e["beats_ms"])
    for k, name in RATIOS:
        if abs(bpm / ref - k) < 0.03 * k:
            return name + (" + dynamic ref" if dyn else "")
    if dyn:
        return "dynamic ref" + (", wobble" if abs(bpm / ref - 1) < 0.04 else "")
    if abs(bpm / ref - 1) < 0.04:
        return "wobble (constant ref)"
    return "other (constant ref)"


def thirds(dump, i):
    try:
        beats = json.load(open(f"{dump}/{i}.json"))["beats"]
    except OSError:
        return ""
    n = len(beats) // 3
    out = []
    for s in range(3):
        seg = beats[s * n:(s + 1) * n]
        ibi = [b - a for a, b in zip(seg, seg[1:]) if 0.25 <= b - a <= 2.0]
        out.append(f"{60 / statistics.median(ibi):.1f}" if ibi else "-")
    return " / ".join(out)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    tol = float(sys.argv[sys.argv.index("--tol") + 1]) if "--tol" in sys.argv else 1.0
    if "--tol" in sys.argv:
        args.remove(sys.argv[sys.argv.index("--tol") + 1])
    ref = json.load(open(args[0]))
    dump = args[2] if len(args) > 2 else None
    counts = {}
    for line in open(args[1]):
        a = json.loads(line)
        if a.get("id") is None or a.get("bpm") is None:
            continue
        e = ref[str(a["id"])]
        if abs(a["bpm"] - e["bpm_masterdb"]) <= tol:
            continue
        cls = classify(a["bpm"], e)
        counts[cls] = counts.get(cls, 0) + 1
        gt = statistics.median(t / 100 for t in e["tempo_x100"])
        print(f"{a['id']:>4} ours {a['bpm']:8.3f} ref {e['bpm_masterdb']:7.2f} rbgrid {gt:7.2f} ratio {a['bpm'] / e['bpm_masterdb']:.3f}  {cls}  thirds {thirds(dump, a['id']) if dump else ''}")
    print(dict(sorted(counts.items(), key=lambda kv: -kv[1])))


main()
