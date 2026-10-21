"""OpenMC reference for tutorial rung 3 (GitHub #525): natural-U metal lump in graphite.

The SAME Wigner-Seitz cell `examples/lumped_ugraphite_kinf.rs` runs on
outram-mc: a natural-uranium-metal sphere of radius r inside a graphite sphere
of radius R with a WHITE outer boundary, at a fixed cell-average N_C/N_U, so
R = r / v^(1/3) with v the uranium volume fraction. `--control` fills both
regions with the cell-average mixture instead (the homogeneous limit).

Four factors from OpenMC's own tallies by the formulas of
`src/physics/reactor_physics.rs` (three groups split at 0.625 eV and 100 keV,
no leakage): eta = P_th / A_th,lump, f = A_th,lump / A_th,
p = A_th / (A_th + A_res), eps = (P_tot/P_th)(A_tot - A_fast)/A_tot. "Fuel" is
the lump material (material filter), as the library defines it; for
`--control` it is the uranium nuclides.

Compositions are recomputed from the same constants the Rust example uses:
U metal 19.05 g/cm3 with IUPAC natural atom fractions, graphite 1.73 g/cm3,
C12/C13 = 98.93/1.07 at%. Physics settings as
`../../tutorial_rung2/openmc_inputs/ugraphite_openmc.py`: 296 K (nearest:
294 K neutron data, 296 K c_Graphite), ptables on, DBRC 1e-5 eV .. 1 keV.

    python lumped_openmc.py --r 2.0 --seed 1 --threads 3 --particles 20000 \
        --inactive 20 --active 100 --workdir runs/r2_s1 --json out.jsonl
"""
import argparse
import json
import math
import os
import pathlib
import time

import numpy as np
import openmc

T = 296.0
NA_B = 0.602214076
M_U = (234.040952, 235.043930, 238.050788)
NAT_U = (0.000054, 0.007204, 0.992742)
M_C = 12.011
C12_AF, C13_AF = 0.9893, 0.0107
RHO_U, RHO_GR = 19.05, 1.73
E_EDGES = [1.0e-5, 0.625, 1.0e5, 2.0e7]
NUCS_U = ("U234", "U235", "U238")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--r", type=float, required=True, help="lump radius [cm]")
    ap.add_argument("--cu", type=float, default=600.0)
    ap.add_argument("--control", action="store_true")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--threads", type=int, default=3)
    ap.add_argument("--particles", type=int, default=20000)
    ap.add_argument("--inactive", type=int, default=20)
    ap.add_argument("--active", type=int, default=100)
    ap.add_argument("--workdir", required=True)
    ap.add_argument("--json", default=None)
    ap.add_argument("--openmc", default=os.path.expanduser(
        "~/Documents/research/openmcbin/bin/openmc"))
    a = ap.parse_args()

    m_u = sum(x * m for x, m in zip(NAT_U, M_U))
    n_u = RHO_U * NA_B / m_u
    n_c = RHO_GR * NA_B / M_C
    v = 1.0 / (1.0 + a.cu * n_u / n_c)
    big_r = a.r / v ** (1.0 / 3.0)
    print(f"v = {v:.6e}, r = {a.r} cm, R = {big_r:.5f} cm, N_U(metal) = {n_u:.6e}, "
          f"N_C = {n_c:.6e}")

    def material(name, nu, nc):
        m = openmc.Material(name=name, temperature=T)
        for nuc, x in zip(NUCS_U, NAT_U):
            if nu > 0:
                m.add_nuclide(nuc, x * nu, "ao")
        if nc > 0:
            m.add_nuclide("C12", C12_AF * nc, "ao")
            m.add_nuclide("C13", C13_AF * nc, "ao")
            m.add_s_alpha_beta("c_Graphite")
        m.set_density("sum")
        return m

    if a.control:
        hom = material("homogenised", v * n_u, (1.0 - v) * n_c)
        lump_m, mod_m = hom, hom
    else:
        lump_m = material("U metal", n_u, 0.0)
        mod_m = material("graphite", 0.0, n_c)
    s_in = openmc.Sphere(r=a.r)
    s_out = openmc.Sphere(r=big_r, boundary_type="white")
    geo = openmc.Geometry([openmc.Cell(fill=lump_m, region=-s_in),
                           openmc.Cell(fill=mod_m, region=+s_in & -s_out)])
    mats = openmc.Materials([lump_m] if a.control else [lump_m, mod_m])

    s = openmc.Settings()
    s.run_mode = "eigenvalue"
    s.particles = a.particles
    s.inactive = a.inactive
    s.batches = a.inactive + a.active
    s.seed = a.seed
    s.source = openmc.IndependentSource(
        space=openmc.stats.Box((-a.r,) * 3, (a.r,) * 3),
        energy=openmc.stats.Watt(a=0.988e6, b=2.249e-6),
        constraints={"fissionable": True})
    s.temperature = {"default": T, "method": "nearest", "tolerance": 10.0}
    s.ptables = True
    s.resonance_scattering = {"enable": True, "method": "dbrc",
                              "energy_min": 1.0e-5, "energy_max": 1000.0}
    s.output = {"tallies": False, "summary": False}

    ef = openmc.EnergyFilter(E_EDGES)
    t_tot = openmc.Tally(name="total")
    t_tot.filters = [ef]
    t_tot.scores = ["absorption", "nu-fission"]
    t_fuel = openmc.Tally(name="fuel")
    if a.control:
        t_fuel.filters = [ef]
        t_fuel.nuclides = list(NUCS_U)
    else:
        t_fuel.filters = [ef, openmc.MaterialFilter([lump_m])]
    t_fuel.scores = ["absorption"]
    model = openmc.Model(geometry=geo, materials=mats, settings=s,
                         tallies=openmc.Tallies([t_tot, t_fuel]))

    wd = pathlib.Path(a.workdir)
    wd.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    sp_path = model.run(cwd=wd, threads=a.threads, openmc_exec=a.openmc, output=False)
    wall = time.time() - t0
    with openmc.StatePoint(sp_path) as sp:
        k = sp.keff
        kgen = np.asarray(sp.k_generation)[a.inactive:]
        tt = sp.get_tally(name="total")
        a_tot = tt.get_values(scores=["absorption"]).ravel()
        p_tot = tt.get_values(scores=["nu-fission"]).ravel()
        tf = sp.get_tally(name="fuel")
        af = tf.get_values(scores=["absorption"]).ravel()
        # by group, summed over nuclides (control) or the single material bin
        a_fuel = af.reshape(3, -1).sum(axis=1)
    eta = p_tot[0] / a_fuel[0]
    f = a_fuel[0] / a_tot[0]
    p = a_tot[0] / (a_tot[0] + a_tot[1])
    eps = (p_tot.sum() / p_tot[0]) * (a_tot.sum() - a_tot[2]) / a_tot.sum()
    out = {
        "r": a.r, "R": big_r, "cu": a.cu, "control": a.control, "seed": a.seed,
        "particles": a.particles, "inactive": a.inactive, "active": a.active,
        "threads": a.threads, "k": k.nominal_value, "k_std": k.std_dev,
        "k_gen_mean": float(kgen.mean()),
        "k_gen_sem": float(kgen.std(ddof=1) / math.sqrt(len(kgen))),
        "eta": eta, "f": f, "p": p, "eps": eps, "k_factors": eta * f * p * eps,
        "A": a_tot.tolist(), "P": p_tot.tolist(), "wall_s": wall,
        "openmc": openmc.__version__,
    }
    print(json.dumps(out, indent=1))
    if a.json:
        with open(a.json, "a") as fh:
            fh.write(json.dumps(out) + "\n")


if __name__ == "__main__":
    main()
