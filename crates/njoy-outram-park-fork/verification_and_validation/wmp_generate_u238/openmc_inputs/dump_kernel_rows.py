"""Dump every `lstsq` call of one pole-relocation call, started from the poles an OpenMC trace logged (gh:#1390).

Usage: python dump_kernel_rows.py <input file> <openmc trace prefix> <row> <out file> [all]
Environment: OpenMC 8f79d9d on PYTHONPATH.

Calls upstream's `vectfit(f, s, poles_k, weight)` UNMODIFIED on the raw poles of trace row k (the real-pole split undone,
as in step_openmc.py), with the module-level name `vectfit.lstsq` wrapped to record each call. Without `all` only the
first call (the pole-identification solve) is kept; with `all` the residue solves too (about 16 MB each).
Layout (little-endian f64): n_records, then per record: m, n, a[m*n] row-major, b[m], x[n], n_s, s[n_s], rank.
"""
import sys
import numpy as np
from scipy.signal import find_peaks
from openmc.data.vectfit import vectfit

vfmod = sys.modules['openmc.data.vectfit']
inp, prefix, row, out = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
keep_all = len(sys.argv) > 5 and sys.argv[5] == "all"
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
L = open(prefix + ".log.tsv").read().splitlines()
hdr = L[0].split("\t")
sizes = [int(l.split("\t")[hdr.index("order")]) for l in L[1:]]
vals = np.fromfile(prefix + ".poles.f64", dtype='<f8')
offs = np.concatenate([[0], np.cumsum([2 * n for n in sizes])])
zz = vals[offs[row]:offs[row + 1]].reshape(-1, 2)
z = zz[:, 0] + 1j * zz[:, 1]
raw, i = [], 0
while i < z.size:
    if i + 1 < z.size and z[i].imag != 0 and z[i].imag == z[i].real * 0.01 and z[i + 1].real == z[i].real \
            and z[i + 1].imag == -(z[i].real * 0.01):
        raw.append(complex(z[i].real, 0.0))
        i += 2
    else:
        raw.append(z[i])
        i += 1
pin = np.array(raw)
recs = []
orig = vfmod.lstsq


def w(a, b, *args, **kw):
    sol = orig(a, b, *args, **kw)
    if keep_all or not recs:
        recs.append((a.copy(), b.copy(), sol[0].copy(), sol[3].copy(), int(sol[2])))
    return sol


vfmod.lstsq = w
try:
    vectfit(f, s, pin, weight)
finally:
    vfmod.lstsq = orig
buf = [float(len(recs))]
for a, b, x, sv, rank in recs:
    buf += [float(a.shape[0]), float(a.shape[1])] + a.ravel().tolist() + b.tolist() + x.tolist() + [float(sv.size)] + sv.tolist() + [float(rank)]
np.array(buf, dtype='<f8').tofile(out)
print(row, "records", len(recs), "ranks", [r[4] for r in recs], flush=True)
