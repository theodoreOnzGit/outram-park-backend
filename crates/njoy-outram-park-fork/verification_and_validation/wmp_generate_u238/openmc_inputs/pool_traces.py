"""Pool the traces of several trials per code (gh:#1390): final results and per-order statistics.

Usage: python pool_traces.py <openmc prefix pattern with {t}> <port prefix pattern with {t}> <trial tags comma separated>
e.g.   python pool_traces.py tr/omc_296_{t} tr/port_296_{t} t0,t1,t2,t3,t4
Trials missing for one code are skipped for that code. Prints, per code: for each trial the final pole count, grid
points outside rtol/atol of the unperturbed data, orders visited, the order of the best fit, best test-grid
quality; then pooled over trials: the per-order best number of test-grid points outside (fewest over the 30
iterations of an order), the share of iterations whose fit is unusable (-inf quality), and a Mann-Whitney U
comparison of the two codes' per-order bests.
"""
import sys, os, json
import numpy as np


def load(prefix):
    L = open(prefix + ".log.tsv").read().splitlines()
    h = L[0].split("\t")
    rows = [dict(zip(h, l.split("\t"))) for l in L[1:]]
    summ = json.load(open(prefix + ".summary.txt"))
    return rows, summ


def stats(prefix):
    rows, summ = load(prefix)
    n_orders = (len(rows) + 29) // 30
    best_n, best_q = [], []
    for c in range(n_orders):
        ch = rows[c * 30:(c + 1) * 30]
        best_n.append(min(int(r["n_out"]) for r in ch))
        best_q.append(max(float(r["quality"]) for r in ch))
    qs = np.array([float(r["quality"]) for r in rows])
    bi = int(np.argmax(best_q))
    return dict(poles=summ["final_poles"], outside=summ["grid_points_outside"], orders=n_orders,
                best_order=summ["first_order"] + 2 * bi, best_q=max(best_q), best_n=best_n,
                unusable=float(np.mean(np.isneginf(qs))), seconds=summ["seconds"])


def mwu(a, b):
    """Mann-Whitney U with normal approximation and tie correction; returns (U_a, z, two-sided p)."""
    from math import erf, sqrt
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
    p = 1 - erf(abs(z) / sqrt(2))
    return ua, z, p


omc_pat, port_pat, tags = sys.argv[1], sys.argv[2], sys.argv[3].split(",")
res = {}
for name, pat in (("OpenMC", omc_pat), ("port", port_pat)):
    res[name] = {}
    for t in tags:
        pre = pat.format(t=t)
        if os.path.exists(pre + ".summary.txt") and os.path.exists(pre + ".log.tsv"):
            res[name][t] = stats(pre)
for name, d in res.items():
    print(f"{name}:")
    for t, s in d.items():
        print(f"  {t}: {s['poles']} poles, {s['outside']} grid points outside, {s['orders']} orders visited, best at order {s['best_order']} "
              f"(quality {s['best_q']:.3f}), {100 * s['unusable']:.0f}% of iterations unusable (-inf), {s['seconds']:.0f} s")
    if d:
        pl = [s["poles"] for s in d.values()]
        ou = [s["outside"] for s in d.values()]
        print(f"  pooled over {len(d)} trials: poles {sorted(pl)}, outside {sorted(ou)}; median outside {np.median(ou):.0f}")
pooled = {name: np.concatenate([s["best_n"] for s in d.values()]) if d else np.array([]) for name, d in res.items()}
for name, v in pooled.items():
    if v.size:
        print(f"{name}: per-order best test-grid points outside, {v.size} orders: median {np.median(v):.0f}, quartiles {np.percentile(v, 25):.0f}/{np.percentile(v, 75):.0f}, min {v.min()}, "
              f"share of orders below 70000: {np.mean(v < 70000):.2f}")
if pooled["OpenMC"].size and pooled["port"].size:
    ua, z, p = mwu(pooled["port"], pooled["OpenMC"])
    print(f"Mann-Whitney (port vs OpenMC per-order best n_out): U={ua:.0f}, z={z:+.2f}, two-sided p={p:.3f} (orders of one trial are not independent: indicative only)")
