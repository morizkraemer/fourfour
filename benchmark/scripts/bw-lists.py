import json, os, statistics
B = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'results')
O = os.path.join(B, 'bw')
os.makedirs(O, exist_ok=True)
ref = json.load(open(B + '/o3/ref.json'))
by_path = {r['path']: k for k, r in ref.items()}
for split in ('dev', 'test'):
    paths = [l.strip() for l in open(f'{B}/o3/{split}.txt') if l.strip()]
    rows = [(by_path[p], p) for p in paths if p in by_path]
    open(f'{O}/{split}.tsv', 'w').write(''.join(f'{i}\t{p}\n' for i, p in rows))
    d = sorted(ref[i]['duration_s'] / 60 for i, _ in rows)
    print(split, len(paths), len(rows), 'min/med/max', round(d[0], 1), round(statistics.median(d), 1), round(d[-1], 1), 'p10/p90', round(d[len(d) // 10], 1), round(d[9 * len(d) // 10], 1))
