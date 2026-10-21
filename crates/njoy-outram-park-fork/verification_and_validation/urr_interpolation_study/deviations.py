"""Relative deviation of NJOY's MF=3 MT=18 from this crate's kernel, per
refinement level, at full precision.

Reads `ours_mt18.csv` (produced by `cargo run --example urr_kernel_dump`)
rather than values rounded to NJOY's seven printed figures -- rounding the
comparison arm reports several deviations as exactly zero when they are
4e-10 to 4e-7.

Usage:  python3 deviations.py <base.pendf> <x2> <x4> <x8> <x16>
Emits:  deviations.csv  (energy_ev, level, njoy_b, ours_b, rel_dev)
"""
import sys, csv, os

def pf(t):
    t = t.strip()
    if not t: return 0.0
    for i in range(1, len(t)):
        if t[i] in "+-" and t[i-1] not in "eE":
            return float(t[:i] + "e" + t[i:])
    return float(t)

def mf3(path, mat, mt):
    rows = [l for l in open(path).read().split("\n")
            if len(l) >= 75 and l[66:70].strip() == str(mat)
            and l[70:72].strip() == "3" and l[72:75].strip() == str(mt)]
    if not rows: return {}
    head = rows[1]
    nr = int(head[44:55]); np_ = int(head[55:66])
    ni = (2*nr + 5)//6
    vals = []
    for l in rows[2+ni:]:
        for k in range(6):
            f = l[k*11:(k+1)*11]
            if f.strip(): vals.append(pf(f))
    return {round(vals[i], 6): vals[i+1] for i in range(0, min(len(vals), 2*np_), 2)}

here = os.path.dirname(os.path.abspath(__file__))
ours = {}
with open(os.path.join(here, "ours_mt18.csv")) as fh:
    for row in fh:
        if row.startswith("#") or row.startswith("energy"): continue
        e, s = row.strip().split(",")
        ours[round(float(e), 6)] = float(s)

levels = ["base", "x2", "x4", "x8", "x16"]
tapes = dict(zip(levels, sys.argv[1:6]))
out = []
for lvl in levels:
    d = mf3(tapes[lvl], 9225, 18)
    for k, o in sorted(ours.items()):
        if k in d:
            out.append((k, lvl, d[k], o, (d[k] - o)/o))

with open(os.path.join(here, "deviations.csv"), "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["energy_ev", "level", "njoy_b", "ours_b", "rel_dev"])
    for r in out: w.writerow([f"{r[0]:.6e}", r[1], f"{r[2]:.7e}", f"{r[3]:.12e}", f"{r[4]:.6e}"])

print(f"{'E (eV)':>12} " + " ".join(f"{l:>11}" for l in levels))
for k in sorted(ours):
    row = {lvl: dv for (e, lvl, _, _, dv) in out if e == k}
    print(f"{k:12.5e} " + " ".join(f"{row[l]:11.2e}" if l in row else f"{'-':>11}" for l in levels))
