from r162 import *

c = dict(np.load(f'{CACHE}/cands.npz'))
names = sorted(c)
masks = [('dev', bdev)] + ([('all', np.ones(len(bt), bool))] if SHOW_TEST else [])
print('# selection among clean candidates by inner CV (pick the variant on 4/5, score the held-out 1/5)')
for mask_name, mask in masks:
    acc, n, picks, pred = dev_cv(lambda k: c[k], names, mask=mask)
    idx = np.where(mask)[0]
    print(f'  {mask_name}: {fmt(*tally(pred[idx], bt[idx]))} picks {picks}', flush=True)
