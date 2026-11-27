import sys
def load(path):
    d = {}
    for line in open(path):
        w = line.split()
        d[(w[1], w[2])] = [float(x) for x in w[3:]]
    return d
up = load(sys.argv[1]); pt = load(sys.argv[2])
for corr in sys.argv[3:]:
    print("corrector", corr)
    for k in up:
        if k[0] != corr: continue
        n = k[1]; a = up[k]; b = pt.get(k)
        if b is None: print(f"  {n:18s} MISSING in port"); continue
        if len(a) != len(b): print(f"  {n:18s} length {len(a)} vs {len(b)}"); continue
        scale = max(abs(x) for x in a) or 1.0
        worst = max(abs(x - y) for x, y in zip(a, b))
        nd = sum(1 for x, y in zip(a, b) if x != y)
        tag = "BIT-IDENTICAL" if worst == 0 else f"worst_rel {worst/scale:.2e} ({nd}/{len(a)} differ)"
        print(f"  {n:18s} {tag}")
