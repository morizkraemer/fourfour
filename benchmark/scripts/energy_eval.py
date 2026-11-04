#!/usr/bin/env python3
"""Fit and evaluate the track energy model (r100). Pure Python, no numpy.

usage:
  energy_eval.py fit <beatport.json> <beatport_dump.jsonl> [--write model.json]
  energy_eval.py rust <model.json>
  energy_eval.py bpm <beatport.json> <beatport_dump.jsonl> <rekordbox-results.json>
  energy_eval.py check <beatport.json> <beatport_dump.jsonl> <model.json>
  energy_eval.py structure <dump.jsonl>
  energy_eval.py stability <ref_dump.jsonl> <variant_dump.jsonl>...

Dumps come from `cargo run --release -p fourfour-analysis-rs --example energy_dump`. The "label" of the
Beatport tracks is an a priori genre energy table (energy_prep.py), not a human energy rating: no
energy labels exist in the benchmark data.
"""
import json
import math
import random
import sys
from collections import defaultdict

NAMES = ["level_db", "dynamics_db", "flux", "low_db", "high_db", "centroid_log2", "flatness_db", "onset_rate"]
FUSION_WEIGHTS = [0.20, 0.0, 0.30, 0.0, 0.0, 0.15, 0.0, 0.10]
TOP_SHARE = 0.5
PAIR_GAP = 4


def load_dump(path):
    out = {}
    for line in open(path):
        doc = json.loads(line)
        if doc.get("energy"):
            out[doc["path"]] = doc
    return out


def ranks(values):
    order = sorted(range(len(values)), key=lambda i: values[i])
    out = [0.0] * len(values)
    i = 0
    while i < len(order):
        j = i
        while j + 1 < len(order) and values[order[j + 1]] == values[order[i]]:
            j += 1
        for k in range(i, j + 1):
            out[order[k]] = (i + j) / 2 + 1
        i = j + 1
    return out


def pearson(a, b):
    n = len(a)
    ma, mb = sum(a) / n, sum(b) / n
    cov = sum((x - ma) * (y - mb) for x, y in zip(a, b))
    va = sum((x - ma) ** 2 for x in a)
    vb = sum((y - mb) ** 2 for y in b)
    return cov / math.sqrt(va * vb) if va > 0 and vb > 0 else 0.0


def spearman(a, b):
    return pearson(ranks(a), ranks(b))


def pair_auc(scores, labels, gap=PAIR_GAP):
    wins = total = 0.0
    for i in range(len(scores)):
        for j in range(len(scores)):
            if labels[i] - labels[j] >= gap:
                total += 1
                wins += 1.0 if scores[i] > scores[j] else 0.5 if scores[i] == scores[j] else 0.0
    return wins / total if total else float("nan")


def bootstrap_ci(scores, labels, stat, n=500, seed=1):
    rng = random.Random(seed)
    idx = list(range(len(scores)))
    vals = []
    for _ in range(n):
        pick = [rng.choice(idx) for _ in idx]
        vals.append(stat([scores[i] for i in pick], [labels[i] for i in pick]))
    vals.sort()
    return vals[int(0.025 * n)], vals[int(0.975 * n)]


def window_stats(rows):
    mean, std = [], []
    for k in range(len(NAMES)):
        v = [w[k] for r in rows for w in r if w[k] is not None]
        if not v:
            mean.append(0.0)
            std.append(1.0)
            continue
        m = sum(v) / len(v)
        mean.append(m)
        std.append(max(math.sqrt(sum((x - m) ** 2 for x in v) / len(v)), 1e-9))
    return mean, std


def zrow(w, mean, std):
    return [0.0 if w[k] is None else (w[k] - mean[k]) / std[k] for k in range(len(NAMES))]


def track_score(windows, model, top=TOP_SHARE):
    scores = []
    for w in windows:
        z = zrow(w, model["mean"], model["std"])
        scores.append(sum(model["weight"][k] * z[k] for k in range(len(NAMES))))
    scores.sort(reverse=True)
    keep = max(1, math.ceil(len(scores) * top))
    return sum(scores[:keep]) / keep


def solve(a, b):
    n = len(b)
    m = [row[:] + [b[i]] for i, row in enumerate(a)]
    for c in range(n):
        p = max(range(c, n), key=lambda r: abs(m[r][c]))
        m[c], m[p] = m[p], m[c]
        for r in range(n):
            if r != c:
                f = m[r][c] / m[c][c]
                m[r] = [x - f * y for x, y in zip(m[r], m[c])]
    return [m[i][n] / m[i][i] for i in range(n)]


def nonneg_ridge(xs, ys, lam, active, sweeps=300):
    cols = [k for k in range(len(NAMES)) if active[k]]
    n = len(xs)
    mx = [sum(x[k] for x in xs) / n for k in range(len(NAMES))]
    my = sum(ys) / n
    xc = [[x[k] - mx[k] for k in range(len(NAMES))] for x in xs]
    yc = [y - my for y in ys]
    w = [0.0] * len(NAMES)
    resid = yc[:]
    for _ in range(sweeps):
        for k in cols:
            num = sum(xc[i][k] * (resid[i] + w[k] * xc[i][k]) for i in range(n))
            den = sum(xc[i][k] ** 2 for i in range(n)) + lam
            new = max(0.0, num / den)
            if new != w[k]:
                d = new - w[k]
                for i in range(n):
                    resid[i] -= d * xc[i][k]
                w[k] = new
    return w


def ridge(xs, ys, lam, active, nonneg=False):
    if nonneg:
        return nonneg_ridge(xs, ys, lam, active)
    cols = [k for k in range(len(NAMES)) if active[k]]
    n = len(xs)
    mx = [sum(x[k] for x in xs) / n for k in cols]
    my = sum(ys) / n
    a = [[sum((x[cols[i]] - mx[i]) * (x[cols[j]] - mx[j]) for x in xs) + (lam if i == j else 0.0) for j in range(len(cols))] for i in range(len(cols))]
    b = [sum((x[cols[i]] - mx[i]) * (y - my) for x, y in zip(xs, ys)) for i in range(len(cols))]
    w = solve(a, b)
    out = [0.0] * len(NAMES)
    for i, k in enumerate(cols):
        out[k] = w[i]
    return out


def quantile_cuts(scores):
    s = sorted(scores)
    return [s[min(len(s) - 1, int(len(s) * q / 10))] for q in range(1, 10)]


def rating_of(score, cuts):
    return 1 + sum(score >= c for c in cuts)


def collect(labels_path, dump_path):
    labels = json.load(open(labels_path))
    dump = load_dump(dump_path)
    rows = []
    for item in labels:
        doc = dump.get(item["path"])
        if doc:
            rows.append({**item, "windows": doc["energy"]["windows"], "rust_rating": doc["energy"]["rating"], "rust_score": doc["energy"]["score"], "bpm": doc["bpm"]})
    return rows


def fit_model(dev, active, nonneg=False, lam_grid=(0.1, 1, 10, 100, 1000)):
    mean, std = window_stats([t["windows"] for t in dev])
    base = {"mean": mean, "std": std}
    xs = []
    for t in dev:
        zs = [zrow(w, mean, std) for w in t["windows"]]
        xs.append([sum(z[k] for z in zs) / len(zs) for k in range(len(NAMES))])
    ys = [t["prior"] for t in dev]
    folds = [[i for i in range(len(dev)) if i % 5 == f] for f in range(5)]
    best = None
    for lam in lam_grid:
        err = 0.0
        for fold in folds:
            held = set(fold)
            tr = [i for i in range(len(dev)) if i not in held]
            w = ridge([xs[i] for i in tr], [ys[i] for i in tr], lam, active, nonneg)
            my = sum(ys[i] for i in tr) / len(tr)
            mx = [sum(xs[i][k] for i in tr) / len(tr) for k in range(len(NAMES))]
            err += sum((my + sum(w[k] * (xs[i][k] - mx[k]) for k in range(len(NAMES))) - ys[i]) ** 2 for i in fold)
        if best is None or err < best[0]:
            best = (err, lam)
    weight = ridge(xs, ys, best[1], active, nonneg)
    model = {**base, "weight": weight, "lambda": best[1]}
    model["cuts"] = quantile_cuts([track_score(t["windows"], model) for t in dev])
    return model


def report(name, model, rows, top=TOP_SHARE):
    scores = [track_score(t["windows"], model, top) for t in rows]
    prior = [t["prior"] for t in rows]
    ratings = [rating_of(s, model["cuts"]) for s in scores]
    rho = spearman(scores, prior)
    lo, hi = bootstrap_ci(scores, prior, spearman)
    auc = pair_auc(scores, prior)
    alo, ahi = bootstrap_ci(scores, prior, pair_auc, n=200)
    print(f"{name:34s} n={len(rows):3d} spearman {rho:.3f} [{lo:.3f},{hi:.3f}]  pair-AUC(gap>={PAIR_GAP}) {auc:.3f} [{alo:.3f},{ahi:.3f}]  rating-spearman {spearman(ratings, prior):.3f}")
    return scores, ratings


def genre_table(rows, ratings):
    by = defaultdict(list)
    for t, r in zip(rows, ratings):
        by[(t["prior"], t["genre"])].append(r)
    for (prior, genre), v in sorted(by.items()):
        print(f"   prior {prior}  {genre:32s} n={len(v):3d}  mean rating {sum(v)/len(v):.2f}  median {sorted(v)[len(v)//2]}")


def cmd_fit(args):
    rows = collect(args[0], args[1])
    dev = [t for t in rows if t["split"] == "dev"]
    test = [t for t in rows if t["split"] == "test"]
    print(f"beatport tracks analysed: {len(rows)}  dev {len(dev)}  held-out test {len(test)}")
    has = [any(w[k] is not None for t in dev for w in t["windows"]) for k in range(len(NAMES))]
    original = [has[k] and NAMES[k] not in ("dynamics_db", "flatness_db") for k in range(len(NAMES))]
    fusion = {**dict(zip(["mean", "std"], window_stats([t["windows"] for t in dev]))), "weight": FUSION_WEIGHTS}
    fusion["cuts"] = quantile_cuts([track_score(t["windows"], fusion) for t in dev])
    variants = [
        ("ridge, unconstrained", has, False),
        ("ridge, non-negative weights", has, True),
        ("non-negative, without dynamics/flatness", original, True),
    ]
    print("features with data:", [n for n, h in zip(NAMES, has) if h])
    print("\n== held-out test (weights and cuts fit on dev only) ==")
    report("a priori fusion weights", fusion, test)
    results = {}
    for name, active, nonneg in variants:
        model = fit_model(dev, active, nonneg)
        results[name] = model
        report(name, model, test)
    print("\n== dev (training) for reference ==")
    report("a priori fusion weights", fusion, dev)
    for name, model in results.items():
        report(name, model, dev)
    full = results["ridge, non-negative weights"]
    print("\n== aggregation over windows (non-negative model, held-out) ==")
    for top in (1.0, 0.5, 0.25):
        report(f"top {int(top*100)}% of windows", full, test, top)
    print("\nfitted weights per standardised window feature (prior units):")
    for n, m, s, w in zip(NAMES, full["mean"], full["std"], full["weight"]):
        print(f"   {n:14s} mean {m:9.4f} std {s:8.4f} weight {w:8.4f}")
    print("lambda", full["lambda"], "cuts", [round(c, 4) for c in full["cuts"]])
    scores, ratings = report("held-out, shipped model", full, test)
    genre_table(test, ratings)
    print("held-out rating histogram:", [ratings.count(r) for r in range(1, 11)])
    if len(args) > 3 and args[2] == "--write":
        json.dump(full, open(args[3], "w"), indent=1)
        print("model written to", args[3])


def cmd_bpm(args):
    rows = collect(args[0], args[1])
    tracks = json.load(open(args[2]))["tracks"]
    bpm_of = {t["path"].rsplit("/", 1)[-1]: t["bpm"] for t in tracks if t.get("bpm")}
    kept = []
    for t in rows:
        bpm = bpm_of.get(t["path"].rsplit("/", 1)[-1])
        if bpm:
            kept.append({**t, "windows": [w + [bpm] for w in t["windows"]]})
    NAMES.append("bpm_rekordbox")
    dev = [t for t in kept if t["split"] == "dev"]
    test = [t for t in kept if t["split"] == "test"]
    print(f"tracks with a Rekordbox BPM: {len(kept)}  dev {len(dev)}  test {len(test)}")
    base = [k < len(NAMES) - 1 for k in range(len(NAMES))]
    for name, active in [("energy features only", base), ("energy features + BPM", [True] * len(NAMES))]:
        model = fit_model(dev, active, True)
        report(name, model, test)
        print("   weights", {n: round(w, 3) for n, w in zip(NAMES, model["weight"]) if w})


def cmd_rust(args):
    model = json.load(open(args[0]))
    row = lambda v: "[" + ", ".join(repr(x) for x in v) + "]"
    print("const MODEL: Model = Model {")
    for key in ("mean", "std", "weight", "cuts"):
        print(f"    {key}: {row(model[key])},")
    print("};")


def cmd_check(args):
    rows = collect(args[0], args[1])
    model = json.load(open(args[2]))
    mismatch = 0
    worst = 0.0
    for t in rows:
        s = track_score(t["windows"], model)
        worst = max(worst, abs(s - t["rust_score"]))
        mismatch += rating_of(s, model["cuts"]) != t["rust_rating"]
    print(f"rust vs python on {len(rows)} tracks: rating mismatches {mismatch}, max |score diff| {worst:.2e}")
    test = [t for t in rows if t["split"] == "test"]
    ratings = [t["rust_rating"] for t in test]
    rho = spearman(ratings, [t["prior"] for t in test])
    print(f"held-out rust rating spearman vs genre prior: {rho:.3f}; rating histogram {[ratings.count(r) for r in range(1, 11)]}")


def cmd_structure(args):
    dump = load_dump(args[0])
    inside = edges = 0
    n = 0
    ratings = []
    for doc in dump.values():
        e = doc["energy"]
        ratings.append(e["rating"])
        curve = e["curve"]
        if doc["duration_s"] < 180 or len(curve) < 12:
            continue
        n += 1
        k = len(curve)
        mid = curve[k // 4 : 3 * k // 4]
        head, tail = curve[: max(1, k // 10)], curve[-max(1, k // 10) :]
        mm = sum(mid) / len(mid)
        inside += mm > sum(head) / len(head) and mm > sum(tail) / len(tail)
        edges += 1
    print(f"tracks {len(dump)}; rating histogram {[ratings.count(r) for r in range(1, 11)]}")
    print(f"full-length tracks (>=3 min): {n}; middle half rated above both the first and last tenth: {inside} ({100*inside/max(n,1):.0f}%)")


def cmd_stability(args):
    ref = load_dump(args[0])
    for variant_path in args[1:]:
        variant = load_dump(variant_path)
        pairs = []
        for path, doc in ref.items():
            stem = path.rsplit("/", 1)[-1].rsplit(".", 1)[0]
            match = next((v for p, v in variant.items() if p.rsplit("/", 1)[-1].rsplit(".", 1)[0] == stem), None)
            if match:
                pairs.append((doc["energy"]["rating"], match["energy"]["rating"], doc["energy"]["score"], match["energy"]["score"]))
        n = len(pairs)
        exact = sum(a == b for a, b, _, _ in pairs)
        within1 = sum(abs(a - b) <= 1 for a, b, _, _ in pairs)
        mad = sum(abs(a - b) for a, b, _, _ in pairs) / n
        rho = spearman([p[2] for p in pairs], [p[3] for p in pairs])
        print(f"{variant_path.rsplit('/', 1)[-1]:28s} n={n}  exact {exact}/{n}  within1 {within1}/{n}  mean |diff| {mad:.2f}  score-spearman {rho:.3f}")


def main():
    cmd, args = sys.argv[1], sys.argv[2:]
    {"fit": cmd_fit, "bpm": cmd_bpm, "rust": cmd_rust, "check": cmd_check, "structure": cmd_structure, "stability": cmd_stability}[cmd](args)


main()
