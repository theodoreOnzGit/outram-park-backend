"""OpenMC reference for tutorial rung 2 (GitHub #524): homogeneous U + graphite.

The SAME mixture `examples/ugraphite_four_factor.rs` runs on outram-mc, in a
reflective cube (k_inf, no leakage), with the four factors computed from
OpenMC's own nuclide-resolved tallies by the formulas of
`src/physics/reactor_physics.rs` (three groups, split at 0.625 eV and 100 keV;
with no leakage the two non-leakage factors are 1):

    eta = P_th / A_th,U        f = A_th,U / A_th
    p   = A_th / (A_th + A_res)
    eps = (P_tot / P_th) * (A_tot - A_fast) / A_tot

and the two-group view (`SixFactors::two_group_openmc_convention`):
p2 = A_th / A_tot, eps2 = P_tot / P_th.

Compositions are recomputed here from the same published constants the Rust
example uses (not copied from its output), so a transcription error on either
side shows up as a density mismatch in the printed table:

  --case pebble          HTR-10 fuel pebble's U and C, smeared over the ball
                         (Li, Yu & Wei 2014 Table 2; IAEA-TECDOC-1382 Table 4-2
                         for 17 wt% and 1.73 g/cm3), O and Si left out.
  --case natural --cu R  natural uranium at N_C/N_U = R, carbon at 1.73 g/cm3.

Physics settings, each matching what outram-mc carries by default:
  * 296 K on the material; nearest-temperature lookup with 10 K tolerance gives
    the library's 294 K neutron data (the library has 250/294/600 K) and the
    296 K c_Graphite table. outram-mc broadens to 296 K exactly; the 2 K
    difference is stated, not corrected.
  * S(a,b) c_Graphite (crystalline graphite, ENDF/B-VIII.0) on C12 and C13.
  * URR probability tables on; resonance elastic scattering (DBRC) on for every
    nuclide with 0 K data, 1e-5 eV .. 1 keV (as icsbep_openmc.py).
  * Watt fission source (OpenMC default a, b) uniform over the cube.

    python ugraphite_openmc.py --case pebble --seed 1 --threads 3 \
        --particles 20000 --inactive 20 --active 100 --workdir runs/pebble_s1
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
HALF = 50.0
NA_B = 0.602214076
M_U235, M_U238, M_O16, M_C, M_SI = 235.043930, 238.050788, 15.9949146, 12.011, 28.0855
C12_AF, C13_AF = 0.9893, 0.0107
NAT_U = (0.000054, 0.007204, 0.992742)  # U234, U235, U238 atom fractions
RHO_UO2, RHO_BUF, RHO_PYC, RHO_SIC, RHO_GR = 10.4, 1.1, 1.9, 3.18, 1.73
ENRICH_WT = 0.17
# TrisoSpec::HTR10_LI2014: kernel, buffer, IPyC, SiC, OPyC outer radii [cm]
R_TRISO = (0.0250, 0.0340, 0.0380, 0.0415, 0.0455)
PACKING = 0.050248114
R_FZ, R_PEB = 2.5, 3.0
E_EDGES = [1.0e-5, 0.625, 1.0e5, 2.0e7]


def ball(r):
    return 4.0 / 3.0 * math.pi * r ** 3


def pebble_mix():
    v = [ball(R_TRISO[0])] + [ball(R_TRISO[i]) - ball(R_TRISO[i - 1]) for i in range(1, 5)]
    v_p = sum(v)
    n_p = round(PACKING * ball(R_FZ) / v_p)
    v_matrix = ball(R_FZ) - n_p * v_p
    v_shell = ball(R_PEB) - ball(R_FZ)
    x5 = (ENRICH_WT / M_U235) / (ENRICH_WT / M_U235 + (1 - ENRICH_WT) / M_U238)
    m_u = x5 * M_U235 + (1 - x5) * M_U238
    mol_u = n_p * v[0] * RHO_UO2 / (m_u + 2 * M_O16)
    mol_c = ((v_matrix + v_shell) * RHO_GR / M_C
             + n_p * (v[1] * RHO_BUF / M_C + (v[2] + v[4]) * RHO_PYC / M_C
                      + v[3] * RHO_SIC / (M_C + M_SI)))
    vp = ball(R_PEB)
    n_u = mol_u * NA_B / vp
    print(f"HTR-10 pebble: {n_p} particles, U {mol_u * m_u:.4f} g, C {mol_c * M_C:.2f} g, "
          f"N_C/N_U = {mol_c / mol_u:.1f}")
    return {"U235": x5 * n_u, "U238": (1 - x5) * n_u}, mol_c * NA_B / vp


def natural_mix(cu):
    n_c = RHO_GR * NA_B / M_C
    n_u = n_c / cu
    return {"U234": NAT_U[0] * n_u, "U235": NAT_U[1] * n_u, "U238": NAT_U[2] * n_u}, n_c


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--case", required=True, choices=["pebble", "natural"])
    ap.add_argument("--cu", type=float, default=None)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--threads", type=int, default=3)
    ap.add_argument("--particles", type=int, default=20000)
    ap.add_argument("--inactive", type=int, default=20)
    ap.add_argument("--active", type=int, default=100)
    ap.add_argument("--workdir", required=True)
    ap.add_argument("--json", default=None, help="append one JSON line with the results")
    ap.add_argument("--openmc", default=os.path.expanduser(
        "~/Documents/research/openmcbin/bin/openmc"))
    a = ap.parse_args()

    if a.case == "pebble":
        u, n_c = pebble_mix()
    else:
        assert a.cu, "--cu is required for --case natural"
        u, n_c = natural_mix(a.cu)
    m = openmc.Material(name=f"U + graphite ({a.case})", temperature=T)
    for nuc, d in u.items():
        m.add_nuclide(nuc, d, "ao")
    m.add_nuclide("C12", C12_AF * n_c, "ao")
    m.add_nuclide("C13", C13_AF * n_c, "ao")
    m.set_density("sum")
    m.add_s_alpha_beta("c_Graphite")
    print("atom densities [1/b-cm]:", {k: f"{v:.5e}" for k, v in u.items()}, f"C {n_c:.5e}",
          f"N_C/N_U = {n_c / sum(u.values()):.2f}")

    planes = [openmc.XPlane(-HALF, boundary_type="reflective"),
              openmc.XPlane(HALF, boundary_type="reflective"),
              openmc.YPlane(-HALF, boundary_type="reflective"),
              openmc.YPlane(HALF, boundary_type="reflective"),
              openmc.ZPlane(-HALF, boundary_type="reflective"),
              openmc.ZPlane(HALF, boundary_type="reflective")]
    region = +planes[0] & -planes[1] & +planes[2] & -planes[3] & +planes[4] & -planes[5]
    geo = openmc.Geometry([openmc.Cell(fill=m, region=region)])

    s = openmc.Settings()
    s.run_mode = "eigenvalue"
    s.particles = a.particles
    s.inactive = a.inactive
    s.batches = a.inactive + a.active
    s.seed = a.seed
    s.source = openmc.IndependentSource(
        space=openmc.stats.Box((-HALF,) * 3, (HALF,) * 3),
        energy=openmc.stats.Watt(a=0.988e6, b=2.249e-6))
    s.temperature = {"default": T, "method": "nearest", "tolerance": 10.0}
    s.ptables = True
    s.resonance_scattering = {"enable": True, "method": "dbrc",
                              "energy_min": 1.0e-5, "energy_max": 1000.0}
    s.output = {"tallies": False, "summary": False}

    t = openmc.Tally(name="four_factor")
    t.filters = [openmc.EnergyFilter(E_EDGES)]
    t.nuclides = ["total"] + list(u.keys())
    t.scores = ["absorption", "nu-fission"]
    model = openmc.Model(geometry=geo, materials=openmc.Materials([m]), settings=s,
                         tallies=openmc.Tallies([t]))

    wd = pathlib.Path(a.workdir)
    wd.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    sp_path = model.run(cwd=wd, threads=a.threads, openmc_exec=a.openmc, output=False)
    wall = time.time() - t0
    with openmc.StatePoint(sp_path) as sp:
        k = sp.keff
        kgen = np.asarray(sp.k_generation)[a.inactive:]
        tt = sp.get_tally(name="four_factor")

        def rate(score, nuc):
            v = tt.get_values(scores=[score], nuclides=[nuc]).ravel()
            e = tt.get_values(scores=[score], nuclides=[nuc], value="std_dev").ravel()
            return v, e  # by group: thermal, resonance, fast

        a_tot, sa = rate("absorption", "total")
        p_tot, sp_ = rate("nu-fission", "total")
        a_u = sum(rate("absorption", n)[0] for n in u)
    eta = p_tot[0] / a_u[0]
    f = a_u[0] / a_tot[0]
    p = a_tot[0] / (a_tot[0] + a_tot[1])
    eps = (p_tot.sum() / p_tot[0]) * (a_tot.sum() - a_tot[2]) / a_tot.sum()
    p2 = a_tot[0] / a_tot.sum()
    eps2 = p_tot.sum() / p_tot[0]
    rel = lambda v, e: e / v
    # delta method, operands independent (conservative, as reactor_physics.rs)
    s_p = p * math.hypot(rel(a_tot[0], sa[0]), rel(a_tot[0] + a_tot[1], math.hypot(sa[0], sa[1])))
    out = {
        "case": a.case, "cu": a.cu, "seed": a.seed, "particles": a.particles,
        "inactive": a.inactive, "active": a.active, "threads": a.threads,
        "k": k.nominal_value, "k_std": k.std_dev, "k_gen_mean": float(kgen.mean()),
        "k_gen_sem": float(kgen.std(ddof=1) / math.sqrt(len(kgen))),
        "eta": eta, "f": f, "p": p, "p_std": s_p, "eps": eps,
        "k_factors": eta * f * p * eps, "p2": p2, "eps2": eps2,
        "A": a_tot.tolist(), "P": p_tot.tolist(), "A_U_th": float(a_u[0]),
        "wall_s": wall, "openmc": openmc.__version__,
    }
    print(json.dumps(out, indent=1))
    if a.json:
        with open(a.json, "a") as fh:
            fh.write(json.dumps(out) + "\n")


if __name__ == "__main__":
    main()
