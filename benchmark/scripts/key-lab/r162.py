import json, os, sys
import numpy as np

from front import beat, fsl, bt, bid, bdev, ft, feature, parse, train, logits, rotations, make_prior, whiten, normv
from fit import load, tally, fmt

SHOW_TEST = os.environ.get('SHOW_TEST') == '1'
unl = load(os.environ['UNL_BANDS'])
CACHE = os.environ.get('R162_CACHE', os.path.expanduser('~/.cache/r162-work'))
os.makedirs(CACHE, exist_ok=True)
BASE = parse('')
PRIOR3 = make_prior(BASE['prior'], 3)
SEG = 54


def cyan_frames(rows, comp=0.75):
    r = whiten(rows, 6) ** comp
    F = np.zeros((72, 36))
    for v, (a, b) in enumerate([(12, 24), (24, 48), (48, 72)]):
        for m in range(a, b):
            F[m, 12 * v + (24 + m) % 12] = 1.0
    return r @ F


HSHIFT = [int(round(12 * np.log2(k))) for k in range(1, 9)]


def kd_frames(rows, nh=8, decay=0.9, white=6, bass=(4, 28), mid=(24, 66)):
    r = np.log1p(10 * rows / (rows.mean() + 1e-12))
    h = np.zeros_like(r)
    for k, s in enumerate(HSHIFT[:nh]):
        if s < 72:
            h[:, :72 - s] += decay ** k * r[:, s:]
    pad = np.pad(h, ((0, 0), (white, white)), mode='edge')
    c = np.cumsum(np.pad(pad, ((0, 0), (1, 0))), axis=1)
    env = (c[:, 2 * white + 1:] - c[:, :-(2 * white + 1)]) / (2 * white + 1)
    w = np.maximum(h - env, 0)
    F = np.zeros((72, 24))
    for v, (a, b) in enumerate([bass, mid]):
        for m in range(a, b):
            F[m, 12 * v + (24 + m) % 12] = 1.0
    return normv(w @ F, 2)


def cached(name, fn, data):
    path = f'{CACHE}/{name}.npy'
    if os.path.exists(path):
        return np.load(path, allow_pickle=True)
    out = np.empty(len(data), dtype=object)
    for i, (_, _, r, _) in enumerate(data):
        out[i] = fn(r).astype(np.float32)
    np.save(path, out, allow_pickle=True)
    return out


def glob(frames, nv):
    return np.array([normv(f.sum(0), nv) for f in frames])


def segs(frames, nv, seg=SEG):
    out = []
    for f in frames:
        n = max(1, len(f) // seg)
        edges = np.linspace(0, len(f), n + 1).astype(int)
        out.append(np.array([normv(f[a:b].sum(0), nv) for a, b in zip(edges[:-1], edges[1:])]))
    return out


def probs(s):
    s = s - s.max(1, keepdims=True)
    p = np.exp(s)
    return p / p.sum(1, keepdims=True)


def lg(x, W, b, nv):
    return logits(rotations(x, nv), W, b)


def student(xf, xu, nv, prior, teacher_logits=None, l2=1e-3, fixbias=True):
    W0, b0 = train(xf, ft, nv, l2=l2, prior=prior)
    tl = lg(xu, W0, b0, nv) if teacher_logits is None else teacher_logits
    pseudo = tl.argmax(1)
    W, b = train(np.concatenate([xf, xu]), np.concatenate([ft, pseudo]), nv, l2=l2, prior=prior,
                 fixed_bias=b0 if fixbias else None)
    return (W0, b0), (W, b)


def line(label, pred, extra=''):
    s = f'{label:58s} dev {fmt(*tally(pred[bdev], bt[bdev]))}'
    if SHOW_TEST:
        s += f' | test {fmt(*tally(pred[~bdev], bt[~bdev]))}'
    print(s + extra, flush=True)


def dev_cv(score_fn, grid, n=5, mask=None):
    mask = bdev if mask is None else mask
    idx = np.where(mask)[0]
    fold = (bid[idx] // 3) % n
    pred = np.zeros(len(bt), dtype=int)
    picks = []
    for k in range(n):
        tr, te = idx[fold != k], idx[fold == k]
        best = max(grid, key=lambda g: ((score_fn(g)[tr].argmax(1)) == bt[tr]).sum())
        picks.append(best)
        pred[te] = score_fn(best)[te].argmax(1)
    acc = (pred[idx] == bt[idx]).sum()
    return acc, len(idx), picks, pred
