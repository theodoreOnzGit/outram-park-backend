"""Iteration 1 of every order, OpenMC's `vectfit` with DGELSD (default) against DGELSS (gh:#1390).

Usage: python iter1_driver_null.py <input file> <first order> <last order> <out tsv>
Environment: OpenMC 8f79d9d on PYTHONPATH.

Iteration 1 starts from the deterministic initial poles of `_vectfit_xs` (linspace, p(1+0.01j), conjugates,
sorted), so it is the one step with no accumulated history. The two drivers solve the same least-squares
problems with the same `cond = eps` truncation rule by different algorithms (divide and conquer against QR
iteration): the null for "a different but valid implementation of LAPACK's linear algebra". The result is
the per-order max-over-poles relative difference of the relocated poles, to be set beside the same quantity
for the port against DGELSD (compare_traces.py, iteration 1).
"""
import sys
import numpy as np
from openmc.data.vectfit import vectfit

vfmod = sys.modules['openmc.data.vectfit']
inp, o0, o1, out = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
v = np.fromfile(inp, dtype='<f8')
ne, nmt = int(v[0]), int(v[1])
energy, ce_xs = v[2:2 + ne].copy(), v[2 + ne:].reshape(nmt, ne).copy()
f = ce_xs * energy
s = np.sqrt(energy)
with np.errstate(divide='ignore'):
    weight = 1.0 / f
for i in range(nmt):
    if np.all(ce_xs[i] <= 1e-7):
        weight[i] = 1.0
    elif np.any(ce_xs[i] <= 1e-7):
        weight[i, ce_xs[i] <= 1e-7] = max(weight[i, ce_xs[i] > 1e-7])
orig = vfmod.lstsq


def diff(a, b):
    d = np.array([np.min(np.abs(b - x)) / abs(x) for x in a])
    d = np.where(np.isnan(d), np.inf, d)
    return float(d.max()), float(np.median(d))


lines = ["order\tgelss_vs_gelsd_max\tgelss_vs_gelsd_median\n"]
for order in range(o0, o1 + 1, 2):
    pr = np.linspace(s[0], s[-1], order // 2)
    poles = pr + pr * 0.01j
    poles = np.sort(np.append(poles, np.conj(poles)))
    a, *_ = vectfit(f, s, poles, weight)
    vfmod.lstsq = lambda x, y, *aa, **kk: orig(x, y, *aa, lapack_driver='gelss', **kk)
    try:
        b, *_ = vectfit(f, s, poles, weight)
    finally:
        vfmod.lstsq = orig
    d = diff(a, b)
    lines.append(f"{order}\t{d[0]!r}\t{d[1]!r}\n")
    print(order, f"{d[0]:.2e} {d[1]:.2e}", flush=True)
open(out, "w").write("".join(lines))
