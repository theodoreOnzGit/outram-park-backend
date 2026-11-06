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

### Addendum (written after the first run of the port, before these cases were generated)

The first run showed the KRG and KPLS fixed-θ numerics within 1e-13 of SMT
once one more upstream fact was ported (**finding 7**, found by that run:
KRG's `_get_pq()` returns (0, 0), so KRG's likelihood and σ² divide by n,
while MFK's divide by n − p − q; the port had used n − p for both). It also
showed every MFK fixed-θ level missing the 1e-8 criterion (2.7e-7 on σ² at
level 0 of `mfk_forrester_fixed`). Checked by hand before changing anything:
at θ = 0.5 the level-0 correlation matrix (11 points 0.1 apart) has
**cond(R) = 1.8e10**, and a pure-Python unblocked Cholesky in the port's
loop order reproduces the port's σ² (96791.98271647282) where LAPACK gives
SMT's (96791.95642930), so the gap is the factorisation order amplified by
the conditioning, not a defect. I chose θ = (0.5, 1.2) without checking
cond(R); that was my error in the case design. The linear-ρ case is worse:
with `rho_regr = poly = linear` the Forrester HF is **exactly** a linear
function of the LF and x, so the level-1 residual is zero and its σ²
(1.5e-14) is round-off; that case cannot test level 1 at all.

So, before generating anything further, the generator now also records
cond(R) (numpy, from SMT's own Cholesky factor) for every fixed case and
every fitted optimum, and the comparison criterion is applied as follows,
**fixed now**: a level with cond(R) ≤ 1e8 is held to 1e-8; a level with
cond(R) > 1e8 is reported with its gap and cond(R)·ε for scale and is not
counted as a code-to-code comparison (as gh:#1193 did for its singular
GP case). Two cases are ADDED, θ chosen as (10, 10) so that neighbouring
standardised points (≈ 0.3 apart) correlate at about exp(−0.9) ≈ 0.4:

- `mfk_forrester_wellcond_fixed`: the default options at θ = (10, 10);
- `mfk_forrester_quadratic_lf_linear_fixed`: `rho_regr = poly = linear`, HF
  at 6 points, and a modified LF `0.5 HF + 10 (x − 0.5)² − 5` so that the HF
  is not exactly representable (not one of SMT's problems: chosen only to
  give level 1 a non-zero residual).
- **P15a** (prediction for both): as P15, i.e. per-level likelihood ≤ 1e-12,
  σ² ≤ 1e-12, β, γ ≤ 1e-9, mean ≤ 1e-10, variance ≤ 1e-9 σ², σ²_ρ ≤ 1e-9.

## Results (measured 2026-11-04, `cargo test --release -p raffles`)

Numbers printed by `tests/smt_kriging_gh1190.rs` (`-- --nocapture`) and the
twin engine's `double_pipe::cokriging` test. rustc 1.99.1, x86_64.

### Findings 6 and 7 (found while porting, after the predictions)

6. **MFK's variance for three or more levels uses μ = 0 above level 1**
   (`predict_variances_all_levels` fills only `mu[:, 0]` but reads
   `mu[:, i − 1]`). From reading only; no three-level case is run here, and
   the port reproduces it.
7. **KRG divides its likelihood and σ² by n; MFK by n − p − q** (`KrgBased.
   _get_pq` returns (0, 0); MFK overrides it). The first port run used
   n − p for KRG and missed every KRG fixed case by up to 37 %; with the
   upstream rule, see below. So KRG's σ² is the ML estimate and MFK's the
   unbiased one, and a one-level MFK is a KRG with σ² larger by n/(n − p)
   (pinned by `mfk::tests::one_level_is_krg_and_undetermined_levels_are_refused`).

### Differential table (port against SMT, well-conditioned cases, cond(R) ≤ 1e8)

| Comparison | Bit-identical | Worst difference | Cause |
|---|---|---|---|
| Standardisation (KRG 8 cases, KPLS 2, MFK 5) | **82 / 82** | 0 | — (column sums sequential, `(n, 1)` sums pairwise, as numpy) |
| KRG reduced likelihood (8 fixed cases) | 0 / 8 | 1.7e-14 rel | LAPACK `dpotrf`/`dgeqrf` (OpenBLAS) against unblocked loops; `log10`, `pow` |
| KRG σ² | 0 / 8 | 3.7e-14 rel | as above |
| KRG β, γ (relative to largest) | 7 / 162 | 6.6e-14 | as above, times cond(R) ≤ 3.8e3 |
| KRG mean, 198 points (relative to max abs y) | 22 / 198 | 3.1e-14 | as above; `exp` (numpy SIMD against `petir`) |
| KRG variance (relative to σ²), incl. SMT's diag-G form | 3 / 198 | 1.1e-15 | as above |
| KRG likelihood gradient (relative to largest) | 0 / 15 | 5.6e-14 | as above; trace via an explicit R⁻¹ |
| KPLS `coeff_pls` | 2 / 24 | 4.2e-15 | BLAS dot order in NIPALS |
| KPLS likelihood / σ² / β, γ / mean / variance / gradient | 0/2, 0/2, 0/82, 2/50, 1/50, 0/4 | 5.8e-14, 2.9e-14, 8.3e-14, 2.3e-14, 1.2e-15, 1.6e-13 | as KRG |
| MFK per-level likelihood and σ² (2 cases × 2 levels) | 1 / 8 | 3.7e-15 | as KRG |
| MFK β, γ | 6 / 41 | 4.6e-15 | as KRG |
| MFK mean (21 points × 2) | 11 / 42 | 4.5e-16 | as KRG |
| MFK variance, every level | 5 / 84 | 8.0e-15 | as KRG |
| MFK σ²_ρ | 0 / 42 | 1.6e-15 | `inv(GᵀG)` by LU in SMT, by G⁻¹ here |

Every comparison on a well-conditioned case is below the 1e-8 criterion with
its cause stated; none is bit-identical in full, because LAPACK's blocked
factorisations are not reproduced (crate rule: no BLAS/LAPACK). **Flagged
for the maintainer:** the largest, 1.6e-13 (KPLS gradient), is explained.

**Ill-conditioned levels, reported and not counted** (addendum): the three
original MFK fixed cases (level 0 cond(R) 1.3e10–1.8e10): likelihood and σ²
2.7e-2 apart at worst, β, γ 1.5e-2, mean 2.9e-9, σ²_ρ 6.3e-9; the variance
of `mfk_forrester_linear_fixed`'s level 1, whose σ² is round-off (1.5e-14),
is 6.7e4 σ² apart. The demonstration holds: an unblocked Cholesky in Python
gives σ² = 96791.98271647282 at level 0 of `mfk_forrester_fixed`, the port's
96791.9827164728 (**1.5e-16** apart), against LAPACK's 96791.95642930
(2.7e-7). The gap is the factorisation order, amplified by cond(R).

### Fits (optimiser-dependent; compared at optima)

| Case | cond(R) at SMT's optimum | From SMT's starts: θ gap to polished, likelihood − SMT's | From the port's own starts | Prediction gap (rel. to max abs y) |
|---|---|---|---|---|
| `fit_doc_1d` | 1.0e1 | 1.9e-11; 0 | 8.1e-10; 0 | 3.1e-12 |
| `fit_branin_squar_exp` | 1.5e6 | 7.9e-7; +1.2e-11 | 1.4e-6; +1.4e-11 | 1.7e-7 |
| `fit_branin_matern52` | 1.2e7 | 1.0e-6; −2.2e-11 | 2.4e-6; +3.2e-11 | 1.5e-8 |
| `fit_rosenbrock_squar_exp` | **6.4e14** | 8.5e-4; −1.8e-4 | 5.9e-4; +4.0e-4 | 1.8e-6 |
| `kpls_fit` | 1.6e1 | 2.0e-8 (to SMT's, = polished); 0 to 10 digits | the same optimum | 1.8e-9 |
| `kplsk_fit` | 4.0e1 | θ up to 5.6e-6; +2e-10 | **a different, higher optimum**: likelihood 8.0798 against 7.1737, θ₁, θ₂ on the 1e-6 bound | 1.9e-6 (SMT starts); 0.37 (own) |
| MFK level 0 | 9.3e4 | 1.4e-8 to SMT's; equal to 10 digits | the same | — |
| MFK level 1 | **1.8e14** | both θ on the 1e-6 bound; likelihood −8.8e-5 | the same | — |

The polished optimum equals SMT's on three of the four KRG cases (TNC had
converged); on Branin squar_exp TNC stopped 2.9e-7 short. The two
ill-conditioned optima differ at the same θ already: on Rosenbrock the
port's likelihood **at SMT's polished θ** is 31.640111 against SMT's
31.643094, so no optimiser comparison is meaningful there.

### Forrester, MFK against single-fidelity KRG (101 held-out points)

| | Port, SMT's starts | Port, own starts | SMT |
|---|---|---|---|
| MFK RMSE; 2σ coverage | 0.05350437406; 88/101 | 0.05350437407; 88/101 | 0.05350437524; 88/101 |
| KRG (4 HF points) RMSE; 2σ coverage | 5.60205408; 59/101 | — | 5.60205408; 59/101 |

MFK is 105 times better than KRG at the same 4-point HF budget, and its 2σ
band covers 87 % of the held-out points against KRG's 58 %. The port agrees
with SMT to 2.2e-8 (MFK) and 5.6e-11 (KRG) in RMSE.

### Cross-check against the RAVEN/sklearn route

At the mapped hyperparameters (`branin_squar_exp`): the RAVEN mean plus m₀
equals SMT's to **7.6e-16** of max abs y; SMT's variance minus RAVEN's is
non-negative everywhere (worst negative part 0) and at most 2.8e-3 σ², the
GLS trend term σ²‖u‖² that sklearn's zero-mean GP does not have. Default
fits on the Branin 20 points, 100 held-out points (std 51.88): SMT KRG RMSE
2.752 (2σ coverage 0.94), RAVEN `Constant*RBF` anisotropic RMSE 1.544
(0.93), amplitude 8.5e4 and length scales (4.29, 18.09) in raw units. The
routes differ as models (GLS constant mean and profiled σ² against a zero
mean and a fitted amplitude on unstandardised y), and here RAVEN's is the
more accurate; neither is "the" kriging.

### Predictions scored

| # | Predicted | Measured | |
|---|---|---|---|
| P1 | standardisation bit-identical | 82 / 82 | hit |
| P2 | likelihood ≤ 1e-12 | 1.7e-14 (KRG), 5.8e-14 (KPLS), 3.7e-15 (MFK) | hit |
| P3 | σ² ≤ 1e-12; β, γ ≤ 1e-9 | 3.7e-14; 8.3e-14 | hit |
| P4 | mean ≤ 1e-10 | 3.1e-14 | hit |
| P5 | variance ≤ 1e-9 σ² | 1.2e-15 | hit |
| P6 | AsSmt as P5 | inside P5's 1.1e-15 | hit |
| P7 | Exact differs > 1e-3 | 2.2e-2 σ² | hit: finding 1 is real |
| P8 | gradient ≤ 1e-8 | 1.6e-13 | hit |
| P9 | from SMT's starts: likelihood ≥ SMT's − 1e-9, θ ≤ 1e-4 of polished | met on the three well-conditioned cases; **Rosenbrock missed** (−1.8e-4, 8.5e-4) | hit ×3, missed ×1 (cond(R) 6.4e14, a condition I did not foresee) |
| P10 | own starts, same optimum | same on the three well-conditioned; higher likelihood on Rosenbrock | hit |
| P11 | `coeff_pls` ≤ 1e-14 | 4.2e-15 | hit |
| P12 | KPLS fixed as P2–P5 | as the table | hit |
| P13 | KPLS fit as P9 | equal | hit |
| P14 | KPLSK: likelihood ≥ SMT's − 1e-6; held-out within 1 % | SMT's starts: hit (+2e-10; 4.5e-7 in RMSE); own starts: a higher optimum, RMSE 1397 against 1440 (3.0 %) | hit / **missed** (own starts) |
| P15 | MFK fixed as P2–P5 | the three original cases: **missed** (cond(R) 1.8e10; the linear case's level 1 is degenerate) | missed |
| P15a | the two added cases as P15 | ≤ 8.0e-15 throughout | hit |
| P16 | MFK fit θ ≤ 1e-4 or the same bound | 1.4e-8; both on 1e-6 | hit |
| P17 | Forrester study equal to SMT's | RMSE 2.2e-8 rel, coverage counts equal | hit |
| P18 | cross-check identities ≤ 1e-9 | 7.6e-16; 0 | hit |
| P19 | RMSEs differ, both < 25 % of std | 5.3 % and 3.0 % | hit |

### The double pipe (MFK on TUAS 1-D + CFD, 8 held-out CFD rows)

Run with `cargo test --release -p outram-park-digital-twin-engine --lib
double_pipe::cokriging -- --nocapture` (MFK fit 1111 s, KRG 35 s, on 2 of
4 shared CPUs; behind the twin engine's `long-tests` feature, on by
default). The low-fidelity grid (`lf_grid.csv`, 375 TUAS runs at nominal θ)
took 1686 s on 2 threads; two of its rows regenerate bit for bit
(`lf_grid::tests::the_committed_low_fidelity_grid_regenerates`).
Relative = held-out RMSE / mean |y| over the 8 rows.

| channel | MFK RMSE (rel.) / max / 2σ cov. | KRG, 36 CFD rows only | polynomial degree 1 (gh:#1194 record) |
|---|---|---|---|
| UA [W/K] | 0.0360 (3.0 %) / 0.0976 / 7/8 | 0.1667 (13.9 %) / 8/8 | 0.1328 (11.1 %) |
| Q [W] | 0.165 (0.56 %) / 0.268 / 7/8 | 3.82 (13.0 %) / 7/8 | 7.349 (25.1 %) |
| ε | 0.0086 (1.3 %) / 0.0159 / 8/8 | 0.170 (26.6 %) / 5/8 | 0.0880 |
| T_hot,out [K] | 0.376 / 0.552 / 8/8 | 2.90 / 6/8 | 3.503 |
| T_cold,out [K] | 0.198 / 0.440 / 8/8 | 5.86 / 6/8 | 3.489 |
| Δp_hot [Pa] | 0.00155 (0.56 %) / 0.00252 / **3/8** | 0.0276 (10.0 %) / 8/8 | 0.0311 (11.3 %) |
| Δp_cold [Pa] | 0.00157 (0.47 %) / 0.00341 / 8/8 | 0.180 (53.6 %) / 8/8 | 0.0430 (12.8 %) |
| Nu_hot | 0.146 (2.8 %) / 0.252 / 8/8 | 0.259 (5.0 %) / 5/8 | 0.2694 (5.2 %) |
| Nu_cold | 0.113 (1.7 %) / 0.221 / 7/8 | 0.209 (3.2 %) / 5/8 | 0.1512 (2.3 %) |

| # | Predicted | Measured | |
|---|---|---|---|
| D1 | UA, Q 1–6 % | 3.0 %, 0.56 % | UA hit; Q better than the range |
| D2 | ε 0.005–0.03; outlet T 0.3–1.5 K | 0.0086; 0.38 K, 0.20 K | hit; T_cold better than the range |
| D3 | Δp 1–8 % | 0.56 %, 0.47 % | better than the range |
| D4 | Nu 2–10 % | 2.8 %, 1.7 % | hit; Nu_cold better than the range |
| D5 | MFK beats the polynomial on ≥ 7 of 9 | 9 of 9 (by 1.3× on Nu_cold to 44× on Q) | hit |
| D6 | MFK beats KRG on ≥ 6 of 9; KRG UA 8–30 % | 9 of 9; KRG UA 13.9 % | hit |
| D7 | coverage 4/8–8/8 per output | 7/8 or 8/8 on eight outputs; **Δp_hot 3/8** | missed on Δp_hot |

**Which numbers to quote:** MFK's held-out RMSE (settings fixed before the
run, nothing revisited). **Interpretation.** The 1-D model's errors against
the CFD are large (UA 3–20 % low, annulus Δp 45–70 % low, gh:#1178) but
smooth, so a constant ρ plus a GP correction trained on 36 CFD rows removes
most of them, and the 375 LF rows carry the k_wall = 10 and interior-L
behaviour no CFD training row shows. Two cautions. The LF grid contains the
held-out points' x (geometric Re levels, k_wall 10, L 0.15–0.25), so MFK
sees the **1-D model** there (never the CFD), which is how co-kriging is
meant to be used but makes these 8 points easier than arbitrary ones. And
the Δp_hot band is too narrow (3/8 inside 2σ with a 0.56 % RMSE): the GP
variance from an ML θ is optimistic where the discrepancy is nearly
constant. This is a surrogate of the CFD, not a validation of either model;
the CFD itself carries about 1 % mesh error (gh:#1178).


## What this does not show

- SMT's TNC is not ported: fits agree at optima, not paths, and where TNC
  stops early (Branin, 2.9e-7) the port's answer is the converged one.
- `SmtStarts::Default` draws from `petir::rng::lcg`, not numpy's PCG64, so
  the port's own multistart points are not SMT's (they found the same optima
  except on KPLSK, where they found a higher one).
- Not ported: noise estimation, `eval_n_comp`, MFKPLS/MFKPLSK, derivatives,
  mixed-integer kernels, `pow_exp`, `squar_sin_exp`; three or more MFK levels
  are implemented but not compared with SMT (finding 6).
- No leave-one-out for the SMT routes (`ValidationReport::loo_rmse` is
  `None`).
- Nothing here is validation, and no human has reviewed it.
