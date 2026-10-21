# Beginner `Diagnostics` on Godiva: checks and the OpenMC comparison

GitHub #940 (diagnostics), #941 (report zip), #942 (`advanced`), tutorial 2
(#943). Measured 2026-10-17. **Not yet human-reviewed.**

> Research, education and V&V only (`RESPONSIBLE_USE.md`). AI-assisted code
> and numbers, untrusted until reviewed.

## Methodology

**The model.** ICSBEP HEU-MET-FAST-001 (Godiva) as `vv::godiva`: a bare
sphere of radius 8.7407 cm, U-234 / U-235 / U-238 at 4.9184e-4 / 4.4994e-2 /
2.4984e-3 atoms per barn·cm, 293.6 K, built through the beginner API
(`examples/tutorial_material_and_tallies.rs`, tutorial 2).

**Our side.** ENDF/B-VIII.0 tapes from `reference-data/endf/`, processed by
this workspace's NJOY port at 293.6 K with URR probability tables and DBRC
on (the defaults). `MCSettings::Auto`, seed 1: the watcher at 1 000 per
cycle, then 250 active cycles of 10 000. `Diagnostics::reactor_physics()`
plus one spectrum per component. Single-thread CSG power iteration.

**What is checked, each a check that can fail** (gates at 4σ, none tuned):

| check | how |
|---|---|
| diagnostics change nothing | k per cycle with and without diagnostics, bit for bit |
| a diagnostic alone = beside the others | every diagnostic run alone and in the full set, results bit for bit (`tests/model_diagnostics.rs`, LOW tier) |
| two estimators of one k | track-length ν-fission per source neutron `P` against the power iteration's k |
| the balance closes | `(absorption + leakage) / ends − 1`, track-length losses against the analog count of how neutrons ended |
| the spectrum integrates to the total flux | whole-model spectrum, all bins, against the balance tally's total flux, to 1e-12 |
| IFP against the k-ratio route | β_eff against `../kinetics/godiva_k_ratio_2026_09_22.md` (688 ± 77 pcm) |
| six factors = k | **does not apply to Godiva**: no neutron is absorbed below 0.625 eV, so every factor is 0/0. The run says "undefined" with the reason; the identity `P/(A+L) × (A+L) = P` is checked instead. HST-009 (tutorial 3) is the thermal case where the factors are defined. |

**The OpenMC side (code-to-code).** `openmc_inputs/godiva_diagnostics.py`
(below, verbatim), written for this comparison from the committed
`../openmc_godiva_cross_code/godiva.py` model, with the tallies the
Diagnostics read, computed the same way (same group edges, the same
1000-bin grid, the same EALF midpoint rule, IFP with 10 generations,
`Λ = time numerator / (denominator × k)` as OpenMC's `kinetics.rst`).

| provenance | |
|---|---|
| deck | `openmc_inputs/godiva_diagnostics.py`, this repository (GPL-3.0), commit adding this file |
| OpenMC | 0.16.1-dev25, commit `d7d3284a1` (local build) |
| data | ENDF/B-VIII.0 HDF5 library from openmc.org (`endfb-viii.0-hdf5`, local copy; release date not recorded: **not re-checked**), 294 K nearest to 293.6 K, URR probability tables on, no resonance elastic scattering (OpenMC's defaults) |
| run | 50 000 particles × [50 inactive + 300 active], seed 1, 4 threads, 17 s |
| entropy | 8 × 8 × 8 mesh over the sphere's box: last inactive 8.0847 bits, active mean 8.0971 (sd 0.0052); the last inactive cycle sits 2.4 sd below the active mean, so 50 inactive cycles at 50 000 may be a little short. Not re-run longer. |

**Known differences of data and physics, not removed:** our data are this
workspace's own processing of the evaluation at 293.6 K with DBRC on;
OpenMC's are openmc.org's at 294 K without resonance elastic scattering.

## Results

Hardware: Intel i9-13900K (16 logical cores, shared with other agents), 62.5
GiB RAM, Linux, CPU only for transport (an RTX A5000 is present and unused).
Transport single-threaded; data processed on 4 threads.

### Checks

| check | result |
|---|---|
| k, Auto, seed 1 | **1.00099 ± 0.00089** (batch-means σ 0.00079), **the same bits as tutorial 1's Auto run** without diagnostics; +99 pcm from the benchmark, +1.17σ from the recorded −6 ± 5 pcm |
| `P` vs k | `P` = 0.99950 ± 0.00056: **−1.41σ** |
| balance closure | −9.2e-4 ± 4.4e-4: **−2.12σ** (see below) |
| spectrum total / balance flux − 1 | **+2.2e-16** |
| normalised spectrum integral | 1 to 1e-12 |
| IFP β_eff vs k-ratio | 647.6 ± 23.7 against 688 ± 77 pcm: **−0.50σ** |
| run health | 3 000 000 histories, 2.68 collisions each; 0 lost, 0 stuck, 0 unbounded escapes, 0 negative distances |
| `--fast` (smoke test) | k 1.00369 ± 0.00183 (tutorial 1's Fast, bit for bit); `P` −1.26σ, closure −1.11σ, β_eff −0.58σ |

**The balance closure at −2.1σ is not a resolved bias.** Two further runs
(`MCSettings::Custom` 10 000 × [50 + 250], seed 1, same model): with URR
**+0.94σ** (+4.3e-4 ± 4.5e-4), with URR ablated through `advanced()`
**−0.88σ**. Its σ is the per-cycle standard error, which ignores
cycle-to-cycle correlation. One thing this does flag for review, unmeasured
at this precision: the track-length tallies score **infinitely dilute**
cross sections (`Material::macro_xs`) while transport samples URR
probability-table bands (`macro_xs_total_urr`); OpenMC scores the sampled
ones. Filed as gh #953.

### Against OpenMC (per source neutron)

| quantity | outram-mc (Auto) | OpenMC | difference |
|---|---|---|---|
| k | 1.00099 ± 0.00089 | 1.00025 ± 0.00017 | +74 pcm, 0.8σ |
| absorption, 0.625 eV – 0.1 MeV | 0.02462 ± 0.00012 | 0.024626 ± 0.000045 | 0.0σ |
| absorption, > 0.1 MeV | 0.40478 ± 0.00025 | 0.405084 ± 0.000094 | −1.1σ |
| ν-fission, 0.625 eV – 0.1 MeV | 0.04494 ± 0.00021 | 0.044935 ± 0.000081 | 0.0σ |
| ν-fission, > 0.1 MeV | 0.95456 ± 0.00055 | 0.955285 ± 0.000208 | −1.2σ |
| leakage, 0.625 eV – 0.1 MeV | 0.01335 ± 0.00008 | 0.013297 ± 0.000033 | +0.6σ |
| leakage, > 0.1 MeV | 0.55902 ± 0.00034 | 0.559722 ± 0.000124 | −1.9σ |
| thermal (< 0.625 eV), all rates | 0 | 0 | — |
| `P/(A+L)` | 0.99773 ± 0.00046 | 0.99750 | +0.5σ (ours only) |
| EALF | 829 170 ± 850 eV | 829 067 eV | +0.1σ (ours only) |
| β_eff (IFP, 10 generations) | 647.6 ± 23.7 pcm | 643.7 ± 9.5 pcm | +0.2σ |
| Λ (IFP) | 5.609 ± 0.026 ns | 5.616 ± 0.011 ns | −0.3σ |
| flux mean ln(E/eV) | 13.67782 | 13.67756 | no σ computed |
| flux fraction below 100 keV | 0.032810 | 0.032818 | no σ computed |

**Interpretation.** Every diagnostic agrees with OpenMC within 2σ on this
case, with independently processed data. Leakage above 0.1 MeV is the
largest offset (−1.9σ, ours lower), the same direction as k (ours higher):
consistent with, not a measurement of, the open spectral residual of
`../openmc_godiva_cross_code/` (op-os8x). The comparison does not test the
six factors (undefined here), the maps (OpenMC's mesh tallies split
segments; ours bin them at their midpoint, a documented approximation of
this crate's regular-mesh filter), or anything thermal.

### Cost

Same machine, same seed, same k bit for bit: transport **10.9 s** without
diagnostics (tutorial 1) and **37.7 s** with `Diagnostics::reactor_physics()`
plus a per-component spectrum (tutorial 2): **3.5×**. Not decomposed. The
likely price is that each track-length segment evaluates the full
macroscopic cross-section set (`Material::macro_xs`) for the reaction-rate
tallies; this is recorded, not optimised (the crate's "cheapest correct
path" rule asks for a measured, exact fast path first).

### The report zip (Auto run)

31 files: `report.md`; `data/` `run_summary.csv`, `run_health.csv`,
`k_by_cycle.csv`, `neutron_balance.csv`, `neutron_balance_check.csv`,
`ealf.csv`, `leakage_3.csv`, `leakage_spectrum_3.csv`, `six_factors_4.csv`,
`six_factor_groups_4.csv`, `spectrum_5_whole_model{,_summary}.csv`,
`kinetics.csv`, `fission_map_7.csv`, `flux_map_8.csv`,
`spectrum_9_core{,_summary}.csv`; PNGs `k.png`, `entropy.png`,
`leakage_3.png`, `spectrum_5_whole_model.png`, `fission_map_7_all_{xy,xz}.png`,
five `flux_map_8_*.png`, `spectrum_9_core.png`, `spectra.png`. Written to
`target/tutorials/godiva_diagnostics/` (gitignored); not committed.

## The OpenMC deck, verbatim

```python
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
```
