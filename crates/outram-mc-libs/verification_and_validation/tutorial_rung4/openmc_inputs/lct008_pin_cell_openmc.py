# SPDX-License-Identifier: GPL-3.0
"""OpenMC deck: LEU-COMP-THERM-008's pin in a reflective square cell (k-infinity).

Code-to-code reference for `examples/lct008_pitch_sweep.rs` (tutorial rung 4,
GitHub #526). Written for this study on 2026-10-04 in the outram-park-backend
repository (GPL-3.0); the material handling and physics settings are copied
from the five-route ICSBEP deck
`verification_and_validation/icsbep/five_route_keff/openmc_inputs/icsbep_openmc.py`
(`lct008()` and `main()`), so the two studies run OpenMC the same way.

The model:

* materials: the committed case-1 `materials.xml` of the `mit-crpg/benchmarks`
  LEU-COMP-THERM-008 model (`../../icsbep/leu-comp-therm-008/`, MIT licence),
  read with OpenMC's own XML reader and restricted to the SAME 11-nuclide tier
  outram-mc runs with `--cheap-nuclides` (dropped, not renormalised: density
  units "sum" sums what is left, as outram-mc's per-nuclide densities do);
* pin: UO2 r < 0.514858 cm, Al-6061 clad to 0.602996 cm, water outside (radii of
  surfaces 1 and 2 of the committed geometry.xml; no gap);
* cell: square, side = pitch, four reflective planes plus two reflective
  z-planes at +-10 cm, so the eigenvalue is k-infinity;
* source: Watt spectrum (a = 0.988 MeV, b = 2.249 /MeV) uniform in the fuel's
  bounding box, fissionable only;
* physics: 293.6 K; URR probability tables on; resonance scattering (DBRC)
  1e-5 eV .. 1 keV on every nuclide; S(a,b) c_H_in_H2O on the water.

  python lct008_pin_cell_openmc.py --pitch 1.63576 --xs <h5>/cross_sections.xml \
      --seed 1 --csv out.csv --workdir runs/p1.63576_s1 --threads 2 \
      [--particles 5000 --inactive 50 --active 200] [--no-soluble-boron]
"""
import argparse
import csv
import os
import pathlib
import time

import numpy as np
import openmc

T = 293.6
R_FUEL = 0.514858
R_CLAD = 0.602996
HALF_Z = 10.0
LCT008_DIR = pathlib.Path(__file__).resolve().parents[2] / "icsbep" / "leu-comp-therm-008"
TIER = {"H1", "B10", "O16", "U234", "U235", "U238", "Al27", "Si28", "Si29", "Si30", "Mn55"}


def materials(no_boron):
    mats = openmc.Materials.from_xml(str(LCT008_DIR / "materials.xml"))
    dropped = set()
    for m in mats:
        m.temperature = T
        is_water = any(s[0] == "c_H_in_H2O" for s in m._sab)
        for nuc in [n.name for n in m.nuclides]:
            if nuc not in TIER or (no_boron and is_water and nuc in ("B10", "B11")):
                m.remove_nuclide(nuc)
                dropped.add(nuc)
    print("dropped:", ", ".join(sorted(dropped)))
    by_id = {m.id: m for m in mats}
    return by_id[1], by_id[2], by_id[3]  # water, fuel, clad


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pitch", type=float, required=True)
    ap.add_argument("--xs", required=True)
    ap.add_argument("--seed", type=int, required=True)
    ap.add_argument("--csv", required=True)
    ap.add_argument("--workdir", required=True)
    ap.add_argument("--threads", type=int, default=2)
    ap.add_argument("--particles", type=int, default=5000)
    ap.add_argument("--inactive", type=int, default=50)
    ap.add_argument("--active", type=int, default=200)
    ap.add_argument("--no-soluble-boron", action="store_true")
    ap.add_argument("--openmc", default=os.path.expanduser("~/Documents/research/openmcbin/bin/openmc"))
    a = ap.parse_args()

    water, fuel, clad = materials(a.no_soluble_boron)
    h = 0.5 * a.pitch
    fuel_or = openmc.ZCylinder(r=R_FUEL)
    clad_or = openmc.ZCylinder(r=R_CLAD)
    box = (+openmc.XPlane(-h, boundary_type="reflective")
           & -openmc.XPlane(h, boundary_type="reflective")
           & +openmc.YPlane(-h, boundary_type="reflective")
           & -openmc.YPlane(h, boundary_type="reflective")
           & +openmc.ZPlane(-HALF_Z, boundary_type="reflective")
           & -openmc.ZPlane(HALF_Z, boundary_type="reflective"))
    cells = [
        openmc.Cell(fill=fuel, region=-fuel_or & box),
        openmc.Cell(fill=clad, region=+fuel_or & -clad_or & box),
        openmc.Cell(fill=water, region=+clad_or & box),
    ]
    geo = openmc.Geometry(openmc.Universe(cells=cells))
    mats = openmc.Materials([water, fuel, clad])
    mats.cross_sections = str(pathlib.Path(a.xs).resolve())

    s = openmc.Settings()
    s.run_mode = "eigenvalue"
    s.particles = a.particles
    s.inactive = a.inactive
    s.batches = a.inactive + a.active
    s.seed = a.seed
    s.source = openmc.IndependentSource(
        space=openmc.stats.Box((-R_FUEL, -R_FUEL, -HALF_Z), (R_FUEL, R_FUEL, HALF_Z)),
        energy=openmc.stats.Watt(a=0.988e6, b=2.249e-6),
        constraints={"fissionable": True})
    s.temperature = {"default": T, "method": "nearest", "tolerance": 10.0}
    s.ptables = True
    s.resonance_scattering = {"enable": True, "method": "dbrc",
                              "energy_min": 1.0e-5, "energy_max": 1000.0}
    s.output = {"tallies": False, "summary": False}
    model = openmc.Model(geometry=geo, materials=mats, settings=s)

    wd = pathlib.Path(a.workdir)
    wd.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    sp_path = model.run(cwd=wd, threads=a.threads, openmc_exec=a.openmc, output=False)
    wall = time.time() - t0
    with openmc.StatePoint(sp_path) as sp:
        k = sp.keff
        kgen = np.asarray(sp.k_generation)[a.inactive:]
        ent = np.asarray(sp.entropy) if sp.entropy is not None and len(sp.entropy) else None
    csv_path = pathlib.Path(a.csv)
    new = not csv_path.is_file()
    with csv_path.open("a", newline="") as f:
        w = csv.writer(f)
        if new:
            w.writerow(["pitch_cm", "boron", "seed", "particles", "inactive", "active",
                        "threads", "k_combined", "k_combined_std", "k_gen_mean",
                        "k_gen_sem", "wall_s"])
        sem = kgen.std(ddof=1) / np.sqrt(len(kgen))
        w.writerow([a.pitch, "none" if a.no_soluble_boron else "case1", a.seed, a.particles,
                    a.inactive, a.active, a.threads, f"{k.nominal_value:.6f}",
                    f"{k.std_dev:.6f}", f"{kgen.mean():.6f}", f"{sem:.6f}", f"{wall:.1f}"])
    print(f"pitch {a.pitch}: k = {k.nominal_value:.5f} +/- {k.std_dev:.5f} "
          f"(generation mean {kgen.mean():.5f})  {wall:.1f} s")


if __name__ == "__main__":
    main()
