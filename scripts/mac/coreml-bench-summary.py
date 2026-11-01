#!/usr/bin/env python3
"""Summary for scripts/mac/coreml-bench.sh: throughput, first result, peak memory, per-chunk ms, logits and grid vs rten.
usage: coreml-bench-summary.py <out dir> <ref.json> <o3-score.py>"""
import glob
import json
import math
import os
import re
import statistics
import struct
import subprocess
import sys

STRIDE = 1500 - 2 * 6


def peak_rss_mb(err):
    m = re.search(r"(\d+)\s+maximum resident set size", err)
    if m:
        return int(m.group(1)) / 2**20
    m = re.search(r"Maximum resident set size \(kbytes\): (\d+)", err)
    return int(m.group(1)) / 1024 if m else None


def coreml_nodes(err):
    m = re.findall(r"number of nodes supported by CoreML: (\d+)", err)
    total = re.findall(r"number of nodes in the graph: (\d+)", err)
    return f"{m[0]} of {total[0]}" if m and total else "-"


def runs(out):
    rows = []
    for err_path in sorted(glob.glob(os.path.join(out, "*-w*-t*-r*.err"))):
        name = os.path.basename(err_path)[:-4]
        err = open(err_path, errors="replace").read()
        s = re.search(r"SUMMARY tracks (\d+) wall ([\d.]+) s = ([\d.]+) tracks/min; model load (\d+) ms; first track done at (\d+) ms", err)
        jsonl = os.path.join(out, name + ".jsonl")
        model_ms = []
        if os.path.exists(jsonl):
            for line in open(jsonl):
                a = json.loads(line)
                model_ms.append(a["ms"].get("model", 0.0))
        rows.append({
            "name": name,
            "runtime": name.split("-")[0],
            "tracks": int(s.group(1)) if s else 0,
            "wall_s": float(s.group(2)) if s else None,
            "tpm": float(s.group(3)) if s else None,
            "load_ms": int(s.group(4)) if s else None,
            "first_ms": int(s.group(5)) if s else None,
            "rss_mb": peak_rss_mb(err),
            "coreml": coreml_nodes(err),
            "model_s": sum(model_ms) / 1000,
        })
    return rows


def per_chunk_ms(dump):
    vals = []
    for p in glob.glob(os.path.join(dump, "*.json")):
        d = json.load(open(p))
        chunks = max(1, math.ceil(d["frames"] / STRIDE))
        vals.append(d["ms"]["model"] / chunks)
    return statistics.median(vals) if vals else None


def logits(path):
    raw = open(path, "rb").read()
    return struct.unpack(f"<{len(raw) // 4}f", raw)


def compare(a_dir, b_dir):
    diffs, peaks_equal, outputs_equal, n = [], 0, 0, 0
    for a_json in glob.glob(os.path.join(a_dir, "*.json")):
        base = os.path.basename(a_json)[:-5]
        b_json = os.path.join(b_dir, base + ".json")
        if not os.path.exists(b_json):
            continue
        n += 1
        la, lb = logits(os.path.join(a_dir, base + ".logits")), logits(os.path.join(b_dir, base + ".logits"))
        diffs.append(max(abs(x - y) for x, y in zip(la, lb)) if len(la) == len(lb) else float("inf"))
        a, b = json.load(open(a_json)), json.load(open(b_json))
        peaks_equal += a["beats"] == b["beats"] and a["downbeats"] == b["downbeats"]
        fa, fb = a["final_beats_s"], b["final_beats_s"]
        outputs_equal += len(fa) == len(fb) and all(abs(x - y) < 1e-3 for x, y in zip(fa, fb)) and abs(a["bpm"] - b["bpm"]) < 0.01
    return n, diffs, peaks_equal, outputs_equal


def main():
    out, ref, score = sys.argv[1], sys.argv[2], sys.argv[3]
    print(f"# R11 CoreML bench `{os.path.basename(out)}`\n")
    print("| Run | Tracks | Tracks/min | First result | Model load | Peak RSS | Model s (sum) | CoreML nodes |")
    print("|---|---:|---:|---:|---:|---:|---:|---|")
    rows = runs(out)
    for r in rows:
        fmt = lambda v, f: "-" if v is None else f.format(v)
        print(f"| {r['name']} | {r['tracks']} | {fmt(r['tpm'], '{:.2f}')} | {fmt(r['first_ms'] and r['first_ms'] / 1000, '{:.1f} s')} | "
              f"{fmt(r['load_ms'], '{} ms')} | {fmt(r['rss_mb'], '{:.0f} MB')} | {r['model_s']:.0f} | {r['coreml']} |")

    chunk = os.path.join(out, "chunk.txt")
    line = next((l for l in open(chunk) if l.startswith("CHUNK")), None) if os.path.exists(chunk) else None
    print(f"\nPer chunk, one thread (chunk test): {line.strip() if line else 'missing, see chunk.err'}\n")

    dumps = sorted(glob.glob(os.path.join(out, "dump-*")))
    for d in dumps:
        print(f"- {os.path.basename(d)}: median model ms per chunk in the run {per_chunk_ms(d) or 0:.0f}")
    rten = next((d for d in dumps if "dump-rten" in d), None)
    for d in dumps:
        if rten is None or d == rten:
            continue
        n, diffs, peaks, outs = compare(rten, d)
        if n:
            print(f"- {os.path.basename(d)} vs {os.path.basename(rten)}: {n} tracks, max abs logit diff {max(diffs):.2e} "
                  f"(median per track {statistics.median(diffs):.2e}), beat peaks equal {peaks}/{n}, final grid within 1 ms and BPM within 0.01 on {outs}/{n}")

    print("\n## Grid eval\n")
    for jsonl in sorted(glob.glob(os.path.join(out, "*-r1.jsonl"))):
        res = subprocess.run([sys.executable, "-I", score, ref, jsonl], capture_output=True, text=True)
        print(f"### {os.path.basename(jsonl)[:-6]}\n\n```\n{res.stdout.strip() or res.stderr.strip()}\n```\n")


main()
