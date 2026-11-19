import json, sys

BEATLESS, MIN_SHARE, MAX_MED, MAX_SPREAD = 0.25, float(__import__("os").environ.get("MIN_SHARE", "0.6")), 15.0, float(sys.argv[4]) if len(sys.argv) > 4 else 10.0


def agree(checks):
    beats = [c for c in checks if c['peaks'] >= BEATLESS * c['expected']]
    if len(beats) < 2 or any(c['median_ms'] is None for c in beats):
        return False
    meds = [c['median_ms'] for c in beats]
    return max(meds) - min(meds) <= MAX_SPREAD and all(c['inliers'] >= MIN_SHARE * c['expected'] and abs(c['median_ms']) <= MAX_MED for c in beats)


full = {json.loads(l)['id']: json.loads(l) for l in open(sys.argv[1])}
out = open(sys.argv[3], 'w')
modes = {}
for l in open(sys.argv[2]):
    w = json.loads(l)
    mode = w.get('beat_mode', 'full')
    if mode == 'windows' and not agree(w['window_checks']):
        mode = 'fallback'
    row = w if mode == 'windows' else dict(full[w['id']], beat_mode=mode, window_checks=w.get('window_checks', []))
    modes[mode] = modes.get(mode, 0) + 1
    out.write(json.dumps(row) + '\n')
print('modes', modes)
