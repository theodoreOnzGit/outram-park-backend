"""Reference run of OpenMC WindowedMultipole.from_endf, dumping intermediates (gh:#1390)."""
import sys, json, time, copy
import numpy as np
import openmc.data as d
from openmc.data import multipole as mp

endf_path, out_path = sys.argv[1], sys.argv[2]
rec = {"vectfit_xs_inputs": [], "vectfit_xs_outputs": [], "first_vectfit": None}
orig_vxs = mp._vectfit_xs
orig_vf = mp.vectfit


def hx(a):
    a = np.asarray(a)
    if np.iscomplexobj(a):
        return {"re": [float(x).hex() for x in a.real.ravel()],
                "im": [float(x).hex() for x in a.imag.ravel()], "shape": list(a.shape)}
    return {"v": [float(x).hex() for x in a.ravel()], "shape": list(a.shape)}


def vf_wrap(f, s, poles, weight, **kw):
    out = orig_vf(f, s, poles, weight, **kw)
    if rec["first_vectfit"] is None:
        rec["first_vectfit"] = {"f": hx(f), "s": hx(s), "poles_in": hx(poles), "weight": hx(weight),
                                "poles_out": hx(out[0]), "residues": hx(out[1]), "rms": float(out[4]).hex()}
    return out


def vxs_wrap(energy, ce_xs, mts, **kw):
    rec["vectfit_xs_inputs"].append({"energy": hx(energy), "ce_xs": hx(ce_xs), "mts": list(mts)})
    t = time.time()
    p, r = orig_vxs(energy, ce_xs, mts, **kw)
    rec["vectfit_xs_outputs"].append({"poles": hx(p), "residues": hx(r), "seconds": time.time() - t})
    print("piece done", p.size, time.time() - t, flush=True)
    return p, r


mp._vectfit_xs = vxs_wrap
mp.vectfit = vf_wrap
t0 = time.time()
mp_data = mp.vectfit_nuclide(endf_path)
t1 = time.time()
rec["mp_data"] = {"name": mp_data["name"], "AWR": float(mp_data["AWR"]).hex(),
                  "E_min": float(mp_data["E_min"]).hex(), "E_max": float(mp_data["E_max"]).hex()}
json.dump(rec, open(out_path, "w"))
wmp = d.WindowedMultipole.from_multipole(copy.deepcopy(mp_data))
t2 = time.time()
rec["wmp"] = {"spacing": float(wmp.spacing).hex(), "sqrtAWR": float(wmp.sqrtAWR).hex(),
              "E_min": float(wmp.E_min).hex(), "E_max": float(wmp.E_max).hex(), "data": hx(wmp.data),
              "windows": wmp.windows.tolist(), "curvefit": hx(wmp.curvefit), "n_poles": int(wmp.n_poles),
              "n_windows": int(wmp.n_windows), "fit_order": int(wmp.fit_order)}
w2 = d.WindowedMultipole.from_multipole(copy.deepcopy(mp_data), search=False, n_win=100, n_cf=5)
rec["wmp_fixed"] = {"spacing": float(w2.spacing).hex(), "data": hx(w2.data),
                    "windows": w2.windows.tolist(), "curvefit": hx(w2.curvefit)}
rec["timing"] = {"vectfit_nuclide": t1 - t0, "from_multipole": t2 - t1}
json.dump(rec, open(out_path, "w"))
wmp.export_to_hdf5(out_path + ".h5", mode="w")
print("done", rec["timing"], "poles", wmp.n_poles, "windows", wmp.n_windows, "fit_order", wmp.fit_order)
