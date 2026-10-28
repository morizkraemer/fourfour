#!/usr/bin/env python3
"""Bar-1 rules vs the Rekordbox bar positions, on ffa JSONL output (final grid + model downbeats).
usage: o3-bar-rules.py <ref.json> <ffa.jsonl>...
Scored on tracks with BPM within 0.5 whose grid matches Rekordbox beat for beat (one bar phase covers at least 90% of the matched beats).
vote = the app's bar_phase (most model downbeats); first = grid beat 0 is bar 1; first-unless-T = beat 0 unless the model's top phase
holds at least T of the downbeat votes."""
import bisect
import collections
import json
import sys


def nearest(xs, t):
    j = bisect.bisect_left(xs, t)
    return min((k for k in (j - 1, j) if 0 <= k < len(xs)), key=lambda k: abs(xs[k] - t))


def main():
    ref = json.load(open(sys.argv[1]))
    tracks = []
    for path in sys.argv[2:]:
        for line in open(path):
            a = json.loads(line)
            if a.get("id") is None or a.get("bpm") is None:
                continue
            r = ref[str(a["id"])]
            fb = a["final_beats_s"]
            est = [t * 1000 for t in fb]
            shifts = collections.Counter()
            for t, b in zip(r["beats_ms"], r["bar_pos"]):
                c = nearest(est, t)
                if abs(est[c] - t) < 35:
                    shifts[(c + 1 - b) % 4] += 1
            if not shifts or abs(a["bpm"] - r["bpm_masterdb"]) > 0.5:
                continue
            truth, n = shifts.most_common(1)[0]
            if n < 0.9 * sum(shifts.values()):
                continue
            delay = a["mp3_delay_ms"] / 1000
            period = (fb[-1] - fb[0]) / (len(fb) - 1)
            votes = [0] * 4
            for d in a["downbeats_s"]:
                c = nearest(fb, d + delay)
                if abs(fb[c] - d - delay) < period / 2:
                    votes[c % 4] += 1
            tracks.append((a["id"], truth, votes))
    n = len(tracks)
    print(f"scored tracks {n}")
    def report(name, rule):
        ok = sum(rule(v) == t for _, t, v in tracks)
        print(f"  {name:22s} {ok}/{n} = {100 * ok / max(1, n):.1f}%")
    vote = lambda v: max(range(4), key=lambda p: (v[p], -p))
    report("vote (app now)", vote)
    report("first", lambda v: 0)
    for t in (0.5, 0.6, 0.7, 0.8, 0.9):
        report(f"first-unless-{t}", lambda v, t=t: vote(v) if v[vote(v)] >= t * max(1, sum(v)) else 0)


main()
