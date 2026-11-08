from r162 import *

xb = glob([cyan_frames(r) for _, _, r, _ in beat], 3)
ab = json.load(open(f'{CACHE}/AB.json'))
seg = json.load(open(f'{CACHE}/student_seg27_wraw.json'))
print(f'Rekordbox library set, n={len(bt)}, truth = Rekordbox key; minor share {np.mean(bt >= 12):.2f}')
for label, (W, b) in (('clean-fsl (shipped)', ab['A']), ('clean-fsl-fma (b14)', ab['B']), ('r162 pick', seg)):
    p = lg(xb, np.array(W), b, 3).argmax(1)
    cc, n, mx = tally(p, bt)
    erf = cc['exact'] + cc['relative'] + cc['fifth']
    print(f'{label:22s} agree {fmt(cc, n, mx)} E+R+F {100*erf/n:.1f}% predicted minor {np.mean(p >= 12):.2f}', flush=True)
