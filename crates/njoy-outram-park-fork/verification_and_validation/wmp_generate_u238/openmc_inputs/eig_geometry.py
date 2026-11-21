"""Geometry of the eigenvalue sets of DGEEV and of the port's solvers on dumped relocation matrices (gh:#1667).

Usage: python eig_geometry.py <test binary> <out tsv> <h_*.npy> ...
For each matrix H (dumped by swap_openmc.py with SWAP_DUMP_EIG) and each solver (DGEEV through scipy; the port's op 6
modes 0 = GSL balancing, 1 = none, 2 = DGEBAL; mode 3 = scipy DGEBAL + DGEHRD then the port's Francis QR) prints the
number of real eigenvalues, the number of complex pairs, the largest |Im|, the largest |Re - median Re| inside the
cluster, and the number of eigenvalues outside the interval [s_lo, s_hi] = [0.003, 4.5] (the sqrt(E) range of piece 296).
Not an accuracy measure (eig_accuracy.py is): it shows whether two backward-stable solvers draw the numerically defective
cluster of H with the same shape.
"""
import os
import subprocess
import sys

import numpy as np
import scipy.linalg as sl

binary, outp = sys.argv[1], sys.argv[2]
files = sys.argv[3:]
d = outp + '.pipes'
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


def port_eig(h, mode):
    n = h.shape[0]
    w.write((6).to_bytes(8, 'little') + n.to_bytes(8, 'little') + mode.to_bytes(8, 'little')
            + np.ascontiguousarray(h, dtype='<f8').tobytes())
    w.flush()
    ri = np.frombuffer(r.read(16 * n), dtype='<f8')
    return ri[0::2] + 1j * ri[1::2]


def summarize(ev):
    real = int(np.sum(ev.imag == 0))
    cpl = int(np.sum(ev.imag > 0))
    return real, cpl, float(np.max(np.abs(ev.imag))), float(np.max(np.abs(ev.real - np.median(ev.real)))), \
        int(np.sum((ev.real < 0.003) | (ev.real > 4.5)))


with open(outp, 'w') as out:
    out.write('file\tn\tsolver\tn_real\tn_pairs\tmax_abs_im\tmax_dev_re\tn_re_outside\n')
    for f in files:
        h = np.load(f)
        n = h.shape[0]
        hb, _ = sl.matrix_balance(h)
        hh = sl.hessenberg(hb)
        sets = [('dgeev', sl.eigvals(h)), ('port_gsl_bal', port_eig(h, 0)), ('port_nobal', port_eig(h, 1)),
                ('port_dgebal', port_eig(h, 2)), ('lapack_bal_hess+port_qr', port_eig(hh, 1))]
        for name, ev in sets:
            s = summarize(ev)
            out.write(f'{os.path.basename(f)}\t{n}\t{name}\t{s[0]}\t{s[1]}\t{s[2]:.4e}\t{s[3]:.4e}\t{s[4]}\n')
            out.flush()
w.write((0).to_bytes(8, 'little'))
w.flush()
proc.wait(timeout=60)
