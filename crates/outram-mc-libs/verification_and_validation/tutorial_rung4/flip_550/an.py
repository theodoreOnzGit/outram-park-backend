import math, sys
d = {}
for l in open(sys.argv[1]):
    if l.startswith("SEED"):
        _, p, s, k, e = l.split()
        d.setdefault(p, []).append(float(k))
old = {"2.4": (0.70536, 0.00027), "1.63576": (1.06216, 0.00031), "1.75": (1.01104, 0.00016)}
oc = {"2.4": (0.70653, 0.00029), "1.63576": (1.06253, 0.00022), "1.75": (1.01104, 0.00033)}
for p, v in d.items():
    n = len(v)
    m = sum(v) / n
    sd = math.sqrt(sum((x - m) ** 2 for x in v) / (n - 1))
    sem = sd / math.sqrt(n)
    o, os_ = old[p]
    dd = (m - o) * 1e5
    ds = math.hypot(sem, os_) * 1e5
    om, oms = oc[p]
    d2 = (m - om) * 1e5
    ds2 = math.hypot(sem, oms) * 1e5
    print(p, n, "%.5f sd %.0f sem %.0f" % (m, sd * 1e5, sem * 1e5),
          "move %+.0f +- %.0f z %+.2f" % (dd, ds, dd / ds),
          "| vs OpenMC %+.0f +- %.0f z %+.2f" % (d2, ds2, d2 / ds2))
