from r162 import *

cb = cached('cyan_beat', cyan_frames, beat); cf = cached('cyan_fsl', cyan_frames, fsl); cu = cached('cyan_unl', cyan_frames, unl)
kb = cached('kd_beat', kd_frames, beat)
xb, xf, xu = glob(cb, 3), glob(cf, 3), glob(cu, 3)
zb = glob(kb, 2)
W0, b0 = train(xf, ft, 3, prior=PRIOR3)
sB = np.load(f'{CACHE}/sB.npy')

print('# reference, NOT licence-clean: models fit on Beatport labels (CC BY-SA), 5-fold CV', flush=True)


def beat_cv(x, nv, mask, l2=1e-3):
    idx = np.where(mask)[0]
    fold = (bid[idx] // 3) % 5
    out = np.zeros((len(bt), 24))
    for k in range(5):
        tr, te = idx[fold != k], idx[fold == k]
        W, b = train(x[tr], bt[tr], nv, l2=l2)
        out[te] = lg(x[te], W, b, nv)
    return out


for mask_name, mask in (('dev', bdev), ('all', np.ones(len(bt), bool))):
    kcv = beat_cv(zb, 2, mask)
    ccv = beat_cv(xb, 3, mask)
    idx = np.where(mask)[0]
    for name, s in (('keydet-like 2-band, Beatport-fit', kcv), ('cyan 3-band, Beatport-fit', ccv)):
        print(f'  {mask_name} {name}: {fmt(*tally(s[idx].argmax(1), bt[idx]))}', flush=True)
    for name, s in (('kd', kcv), ('cyan', ccv)):
        grid = [0.0, 0.2, 0.35, 0.5, 0.65, 0.8, 1.0]
        acc, n, picks, pred = dev_cv(lambda a: (1 - a) * sB + a * s, grid, mask=mask)
        print(f'  {mask_name} clean B + {name} Beatport-fit, weight by inner CV: {fmt(*tally(pred[idx], bt[idx]))} picks {picks}', flush=True)

print('# clean candidates', flush=True)
res = {}
for seg in (14, 27, 54):
    su = np.concatenate(segs(cu, 3, seg))
    ps = lg(su, W0, b0, 3).argmax(1)
    for sw in ('raw', 1.0, 0.5):
        per = 1.0 if sw == 'raw' else sw * len(cu) / len(su)
        w_ = np.concatenate([np.ones(len(ft)), np.full(len(su), per)])
        W, b = train(np.concatenate([xf, su]), np.concatenate([ft, ps]), 3, prior=PRIOR3, fixed_bias=b0, sw=w_)
        s = lg(xb, W, b, 3)
        res[f'seg{seg}_w{sw}'] = s
        line(f'student on {seg}-frame segments, unl weight {sw}', s.argmax(1))
        json.dump([W.tolist(), b], open(f'{CACHE}/student_seg{seg}_w{sw}.json', 'w'))
res['b14'] = sB
res['fsl'] = np.load(f'{CACHE}/sA.npy')
np.savez(f'{CACHE}/cands.npz', **res)
