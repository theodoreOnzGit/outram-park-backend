import sys, math
# usage: cmp.py up.txt port.txt  : per (corr, name) the worst |a-b| / max|a| (and absolute), in dump order
def load(p):
    d = {}
    order = []
    for line in open(p):
        t = line.split()
        k = (int(t[1]), t[2])
        if k not in d:
            order.append(k)
        d[k] = [float(x) for x in t[3:]]
    return d, order
u, order = load(sys.argv[1])
q, _ = load(sys.argv[2])
thr = float(sys.argv[3]) if len(sys.argv) > 3 else 0.0
for k in order:
    if k not in q:
        print(k, "missing in port")
        continue
    a, b = u[k], q[k]
    if len(a) != len(b):
        print(k, "length", len(a), len(b))
        continue
    m = max((abs(x) for x in a), default=0.0)
    w = max((abs(x - y) for x, y in zip(a, b)), default=0.0)
    rel = w / m if m > 0 else w
    if rel >= thr:
        print("%d %-16s n=%-5d max|up|=%.3e worst abs=%.3e rel=%.2e" % (k[0], k[1], len(a), m, w, rel))
