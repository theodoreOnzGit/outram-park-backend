# #498 — UQ and sensitivity by sampling: V&V protocol (stub)

**Issue:** [gh:#498](https://github.com/theodoreOnzGit/outram-park-backend/issues/498), epic #493.
**Code:** `src/stats/uq.rs` (`UqDesign`, `UqReport`, `DirectPerturbation`);
`raffles::{distributions, samplers, sensitivity}`.

> **Status: NOT YET MEASURED (testing deferred by maintainer, 2026-10-03).**
> Nothing below has been run. No number on this page is a result.

## Scope rule

Quantification, not calibration. No input is adjusted toward a reference.
Any later Bayesian calibration follows the root `CLAUDE.md` model-hierarchy
order (uncalibrated → calibrated → ablation).

## What is being checked

That the sensitivity a sampled design produces matches an independent
reference: on a bare sphere, `dk/dρ` (ρ = a uniform scale on all atom
densities) from the marginal least-squares slope over sampled runs agrees
with a central-difference direct perturbation.

## Methodology

- Case: Godiva (ICSBEP HEU-MET-FAST-001) geometry and composition, as in
  `examples/godiva_keff_ensemble.rs`; the uncertain input is a density scale
  factor `f ~ Normal(1, 0.01)` applied to all three nuclides.
- Sampled arm: Latin hypercube, `N` points (fixed before the run), one
  independent seed per point, `UqReport::analyse` → `sensitivities[0].slope`.
- Reference arm: `DirectPerturbation::new(0.01, k(1.01), k(0.99))`, each an
  ensemble mean with its sem (#494), so its σ is small against the slope's.
- Analytic cross-check (reported, not gated): for a bare homogeneous sphere a
  uniform density scale `f` is equivalent to a radius scale `f` at fixed
  density (all macroscopic cross sections scale with `f`), so
  `f dk/df = R dk/dR`; the radius perturbation is a second, independent
  reference.

## Pass criterion (fixed in advance, 2026-10-03)

`DirectPerturbation::agreement(slope)`: `|z| ≤ 2`. Also reported: the noise
fraction (`mc_noise_variance / sd_total²`); if it is ≥ 0.5 the run is
noise-dominated, the comparison is not interpretable, and the result is
recorded as such rather than as a pass or a fail.

## Results

| case | date | N | slope dk/df | direct dk/df | z | noise fraction | pass |
|---|---|---|---|---|---|---|---|
| Godiva density scale | — | — | NOT YET MEASURED | | | | |
