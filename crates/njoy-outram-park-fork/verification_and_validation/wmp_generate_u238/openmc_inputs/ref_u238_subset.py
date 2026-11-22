"""Reference run of OpenMC 8f79d9d on U-238, fitting a stated SUBSET of the pieces (gh:#1390).

Usage: python ref_u238_subset.py <endf tape> <out dir> <prefix K> <extra pieces, comma separated>
Environment: njoy (NJOY2016.80) on PATH; OpenMC 8f79d9d on PYTHONPATH.

Why a subset: the full `vectfit_nuclide` on this tape was started with
`ref_u238.py` and measured at ~300 s per piece for the first 12 of 304
pieces (see the README), about 25 h in total, which does not fit the session.

What is real and what is a stand-in:
  * Unmodified OpenMC runs `vectfit_nuclide`: NJOY, the front end, the piece
    boundaries and the slicing of all 304 pieces are upstream's.
  * `_vectfit_xs` is wrapped. For every piece it records the size and a
    checksum of the inputs it was given (the front-end gate covers all 304).
    For pieces 0..K-1 and the extra pieces it calls upstream's `_vectfit_xs`
    unchanged and saves poles and residues at once. For every other piece it
    returns an empty stand-in (0 poles) WITHOUT fitting.
  * The prefix library: the first K pieces' multipole data, over the energy
    range of those K pieces (piece width of the full 304-piece split), is
    given to upstream's `WindowedMultipole.from_multipole(search=True)` and
    saved. That is a library of the lowest K pieces only.

Checksum (identical in the Rust test): the f64 words of [energy, row 0, row 1,
row 2] concatenated; h_xor = XOR of all words, h_sum = sum of the words mod
2^64, h_pos = sum of word*(index+1) mod 2^64.
"""
import sys, os, json, time, copy
from math import sqrt
import numpy as np
import openmc.data as d
from openmc.data import multipole as mp

endf_path, out_dir, K, extra = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
fit_set = set(range(K)) | {int(x) for x in extra.split(",") if x}
os.makedirs(out_dir, exist_ok=True)


def checksum(energy, ce_xs):
    w = np.ascontiguousarray(np.concatenate([np.asarray(energy, dtype='<f8')] +
                                            [np.asarray(r, dtype='<f8') for r in ce_xs])).view('<u8')
    idx = np.arange(1, w.size + 1, dtype=np.uint64)
    with np.errstate(over='ignore'):
        return [int(np.bitwise_xor.reduce(w)), int(w.sum(dtype=np.uint64)), int((w * idx).sum(dtype=np.uint64))]


def write_piece(i, p, r):
    buf = [float(i), float(p.size), float(r.shape[0])]
    buf += np.column_stack([p.real, p.imag]).ravel().tolist()
    buf += np.column_stack([r.real.ravel(), r.imag.ravel()]).ravel().tolist()
    np.array(buf, dtype='<f8').tofile(os.path.join(out_dir, f"piece_{i}.f64"))


pieces = []
fitted = {}
upstream_vectfit_xs = mp._vectfit_xs


def wrap(energy, ce_xs, mts, **kw):
    i = len(pieces)
    rec = {"index": i, "n_points": int(energy.size), "e_first": float(energy[0]).hex(),
           "e_last": float(energy[-1]).hex(), "mts": list(mts), "checksum": checksum(energy, ce_xs)}
    if i in fit_set:
        t = time.time()
        p, r = upstream_vectfit_xs(energy, ce_xs, mts, **kw)
        rec.update(fitted=True, n_poles=int(p.size), seconds=time.time() - t)
        write_piece(i, p, r)
        fitted[i] = (p, r)
        print(f"piece {i} FITTED points {energy.size} poles {p.size} seconds {rec['seconds']:.1f}", flush=True)
    else:
        p, r = np.zeros(0, dtype=complex), np.zeros((len(mts), 0), dtype=complex)
        rec.update(fitted=False, n_poles=0, seconds=0.0)
    pieces.append(rec)
    json.dump(pieces, open(os.path.join(out_dir, "pieces.json"), "w"))
    return p, r


mp._vectfit_xs = wrap
t0 = time.time()
mp_data = mp.vectfit_nuclide(endf_path)
t1 = time.time()
n_pieces = len(pieces)
print("vectfit_nuclide seconds (fits of the chosen pieces only)", t1 - t0, "pieces", n_pieces, flush=True)

# prefix multipole data: the first K pieces over their own share of the 304-piece split
width = (sqrt(mp_data["E_max"]) - sqrt(mp_data["E_min"])) / n_pieces
e_max_prefix = (sqrt(mp_data["E_min"]) + K * width) ** 2
prefix = {"name": mp_data["name"], "AWR": mp_data["AWR"], "E_min": mp_data["E_min"], "E_max": e_max_prefix,
          "poles": [fitted[i][0].copy() for i in range(K)], "residues": [fitted[i][1].copy() for i in range(K)]}
json.dump({"name": mp_data["name"], "AWR": float(mp_data["AWR"]).hex(), "E_min": float(mp_data["E_min"]).hex(),
           "E_max_full": float(mp_data["E_max"]).hex(), "E_max_prefix": float(e_max_prefix).hex(), "K": K,
           "n_pieces": n_pieces, "fit_seconds": t1 - t0}, open(os.path.join(out_dir, "meta.json"), "w"))
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
np.array(lib, dtype='<f8').tofile(os.path.join(out_dir, "library_prefix.f64"))
json.dump({"n_poles": int(wmp.n_poles), "n_windows": int(wmp.n_windows), "fit_order": int(wmp.fit_order),
           "spacing": float(wmp.spacing).hex(), "from_multipole_seconds": t3 - t2},
          open(os.path.join(out_dir, "library_prefix_meta.json"), "w"))
print("done: prefix poles", wmp.n_poles, "windows", wmp.n_windows, "fit_order", wmp.fit_order, flush=True)
