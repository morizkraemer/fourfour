#!/usr/bin/env python3
"""Scores two ffa JSONL runs on the tracks both finished, with o3-score.py, and counts tracks whose BPM or beats changed.
usage: r94-pair.py <ref.json> <base.jsonl> <variant.jsonl>"""
import json
import os
import subprocess
import sys
import tempfile


def rows(path):
    out = {}
    for line in open(path):
        a = json.loads(line)
        if a.get("id") is not None:
            out[a["id"]] = (line, a)
    return out


ref, base_path, var_path = sys.argv[1:4]
base, var = rows(base_path), rows(var_path)
ids = sorted(set(base) & set(var))
score = os.path.join(os.path.dirname(os.path.abspath(__file__)), "o3-score.py")
for label, src in (("base", base), ("variant", var)):
    with tempfile.NamedTemporaryFile("w", suffix=".jsonl", delete=False, dir=os.path.dirname(os.path.abspath(var_path))) as f:
        f.writelines(src[i][0] for i in ids)
    res = subprocess.run([sys.executable, "-I", score, ref, f.name], capture_output=True, text=True, check=True).stdout
    os.unlink(f.name)
    for line in res.splitlines():
        if line.startswith(("n ", "bpm within 0.1", "bpm within 1.0", "grid within 20 ms:", "beat-aligned")):
            print(f"{label}: {line}")
bpm_diff = sum(abs(base[i][1]["bpm"] - var[i][1]["bpm"]) > 0.01 for i in ids)
print(f"paired {len(ids)}; bpm differs (>0.01) on {bpm_diff}")
