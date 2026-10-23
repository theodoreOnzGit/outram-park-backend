"""Apply the convergence rule fixed in PREDICTION.md to *_cycles.csv traces.
python3 -I conv.py trace.csv [inactive_used]"""
import csv, sys, math
from collections import defaultdict

rows = list(csv.DictReader(open(sys.argv[1])))
cases = defaultdict(list)
for r in rows:
    cases[(r['layers'], r['zt_cm'], r['rod'])].append((int(r['cycle']), int(r['active']), float(r['k']), float(r['entropy'])))
for key, v in cases.items():
    v.sort()
    H = [x[3] for x in v]
    K = [x[2] for x in v]
    n = len(H)
    tail = H[-40:]
    m = sum(tail) / len(tail)
    sd = math.sqrt(sum((h - m) ** 2 for h in tail) / (len(tail) - 1))
    ma = [sum(H[max(0, i - 4):i + 1]) / len(H[max(0, i - 4):i + 1]) for i in range(n)]
    c = None
    for i in range(n):
        if all(abs(ma[j] - m) <= 2 * sd for j in range(i, n)):
            c = i + 1
            break
    ninact = max(20, int(math.ceil(2 * c / 10.0) * 10)) if c else None
    act = [x for x in v if x[1] == 1]
    ha = [x[3] for x in act]
    h1, h2 = ha[: len(ha) // 2], ha[len(ha) // 2:]
    def ms(a):
        mu = sum(a) / len(a)
        s = math.sqrt(sum((x - mu) ** 2 for x in a) / (len(a) - 1))
        return mu, s / math.sqrt(len(a))
    (m1, e1), (m2, e2) = ms(h1), ms(h2)
    z = (m2 - m1) / math.sqrt(e1 * e1 + e2 * e2)
    ka = [x[2] for x in act]
    print(f"case layers={key[0]} zT={key[1]} rod={key[2]}: {n} cycles; tail-40 H = {m:.4f} sd {sd:.4f}; "
          f"converged at cycle {c} -> n_inactive {ninact}; active H halves {m1:.4f}+-{e1:.4f} vs {m2:.4f}+-{e2:.4f} (z={z:+.2f}); "
          f"active k mean {sum(ka)/len(ka):.5f}")
    print("   H by cycle:", " ".join(f"{h:.3f}" for h in H))
