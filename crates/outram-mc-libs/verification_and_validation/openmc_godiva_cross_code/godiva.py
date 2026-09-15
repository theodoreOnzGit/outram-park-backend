#!/usr/bin/env python3
"""OpenMC model of ICSBEP HEU-MET-FAST-001 (Godiva), matched to the
outram-mc-libs deck as closely as the two codes allow.

Identical to the Rust runs in: radius, the three ICSBEP nuclide atom densities,
temperature, histories per generation, inactive/active generation split, and the
nuclear data (same ENDF/B-VIII.0 tapes, same NJOY2016 build).

`ptables` is a command-line switch because outram-mc-libs has NO unresolved
probability tables at all -- its chain is RECONR + BROADR only. Running OpenMC
both ways gives (a) an apples-to-apples transport comparison with ptables off and
(b) a direct measurement of what URR self-shielding is worth on Godiva.
"""
import sys, os, openmc

WORK = os.environ.get("WORK_DIR", "./work")

PTABLES = "--ptables" in sys.argv
KINF = "--kinf" in sys.argv   # reflective boundary: infinite medium, no leakage
SEED = 1
for a in sys.argv:
    if a.startswith("--seed="):
        SEED = int(a.split("=", 1)[1])

RADIUS = 8.7407          # cm, ICSBEP HEU-MET-FAST-001
TEMP_K = 293.6
DENS = {"U234": 4.9184e-4, "U235": 4.4994e-2, "U238": 2.4984e-3}  # atoms/b-cm

os.environ["OPENMC_CROSS_SECTIONS"] = f"{WORK}/cross_sections.xml"

fuel = openmc.Material(name="Godiva HEU")
for nuc, n in DENS.items():
    fuel.add_nuclide(nuc, n, "ao")
fuel.set_density("atom/b-cm", sum(DENS.values()))
fuel.temperature = TEMP_K
materials = openmc.Materials([fuel])
materials.cross_sections = f"{WORK}/cross_sections.xml"

sphere = openmc.Sphere(r=RADIUS, boundary_type="reflective" if KINF else "vacuum")
core = openmc.Cell(name="core", fill=fuel, region=-sphere)
geometry = openmc.Geometry([core])

settings = openmc.Settings()
settings.run_mode = "eigenvalue"
settings.particles = 5000
settings.inactive = 40
settings.batches = 160          # 40 inactive + 120 active, as in the Rust deck
settings.seed = SEED
settings.ptables = PTABLES
settings.temperature = {"method": "nearest", "tolerance": 1000.0}
# Uniform-in-volume fission source over the sphere, matching the Rust driver's
# initial source rather than OpenMC's default point source.
settings.source = openmc.IndependentSource(
    space=openmc.stats.Point((0.0, 0.0, 0.0)) if False else
          openmc.stats.Box((-RADIUS,)*3, (RADIUS,)*3, only_fissionable=True),
    energy=openmc.stats.Watt(a=0.988e6, b=2.249e-6),
)
settings.output = {"tallies": False}

model = openmc.Model(geometry=geometry, materials=materials, settings=settings)
tag = ("kinf_" if KINF else "") + ("ptables_on" if PTABLES else "ptables_off")
cwd = f"{WORK}/run_{tag}_seed{SEED}"
os.makedirs(cwd, exist_ok=True)
model.export_to_model_xml(f"{cwd}/model.xml")
openmc.run(cwd=cwd)

sp_path = f"{cwd}/statepoint.{settings.batches}.h5"
with openmc.StatePoint(sp_path) as sp:
    k = sp.keff
print(f"RESULT {tag} seed={SEED} k={k.nominal_value:.6f} +/- {k.std_dev:.6f} "
      f"pcm={(k.nominal_value-1.0)*1e5:+.0f} +/- {k.std_dev*1e5:.0f}")
