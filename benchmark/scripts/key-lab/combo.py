import json, os, sys
import numpy as np

from lab import norm3
from profiles import templates
from fit import load, features, train, predict, tally, fmt

beat = load(os.environ['KEY_BANDS'])
fsl = load(os.environ['FSL_BANDS'], os.environ['FSL_MANIFEST'])
bt = np.array([t for _, t, _, _ in beat])
bdev = np.array([i % 3 != 0 for i, _, _, _ in beat])
ft = np.array([t for _, t, _, _ in fsl])
folds = np.array([f for _, _, _, f in fsl])
xb = features(beat, 1.0)
xf = features(fsl, 1.0)


def blocks_file(path):
    tr = json.load(open(path))['tracks']
    x = norm3(np.array([np.array(t['blocks'])[:, :36].sum(0) for t in tr]))
    return x, np.array([t['class'] for t in tr])


extra = {}
for spec in sys.argv[1:]:
    name, paths = spec.split('=')
    xs, ys = zip(*(blocks_file(p) for p in paths.split(',')))
    extra[name] = (np.concatenate(xs), np.concatenate(ys))

for name, (xe, ye) in extra.items():
    for wf in (1, 4):
        for l2, pr in ((0.001, None), (0.001, 'krumhansl'), (0.0003, None)):
            prior = None if pr is None else 3 * templates(pr, (1, 1, 1), 0.6, 4)
            pred = np.zeros(len(ft), dtype=int)
            for k in range(5):
                tr = folds != k
                X = np.concatenate([np.repeat(xf[tr], wf, 0), xe]); Y = np.concatenate([np.repeat(ft[tr], wf), ye])
                W, b = train(X, Y, l2=l2, prior=prior)
                pred[~tr] = predict(xf[~tr], W, b)
            cv = tally(pred, ft)
            W, b = train(np.concatenate([np.repeat(xf, wf, 0), xe]), np.concatenate([np.repeat(ft, wf), ye]), l2=l2, prior=prior)
            p = predict(xb, W, b)
            print(f'FSLx{wf}+{name} l2 {l2} prior {pr}: fsl-cv {fmt(*cv)} | dev {fmt(*tally(p[bdev], bt[bdev]))} | test {fmt(*tally(p[~bdev], bt[~bdev]))}', flush=True)
