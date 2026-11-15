import os, sys
import numpy as np

from front import beat, fsl, bt, bid, bdev, ft, whiten, fold_matrix, normv, RANGES, make_prior
from fit import tally, fmt

SHOW_TEST = os.environ.get('SHOW_TEST') == '1'


def perms(nv, quad_bands):
    base = []
    for k in range(12):
        p = [12 * v + (i + k) % 12 for v in range(nv) for i in range(12)]
        off = 12 * nv
        for qb in range(quad_bands):
            for i in range(12):
                for j in range(12):
                    p.append(off + 144 * qb + 12 * ((i + k) % 12) + (j + k) % 12)
        base.append(p)
    return np.array(base)


def feature(rows, cfg):
    r = whiten(rows, cfg['white']) ** cfg['comp']
    F = fold_matrix(cfg['ranges'])
    c = r @ F
    nv = len(cfg['ranges'])
    x = normv(c.sum(0), nv)
    extra = []
    for v in cfg['quad']:
        seg = c[:, 12 * v:12 * v + 12]
        seg = seg / (np.linalg.norm(seg, axis=1, keepdims=True) + 1e-12)
        seg = seg - seg.mean(1, keepdims=True)
        C = seg.T @ seg / len(seg)
        extra.append(C.ravel() / (np.linalg.norm(C) + 1e-12))
    return np.concatenate([x] + extra)


def logits(X, P, W, b):
    xr = X[:, P]
    s = np.einsum('nkf,mf->nmk', xr, W)
    s[:, 0, :] += b
    return s.reshape(len(X), 24)


def train(X, y, P, l2, prior=None, epochs=600, lr=0.05, l2q=None):
    xr = X[:, P]
    nf = X.shape[1]
    W = np.zeros((2, nf)); b = 0.0
    if prior is not None:
        W[:, :prior.shape[1]] = prior * 0
    reg = np.full(nf, l2)
    if l2q is not None:
        reg[prior.shape[1] if prior is not None else 36:] = l2q
    PR = np.zeros((2, nf))
    if prior is not None:
        PR[:, :prior.shape[1]] = prior
    mW, vW, mb, vb = np.zeros_like(W), np.zeros_like(W), 0.0, 0.0
    Y = np.zeros((len(y), 24)); Y[np.arange(len(y)), y] = 1
    for t in range(1, epochs + 1):
        s = np.einsum('nkf,mf->nmk', xr, W); s[:, 0, :] += b; s = s.reshape(len(y), 24)
        s -= s.max(1, keepdims=True)
        p = np.exp(s); p /= p.sum(1, keepdims=True)
        g = (p - Y).reshape(len(y), 2, 12) / len(y)
        gW = np.einsum('nmk,nkf->mf', g, xr) + reg * (W - PR)
        gb = g[:, 0, :].sum()
        mW = 0.9 * mW + 0.1 * gW; vW = 0.999 * vW + 0.001 * gW ** 2
        mb = 0.9 * mb + 0.1 * gb; vb = 0.999 * vb + 0.001 * gb ** 2
        W -= lr * (mW / (1 - 0.9 ** t)) / (np.sqrt(vW / (1 - 0.999 ** t)) + 1e-8)
        b -= lr * (mb / (1 - 0.9 ** t)) / (np.sqrt(vb / (1 - 0.999 ** t)) + 1e-8)
    return W, b


def run(spec):
    kv = dict(x.split('=') for x in spec.split())
    cfg = {'white': int(kv.get('white', 6)), 'comp': float(kv.get('comp', 0.75)), 'ranges': RANGES[kv.get('ranges', '3')],
           'quad': [int(q) for q in kv.get('quad', '').split(',') if q]}
    nv = len(cfg['ranges'])
    P = perms(nv, len(cfg['quad']))
    xb = np.array([feature(r, cfg) for _, _, r, _ in beat])
    xf = np.array([feature(r, cfg) for _, _, r, _ in fsl])
    prior = make_prior(kv.get('prior', 'krumhansl,3'), nv)
    l2 = float(kv.get('l2', 1e-3)); l2q = float(kv.get('l2q', l2))
    W, b = train(xf, ft, P, l2, prior, l2q=l2q)
    p = logits(xb, P, W, b).argmax(1)
    line = f'{spec:50s} b={b:+.2f} dev {fmt(*tally(p[bdev], bt[bdev]))}'
    if kv.get('oracle') == '1':
        dev_idx = np.where(bdev)[0]
        pred = np.zeros(len(dev_idx), dtype=int)
        fold = (bid[dev_idx] // 3) % 5
        for k in range(5):
            tr = fold != k
            Wo, bo = train(xb[dev_idx[tr]], bt[dev_idx[tr]], P, 1e-4, l2q=l2q)
            pred[~tr] = logits(xb[dev_idx[~tr]], P, Wo, bo).argmax(1)
        line += f' | oracle-cv {fmt(*tally(pred, bt[dev_idx]))}'
    if SHOW_TEST:
        line += f' | test {fmt(*tally(p[~bdev], bt[~bdev]))}'
    print(line, flush=True)


if __name__ == '__main__':
    for spec in sys.argv[1:]:
        run(spec)
