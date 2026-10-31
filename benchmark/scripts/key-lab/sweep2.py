from lab import *

cache = {}
def chroma_sum(i, r, power, W=6):
    key = (i, power, W)
    if key not in cache:
        w = whiten(r, W)
        cache[key] = fold(w ** power).sum(0)
    return cache[key]

def run(power, bias, W=6, split='dev', label=None):
    M, b = tmat(bias=bias)
    c = Counter(); n = 0
    for i, t, r in DATA:
        if not in_split(i, split): continue
        x = norm3(chroma_sum(i, r, power, W))
        p = int(np.argmax(x @ M + b))
        c[bucket(p, t)] += 1; n += 1
    print(f"{label or f'power {power} bias {bias} W {W}':44s} n={n} exact={c['exact']} ({100*c['exact']/n:.1f}%) fifth={c['fifth']} rel={c['relative']} par={c['parallel']} other={c['other']}", flush=True)
    return c['exact']

grid = {}
for p in (0.4, 0.5, 0.6, 0.7, 0.75, 0.8, 0.9, 1.0):
    for b in (0.0, 0.1, 0.2, 0.3, 0.442):
        grid[(p, b)] = run(p, b)
best = max(grid, key=grid.get)
print('best', best, grid[best])
