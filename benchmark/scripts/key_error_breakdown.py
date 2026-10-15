"""Break key errors down by musical relationship between detected and reference key.

Usage: key_error_breakdown.py <corpus.json> <run_dir> [variant]
"""

import collections
import json
import sys
from pathlib import Path


def tonic_and_mode(camelot):
    number, letter = int(camelot[:-1]), camelot[-1]
    if letter == "B":
        return ((number - 8) * 7) % 12, "major"
    return ((number - 5) * 7) % 12, "minor"


def relationship(ours, ref):
    if ours == ref:
        return "exact"
    (a, am), (b, bm) = tonic_and_mode(ours), tonic_and_mode(ref)
    if a == b:
        return "parallel (same tonic, other mode)"
    if am != bm:
        major, minor = (a, b) if am == "major" else (b, a)
        return "relative major/minor" if (major - minor) % 12 == 3 else "other mode swap"
    step = (a - b) % 12
    if step in (5, 7):
        return "fifth neighbour (same mode)"
    if step in (2, 10):
        return "whole tone (same mode)"
    if step == 6:
        return "tritone (same mode)"
    return "other same mode"


def main():
    corpus = json.loads(Path(sys.argv[1]).read_text())
    entries = {e["id"]: e for e in corpus["entries"]}
    variant = sys.argv[3] if len(sys.argv) > 3 else "deeprhythm_essentia"
    records = [r for f in sorted((Path(sys.argv[2]) / "raw").glob(f"{variant}*.json")) for r in json.loads(f.read_text())]
    counts = collections.Counter()
    for r in records:
        if r["status"] != "ok":
            continue
        ref = (entries[r["track_id"]].get("ground_truth") or {}).get("key")
        ours = r["result"].get("key")
        if ref and ours:
            counts[relationship(ours, ref)] += 1
    total = sum(counts.values())
    print(f"denominator {total}")
    for name, n in counts.most_common():
        print(f"| {name} | {n} / {total} = {100 * n / total:.1f}% |")


if __name__ == "__main__":
    main()
