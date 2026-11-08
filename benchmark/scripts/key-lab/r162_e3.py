from r162 import *

cb = cached('cyan_beat', cyan_frames, beat); cf = cached('cyan_fsl', cyan_frames, fsl); cu = cached('cyan_unl', cyan_frames, unl)
kb = cached('kd_beat', kd_frames, beat); kf = cached('kd_fsl', kd_frames, fsl); ku = cached('kd_unl', kd_frames, unl)
xb, xf, xu = glob(cb, 3), glob(cf, 3), glob(cu, 3)
zb, zf, zu = glob(kb, 2), glob(kf, 2), glob(ku, 2)
PRIOR2 = make_prior(BASE['prior'], 2)
folds = np.array([f for _, _, _, f in fsl])

W0, b0 = train(xf, ft, 3, prior=PRIOR3)
V0, c0 = train(zf, ft, 2, prior=PRIOR2)
ta, tk = lg(xu, W0, b0, 3), lg(zu, V0, c0, 2)
pa, pk = ta.argmax(1), tk.argmax(1)
agree = pa == pk
print('co-training: teachers agree on', agree.sum(), 'of', len(pa), flush=True)
for name, keep, lab in (('agree only', agree, pa), ('all, label=sum of teachers', np.ones(len(pa), bool), (ta + tk).argmax(1))):
    W, b = train(np.concatenate([xf, xu[keep]]), np.concatenate([ft, lab[keep]]), 3, prior=PRIOR3, fixed_bias=b0)
    V, c = train(np.concatenate([zf, zu[keep]]), np.concatenate([ft, lab[keep]]), 2, prior=PRIOR2, fixed_bias=c0)
    sa, sk = lg(xb, W, b, 3), lg(zb, V, c, 2)
    line(f'co-train {name}: cyan student', sa.argmax(1))
    line(f'co-train {name}: kd student', sk.argmax(1))
    for a in (0.2, 0.35, 0.5):
        line(f'  ensemble a={a}', ((1 - a) * sa + a * sk).argmax(1))

xr = np.concatenate([xf, zf], 1)
xur = np.concatenate([xu, zu], 1)
xbr = np.concatenate([xb, zb], 1)
P5 = np.concatenate([PRIOR3, PRIOR2], 1)
J0, j0 = train(xr, ft, 5, prior=P5)
line('joint 5-band linear (fsl)', lg(xbr, J0, j0, 5).argmax(1))
J, j = train(np.concatenate([xr, xur]), np.concatenate([ft, lg(xur, J0, j0, 5).argmax(1)]), 5, prior=P5, fixed_bias=j0)
line('joint 5-band linear student', lg(xbr, J, j, 5).argmax(1))

oof = np.zeros((len(ft), 24))
for k in range(5):
    tr = folds != k
    Wk, bk = train(xf[tr], ft[tr], 3, prior=PRIOR3)
    oof[~tr] = lg(xf[~tr], Wk, bk, 3)
REL = lambda c: (c % 12 + 9) % 12 + 12 if c < 12 else (c % 12 + 3) % 12
PAR = lambda c: c + 12 if c < 12 else c - 12


def mode_feats(x, s, nv=3):
    xr = rotations(x, nv)
    out = []
    for i, c in enumerate(s.argmax(1)):
        cands = [c, REL(c), PAR(c)]
        row = []
        for d in cands:
            minor = d >= 12
            v = xr[i, d % 12]
            row.append(np.concatenate([v, [s[i, d], float(minor)]]))
        out.append(row)
    return np.array(out), [[c, REL(c), PAR(c)] for c in s.argmax(1)]


def rerank_train(F, cands, y, l2=1e-2, epochs=400, lr=0.05):
    d = F.shape[2]
    w = np.zeros(d); w[-2] = 1.0
    T = np.array([[1.0 if c == t else 0.0 for c in cs] for cs, t in zip(cands, y)])
    ok = T.sum(1) > 0
    F, T = F[ok], T[ok]
    m, v = np.zeros(d), np.zeros(d)
    for t in range(1, epochs + 1):
        z = F @ w
        z -= z.max(1, keepdims=True)
        p = np.exp(z); p /= p.sum(1, keepdims=True)
        g = np.einsum('nc,ncd->d', p - T, F) / len(F) + l2 * w
        m = 0.9 * m + 0.1 * g; v = 0.999 * v + 0.001 * g * g
        w -= lr * (m / (1 - 0.9 ** t)) / (np.sqrt(v / (1 - 0.999 ** t)) + 1e-8)
    return w


Fo, co = mode_feats(xf, oof)
Wb, bb = train(np.concatenate([xf, xu]), np.concatenate([ft, ta.argmax(1)]), 3, prior=PRIOR3, fixed_bias=b0)
sb = lg(xb, Wb, bb, 3)
for l2 in (1e-3, 1e-2, 1e-1):
    w = rerank_train(Fo, co, ft, l2=l2)
    for name, s in (('A', lg(xb, W0, b0, 3)), ('B', sb)):
        Fb, cbb = mode_feats(xb, s)
        pick = (Fb @ w).argmax(1)
        line(f'mode reranker (fsl oof) l2={l2} on {name}', np.array([c[k] for c, k in zip(cbb, pick)]))
