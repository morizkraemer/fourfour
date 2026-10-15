"""Score a fourfour-benchmark run: BPM and key agreement tables, worst failures, runtime.

Reads benchmark/results/<run_id>/raw/<variant>.json and the corpus manifest the run used.
Prints markdown. Never prints file paths.
"""

import argparse
import json
import statistics
from pathlib import Path

BPM_TOL = 0.1


def parse_camelot(key):
    if not key or len(key) < 2 or key[-1] not in "AB" or not key[:-1].isdigit():
        return None
    return int(key[:-1]), key[-1]


def key_class(ours, gt):
    a, b = parse_camelot(ours), parse_camelot(gt)
    if a is None or b is None:
        return "unparsed"
    if a == b:
        return "exact"
    if a[0] == b[0]:
        return "relative"
    if a[1] == b[1] and min((a[0] - b[0]) % 12, (b[0] - a[0]) % 12) == 1:
        return "neighbour"
    return "other"


def pct(n, d):
    return f"{n} / {d} = {100 * n / d:.1f}%" if d else "n/a"


def bpm_metrics(ours, gt):
    octave = {"x1": 1.0, "x2": 2.0, "x0.5": 0.5}
    exact = abs(ours - gt) <= BPM_TOL
    octave_hit = next((n for n, m in octave.items() if abs(ours - gt * m) <= BPM_TOL), None)
    rounded = round(ours) == round(gt)
    rounded_octave = next((n for n, m in octave.items() if round(ours) == round(gt * m)), None)
    within_1pct = abs(ours - gt) / gt <= 0.01
    return exact, octave_hit, rounded, rounded_octave, within_1pct


def main():
    p = argparse.ArgumentParser()
    p.add_argument("run_dir")
    p.add_argument("--variant", default="deeprhythm_essentia")
    p.add_argument("--corpus", required=True)
    p.add_argument("--worst", type=int, default=15)
    p.add_argument("--dedupe", action="store_true", help="count each artist+title+reference BPM+reference key once")
    p.add_argument("--only-ids-from", help="restrict to track ids present in this (original) manifest")
    p.add_argument("--json-out")
    args = p.parse_args()

    corpus = json.loads(Path(args.corpus).read_text())
    entries = {e["id"]: e for e in corpus["entries"]}
    raw_files = sorted((Path(args.run_dir) / "raw").glob(f"{args.variant}*.json"))
    records = [r for f in raw_files for r in json.loads(f.read_text())]

    if args.only_ids_from:
        wanted = {e["id"] for e in json.loads(Path(args.only_ids_from).read_text())["entries"]}
        records = [r for r in records if r["track_id"] in wanted]
        entries = {k: v for k, v in entries.items() if k in wanted}
    ok = [r for r in records if r["status"] == "ok" and r["result"]]
    failed = [r for r in records if r["status"] != "ok"]
    print(f"Tracks in corpus: {len(entries)}; records: {len(records)}; analysed ok: {len(ok)}; backend errors: {len(failed)}\n")

    bpm_rows, key_rows = [], []
    seen = set()
    for r in ok:
        e = entries[r["track_id"]]
        if args.dedupe:
            ident = (e.get("artist"), e.get("title"), (e.get("ground_truth") or {}).get("bpm"), (e.get("ground_truth") or {}).get("key"))
            if ident in seen:
                continue
            seen.add(ident)
        gt = e.get("ground_truth") or {}
        name = f"{e.get('artist') or '?'} - {e.get('title') or '?'}"
        res = r["result"]
        if gt.get("bpm") and res.get("bpm"):
            bpm_rows.append((name, res["bpm"], gt["bpm"]))
        if gt.get("key") and res.get("key"):
            key_rows.append((name, res["key"], gt["key"]))

    summary = {"tracks": len(entries), "ok": len(ok), "errors": len(failed)}

    if bpm_rows:
        n = len(bpm_rows)
        ms = [bpm_metrics(o, g) for _, o, g in bpm_rows]
        no_bpm = sum(1 for r in ok if (entries[r["track_id"]].get("ground_truth") or {}).get("bpm") and not r["result"].get("bpm"))
        print(f"### BPM (denominator: {n} tracks with ground-truth BPM and a detected BPM; {no_bpm} with GT but no detection)\n")
        print("| Metric | Result |\n|---|---:|")
        print(f"| Exact within +-{BPM_TOL} BPM | {pct(sum(m[0] for m in ms), n)} |")
        print(f"| Octave-tolerant (x1, x2, x0.5) within +-{BPM_TOL} | {pct(sum(m[1] is not None for m in ms), n)} |")
        print(f"| Integer-rounded equal | {pct(sum(m[2] for m in ms), n)} |")
        print(f"| Integer-rounded, octave-tolerant | {pct(sum(m[3] is not None for m in ms), n)} |")
        print(f"| Within 1% (extra) | {pct(sum(m[4] for m in ms), n)} |")
        octave_only = {"x2": 0, "x0.5": 0}
        for m in ms:
            if m[3] in octave_only:
                octave_only[m[3]] += 1
        print(f"\nOctave errors (integer-rounded): detected = 2x GT: {octave_only['x2']}, detected = 0.5x GT: {octave_only['x0.5']}\n")
        summary["bpm"] = {"n": n, "exact_0.1": sum(m[0] for m in ms), "octave_0.1": sum(m[1] is not None for m in ms),
                          "rounded": sum(m[2] for m in ms), "rounded_octave": sum(m[3] is not None for m in ms),
                          "within_1pct": sum(m[4] for m in ms)}
        fractional = sum(1 for _, o, _ in bpm_rows if abs(o - round(o)) > 1e-6)
        within_1 = sum(abs(o - g) <= 1.0 for _, o, g in bpm_rows)
        mean_signed = statistics.mean(o - g for _, o, g in bpm_rows)
        print(f"Within 1.0 BPM absolute: {pct(within_1, n)}; mean signed error (detected - reference): {mean_signed:+.2f} BPM\n")
        print(f"Detected BPM values that are non-integer: {fractional} / {n}\n")
        worst = sorted((x for x, m in zip(bpm_rows, ms) if not m[3]), key=lambda x: -abs(x[1] - x[2]) / x[2])[:args.worst]
        print("Worst BPM misses (not even octave-tolerant integer match):\n\n| Track | Detected | Reference | Ratio |\n|---|---:|---:|---:|")
        for name, o, g in worst:
            print(f"| {name} | {o:.2f} | {g:.2f} | {o / g:.3f} |")
        print()

    if key_rows:
        n = len(key_rows)
        classes = [key_class(o, g) for _, o, g in key_rows]
        c = {k: classes.count(k) for k in ("exact", "relative", "neighbour", "other", "unparsed")}
        no_key = sum(1 for r in ok if (entries[r["track_id"]].get("ground_truth") or {}).get("key") and not r["result"].get("key"))
        print(f"### Key (denominator: {n} tracks with ground-truth key and a detected key; {no_key} with GT but no detection)\n")
        print("| Class | Result |\n|---|---:|")
        print(f"| Exact | {pct(c['exact'], n)} |")
        print(f"| Relative major/minor (same Camelot number) | {pct(c['relative'], n)} |")
        print(f"| Camelot neighbour (+-1, same letter) | {pct(c['neighbour'], n)} |")
        print(f"| Exact + relative + neighbour | {pct(c['exact'] + c['relative'] + c['neighbour'], n)} |")
        print(f"| Exact + neighbour (findings-doc metric) | {pct(c['exact'] + c['neighbour'], n)} |")
        print(f"| Other | {pct(c['other'] + c['unparsed'], n)} |\n")
        summary["key"] = {"n": n, **c}
        worst_rows = [x for x, k in zip(key_rows, classes) if k == "other"][:args.worst]
        print("Examples of 'other' key misses:\n\n| Track | Detected | Reference |\n|---|---|---|")
        for name, o, g in worst_rows:
            print(f"| {name} | {o} | {g} |")
        print()

    times = [r["result"]["elapsed_seconds"] for r in ok if r["result"].get("elapsed_seconds")]
    if times:
        qs = statistics.quantiles(times, n=20) if len(times) >= 2 else times
        print(f"### Runtime (analysis seconds per track, n={len(times)})\n")
        print(f"mean {statistics.mean(times):.2f}, median {statistics.median(times):.2f}, p95 {qs[-1]:.2f}, max {max(times):.2f}\n")
        summary["runtime"] = {"n": len(times), "mean": statistics.mean(times), "median": statistics.median(times)}

    if args.json_out:
        Path(args.json_out).write_text(json.dumps(summary, indent=1))


if __name__ == "__main__":
    main()
