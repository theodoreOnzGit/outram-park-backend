# N-4 (gh:#1130): turn the N-1 example's --axial-csv tally into the line
# model's shape table. Run: python3 -I axial_shape.py <axial.csv>
#
# Source term in the transit-time coordinate: a bin's fission per unit
# transit time, F / dt with dt = tau_k * dz / L_k (plug flow in region k).
#   - graphite region: the inner ring only (r < R_graphite); the outer ring
#     there is the gap, can and downcomer, which are other parts of the path;
#   - plenums: both rings (the salt crosses the whole vessel).
# Importance: one-group proxy, the scalar flux per unit volume inside
# r < R_graphite (flux_inner / dz). Seeds are summed. Both columns are
# normalised to a maximum of 1.
import csv, sys
TAU = {'lp': 3.8, 'core': 9.4, 'up': 3.9}   # TM-1626 Table 1
rows = list(csv.DictReader(open(sys.argv[1])))
bins = {}
for r in rows:
    key = (float(r['z_lo_cm']), float(r['z_hi_cm']))
    b = bins.setdefault(key, [0.0, 0.0, 0.0])
    b[0] += float(r['fission_inner']); b[1] += float(r['fission_outer']); b[2] += float(r['flux_inner'])
keys = sorted(bins)
z_bottom, z_top = keys[0][0], keys[-1][1]
core_top = max(k[1] for k in keys if k[1] <= 162.56 + 1e-6)
L = {'lp': -z_bottom, 'core': core_top, 'up': z_top - core_top}
out = []
for (lo, hi) in keys:
    dz = hi - lo; zc = 0.5 * (lo + hi)
    reg = 'lp' if hi <= 1e-9 else ('core' if hi <= core_top + 1e-6 else 'up')
    f_in, f_out, phi = bins[(lo, hi)]
    f = f_in if reg == 'core' else f_in + f_out
    s = f / (TAU[reg] * dz / L[reg])
    out.append((zc, s, phi / dz))
ms = max(o[1] for o in out); ma = max(o[2] for o in out)
print(f"// z_bottom {z_bottom:.4f}, z_core_top {core_top:.4f}, z_top {z_top:.4f}; seeds {sorted(set(r['seed'] for r in rows))}")
for zc, s, a in out:
    print(f"    ({zc:.4f}, {s/ms:.5f}, {a/ma:.5f}),")
