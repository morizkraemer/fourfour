#!/usr/bin/env python3
"""Grid within 20 ms by container from `grid --out` files. usage: q30-by-container.py <grid out json> [...]"""
import json, sys, collections

for p in sys.argv[1:]:
    c = collections.defaultdict(lambda: [0, 0])
    for t in json.load(open(p))["tracks"]:
        k = t["container"].split(":")[0]
        c[k][1] += 1
        c[k][0] += t["max_dev_ms"] <= 20
    print(p.split("/")[-1], {k: f"{a}/{b}" for k, (a, b) in c.items()})
