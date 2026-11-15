#!/usr/bin/env python3
"""Copy a `bench_startup dump` state file and add three large playlists (1k, 5k, 20k) with shuffled track order.

usage: r124-playlist-variant.py <in-state.json> <out-state.json>
The three lists hold distinct library ids in random order plus two ids that are not in the library.
"""
import json
import random
import sys

src, dst = sys.argv[1:3]
state = json.load(open(src))
ids = list(state["tracks"]["id"])
rng = random.Random(124)
next_id = max(p["id"] for p in state["playlists"]) + 1
for name, size in (("R124 1k", 1000), ("R124 5k", 5000), ("R124 20k", 20000)):
    size = min(size, len(ids))
    picked = rng.sample(ids, size)
    picked[size // 3] = max(ids) + 1000
    picked[size // 2] = max(ids) + 1001
    state["playlists"].append({"id": next_id, "name": name, "track_ids": picked, "parent_id": 0, "is_folder": False})
    next_id += 1
json.dump(state, open(dst, "w"), separators=(",", ":"))
