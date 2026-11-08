import json, os, sys
import numpy as np

from front import beat, fsl, bt, bdev, ft, feature, parse, train, predict, make_prior
from fit import tally, fmt

meta = json.load(open(os.environ['FSL_META']))
clean = {r['id']: r for r in json.load(open(os.environ['FSL_CLEAN']))}
ids = [i for i, _, _, _ in fsl]
tags = [' '.join(meta[str(i)].get('tags', []) + [meta[str(i)]['name']]).lower() for i in ids]
nann = np.array([clean[i]['n_ann'] for i in ids])


def has(words):
    return np.array([any(w in t for w in words) for t in tags])


bass = has(['bass'])
drum = has(['drum', 'beat', 'perc'])
chord = has(['chord', 'pad', 'piano', 'keys', 'synth', 'guitar', 'strings', 'organ', 'arp', 'melod', 'lead'])
cfg = parse(sys.argv[1] if len(sys.argv) > 1 else '')
nv = len(cfg['ranges'])
xb = np.array([feature(r, cfg) for _, _, r, _ in beat])
xf = np.array([feature(r, cfg) for _, _, r, _ in fsl])
prior = make_prior(cfg['prior'], nv)
print('n', len(ids), 'bass', bass.sum(), 'drum', drum.sum(), 'chord', chord.sum(), 'multi', (nann > 1).sum())
for name, keep in (('all', np.ones(len(ids), bool)), ('no bass', ~bass), ('no drum', ~drum), ('chord-ish', chord), ('chord no bass', chord & ~bass), ('multi-ann', nann > 1), ('no bass-only', ~(bass & ~chord))):
    W, b = train(xf[keep], ft[keep], nv, l2=cfg['l2'], prior=prior)
    p = predict(xb, W, b, nv)
    print(f'{name:16s} n={keep.sum():4d} b={b:+.2f} dev {fmt(*tally(p[bdev], bt[bdev]))}', flush=True)
