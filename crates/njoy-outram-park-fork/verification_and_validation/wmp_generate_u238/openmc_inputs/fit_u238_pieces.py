"""OpenMC 8f79d9d's `_vectfit_xs`, unmodified, on captured piece inputs (gh:#1390 extension).

Usage: python fit_u238_pieces.py <capture dir> <out dir> <piece indices, comma separated>
Environment: OpenMC 8f79d9d on PYTHONPATH. Writes <out dir>/piece_<i>.f64 in the format of
ref_u238_subset.py (i, n_poles, n_mts, poles re/im, residues re/im) and appends to <out dir>/fit_log.txt.
The call is `_vectfit_xs(energy, ce_xs, mts, log=False, **kwargs)` exactly as `vectfit_nuclide` makes it
(kwargs empty: upstream defaults rtol 1e-3, atol 1e-5, n_vf_iter 30).
"""
import sys, os, time, json
import numpy as np
from openmc.data import multipole as mp

cap, out, which = sys.argv[1], sys.argv[2], [int(x) for x in sys.argv[3].split(",")]
os.makedirs(out, exist_ok=True)
kw = json.load(open(os.path.join(cap, "capture.json")))["kwargs"]
for i in which:
    d = np.load(os.path.join(cap, f"in_{i}.npz"))
    e, ce, mts = d["energy"], d["ce_xs"], [int(m) for m in d["mts"]]
    k = {a: b for a, b in kw[str(i)].items() if a != "log"}
    t = time.time()
    p, r = mp._vectfit_xs(e, ce, mts, log=False, **k)
    dt = time.time() - t
    buf = [float(i), float(p.size), float(r.shape[0])]
    buf += np.column_stack([p.real, p.imag]).ravel().tolist()
    buf += np.column_stack([r.real.ravel(), r.imag.ravel()]).ravel().tolist()
    np.array(buf, dtype='<f8').tofile(os.path.join(out, f"piece_{i}.f64"))
    line = f"piece {i} FITTED points {e.size} poles {p.size} seconds {dt:.1f} kwargs {k}"
    print(line, flush=True)
    open(os.path.join(out, "fit_log.txt"), "a").write(line + "\n")
