# PETIR depletion integrators: code-to-code against OpenMC, and measured orders of convergence

gh:#950, 2026-10-17. PETIR `src/depletion/` (OpenMC `openmc/deplete/` at
`d7d3284a1`, MIT). Research, education and V&V only.

> **Unverified until reviewed.** AI-assisted draft; not yet human-reviewed.

## 1. Predictions, written before any order was measured

The problem (`tests/depletion_common/mod.rs`): three nuclides, constant power
(fission rate `S = 0.3`), so the flux `phi = S / (sigma_f N_A)` depends on the
composition, and the decay and reaction parts of `A(n)` do not commute
(`lambda_A = 0.5`, `lambda_B = 2`). The exact answer is closed-form (because
`phi N_A` is constant). Uniform steps over `T = 1 s`, CRAM48, error = the
largest relative error of the three densities at `T`.

| integrator | predicted order | where the prediction comes from |
|---|---|---|
| Predictor | 1 | upstream docstring, "first-order" (`integrators.py:18-20`) |
| CE/CM | 2 | upstream docstring, "second order" (`:61`) |
| CF4 | 4 | upstream docstring, "fourth order" (`:116`) |
| CE/LI | 2 | local-error argument: the corrector interpolates `A` linearly between the step ends, so `A` is wrong by `O(h^2)` mid-step; local error `O(h^3)` |
| EPC-RK4 | 2 | a single exponential of the RK4-weighted average matrix drops the Magnus commutator term, which is `O(h^3)` locally when `A(t1)` and `A(t2)` do not commute |
| LE/QI | 3 | quadratic interpolation of `A` through `A(t_{i-1})`, `A(t_i)` and the predicted `A(t_{i+1})`: `A` wrong by `O(h^3)`, local `O(h^4)` |
| SI-CE/LI | 2 | as CE/LI; the iterations only refine the end-point rates, the interpolation is still linear |
| SI-LE/QI | 3 | as LE/QI |

**CF4 docstring order (ablation):** applying the two final exponentials in the
order the docstring's product implies (the `f4` matrix first), as the
pre-move `outram-mc-libs` CF4 did, flips the sign of the commutator term the
pair is built to reproduce. Predicted order **2**.

**Pre-move outram-mc CF4 on its own problem** (`DepletionChain::simple`, 1 MW
in 1000 cm^3, 40 d; `tests/depletion_step_convergence.rs` recorded
**1.94**): that 1.94 is predicted to be the docstring-order defect rather than
the composition dependence the test's comment proposes. Predicted after the
fix: **well above 2, toward 4**, on the same three step sizes, if Xe-135 is in
its asymptotic regime there (the test chose steps below its half-life for
that reason).

## 2. Code-to-code against OpenMC's running code

*Methodology* — `openmc_inputs/integrators_reference.py` runs OpenMC's own
unmodified `integrate()` (OpenMC 0.16.1-dev25 `d7d3284a1`, numpy 2.5.3, scipy
1.18.1, Python 3.14) for all eight integrators, four unequal steps
(0.1, 0.25, 0.2, 0.45 s), `n_steps = 10` for SI. Shims, both outside the
arithmetic: `StepResult.save` replaced by an in-memory recorder;
`pool.USE_MULTIPROCESSING = False`. Run twice: upstream `CRAM48` (SuperLU),
and `dense_ipf48`, a pure-Python replica of PETIR's dense solver passed
through upstream's `solver=` argument. Output: `openmc_inputs/integrators_reference.txt`.
Gate: `tests/depletion_vs_openmc.rs`.

*Criteria, fixed before the run* — bit-identical against `dense_ipf48` for
every time, density, `k` and rate in every record; below `1e-10` relative
against upstream `CRAM48` on every density; equal operator-call totals.

*Results (first run, 2026-10-17)*

- **Bit-identical, all eight integrators, all 40 records** (4 steps + the
  final record each), every time, density, `k` and rate. The integrator
  logic — stage order, matrix combinations, the LE/QI coefficients at
  unequal steps, the SI running average, clipping, the driver loop — is
  upstream's to the bit.
- **`k`'s standard deviation** bit-identical in 35 of 40 records; the five
  others are SI records, where upstream propagates with `uncertainties`
  (DEVIATION 7).
- **Against upstream's SuperLU CRAM48:** worst relative density difference
  per integrator 2.29e-15 (Predictor), 1.40e-15 (CE/CM), 3.30e-15 (CF4),
  2.81e-15 (CE/LI), 1.79e-15 (EPC-RK4), 3.28e-15 (LE/QI), 2.51e-15
  (SI-CE/LI), 5.86e-15 (SI-LE/QI). This is the price of DEVIATION 2 (dense
  elimination instead of SuperLU): a few ulp.
- **Operator calls** equal on both sides: 5, 9, 17, 9, 17, 12, 45, 45 for
  the 4-step runs, as `Integrator::operator_calls` states.
- `pow(dt, 2) == dt * dt` for every step used (DEVIATION 5 changes nothing
  here).

## 3. Measured orders of convergence

(filled in by section 3's run; see below)
