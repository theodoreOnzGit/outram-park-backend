"""Order-by-order trace of OpenMC 8f79d9d's UNMODIFIED `_vectfit_xs` on one piece (gh:#1390, 2026-11-19).

Usage: python trace_openmc.py <input file from make_trial_inputs.py> <out prefix> [<unperturbed input file>]
Environment: OpenMC 8f79d9d on PYTHONPATH (no njoy needed).

Nothing in OpenMC is edited. The module-level names `multipole.vectfit`, `multipole.evaluate` and
`vectfit.lstsq` are replaced by wrappers that call the originals and return their values unchanged, and log:
  * every `evaluate` call on the 10x finer test grid (the one in the order loop): the metric of
    `_vectfit_xs` recomputed from the test grid with the same expressions (ratio, ratio2, maxre, quality,
    points outside rtol/atol on the test grid), whether it is accepted (strict `>`), the iteration within
    the order;
  * every `lstsq` call: shape, DGELSD rank, s_min/s_max, the number of singular values within 1e2 of the
    truncation threshold `eps * s_max`;
  * the poles of each call, to <out>.poles.f64 (re, im pairs, row order of <out>.log.tsv).
The returned fit is upstream's own, so the final result needs no separate validation except that the
unperturbed trace reproduces the saved fixture (checked by the caller).
Outputs: <out>.log.tsv, <out>.orders.tsv, <out>.poles.f64, <out>.final.f64 (poles, residues),
<out>.summary.txt.
"""
import sys, os, time, json
import numpy as np
from scipy.signal import find_peaks
import openmc.data  # noqa: F401  (`openmc.data.vectfit` the attribute is the function, so take the module from sys.modules)
from openmc.data import multipole as mp
vf = sys.modules['openmc.data.vectfit']

inp, out = sys.argv[1], sys.argv[2]
ref_inp = sys.argv[3] if len(sys.argv) > 3 else inp


def load(path):
    v = np.fromfile(path, dtype='<f8')
    ne, nmt = int(v[0]), int(v[1])
    return v[2:2 + ne].copy(), v[2 + ne:].reshape(nmt, ne).copy()


energy, ce_xs = load(inp)
_, ce_ref = load(ref_inp)
mts = [2, 27, 18]
rtol, atol = 1e-3, 1e-5
n_peaks = find_peaks(ce_xs[0] + ce_xs[1])[0].size

# the test grid, exactly as _vectfit_xs builds it
ne = energy.size
n_finer = 10
ne_test = (ne - 1) * n_finer + 1
test_energy = np.interp(np.arange(ne_test), np.arange(ne_test, step=n_finer), energy)
test_energy[[0, -1]] = energy[[0, -1]]
test_xs_ref = np.zeros((len(mts), ne_test))
for i in range(len(mts)):
    test_xs_ref[i] = np.interp(test_energy, energy, ce_xs[i])

rows = []          # one dict per test-grid evaluate call
poles_dump = []    # new_poles per call
lstsq_log = []     # lstsq calls since the last test-grid evaluate call
order_end = []
state = {"best_quality": -np.inf, "best_ratio": -np.inf, "order": None, "iter": 0, "found_better": False,
         "n_discarded": 0}
orig_lstsq, orig_evaluate = vf.lstsq, mp.evaluate
eps = np.finfo(float).eps


# Optional experiment (gh:#1390): TRACE_LSTSQ_DRIVER=gelss makes every least-squares solve use DGELSS (same `cond = eps` rule)
# instead of scipy's default DGELSD. Unset = upstream exactly as shipped.
_drv = os.environ.get("TRACE_LSTSQ_DRIVER")


def lstsq_w(a, b, *args, **kw):
    if _drv:
        kw["lapack_driver"] = _drv
    sol = orig_lstsq(a, b, *args, **kw)
    s = sol[3]
    smax = s.max() if s.size else 0.0
    near = int(np.sum((s > eps * smax * 1e-2) & (s < eps * smax * 1e2)))
    lstsq_log.append((a.shape[0], a.shape[1], int(sol[2]), float(s.min() / smax) if smax > 0 else 0.0, near))
    return sol


def _end_order():
    # the upstream acceleration bookkeeping, to log what it would decide
    o = state["order"]
    if state["found_better"]:
        state["n_discarded"] = 0
    elif o > max(2 * n_peaks, 50) and state["best_ratio"] > 0.7:
        state["n_discarded"] += 1
    order_end.append((o, int(state["found_better"]), state["n_discarded"], state["best_ratio"], state["best_quality"]))


def evaluate_w(eval_points, pole_values, residue_matrix, *a, **kw):
    res = orig_evaluate(eval_points, pole_values, residue_matrix, *a, **kw)
    if eval_points.size != ne_test:
        return res  # vectfit's own evaluate on the fit grid
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
    order = int(new_poles.size)
    if order != state["order"]:
        if state["order"] is not None:
            _end_order()
        state.update(order=order, iter=0, found_better=False)
    state["iter"] += 1
    accepted = bool(quality > state["best_quality"])
    if accepted:
        state["best_quality"], state["best_ratio"] = quality, ratio
        state["found_better"] = True
    rk = lstsq_log[0] if lstsq_log else (0, 0, -1, 0.0, 0)
    rows.append(dict(order=order, it=state["iter"], n_real=int(sum(1 for p in new_poles if p.imag == 0)),
                     ratio=float(ratio), ratio2=float(ratio2), maxre=float(maxre), quality=float(quality),
                     n_out=n_out, accepted=int(accepted), rank_id=rk[2], ncols_id=rk[1],
                     smin_over_smax=rk[3], near_thr=rk[4],
                     rank_min_all=min(x[2] for x in lstsq_log) if lstsq_log else -1,
                     near_thr_all=sum(x[4] for x in lstsq_log)))
    lstsq_log.clear()
    poles_dump.append(np.column_stack([new_poles.real, new_poles.imag]).ravel())
    return res


vf.lstsq = lstsq_w
mp.evaluate = evaluate_w
t0 = time.time()
p, r = mp._vectfit_xs(energy, ce_xs, mts)
dt = time.time() - t0
_end_order()
vf.lstsq, mp.evaluate = orig_lstsq, orig_evaluate

# result quality on the piece's own grid against the UNPERTURBED data
s = np.sqrt(energy)
sg = vf.evaluate(s, p, r * 1j) / energy
dab = np.abs(sg - ce_ref)
outside = int(np.sum(~((dab / np.abs(ce_ref) < rtol) | (dab < atol))))
keys = list(rows[0].keys())
with open(out + ".log.tsv", "w") as fh:
    fh.write("\t".join(keys) + "\n")
    for d in rows:
        fh.write("\t".join(repr(d[k]) if isinstance(d[k], float) else str(d[k]) for k in keys) + "\n")
with open(out + ".orders.tsv", "w") as fh:
    fh.write("order\tfound_better\tn_discarded\tbest_ratio_after\tbest_quality_after\n")
    for o in order_end:
        fh.write("\t".join(repr(x) if isinstance(x, float) else str(x) for x in o) + "\n")
np.concatenate(poles_dump).astype('<f8').tofile(out + ".poles.f64")
buf = [float(p.size), float(r.shape[0])] + np.column_stack([p.real, p.imag]).ravel().tolist()
buf += np.column_stack([r.real.ravel(), r.imag.ravel()]).ravel().tolist()
np.array(buf, dtype='<f8').tofile(out + ".final.f64")
orders_all = list(range(max(2, 2 * n_peaks), max(200, 4 * n_peaks) + 1, 2))
last = order_end[-1][0]
ideal = bool(rows[-1]["accepted"] and rows[-1]["ratio"] >= 1.0)
reason = "found_ideal" if ideal else ("acceleration" if last < orders_all[-1] else "orders exhausted")
summ = dict(code="openmc", input=inp, n_peaks=n_peaks, first_order=orders_all[0], last_order_in_list=orders_all[-1],
            last_order_visited=last, orders_visited=len(order_end), stop=reason, final_poles=int(p.size),
            grid_points_outside=outside, seconds=dt, best_quality=state["best_quality"], best_ratio=state["best_ratio"])
open(out + ".summary.txt", "w").write(json.dumps(summ, indent=1) + "\n")
print(json.dumps(summ), flush=True)
