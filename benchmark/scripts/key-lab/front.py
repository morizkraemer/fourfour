import os, sys
import numpy as np

from profiles import templates
from fit import load, tally, fmt

MIDI_LO = 24
beat = load(os.environ['KEY_BANDS'])
fsl = load(os.environ['FSL_BANDS'], os.environ['FSL_MANIFEST'])
bt = np.array([t for _, t, _, _ in beat])
bid = np.array([i for i, _, _, _ in beat])
bdev = bid % 3 != 0
ft = np.array([t for _, t, _, _ in fsl])
SHOW_TEST = os.environ.get('SHOW_TEST') == '1'


def whiten(rows, w):
    if w <= 0:
        return rows / (rows.mean(1, keepdims=True) + 1e-12)
    top = rows.max()
    floor = 1e-6 * top + 1e-12
    pad = np.pad(rows, ((0, 0), (w, w)), mode='edge')
    c = np.cumsum(np.pad(pad, ((0, 0), (1, 0))), axis=1)
    env = (c[:, 2 * w + 1:] - c[:, :-(2 * w + 1)]) / (2 * w + 1)
    return rows / (env + floor)


def fold_matrix(ranges):
    F = np.zeros((72, 12 * len(ranges)))
    for v, (a, b) in enumerate(ranges):
        for m in range(a, b):
            F[m, 12 * v + (MIDI_LO + m) % 12] = 1.0
    return F


def normv(x, nv):
    x = x.copy()
    for v in range(nv):
        seg = x[..., 12 * v:12 * v + 12]
        n = np.linalg.norm(seg, axis=-1, keepdims=True)
        x[..., 12 * v:12 * v + 12] = np.where(n > 0, seg / np.where(n > 0, n, 1), 0)
    return x


def feature(rows, cfg):
    r = whiten(rows, cfg['white'])
    if cfg.get('peaks'):
        left = np.pad(r, ((0, 0), (1, 0)), mode='edge')[:, :-1]
        right = np.pad(r, ((0, 0), (0, 1)), mode='edge')[:, 1:]
        r = np.where((r >= left) & (r >= right), r, cfg['peaks'] * r)
    if cfg['comp'] == 'log':
        r = np.log1p(r)
    else:
        r = r ** cfg['comp']
    F = fold_matrix(cfg['ranges'])
    c = r @ F
    if cfg.get('framenorm'):
        c = normv(c, len(cfg['ranges']))
    if cfg.get('hsmooth'):
        k = cfg['hsmooth']
        if len(c) > 1:
            pad = np.pad(c, ((k, k), (0, 0)), mode='edge')
            cs = np.cumsum(np.pad(pad, ((1, 0), (0, 0))), axis=0)
            c = (cs[2 * k + 1:] - cs[:-(2 * k + 1)]) / (2 * k + 1)
    agg = cfg.get('agg', 'sum')
    x = np.median(c, 0) if agg == 'median' else c.sum(0)
    return normv(x, len(cfg['ranges']))


def rotations(x, nv):
    ROT = np.array([[(i + k) % 12 for i in range(12)] for k in range(12)])
    return np.stack([np.concatenate([x[..., 12 * v + ROT[k]] for v in range(nv)], axis=-1) for k in range(12)], axis=-2)


def logits(xr, W, b):
    s = np.einsum('nkf,mf->nmk', xr, W)
    s[:, 0, :] += b
    return s.reshape(len(xr), 24)


def train(x, y, nv, l2=1e-3, epochs=600, lr=0.05, prior=None, sw=None, fixed_bias=None):
    xr = rotations(x, nv)
    W = np.zeros((2, 12 * nv)); b = 0.0 if fixed_bias is None else fixed_bias
    mW, vW, mb, vb = np.zeros_like(W), np.zeros_like(W), 0.0, 0.0
    if np.ndim(y) == 2:
        Y = np.asarray(y, dtype=np.float64)
    else:
        Y = np.zeros((len(y), 24)); Y[np.arange(len(y)), y] = 1
    sw = np.ones(len(y)) if sw is None else sw
    sw = sw / sw.sum()
    for t in range(1, epochs + 1):
        s = logits(xr, W, b)
        s -= s.max(1, keepdims=True)
        p = np.exp(s); p /= p.sum(1, keepdims=True)
        g = ((p - Y) * sw[:, None]).reshape(len(y), 2, 12)
        gW = np.einsum('nmk,nkf->mf', g, xr) + l2 * (W if prior is None else W - prior)
        gb = g[:, 0, :].sum() if fixed_bias is None else 0.0
        mW = 0.9 * mW + 0.1 * gW; vW = 0.999 * vW + 0.001 * gW ** 2
        mb = 0.9 * mb + 0.1 * gb; vb = 0.999 * vb + 0.001 * gb ** 2
        W -= lr * (mW / (1 - 0.9 ** t)) / (np.sqrt(vW / (1 - 0.999 ** t)) + 1e-8)
        b -= lr * (mb / (1 - 0.9 ** t)) / (np.sqrt(vb / (1 - 0.999 ** t)) + 1e-8)
    return W, b


def predict(x, W, b, nv):
    return np.argmax(logits(rotations(x, nv), W, b), axis=1)


def make_prior(spec, nv):
    if spec == 'none':
        return None
    name, scale = spec.split(',')
    return float(scale) * templates(name, (1,) * nv, 0.6, 4)


RANGES = {
    '3': [(12, 24), (24, 48), (48, 72)],
    '4': [(0, 12), (12, 24), (24, 48), (48, 72)],
    '6': [(0, 12), (12, 24), (24, 36), (36, 48), (48, 60), (60, 72)],
    '5': [(12, 24), (24, 36), (36, 48), (48, 60), (60, 72)],
    '1': [(12, 72)],
    '2': [(12, 36), (36, 72)],
}


def parse(spec):
    kv = dict(x.split('=') for x in spec.split())
    comp = kv.get('comp', '0.75')
    return {
        'white': int(kv.get('white', 6)),
        'comp': comp if comp == 'log' else float(comp),
        'ranges': RANGES[kv.get('ranges', '3')],
        'framenorm': kv.get('framenorm') == '1',
        'peaks': float(kv.get('peaks', 0)) if 'peaks' in kv else 0,
        'hsmooth': int(kv.get('hsmooth', 0)),
        'agg': kv.get('agg', 'sum'),
        'l2': float(kv.get('l2', 1e-3)),
        'prior': kv.get('prior', 'krumhansl,3'),
        'balance': kv.get('balance', '0') == '1',
        'oracle': kv.get('oracle', '0') == '1',
    }


def run(spec):
    cfg = parse(spec)
    nv = len(cfg['ranges'])
    xb = np.array([feature(r, cfg) for _, _, r, _ in beat])
    xf = np.array([feature(r, cfg) for _, _, r, _ in fsl])
    prior = make_prior(cfg['prior'], nv)
    sw = None
    if cfg['balance']:
        minor = ft >= 12
        sw = np.where(minor, 0.5 / minor.mean(), 0.5 / (1 - minor.mean()))
    W, b = train(xf, ft, nv, l2=cfg['l2'], prior=prior, sw=sw)
    p = predict(xb, W, b, nv)
    d = tally(p[bdev], bt[bdev])
    line = f'{spec:70s} b={b:+.2f} dev {fmt(*d)}'
    if cfg['oracle']:
        dev_idx = np.where(bdev)[0]
        pred = np.zeros(len(dev_idx), dtype=int)
        fold = (bid[dev_idx] // 3) % 5
        for k in range(5):
            tr = fold != k
            Wo, bo = train(xb[dev_idx[tr]], bt[dev_idx[tr]], nv, l2=1e-4)
            pred[~tr] = predict(xb[dev_idx[~tr]], Wo, bo, nv)
        o = tally(pred, bt[dev_idx])
        line += f' | oracle-cv {fmt(*o)}'
    if SHOW_TEST:
        line += f' | test {fmt(*tally(p[~bdev], bt[~bdev]))}'
    print(line, flush=True)
    return W, b, cfg


if __name__ == '__main__':
    for spec in sys.argv[1:]:
        run(spec)
