"""Statistics of the paired one-step test (paired_step.py output), gh:#1667.

Usage: python paired_stats.py <pair .tsv> [<pair .tsv> ...]
Pools the pairs of all files.  Reports: the unusable share (quality -inf, a negative cross section) of each eigenvalue set;
McNemar's exact test on the pairs where exactly one set is unusable; among pairs where both are usable, the median of
q_port - q_dgeev and the exact sign test; the number of pairs above 1.4 and 1.5 for each set and, where exactly one
set exceeds 1.4, which one; the median of the largest distance of a DGEEV pole to the nearest port pole.
"""
import sys
from math import comb

import numpy as np

q_l, q_p, dist = [], [], []
for path in sys.argv[1:]:
    for line in open(path).read().splitlines()[1:]:
        c = line.split("\t")
        q_l.append(float(c[2]))
        q_p.append(float(c[3]))
        dist.append(float(c[6]))
q_l, q_p, dist = np.array(q_l), np.array(q_p), np.array(dist)
n = q_l.size


def binom_two_sided(k, m):
    """exact two-sided binomial p for k successes of m at p = 1/2"""
    if m == 0:
        return float("nan")
    pk = comb(m, k)
    return min(1.0, sum(comb(m, j) for j in range(m + 1) if comb(m, j) <= pk) / 2**m)


ul, up = np.isneginf(q_l), np.isneginf(q_p)
print(f"pairs {n}; unusable: DGEEV {ul.mean() * 100:.1f} %, port {up.mean() * 100:.1f} %")
only_l = int((~ul & up).sum())   # DGEEV usable, port not
only_p = int((ul & ~up).sum())
print(f"usability discordant pairs: DGEEV usable only {only_l}, port usable only {only_p}; McNemar exact two-sided p = "
      f"{binom_two_sided(only_p, only_l + only_p):.3g}")
both = ~ul & ~up
d = q_p[both] - q_l[both]
pos, neg = int((d > 0).sum()), int((d < 0).sum())
print(f"both usable {int(both.sum())}: median q_port - q_dgeev = {np.median(d):+.4f}; port higher {pos}, lower {neg}; "
      f"sign test p = {binom_two_sided(pos, pos + neg):.3g}")
for thr in (1.4, 1.5):
    a, b = int((q_l > thr).sum()), int((q_p > thr).sum())
    ol = int(((q_l > thr) & ~(q_p > thr)).sum())
    op = int(((q_p > thr) & ~(q_l > thr)).sum())
    print(f"quality > {thr}: DGEEV {a}, port {b}; only DGEEV {ol}, only port {op}; exact two-sided p = {binom_two_sided(op, ol + op):.3g}")
print(f"largest distance of a DGEEV pole to the nearest port pole: median {np.median(dist):.3g}, 90th percentile {np.percentile(dist, 90):.3g}")
