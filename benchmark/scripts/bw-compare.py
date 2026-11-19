import bisect, json, sys, statistics

full = {}
for l in open(sys.argv[1]):
    a = json.loads(l)
    full[a['id']] = a
rows = []
modes = {}
for l in open(sys.argv[2]):
    w = json.loads(l)
    f = full.get(w['id'])
    if f is None or 'error' in w or 'error' in f:
        continue
    mode = w.get('beat_mode', 'full')
    modes[mode] = modes.get(mode, 0) + 1
    fb, wb = f['final_beats_s'], w['final_beats_s']
    if not fb or not wb:
        rows.append((mode, 0.0, False, f['bpm'], w['bpm'], f['duration_s']))
        continue
    near = 0
    for t in fb:
        j = bisect.bisect_left(wb, t)
        d = min(abs(wb[k] - t) for k in (j - 1, j) if 0 <= k < len(wb))
        near += d <= 0.020
    share = near / len(fb)
    fbar = [(i - f['bar_phase']) % 4 for i in range(len(fb))]
    bar_same = None
    if share > 0.9:
        same = tot = 0
        for i, t in enumerate(fb):
            j = bisect.bisect_left(wb, t)
            k = min((k for k in (j - 1, j) if 0 <= k < len(wb)), key=lambda k: abs(wb[k] - t))
            if abs(wb[k] - t) <= 0.035:
                tot += 1
                same += fbar[i] == (k - w['bar_phase']) % 4
        bar_same = same / tot >= 0.9 if tot else None
    rows.append((mode, share, bar_same, f['bpm'], w['bpm'], f['duration_s']))
n = len(rows)
print('n', n, 'modes', modes)
for sel in ('all', 'windows'):
    rs = [r for r in rows if sel == 'all' or r[0] == sel]
    if not rs:
        continue
    print(f'[{sel}] n {len(rs)}: beats within 20ms of full (mean share) {100 * statistics.mean(r[1] for r in rs):.1f}%; '
          f'tracks with >=99% beats within 20ms {sum(r[1] >= 0.99 for r in rs)} = {100 * sum(r[1] >= 0.99 for r in rs) / len(rs):.1f}%; '
          f'bpm same (0.01) {sum(abs((r[3] or 0) - (r[4] or 0)) <= 0.01 for r in rs)}; '
          f'bar-1 same {sum(r[2] is True for r in rs)}/{sum(r[2] is not None for r in rs)}')
bad = sorted((r for r in rows if r[0] == 'windows' and r[1] < 0.99), key=lambda r: r[1])
for r in bad[:15]:
    print('  differs', r)
