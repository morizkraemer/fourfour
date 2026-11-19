import json, sys
ref = json.load(open('/home/merlin/dev/projects/fourfour/benchmark/results/o3/ref.json'))
rows = [json.loads(l) for l in open(sys.argv[1])]
a = sum(1 for r in rows if r.get('bpm') and abs(r['bpm'] - ref[str(r['id'])]['bpm_masterdb']) <= 0.04 * ref[str(r['id'])]['bpm_masterdb'])
print('Acc1', a, '/', len(rows))
