from r162 import *

cb = cached('cyan_beat', cyan_frames, beat); cf = cached('cyan_fsl', cyan_frames, fsl); cu = cached('cyan_unl', cyan_frames, unl)
xb, xf, xu = glob(cb, 3), glob(cf, 3), glob(cu, 3)
(W0, b0), (W1, b1) = student(xf, xu, 3, PRIOR3)
sA = lg(xb, W0, b0, 3); sB = lg(xb, W1, b1, 3)
line('clean-fsl (shipped)', sA.argmax(1))
line('clean-fsl-fma (b14)', sB.argmax(1))
np.save(f'{CACHE}/sA.npy', sA); np.save(f'{CACHE}/sB.npy', sB)
json.dump({'A': [W0.tolist(), b0], 'B': [W1.tolist(), b1]}, open(f'{CACHE}/AB.json', 'w'))


def biased(s, d):
    s = s.copy(); s[:, :12] += d; return s


grid = [round(d, 2) for d in np.arange(-1.0, 1.01, 0.1)]
for name, s in (('A', sA), ('B', sB)):
    for d in (-0.4, -0.2, 0.2, 0.4, 0.6):
        line(f'{name} major bias {d:+.1f}', biased(s, d).argmax(1))
    acc, n, picks, _ = dev_cv(lambda d: biased(s, d), grid)
    print(f'{name} bias by dev-CV: {acc}/{n} = {100*acc/n:.1f}% picks {picks}', flush=True)

kb = cached('kd_beat', kd_frames, beat); kf = cached('kd_fsl', kd_frames, fsl); ku = cached('kd_unl', kd_frames, unl)
zb, zf, zu = glob(kb, 2), glob(kf, 2), glob(ku, 2)
PRIOR2 = make_prior(BASE['prior'], 2)
for l2, pr in ((1e-3, PRIOR2), (1e-3, None), (1e-2, PRIOR2)):
    (V0, c0), _ = student(zf, zu, 2, pr, l2=l2)
    line(f'kd fsl l2 {l2} prior {pr is not None}', lg(zb, V0, c0, 2).argmax(1))
    _, (V2, c2) = student(zf, zu, 2, pr, teacher_logits=lg(xu, W0, b0, 3), l2=l2)
    tB = lg(zb, V2, c2, 2)
    line(f'kd student (cyan teacher) l2 {l2} prior {pr is not None}', tB.argmax(1))
    np.save(f'{CACHE}/kd_{l2}_{pr is not None}.npy', tB)
    for a in (0.3, 0.5, 0.7):
        line(f'  ens B*(1-a)+kdB*a a={a}', ((1 - a) * sB + a * tB).argmax(1))
