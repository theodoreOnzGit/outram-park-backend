"""OpenMC 8f79d9d windowing (`from_multipole(search=True)`) of the contiguous prefix 0..K-1 of U-238 (gh:#1390 extension).

Usage: python window_prefix_u238.py <meta.txt> <out dir> <K> <dir with piece_<i>.f64 ...> [more dirs]
Environment: OpenMC 8f79d9d on PYTHONPATH.
Pieces 0..K-1 are read from the given directories (first that has the file); the prefix library covers
the K pieces over their own energy range, E_max = (sqrt(E_min) + K w)^2 with w = (sqrt(E_max_full) -
sqrt(E_min)) / 304, exactly as ref_u238_subset.py. Writes library_prefix.f64 (same layout) and
library_prefix_meta.json into <out dir>.
"""
import sys, os, json, time, copy
from math import sqrt
import numpy as np
import openmc.data as d

meta_txt, out, K, dirs = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4:]
meta = {}
for l in open(meta_txt):
    if not l.startswith('#') and l.strip():
        k, v = l.split()
        meta[k] = float(v)
os.makedirs(out, exist_ok=True)


def load_piece(i):
    for dd in dirs:
        f = os.path.join(dd, f"piece_{i}.f64")
        if os.path.exists(f):
            v = np.fromfile(f, dtype='<f8')
            assert int(v[0]) == i
            n, nmt = int(v[1]), int(v[2])
            c = v[3:].reshape(-1, 2)
            return c[:n, 0] + 1j * c[:n, 1], (c[n:, 0] + 1j * c[n:, 1]).reshape(nmt, n)
    raise FileNotFoundError(i)


width = (sqrt(meta["e_max_full"]) - sqrt(meta["e_min"])) / meta["n_pieces"]
e_max_prefix = (sqrt(meta["e_min"]) + K * width) ** 2
pr = [load_piece(i) for i in range(K)]
prefix = {"name": "U238", "AWR": meta["awr"], "E_min": meta["e_min"], "E_max": e_max_prefix,
          "poles": [p.copy() for p, _ in pr], "residues": [r.copy() for _, r in pr]}
t2 = time.time()
wmp = d.WindowedMultipole.from_multipole(copy.deepcopy(prefix))
t3 = time.time()
print("from_multipole(search) on the prefix seconds", t3 - t2, flush=True)
lib = [wmp.spacing, wmp.sqrtAWR, wmp.E_min, wmp.E_max, float(wmp.n_poles), float(wmp.data.shape[1]),
       float(wmp.n_windows), float(wmp.fit_order)]
lib += np.column_stack([wmp.data.real.ravel(), wmp.data.imag.ravel()]).ravel().tolist()
lib += np.asarray(wmp.windows, dtype=float).ravel().tolist()
lib += [float(x) for x in wmp.curvefit.shape]
lib += wmp.curvefit.ravel().tolist()
lib += np.asarray(wmp.broaden_poly, dtype=float).ravel().tolist()
np.array(lib, dtype='<f8').tofile(os.path.join(out, "library_prefix.f64"))
json.dump({"K": K, "E_max_prefix": float(e_max_prefix).hex(), "n_poles": int(wmp.n_poles), "n_windows": int(wmp.n_windows),
           "fit_order": int(wmp.fit_order), "spacing": float(wmp.spacing).hex(), "from_multipole_seconds": t3 - t2},
          open(os.path.join(out, "library_prefix_meta.json"), "w"))
print("done: prefix K", K, "e_max", e_max_prefix, "poles", wmp.n_poles, "windows", wmp.n_windows, "fit_order", wmp.fit_order, flush=True)
