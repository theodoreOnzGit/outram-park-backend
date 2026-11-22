"""Markdown table of the kernel-swap conditions of gh:#1667 (2026-11-19).

Usage: python condition_table.py <swaps dir> <old traces dir>
<swaps dir> holds <tag>_<trial>.log.tsv written by swap_openmc.py; <old traces dir> is diagnosis_296/.  Every condition
is restricted to orders 98 to 130 (the orders all runs share; the acceleration rule can stop a run earlier) and judged
against the pooled OpenMC baseline of the same protocol (`base1`).  Columns: iterations, share of unusable (-inf)
iterations, iterations with quality > 1.5 (count and share), the median of the per-order best quality (complete
orders only), and the one-sided exact Fisher p that the share above 1.5 is LOWER than the new-protocol OpenMC baseline's
(small p = the swap moved OpenMC toward the port), and that it is lower than the OLD five-trial OpenMC pool's.
"""
import os
import sys
from math import comb

import numpy as np

swaps, old = sys.argv[1], sys.argv[2]
LO, HI = 98, 130


def rows(prefix):
    lines = open(prefix + ".log.tsv").read().splitlines()
    h = lines[0].split("\t")
    qi = h.index("quality")
    out = []
    for k, line in enumerate(lines[1:]):
        o = 98 + 2 * (k // 30)
        if LO <= o <= HI:
            out.append((o, float(line.split("\t")[qi])))
    return out


def pool(prefixes):
    prefixes = [p for p in prefixes if os.path.exists(p + ".log.tsv")]
    q, best = [], []
    for p in prefixes:
        r = rows(p)
        q += [v for _, v in r]
        for o in sorted(set(x[0] for x in r)):
            qs = [v for (oo, v) in r if oo == o]
            if len(qs) == 30:
                best.append(max(qs))
    return len(prefixes), np.array(q), np.array(best)


def fisher_lower(hit, n, ref_hit, ref_n):
    """P(hits in the condition <= hit) given the pooled hit count (hypergeometric)."""
    k = hit + ref_hit
    tot = comb(n + ref_n, n)
    return sum(comb(k, x) * comb(n + ref_n - k, n - x) for x in range(0, hit + 1)) / tot


conds = [
    ("OpenMC as shipped, old five trials (2 threads)", [os.path.join(old, f"omc_296_t{i}") for i in range(5)]),
    ("OpenMC as shipped, this protocol (`base1`)", [os.path.join(swaps, f"base1_t{i}") for i in range(3)]),
    ("port, five trials", [os.path.join(old, f"port_296_t{i}") for i in range(5)]),
    ("E: eigvals = port (GSL balance + Francis)", [os.path.join(swaps, f"eig_t{i}") for i in range(3)]),
    ("E1: port Francis, no balancing", [os.path.join(swaps, f"eig1_t{i}") for i in range(3)]),
    ("E2: DGEBAL then port Francis", [os.path.join(swaps, f"eigdgebal_t{i}") for i in range(3)]),
    ("E3: DGEBAL + DGEHRD (LAPACK), port Francis only", [os.path.join(swaps, f"eig3_t{i}") for i in range(4)]),
    ("L: pole-identification lstsq = port", [os.path.join(swaps, f"lp_t{i}") for i in range(3)]),
    ("R: residue lstsq = port", [os.path.join(swaps, f"lr_t{i}") for i in range(3)]),
    ("Q: qr = port (unblocked)", [os.path.join(swaps, f"qr_t{i}") for i in range(3)]),
    ("V: test-grid evaluate = port", [os.path.join(swaps, f"ev_t{i}") for i in range(3)]),
    ("OpenMC forced onto DGELSS, old four trials", [os.path.join(old, f"omcss_296_t{i}") for i in range(4)]),
]
_, bq, _ = pool(conds[1][1])
_, oq, _ = pool(conds[0][1])
b_hit, o_hit = int((bq > 1.5).sum()), int((oq > 1.5).sum())
print("| condition | trials | iterations | unusable | quality > 1.5 | median per-order best | p(lower than `base1`) | p(lower than old pool) |")
print("|---|---|---|---|---|---|---|---|")
for lab, pre in conds:
    nt, q, b = pool(pre)
    if nt == 0:
        continue
    hit = int((q > 1.5).sum())
    p1 = fisher_lower(hit, q.size, b_hit, bq.size)
    p2 = fisher_lower(hit, q.size, o_hit, oq.size)
    print(f"| {lab} | {nt} | {q.size} | {np.mean(np.isneginf(q)) * 100:.1f} % | {hit} ({hit / q.size * 100:.2f} %) | "
          f"{np.median(b):.3f} | {p1:.2g} | {p2:.2g} |")
