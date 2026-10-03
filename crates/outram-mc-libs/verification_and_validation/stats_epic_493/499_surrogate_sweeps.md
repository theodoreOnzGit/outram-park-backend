# #499 — surrogate-accelerated sweeps: V&V protocol (stub)

**Issue:** [gh:#499](https://github.com/theodoreOnzGit/outram-park-backend/issues/499), epic #493.
**Code:** `src/stats/sweep.rs` (`SweepSurrogate`, `SurrogatePrediction`,
`LooGate`); `raffles::surrogate::{PolynomialSurrogate, validation}`.
**Runner:** `examples/stats_sweep_loo.rs` (reads a committed CSV; runs no
transport).

> **Status: NOT YET MEASURED (testing deferred by maintainer, 2026-10-03).**
> Nothing below has been run. No number on this page is a result.

## Rule

A surrogate value is never reported as a transport result. It is printed and
typed as a `SurrogatePrediction` with its jackknife+ interval and its distance
to the nearest full run.

## Data

`crates/nee_soon/verification_and_validation/htr10_seker_2026_10_01_10k/results_table.csv`:
22 full runs, 11 loading heights × {ENDF/B-VIII.0, VII.0}, 10 000 histories ×
[5 + 135]. One surrogate per library.

## Methodology and criterion (fixed in advance, 2026-10-03)

- Gated model: polynomial in `built_height_cm`, total degree 2. Degrees 1 and
  3 are reported as a degree study, not as alternative gates.
- #499 gate: leave-one-out error within the per-run MC `σ`.
- **Two readings, both reported**, because the literal one cannot be met by a
  correct surrogate: a held-out residual includes the held-out run's own
  noise, so a perfect surrogate's expected LOO RMS is about
  `σ·sqrt(1 + p/(n − p))` (≈ `1.15 σ` at `n = 11`, `p = 3`).
  1. Literal: `loo_rmse ≤ rms(σ_i)`.
  2. Noise-floor-aware: `χ²/point ÷ (1 + p/(n − p))` inside the two-sided
     95 % band of `χ²_n / n` (approximate; LOO residuals are not
     independent).
  Which reading #499 means is **for the maintainer to decide**; it is raised
  on the issue and not settled by looking at the numbers.

## Results

| library | date | degree | LOO RMSE (pcm) | rms σ (pcm) | literal | χ²/pt | floor | floor-aware |
|---|---|---|---|---|---|---|---|---|
| VIII.0 | — | 2 | NOT YET MEASURED | | | | | |
| VII.0 | — | 2 | NOT YET MEASURED | | | | | |
