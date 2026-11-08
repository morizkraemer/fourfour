from r162 import *

cb = cached('cyan_beat', cyan_frames, beat); cf = cached('cyan_fsl', cyan_frames, fsl); cu = cached('cyan_unl', cyan_frames, unl)
xb, xf, xu = glob(cb, 3), glob(cf, 3), glob(cu, 3)
W0, b0 = train(xf, ft, 3, prior=PRIOR3)
tu = lg(xu, W0, b0, 3)
pu = probs(tu)
conf = pu.max(1)
pseudo = tu.argmax(1)
print('pseudo minor share', (pseudo >= 12).mean(), 'fsl minor share', (ft >= 12).mean(), flush=True)


def fit_student(keep, labels=None, sw_unl=1.0, soft=None):
    X = np.concatenate([xf, xu[keep]])
    if soft is not None:
        F1 = np.zeros((len(ft), 24)); F1[np.arange(len(ft)), ft] = 1
        Y = np.concatenate([F1, probs(tu[keep] * soft)])
    else:
        Y = np.concatenate([ft, (pseudo if labels is None else labels)[keep]])
    sw = np.concatenate([np.ones(len(ft)), np.full(keep.sum(), sw_unl)])
    return train(X, Y, 3, prior=PRIOR3, sw=sw, fixed_bias=b0)


rng = np.random.default_rng(0)
order = rng.permutation(len(xu))
for n in (500, 1000, 1500):
    keep = np.zeros(len(xu), bool); keep[order[:n]] = True
    W, b = fit_student(keep)
    line(f'student nunl={n}', lg(xb, W, b, 3).argmax(1))
for q in (0.3, 0.5, 0.7):
    keep = conf >= np.quantile(conf, q)
    W, b = fit_student(keep)
    line(f'student keep top {100*(1-q):.0f}% confidence', lg(xb, W, b, 3).argmax(1))
for t in (0.5, 1.0, 2.0):
    W, b = fit_student(np.ones(len(xu), bool), soft=t)
    line(f'student soft labels temp x{t}', lg(xb, W, b, 3).argmax(1))
for w in (0.5, 2.0):
    W, b = fit_student(np.ones(len(xu), bool), sw_unl=w)
    line(f'student unl weight {w}', lg(xb, W, b, 3).argmax(1))

su = segs(cu, 3, 27)
seg_votes = np.array([lg(s, W0, b0, 3).argmax(1) for s in su], dtype=object)
agree = np.array([np.mean(v == p) for v, p in zip(seg_votes, pseudo)])
keep = agree >= 1.0
print('segments agree on', keep.sum(), flush=True)
W, b = fit_student(keep)
line('student only clips whose 10 s halves agree', lg(xb, W, b, 3).argmax(1))

xu_seg = np.concatenate(su)
ps = lg(xu_seg, W0, b0, 3).argmax(1)
X = np.concatenate([xf, xu_seg]); Y = np.concatenate([ft, ps])
W, b = train(X, Y, 3, prior=PRIOR3, fixed_bias=b0)
line('student on 10 s segments as examples', lg(xb, W, b, 3).argmax(1))

Wb, bb = fit_student(np.ones(len(xu), bool))
line('student (b14 recipe)', lg(xb, Wb, bb, 3).argmax(1))
sb = segs(cb, 3)
for wexp in (0.0, 1.0):
    pred = []
    for f, s in zip(cb, sb):
        n = len(s)
        edges = np.linspace(0, len(f), n + 1).astype(int)
        e = np.array([f[a:c].sum() for a, c in zip(edges[:-1], edges[1:])]) ** wexp
        pred.append((probs(lg(s, Wb, bb, 3)) * e[:, None]).sum(0).argmax())
    line(f'student, 20 s segment voting energy^{wexp}', np.array(pred))
