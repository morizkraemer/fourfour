import json, os, random
B = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'results')
ref = json.load(open(os.path.join(B, 'o3', 'ref.json')))
used = set()
for s in ('dev', 'test'):
    used |= {l.split('\t')[0] for l in open(os.path.join(B, 'bw', f'{s}.tsv'))}
cand = [k for k, r in ref.items() if k not in used and r.get('audio_present') and r.get('fingerprint_verified', True) and r['duration_s'] > 60]
random.Random(7).shuffle(cand)
pick = cand[:150]
open(os.path.join(B, 'bw', 'tune.tsv'), 'w').write(''.join(f"{k}\t{ref[k]['path']}\n" for k in pick))
print(len(cand), 'candidates, picked', len(pick))
