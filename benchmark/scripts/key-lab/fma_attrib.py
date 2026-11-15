import json, sys, collections

cands_path, bands_index, out_manifest, out_attrib = sys.argv[1:5]
cands = {c['id']: c for c in json.load(open(cands_path))}
used = sorted(int(e['id']) for e in json.load(open(bands_index)))
items = [{'id': i, 'artist': cands[i]['artist'], 'title': cands[i]['title'], 'licence': cands[i]['licence_text'],
          'url': f'https://freemusicarchive.org/track/{i}'} for i in used]
json.dump({'name': 'fma-electronic-cc-unlabelled', 'source': 'FMA fma_large (Defferrard et al., ISMIR 2017), 30 s clips; per-track licence from fma_metadata/tracks.csv; no key labels (pseudo-labelled by the FSL model)', 'items': items},
          open(out_manifest, 'w'), indent=0)
lic = collections.Counter(x['licence'] for x in items)
by_artist = collections.Counter(x['artist'] for x in items if x['licence'] not in ('CC0 1.0 Universal', 'Public Domain Mark 1.0'))
lines = [f'  {a} ({n})' for a, n in sorted(by_artist.items(), key=lambda t: t[0].lower())]
open(out_attrib, 'w').write('\n'.join(lines) + '\n')
print(len(items), lic.most_common(), 'credited artists', len(by_artist))
