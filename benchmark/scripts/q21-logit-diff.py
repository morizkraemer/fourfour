"""Compare Beat This! logits of two dump dirs (bgdump format): q21-logit-diff.py <dirA> <dirB> [max tracks]"""
import json, os, struct, sys

a, b = sys.argv[1], sys.argv[2]
limit = int(sys.argv[3]) if len(sys.argv) > 3 else 20
ids = sorted(int(f[:-5]) for f in os.listdir(a) if f.endswith('.json') and os.path.exists(f'{b}/{f}'))[:limit]
worst = 0.0
same_peaks = 0
for i in ids:
    da, db = json.load(open(f'{a}/{i}.json')), json.load(open(f'{b}/{i}.json'))
    ra, rb = open(f'{a}/{i}.logits', 'rb').read(), open(f'{b}/{i}.logits', 'rb').read()
    n = min(len(ra), len(rb)) // 4
    xa, xb = struct.unpack(f'{n}f', ra[:n * 4]), struct.unpack(f'{n}f', rb[:n * 4])
    d = max(abs(p - q) for p, q in zip(xa, xb))
    worst = max(worst, d)
    same = da['beats'] == db['beats']
    same_peaks += same
    print(i, 'frames', da['frames'], db['frames'], 'max logit diff', round(d, 5), 'same peaks', same)
print('tracks', len(ids), 'worst diff', round(worst, 5), 'identical peak lists', same_peaks)
