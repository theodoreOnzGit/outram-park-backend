"""Convert ref.py's JSON dump into the line-based fixture the Rust test reads."""
import json, sys

rec = json.load(open(sys.argv[1]))
out = open(sys.argv[2], "w")
fh = float.fromhex


def put(key, vals):
    out.write(key + " " + str(len(vals)) + " " + " ".join(repr(v) for v in vals) + "\n")


def real(key, d):
    put(key, [fh(x) for x in d["v"]])


def cplx(key, d):
    put(key + ".re", [fh(x) for x in d["re"]])
    put(key + ".im", [fh(x) for x in d["im"]])
    put(key + ".shape", [float(x) for x in d["shape"]])


out.write("# OpenMC 8f79d9d WindowedMultipole.from_endf reference, gh:#1390. See README.md.\n")
mp = rec["mp_data"]
out.write("name 1 " + mp["name"] + "\n")
put("awr", [fh(mp["AWR"])])
put("e_min", [fh(mp["E_min"])])
put("e_max", [fh(mp["E_max"])])
inp = rec["vectfit_xs_inputs"][0]
real("energy", inp["energy"])
real("ce_xs", inp["ce_xs"])
put("n_pieces", [float(len(rec["vectfit_xs_inputs"]))])
fv = rec["first_vectfit"]
real("vf1.f", fv["f"])
real("vf1.weight", fv["weight"])
cplx("vf1.poles_in", fv["poles_in"])
cplx("vf1.poles_out", fv["poles_out"])
cplx("vf1.residues", fv["residues"])
o = rec["vectfit_xs_outputs"][0]
cplx("mp.poles", o["poles"])
cplx("mp.residues", o["residues"])
for tag in ("wmp", "wmp_fixed"):
    w = rec[tag]
    put(tag + ".spacing", [fh(w["spacing"])])
    cplx(tag + ".data", w["data"])
    put(tag + ".windows", [float(x) for p in w["windows"] for x in p])
    real(tag + ".curvefit", w["curvefit"])
    put(tag + ".curvefit_shape", [float(x) for x in w["curvefit"]["shape"]])
out.close()
