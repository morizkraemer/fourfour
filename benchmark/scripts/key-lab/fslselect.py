import json, zipfile, collections, sys, os

ann_zip, meta_path, names_path, out = sys.argv[1:5]
meta = json.load(open(meta_path))
names = json.load(open(names_path))
wav = {}
for n in names:
    if n.startswith('audio/wav/') and n.endswith('.wav'):
        wav[os.path.basename(n).split('_')[0]] = n
PITCH = {'c': 0, 'c#': 1, 'd': 2, 'd#': 3, 'e': 4, 'f': 5, 'f#': 6, 'g': 7, 'g#': 8, 'a': 9, 'a#': 10, 'b': 11}
z = zipfile.ZipFile(ann_zip)
votes = collections.defaultdict(list)
modes = collections.Counter()
for n in z.namelist():
    if not n.endswith('.json'):
        continue
    d = json.loads(z.read(n))
    sid = n.split('sound-')[1][:-5]
    if d.get('discard'):
        votes[sid].append('discard')
        continue
    k, m = str(d.get('key')).lower(), str(d.get('mode')).lower()
    modes[m] += 1
    if k in PITCH and m in ('maj', 'min'):
        votes[sid].append(PITCH[k] + (12 if m == 'min' else 0))
    else:
        votes[sid].append('none')
print('modes', modes.most_common())
CLEAN = ('http://creativecommons.org/licenses/by/3.0/', 'http://creativecommons.org/publicdomain/zero/1.0/')
stats = collections.Counter()
rows = []
for sid, v in votes.items():
    keys = [x for x in v if isinstance(x, int)]
    if not keys:
        stats['no key'] += 1
        continue
    if len(set(keys)) > 1 or len(keys) < len(v):
        stats['annotators disagree'] += 1
        continue
    md = meta.get(sid)
    if md is None or sid not in wav:
        stats['no audio'] += 1
        continue
    if md['license'] not in CLEAN:
        stats['licence ' + md['license'].split('/')[-3]] += 1
        continue
    stats['kept'] += 1
    rows.append({'id': int(sid), 'key': keys[0], 'n_ann': len(keys), 'licence': md['license'], 'user': md['username'], 'name': md['name'], 'zip': wav[sid]})
print(stats.most_common())
print('minor share', sum(r['key'] >= 12 for r in rows) / max(1, len(rows)), 'multi-annotator', sum(r['n_ann'] > 1 for r in rows))
print('users', len(set(r['user'] for r in rows)), collections.Counter(r['user'] for r in rows).most_common(5))
json.dump(rows, open(out, 'w'), indent=0)
