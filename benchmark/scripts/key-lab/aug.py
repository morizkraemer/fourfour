import json, os, sys
import numpy as np

from lab import whiten, fold, norm3
from profiles import templates
from fit import load, train, predict, tally, fmt

beat = load(os.environ['KEY_BANDS'])
fsl = load(os.environ['FSL_BANDS'], os.environ['FSL_MANIFEST'])
bt = np.array([t for _, t, _, _ in beat])
bdev = np.array([i % 3 != 0 for i, _, _, _ in beat])
ft = np.array([t for _, t, _, _ in fsl])
folds = np.array([f for _, _, _, f in fsl])
SHOW_TEST = os.environ.get('SHOW_TEST') == '1'


def feat(rows, comp):
    return norm3(fold(whiten(rows) ** comp).sum(0))


def shift(rows, s):
    out = np.zeros_like(rows)
    if s > 0:
        out[:, s:] = rows[:, :-s]
    elif s < 0:
        out[:, :s] = rows[:, -s:]
    else:
        out[:] = rows
    return out


def transpose(key, s):
    return (key % 12 + s) % 12 + (12 if key >= 12 else 0)


def tile(rows, n):
    reps = -(-n // len(rows))
    return np.tile(rows, (reps, 1))[:n]


def augmented(idx, comp, shifts, n_mix, mix_sizes, rng):
    X, Y = [], []
    for i in idx:
        _, t, r, _ = fsl[i]
        r = r / (r.max() + 1e-12)
        for s in shifts:
            X.append(feat(shift(r, s), comp)); Y.append(transpose(t, s))
    by_mode = [[i for i in idx if (fsl[i][1] >= 12) == m] for m in (0, 1)]
    for _ in range(n_mix):
        mode = rng.integers(2)
        k = rng.integers(12)
        size = rng.choice(mix_sizes)
        pick = rng.choice(by_mode[mode], size=size, replace=False)
        n = max(len(fsl[i][2]) for i in pick)
        acc = np.zeros((n, 72))
        for i in pick:
            t, r = fsl[i][1], fsl[i][2]
            s = (k - t % 12) % 12
            if s > 6:
                s -= 12
            acc += rng.uniform(0.3, 1.0) * tile(shift(r / (r.max() + 1e-12), s), n)
        X.append(feat(acc, comp)); Y.append(k + 12 * mode)
    return np.array(X), np.array(Y)


def run(label, comp, shifts, n_mix, mix_sizes, l2, prior_spec, seed=0, out=None):
    rng = np.random.default_rng(seed)
    prior = None if prior_spec == 'none' else float(prior_spec.split(',')[1]) * templates(prior_spec.split(',')[0], (1, 1, 1), 0.6, 4)
    X, Y = augmented(range(len(fsl)), comp, shifts, n_mix, mix_sizes, rng)
    W, b = train(X, Y, l2=l2, prior=prior)
    xb = np.array([feat(r, comp) for _, _, r, _ in beat])
    p = predict(xb, W, b)
    d = tally(p[bdev], bt[bdev])
    line = f'{label:60s} b={b:+.2f} dev {fmt(*d)}'
    if SHOW_TEST:
        line += f' | test {fmt(*tally(p[~bdev], bt[~bdev]))}'
    print(line, flush=True)
    if out:
        json.dump({'compression': comp, 'l2': l2, 'prior': prior_spec, 'shifts': list(shifts), 'n_mix': n_mix, 'mix_sizes': list(mix_sizes), 'seed': seed, 'templates': W.tolist(), 'major_bias': float(b)}, open(out, 'w'), indent=1)
    return W, b


if __name__ == '__main__':
    for spec in sys.argv[1:]:
        kv = dict(x.split('=') for x in spec.split())
        shifts = [int(s) for s in kv.get('shifts', '0').split(',')]
        sizes = [int(s) for s in kv.get('sizes', '2,3').split(',')]
        run(spec, float(kv.get('comp', 0.75)), shifts, int(kv.get('mix', 0)), sizes, float(kv.get('l2', 1e-3)), kv.get('prior', 'krumhansl,3'), int(kv.get('seed', 0)), kv.get('out'))
