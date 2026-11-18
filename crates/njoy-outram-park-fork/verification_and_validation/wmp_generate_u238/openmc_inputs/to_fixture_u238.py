"""Turn ref_u238.py's pieces.json into the committed text fixture pieces.txt (gh:#1390).

Usage: python to_fixture_u238.py <pieces.json> <pieces.txt>
One line per piece: n_points n_poles h_xor h_sum h_pos e_first e_last
(floats as shortest round-trip decimals, so no precision is lost).
"""
import sys, json

pieces = json.load(open(sys.argv[1]))
with open(sys.argv[2], "w") as out:
    out.write("# OpenMC 8f79d9d _vectfit_xs per-piece record for U-238 (gh:#1390); columns: "
              "n_points n_poles h_xor h_sum h_pos e_first e_last\n")
    for p in pieces:
        c = p["checksum"]
        out.write(f"{p['n_points']} {p['n_poles']} {c[0]} {c[1]} {c[2]} "
                  f"{float.fromhex(p['e_first'])!r} {float.fromhex(p['e_last'])!r}\n")
