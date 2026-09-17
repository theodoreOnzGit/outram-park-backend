"""Generate the Shannon-entropy code-to-code reference from OpenMC.

Committed per the workspace rule that every V&V running OpenMC must embed the
deck that produced the reference.

Model: bare Godiva HEU sphere (ICSBEP HEU-MET-FAST-001), with a 5x5x5 regular
entropy mesh sized to the sphere's bounding box so that fission sites land
near - and occasionally exactly on - the mesh faces. That is deliberate: the
lower-face boundary branch of RegularMesh::get_index_in_direction is the part
of the port most easily got wrong.

Emits entropy_reference.json holding
  - the mesh definition,
  - OpenMC's own per-generation Shannon entropy (statepoint 'entropy'),
  - the full final source bank (r, wgt) at full double precision,
so the Rust port can be run on the IDENTICAL bank rather than on a bank it
generated itself, which would conflate transport differences with entropy
differences.

Run:
    source ~/Documents/research/.venv-openmc/bin/activate
    OPENMC_CROSS_SECTIONS=~/Documents/research/endfb-viii.0-hdf5/cross_sections.xml \
        python entropy_reference.py
"""
import json, os, math
import numpy as np
import openmc

R = 8.7407  # cm, Godiva critical radius

hue = openmc.Material(name="HEU")
hue.add_nuclide("U234", 4.9184e-4)
hue.add_nuclide("U235", 4.4994e-2)
hue.add_nuclide("U238", 2.4984e-3)
hue.set_density("sum")
openmc.Materials([hue]).export_to_xml()

sph = openmc.Sphere(r=R, boundary_type="vacuum")
core = openmc.Cell(fill=hue, region=-sph)
openmc.Geometry([core]).export_to_xml()

# Mesh bounds the sphere exactly, so surface-adjacent sites probe the faces.
LL, UR, DIM = [-R, -R, -R], [R, R, R], [5, 5, 5]
mesh = openmc.RegularMesh()
mesh.lower_left, mesh.upper_right, mesh.dimension = LL, UR, DIM

s = openmc.Settings()
s.run_mode = "eigenvalue"
s.particles = 5000
s.batches = 40
s.inactive = 10
s.seed = 20260917
s.entropy_mesh = mesh
s.source = openmc.IndependentSource(
    space=openmc.stats.Point((0.0, 0.0, 0.0)))
s.export_to_xml()

openmc.run(output=True)

sp_path = f"statepoint.{s.batches}.h5"
with openmc.StatePoint(sp_path) as sp:
    entropy = np.asarray(sp.entropy, dtype=float)
    src = sp.source
    # 'r' is a structured dtype with x/y/z fields, not an (N,3) array.
    rr = src["r"]
    r = np.column_stack([rr["x"], rr["y"], rr["z"]]).astype(float)
    wgt = np.asarray(src["wgt"], dtype=float)  # shape (N,)
    k_mean, k_std = float(sp.keff.nominal_value), float(sp.keff.std_dev)

# Independent transcription of src/eigenvalue.cpp:587-616 + mesh.cpp:1580,1134.
# This is the EXACT oracle the Rust port is checked against: same bank, same
# mesh, algorithm transcribed branch-for-branch from the C++.
def index_in_direction(x, lo, hi, n):
    w = (hi - lo) / n
    if x <= lo:
        return 1 if x == lo else 0
    if x >= hi:
        return n if x == hi else n + 1
    return math.ceil((x - lo) / w)

def get_bin(p):
    ijk = []
    for i in range(3):
        c = index_in_direction(p[i], LL[i], UR[i], DIM[i])
        if c < 1 or c > DIM[i]:
            return None
        ijk.append(c - 1)
    return (ijk[2] * DIM[1] + ijk[1]) * DIM[0] + ijk[0]

counts = [0.0] * (DIM[0] * DIM[1] * DIM[2])
outside = False
for p, w in zip(r, wgt):
    b = get_bin(p)
    if b is None:
        outside = True
    else:
        counts[b] += float(w)
tot = sum(counts)
H_ref = -sum((c / tot) * math.log2(c / tot) for c in counts if c > 0.0)

out = {
    "openmc_version": openmc.__version__,
    "seed": s.seed, "particles": s.particles,
    "batches": s.batches, "inactive": s.inactive,
    "k_mean": k_mean, "k_std": k_std,
    "mesh": {"lower_left": LL, "upper_right": UR, "dimension": DIM},
    "openmc_entropy_per_generation": entropy.tolist(),
    "transcribed_entropy_on_final_source_bank": H_ref,
    "counts_on_final_source_bank": counts,
    "any_site_outside_mesh": outside,
    "n_sites": int(len(wgt)),
    "source_bank": {"r": r.tolist(), "wgt": wgt.tolist()},
}
with open("entropy_reference.json", "w") as f:
    json.dump(out, f)

# The Rust test reads CSV, not JSON: this crate has no serde_json dependency
# and the workspace keeps outram-mc-libs lean. Oracles live one level up,
# beside the topic, because openmc_inputs/ is excluded from the published
# tarball while the oracle must ship with the test that consumes it.
import csv, pathlib
topic = pathlib.Path("..")
with open(topic / "entropy_source_bank.csv", "w", newline="") as f:
    w = csv.writer(f)
    w.writerow(["x_cm", "y_cm", "z_cm", "wgt"])
    for p_, wt in zip(r, wgt):
        w.writerow([repr(float(p_[0])), repr(float(p_[1])),
                    repr(float(p_[2])), repr(float(wt))])
with open(topic / "entropy_oracle.csv", "w", newline="") as f:
    w = csv.writer(f)
    w.writerow(["key", "value"])
    for k, v in [
        ("openmc_version", openmc.__version__), ("seed", s.seed),
        ("particles", s.particles), ("batches", s.batches),
        ("inactive", s.inactive),
        ("k_mean", repr(k_mean)), ("k_std", repr(k_std)),
        ("ll_x", repr(LL[0])), ("ll_y", repr(LL[1])), ("ll_z", repr(LL[2])),
        ("ur_x", repr(UR[0])), ("ur_y", repr(UR[1])), ("ur_z", repr(UR[2])),
        ("dim_x", DIM[0]), ("dim_y", DIM[1]), ("dim_z", DIM[2]),
        ("n_sites", int(len(wgt))),
        ("any_site_outside_mesh", int(outside)),
        ("transcribed_entropy_on_final_source_bank", repr(H_ref)),
        ("openmc_entropy_last_generation", repr(float(entropy[-1]))),
        ("max_possible_entropy_log2_nbins", repr(math.log2(len(counts)))),
    ]:
        w.writerow([k, v])
print("wrote entropy_source_bank.csv and entropy_oracle.csv")
print(f"k = {k_mean:.6f} +/- {k_std:.6f}")
print(f"generations of entropy: {len(entropy)}  last = {entropy[-1]:.10f}")
print(f"transcribed H on final source bank = {H_ref:.15f}")
print(f"max possible H = log2({len(counts)}) = {math.log2(len(counts)):.6f}")
print(f"sites = {len(wgt)}  any outside mesh = {outside}")
