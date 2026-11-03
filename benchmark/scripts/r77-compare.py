import json
import sys


def load(path):
    rows = {}
    for line in open(path):
        r = json.loads(line)
        rows[r["id"]] = r
    return rows


def ok(r):
    return r["max_dev_ms"] <= 20.0


def bpm_ok(r):
    return abs(r["bpm"] - r["ref_bpm"]) <= 0.1


def summary(rows):
    n = len(rows)
    fit = sorted(r["fit_us"] for r in rows.values())
    return f"n={n} grid20={sum(ok(r) for r in rows.values())} bpm0.1={sum(bpm_ok(r) for r in rows.values())} fit_median_ms={fit[n // 2] / 1000:.1f}"


base = load(sys.argv[1])
print("base", sys.argv[1], summary(base))
for path in sys.argv[2:]:
    new = load(path)
    print("new ", path, summary(new))
    for i in sorted(base):
        if i not in new:
            continue
        a, b = base[i], new[i]
        if ok(a) != ok(b) or bpm_ok(a) != bpm_ok(b) or abs(a["bpm"] - b["bpm"]) > 0.01:
            tag = "GAIN" if ok(b) and not ok(a) else "LOSS" if ok(a) and not ok(b) else "bpm"
            print(f"  {tag} {i} ref {a['ref_bpm']} bpm {a['bpm']:.4f}->{b['bpm']:.4f} max {a['max_dev_ms']:.1f}->{b['max_dev_ms']:.1f} drift {a['drift_ms']:.1f}->{b['drift_ms']:.1f} med {a['med_offset_ms']:.1f}->{b['med_offset_ms']:.1f}")
    moved = sum(1 for i in base if i in new and abs(base[i]["bpm"] - new[i]["bpm"]) > 1e-6)
    print(f"  tracks with any bpm change: {moved}")
