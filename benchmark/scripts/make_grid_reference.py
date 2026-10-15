#!/usr/bin/env python3
import argparse
import collections
import hashlib
import json
import os
import sys

MAC_PREFIX = "/Users/merlinkraemer/local/2-Areas/1-Mukke/"


def fingerprint(path):
    size = os.path.getsize(path)
    h = hashlib.sha256()
    with open(path, "rb") as f:
        h.update(f.read(65536))
        if size > 65536:
            f.seek(max(size - 65536, 65536))
            h.update(f.read(65536))
    h.update(str(size).encode())
    return h.hexdigest()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--library-dump", required=True)
    ap.add_argument("--manifest", required=True)
    ap.add_argument("--dataset-root", required=True)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()

    lib = json.load(open(a.library_dump))
    entries = json.load(open(a.manifest))["entries"]
    by_path = {r["path"]: r for r in lib}
    by_name = collections.defaultdict(list)
    for r in lib:
        by_name[os.path.basename(r["path"])].append(r)

    out = {}
    stats = collections.Counter()
    for e in entries:
        stats["manifest"] += 1
        rel = e["path"][len(MAC_PREFIX):] if e["path"].startswith(MAC_PREFIX) else e["path"]
        local = os.path.join(a.dataset_root, rel)
        present = os.path.exists(local)
        stats["audio_present"] += present
        row = by_path.get(e["path"])
        how = "path"
        if row is None:
            cands = [r for r in by_name.get(os.path.basename(e["path"]), [])
                     if abs(r["duration_s"] - e["duration_seconds"]) <= 2]
            if len(cands) >= 1:
                row, how = cands[0], "filename+duration"
        if row is None:
            stats["no_library_row"] += 1
            continue
        stats["library_row_" + how.replace("+", "_")] += 1
        if not row["beats"]:
            stats["library_row_without_grid"] += 1
            continue
        verified = False
        if present:
            verified = fingerprint(local) == e["content_fingerprint"]
        stats["grid"] += 1
        stats["grid_audio_present"] += present
        stats["grid_audio_verified"] += verified
        beats = row["beats"]
        out[e["id"]] = {
            "path": local if present else None,
            "audio_present": present,
            "fingerprint_verified": verified,
            "artist": e["artist"],
            "title": e["title"],
            "duration_s": e["duration_seconds"],
            "bpm_masterdb": row["bpm"],
            "bpm_manifest": e["ground_truth"].get("bpm"),
            "beats_ms": [b[1] for b in beats],
            "bar_pos": [b[0] for b in beats],
            "tempo_x100": [b[2] for b in beats],
            "match": how,
        }
    json.dump(out, open(a.out, "w"))
    for k, v in stats.items():
        print(k, v, file=sys.stderr)


main()
