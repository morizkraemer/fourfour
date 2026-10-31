import json, re, sys, os
from collections import Counter
import numpy as np
BANDS = os.environ['KEY_BANDS']
RS = os.path.join(os.path.dirname(os.path.abspath(__file__)), '../../../fourfour-analysis-rs/src/cyankey.rs')
src = open(RS).read()
def arr(name):
    m = re.search(name + r'[^=]*=\s*\[(.*?)\];', src, re.S)
    return np.array([float(x) for x in re.findall(r'-?\d+\.\d+', m.group(1))])
T = arr('const TEMPLATES').reshape(2, 36)
MAJOR_BIAS = 0.442
MIDI_LO = 24
NB = 72
idx = json.load(open(f'{BANDS}/index.json'))
idx.sort(key=lambda e: e['id'])
DATA = []
for e in idx:
    r = np.fromfile(f"{BANDS}/{e['id']}.f32", dtype='<f4').reshape(e['frames'], NB).astype(np.float64)
    DATA.append((e['id'], e['truth'], r))

def whiten(rows, W=6, eps_rel=1e-6):
    top = rows.max()
    floor = eps_rel * top + 1e-12
    pad = np.pad(rows, ((0, 0), (W, W)), mode='edge')
    c = np.cumsum(np.pad(pad, ((0, 0), (1, 0))), axis=1)
    env = (c[:, 2 * W + 1:] - c[:, :-(2 * W + 1)]) / (2 * W + 1)
    return rows / (env + floor)

RANGES = [(12, 24), (24, 48), (48, 72)]
FOLD = np.zeros((72, 36))
for v, (a, b) in enumerate(RANGES):
    for m in range(a, b):
        FOLD[m, 12 * v + (MIDI_LO + m) % 12] = 1.0

def fold(rows):
    return rows @ FOLD

def norm3(x):
    x = np.array(x, dtype=np.float64)
    for v in range(3):
        seg = x[..., 12 * v:12 * v + 12]
        n = np.linalg.norm(seg, axis=-1, keepdims=True)
        x[..., 12 * v:12 * v + 12] = np.where(n > 0, seg / np.where(n > 0, n, 1), 0)
    return x

ROLL = np.array([[(i + k) % 12 for i in range(12)] for k in range(12)])
def tmat(tmpl=T, bias=MAJOR_BIAS):
    M = np.zeros((36, 24))
    for mode in range(2):
        for k in range(12):
            for v in range(3):
                for i in range(12):
                    M[12 * v + (i + k) % 12, 12 * mode + k] += tmpl[mode, 12 * v + i]
    b = np.zeros(24); b[:12] = bias
    return M, b
M0, B0 = tmat()
def scores(x, M=M0, b=B0):
    return x @ M + b

def bucket(p, t):
    pt, tt, pm, tm = p % 12, t % 12, p >= 12, t >= 12
    if p == t: return 'exact'
    if pm != tm and pt == tt: return 'parallel'
    if pm != tm and ((not pm and pt == (tt + 3) % 12) or (pm and pt == (tt + 9) % 12)): return 'relative'
    if pm == tm and ((pt - tt) % 12 in (5, 7)): return 'fifth'
    return 'other'

def in_split(i, split):
    return split == 'all' or (split == 'dev') == (i % 3 != 0)

def evaluate(predict, split='dev', verbose=True, label=''):
    c = Counter(); n = 0; preds = {}
    for i, t, r in DATA:
        if not in_split(i, split): continue
        p = predict(r)
        preds[i] = p
        c[bucket(p, t)] += 1; n += 1
    if verbose:
        print(f"{label:44s} n={n} exact={c['exact']} ({100*c['exact']/n:.1f}%) fifth={c['fifth']} rel={c['relative']} par={c['parallel']} other={c['other']}", flush=True)
    return c['exact'], preds
