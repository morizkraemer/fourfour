from lab import *
def base(r):
    x = norm3(fold(whiten(r)).sum(0))
    return int(np.argmax(scores(x)))
_, p = evaluate(base, 'all', label='base all')
evaluate(base, 'dev', label='base dev')
if len(sys.argv) > 1:
    rust = {int(l.split()[0]): int(l.split()[2]) for l in open(sys.argv[1])}
    print('mismatch vs rust', sum(1 for k in p if p[k] != rust[k]))
