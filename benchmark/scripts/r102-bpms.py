#!/usr/bin/env python3
"""Prints id, BPM and reference BPM per track of ffa JSONL files.
usage: r102-bpms.py <ref.json> <ffa.jsonl>..."""
import json
import sys

ref = json.load(open(sys.argv[1]))
for path in sys.argv[2:]:
    for line in open(path):
        a = json.loads(line)
        r = ref[str(a["id"])]["bpm_masterdb"]
        print(f"{path.rsplit('/', 1)[-1]:16} id {a['id']:>4} bpm {a['bpm']:9.3f} ref {r:9.3f} diff {a['bpm'] - r:+8.3f}")
