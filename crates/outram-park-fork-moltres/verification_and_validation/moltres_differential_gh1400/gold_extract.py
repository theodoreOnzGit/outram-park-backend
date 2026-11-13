"""Extract the numbers this record uses from Moltres' committed gold Exodus files.

Usage:  python3 -I gold_extract.py <moltres clone at 3dd2ce7> > moltres_gold_values.txt
Needs scipy (NetCDF3 reader; the gold files are 'NetCDF Data Format data (64-bit offset)').

Only reads files; nothing in the Moltres tree is run. Output lines:
  GOLD <file> glo <name> <value at the last stored time step, 17 sig. digits>
  GOLD <file> nod <name> <node index> <x> <y> <value>
"""
import sys
from scipy.io import netcdf_file


def text(rows):
    return [b"".join(r).decode().strip("\x00 ").strip() for r in rows]


def dump(root, rel):
    f = netcdf_file(f"{root}/tests/{rel}", mmap=False)
    v = f.variables
    if "name_glo_var" in v:
        names = text(v["name_glo_var"][:])
        last = v["vals_glo_var"][:][-1]
        for n, x in zip(names, last):
            print(f"GOLD {rel} glo {n} {x:.17g}")
    names = text(v["name_nod_var"][:])
    xs = v["coord"][:] if "coord" in v else None
    if xs is None:
        xs = [v["coordx"][:], v["coordy"][:]]
    for k, n in enumerate(names):
        vals = v[f"vals_nod_var{k + 1}"][:][-1]
        for i, val in enumerate(vals):
            print(f"GOLD {rel} nod {n} {i} {xs[0][i]:.17g} {xs[1][i]:.17g} {val:.17g}")


root = sys.argv[1]
for rel in [
    "bcs/gold/vacuum_bc_out.e",
    "bcs/gold/vacuum_bc_mark_out.e",
    "bcs/gold/vacuum_bc_milne_out.e",
    "materials/gold/gmm_none_out.e",
    "materials/gold/gmm_linear_out.e",
    "materials/gold/gmm_spline_out.e",
    "materials/gold/gmm_monotone_cubic_out.e",
]:
    dump(root, rel)
