from lab import *

PREP = {}
for i, t, r in DATA:
    w = whiten(r)
    PREP[i] = {p: fold(w ** p).sum(0) for p in (0.75, 1.0)}

MAJ = [4, 9, 11]
MIN = [3, 8, 10]

def report(fn, label):
    c = Counter(); mm = Counter(); halves = [0, 0]
    for i, t, r in DATA:
        if i % 3 == 0: continue
        p = fn(i, t)
        b = bucket(p, t)
        c[b] += 1
        if b == 'exact': halves[i % 3 - 1] += 1
        mm[(t >= 12, p >= 12)] += 1
    print(f"{label:48s} dev={c['exact']} ({100*c['exact']/399:.1f}%) halves={halves} fifth={c['fifth']} rel={c['relative']} par={c['parallel']} other={c['other']} | maj->min {mm[(False,True)]} min->maj {mm[(True,False)]}", flush=True)

M, B = tmat()
def base(power=1.0, bias=MAJOR_BIAS):
    M, B = tmat(bias=bias)
    return lambda i, t: int(np.argmax(norm3(PREP[i][power]) @ M + B))

def oracle_mode(power):
    def f(i, t):
        p = int(np.argmax(norm3(PREP[i][power]) @ M + B))
        return (p % 12) + (12 if t >= 12 else 0)
    return f

def third_rule(power, bands=(1, 2), margin=1e9, w=1.0, bias=MAJOR_BIAS):
    M, B = tmat(bias=bias)
    def f(i, t):
        x = norm3(PREP[i][power])
        s = x @ M + B
        p = int(np.argmax(s))
        tonic = p % 12
        d = s[tonic] - s[tonic + 12]
        if abs(d) > margin:
            return p
        if p >= 12 and s[tonic] < s.max() - margin:
            return p
        c = sum(x[12 * v:12 * v + 12] for v in bands)
        maj = sum(c[(tonic + k) % 12] for k in MAJ)
        mi = sum(c[(tonic + k) % 12] for k in MIN)
        return tonic + (0 if maj * w > mi else 12)
    return f

report(base(), 'base')
report(base(0.75), 'power 0.75')
report(oracle_mode(1.0), 'oracle mode (ceiling)')
report(oracle_mode(0.75), 'oracle mode p0.75 (ceiling)')
for bands in ((1,), (2,), (1, 2), (0, 1, 2)):
    report(third_rule(1.0, bands), f'third rule always bands {bands}')
for m in (0.25, 0.5, 1.0):
    for w in (1.0, 1.05, 1.1):
        report(third_rule(1.0, (1, 2), margin=m, w=w), f'third rule margin {m} w {w}')
