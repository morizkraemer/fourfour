import csv, json, sys, collections

tracks_csv, genres_csv, out = sys.argv[1:4]
csv.field_size_limit(1 << 30)
genre_names = {}
for row in csv.DictReader(open(genres_csv)):
    genre_names[int(row['genre_id'])] = row['title']
f = open(tracks_csv, newline='')
r = csv.reader(f)
h1, h2 = next(r), next(r)
next(r)
cols = {f'{a}.{b}': i for i, (a, b) in enumerate(zip(h1, h2))}
LIC = {
    'Attribution': 'CC BY',
    'Creative Commons Attribution 4.0 International License': 'CC BY 4.0',
    'Attribution 3.0 International': 'CC BY 3.0',
    'Attribution 3.0 United States': 'CC BY 3.0 US',
    'Creative Commons Attribution': 'CC BY',
    'CC0 1.0 Universal': 'CC0',
    'Public Domain Mark 1.0': 'PDM',
}
WANT = {'Electronic', 'House', 'Techno', 'Dance', 'Trance', 'Dubstep', 'Drum & Bass', 'Electro-Punk', 'Breakbeat', 'Downtempo', 'Bass Music', 'Chiptune', 'Hardcore', 'Jungle', 'Minimal Electronic', 'Glitch', 'IDM', 'Trip-Hop', 'Chill-out', 'Synth Pop', 'Electroacoustic'}
lic_count = collections.Counter()
rows = []
for row in r:
    lic = row[cols['track.license']]
    lic_count[lic] += 1
    if lic not in LIC:
        continue
    gs = [genre_names.get(int(g)) for g in json.loads(row[cols['track.genres_all']] or '[]')]
    top = row[cols['track.genre_top']]
    if top != 'Electronic' and not (set(gs) & WANT):
        continue
    if set(gs) & {'Noise', 'Musique Concrete', 'Drone', 'Unclassifiable', 'Field Recordings', 'Sound Collage', 'Spoken', 'Audio Collage'}:
        continue
    dur = int(row[cols['track.duration']] or 0)
    if dur < 60:
        continue
    rows.append({'id': int(row[0]), 'licence': LIC[lic], 'licence_text': lic, 'artist': row[cols['artist.name']], 'title': row[cols['track.title']],
                 'genres': gs, 'duration': dur, 'subset': row[cols['set.subset']]})
print(lic_count.most_common(25))
print(len(rows), collections.Counter(x['licence'] for x in rows), collections.Counter(x['subset'] for x in rows))
print(collections.Counter(g for x in rows for g in x['genres']).most_common(25))
print('artists', len(set(x['artist'] for x in rows)))
json.dump(rows, open(out, 'w'), indent=0)
