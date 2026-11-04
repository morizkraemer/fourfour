#!/usr/bin/env python3
"""Replaces the rows of <base.jsonl> by the rows with the same id in <patch.jsonl>, writes <out.jsonl>.
usage: r101-splice.py <base.jsonl> <patch.jsonl> <out.jsonl>"""
import json
import sys

patch = {json.loads(line)["id"]: line for line in open(sys.argv[2])}
with open(sys.argv[3], "w") as out:
    for line in open(sys.argv[1]):
        out.write(patch.get(json.loads(line)["id"], line))
