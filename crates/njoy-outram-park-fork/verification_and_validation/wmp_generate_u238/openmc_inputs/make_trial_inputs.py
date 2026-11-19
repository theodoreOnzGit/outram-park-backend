"""Piece input files for the order-search diagnosis (gh:#1390, 2026-11-19).

Usage: python make_trial_inputs.py <capture dir> <piece> <n_trials> <out dir>
Reads OpenMC's captured input to `_vectfit_xs` (capture_u238_inputs.py) and writes
  <out>/in_<piece>_t0.f64                 the unperturbed input,
  <out>/in_<piece>_t<k>.f64  (k = 1..n)  every cross-section value times (1 +- 2^-52), the sign drawn
                                          per value from numpy default_rng(1000 + k); energy untouched.
Layout (little-endian f64): ne, nmt, energy[ne], then nmt rows of ne values.
BOTH codes read these same files, so a trial is the same input for OpenMC and for the port.
"""
import sys, os
import numpy as np

cap, piece, ntr, out = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
os.makedirs(out, exist_ok=True)
d = np.load(os.path.join(cap, f"in_{piece}.npz"))
e, ce = d["energy"], d["ce_xs"]


def write(name, x):
    np.concatenate([[float(e.size), float(x.shape[0])], e, x.ravel()]).astype('<f8').tofile(os.path.join(out, name))


write(f"in_{piece}_t0.f64", ce)
for k in range(1, ntr + 1):
    rng = np.random.default_rng(1000 + k)
    sign = rng.choice([-1.0, 1.0], ce.shape)
    write(f"in_{piece}_t{k}.f64", ce * (1 + sign * 2.0**-52))
print("mts", [int(m) for m in d["mts"]], "points", e.size)
