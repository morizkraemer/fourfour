import json, re, sys, zipfile, collections, os

ann_zip, meta_path, names_path, clean_path, out = sys.argv[1:6]
meta = json.load(open(meta_path))
names = json.load(open(names_path))
wav = {os.path.basename(n).split('_')[0]: n for n in names if n.startswith('audio/wav/') and n.endswith('.wav')}
used = {str(r['id']) for r in json.load(open(clean_path))}
CLEAN = ('http://creativecommons.org/licenses/by/3.0/', 'http://creativecommons.org/publicdomain/zero/1.0/')
PITCH = {'c': 0, 'd': 2, 'e': 4, 'f': 5, 'g': 7, 'a': 9, 'b': 11}

annotated = collections.defaultdict(list)
z = zipfile.ZipFile(ann_zip)
for n in z.namelist():
    if n.endswith('.json'):
        d = json.loads(z.read(n))
        sid = n.split('sound-')[1][:-5]
        annotated[sid].append(d)

MINOR_WORDS = r'(?:m|min|minor|moll)'
MAJOR_WORDS = r'(?:maj|major|dur)'
PAT = re.compile(r'(?<![a-z0-9])([a-g])\s?(#|sharp|b|flat|♯|♭)?[\s_\-]?(' + MINOR_WORDS + '|' + MAJOR_WORDS + r')?(?![a-z0-9])', re.I)
KEYWORD = re.compile(r'\b(key|in)\b', re.I)


def parse(text):
    hits = set()
    for m in PAT.finditer(text):
        root, acc, mode = m.group(1).lower(), (m.group(2) or '').lower(), (m.group(3) or '').lower()
        if not mode and root in ('a',):
            continue
        pc = PITCH[root] + (1 if acc in ('#', 'sharp', '♯') else -1 if acc in ('b', 'flat', '♭') else 0)
        if not mode:
            continue
        minor = mode in ('m', 'min', 'minor', 'moll')
        hits.add(pc % 12 + (12 if minor else 0))
    return hits


stats = collections.Counter()
rows = []
for sid, md in meta.items():
    if sid in used or sid not in wav:
        continue
    if md['license'] not in CLEAN:
        stats['licence'] += 1
        continue
    anns = annotated.get(sid, [])
    if any(str(a.get('mode')).lower() in ('maj', 'min') for a in anns):
        stats['annotated (excluded by b8 rule)'] += 1
        continue
    if any(str(a.get('mode')).lower() == 'none' or a.get('discard') for a in anns):
        stats['annotated no key'] += 1
        continue
    hits = parse(md['name'].replace('.wav', '').replace('.aif', '')) or set()
    tag_hits = set()
    for t in md.get('tags', []):
        tag_hits |= parse(t.replace('-', ' '))
    allh = hits | tag_hits
    if len(allh) == 1:
        k = allh.pop()
        stats['tagged'] += 1
        rows.append({'id': int(sid), 'key': k, 'n_ann': 0, 'source': 'uploader name/tags', 'licence': md['license'], 'user': md['username'], 'name': md['name'], 'tags': md.get('tags', []), 'zip': wav[sid]})
    elif len(allh) > 1:
        stats['conflicting tags'] += 1
    else:
        stats['no key tag'] += 1
print(stats.most_common())
print('minor share', sum(r['key'] >= 12 for r in rows) / max(1, len(rows)), 'users', len(set(r['user'] for r in rows)))
print(collections.Counter(r['user'] for r in rows).most_common(8))
json.dump(rows, open(out, 'w'), indent=0)
