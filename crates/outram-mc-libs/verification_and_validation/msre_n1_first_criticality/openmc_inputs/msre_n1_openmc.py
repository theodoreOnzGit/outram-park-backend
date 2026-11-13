"""MSRE full core in OpenMC: route 1 for N-1 (gh:#1128) and N-3 (gh:#1129).

A transcription of ``outram_mc_libs::vv::msre_n1`` (the route-4 model): same
surfaces, cells, materials, atom densities and temperature handling. Every
dimension and simplification is cited in that module; nothing new is
introduced here.

Data: NJOY2016 ac5adf5 through OpenMC's own driver
(``IncidentNeutron.from_njoy``) at 1131/1181/1231 F, with 0 K elastic for
DBRC; graphite 30P from the published tape at its tabulated 800 and 1000 K,
OpenMC interpolating in temperature (route 4 regenerates the law by LEAPR at
the exact temperature instead).

    python msre_n1_openmc.py --xs <cross_sections.xml> --t-salt-f 1181 \
        --t-graphite-f 1181 --seed 1 --csv out.csv --workdir <dir> \
        --threads 16 --openmc <openmc> [--label route1]
"""
import argparse
import csv
import math
import pathlib
import time

import numpy as np
import openmc

INCH = 2.54
PITCH = 2.0 * INCH
R_GRAPHITE = 55.25 / 2.0 * INCH
H_CORE = 64.0 * INCH
R_CAN_IN = 55.5 / 2.0 * INCH
R_CAN_OUT = 56.0 / 2.0 * INCH
R_VESSEL_IN = 58.0 / 2.0 * INCH
R_VESSEL_OUT = 59.125 / 2.0 * INCH
T_HEAD = INCH
FT3 = (12.0 * INCH) ** 3
V_UPPER_HEAD = 10.5 * FT3
V_LOWER_HEAD = 10.0 * FT3
PASSAGE_HALF_DEPTH = 0.2 * INCH
PASSAGE_HALF_FLAT = 0.4 * INCH
PASSAGE_END_RADIUS = 0.2 * INCH
OPENING_OFFSET = 2.0 * INCH
R_OPENING = 1.25 * INCH
R_THIMBLE_OUT = INCH
R_THIMBLE_IN = INCH - 0.065 * INCH
OPENINGS = [(-OPENING_OFFSET, OPENING_OFFSET), (OPENING_OFFSET, OPENING_OFFSET),
            (-OPENING_OFFSET, -OPENING_OFFSET), (OPENING_OFFSET, -OPENING_OFFSET)]

SALT = [("Li7", 2.218270e-02), ("Li6", 2.218492e-06), ("Be9", 9.910143e-03),
        ("U235", 8.398016e-05), ("U238", 1.845625e-04), ("F19", 4.964736e-02),
        ("Zr90", 8.448066e-04), ("Zr91", 1.842319e-04), ("Zr92", 2.816022e-04),
        ("Zr94", 2.853788e-04), ("Zr96", 4.597587e-05)]
GRAPHITE = [("C12", 9.271946e-02), ("C13", 1.038652e-03), ("B10", 1.651533e-08), ("B11", 6.681124e-08)]
INOR8 = [("Ni58", 4.351546e-02), ("Ni60", 1.676208e-02), ("Ni61", 7.286360e-04), ("Ni62", 2.323210e-03),
         ("Ni64", 5.916532e-04), ("Cr50", 3.090949e-04), ("Cr52", 5.960587e-03), ("Cr53", 6.758827e-04),
         ("Cr54", 1.682415e-04), ("Fe54", 2.212254e-04), ("Fe56", 3.472766e-03), ("Fe57", 8.020130e-05),
         ("Fe58", 1.067332e-05)]  # arm A: Mo omitted
SALT_EXPANSION_PER_F = -1.18e-4
REFERENCE_F = 1181.0


def kelvin(t_f):
    return (t_f - 32.0) * 5.0 / 9.0 + 273.15


def basket_fractions():
    opening = math.pi * 1.25 * 1.25
    inor = 12.0 * math.pi * 0.125 * 0.125 / opening
    graphite = 15.0 * 0.25 * 0.47 / opening
    return inor, graphite, 1.0 - inor - graphite


def material(name, comps, t_k, sab=False):
    m = openmc.Material(name=name, temperature=t_k)
    for n, d in comps:
        m.add_nuclide(n, d, "ao")
    m.set_density("sum")
    if sab:
        m.add_s_alpha_beta("c_Graphite_30p")
    return m


def build(t_salt_f, t_graphite_f):
    ts, tg = kelvin(t_salt_f), kelvin(t_graphite_f)
    f = 1.0 + SALT_EXPANSION_PER_F * (t_salt_f - REFERENCE_F)
    salt_comps = [(n, d * f) for n, d in SALT]
    fi, fg, fs = basket_fractions()
    basket = {}
    for w, comps in [(fs, salt_comps), (fg, GRAPHITE), (fi, INOR8)]:
        for n, d in comps:
            basket[n] = basket.get(n, 0.0) + w * d
    salt = material("MSRE fuel salt, first criticality", salt_comps, ts)
    # Route 4 runs the stringer graphite at the graphite temperature and the
    # basket (graphite share included) at the salt temperature; so here.
    graphite = material("MSRE CGB graphite", GRAPHITE, tg, sab=True)
    inor8 = material("INOR-8", INOR8, ts)
    basket_m = material("Sample baskets (homogenised)", list(basket.items()), ts, sab=True)

    # Stringer universe: four half-stadium passages (salt), graphite around.
    h = PITCH / 2.0
    floor = h - PASSAGE_HALF_DEPTH
    a, r = PASSAGE_HALF_FLAT, PASSAGE_END_RADIUS
    xf_p, xf_m = openmc.XPlane(floor), openmc.XPlane(-floor)
    yf_p, yf_m = openmc.YPlane(floor), openmc.YPlane(-floor)
    ya_p, ya_m = openmc.YPlane(a), openmc.YPlane(-a)
    xa_p, xa_m = openmc.XPlane(a), openmc.XPlane(-a)
    passages = []
    for floor_s, beyond, sx in [(xf_p, False, 1.0), (xf_m, True, -1.0)]:
        slab = (+floor_s if not beyond else -floor_s) & +ya_m & -ya_p
        passages.append(slab | -openmc.ZCylinder(sx * h, a, r) | -openmc.ZCylinder(sx * h, -a, r))
    for floor_s, beyond, sy in [(yf_p, False, 1.0), (yf_m, True, -1.0)]:
        slab = (+floor_s if not beyond else -floor_s) & +xa_m & -xa_p
        passages.append(slab | -openmc.ZCylinder(a, sy * h, r) | -openmc.ZCylinder(-a, sy * h, r))
    fuel = passages[0] | passages[1] | passages[2] | passages[3]
    stringer = openmc.Universe(cells=[openmc.Cell(fill=salt, region=fuel), openmc.Cell(fill=graphite, region=~fuel)])

    n = math.ceil(2.0 * R_VESSEL_IN / PITCH) | 1
    half = n * PITCH / 2.0
    lat = openmc.RectLattice()
    lat.lower_left = (-half, -half)
    lat.pitch = (PITCH, PITCH)
    lat.universes = [[stringer] * n for _ in range(n)]

    z_lp = -V_LOWER_HEAD / (math.pi * R_VESSEL_IN ** 2)
    z_up = H_CORE + V_UPPER_HEAD / (math.pi * R_VESSEL_IN ** 2)
    r_g, r_ci, r_co, r_vi = (openmc.ZCylinder(r=x) for x in (R_GRAPHITE, R_CAN_IN, R_CAN_OUT, R_VESSEL_IN))
    r_vo = openmc.ZCylinder(r=R_VESSEL_OUT, boundary_type="vacuum")
    z0, zt, zl, zu = (openmc.ZPlane(z) for z in (0.0, H_CORE, z_lp, z_up))
    zlb = openmc.ZPlane(z_lp - T_HEAD, boundary_type="vacuum")
    zut = openmc.ZPlane(z_up + T_HEAD, boundary_type="vacuum")
    openings = [openmc.ZCylinder(x, y, R_OPENING) for x, y in OPENINGS]
    th_out = [openmc.ZCylinder(x, y, R_THIMBLE_OUT) for x, y in OPENINGS[:3]]
    th_in = [openmc.ZCylinder(x, y, R_THIMBLE_IN) for x, y in OPENINGS[:3]]
    core_z = +z0 & -zt
    cells = []
    core = -r_g & core_z
    for o in openings:
        core = core & +o
    cells.append(openmc.Cell(fill=lat, region=core))
    for i in range(3):
        cells.append(openmc.Cell(fill=inor8, region=-th_out[i] & +th_in[i] & +z0 & -zu))
        cells.append(openmc.Cell(fill=None, region=-th_in[i] & +z0 & -zu))
        cells.append(openmc.Cell(fill=salt, region=-openings[i] & +th_out[i] & core_z))
    cells.append(openmc.Cell(fill=basket_m, region=-openings[3] & core_z))
    for inner, outer, m in [(r_g, r_ci, salt), (r_ci, r_co, inor8), (r_co, r_vi, salt)]:
        cells.append(openmc.Cell(fill=m, region=+inner & -outer & core_z))
    cells.append(openmc.Cell(fill=salt, region=-r_vi & +zl & -z0))
    upper = -r_vi & +zt & -zu
    for o in th_out:
        upper = upper & +o
    cells.append(openmc.Cell(fill=salt, region=upper))
    cells.append(openmc.Cell(fill=inor8, region=+r_vi & -r_vo & +zl & -zu))
    cells.append(openmc.Cell(fill=inor8, region=-r_vo & +zlb & -zl))
    cells.append(openmc.Cell(fill=inor8, region=-r_vo & +zu & -zut))
    geo = openmc.Geometry(cells)
    src = openmc.stats.Box((-R_GRAPHITE, -R_GRAPHITE, 0.0), (R_GRAPHITE, R_GRAPHITE, H_CORE))
    return [salt, graphite, inor8, basket_m], geo, src, ts


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--xs", required=True)
    ap.add_argument("--t-salt-f", type=float, default=REFERENCE_F)
    ap.add_argument("--t-graphite-f", type=float, default=REFERENCE_F)
    ap.add_argument("--seed", type=int, required=True)
    ap.add_argument("--csv", required=True)
    ap.add_argument("--workdir", required=True)
    ap.add_argument("--threads", type=int, default=16)
    ap.add_argument("--particles", type=int, default=20000)
    ap.add_argument("--inactive", type=int, default=100)
    ap.add_argument("--active", type=int, default=200)
    ap.add_argument("--label", default="route1")
    ap.add_argument("--commit", default="")
    ap.add_argument("--openmc", default="openmc")
    ap.add_argument("--plot", action="store_true", help="write geometry plots and exit")
    ap.add_argument("--no-dbrc", action="store_true", help="ablation (gh:#1311): resonance scattering off")
    a = ap.parse_args()
    mats, geo, src, ts = build(a.t_salt_f, a.t_graphite_f)
    materials = openmc.Materials(mats)
    materials.cross_sections = str(pathlib.Path(a.xs).resolve())
    s = openmc.Settings()
    s.run_mode = "eigenvalue"
    s.particles, s.inactive, s.batches, s.seed = a.particles, a.inactive, a.inactive + a.active, a.seed
    s.source = openmc.IndependentSource(space=src, energy=openmc.stats.Watt(a=0.988e6, b=2.249e-6),
                                        constraints={"fissionable": True})
    s.temperature = {"default": ts, "method": "interpolation"}
    s.ptables = True
    s.resonance_scattering = {"enable": not a.no_dbrc, "method": "rvs", "energy_min": 0.01, "energy_max": 1000.0}
    s.output = {"tallies": False, "summary": False}
    model = openmc.Model(geometry=geo, materials=materials, settings=s)
    wd = pathlib.Path(a.workdir)
    wd.mkdir(parents=True, exist_ok=True)
    if a.plot:
        plots = []
        for name, basis, origin, width in [("xy_core", "xy", (0, 0, H_CORE / 2), (160, 160)),
                                           ("xy_centre", "xy", (0, 0, H_CORE / 2), (25, 25)),
                                           ("xz", "xz", (0, 2 * INCH, H_CORE / 2), (160, 230))]:
            p = openmc.Plot(name=name)
            p.basis, p.origin, p.width, p.pixels, p.color_by = basis, origin, width, (1000, 1000), "material"
            p.filename = name
            plots.append(p)
        model.plots = openmc.Plots(plots)
        model.plot_geometry(cwd=wd, openmc_exec=a.openmc, output=False)
        return
    t0 = time.time()
    sp_path = model.run(cwd=wd, threads=a.threads, openmc_exec=a.openmc, output=False)
    wall = time.time() - t0
    with openmc.StatePoint(sp_path) as sp:
        k = sp.keff
        kgen = np.asarray(sp.k_generation)[a.inactive:]
    case = f"msre_n1_salt{a.t_salt_f:g}F_graphite{a.t_graphite_f:g}F" + ("_nodbrc" if a.no_dbrc else "")
    csv_path = pathlib.Path(a.csv)
    new = not csv_path.is_file()
    with csv_path.open("a", newline="") as f:
        w = csv.writer(f)
        if new:
            w.writerow(["case", "route", "code", "seed", "k", "k_std_internal", "particles",
                        "inactive", "active", "threads", "wall_s", "commit", "k_alt"])
        w.writerow([case, a.label, "openmc", a.seed, f"{k.nominal_value:.6f}", f"{k.std_dev:.6f}",
                    a.particles, a.inactive, a.active, a.threads, f"{wall:.1f}", a.commit, f"{kgen.mean():.6f}"])
    print(f"{case} {a.label} seed {a.seed}: k = {k.nominal_value:.5f} +/- {k.std_dev:.5f} "
          f"(generation mean {kgen.mean():.5f}) {wall:.1f} s")


if __name__ == "__main__":
    main()
