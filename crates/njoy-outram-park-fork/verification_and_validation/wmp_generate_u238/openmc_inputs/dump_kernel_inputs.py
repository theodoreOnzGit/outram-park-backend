"""Dump the matrices of one pole-relocation call and LAPACK's answers (gh:#1390).

Usage: python dump_kernel_inputs.py <input file> <order> <out file>
Environment: OpenMC 8f79d9d on PYTHONPATH.

Runs upstream's `vectfit(f, s, initial_poles, weight)` (iteration 1 of `_vectfit_xs` at `order`) UNMODIFIED, with
the module-level names `vectfit.lstsq` and `vectfit.eigvals` wrapped to record
  * the first `lstsq` call (the pole-identification least squares; `a` is the column-scaled matrix `wlstsq`
    passes, `b` the right-hand side): the solution, DGELSD's singular values and rank;
  * the following `eigvals` call: the matrix and DGEEV's eigenvalues.
File layout (little-endian f64): m, n, a[m*n] (row-major), b[m], x[n], n_s, s[n_s], rank, N, H[N*N] (row-major),
then N eigenvalue pairs (re, im).
"""
import sys
import numpy as np
from openmc.data.vectfit import vectfit

vfmod = sys.modules['openmc.data.vectfit']
inp, order, out = sys.argv[1], int(sys.argv[2]), sys.argv[3]
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
rec = {}
o_lstsq, o_eig = vfmod.lstsq, vfmod.eigvals


def lstsq_w(a, b, *args, **kw):
    sol = o_lstsq(a, b, *args, **kw)
    if "lstsq" not in rec:
        rec["lstsq"] = (a.copy(), b.copy(), sol[0].copy(), sol[3].copy(), int(sol[2]))
    return sol


def eig_w(m, *args, **kw):
    r = o_eig(m, *args, **kw)
    if "eig" not in rec:
        rec["eig"] = (m.copy(), r.copy())
    return r


vfmod.lstsq, vfmod.eigvals = lstsq_w, eig_w
pr = np.linspace(s[0], s[-1], order // 2)
poles = pr + pr * 0.01j
poles = np.sort(np.append(poles, np.conj(poles)))
vectfit(f, s, poles, weight)
a, b, x, sv, rank = rec["lstsq"]
H, ev = rec["eig"]
buf = [float(a.shape[0]), float(a.shape[1])] + a.ravel().tolist() + b.tolist() + x.tolist() + [float(sv.size)] + sv.tolist() + [float(rank)]
buf += [float(H.shape[0])] + H.ravel().tolist() + np.column_stack([ev.real, ev.imag]).ravel().tolist()
np.array(buf, dtype='<f8').tofile(out)
thr = np.finfo(float).eps * sv.max()
print(f"order {order}: lstsq {a.shape}, DGELSD rank {rank}, s_max {sv.max():.3e}, singular values within 1e2 of eps*s_max: "
      f"{int(np.sum((sv > thr / 100) & (sv < thr * 100)))}; eig N = {H.shape[0]}", flush=True)
