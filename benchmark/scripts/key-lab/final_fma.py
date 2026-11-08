import json, os, sys
import numpy as np

from front import beat, fsl, bt, bdev, ft, feature, parse, train, predict, logits, rotations, make_prior
from fit import load, tally, fmt
from clean import fit_cal, softmax_best

unl = load(os.environ['UNL_BANDS'])
folds = np.array([f for _, _, _, f in fsl])
cfg = parse('')
nv = 3
prior = make_prior(cfg['prior'], nv)
xb = np.array([feature(r, cfg) for _, _, r, _ in beat])
xf = np.array([feature(r, cfg) for _, _, r, _ in fsl])
xu = np.array([feature(r, cfg) for _, _, r, _ in unl])


def student(xf_, ft_):
    W0, b0 = train(xf_, ft_, nv, l2=cfg['l2'], prior=prior)
    pseudo = logits(rotations(xu, nv), W0, b0).argmax(1)
    X = np.concatenate([xf_, xu])
    Y = np.concatenate([ft_, pseudo])
    W, b = train(X, Y, nv, l2=cfg['l2'], prior=prior, fixed_bias=b0)
    return W, b


W, b = student(xf, ft)
p = predict(xb, W, b, nv)
print('dev ', fmt(*tally(p[bdev], bt[bdev])))
if os.environ.get('SHOW_TEST') == '1':
    print('test', fmt(*tally(p[~bdev], bt[~bdev])))
oof = np.zeros((len(ft), 24))
for k in range(5):
    tr = folds != k
    Wk, bk = student(xf[tr], ft[tr])
    oof[~tr] = logits(rotations(xf[~tr], nv), Wk, bk)
pb = np.clip(softmax_best(oof), 1e-6, 1 - 1e-6)
a, c = fit_cal(np.log(pb / (1 - pb)), (oof.argmax(1) == ft).astype(float))
print(f'fsl oof exact {(oof.argmax(1) == ft).mean():.3f} cal A={a:.5f} C={c:.5f}')
for name, mask in (('dev', bdev), ('test', ~bdev)):
    if name == 'test' and os.environ.get('SHOW_TEST') != '1':
        continue
    s = logits(rotations(xb[mask], nv), W, b)
    q = np.clip(softmax_best(s), 1e-6, 1 - 1e-6)
    conf = 1 / (1 + np.exp(-(a * np.log(q / (1 - q)) + c)))
    ok = (s.argmax(1) == bt[mask]).astype(float)
    print(f'  {name}: mean confidence {conf.mean():.3f} vs accuracy {ok.mean():.3f}, Brier {((conf - ok) ** 2).mean():.3f}')
json.dump({'templates': W.tolist(), 'major_bias': float(b), 'cal': [float(a), 0.0, float(c)], 'n_fsl': len(ft), 'n_fma': len(xu)}, open(sys.argv[1], 'w'), indent=1)
rows = ',\n'.join('    [\n' + ',\n'.join('        ' + ', '.join(f'{v:.6f}' for v in W[m, 12 * v0:12 * v0 + 12]) for v0 in range(3)) + ',\n    ]' for m in range(2))
print(f'templates: [\n{rows}\n]\nmajor_bias: {b:.6f}\ncal: [{a:.5f}, 0.0, {c:.5f}]')
