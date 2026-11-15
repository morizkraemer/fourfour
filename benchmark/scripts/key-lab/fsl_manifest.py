import json, os, sys, zlib

MAJOR = ["8B", "3B", "10B", "5B", "12B", "7B", "2B", "9B", "4B", "11B", "6B", "1B"]
MINOR = ["5A", "12A", "7A", "2A", "9A", "4A", "11A", "6A", "1A", "8A", "3A", "10A"]

rows = json.load(open(sys.argv[1]))
audio_dir = sys.argv[2]
entries = []
for r in rows:
    f = os.path.basename(r['zip'])
    if not os.path.exists(os.path.join(audio_dir, f)):
        continue
    k = r['key']
    entries.append({
        'id': str(r['id']),
        'path': f,
        'fold': zlib.crc32(r['user'].encode()) % 5,
        'licence': r['licence'],
        'ground_truth': {'key': MINOR[k - 12] if k >= 12 else MAJOR[k]},
    })
json.dump({'name': 'fsl10k-key-clean', 'source': 'Freesound Loop Dataset (Zenodo 3967852), CC0 / CC BY 3.0 loops, agreeing annotators', 'entries': entries}, open(sys.argv[3], 'w'), indent=1)
print(len(entries), 'entries')
