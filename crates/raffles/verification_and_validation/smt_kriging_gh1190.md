# SMT's kriging family in `raffles`: KRG, KPLS/KPLSK and MFK co-kriging (gh:#1190)

Part of epic gh:#1177 (rung 2, and rung 4 through MFK, gh:#1181). AI-assisted
draft material under `RESPONSIBLE_USE.md`: no human has reviewed it.
**Verification only** ("is the port implemented correctly, against SMT
itself?"), plus one application (the double-pipe co-kriging) whose numbers
are a surrogate's errors against a CFD dataset, not a validation of any
physical model.

**Order of this record.** The upstream facts, the method and the predictions
below were committed **before any Rust port code existed**, with the
reference outputs; git history shows it. The results section is filled in
afterwards, from runs. The protocol and layout are gh:#1193's
(`raven_routes_gh1193.md`).

## Upstream, and what was run

| | |
|---|---|
| SMT | `SMTorg/smt`, branch `master`, commit `151a3ff3c13733d168b0ca8844ff0a4ff072252a` (2026-11-03), shallow clone, accessed 2026-11-04 |
| Licence | BSD-3-Clause, "Copyright (c) 2017, SMT developers" (`LICENSE.txt`, sha256 `091e8e50…e107c`), preserved verbatim as `LICENSE-BSD-SMT`; `pyproject.toml` says `license = "BSD-3-Clause"` |
| Python | 3.11.17, throwaway venv |
| numpy / scipy / scikit-learn | 2.4.6 / 1.17.1 / 1.9.1 (SMT pins none of them; these are what `pip` resolved on 2026-11-04) |
| Generator | `smt_kriging_gh1190/generate_references.py` |
| Outputs | `smt_kriging_gh1190/references/{krg,kpls,mfk}.json`, each with a `meta` block recording the above |

Command, from the workspace root (SMT imported from the clone: KRG, KPLS,
KPLSK and MFK are pure Python, so nothing was compiled):

```bash
SMT_DIR=<smt clone> <venv>/bin/python -I \
  crates/raffles/verification_and_validation/smt_kriging_gh1190/generate_references.py
```

Every model is SMT's own class. Fixed-θ cases use `hyper_opt="NoOp"`, which
evaluates SMT's likelihood, GLS weights, predictor and variance at θ0 with no
optimiser. Fit cases use SMT's defaults. The multistart points SMT hands its
optimiser are captured by wrapping `HyperparamOptimizer.optimize` (the wrapper
records its argument and calls the original).

## SMT's defaults, read from source (`krg_based/krg_based.py`, `krg.py`, `kpls.py`, `kplsk.py`, `applications/mfk.py`)

| Option | Default | Notes |
|---|---|---|
| `corr` | `squar_exp` | also `abs_exp`, `matern52`, `matern32` (and `pow_exp`, `squar_sin_exp`, not ported). All are **products of 1-D kernels** on component distances: `squar_exp` `exp(−Σ θ_i d_i²)`, `abs_exp` `exp(−Σ θ_i |d_i|)`, Matérn `Π(1 + √5 θ_i|d_i| + 5/3 θ_i² d_i²) exp(−√5 Σ θ_i|d_i|)` |
| `poly` | `constant` | `linear`, `quadratic` (universal kriging); GLS through a Householder QR of `C⁻¹F` |
| `nugget` | `100 ε` = 2.2e-14 | added to the correlation diagonal |
| `theta0` | `[1e-2]` | broadcast to every dimension |
| `theta_bounds` | `[1e-6, 20]` | on θ, optimised in log10 θ |
| normalisation | always | X and y standardised by mean and **sample** std (`ddof = 1`), a zero X std (below 100 ε) replaced by 1 |
| `hyper_opt` | `TNC` | scipy TNC, `maxfun = max(12 n_θ, 50)` per start; `Cobyla` the alternative |
| `n_start` | 10 | starts: θ0, one uniform draw in log10 bounds, then an `n_start` maximin LHS, all from `np.random.default_rng(41)` |
| likelihood | reduced, **log10** | `−(n − p − q) log10 σ² − n log10 det(R)^{1/n}`; σ² profiled; `+1e15` cap |
| KPLS | `n_comp = 1`, `squar_exp` (`abs_exp`, `pow_exp` allowed) | `coeff_pls = |x_rotations_|` of sklearn's `PLSRegression(n_comp)` fitted on the raw X, y |
| KPLSK | `squar_exp` only | KPLS optimisation, then θ_full = Σ_l θ_l coeff²_{·l} as θ0 of a full KRG optimisation (starts θ0 + one uniform, no LHS, `maxfun = 10 nx`); prediction in the full space |
| MFK | `rho_regr = constant`, `propagate_uncertainty = True`, `optim_var = False` | Le Gratiet's recursive AR(1): level 0 a KRG; level i a KRG with regression `[ρ(x) μ_{i−1}(x), f(x)]`; X, y standardised once over **all levels concatenated** |

### Upstream behaviours found by reading, before any port was written

1. **KRG's variance uses only the diagonal of G for universal kriging.**
   `_predict_variances` (`krg_based.py`) solves
   `solve_triangular(G.T, Ftᵀ rt − fᵀ)` without `lower=True`. `G` is the
   upper-triangular QR factor, so `G.T` is lower-triangular, and a solve that
   reads only the upper triangle sees its diagonal alone. For `poly =
   constant` (p = 1) this is exact; for `linear` / `quadratic` the trend term
   of the variance is wrong. MFK's variance (`predict_variances_all_levels`)
   passes `lower=True` and is right. The port reproduces SMT's form in the
   SMT mode (`TrendVariance::AsSmt`, the default, so that it can be compared)
   and offers the exact one beside it (`TrendVariance::Exact`); it does not
   "fix" the SMT mode.
2. **The likelihood gradient adds the nugget to a σ² already in output
   units** (`likelihood_eval.gradient`: `sigma_2 = par["sigma2"] + nugget`,
   where `par["sigma2"]` carries `y_std²`). With nugget 2.2e-14 it is
   negligible; the port keeps it.
3. **TNC is stopped at `maxfun = max(12 n_θ, 50)` per start**, so SMT's
   fitted θ need not be a converged optimum. The generator therefore also
   records a **polished** optimum: scipy's TNC restarted from SMT's answer on
   SMT's own objective with `maxfun = 10000`.
4. **MFK standardises with the concatenation of all levels**, so the
   high-fidelity level's `y` is scaled by a std dominated by the
   low-fidelity data.
5. **KPLS's PLS is on the raw (unstandardised) data** (sklearn rescales
   internally), while its distances are on SMT's standardised X.

## Method

### Routes compared, and the instrument for each

| Route | Instrument | Why this one |
|---|---|---|
| Standardisation | `X_offset`, `X_scale`, `y_mean`, `y_std` | the first arithmetic, and the cause of any later shift |
| KRG / KPLS / MFK at **fixed θ** | reduced likelihood, σ², β, γ, predictive mean and variance at test points, the likelihood gradient | optimiser-path-free: these are the numerics |
| PLS | `coeff_pls` | sklearn's NIPALS, ported |
| Fits | fitted θ and likelihood against SMT's and against the polished optimum; predictions after each code's own fit | the optimiser is **not** a port of TNC (below), so this compares optima, not paths |

**Optimiser.** SMT's TNC (scipy's C `tnc`) is not ported. The port reuses
`raffles`' bounded BFGS from the gh:#1193 GP route (generalised to take an
objective closure), on SMT's objective (−reduced likelihood in log10 θ, SMT's
gradient by the chain rule), run to convergence from each start. Starts:
`SmtStarts::Explicit` (the tests hand it SMT's recorded starts) or
`SmtStarts::Default` (θ0, one uniform and an `n_start` LHS from this crate's
`petir::rng::lcg` sampler: a different generator from numpy's PCG64, so not
SMT's points).

**Match criterion** (root `CLAUDE.md`, "Code-to-code"): bit-identical
preferred; otherwise below 1e-8 relative with the cause explained.
Quantities near zero are compared absolutely on the scale of their
neighbours (γ, β relative to the largest entry; variances relative to σ²).

### Cases (all in the JSON files)

- **KRG fixed θ** (`krg.json`): `doc_1d_squar_exp` (SMT's documentation
  example, θ 0.3); Branin, 20-point LHS (`criterion="ese"`, seed 1), θ (0.4,
  1.7), with each of `squar_exp`, `abs_exp`, `matern52`, `matern32` and
  constant trend, plus `squar_exp` with `linear` and `quadratic` trends;
  Rosenbrock 2-D, 20 points, `matern32`, linear, θ (0.8, 0.3).
- **KRG fits**, SMT defaults: the 1-D example; Branin `squar_exp` and
  `matern52`; Rosenbrock `squar_exp`; held-out sets of 100 uniform points
  (seeds 7, 8) with SMT's predictions and the true function values.
- **KPLS / KPLSK** (`kpls.json`): Rosenbrock 6-D, 40 points (seed 3),
  `n_comp = 2`: KPLS at fixed θ (0.05, 0.2) with `squar_exp` and `abs_exp`;
  KPLS fit; KPLSK fit; 100 held-out points (seed 9).
- **MFK** (`mfk.json`), Forrester: HF `(6x − 2)² sin(12x − 4)`, LF
  `0.5 HF + 10 (x − 0.5) − 5`; LF at 11 equispaced points, HF at
  {0, 0.4, 0.6, 1} (SMT's documentation example). Fixed θ (0.5, 1.2) with
  defaults; with `rho_regr = poly = linear` (HF at 6 equispaced points, since
  p + q = 4 needs n > 4); with `propagate_uncertainty = False`. A default fit,
  and SMT's single-fidelity KRG on the 4 HF points alone, both on 101 held-out
  points.

### Cross-check against the RAVEN/sklearn route (Rust only)

At fixed hyperparameters the two routes are the same predictor, once the
mapping is made explicit. SMT: `y ≈ m₀ + σ² r(x)ᵀ (σ²(R + νI))⁻¹ (y − m₀)` with
`m₀ = y_mean + y_std β`, `r` = `exp(−Σ θ_i ((x_i − x'_i)/s_i)²)`. The RAVEN
route with a fixed `ConstantKernel(σ²) * RBF(ℓ)`, `ℓ_i = s_i / √(2 θ_i)`,
`alpha = σ² ν`, no `normalize_y`, on `y − m₀` gives the same mean, and a
variance smaller by exactly SMT's GLS term `σ² ‖u‖²` (sklearn's GP has no
trend, so no trend uncertainty). The test builds both on Branin
(`branin_squar_exp`) and checks those two identities. Then each route is fitted
with its own defaults (SMT KRG; RAVEN `Constant*RBF`, anisotropic) on the
Branin 20 points and scored on the 100 held-out points: there the routes are
expected to differ, because the models differ (GLS constant mean and profiled
σ² against a zero mean and a free amplitude; REML-like `n − p` against ML;
nugget 2.2e-14 against alpha 1e-10).

### The double pipe (Rust only)

MFK on gh:#1178's pair, through `Surrogate::fit_multi` with a
`MultiFidelityDataset`:

- **Low fidelity:** the TUAS 1-D model at **nominal θ** (uncalibrated) on a
  375-point grid fixed now: Re_hot geometric 5 levels in [10, 90], Re_cold
  geometric 5 levels in [5, 45], k_wall ∈ {1, 10, 100}, L ∈ {0.10, 0.15,
  0.20, 0.25, 0.30} m (`outram_park_digital_twin_engine::multifidelity::
  double_pipe::lf_grid`). The geometric Re levels contain the held-out points'
  Re values; that puts *low-fidelity* values at the held-out x, which is what
  co-kriging is for. No held-out **CFD** value is read.
- **High fidelity:** the 36 CFD training rows of the committed dataset.
- **Check:** the 8 held-out CFD rows, never read by any fit (the
  `MultiFidelityDataset` levels' splits).
- **Settings, fixed now, not revisited:** SMT's MFK defaults (squar_exp,
  constant trend, constant ρ, nugget, θ bounds, `SmtStarts::Default` with
  n_start 10), raw SI inputs (the polynomial baseline's), one MFK per output.
  Comparators: SMT KRG (defaults) on the 36 HF rows alone, and the polynomial
  baseline of `fidelity_interface_gh1194.md` (degree 1, its quoted setting).
- **Which numbers to quote:** MFK's held-out RMSE per output.

## Predictions (written before the port existed)

The fixed-θ and fit predictions are about the port reproducing SMT. Note that
the reference JSON files were generated, and SMT's own held-out errors were
looked at (to check the generator ran), before these were written, so the
Forrester and Branin study predictions are code-to-code ones (port = SMT), not
forecasts of the methods. The double-pipe predictions are forecasts: nothing
had been run.

| # | Quantity | Prediction | Reason |
|---|---|---|---|
| P1 | `X_offset`, `X_scale`, `y_mean`, `y_std` | bit-identical | numpy's column sums along axis 0 are sequential; the y reduction is its pairwise sum, already ported (`polynomial_chaos::special::numpy_sum`) |
| P2 | reduced likelihood, fixed θ | ≤ 1e-12 relative | LAPACK `dpotrf`/`dgeqrf` (OpenBLAS, blocked) against unblocked loops; `log10`, `pow(·, 2/n)` ulps |
| P3 | σ²; β, γ (relative to the largest entry) | σ² ≤ 1e-12; β, γ ≤ 1e-9 | γ = R⁻¹ρ carries R's condition number (up to ~1e6–1e8 here) |
| P4 | predictive mean, fixed θ (relative to max abs y) | ≤ 1e-10 | as P3, with `exp` ulps (numpy AVX-512 against `petir`) |
| P5 | predictive variance (absolute, relative to σ²) | ≤ 1e-9 | `1 − ‖rt‖² + ‖u‖²` cancels near training points |
| P6 | KRG linear/quadratic trend, `TrendVariance::AsSmt` against SMT | as P5 | faithful port of finding 1 |
| P7 | `TrendVariance::Exact` against SMT's variance, `branin_linear` / `branin_quadratic` | differs by more than 1e-3 relative at some test point | finding 1 is not round-off |
| P8 | likelihood gradient, fixed θ (relative to largest) | ≤ 1e-8 | `tr(R⁻¹ dR)` is formed as `Σ (R⁻¹)_ij dR_ji` from an explicit inverse here, by two triangular solves of the full `dR` in SMT |
| P9 | KRG fits, from SMT's recorded starts | likelihood ≥ SMT's − 1e-9 (absolute, log10 units); θ within 1e-4 relative of the **polished** optimum | a converged BFGS reaches the same smooth optimum; SMT's TNC may stop short (finding 3) |
| P10 | KRG fits, from the port's own starts | the same optimum as P9 on all four cases | the likelihoods here are low-dimensional with one dominant mode |
| P11 | `coeff_pls` | ≤ 1e-14 relative | one NIPALS iteration (single output), `pinv` of a 2×2 = its inverse |
| P12 | KPLS fixed θ | as P2–P5 | as there |
| P13 | KPLS fit | as P9 | |
| P14 | KPLSK fit | likelihood ≥ SMT's − 1e-6; held-out predictions within 1 % of SMT's RMS | two passes; SMT's second pass starts from a TNC-truncated first pass, and a random point |
| P15 | MFK fixed θ, all three cases: per-level likelihood, σ², β, γ; mean; variance; σ²_ρ | as P2–P5 | the same kernels and GLS, recursively |
| P16 | MFK fit | each level's θ within 1e-4 relative of SMT's, or both on the same bound | SMT's level-1 θ is on its 1e-6 lower bound (a constant discrepancy); the port should find the same |
| P17 | Forrester study, port | MFK and KRG held-out RMSE and 2σ coverage equal to SMT's (RMSE within 1e-6 relative; coverage counts identical) | code to code |
| P18 | RAVEN cross-check at mapped hyperparameters | means agree ≤ 1e-9 relative to max abs y; `var_SMT − σ²‖u‖² = var_RAVEN` ≤ 1e-9 σ² | the identities above |
| P19 | RAVEN vs SMT default fits, Branin held-out | RMSEs differ (different models); both below 25 % of the held-out std of Branin | stated without running |

### Double pipe (forecasts; relative = held-out RMSE / mean |y| over the 8 held-out rows)

The polynomial baseline (degree 1) measured UA 11.1 %, Q 25.1 %, ε RMSE
0.088, outlet T 3.5 K, Δp 11–13 %, Nu 2.3–5.2 %.

| # | Quantity | Prediction | Reason |
|---|---|---|---|
| D1 | MFK UA, Q | 1–6 % each | the 1-D UA is low by 3–20 % but smoothly (gh:#1178 P3), a near-multiplicative bias a constant ρ plus a GP correction absorbs; and the LF grid carries the k_wall = 10 saturation that no HF row shows |
| D2 | MFK ε; outlet temperatures | RMSE 0.005–0.03; 0.3–1.5 K | follow D1 |
| D3 | MFK Δp (both sides) | 1–8 % | the 1-D/CFD Δp ratio is nearly constant per side (gh:#1178 P5, P6) |
| D4 | MFK Nu (both sides) | 2–10 % | the 1-D Nu is TUAS's correlation, whose ratio to the CFD's varies (+26 % to +58 % on the annulus) |
| D5 | MFK against the polynomial baseline | MFK lower held-out RMSE on at least 7 of 9 outputs | D1–D4 |
| D6 | single-fidelity KRG (36 HF rows) against MFK | MFK lower on at least 6 of 9 outputs; KRG UA 8–30 % | KRG must interpolate k_wall between 1 and 100 with no data at 10 |
| D7 | MFK 2σ coverage on the 8 rows | between 4/8 and 8/8 per output; not a calibrated 95 % | 8 points, and kriging variances from ML θ are usually optimistic |

## Results

Not yet run (filled in after the port).
