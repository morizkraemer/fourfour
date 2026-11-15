from lab import *

def mk(power=1.0, W=6, med=0, frame_norm=False, energy_a=0.0, bias=MAJOR_BIAS, peak=False, drop_q=0.0, log_g=0.0):
    M, b = tmat(bias=bias)
    def f(r):
        rr = r
        if med > 1:
            pad = med // 2
            p = np.pad(rr, ((pad, pad), (0, 0)), mode='edge')
            st = np.stack([p[i:i + rr.shape[0]] for i in range(med)])
            rr = np.median(st, axis=0)
        w = whiten(rr, W)
        if peak:
            l = np.pad(w, ((0, 0), (1, 1)), mode='edge')
            mask = (w >= l[:, :-2]) & (w >= l[:, 2:])
            w = w * mask
        if log_g > 0:
            w = np.log1p(log_g * w)
        if power != 1.0:
            w = w ** power
        c = fold(w)
        weights = np.ones(len(c))
        if drop_q > 0:
            e = r.sum(1)
            weights = (e >= np.quantile(e, drop_q)).astype(float)
        if frame_norm:
            cn = norm3(c)
            e = r.sum(1)
            weights = weights * (e / (e.max() + 1e-12)) ** energy_a
            x = (cn * weights[:, None]).sum(0)
        else:
            x = (c * weights[:, None]).sum(0)
        return int(np.argmax(norm3(x) @ M + b))
    return f

configs = [('base', {})]
configs += [(f'power {p}', {'power': p}) for p in (0.5, 0.75, 1.5, 2.0)]
configs += [(f'log g {g}', {'log_g': g}) for g in (1.0, 10.0)]
configs += [(f'W {w}', {'W': w}) for w in (3, 4, 8, 10)]
configs += [(f'median {k}', {'med': k}) for k in (3, 5, 9)]
configs += [(f'frame norm a {a}', {'frame_norm': True, 'energy_a': a}) for a in (0.0, 0.5, 1.0)]
configs += [(f'peak mask', {'peak': True})]
configs += [(f'drop quiet {q}', {'drop_q': q}) for q in (0.1, 0.25)]
configs += [(f'bias {b}', {'bias': b}) for b in (0.0, 0.2, 0.3, 0.6, 0.8, 1.0)]
for name, kw in configs:
    evaluate(mk(**kw), 'dev', label=name)
