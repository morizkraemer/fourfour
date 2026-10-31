import sys
from lab import *
from profiles import PROFILES, templates

COMP = float(os.environ.get('KEY_COMPRESSION', '0.75'))
X = {i: norm3(fold(whiten(r) ** COMP).sum(0)) for i, t, r in DATA}


def score(M, b, label, split):
    from collections import Counter
    c = Counter(); n = 0
    for i, t, r in DATA:
        if not in_split(i, split):
            continue
        p = int(np.argmax(X[i] @ M + b))
        c[bucket(p, t)] += 1; n += 1
    mirex = (c['exact'] + 0.5 * c['fifth'] + 0.3 * c['relative'] + 0.2 * c['parallel']) / n * 100
    return c, n, mirex


def line(label, tmpl, bias):
    M, b = tmat(tmpl, bias)
    out = [label]
    for split in ('dev', 'test'):
        c, n, mx = score(M, b, label, split)
        out.append(f"{split} {c['exact']}/{n}={100*c['exact']/n:.1f}% f{c['fifth']} r{c['relative']} p{c['parallel']} o{c['other']} M{mx:.1f}")
    print(' | '.join(out), flush=True)


if __name__ == '__main__':
    line('shipped cyanbox', T, MAJOR_BIAS)
    for name in PROFILES:
        for bw in [(1, 1, 1), (0, 1, 1), (0.5, 1, 1), (1, 1, 0.5)]:
            line(f'{name} bands={bw}', templates(name, bw), 0.0)
