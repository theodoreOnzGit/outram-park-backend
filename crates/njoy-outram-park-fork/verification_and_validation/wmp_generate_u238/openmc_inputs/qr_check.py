"""The port's Householder QR against scipy's (DGEQRF) on the matrices OpenMC's vectfit builds (gh:#1667).

Usage: python qr_check.py <test binary> <piece input> <order>
Runs upstream's `vectfit` once (iteration 1 of the order, initial poles as `_vectfit_xs` builds them) with `qr` wrapped:
every matrix A handed to `qr(A, mode="economic")` is also sent to the port's `HouseholderQr` (trace_server.rs op 3),
and R is compared with LAPACK's row by row: the largest |R_port| - |R_lapack| difference relative to max|R|, and the
number of rows whose sign differs.  Row-sign differences are a convention (they do not change R^T R or the solve);
a magnitude difference would be a defect.
"""
import os
import subprocess
import sys

import numpy as np
from scipy.signal import find_peaks

import openmc.data  # noqa: F401
from openmc.data import multipole  # noqa: F401

vf = sys.modules['openmc.data.vectfit']
binary, inp, order = sys.argv[1], sys.argv[2], int(sys.argv[3])
v = np.fromfile(inp, dtype='<f8')
ne, nmt = int(v[0]), int(v[1])
energy, ce_xs = v[2:2 + ne], v[2 + ne:].reshape(nmt, ne)
f = ce_xs * energy
s = np.sqrt(energy)
with np.errstate(divide='ignore'):
    weight = 1.0 / f
for i in range(nmt):
    if np.all(ce_xs[i] <= 1e-7):
        weight[i] = 1.0
    elif np.any(ce_xs[i] <= 1e-7):
        weight[i, ce_xs[i] <= 1e-7] = max(weight[i, ce_xs[i] > 1e-7])
poles_r = np.linspace(s[0], s[-1], order // 2)
poles = poles_r + poles_r * 0.01j
poles = np.sort(np.append(poles, np.conj(poles)))

d = '/tmp/qr_check_pipes_%d' % os.getpid()
os.makedirs(d)
rq, rs = d + '/req', d + '/resp'
os.mkfifo(rq)
os.mkfifo(rs)
env = dict(os.environ, OUTRAM_WMP_SRV_REQ=rq, OUTRAM_WMP_SRV_RESP=rs)
proc = subprocess.Popen([binary, 'wmp::generate::trace_server::kernel_server', '--ignored', '--exact', '--nocapture'],
                        env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
w = open(rq, 'wb', buffering=1 << 20)
r = open(rs, 'rb', buffering=1 << 20)
orig_qr = vf.qr


def qr_cmp(a, *args, **kw):
    q, rl = orig_qr(a, *args, **kw)
    m, n = a.shape
    w.write((3).to_bytes(8, 'little') + m.to_bytes(8, 'little') + n.to_bytes(8, 'little')
            + np.asarray(a, dtype='<f8').ravel(order='F').tobytes())
    w.flush()
    k = min(m, n)
    rp = np.frombuffer(r.read(8 * k * n), dtype='<f8').reshape(k, n)
    scale = np.max(np.abs(rl))
    dabs = np.max(np.abs(np.abs(rp) - np.abs(rl))) / scale
    dsig = np.max(np.abs(rp - rl)) / scale
    flips = int(np.sum(np.sign(np.diag(rp)) != np.sign(np.diag(rl))))
    # the invariant that does not depend on row signs or on the arbitrary rows beyond numerical rank: R^T R = A^T A
    ata = a.T @ a
    sc2 = np.max(np.abs(ata))
    e_l = np.max(np.abs(rl.T @ rl - ata)) / sc2
    e_p = np.max(np.abs(rp.T @ rp - ata)) / sc2
    print(f'   |R^T R - A^T A|_max / |A^T A|_max: LAPACK {e_l:.2e}, port {e_p:.2e}', flush=True)
    # diag of R: relative to its own size
    dd = np.abs(np.abs(np.diag(rp)) - np.abs(np.diag(rl)))
    print(f'qr {m}x{n}: max|R| = {scale:.3e}; max ||R_port|-|R_lapack|| / max|R| = {dabs:.2e}; with signs {dsig:.2e}; '
          f'diagonal sign flips {flips}; diag abs difference max {dd.max():.2e}, worst relative to |R_ii| '
          f'{np.max(dd / np.maximum(np.abs(np.diag(rl)), 1e-300)):.2e}', flush=True)
    return q, rl


vf.qr = qr_cmp
vf.vectfit(f, s, poles, weight)
w.write((0).to_bytes(8, 'little'))
w.flush()
proc.wait(timeout=60)
