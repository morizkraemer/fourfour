import json, os, sys
import numpy as np

from lab import T, MAJOR_BIAS
from profiles import PROFILES, templates
from fit import load, features, train, predict, tally, fmt, logits, rotations

beat = load(os.environ['KEY_BANDS'])
fsl = load(os.environ['FSL_BANDS'], os.environ['FSL_MANIFEST'])
bt = np.array([t for _, t, _, _ in beat])
bdev = np.array([i % 3 != 0 for i, _, _, _ in beat])
ft = np.array([t for _, t, _, _ in fsl])
folds = np.array([f for _, _, _, f in fsl])
FEAT = {}


def feats(data, comp):
    key = (id(data), comp)
    if key not in FEAT:
        FEAT[key] = features(data, comp)
    return FEAT[key]


def beat_line(label, W, b, comp):
    p = predict(feats(beat, comp), W, b)
    d = tally(p[bdev], bt[bdev]); t = tally(p[~bdev], bt[~bdev])
    f = tally(predict(feats(fsl, comp), W, b), ft)
    print(f'{label:44s} fsl {fmt(*f)} | dev {fmt(*d)} | test {fmt(*t)}', flush=True)
    return f[0]['exact'], d, t


def option_a():
    print('# option A: theory profiles, every choice made on FSL (clean)')
    best = None
    for comp in (0.75, 1.0):
        for name in PROFILES:
            for h in ((0, 1), (0.6, 4), (0.8, 6)):
                for bw in ((1, 1, 1), (0, 1, 1), (0.5, 1, 1)):
                    for bias in (-0.2, 0.0, 0.2):
                        W = templates(name, bw, *h)
                        p = predict(feats(fsl, comp), W, bias)
                        e = tally(p, ft)[0]['exact']
                        cand = (e, comp, name, h, bw, bias)
                        if best is None or e > best[0]:
                            best = cand
    e, comp, name, h, bw, bias = best
    beat_line(f'A pick: {name} h{h} bands{bw} bias{bias} c{comp}', templates(name, bw, *h), bias, comp)
    beat_line('A rule: krumhansl, no harmonics, bias 0, c1', templates('krumhansl'), 0.0, 1.0)


def softmax_best(s, temp=1.0):
    s = s - s.max(1, keepdims=True)
    return 1.0 / np.exp(temp * s).sum(1)


def fit_cal(lp, y):
    X = np.stack([lp, np.ones_like(lp)], 1)
    w = np.array([1.0, 0.0])
    for _ in range(200):
        z = X @ w
        q = 1 / (1 + np.exp(-z))
        g = X.T @ (q - y) / len(y)
        H = (X * (q * (1 - q))[:, None]).T @ X / len(y) + 1e-6 * np.eye(2)
        w -= np.linalg.solve(H, g)
    return w


def option_b(comp, l2, prior_spec, out):
    prior = None if prior_spec == 'none' else float(prior_spec.split(',')[1]) * templates(prior_spec.split(',')[0], (1, 1, 1), 0.6, 4)
    xf = feats(fsl, comp)
    oof = np.zeros((len(ft), 24))
    for k in range(5):
        tr = folds != k
        W, b = train(xf[tr], ft[tr], l2=l2, prior=prior)
        oof[~tr] = logits(rotations(xf[~tr]), W, b)
    W, b = train(xf, ft, l2=l2, prior=prior)
    beat_line(f'B c{comp} l2 {l2} prior {prior_spec}', W, b, comp)
    p = np.clip(softmax_best(oof), 1e-6, 1 - 1e-6)
    correct = (oof.argmax(1) == ft).astype(float)
    a, c = fit_cal(np.log(p / (1 - p)), correct)
    print(f'calibration on FSL out-of-fold: A={a:.5f} C={c:.5f} (B=0, temperature=1)')
    for name, data, mask, truth in (('beatport dev', beat, bdev, bt[bdev]), ('beatport test', beat, ~bdev, bt[~bdev])):
        s = logits(rotations(feats(data, comp)[mask]), W, b)
        pb = np.clip(softmax_best(s), 1e-6, 1 - 1e-6)
        conf = 1 / (1 + np.exp(-(a * np.log(pb / (1 - pb)) + c)))
        ok = (s.argmax(1) == truth).astype(float)
        print(f'  {name}: mean confidence {conf.mean():.3f} vs accuracy {ok.mean():.3f}, Brier {((conf - ok) ** 2).mean():.3f}')
    json.dump({'compression': comp, 'l2': l2, 'prior': prior_spec, 'templates': W.tolist(), 'major_bias': float(b), 'cal': [float(a), 0.0, float(c)]}, open(out, 'w'), indent=1)
    rows = ',\n'.join('        [\n' + ',\n'.join('            ' + ', '.join(f'{v:.6f}' for v in W[m, 12 * v0:12 * v0 + 12]) for v0 in range(3)) + ',\n        ]' for m in range(2))
    print(f'templates: [\n{rows}\n    ],\n    major_bias: {b:.6f},\n    cal: [{a:.5f}, 0.0, {c:.5f}],')


def shipped_cal():
    s = logits(rotations(feats(beat, 0.75)), T.reshape(2, 36), MAJOR_BIAS)
    for name, mask in (('beatport dev', bdev), ('beatport test', ~bdev)):
        pb = np.clip(softmax_best(s[mask], 0.9349), 1e-6, 1 - 1e-6)
        print(f'  shipped {name}: mean p_best {pb.mean():.3f} vs accuracy {(s[mask].argmax(1) == bt[mask]).mean():.3f} (cal also uses segment agreement)')


if __name__ == '__main__':
    beat_line('shipped cyanbox (GiantSteps MTG fit)', T.reshape(2, 36), MAJOR_BIAS, 0.75)
    if 'a' in sys.argv[1]:
        option_a()
    if 'b' in sys.argv[1]:
        option_b(float(os.environ.get('COMP', '0.75')), float(os.environ.get('L2', '0.001')), os.environ.get('PRIOR', 'krumhansl,3'), sys.argv[2])
        shipped_cal()
