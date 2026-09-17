<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

# CRP-6 Case 1 — Walk-on-Spheres kernel release vs Crank analytical

**Generated:** 2026-07-23T00:28:53Z
**Crate version / commit:** boon-lay 0.1.2, working tree atop `ae84a04` (Phase 4)

## Methodology

**What is computed.** The fractional release of cesium from a bare spherical
UO2 fuel kernel (IAEA CRP-6 Case 1: single-layer diffusion, uniform initial
concentration, perfect-sink surface), evaluated at 200 h for two kernel
temperatures (Case 1a: 1200 °C; Case 1b: 1600 °C).

**Lagrangian side (this crate).** `mc_kernel_release_fraction`
(`release_fraction_crp_6_case_1a_1b/simulation_code.rs`) places `N = 40 000`
atoms uniformly through the kernel volume ($r = R\,U^{1/3}$, isotropic
direction) and walks each to the perfect-sink surface with the Walk-on-Spheres
first-passage engine (`first_passage/walk_on_spheres.rs::walk_to_absorbing_sphere`).
An atom's release time is the sum of its per-hop first-passage times; the release
fraction is the count with release time $\le 200$ h divided by $N$. There is
**no timestep** — the engine is event-driven, which is precisely what removes the
buffer-overshoot failure of the old single-Gaussian step
(`docs/buffer_clt_failure_analysis.md`).

**Reference side (Eulerian continuum).** The Crank closed-form series for
release from a sphere with uniform initial concentration and a zero-concentration
surface,

$$\frac{M_t}{M_\infty} = 1 - \frac{6}{\pi^2}\sum_{n=1}^{\infty}\frac{1}{n^2}\exp\left(-\frac{D n^2 \pi^2 t}{R^2}\right),$$

as implemented in `release_fraction_analytical_solution.rs`
(`calculate_analytical_fraction_released`, 200 terms).

**Inputs.** Kernel radius $R = 212.5\ \mu\text{m}$ (425 µm diameter). Diffusion
coefficient $D$ from the temperature-dependent Jiang correlation for Cs in UO2
(`temperature_dependent_collisions`, zero neutron fluence). Nuclide Cs-137.

**Pass criterion.** The Monte-Carlo release must agree with the Crank analytical
value to within Monte-Carlo statistics, $|MC - \text{Crank}| < 0.02$ (the
$1\sigma$ binomial error at $N = 40\,000$ near $p = 0.5$ is $\approx 0.0025$).
The analytical value is additionally checked against the literature range cited
for these cases (Hales et al. 2021, Table 4).

## Reference

```bibtex
@article{hales2021bison,
  title   = {Modeling fission product diffusion in {TRISO} fuel particles with {BISON}},
  author  = {Hales, J. D. and Jiang, W. and Toptan, A. and Gamble, K. A.},
  journal = {Journal of Nuclear Materials},
  volume  = {548},
  pages   = {152840},
  year    = {2021},
  note    = {Table 4 — Cs release from UO2 kernel, single-layer cases}
}
@book{crank1975diffusion,
  title     = {The Mathematics of Diffusion},
  author    = {Crank, J.},
  edition   = {2nd},
  publisher = {Clarendon Press},
  year      = {1975},
  note      = {Release from a sphere, uniform initial concentration (p.~91)}
}
@techreport{jiang2023fission,
  title       = {Fission product transport in {TRISO} particles and pebbles},
  author      = {Jiang, W. and Toptan, A. and Hales, J. D. and Spencer, B. W. and Novascone, S. R.},
  number      = {INL/EXT-21-63549-Rev001},
  institution = {Idaho National Laboratory},
  year        = {2023},
  note        = {Kernel diffusion-coefficient correlation, Table on p.~13}
}
```

## Results

Measured on the generation date above (`cargo test -p boon-lay --release
crp6_case1 -- --nocapture`), $N = 40\,000$ histories, RNG seed `0x0C1A5EED`:

```csv
case,temperature_C,D_m2_per_s,mc_release,crank_release,abs_error
1a,1200,2.2519e-15,0.5332,0.5337,0.0006
1b,1600,1.2503e-13,1.0000,1.0000,0.0000
```

**Interpretation.** The Lagrangian Walk-on-Spheres release fraction reproduces
the Crank continuum solution to within $6\times10^{-4}$ at 1200 °C and exactly
(to four decimals) at 1600 °C — far inside the $0.02$ criterion and consistent
with the expected Monte-Carlo statistical error. Both analytical values sit in
the literature-cited ranges (~0.53 at 1200 °C; ~1.0 at 1600 °C). This verifies
the first-passage engine (geometry, first-passage-time sampling, uniform-in-ball
birth, perfect-sink capture) against a known closed-form solution for the
single-layer TRISO case, and establishes the Lagrangian ⟷ Eulerian consistency
that the two release models (this crate's random walk and the `triso_atops_fork`
continuum models) are meant to provide.

**Scope / limitations.** This is the *single-layer* case (bare kernel). The
multilayer release through the buffer/IPyC/SiC/OPyC coatings — where the SiC
interface barrier dominates — exercises the interface physics validated
separately (`interface_uniform_equilibrium_density.md`) and is a heavier
computation; a full multilayer CRP-6 release record is future work. The
steady-state release-to-birth `<R/B>` comparison against the `triso_atops_fork`
Booth models (a different, continuous-production scenario) is likewise deferred.
