# #499 — surrogate-accelerated sweeps: V&V protocol (stub)

**Issue:** [gh:#499](https://github.com/theodoreOnzGit/outram-park-backend/issues/499), epic #493.
**Code:** `src/stats/sweep.rs` (`SweepSurrogate`, `SurrogatePrediction`,
`LooGate`); `raffles::surrogate::{PolynomialSurrogate, validation}`.
**Runner:** `examples/stats_sweep_loo.rs` (reads a committed CSV; runs no
transport).

> ~~**Status: NOT YET MEASURED (testing deferred by maintainer, 2026-10-03).**
> Nothing below has been run. No number on this page is a result.~~
> **CORRECTED 2026-10-07** (gh:#782): the gate was run once, on the
> 2026-10-07 HTR-10 record (see Data and Results). The rows in Results are
> measured; nothing else on this page is a result.

## Rule

A surrogate value is never reported as a transport result. It is printed and
typed as a `SurrogatePrediction` with its jackknife+ interval and its distance
to the nearest full run.

## Data

`crates/nee_soon/verification_and_validation/htr10_seker_2026_10_07_10k/results_table.csv`:
22 full runs, 11 loading heights × {ENDF/B-VIII.0, VII.0}, 10 000 histories ×
[5 + 135], on the bounded delta-tracking majorant (gh:#589). One surrogate per
library.

~~`crates/nee_soon/verification_and_validation/htr10_seker_2026_10_01_10k/results_table.csv`~~
**CORRECTED 2026-10-07** (gh:#782): superseded by the record above (same
statistics and columns); the 2026-10-01 runs were measured on a majorant
under-bound 14× at 661 eV. No result was recorded on them.

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
| VIII.0 | 2026-10-07 | 2 | **352** | 107 | **FAIL** | 10.94 | 1.38 | **FAIL** (7.9× floor) |
| VII.0 | 2026-10-07 | 2 | **330** | 106 | **FAIL** | 10.39 | 1.38 | **FAIL** (7.5× floor) |

Run: `cargo run --release -p outram-mc-libs --example stats_sweep_loo`,
worktree of `develop` at `065bd710`. Degree study (not gates): degree 1
fails both readings (LOO RMSE 1418 / 1567 pcm); degree 3 has LOO RMSE 168 /
148 pcm, literal FAIL, floor-aware PASS (χ²/pt 2.50 / 2.16 against floor
1.57).

**Interpretation.** The gated degree-2 surrogate fails the #499 gate on both
libraries in both readings. Its LOO error, about 3.2 σ, is far above the
≈ 1.15 σ a correct surrogate would show, so this is lack of fit, not the
literal reading's built-in excess: a quadratic in height does not carry
k(H) to MC precision on these runs. Degree 3 passing the floor-aware reading
does not change the gate; the degree was fixed in advance. Which reading
#499 means, and whether the gated degree should be revisited for a future
sweep, remain the maintainer's call.
