"""Single-step sensitivity of OpenMC 8f79d9d's `vectfit` (one pole-relocation call) on a piece (gh:#1390).

Usage: python step_openmc.py <input file> <openmc trace prefix> <rows comma separated> <out tsv> [<n perturbations, default 2>]
Environment: OpenMC 8f79d9d on PYTHONPATH.

For each trace row k (iteration j of an order, step k -> k+1 inside one order, both without a real-pole
split): call upstream's `vectfit(f, s, poles_k, weight)` UNMODIFIED on
  (a) the exact poles OpenMC logged at row k: the output must equal the logged row k+1 (reproducibility);
  (b) the same poles with every real and imaginary part multiplied by 1 +- 2^-52 (random sign, default_rng(7)):
      how far OpenMC's own output moves under a 1-ulp change of its INPUT POLES.
The metric is the largest (and median) over the poles of |a - nearest b| / |a|, as in trace_step.rs.
"""
import sys
import numpy as np
from scipy.signal import find_peaks
from openmc.data.vectfit import vectfit

inp, prefix, rows, out = sys.argv[1], sys.argv[2], [int(x) for x in sys.argv[3].split(",")], sys.argv[4]
nper = int(sys.argv[5]) if len(sys.argv) > 5 else 2
v = np.fromfile(inp, dtype='<f8')
ne, nmt = int(v[0]), int(v[1])
energy, ce_xs = v[2:2 + ne].copy(), v[2 + ne:].reshape(nmt, ne).copy()
f = ce_xs * energy
s = np.sqrt(energy)
with np.errstate(divide='ignore'):
    weight = 1.0 / f
for i in range(nmt):  # as _vectfit_xs
    if np.all(ce_xs[i] <= 1e-7):
        weight[i] = 1.0
    elif np.any(ce_xs[i] <= 1e-7):
        weight[i, ce_xs[i] <= 1e-7] = max(weight[i, ce_xs[i] > 1e-7])
first_order = max(2, 2 * find_peaks(ce_xs[0] + ce_xs[1])[0].size)
L = open(prefix + ".log.tsv").read().splitlines()
hdr = L[0].split("\t")
sizes = [int(l.split("\t")[hdr.index("order")]) for l in L[1:]]
vals = np.fromfile(prefix + ".poles.f64", dtype='<f8')
offs = np.concatenate([[0], np.cumsum([2 * n for n in sizes])])


def poles_of(r):
    # collapse each split pair p(1 +- 0.01j) back to the real pole: the raw relocation output
    zz = vals[offs[r]:offs[r + 1]].reshape(-1, 2)
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
    return np.array(raw)


def diff(a, b):
    d = np.array([np.min(np.abs(b - x)) / abs(x) for x in a])
    d = np.where(np.isnan(d), np.inf, d)
    return float(d.max()), float(np.median(d))


# A second, equally valid LAPACK least-squares driver, as a null for "another implementation of the same
# algebra": scipy.linalg.lstsq(..., lapack_driver='gelss') (SVD by QR iteration, the same `cond` truncation
# rule as DGELSD's) in place of the default 'gelsd'. Reported in the columns driver_max/driver_median.
import os
vfmod = sys.modules['openmc.data.vectfit']
_lstsq = vfmod.lstsq
_driver = os.environ.get("STEP_DRIVER", "gelss")


def _lstsq_alt(a, b, *args, **kw):
    return _lstsq(a, b, *args, lapack_driver=_driver, **kw)


rng = np.random.default_rng(7)
res = ["row\torder\titer_from\treproduce_max\tpert_max\tpert_median\tdriver_max\tdriver_median\n"]
for k in rows:
    order = first_order + 2 * (k // 30)
    if (k + 1) // 30 != k // 30:
        continue
    pin, pexp = poles_of(k), poles_of(k + 1)
    if pin.size != order or pexp.size != order:
        continue
    po, *_ = vectfit(f, s, pin, weight)
    rep = diff(pexp, po)[0]
    vfmod.lstsq = _lstsq_alt
    try:
        pg, *_ = vectfit(f, s, pin, weight)
    finally:
        vfmod.lstsq = _lstsq
    dg = diff(po, pg)
    pm = []
    for _ in range(nper):
        sg = rng.choice([-1.0, 1.0], (pin.size, 2))
        pp = pin.real * (1 + sg[:, 0] * 2.0**-52) + 1j * pin.imag * (1 + sg[:, 1] * 2.0**-52)
        q, *_ = vectfit(f, s, pp, weight)
        pm.append(diff(po, q))
    for mx, md in pm:
        res.append(f"{k}\t{order}\t{k % 30}\t{rep:.3e}\t{mx!r}\t{md!r}\t{dg[0]!r}\t{dg[1]!r}\n")
    print(k, order, k % 30, f"reproduce {rep:.2e}", " ".join(f"{m[0]:.2e}" for m in pm), f"{_driver} {dg[0]:.2e}", flush=True)
open(out, "w").write("".join(res))
