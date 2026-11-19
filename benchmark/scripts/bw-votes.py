import bisect, json, sys


def votes(r):
    beats, delay = r['final_beats_s'], r['mp3_delay_ms'] / 1000
    period = (beats[-1] - beats[0]) / (len(beats) - 1)
    v = [0, 0, 0, 0]
    for d in r['downbeats_s']:
        t = d + delay
        j = bisect.bisect_left(beats, t)
        c = min((k for k in (j - 1, j) if 0 <= k < len(beats)), key=lambda k: abs(beats[k] - t))
        if abs(beats[c] - t) < period / 2:
            v[c % 4] += 1
    return v


def decide(v, share):
    top = max(range(4), key=lambda p: (v[p], -p))
    return top if v[top] >= share * max(1, sum(v)) else 0


full = {json.loads(l)['id']: json.loads(l) for l in open(sys.argv[1])}
win = [json.loads(l) for l in open(sys.argv[2])]
for share in [0.8, 0.85, 0.9, 0.95, 1.01]:
    agree = differ = 0
    ids = []
    for w in win:
        if w.get('beat_mode') not in ('windows', 'extended'):
            continue
        f = full[w['id']]
        if decide(votes(w), share) == f['bar_phase']:
            agree += 1
        else:
            differ += 1
            ids.append(w['id'])
    print(f'share {share}: windowed bar decision same as full {agree}, different {differ} {ids}')
