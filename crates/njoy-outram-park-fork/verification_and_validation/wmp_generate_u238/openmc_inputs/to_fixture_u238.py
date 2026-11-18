"""Turn ref_u238_subset.py's output directory into the committed fixtures (gh:#1390).

Usage: python to_fixture_u238.py <run dir> <fixture dir>
Writes in <fixture dir>:
  pieces.txt          one line per piece: n_points n_poles h_xor h_sum h_pos e_first e_last fitted(0/1)
  meta.txt            key value lines: K awr e_min e_max_prefix e_max_full n_pieces
  fitted_pieces.f64   the piece_<i>.f64 files of the fitted pieces, concatenated in index order
  library_prefix.f64  OpenMC's searched library of the lowest K pieces
Floats are shortest round-trip decimals in the text files, so nothing is lost.
"""
import sys, os, json, glob, re, shutil

run, out = sys.argv[1], sys.argv[2]
pieces = json.load(open(os.path.join(run, "pieces.json")))
with open(os.path.join(out, "pieces.txt"), "w") as f:
    f.write("# OpenMC 8f79d9d _vectfit_xs per-piece record for U-238 (gh:#1390); columns: "
            "n_points n_poles h_xor h_sum h_pos e_first e_last fitted\n")
    for p in pieces:
        c = p["checksum"]
        f.write(f"{p['n_points']} {p['n_poles']} {c[0]} {c[1]} {c[2]} "
                f"{float.fromhex(p['e_first'])!r} {float.fromhex(p['e_last'])!r} {1 if p['fitted'] else 0}\n")

m = json.load(open(os.path.join(run, "meta.json")))
with open(os.path.join(out, "meta.txt"), "w") as f:
    f.write("# U-238 subset run (gh:#1390)\n")
    f.write(f"K {m['K']}\nawr {float.fromhex(m['AWR'])!r}\ne_min {float.fromhex(m['E_min'])!r}\n"
            f"e_max_prefix {float.fromhex(m['E_max_prefix'])!r}\ne_max_full {float.fromhex(m['E_max_full'])!r}\n"
            f"n_pieces {m['n_pieces']}\n")

files = sorted(glob.glob(os.path.join(run, "piece_*.f64")), key=lambda x: int(re.search(r"piece_(\d+)", x).group(1)))
with open(os.path.join(out, "fitted_pieces.f64"), "wb") as f:
    for p in files:
        f.write(open(p, "rb").read())
shutil.copy(os.path.join(run, "library_prefix.f64"), os.path.join(out, "library_prefix.f64"))
print("pieces", len(pieces), "fitted files", len(files))
