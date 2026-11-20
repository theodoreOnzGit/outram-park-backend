"""Paired one-step test of the eigensolvers on OpenMC's own relocation matrices (gh:#1667, 2026-11-20).

Usage: python paired_step.py <input file> <out prefix> <orders lo:hi:step> <test binary>
Environment: OpenMC 8f79d9d on PYTHONPATH (unmodified).
OpenMC's `_vectfit_xs` loop runs with scipy's DGEEV, so its chain of poles is the baseline chain.  At every pole
relocation the matrix H handed to `eigvals` is ALSO solved by the port's `eigvals` (trace_server.rs op 2), and both pole
sets are scored one step on exactly as the loop scores a relocation: real poles inside [s0, s_last] split into
p(1 +- 0.01j), upstream's `vectfit(..., skip_pole_update=True)` for the residues, the test-grid metric and quality of
`_vectfit_xs`.  Nothing is accumulated: the same H, one step, two eigenvalue sets.
Output: <out>.pairs.tsv (order, iteration, quality for DGEEV, quality for the port, n_real each, largest |pole| difference).
"""
import os
import subprocess
import sys
import time

import numpy as np

import openmc.data  # noqa: F401
from openmc.data import multipole as mp

vf = sys.modules['openmc.data.vectfit']
inp, out, orders_arg, binary = sys.argv[1:5]
lo, hi, step = [int(x) for x in orders_arg.split(':')]
orders = list(range(lo, hi + 1, step))

v = np.fromfile(inp, dtype='<f8')
ne, nmt = int(v[0]), int(v[1])
energy, ce_xs = v[2:2 + ne].copy(), v[2 + ne:].reshape(nmt, ne).copy()
mts = [2, 27, 18]
rtol, atol = 1e-3, 1e-5
f = ce_xs * energy
s = np.sqrt(energy)
with np.errstate(divide='ignore'):
    weight = 1.0 / f
for i in range(nmt):
    if np.all(ce_xs[i] <= 1e-7):
        weight[i] = 1.0
    elif np.any(ce_xs[i] <= 1e-7):
        weight[i, ce_xs[i] <= 1e-7] = max(weight[i, ce_xs[i] > 1e-7])
ne_test = (ne - 1) * 10 + 1
test_energy = np.interp(np.arange(ne_test), np.arange(ne_test, step=10), energy)
test_energy[[0, -1]] = energy[[0, -1]]
test_s = np.sqrt(test_energy)
test_xs_ref = np.zeros((nmt, ne_test))
for i in range(nmt):
    test_xs_ref[i] = np.interp(test_energy, energy, ce_xs[i])

d = out + '.pipes'
os.makedirs(d, exist_ok=True)
rq, rs = os.path.join(d, 'req'), os.path.join(d, 'resp')
for p in (rq, rs):
    if os.path.exists(p):
        os.remove(p)
    os.mkfifo(p)
env = dict(os.environ, OUTRAM_WMP_SRV_REQ=rq, OUTRAM_WMP_SRV_RESP=rs)
proc = subprocess.Popen([binary, 'wmp::generate::trace_server::kernel_server', '--ignored', '--exact', '--nocapture'],
                        env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
w = open(rq, 'wb', buffering=1 << 20)
r = open(rs, 'rb', buffering=1 << 20)


def port_eig(h):
    n = h.shape[0]
    w.write((2).to_bytes(8, 'little') + n.to_bytes(8, 'little') + np.ascontiguousarray(h, dtype='<f8').tobytes())
    w.flush()
    ri = np.frombuffer(r.read(16 * n), dtype='<f8')
    return ri[0::2] + 1j * ri[1::2]


def score(poles):
    """The loop's own treatment of a relocation result `poles`, then quality; returns (quality, n_real)."""
    new_poles, n_real = [], 0
    for p in poles:
        if (s[0] <= p.real <= s[-1]) and p.imag == 0.:
            new_poles += [p.real + p.real * 0.01j, p.real - p.real * 0.01j]
            n_real += 1
        else:
            new_poles += [p]
    new_poles = np.array(new_poles)
    # residues of the (possibly split) poles: upstream's own call with skip_pole_update
    _, residues, *_ = vf.vectfit(f, s, new_poles, weight, skip_pole_update=True)
    test_xs = orig_evaluate(test_s, new_poles, residues) / test_energy
    abserr = np.abs(test_xs - test_xs_ref)
    with np.errstate(invalid='ignore', divide='ignore'):
        relerr = abserr / test_xs_ref
        if np.any(np.isnan(abserr)):
            maxre, ratio, ratio2 = np.inf, -np.inf, -np.inf
        elif np.all(abserr <= atol):
            maxre, ratio, ratio2 = 0., 1., 1.
        else:
            maxre = np.max(relerr[abserr > atol])
            ratio = np.sum((relerr < rtol) | (abserr < atol)) / relerr.size
            ratio2 = np.sum((relerr < 10 * rtol) | (abserr < atol)) / relerr.size
    quality = ratio + ratio2 - min(0.1 * maxre, 1) - 0.001 * new_poles.size
    if np.any(test_xs < -atol):
        quality = -np.inf
    return float(quality), n_real


orig_eig, orig_evaluate = vf.eigvals, mp.evaluate
rows = []
count = {'k': 0}
t0 = time.time()


def eig_pair(h, *a, **k):
    ev_l = orig_eig(h, *a, **k)
    ev_p = port_eig(np.asarray(h, dtype=float))
    ql, nl = score(ev_l)
    qp, npr = score(ev_p)
    k_ = count['k']
    count['k'] += 1
    dist = float(np.max(np.abs(ev_l[:, None] - ev_p[None, :]).min(axis=1)))
    rows.append((orders[min(k_ // 30, len(orders) - 1)], k_ % 30 + 1, ql, qp, nl, npr, dist))
    if (k_ + 1) % 30 == 0:
        sys.stderr.write(f'{time.time() - t0:7.0f}s order {rows[-1][0]} done, pairs {len(rows)}\n')
        sys.stderr.flush()
    return ev_l


vf.eigvals = eig_pair
try:
    mp._vectfit_xs(energy, ce_xs, mts, orders=orders)
finally:
    vf.eigvals = orig_eig
    w.write((0).to_bytes(8, 'little'))
    w.flush()
    proc.wait(timeout=60)
    with open(out + '.pairs.tsv', 'w') as fh:
        fh.write('order\tit\tq_dgeev\tq_port\tn_real_dgeev\tn_real_port\tmax_pole_dist\n')
        for row in rows:
            fh.write('\t'.join(repr(x) if isinstance(x, float) else str(x) for x in row) + '\n')
print('pairs', len(rows))
