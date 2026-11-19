"""Eigenvalue accuracy of DGEEV and of the port's eigvals against a high-precision refinement (gh:#1667).

Usage: python eig_accuracy.py <test binary> <out tsv> <h_*.npy> ...
The matrices are relocation matrices H dumped by swap_openmc.py (SWAP_DUMP_EIG).  For each: DGEEV's
eigenvalues (scipy.linalg.eigvals), the port's (served by trace_server.rs op 2), and a reference: every
candidate eigenvalue of either code is refined by Newton iteration on det(H - z I) in 80-bit extended precision
(numpy clongdouble, unit round-off 5e-20; Gauss-Jordan inverse, f'/f = -trace of the inverse) from the two codes'
values; the reference set is the union of the refined roots, duplicates (relative distance < 1e-13) removed.  The
differences of interest (1e-4 and up) are far above the reference's own round-off unless an eigenvalue has a condition
number above about 1e12, which `roots_found < n` would show.  Reported, per code and matrix:
the largest, 90th percentile and median over eigenvalues of |computed - nearest reference root| / max(|root|, 0.01)
(the poles live in sqrt(E), 0.003 to 4.4; the floor keeps a root near zero from dominating), the count above 1e-8, and the number of
reference roots found (n expected).  A reference with fewer than n distinct roots means the matrix has
numerically multiple eigenvalues and the per-eigenvalue error is only a conditioning measure.
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


def port_eig(h):
    n = h.shape[0]
    w.write((2).to_bytes(8, 'little') + n.to_bytes(8, 'little') + np.ascontiguousarray(h, dtype='<f8').tobytes())
    w.flush()
    ri = np.frombuffer(r.read(16 * n), dtype='<f8')
    return ri[0::2] + 1j * ri[1::2]


def inv_ld(a):
    """Gauss-Jordan inverse with partial pivoting in clongdouble."""
    n = a.shape[0]
    m = np.concatenate([a.astype(np.clongdouble), np.eye(n, dtype=np.clongdouble)], axis=1)
    for c in range(n):
        p = c + int(np.argmax(np.abs(m[c:, c])))
        if p != c:
            m[[c, p]] = m[[p, c]]
        m[c] = m[c] / m[c, c]
        col = m[:, c].copy()
        col[c] = 0
        m -= np.outer(col, m[c])
    return m[:, n:]


def refine(hl, z0):
    z = np.clongdouble(complex(z0))
    n = hl.shape[0]
    eye = np.eye(n, dtype=np.clongdouble)
    for _ in range(40):
        with np.errstate(all='ignore'):
            inv = inv_ld(hl - z * eye)
            dz = 1 / np.trace(inv)
        if not np.isfinite(dz):   # z sits on an eigenvalue to working precision (singular pivot): done
            break
        z = z + dz
        if abs(dz) < 1e-17 * max(abs(z), 1e-30):
            break
    return z


with open(outp, 'w') as out:
    out.write('file\tn\tcode\tmax_err\tp90_err\tmedian_err\tn_above_1e-8\troots_found\n')
    for f in files:
        h = np.load(f)
        n = h.shape[0]
        ev_l = sl.eigvals(h)
        ev_p = port_eig(h)
        hl = h.astype(np.longdouble)
        cand = np.concatenate([ev_l, ev_p])
        roots = []
        for z0 in cand:
            z = refine(hl, z0)
            if not any(abs(z - q) < 1e-13 * max(abs(z), 1e-30) for q in roots):
                roots.append(z)
        for name, ev in (('dgeev', ev_l), ('port', ev_p)):
            err = np.array([float(min(abs(np.clongdouble(complex(e)) - q) / max(abs(q), 1e-2) for q in roots)) for e in ev])
            out.write(f'{os.path.basename(f)}\t{n}\t{name}\t{err.max():.4e}\t{np.percentile(err, 90):.4e}\t{np.median(err):.4e}\t{int((err > 1e-8).sum())}\t{len(roots)}\n')
            out.flush()
w.write((0).to_bytes(8, 'little'))
w.flush()
proc.wait(timeout=60)
