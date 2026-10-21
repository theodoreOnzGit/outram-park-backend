# #497 — variance reduction from learned importance: V&V protocol (stub)

**Issue:** [gh:#497](https://github.com/theodoreOnzGit/outram-park-backend/issues/497), epic #493.
**Code:** `src/stats/learned_importance.rs` (`learned_weight_windows`,
`VrArm`, `FomComparison`); `raffles::surrogate::{PolynomialSurrogate,
validation::LeaveOneOut}`.

> **Status: NOT YET MEASURED (testing deferred by maintainer, 2026-10-03).**
> Nothing below has been run. No number on this page is a result.

## Off by default — and why

This is an **estimator**, not physics: weight windows play fair split /
roulette games that leave every expected score unchanged. So, as for
`physics/variance_reduction.rs`, analog is the correct default and the root
`CLAUDE.md` "correct physics is the default" rule does not apply. Nothing here
runs unless a caller builds the windows and attaches them with
`VarianceReduction::with_weight_windows`.

## What is being checked

1. **Unbiasedness (the gate).** The tally mean with learned windows agrees
   with the analog mean. A biased answer fails regardless of FOM.
2. **Figure of merit.** `FOM = 1/(R²T)`, `R` the relative error of the tally,
   `T` the wall-clock, compared against (a) analog and (b) plain MAGIC
   windows (`WeightWindows::update_magic`) built from the **same** early
   tally. Ratio reported with its uncertainty
   (`FomComparison::fom_ratio_sigma`).
3. **Fill quality.** Per energy group: resolved / filled / clamped / empty
   cell counts, and the surrogate's leave-one-out RMSE in `ln φ`.

## Methodology

- Case: the shielded room of `tests/openmc_notebooks/shielded_room_weight_window.rs`
  (31 × 38 mesh, the case `variance_reduction/fom_2026_09_24.md` measured
  MAGIC on), so the MAGIC arm has a recorded precedent. Same geometry,
  materials, source and mesh in all three arms.
- Early tally: the analog arm's first batches (count fixed before the run);
  windows built once, then held fixed for the measured run.
- All three arms on one machine, hardware recorded (crate `CLAUDE.md`).

## Pass criteria (fixed in advance, 2026-10-03)

- **Unbiased:** `|z| ≤ 2` (`UNBIASED_Z_BAND`), `z = Δmean / sqrt(σ_a² + σ_v²)`,
  for the headline tally and for every mesh cell both arms resolve (with the
  expected ~5 % of cells outside 2σ by chance stated, not hidden).
  **Open question for the maintainer:** #497 says "within combined σ"; read
  literally as 1σ, a correct estimator fails 32 % of the time. 2σ is used
  here and flagged, decided before any result exists.
- **FOM:** reported, with uncertainty, against analog and MAGIC. No FOM
  threshold is a pass/fail gate — the issue asks for a measured ratio.

## Results

| arm | date | mean | R | T (s) | FOM | FOM / analog | z vs analog | hardware |
|---|---|---|---|---|---|---|---|---|
| analog | — | NOT YET MEASURED | | | | | | |
| MAGIC | — | NOT YET MEASURED | | | | | | |
| learned | — | NOT YET MEASURED | | | | | | |
