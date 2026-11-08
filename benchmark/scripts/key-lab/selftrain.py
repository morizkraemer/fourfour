import json, os, sys
import numpy as np

from front import beat, fsl, bt, bdev, ft, feature, parse, train, predict, logits, rotations, make_prior
from fit import load, tally, fmt

SHOW_TEST = os.environ.get('SHOW_TEST') == '1'
unl = load(os.environ['UNL_BANDS'])


def probs(x, W, b, nv):
    s = logits(rotations(x, nv), W, b)
    s -= s.max(1, keepdims=True)
    p = np.exp(s)
    return p / p.sum(1, keepdims=True)


def report(label, W, b, nv, xb):
    p = predict(xb, W, b, nv)
    line = f'{label:64s} b={b:+.2f} dev {fmt(*tally(p[bdev], bt[bdev]))}'
    if SHOW_TEST:
        line += f' | test {fmt(*tally(p[~bdev], bt[~bdev]))}'
    print(line, flush=True)


def selftrain(xf, xu, nv, cfg, prior, rounds, thr, weight, verbose_xb=None, label='', fixbias=False, soft=0.0, keep_fsl=None):
    global ft
    if keep_fsl is not None:
        xf, ft = xf[keep_fsl], ft[keep_fsl]
    W, b = train(xf, ft, nv, l2=cfg['l2'], prior=prior)
    b0 = b if fixbias else None
    if verbose_xb is not None:
        report(f'{label} round 0', W, b, nv, verbose_xb)
    for rnd in range(1, rounds + 1):
        p = probs(xu, W, b, nv)
        keep = p.max(1) >= thr
        X = np.concatenate([xf, xu[keep]])
        if soft > 0:
            s_ = logits(rotations(xu[keep], nv), W, b) * soft
            s_ -= s_.max(1, keepdims=True)
            q = np.exp(s_); q /= q.sum(1, keepdims=True)
            F1 = np.zeros((len(ft), 24)); F1[np.arange(len(ft)), ft] = 1
            Y = np.concatenate([F1, q])
        else:
            Y = np.concatenate([ft, p.argmax(1)[keep]])
        sw = np.concatenate([np.ones(len(ft)), np.full(keep.sum(), weight)])
        W, b = train(X, Y, nv, l2=cfg['l2'], prior=prior, sw=sw, fixed_bias=b0)
        if verbose_xb is not None:
            report(f'{label} round {rnd} kept {keep.sum()}', W, b, nv, verbose_xb)
    return W, b


def fsl_keep(name):
    if not name:
        return None
    meta = json.load(open(os.environ['FSL_META']))
    tags = [' '.join(meta[str(i)].get('tags', []) + [meta[str(i)]['name']]).lower() for i, _, _, _ in fsl]
    drum = np.array([any(w in t for w in ('drum', 'beat', 'perc')) for t in tags])
    if name == 'nodrum':
        return ~drum
    raise ValueError(name)


if __name__ == '__main__':
    for spec in sys.argv[1:]:
        kv = dict(x.split('=') for x in spec.split())
        cfg = parse(' '.join(f'{k}={v}' for k, v in kv.items() if k not in ('rounds', 'thr', 'w', 'out', 'nunl', 'fixbias', 'genre', 'soft', 'fslfilter')))
        nv = len(cfg['ranges'])
        xb = np.array([feature(r, cfg) for _, _, r, _ in beat])
        xf = np.array([feature(r, cfg) for _, _, r, _ in fsl])
        xu = np.array([feature(r, cfg) for _, _, r, _ in unl])
        if kv.get('genre') == 'dance':
            cands = {c['id']: c for c in json.load(open(os.environ['FMA_CANDS']))}
            DANCE = {'House', 'Techno', 'Dance', 'Trance', 'Dubstep', 'Drum & Bass', 'Breakbeat', 'Bass Music', 'Hardcore', 'Jungle', 'Minimal Electronic', 'Downtempo', 'Trip-Hop', 'Chill-out', 'IDM', 'Glitch', 'Electro-Punk'}
            sel = np.array([bool(set(cands[i]['genres']) & DANCE) and not set(cands[i]['genres']) & {'Pop', 'Rock', 'Chip Music', 'Chiptune', 'Folk', 'Soundtrack'} for i, _, _, _ in unl])
            xu = xu[sel]
            print('dance pool', sel.sum(), flush=True)
        if 'nunl' in kv:
            xu = xu[np.random.default_rng(0).permutation(len(xu))[:int(kv['nunl'])]]
        prior = make_prior(cfg['prior'], nv)
        W, b = selftrain(xf, xu, nv, cfg, prior, int(kv.get('rounds', 3)), float(kv.get('thr', 0)), float(kv.get('w', 1)), xb, spec, kv.get('fixbias') == '1', float(kv.get('soft', 0)), fsl_keep(kv.get('fslfilter')))
        if 'out' in kv:
            json.dump({'spec': spec, 'templates': W.tolist(), 'major_bias': float(b)}, open(kv['out'], 'w'), indent=1)
