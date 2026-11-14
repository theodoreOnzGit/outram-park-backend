"""Upstream's own round-off sensitivity (gh:#1390): rerun OpenMC's stages on
inputs perturbed by ~1 ulp and measure how far OpenMC moves itself."""
import sys, json, copy
import numpy as np
from openmc.data import multipole as mp
from openmc.data.vectfit import vectfit

rec = json.load(open(sys.argv[1]))
fh = float.fromhex
arr = lambda d: np.array([fh(x) for x in d["v"]]).reshape(d["shape"])
carr = lambda d: (np.array([fh(x) for x in d["re"]]) + 1j * np.array([fh(x) for x in d["im"]])).reshape(d["shape"])
rng = np.random.default_rng(7)
ulp = lambda a: a * (1 + rng.choice([-1.0, 1.0], a.shape) * 2.0**-52)


def match(o, t):
    idx = [np.argmin(np.abs(o - x)) for x in t]
    return idx, max(np.abs(o[idx] - t) / np.abs(t))


fv = rec["first_vectfit"]
f, w, s = arr(fv["f"]), arr(fv["weight"]), np.sqrt(arr(rec["vectfit_xs_inputs"][0]["energy"]))
p_in = carr(fv["poles_in"])
p0, r0, *_ = vectfit(f, s, p_in, w)
for trial in range(3):
    p1, r1, *_ = vectfit(ulp(f), s, p_in, w)
    idx, dp = match(p1, p0)
    dr = np.max(np.abs(r1[:, idx] - r0) / np.where(np.abs(r0) > 0, np.abs(r0), 1))
    print(f"one vectfit, f perturbed 1 ulp: poles move {dp:.3e}, residues {dr:.3e} relative")
k = np.argmax(np.max(np.abs(r1[:, idx] - r0) / np.where(np.abs(r0) > 0, np.abs(r0), 1), axis=0))
print("worst residue at pole", p0[k], "|r| =", np.abs(r0[:, k]))

inp = rec["vectfit_xs_inputs"][0]
e, ce = arr(inp["energy"]), arr(inp["ce_xs"])
P0 = carr(rec["vectfit_xs_outputs"][0]["poles"])
R0 = carr(rec["vectfit_xs_outputs"][0]["residues"])
P1, R1 = mp._vectfit_xs(e, ulp(ce), [2, 27])
if P1.size == P0.size:
    idx, dp = match(P1, P0)
    dr = np.max(np.abs(R1[:, idx] - R0) / np.abs(R0))
    print(f"_vectfit_xs, xs perturbed 1 ulp: {P1.size} poles, poles move {dp:.3e}, residues {dr:.3e}")
else:
    print("_vectfit_xs, xs perturbed 1 ulp: pole count", P1.size, "vs", P0.size)

mpd = {"name": "Mg26", "AWR": fh(rec["mp_data"]["AWR"]), "E_min": fh(rec["mp_data"]["E_min"]),
       "E_max": fh(rec["mp_data"]["E_max"]), "poles": [P0], "residues": [R0]}
w0 = mp._windowing(copy.deepcopy(mpd), n_cf=5, n_win=100)
mpd2 = copy.deepcopy(mpd)
mpd2["residues"] = [ulp(R0.real) + 1j * ulp(R0.imag)]
w1 = mp._windowing(mpd2, n_cf=5, n_win=100)
same = np.array_equal(w0.windows, w1.windows)
sc = np.max(np.abs(w0.curvefit), axis=1, keepdims=True)
print(f"windowing n_cf=5, residues perturbed 1 ulp: windows same {same}, curve fit moves {np.max(np.abs(w1.curvefit - w0.curvefit) / sc):.3e} of scale")
