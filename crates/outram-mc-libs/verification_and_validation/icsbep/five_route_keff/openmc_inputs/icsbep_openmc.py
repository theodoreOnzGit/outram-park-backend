"""OpenMC decks for the four ICSBEP cases of the five-route study (routes 1, 2).

The SAME models `examples/icsbep_five_route_keff.rs` runs on outram-mc: Godiva,
Jemima and HST-009 with the atom densities and dimensions copied from that file
(which cites godiva_keff_endf_local.rs, jemima_keff.rs, hst009_keff.rs), and
LCT-008 case 1 read from the committed benchmark XML (see `lct008()` below). Only the cross-section library differs between
route 1 (NJOY2016 ACE -> HDF5) and route 2 (Rust-NJOY ACE -> HDF5), selected
with --xs <cross_sections.xml>.

  python icsbep_openmc.py --case godiva --xs <h5>/cross_sections.xml --seed 1 \
      --label route1 --csv out.csv --workdir runs/godiva_route1_s1 [--threads 8] \
      [--particles 5000 --inactive 40 --active 120]

Physics settings, each chosen to match what outram-mc carries by default:

* temperature 293.6 K on every material (the libraries hold one temperature,
  labelled 294K by OpenMC from kT = 2.53e-8 MeV);
* URR probability tables ON (``ptables``, OpenMC's default);
* resonance elastic scattering ON, method ``dbrc``, 1e-5 eV <= E <= 1 keV,
  every nuclide carrying 0 K elastic (all of them) -- outram-mc applies DBRC
  to every nuclide below 1 keV with no lower limit (``DbrcTable::applies``).
  OpenMC's Python API refuses energy_min <= 0, so it is set to 1e-5 eV, the
  bottom of every ENDF evaluation's grid, rather than OpenMC's 0.01 eV default;
* S(a,b) ``c_H_in_H2O`` on the water-bearing materials (HST-009 solution and
  reflector, LCT-008s mixture), none elsewhere;
* source: Watt fission spectrum (OpenMC's default, the same a, b as outram-mc's
  ``KeffSettings`` default) uniform over the fuel region, fissionable-only.

One CSV row is appended per run. ``k`` is OpenMC's combined estimator
(``StatePoint.keff``); ``k_alt`` is the mean of the per-batch generation
k over active batches, the estimator outram-mc reports. Both are recorded so the
choice of estimator can be checked rather than argued.
"""
import argparse
import csv
import os
import pathlib
import time

import numpy as np
import openmc

T = 293.6


def mat(name, comps, sab=False):
    m = openmc.Material(name=name, temperature=T)
    for n, d in comps:
        m.add_nuclide(n, d, "ao")
    m.set_density("sum")
    if sab:
        m.add_s_alpha_beta("c_H_in_H2O")
    return m


def godiva():
    m = mat("Godiva HEU", [("U234", 4.9184e-4), ("U235", 4.4994e-2), ("U238", 2.4984e-3)])
    s = openmc.Sphere(r=8.7407, boundary_type="vacuum")
    geo = openmc.Geometry([openmc.Cell(fill=m, region=-s)])
    src = openmc.stats.spherical_uniform(r_outer=8.7407)
    return [m], geo, src


def jemima():
    core = mat("oralloy core", [("U234", 8.4430e-05), ("U235", 7.7777e-03), ("U238", 3.9671e-02)])
    refl = mat("natural uranium reflector",
               [("U234", 2.6433e-06), ("U235", 3.4603e-04), ("U238", 4.7711e-02)])
    z0 = openmc.ZPlane(0.0, boundary_type="vacuum")
    z1 = openmc.ZPlane(7.62)
    z2 = openmc.ZPlane(39.571)
    z3 = openmc.ZPlane(47.0894, boundary_type="vacuum")
    c1 = openmc.ZCylinder(r=19.05)
    c2 = openmc.ZCylinder(r=26.6446, boundary_type="vacuum")
    cells = [
        openmc.Cell(fill=refl, region=+z0 & -z1 & -c2),
        openmc.Cell(fill=core, region=+z1 & -z2 & -c1),
        openmc.Cell(fill=refl, region=+z1 & -z2 & +c1 & -c2),
        openmc.Cell(fill=refl, region=+z2 & -z3 & -c2),
    ]
    geo = openmc.Geometry(cells)
    src = openmc.stats.Box((-19.05, -19.05, 7.62), (19.05, 19.05, 39.571))
    return [core, refl], geo, src


def hst009():
    sol = mat("uranium oxyfluoride solution", [
        ("U234", 1.7561e-05), ("U235", 1.6626e-03), ("U238", 9.4079e-05),
        ("F19", 3.5663e-03), ("O16", 3.334735656e-02 + 1.264344e-05), ("H1", 5.9587e-02)], sab=True)
    tank = mat("1100 aluminium tank", [
        ("Al27", 5.9699e-02), ("Si28", 5.09126279536e-04), ("Si29", 2.5851979832e-05),
        ("Si30", 1.7041740632e-05), ("Mn55", 1.4853e-05)])
    water = mat("water reflector", [("H1", 6.6659e-02), ("O16", 3.3316368309e-02 + 1.2631691e-05)],
                sab=True)
    s1 = openmc.Sphere(r=11.5177)
    s2 = openmc.Sphere(r=11.6764)
    s3 = openmc.Sphere(r=35.0, boundary_type="vacuum")
    geo = openmc.Geometry([openmc.Cell(fill=sol, region=-s1),
                           openmc.Cell(fill=tank, region=+s1 & -s2),
                           openmc.Cell(fill=water, region=+s2 & -s3)])
    r = 11.5177
    src = openmc.stats.Box((-r, -r, -r), (r, r, r))
    return [sol, tank, water], geo, src


# LEU-COMP-THERM-008 case 1, the REAL lattice: the same committed
# mit-crpg/benchmarks cards outram-mc parses (examples/common/lct008_model.rs),
# read here with OpenMC's own XML readers, so geometry and densities are the
# file's, not a transcription. Restricted to the SAME 11-nuclide tier outram-mc
# runs (lct008_keff.rs `TAPES_CHEAP`): every other nuclide is dropped and NOT
# renormalised, matching `build_materials` (density units="sum" then sums what
# is left, exactly as outram-mc's per-nuclide atom densities do).
LCT008_DIR = pathlib.Path(__file__).resolve().parents[2] / "leu-comp-therm-008"
LCT008_TIER = {"H1", "B10", "O16", "U234", "U235", "U238", "Al27",
               "Si28", "Si29", "Si30", "Mn55"}


def lct008():
    mats = openmc.Materials.from_xml(str(LCT008_DIR / "materials.xml"))
    dropped = set()
    for m in mats:
        m.temperature = T
        for nuc in [n.name for n in m.nuclides]:
            if nuc not in LCT008_TIER:
                m.remove_nuclide(nuc)
                dropped.add(nuc)
    print("lct008: dropped (not in the outram-mc tier):", ", ".join(sorted(dropped)))
    geo = openmc.Geometry.from_xml(str(LCT008_DIR / "geometry.xml"), materials=mats)
    used = list(geo.get_all_materials().values())
    # Same source region as outram-mc: the core cylinder's bounding box.
    src = openmc.stats.Box((-76.2, -76.2, -81.662), (76.2, 76.2, 81.662))
    return used, geo, src


CASES = {"godiva": godiva, "jemima": jemima, "hst009": hst009, "lct008": lct008}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--case", required=True, choices=CASES)
    ap.add_argument("--xs", required=True)
    ap.add_argument("--seed", type=int, required=True)
    ap.add_argument("--label", required=True)
    ap.add_argument("--csv", required=True)
    ap.add_argument("--workdir", required=True)
    ap.add_argument("--threads", type=int, default=8)
    ap.add_argument("--particles", type=int, default=5000)
    ap.add_argument("--inactive", type=int, default=40)
    ap.add_argument("--active", type=int, default=120)
    ap.add_argument("--commit", default="unknown")
    ap.add_argument("--variant", default=None, choices=["solution-inf", "solution-bare"],
                    help="diagnostic (GitHub #367, hst009 only): the solution sphere alone, "
                         "reflective or vacuum boundary")
    ap.add_argument("--drop-nuclide", default=None,
                    help="diagnostic (GitHub #367): remove one nuclide everywhere")
    ap.add_argument("--no-sab", action="store_true",
                    help="diagnostic (GitHub #367): drop S(a,b), H-1 free gas")
    ap.add_argument("--openmc", default=os.path.expanduser("~/Documents/research/openmcbin/bin/openmc"))
    a = ap.parse_args()

    mats, geo, space = CASES[a.case]()
    if a.variant:
        assert a.case == "hst009", "--variant is defined for hst009 only"
        sol = mats[0]
        bc = "reflective" if a.variant == "solution-inf" else "vacuum"
        s1 = openmc.Sphere(r=11.5177, boundary_type=bc)
        geo = openmc.Geometry([openmc.Cell(fill=sol, region=-s1)])
        mats = [sol]
    if a.no_sab:
        for m in mats:
            m._sab = []
    if a.drop_nuclide:
        for m in mats:
            if a.drop_nuclide in [n.name for n in m.nuclides]:
                m.remove_nuclide(a.drop_nuclide)
    materials = openmc.Materials(mats)
    materials.cross_sections = str(pathlib.Path(a.xs).resolve())
    s = openmc.Settings()
    s.run_mode = "eigenvalue"
    s.particles = a.particles
    s.inactive = a.inactive
    s.batches = a.inactive + a.active
    s.seed = a.seed
    s.source = openmc.IndependentSource(
        space=space, energy=openmc.stats.Watt(a=0.988e6, b=2.249e-6),
        constraints={"fissionable": True})
    s.temperature = {"default": T, "method": "nearest", "tolerance": 10.0}
    s.ptables = True
    s.resonance_scattering = {"enable": True, "method": "dbrc",
                              "energy_min": 1.0e-5, "energy_max": 1000.0}
    s.output = {"tallies": False, "summary": False}
    model = openmc.Model(geometry=geo, materials=materials, settings=s)

    wd = pathlib.Path(a.workdir)
    wd.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    sp_path = model.run(cwd=wd, threads=a.threads, openmc_exec=a.openmc, output=False)
    wall = time.time() - t0
    with openmc.StatePoint(sp_path) as sp:
        k = sp.keff
        kgen = np.asarray(sp.k_generation)[a.inactive:]
    # Keep the statepoint small on disk: the CSV row is the record.
    csv_path = pathlib.Path(a.csv)
    new = not csv_path.is_file()
    with csv_path.open("a", newline="") as f:
        w = csv.writer(f)
        if new:
            w.writerow(["case", "route", "code", "seed", "k", "k_std_internal", "particles",
                        "inactive", "active", "threads", "wall_s", "load_s", "n_nuclides",
                        "n_urr", "n_dbrc", "sab", "commit", "k_alt"])
        n_nuc = len({n.name for m in mats for n in m.nuclides})
        w.writerow([a.case, a.label, "openmc", a.seed, f"{k.nominal_value:.6f}",
                    f"{k.std_dev:.6f}", a.particles, a.inactive, a.active, a.threads,
                    f"{wall:.1f}", "", n_nuc, "", "", str(CASES[a.case] in (hst009, lct008)).lower(),
                    a.commit, f"{kgen.mean():.6f}"])
    print(f"{a.case} {a.label} seed {a.seed}: k = {k.nominal_value:.5f} +/- {k.std_dev:.5f}"
          f"  (generation mean {kgen.mean():.5f})  {wall:.1f} s")


if __name__ == "__main__":
    main()
