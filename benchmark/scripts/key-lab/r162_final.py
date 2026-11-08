from r162 import *

c = dict(np.load(f'{CACHE}/cands.npz'))
PICK = 'seg27_wraw'
test = ~bdev
for k, label in (('fsl', 'clean-fsl (shipped)'), ('b14', 'clean-fsl-fma (b14)'), (PICK, 'r162 pick')):
    for name, m in (('test', test), ('dev', bdev), ('all', np.ones(len(bt), bool))):
        cc, n, mx = tally(c[k][m].argmax(1), bt[m])
        erf = cc['exact'] + cc['relative'] + cc['fifth']
        print(f'{label:22s} {name:4s} {fmt(cc, n, mx)} E+R+F {erf}={100*erf/n:.1f}%', flush=True)
