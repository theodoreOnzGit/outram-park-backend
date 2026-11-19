"""Per-iteration quality statistics of order-search traces (gh:#1667, 2026-11-19).

Usage: python tail_stats.py <label>=<prefix>[,<prefix>...] ... [--orders 98:130]
Each prefix names a trace written by trace_openmc.py / the port trace (<prefix>.log.tsv, 30 iterations per
order, order = 98 + 2 * (row // 30), which is valid for both codes: the OpenMC `order` column is the size of
`new_poles`, see README "Step 2").  Rows of orders outside --orders are dropped, so runs with a different
stopping rule compare on the orders they share.  For each label (pooled over its prefixes) prints:
iterations, share unusable (-inf), counts of iterations with quality > 1.4, 1.5, 1.6, the number of complete
orders and the min / median / max of the per-order best quality.  Labels are compared pairwise by an exact
one-sided Fisher test on the count of iterations with quality > 1.5 (the label listed first is the one
expected to have the larger tail) and by a Mann-Whitney test on the per-order best quality.
"""
import sys
from math import comb, erf, sqrt

import numpy as np


def rows_of(prefix, lo, hi, first=98):
    lines = open(prefix + ".log.tsv").read().splitlines()
    h = lines[0].split("\t")
    qi = h.index("quality")
    out = []
    for k, line in enumerate(lines[1:]):
        o = first + 2 * (k // 30)
        if lo <= o <= hi:
            out.append((o, float(line.split("\t")[qi])))
    return out


def fisher_upper(a_hit, a_n, b_hit, b_n):
    """P(hits in A >= a_hit) under the hypergeometric with the pooled hit count."""
    k = a_hit + b_hit
    n = a_n + b_n
    tot = comb(n, a_n)
    p = 0
    for x in range(a_hit, min(k, a_n) + 1):
        p += comb(k, x) * comb(n - k, a_n - x)
    return p / tot


def mwu(a, b):
    a, b = np.asarray(a, float), np.asarray(b, float)
    allv = np.concatenate([a, b])
    order = allv.argsort(kind="mergesort")
    ranks = np.empty(allv.size)
    sv = allv[order]
    i = 0
    while i < sv.size:
        j = i
        while j + 1 < sv.size and sv[j + 1] == sv[i]:
            j += 1
        ranks[order[i:j + 1]] = (i + j) / 2 + 1
        i = j + 1
    ua = ranks[:a.size].sum() - a.size * (a.size + 1) / 2
    n1, n2 = a.size, b.size
    mu = n1 * n2 / 2
    _, cnt = np.unique(allv, return_counts=True)
    sd = sqrt(n1 * n2 / 12 * ((n1 + n2 + 1) - (cnt**3 - cnt).sum() / ((n1 + n2) * (n1 + n2 - 1))))
    z = (ua - mu) / sd
    return z, 1 - erf(abs(z) / sqrt(2))


def main():
    args = sys.argv[1:]
    lo, hi = 98, 130
    if "--orders" in args:
        i = args.index("--orders")
        lo, hi = [int(x) for x in args[i + 1].split(":")]
        del args[i:i + 2]
    sets = {}
    for a in args:
        lab, pre = a.split("=", 1)
        rows = []
        bests = []
        for p in pre.split(","):
            r = rows_of(p, lo, hi)
            rows += r
            for o in sorted(set(x[0] for x in r)):
                qs = [v for (oo, v) in r if oo == o]
                if len(qs) == 30:
                    bests.append(max(qs))
        sets[lab] = (np.array([v for _, v in rows]), np.array(bests))
    print(f"orders {lo}..{hi}")
    print(f"{'label':26s} {'iters':>6s} {'unusable':>8s} {'>1.4':>5s} {'>1.5':>5s} {'>1.6':>5s} {'orders':>6s}   best q per order: min / median / max")
    for lab, (q, b) in sets.items():
        print(f"{lab:26s} {q.size:6d} {np.mean(np.isneginf(q)):8.3f} {int((q > 1.4).sum()):5d} {int((q > 1.5).sum()):5d} "
              f"{int((q > 1.6).sum()):5d} {b.size:6d}   {b.min():.3f} / {np.median(b):.3f} / {b.max():.3f}")
    labs = list(sets)
    for i in range(len(labs)):
        for j in range(i + 1, len(labs)):
            qa, ba = sets[labs[i]]
            qb, bb = sets[labs[j]]
            ha, hb = int((qa > 1.5).sum()), int((qb > 1.5).sum())
            p1 = fisher_upper(ha, qa.size, hb, qb.size)
            z, p = mwu(ba, bb)
            print(f"{labs[i]} vs {labs[j]}: tail>1.5 {ha}/{qa.size} vs {hb}/{qb.size}, one-sided Fisher p({labs[i]} larger) = {p1:.3g}; "
                  f"per-order best Mann-Whitney z = {z:+.2f}, two-sided p = {p:.3g}")


main()
