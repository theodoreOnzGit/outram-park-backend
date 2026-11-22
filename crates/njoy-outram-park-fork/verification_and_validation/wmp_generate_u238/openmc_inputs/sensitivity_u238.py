"""OpenMC's own round-off sensitivity on U-238 pieces (gh:#1390): rerun upstream's
`_vectfit_xs` on a piece's input perturbed by about 1 ulp (random sign per value)
and measure how far OpenMC moves itself, with the same metrics the Rust test
uses for port against OpenMC.

Usage: python sensitivity_u238.py <endf tape> <run dir with piece_<i>.f64> <trials> <piece indices, comma separated>
Environment: njoy on PATH; OpenMC 8f79d9d on PYTHONPATH.
Unmodified OpenMC runs `vectfit_nuclide` (NJOY, front end, slicing); `_vectfit_xs` is
wrapped to capture the chosen pieces' inputs and to return a stand-in for all
other pieces. The baseline for each chosen piece is the fit already saved by
ref_u238_subset.py (same inputs, same code, so it must reproduce; checked here
by the first trial, which is unperturbed).
"""
import sys, time
import numpy as np
from openmc.data import multipole as mp
from openmc.data.vectfit import evaluate

endf_path, run_dir, trials, which = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
chosen = {int(x) for x in which.split(",")}
rng = np.random.default_rng(7)
ulp = lambda a: a * (1 + rng.choice([-1.0, 1.0], a.shape) * 2.0**-52)


def load_piece(i):
    v = np.fromfile(f"{run_dir}/piece_{i}.f64", dtype='<f8')
    n, nmt = int(v[1]), int(v[2])
    c = v[3:].reshape(-1, 2)
    p = c[:n, 0] + 1j * c[:n, 1]
    r = (c[n:, 0] + 1j * c[n:, 1]).reshape(nmt, n)
    return p, r


def match(o, t):
    idx = [int(np.argmin(np.abs(o - x))) for x in t]
    return idx, float(np.max(np.abs(o[idx] - t) / np.abs(t)))


captured = {}
upstream = mp._vectfit_xs
count = [0]


def wrap(energy, ce_xs, mts, **kw):
    i = count[0]
    count[0] += 1
    if i in chosen:
        captured[i] = (energy.copy(), ce_xs.copy(), list(mts))
    return np.zeros(0, dtype=complex), np.zeros((len(mts), 0), dtype=complex)


mp._vectfit_xs = wrap
mp.vectfit_nuclide(endf_path)
mp._vectfit_xs = upstream
for i in sorted(captured):
    e, ce, mts = captured[i]
    p0, r0 = load_piece(i)
    s = np.sqrt(e)
    sig0 = evaluate(s, p0, r0 * 1j) / e
    for t in range(trials):
        x = ce if t == 0 else ulp(ce)
        t0 = time.time()
        p1, r1 = upstream(e, x, mts)
        dt = time.time() - t0
        if p1.size != p0.size:
            print(f"piece {i} trial {t} ({'unperturbed' if t == 0 else 'xs +-1 ulp'}): pole count {p1.size} vs saved {p0.size}, {dt:.0f} s", flush=True)
            continue
        idx, dp = match(p1, p0)
        nz = np.abs(r0) > 0
        dr = float(np.max(np.abs(r1[:, idx] - r0)[nz] / np.abs(r0)[nz]))
        sig1 = evaluate(s, p1, r1 * 1j) / e
        ab = np.abs(sig1 - sig0)
        rel = float(np.max((ab / np.abs(sig0))[ab > 1e-5])) if np.any(ab > 1e-5) else 0.0
        print(f"piece {i} trial {t} ({'unperturbed' if t == 0 else 'xs +-1 ulp'}): {p1.size} poles, poles move {dp:.3e}, "
              f"residues {dr:.3e} relative; sigma worst {ab.max():.3e} b absolute, {rel:.3e} relative above atol; {dt:.0f} s", flush=True)
