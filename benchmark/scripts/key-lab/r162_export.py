from r162 import *
from clean import fit_cal, softmax_best

cf = cached('cyan_fsl', cyan_frames, fsl); cu = cached('cyan_unl', cyan_frames, unl); cb = cached('cyan_beat', cyan_frames, beat)
xf, xb = glob(cf, 3), glob(cb, 3)
su = np.concatenate(segs(cu, 3, 27))
folds = np.array([f for _, _, _, f in fsl])


def recipe(xf_, ft_):
    W0, b0 = train(xf_, ft_, 3, prior=PRIOR3)
    ps = lg(su, W0, b0, 3).argmax(1)
    return train(np.concatenate([xf_, su]), np.concatenate([ft_, ps]), 3, prior=PRIOR3, fixed_bias=b0)


W, b = recipe(xf, ft)
ref = json.load(open(f'{CACHE}/student_seg27_wraw.json'))
assert np.allclose(W, ref[0]) and np.isclose(b, ref[1])
oof = np.zeros((len(ft), 24))
for k in range(5):
    tr = folds != k
    Wk, bk = recipe(xf[tr], ft[tr])
    oof[~tr] = lg(xf[~tr], Wk, bk, 3)
pb = np.clip(softmax_best(oof), 1e-6, 1 - 1e-6)
a, c = fit_cal(np.log(pb / (1 - pb)), (oof.argmax(1) == ft).astype(float))
print(f'fsl oof exact {(oof.argmax(1) == ft).mean():.3f} cal A={a:.5f} C={c:.5f}')
for name, mask in (('dev', bdev), ('test', ~bdev)):
    s = lg(xb[mask], W, b, 3)
    q = np.clip(softmax_best(s), 1e-6, 1 - 1e-6)
    conf = 1 / (1 + np.exp(-(a * np.log(q / (1 - q)) + c)))
    ok = (s.argmax(1) == bt[mask]).astype(float)
    print(f'  {name}: mean confidence {conf.mean():.3f} vs accuracy {ok.mean():.3f}, Brier {((conf - ok) ** 2).mean():.3f}')
rows = '\n'.join('    [\n' + '\n'.join('        ' + ', '.join(f'{v:.6f}' for v in W[m, 12 * v0:12 * v0 + 12]) + ',' for v0 in range(3)) + '\n    ],' for m in range(2))
print(f'templates: [\n{rows}\n]\nmajor_bias: {b:.6f}\ncal: [{a:.5f}, 0.0, {c:.5f}]')
pred = lg(xb, W, b, 3).argmax(1)
with open(f'{CACHE}/rows-lab.txt', 'w') as f:
    for i, t, p in zip(bid, bt, pred):
        f.write(f'{i} {t} {p}\n')
