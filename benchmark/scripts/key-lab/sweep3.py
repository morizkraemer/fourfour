from lab import *

KK = np.array([[6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88],
               [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17]])
TEMP = np.array([[5.0, 2.0, 3.5, 2.0, 4.5, 4.0, 2.0, 4.5, 2.0, 3.5, 1.5, 4.0],
                 [5.0, 2.0, 3.5, 4.5, 2.0, 4.0, 2.0, 4.5, 3.5, 2.0, 1.5, 4.0]])

def profile_corr(c12, prof):
    out = np.zeros(24)
    for mode in range(2):
        p = prof[mode] - prof[mode].mean()
        for k in range(12):
            v = c12[ROLL[k]]
            v = v - v.mean()
            d = np.linalg.norm(v) * np.linalg.norm(p)
            out[12 * mode + k] = (v @ p) / d if d > 0 else 0
    return out

PREP = {}
for i, t, r in DATA:
    w = whiten(r)
    PREP[i] = {p: fold(w ** p) for p in (0.75, 0.8, 1.0)}

def split_halves(predict, label):
    c = [Counter(), Counter()]
    mm = Counter()
    for i, t, r in DATA:
        if i % 3 == 0: continue
        p = predict(i)
        c[i % 3 - 1][bucket(p, t)] += 1
        mm[(t >= 12, p >= 12)] += 1
    e = [x['exact'] for x in c]
    tot = c[0] + c[1]
    print(f"{label:44s} dev={sum(e)} ({100*sum(e)/399:.1f}%) halves={e} fifth={tot['fifth']} rel={tot['relative']} par={tot['parallel']} other={tot['other']} | maj->maj {mm[(False,False)]} maj->min {mm[(False,True)]} min->maj {mm[(True,False)]} min->min {mm[(True,True)]}", flush=True)

def mk(power=1.0, bias=MAJOR_BIAS, seg=0, mix=0.0, prof=TEMP, frame_norm=False, trim=0.0):
    M, b = tmat(bias=bias)
    def f(i):
        c = PREP[i][power]
        n = len(c)
        if trim > 0:
            k = int(n * trim)
            c = c[k:n - k] if n - 2 * k > 10 else c
        if frame_norm:
            c = norm3(c)
        x = norm3(c.sum(0))
        s = x @ M + b
        if mix > 0:
            s = s + mix * profile_corr(x[:12] + x[12:24] + x[24:], prof)
        if seg > 0:
            votes = np.zeros(24)
            for f0 in range(0, len(c), seg):
                xs = norm3(c[f0:f0 + seg].sum(0))
                ss = xs @ M + b
                e = np.exp(0.9349 * (ss - ss.max()))
                votes += e / e.sum()
            s = s + 0.0
            return int(np.argmax(votes))
        return int(np.argmax(s))
    return f

split_halves(mk(), 'base')
split_halves(mk(0.75), 'power 0.75')
split_halves(mk(0.8, 0.3), 'power 0.8 bias 0.3')
split_halves(mk(1.0, 0.0), 'power 1 bias 0')
for seg in (27, 54, 81):
    split_halves(mk(0.75, seg=seg), f'power 0.75 segvote {seg}')
for mix in (0.5, 1.0, 2.0):
    split_halves(mk(0.75, mix=mix), f'power 0.75 temperley mix {mix}')
    split_halves(mk(0.75, mix=mix, prof=KK), f'power 0.75 krumhansl mix {mix}')
split_halves(mk(0.75, frame_norm=True), 'power 0.75 frame norm')
for tr in (0.05, 0.1):
    split_halves(mk(0.75, trim=tr), f'power 0.75 trim {tr}')
