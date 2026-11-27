import sys

def load(path):
    d = {}
    for line in open(path):
        w = line.split()
        d[(w[1], w[2])] = [float(x) for x in w[3:]]
    return d

up = load(sys.argv[1])
pt = load(sys.argv[2])
corr = sys.argv[3] if len(sys.argv) > 3 else "1"
names = []
for k in up:
    if k[0] == corr and k not in [(corr, n) for n in []]:
        names.append(k[1])
for n in names:
    a = up[(corr, n)]
    b = pt.get((corr, n))
    if b is None:
        print(f"{n:18s} MISSING in port")
        continue
    if len(a) != len(b):
        print(f"{n:18s} length {len(a)} vs {len(b)}")
        continue
    scale = max(abs(x) for x in a) or 1.0
    worst = max(abs(x - y) for x, y in zip(a, b))
    iw = max(range(len(a)), key=lambda i: abs(a[i] - b[i]))
    print(f"{n:18s} scale {scale:.3e} worst_rel {worst/scale:.2e} at idx {iw} up={a[iw]:.15e} port={b[iw]:.15e}")
