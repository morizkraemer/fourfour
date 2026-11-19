import json, statistics, sys

for name in sys.argv[1:]:
    rows = [json.loads(l) for l in open(name + '.jsonl')]
    rows = [r for r in rows if 'error' not in r]
    user, sys_, wall = map(float, open(name + '.err').read().strip().splitlines()[-1].split())
    cpu_ms = (user + sys_) * 1000
    audio_min = sum(r['duration_s'] for r in rows) / 60
    model = [r['ms']['model'] / (r['duration_s'] / 60) for r in rows]
    total = [r['ms']['total'] / (r['duration_s'] / 60) for r in rows]
    modes = {}
    for r in rows:
        modes[r.get('beat_mode', 'full')] = modes.get(r.get('beat_mode', 'full'), 0) + 1
    print(f'{name}: n {len(rows)} audio {audio_min:.0f} min (median {statistics.median(r["duration_s"] for r in rows) / 60:.1f} min); '
          f'CPU {cpu_ms / len(rows):.0f} ms/track, {cpu_ms / audio_min:.0f} ms/audio-min; wall {wall:.0f} s; '
          f'median per-track model {statistics.median(model):.0f} ms/audio-min, total {statistics.median(total):.0f} ms/audio-min; modes {modes}')
    work = 0.0
    for r in rows:
        m = r.get('beat_mode', 'full')
        work += sum(c['to_s'] - c['from_s'] for c in r['window_checks']) if m in ('windows', 'extended') else r['duration_s']
    print(f'    beat-model work share {work / sum(r["duration_s"] for r in rows):.3f}')
