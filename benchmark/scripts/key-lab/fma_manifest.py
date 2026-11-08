import json, os, sys

cands = json.load(open(sys.argv[1]))
audio_dir = sys.argv[2]
entries = []
for c in cands:
    f = f"{c['id']:06d}.mp3"
    if os.path.exists(os.path.join(audio_dir, f)) and os.path.getsize(os.path.join(audio_dir, f)) > 10000:
        entries.append({'id': str(c['id']), 'path': f, 'licence': c['licence'], 'ground_truth': {'key': '8B'}})
json.dump({'name': 'fma-electronic-cc-by-unlabelled', 'source': 'FMA fma_large 30 s clips, CC BY / CC0 / PDM, Electronic; key field is a placeholder, unlabelled', 'entries': entries}, open(sys.argv[3], 'w'), indent=1)
print(len(entries), 'entries')
