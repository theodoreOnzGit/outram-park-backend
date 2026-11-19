"""Extension fixtures (gh:#1390): OpenMC's fits of the new pieces into the committed fixture files.

Usage: python to_fixture_ext_u238.py <fixture dir> <run dir with piece_<i>.f64 of the new fits>
- rewrites the `n_poles` and `fitted` columns of the new pieces' rows in <fixture dir>/pieces.txt
  (the checksum and energy columns are untouched, they come from the front end);
- writes <fixture dir>/fitted_pieces_ext.f64: the new pieces' `piece_<i>.f64` records concatenated in
  index order (same record layout as fitted_pieces.f64).
"""
import sys, os, glob, re
import numpy as np

fx, run = sys.argv[1], sys.argv[2]
files = sorted(glob.glob(os.path.join(run, "piece_*.f64")), key=lambda x: int(re.search(r"piece_(\d+)", x).group(1)))
new = {}
for f in files:
    v = np.fromfile(f, dtype='<f8')
    new[int(v[0])] = (int(v[1]), f)
lines = open(os.path.join(fx, "pieces.txt")).read().split("\n")
out, idx = [], 0
for l in lines:
    if l.startswith("#") or not l.strip():
        out.append(l)
        continue
    w = l.split()
    if idx in new:
        assert w[7] == "0", f"piece {idx} already fitted"
        w[1], w[7] = str(new[idx][0]), "1"
    out.append(" ".join(w))
    idx += 1
open(os.path.join(fx, "pieces.txt"), "w").write("\n".join(out))
with open(os.path.join(fx, "fitted_pieces_ext.f64"), "wb") as g:
    for i in sorted(new):
        g.write(open(new[i][1], "rb").read())
print("new pieces", sorted(new))
