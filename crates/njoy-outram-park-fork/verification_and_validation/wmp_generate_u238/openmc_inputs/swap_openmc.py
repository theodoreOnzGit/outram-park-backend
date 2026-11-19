"""OpenMC's UNMODIFIED `_vectfit_xs` loop with ONE numerical kernel swapped for the port's (gh:#1667, 2026-11-19).

Usage: python swap_openmc.py <input file> <out prefix> <swap list> <orders lo:hi> [<test binary>]
  <input file>   piece input (make_trial_inputs.py layout), e.g. in_296_t0.f64
  <swap list>    comma separated, any of: none, eig, lstsq_pole, lstsq_res, qr, eval, all (= the five)
  <orders lo:hi> fitting orders lo, lo+2, ..., hi (the 17 orders 98:130 of the 296 diagnosis)
  <test binary>  the njoy-outram-park-fork lib test binary (cargo test --release --lib --no-run); its
                 `wmp::generate::trace_server::kernel_server` test is started as a subprocess when a swap needs it.
Environment: OpenMC 8f79d9d on PYTHONPATH (nothing in it is edited), OMP_NUM_THREADS / OPENBLAS_NUM_THREADS.
             SWAP_COMPARE=1 also computes the LAPACK answer for every swapped call and logs the two in
             <out>.kcmp.tsv (the static kernel measurement; the PORT's answer is what OpenMC then uses).

What a swap means. Module-level names of `openmc.data.vectfit` (`eigvals`, `lstsq`, `qr`) and
`openmc.data.multipole` (`evaluate`, the test-grid call only) are replaced by functions that send the SAME array to the
port's kernel (opcodes in trace_server.rs) and return what the port returns in scipy's shape:
  eig         `eigvals(H)`                              -> npy.rs::eigvals (petir Francis QR, GSL balancing)
  lstsq_pole  `lstsq` of the pole-identification solves -> npy.rs::lstsq (QR, then Golub-Reinsch SVD of R)
  lstsq_res   `lstsq` of the residue solve (2 n_s rows) -> npy.rs::lstsq
  qr          `qr(A, mode="economic")`                  -> npy.rs::HouseholderQr (unblocked DGEQR2); Q is never formed:
              the two ways `vectfit.py` reads it, `Q[-1, a:b]` and `Q[:, a:b].T @ b`, are served by `apply_qt`
              SWAP_EIG_MODE=0/1/2 picks a balancing variant of the port's solver (op 6 of trace_server.rs): 0 GSL
              balancing (= the library), 1 none, 2 LAPACK DGEBAL('B') then the Francis QR without GSL's balancing
  eval        `evaluate` on the 10x test grid           -> vectfit.rs::evaluate (sequential complex sum)
The loop, the metric and the stopping rule are upstream's; per-iteration rows are logged exactly as in
trace_openmc.py (`order` here is the true order).
Outputs: <out>.log.tsv, <out>.summary.txt (and <out>.kcmp.tsv with SWAP_COMPARE=1).
"""
import json
import os
import subprocess
import sys
import time

import numpy as np
from scipy.signal import find_peaks

import openmc.data  # noqa: F401
from openmc.data import multipole as mp

vf = sys.modules['openmc.data.vectfit']

inp, out, swap_arg, orders_arg = sys.argv[1:5]
test_bin = sys.argv[5] if len(sys.argv) > 5 else None
swaps = set(swap_arg.split(','))
if 'all' in swaps:
    swaps |= {'eig', 'lstsq_pole', 'lstsq_res', 'qr', 'eval'}
swaps.discard('none')
lo, hi = [int(x) for x in orders_arg.split(':')]
compare = os.environ.get('SWAP_COMPARE') == '1'


def load(path):
    v = np.fromfile(path, dtype='<f8')
    ne, nmt = int(v[0]), int(v[1])
    return v[2:2 + ne].copy(), v[2 + ne:].reshape(nmt, ne).copy()


energy, ce_xs = load(inp)
ref_inp = os.environ.get('SWAP_REF', inp)
_, ce_ref = load(ref_inp)
mts = [2, 27, 18]
rtol, atol = 1e-3, 1e-5

ne = energy.size
ne_test = (ne - 1) * 10 + 1
test_energy = np.interp(np.arange(ne_test), np.arange(ne_test, step=10), energy)
test_energy[[0, -1]] = energy[[0, -1]]
test_xs_ref = np.zeros((len(mts), ne_test))
for i in range(len(mts)):
    test_xs_ref[i] = np.interp(test_energy, energy, ce_xs[i])


class Server:
    def __init__(self, binary):
        d = out + '.pipes'
        os.makedirs(d, exist_ok=True)
        rq, rs = os.path.join(d, 'req'), os.path.join(d, 'resp')
        for p in (rq, rs):
            if os.path.exists(p):
                os.remove(p)
            os.mkfifo(p)
        env = dict(os.environ, OUTRAM_WMP_SRV_REQ=rq, OUTRAM_WMP_SRV_RESP=rs)
        self.proc = subprocess.Popen([binary, 'wmp::generate::trace_server::kernel_server', '--ignored', '--exact',
                                      '--nocapture'], env=env, stdout=subprocess.DEVNULL,
                                     stderr=open(out + '.server.log', 'w'))
        self.w = open(rq, 'wb', buffering=1 << 20)
        self.r = open(rs, 'rb', buffering=1 << 20)

    def send(self, *parts):
        for p in parts:
            if isinstance(p, (int, np.integer)):
                self.w.write(int(p).to_bytes(8, 'little'))
            elif isinstance(p, float):
                self.w.write(np.float64(p).tobytes())
            else:
                self.w.write(np.ascontiguousarray(p, dtype='<f8').tobytes())
        self.w.flush()

    def recv(self, n):
        return np.frombuffer(self.r.read(8 * n), dtype='<f8').copy()

    def close(self):
        self.send(0)
        self.proc.wait(timeout=60)


srv = Server(test_bin) if swaps else None
kcmp = open(out + '.kcmp.tsv', 'w') if (compare and swaps) else None
if kcmp:
    kcmp.write('kind\tm\tn\trel_diff\tmetric_port\tmetric_lapack\textra\n')

orig = dict(eigvals=vf.eigvals, lstsq=vf.lstsq, qr=vf.qr, evaluate=mp.evaluate)


def match_dist(a, b):
    """largest over a of |a - nearest b| / max(|a|, 1e-300)"""
    d = np.abs(a[:, None] - b[None, :]).min(axis=1)
    return float(np.max(d / np.maximum(np.abs(a), 1e-300)))


def eig_srv(h, *args, **kw):
    n = h.shape[0]
    mode = os.environ.get('SWAP_EIG_MODE')   # unset: op 2 (the library's eigvals); 0, 1, 2: op 6 variants (trace_server.rs)
    if mode is None:
        srv.send(2, n, np.asarray(h, dtype=float).ravel(order='C'))
    else:
        srv.send(6, n, int(mode), np.asarray(h, dtype=float).ravel(order='C'))
    ri = srv.recv(2 * n)
    ev = ri[0::2] + 1j * ri[1::2]
    if kcmp:
        el = orig['eigvals'](h)
        kcmp.write(f'eig\t{n}\t{n}\t{match_dist(el, ev):.6e}\t{0}\t{0}\t{int(np.sum(el.imag == 0))}/{int(np.sum(ev.imag == 0))}\n')
    return ev


def lstsq_srv(a, b, *args, **kw):
    m, n = a.shape
    is_res = m > 2000
    key = 'lstsq_res' if is_res else 'lstsq_pole'
    if key not in swaps:
        return orig['lstsq'](a, b, *args, **kw)
    srv.send(1, m, n, float(np.finfo(float).eps), np.asarray(a, dtype=float).ravel(order='F'), np.asarray(b, dtype=float))
    x = srv.recv(n)
    if kcmp:
        sl = orig['lstsq'](a, b)
        xl = sl[0]
        rp = float(np.linalg.norm(a @ x - b))
        rl = float(np.linalg.norm(a @ xl - b))
        dx = float(np.max(np.abs(x - xl)) / max(np.max(np.abs(xl)), 1e-300))
        sg = orig['lstsq'](a, b, lapack_driver='gelss')
        rg = float(np.linalg.norm(a @ sg[0] - b))
        kcmp.write(f'{key}\t{m}\t{n}\t{dx:.6e}\t{rp:.9e}\t{rl:.9e}\trank_gelsd={int(sl[2])};rank_gelss={int(sg[2])};resid_gelss={rg:.9e}\n')
    return x, np.zeros(0), -1, np.zeros(0)


class QCols:
    def __init__(self, s, lo_, hi_):
        self.s, self.lo, self.hi = s, lo_, hi_

    @property
    def T(self):
        return self

    def __matmul__(self, b):
        b = np.asarray(b, dtype=float)
        self.s.send(4, self.lo, self.hi, b.size, b)
        return self.s.recv(self.hi - self.lo)


class QSrv:
    def __init__(self, s, m, n, a):
        self.s, self.m, self.n, self.a = s, m, n, a

    def __getitem__(self, key):
        i, j = key
        if isinstance(i, slice):
            return QCols(self.s, j.start, j.stop)
        assert i == -1 and isinstance(j, slice)
        e = np.zeros(self.m)
        e[-1] = 1.0
        self.s.send(4, j.start, j.stop, self.m, e)
        return self.s.recv(j.stop - j.start)


def qr_srv(a, *args, **kw):
    m, n = a.shape
    srv.send(3, m, n, np.asarray(a, dtype=float).ravel(order='F'))
    k = min(m, n)
    r = srv.recv(k * n).reshape(k, n)
    q = QSrv(srv, m, n, a)
    if kcmp:
        ql, rl = orig['qr'](a, *args, **kw)
        # R rows agree up to LAPACK's sign convention (both use beta = -sign(alpha) ||x||), so compare directly
        dr = float(np.max(np.abs(r - rl)) / max(np.max(np.abs(rl)), 1e-300))
        qlast = ql[-1, :]
        qs = q[-1, 0:k]
        dq = float(np.max(np.abs(qs - qlast)) / max(np.max(np.abs(qlast)), 1e-300))
        kcmp.write(f'qr\t{m}\t{n}\t{dr:.6e}\t{dq:.6e}\t0\tR_rel_diff;Qlast_rel_diff\n')
    return q, r


ne_test_size = ne_test
rows = []
state = {"best_quality": -np.inf, "order": None, "iter": 0, "ocount": 0}
orders = list(range(lo, hi + 1, 2))


def evaluate_w(eval_points, pole_values, residue_matrix, *a, **kw):
    if eval_points.size == ne_test_size and 'eval' in swaps:
        rm = np.asarray(residue_matrix)
        nv, npol = rm.shape
        pv = np.asarray(pole_values)
        srv.send(5, eval_points.size, npol, nv, np.asarray(eval_points, dtype=float),
                 np.column_stack([pv.real, pv.imag]).ravel(),
                 np.column_stack([rm.real.ravel(), rm.imag.ravel()]).ravel())
        res = srv.recv(nv * eval_points.size).reshape(nv, eval_points.size)
        if kcmp:
            rl = orig['evaluate'](eval_points, pole_values, residue_matrix, *a, **kw)
            kcmp.write(f'eval\t{nv}\t{eval_points.size}\t{float(np.max(np.abs(res - rl)) / max(np.max(np.abs(rl)), 1e-300)):.6e}\t0\t0\t'
                       f'differing_values={int(np.sum(res != rl))}\n')
    else:
        res = orig['evaluate'](eval_points, pole_values, residue_matrix, *a, **kw)
    if eval_points.size != ne_test_size:
        return res
    new_poles = pole_values
    test_xs = res / test_energy
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
    n_out = int(np.sum(~((relerr < rtol) | (abserr < atol))))
    state["iter"] += 1
    if state["iter"] == 31:
        state["iter"] = 1
        state["ocount"] += 1
    order = orders[min(state["ocount"], len(orders) - 1)]
    accepted = bool(quality > state["best_quality"])
    if accepted:
        state["best_quality"] = quality
    rows.append(dict(order=order, it=state["iter"], n_real=int(sum(1 for p in new_poles if p.imag == 0)),
                     ratio=float(ratio), ratio2=float(ratio2), maxre=float(maxre), quality=float(quality),
                     n_out=n_out, accepted=int(accepted)))
    if len(rows) % 30 == 0:
        sys.stderr.write(f"{time.time() - t0:8.0f}s order {order} done, best so far {state['best_quality']:.4f}\n")
        sys.stderr.flush()
    return res


dump_dir = os.environ.get('SWAP_DUMP_EIG')   # directory: save every `SWAP_DUMP_EVERY`-th relocation matrix H as .npy
dump_n = {'calls': 0}


def eig_dump_wrap(inner):
    def f(h, *a, **k):
        dump_n['calls'] += 1
        if dump_dir and dump_n['calls'] % int(os.environ.get('SWAP_DUMP_EVERY', '20')) == 0:
            np.save(os.path.join(dump_dir, f'h_{dump_n["calls"]:05d}.npy'), np.asarray(h))
        return inner(h, *a, **k)
    return f


if 'eig' in swaps:
    vf.eigvals = eig_srv
if dump_dir:
    os.makedirs(dump_dir, exist_ok=True)
    vf.eigvals = eig_dump_wrap(vf.eigvals)
if 'lstsq_pole' in swaps or 'lstsq_res' in swaps:
    vf.lstsq = lstsq_srv
if 'qr' in swaps:
    vf.qr = qr_srv
mp.evaluate = evaluate_w

t0 = time.time()
p, r = mp._vectfit_xs(energy, ce_xs, mts, orders=orders)
dt = time.time() - t0
mp.evaluate = orig['evaluate']
vf.eigvals, vf.lstsq, vf.qr = orig['eigvals'], orig['lstsq'], orig['qr']
if srv:
    srv.close()
if kcmp:
    kcmp.close()

s = np.sqrt(energy)
sg = vf.evaluate(s, p, r * 1j) / energy
dab = np.abs(sg - ce_ref)
outside = int(np.sum(~((dab / np.abs(ce_ref) < rtol) | (dab < atol))))
keys = list(rows[0].keys())
with open(out + ".log.tsv", "w") as fh:
    fh.write("\t".join(keys) + "\n")
    for d in rows:
        fh.write("\t".join(repr(d[k]) if isinstance(d[k], float) else str(d[k]) for k in keys) + "\n")
summ = dict(code="openmc", swaps=sorted(swaps), input=inp, orders=orders_arg, iterations=len(rows),
            final_poles=int(p.size), grid_points_outside=outside, seconds=dt, best_quality=state["best_quality"])
open(out + ".summary.txt", "w").write(json.dumps(summ, indent=1) + "\n")
print(json.dumps(summ), flush=True)
