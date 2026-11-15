import json, os, sys
from collections import Counter
import numpy as np

from lab import whiten, fold, norm3, bucket, tmat, T, MAJOR_BIAS, NB

ROT = np.array([[(i + k) % 12 for i in range(12)] for k in range(12)])


def load(bands_dir, manifest=None):
    idx = json.load(open(f'{bands_dir}/index.json'))
    meta = {}
    if manifest:
        meta = {int(e['id']): e for e in json.load(open(manifest))['entries']}
    out = []
    for e in sorted(idx, key=lambda e: e['id']):
        r = np.fromfile(f"{bands_dir}/{e['id']}.f32", dtype='<f4').reshape(e['frames'], NB).astype(np.float64)
        out.append((e['id'], e['truth'], r, meta.get(e['id'], {}).get('fold', e['id'] % 3)))
    return out


def features(data, comp):
    return np.array([norm3(fold(whiten(r) ** comp).sum(0)) for _, _, r, _ in data])


def rotations(x):
    return np.stack([np.concatenate([x[..., 12 * v + ROT[k]] for v in range(3)], axis=-1) for k in range(12)], axis=-2)


def logits(xr, W, b):
    s = np.einsum('nkf,mf->nmk', xr, W)
    s[:, 0, :] += b
    return s.reshape(len(xr), 24)


def train(x, y, l2=1e-3, epochs=600, lr=0.05, bias=True, init=None, prior=None):
    xr = rotations(x)
    W = np.zeros((2, 36)) if init is None else init[0].copy()
    b = 0.0 if init is None else init[1]
    mW, vW, mb, vb = np.zeros_like(W), np.zeros_like(W), 0.0, 0.0
    Y = np.zeros((len(y), 24)); Y[np.arange(len(y)), y] = 1
    for t in range(1, epochs + 1):
        s = logits(xr, W, b)
        s -= s.max(1, keepdims=True)
        p = np.exp(s); p /= p.sum(1, keepdims=True)
        g = (p - Y).reshape(len(y), 2, 12) / len(y)
        gW = np.einsum('nmk,nkf->mf', g, xr) + l2 * (W if prior is None else W - prior)
        gb = g[:, 0, :].sum() if bias else 0.0
        mW = 0.9 * mW + 0.1 * gW; vW = 0.999 * vW + 0.001 * gW ** 2
        mb = 0.9 * mb + 0.1 * gb; vb = 0.999 * vb + 0.001 * gb ** 2
        W -= lr * (mW / (1 - 0.9 ** t)) / (np.sqrt(vW / (1 - 0.999 ** t)) + 1e-8)
        b -= lr * (mb / (1 - 0.9 ** t)) / (np.sqrt(vb / (1 - 0.999 ** t)) + 1e-8)
    return W, b


def predict(x, W, b):
    return np.argmax(logits(rotations(x), W, b), axis=1)


def tally(pred, truth):
    c = Counter(bucket(int(p), int(t)) for p, t in zip(pred, truth))
    n = len(truth)
    mirex = 100 * (c['exact'] + 0.5 * c['fifth'] + 0.3 * c['relative'] + 0.2 * c['parallel']) / n
    return c, n, mirex


def fmt(c, n, mx):
    return f"{c['exact']}/{n}={100*c['exact']/n:.1f}% f{c['fifth']} r{c['relative']} p{c['parallel']} o{c['other']} M{mx:.1f}"
