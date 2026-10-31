import json, sys, os
import numpy as np

from lab import T, MAJOR_BIAS
from profiles import templates
from fit import load, features, train, predict, tally, fmt, logits, rotations

BEAT = os.environ['KEY_BANDS']
FSL = os.environ.get('FSL_BANDS')
FSL_MANIFEST = os.environ.get('FSL_MANIFEST')
mode = sys.argv[1]

beat = load(BEAT)
bt = np.array([t for _, t, _, _ in beat])
bdev = np.array([i % 3 != 0 for i, _, _, _ in beat])


def report(label, W, b, comp, xb=None):
    xb = features(beat, comp) if xb is None else xb
    p = predict(xb, W, b)
    d = tally(p[bdev], bt[bdev]); t = tally(p[~bdev], bt[~bdev])
    print(f'{label:52s} dev {fmt(*d)} | test {fmt(*t)}', flush=True)
    return d, t


if mode == 'shipped':
    report('shipped cyanbox', T.reshape(2, 36), MAJOR_BIAS, 0.75)

if mode == 'oracle':
    for comp in (0.75,):
        xb = features(beat, comp)
        for l2 in (0.0, 1e-4, 3e-4):
            W, b = train(xb[bdev], bt[bdev], l2=l2)
            report(f'ORACLE fit on Beatport dev c{comp} l2 {l2}', W, b, comp, xb)

if mode in ('cv', 'final'):
    fsl = load(FSL, FSL_MANIFEST)
    ft = np.array([t for _, t, _, _ in fsl])
    folds = np.array([f for _, _, _, f in fsl])
    print('fsl n', len(fsl), 'minor share', (ft >= 12).mean().round(3), flush=True)
    grid_comp = [float(c) for c in os.environ.get('COMPS', '0.5,0.75,1.0').split(',')]
    grid_l2 = [float(c) for c in os.environ.get('L2S', '0.0003,0.001,0.003,0.01,0.03').split(',')]
    results = []
    for comp in grid_comp:
        xf = features(fsl, comp)
        xb = features(beat, comp)
        for l2 in grid_l2:
          for pr in os.environ.get('PRIORS', 'none').split(';'):
            prior = None if pr == 'none' else float(pr.split(',')[1]) * templates(pr.split(',')[0], (1, 1, 1), 0.6, 4)
            if mode == 'cv':
                pred = np.zeros(len(ft), dtype=int)
                for k in range(5):
                    tr = folds != k
                    W, b = train(xf[tr], ft[tr], l2=l2, prior=prior)
                    pred[~tr] = predict(xf[~tr], W, b)
                c, n, mx = tally(pred, ft)
                results.append((c['exact'] / n, comp, l2, pr))
                W, b = train(xf, ft, l2=l2, prior=prior)
                report(f'FSL cv c{comp} l2 {l2} prior {pr}: fsl-cv {fmt(c, n, mx)}', W, b, comp, xb)
    if mode == 'cv':
        print('best by FSL cv:', max(results))
    else:
        comp, l2, pr = float(os.environ['COMP']), float(os.environ['L2']), os.environ.get('PRIOR', 'none')
        prior = None if pr == 'none' else float(pr.split(',')[1]) * templates(pr.split(',')[0], (1, 1, 1), 0.6, 4)
        xf = features(fsl, comp)
        W, b = train(xf, ft, l2=l2, prior=prior)
        report(f'FINAL FSL all c{comp} l2 {l2}', W, b, comp)
        json.dump({'compression': comp, 'l2': l2, 'prior': pr, 'templates': W.tolist(), 'major_bias': b}, open(sys.argv[2], 'w'), indent=1)
