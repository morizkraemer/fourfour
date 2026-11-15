import json
import sys


def load(path):
    return {r["id"]: r for r in map(json.loads, open(path))}


def bpm_ok(r):
    return abs(r["bpm"] - r["ref_bpm"]) <= 0.1


def grid_ok(r):
    return r["max_dev_ms"] <= 20.0


a, b = load(sys.argv[1]), load(sys.argv[2])
for i in sorted(a):
    if i not in b:
        print(f"  {i} missing in new")
        continue
    x, y = a[i], b[i]
    moved = abs(x["bpm"] - y["bpm"]) > 1e-6 or abs(x["med_offset_ms"] - y["med_offset_ms"]) > 0.5 or abs(x["max_dev_ms"] - y["max_dev_ms"]) > 0.5
    if moved:
        tags = []
        if bpm_ok(x) != bpm_ok(y):
            tags.append("BPM GAIN" if bpm_ok(y) else "BPM LOSS")
        if grid_ok(x) != grid_ok(y):
            tags.append("GRID GAIN" if grid_ok(y) else "GRID LOSS")
        print(f"  {i} {' '.join(tags) or 'changed'}: bpm {x['bpm']:.4f}->{y['bpm']:.4f} (ref {x['ref_bpm']}) max {x['max_dev_ms']:.1f}->{y['max_dev_ms']:.1f} med {x['med_offset_ms']:.1f}->{y['med_offset_ms']:.1f}")
print(f"bpm0.1 {sum(map(bpm_ok, a.values()))}->{sum(map(bpm_ok, b.values()))}  grid20 {sum(map(grid_ok, a.values()))}->{sum(map(grid_ok, b.values()))}  n {len(a)}/{len(b)}")
