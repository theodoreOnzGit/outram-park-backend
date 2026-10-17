#!/usr/bin/env python3
"""OpenMC reference for the outram-mc beginner Diagnostics on Godiva (gh #940).

ICSBEP HEU-MET-FAST-001 as `outram_mc_libs::vv::godiva` and the committed
`../../openmc_godiva_cross_code/godiva.py` model it (radius 8.7407 cm, the
spec's U-234/235/238 atom densities, 293.6 K), with the tallies the
Diagnostics read, computed the SAME way:

- three-group absorption and nu-fission (edges 0.625 eV and 0.1 MeV,
  0 .. infinity), and the leakage in the same groups (outgoing current on
  the vacuum sphere), all per source neutron;
- the fission rate on the 1000-bin equal-lethargy grid 1e-3 eV .. 20 MeV,
  turned into EALF with the same bin-midpoint rule (exp of the
  fission-weighted mean of the bins' mid-lethargy);
- the flux on that grid (normalised per unit lethargy);
- beta_eff and Lambda by IFP (ifp_n_generation = 10, OpenMC's default):
  beta = beta-numerator / denominator, Lambda = time-numerator /
  (denominator * k), docs/source/usersguide/kinetics.rst.

Data: the ENDF/B-VIII.0 HDF5 library from openmc.org (whatever
OPENMC_CROSS_SECTIONS points at), 294 K nearest to 293.6 K, URR probability
tables on (OpenMC's default). outram-mc processes the same evaluation itself
(RECONR/BROADR/PURR ports) at 293.6 K with DBRC on; OpenMC's run here has no
resonance elastic scattering (its default). Those are the known differences.

Usage:  python godiva_diagnostics.py [--particles N] [--active N] [--threads N]
Prints `key,value,sigma` lines.
"""
import os
import sys
import tempfile

import numpy as np
import openmc

ARGS = sys.argv[1:]


def arg(name, default):
    return int(ARGS[ARGS.index(name) + 1]) if name in ARGS else default


PARTICLES = arg("--particles", 50000)
ACTIVE = arg("--active", 300)
INACTIVE = arg("--inactive", 50)
THREADS = arg("--threads", 4)
SEED = arg("--seed", 1)

RADIUS = 8.7407  # cm, ICSBEP HEU-MET-FAST-001
TEMP_K = 293.6
DENS = {"U234": 4.9184e-4, "U235": 4.4994e-2, "U238": 2.4984e-3}  # atoms/b-cm
T_EDGE, F_EDGE = 0.625, 1.0e5
GROUPS = [0.0, T_EDGE, F_EDGE, 1.0e300]
FINE = np.exp(np.linspace(np.log(1.0e-3), np.log(2.0e7), 1001))

fuel = openmc.Material(name="Godiva HEU")
for nuc, n in DENS.items():
    fuel.add_nuclide(nuc, n, "ao")
fuel.set_density("atom/b-cm", sum(DENS.values()))
fuel.temperature = TEMP_K
sphere = openmc.Sphere(r=RADIUS, boundary_type="vacuum")
core = openmc.Cell(name="core", fill=fuel, region=-sphere)

settings = openmc.Settings()
settings.run_mode = "eigenvalue"
settings.particles = PARTICLES
settings.inactive = INACTIVE
settings.batches = INACTIVE + ACTIVE
settings.seed = SEED
settings.temperature = {"method": "nearest", "tolerance": 1000.0}
settings.source = openmc.IndependentSource(
    space=openmc.stats.Box((-RADIUS,) * 3, (RADIUS,) * 3, only_fissionable=True),
    energy=openmc.stats.Watt(a=0.988e6, b=2.249e-6),
)
settings.ifp_n_generation = 10
entropy = openmc.RegularMesh()
entropy.lower_left = (-RADIUS,) * 3
entropy.upper_right = (RADIUS,) * 3
entropy.dimension = (8, 8, 8)
settings.entropy_mesh = entropy

groups = openmc.EnergyFilter(GROUPS)
fine = openmc.EnergyFilter(FINE)
t_groups = openmc.Tally(name="groups")
t_groups.filters = [groups]
t_groups.scores = ["absorption", "nu-fission", "fission"]
t_leak = openmc.Tally(name="leak")
t_leak.filters = [openmc.SurfaceFilter(sphere), groups]
t_leak.scores = ["current"]
t_fine = openmc.Tally(name="fine")
t_fine.filters = [fine]
t_fine.scores = ["flux", "fission"]
t_ifp = openmc.Tally(name="ifp")
t_ifp.scores = ["ifp-time-numerator", "ifp-beta-numerator", "ifp-denominator"]

model = openmc.Model(
    geometry=openmc.Geometry([core]),
    materials=openmc.Materials([fuel]),
    settings=settings,
    tallies=openmc.Tallies([t_groups, t_leak, t_fine, t_ifp]),
)

work = tempfile.mkdtemp(prefix="godiva_diag_")
sp_path = model.run(cwd=work, threads=THREADS, output=False)
sp = openmc.StatePoint(sp_path)
k = sp.keff


def out(key, v, s=0.0):
    print(f"{key},{v:.9g},{s:.9g}")


out("k", k.n, k.s)
g = sp.get_tally(name="groups")
a = g.get_values(scores=["absorption"]).ravel()
sa = g.get_values(scores=["absorption"], value="std_dev").ravel()
p = g.get_values(scores=["nu-fission"]).ravel()
spp = g.get_values(scores=["nu-fission"], value="std_dev").ravel()
f = g.get_values(scores=["fission"]).ravel()
l = sp.get_tally(name="leak")
lv = l.get_values().ravel()
ls = l.get_values(value="std_dev").ravel()
for i, name in enumerate(["thermal", "resonance", "fast"]):
    out(f"absorption_{name}", a[i], sa[i])
    out(f"nu_fission_{name}", p[i], spp[i])
    out(f"leakage_{name}", lv[i], ls[i])
out("absorption_total", a.sum(), np.sqrt((sa ** 2).sum()))
out("fission_total", f.sum())
out("leakage_total", lv.sum(), np.sqrt((ls ** 2).sum()))
P, A, L = p.sum(), a.sum(), lv.sum()
out("P_over_A_plus_L", P / (A + L))
out("leakage_fraction_of_losses", L / (A + L))
fine_t = sp.get_tally(name="fine")
fis = fine_t.get_values(scores=["fission"]).ravel()
mid = 0.5 * (np.log(FINE[:-1]) + np.log(FINE[1:]))
out("ealf_ev", float(np.exp((fis * mid).sum() / fis.sum())))
flux = fine_t.get_values(scores=["flux"]).ravel()
out("flux_mean_ln_e", float((flux * mid).sum() / flux.sum()))
out("flux_fraction_below_100keV", float(flux[FINE[1:] <= 1.0e5].sum() / flux.sum()))
ifp = sp.get_tally(name="ifp")


def val(score):
    return ifp.get_values(scores=[score]).ravel()[0], ifp.get_values(scores=[score], value="std_dev").ravel()[0]


tn, ts = val("ifp-time-numerator")
bn, bs = val("ifp-beta-numerator")
dn, ds = val("ifp-denominator")
beta = bn / dn
out("beta_eff", beta, beta * np.hypot(bs / bn, ds / dn))
lam = tn / (dn * k.n)
out("lambda_s", lam, lam * np.sqrt((ts / tn) ** 2 + (ds / dn) ** 2 + (k.s / k.n) ** 2))
H = np.asarray(sp.entropy)
out("entropy_last_inactive", float(H[INACTIVE - 1]))
out("entropy_active_mean", float(H[INACTIVE:].mean()), float(H[INACTIVE:].std(ddof=1)))
out("particles", PARTICLES)
out("active", ACTIVE)
out("inactive", INACTIVE)
print(f"# openmc {openmc.__version__}, cross sections {os.environ.get('OPENMC_CROSS_SECTIONS')}, work dir {work}")
