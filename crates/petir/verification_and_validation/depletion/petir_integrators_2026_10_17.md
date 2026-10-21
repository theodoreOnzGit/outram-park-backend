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

*Methodology* — `tests/depletion_integrator_orders.rs`: the problem of
section 1, `T = 1 s`, `N = 4 ... 1024` uniform steps, CRAM48,
`DepletionIntegrator::integrate`, SI `n_steps = 10`. Error = largest relative
error of the three densities at `T` against the closed form; observed order
`log2(e(N)/e(2N))`.

*Criterion, fixed with the predictions* — the order between `N = 128` and
`N = 256` within 0.1 of the predicted order, for each integrator and for the
docstring-order CF4.

*Results (2026-10-17, first run)* — every prediction held.

| integrator | predicted | observed order, N = 4→8 … 512→1024 | at 128→256 | error at N = 256 |
|---|---|---|---|---|
| Predictor | 1 | 0.850, 0.912, 0.952, 0.974, 0.987, 0.993, 0.997, 0.998 | 0.993 | 3.89e-3 |
| CE/CM | 2 | 1.815, 1.913, 1.959, 1.981, 1.991, 1.995, 1.998, 1.999 | 1.995 | 9.30e-6 |
| CF4 | 4 | 3.629, 3.835, 3.924, 3.964, 3.982, 3.981, 3.640, −0.913 | 3.981 | 1.67e-11 |
| CE/LI | 2 | 2.126, 2.126, 2.087, 2.051, 2.027, 2.014, 2.007, 2.004 | 2.014 | 1.34e-6 |
| EPC-RK4 | 2 | 1.976, 1.993, 1.998, 2.000, 2.000, 2.000, 2.000, 2.000 | 2.000 | 3.00e-6 |
| LE/QI | 3 | 2.960, 3.037, 3.041, 3.028, 3.016, 3.009, 3.005, 3.007 | 3.009 | 8.44e-9 |
| SI-CE/LI | 2 | 2.014, 1.999, 1.997, 1.998, 1.999, 1.999, 2.000, 2.000 | 1.999 | 6.46e-6 |
| SI-LE/QI | 3 | 2.753, 2.847, 2.915, 2.956, 2.977, 2.989, 2.994, 3.001 | 2.989 | 5.85e-8 |
| CF4, docstring order (ablation) | 2 | 1.976, 1.994, 1.998, 2.000, 2.000, 2.000, 2.000, 2.000 | 2.000 | 6.01e-6 |

*Interpretation*

- **CF4 is fourth order** until its error reaches a round-off floor of
  ~1.3e-12 at `N = 512` (2560 CRAM48 solves of a 3x3 system); the last two
  "orders" are that floor, not the method. The gate pair 128→256 sits above it.
- **The CF4 docstring order is second order.** The sign of the commutator
  term matters exactly as predicted. The pre-move `outram-mc-libs` CF4 used
  that order, which is why it measured 1.94 on its own burnup history; with
  the port it measures **3.930** there (prediction "well above 2, toward 4"
  held; `crates/outram-mc-libs/tests/depletion_step_convergence.rs`).
- EPC-RK4 is the clearest case of an integrator whose name overstates its
  order on a non-commuting problem: four transport solves per step for second
  order, the same as CE/CM's two.
- The CF4 by-hand harness used for the ablation reproduces
  `Integrator::Cf4` bit for bit in the code order, so the ablation differs
  from the integrator only in the exponential order.

*Limitations* — one problem, one functional (densities at `T`), uniform
steps for the orders (the code-to-code run covers unequal steps). The problem
has no stiffness of note; on a stiff chain the asymptotic range starts at
smaller steps (outram-mc's Xe-135 record). A deterministic operator: the SI
methods' purpose, averaging Monte Carlo noise, is not exercised here.

## 4. Bit-identity of the move (outram-mc-libs)

`crates/outram-mc-libs/tests/depletion_move_fingerprints.rs`, pins printed on
the pre-move code (`357033acbe`): CRAM16/48, `mat_vec`, `combine`, the
notebook predictor case, `deplete_with` and `deplete_independent` with
Predictor and CE/CM, and `deplete_coupled` — **all bit-identical after the
move**. The two CF4 cases changed, deliberately; a verbatim replay of the
removed CF4 on the moved CRAM reproduces the old CF4 pin exactly, so the
change is the algorithm alone.
