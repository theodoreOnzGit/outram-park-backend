"""Reference run of OpenMC 8f79d9d WindowedMultipole.from_endf on one tape (gh:#1390).

Usage: python ref_u238.py <endf tape> <out dir>
Environment: njoy (NJOY2016.80) on PATH; OpenMC 8f79d9d on PYTHONPATH.

Unmodified OpenMC does the work; this driver only wraps `_vectfit_xs` to record,
per energy piece, the piece's size, a checksum of its inputs, the pole count and
the wall time, and saves the multipole data and the searched library.

Checksum (identical in the Rust test): the f64 words of [energy, row 0, row 1,
row 2 (if present)] concatenated; h_xor = XOR of all words, h_sum = sum of the
words mod 2^64, h_pos = sum of word*(index+1) mod 2^64.
"""
import sys, os, json, time, copy
import numpy as np
import openmc.data as d
from openmc.data import multipole as mp

endf_path, out_dir = sys.argv[1], sys.argv[2]
os.makedirs(out_dir, exist_ok=True)


def checksum(energy, ce_xs):
    w = np.ascontiguousarray(np.concatenate([np.asarray(energy, dtype='<f8')] +
                                            [np.asarray(r, dtype='<f8') for r in ce_xs])).view('<u8')
    idx = np.arange(1, w.size + 1, dtype=np.uint64)
    with np.errstate(over='ignore'):
        return [int(np.bitwise_xor.reduce(w)), int(w.sum(dtype=np.uint64)), int((w * idx).sum(dtype=np.uint64))]


pieces = []
upstream_vectfit_xs = mp._vectfit_xs


def wrap(energy, ce_xs, mts, **kw):
    t = time.time()
    p, r = upstream_vectfit_xs(energy, ce_xs, mts, **kw)
    dt = time.time() - t
    pieces.append({"n_points": int(energy.size), "e_first": float(energy[0]).hex(), "e_last": float(energy[-1]).hex(),
                   "mts": list(mts), "checksum": checksum(energy, ce_xs), "n_poles": int(p.size), "seconds": dt})
    print(f"piece {len(pieces)} points {energy.size} poles {p.size} seconds {dt:.1f}", flush=True)
    json.dump(pieces, open(os.path.join(out_dir, "pieces.json"), "w"))
    return p, r


mp._vectfit_xs = wrap
t0 = time.time()
mp_data = mp.vectfit_nuclide(endf_path)
t1 = time.time()
print("vectfit_nuclide seconds", t1 - t0, flush=True)

# multipole data as little-endian f64: n_pieces; per piece n_poles, n_mt, poles (re, im),
# residues [mt][pole] (re, im)
buf = [float(len(mp_data["poles"]))]
for p, r in zip(mp_data["poles"], mp_data["residues"]):
    buf += [float(p.size), float(r.shape[0])]
    buf += np.column_stack([p.real, p.imag]).ravel().tolist()
    buf += np.column_stack([r.real.ravel(), r.imag.ravel()]).ravel().tolist()
np.array(buf, dtype='<f8').tofile(os.path.join(out_dir, "mp_data.f64"))
json.dump({"name": mp_data["name"], "AWR": float(mp_data["AWR"]).hex(), "E_min": float(mp_data["E_min"]).hex(),
           "E_max": float(mp_data["E_max"]).hex(), "seconds": t1 - t0}, open(os.path.join(out_dir, "mp_meta.json"), "w"))

wmp = d.WindowedMultipole.from_multipole(copy.deepcopy(mp_data))
t2 = time.time()
print("from_multipole(search) seconds", t2 - t1, flush=True)
lib = [wmp.spacing, wmp.sqrtAWR, wmp.E_min, wmp.E_max, float(wmp.n_poles), float(wmp.data.shape[1]),
       float(wmp.n_windows), float(wmp.fit_order)]
lib += np.column_stack([wmp.data.real.ravel(), wmp.data.imag.ravel()]).ravel().tolist()
lib += np.asarray(wmp.windows, dtype=float).ravel().tolist()
lib += [float(x) for x in wmp.curvefit.shape]
lib += wmp.curvefit.ravel().tolist()
lib += np.asarray(wmp.broaden_poly, dtype=float).ravel().tolist()
np.array(lib, dtype='<f8').tofile(os.path.join(out_dir, "library.f64"))
json.dump({"n_poles": int(wmp.n_poles), "n_windows": int(wmp.n_windows), "fit_order": int(wmp.fit_order),
           "spacing": float(wmp.spacing).hex(), "from_multipole_seconds": t2 - t1,
           "n_pieces": len(pieces), "total_seconds": t2 - t0},
          open(os.path.join(out_dir, "library_meta.json"), "w"))
wmp.export_to_hdf5(os.path.join(out_dir, "library.h5"), mode="w")
print("done: poles", wmp.n_poles, "windows", wmp.n_windows, "fit_order", wmp.fit_order, "total seconds", t2 - t0, flush=True)
