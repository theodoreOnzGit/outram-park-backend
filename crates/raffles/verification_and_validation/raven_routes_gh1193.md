# RAVEN first: the calibration and surrogate routes in `raffles` (gh:#1193)

Part of epic gh:#1177. AI-assisted draft material under `RESPONSIBLE_USE.md`:
no human has reviewed it. **Verification only** ("is the port implemented
correctly, against its upstream and against closed forms?"); nothing here
validates a physical model.

**Order of this record.** The upstream facts, the method and the predictions
below were committed **before any Rust port code existed**; git history
shows it. The results section was filled in afterwards, from runs.

## Upstream, and what was run

| | |
|---|---|
| RAVEN | `idaholab/raven`, branch `devel`, commit `8baaae3a97f1392318074ad6d3f8efc26ca81c7e` (2026-11-03), shallow sparse clone, accessed 2026-11-04 |
| Python | 3.12.3, in a throwaway venv |
| numpy / scipy / scikit-learn | 1.26.4 / 1.14.1 / 1.3.2 (RAVEN's own pins in `dependencies.xml`: 1.26, 1.14, 1.3) |
| Generator | `raven_routes_gh1193/generate_references.py` (+ `ravenimport.py`) |
| Outputs | `raven_routes_gh1193/references/{mcmc,pce,quadrature,gp}.json`, each with a `meta` block recording the versions above |

Command, from the workspace root:

```bash
RAVEN_DIR=<raven clone> <venv>/bin/python -I \
  crates/raffles/verification_and_validation/raven_routes_gh1193/generate_references.py
```

**How RAVEN was run without building it.** `ravenimport.py` replaces
`ravenframework/__init__.py` (which imports the whole XML driver) with a bare
package, stubs the retired C++ `crow_modules` (RAVEN's default stochastic
environment is numpy, `utils/randomUtils.py:37`), and stubs optional
packages that RAVEN modules import at load time but that nothing called here
uses (the list is recorded in each JSON's `meta.stubbed_imports`: AMSC, dask,
h5py, lxml, matplotlib, pandas, pyDOE3, statsmodels, xarray, ...). A call into
any stub raises, so a stub cannot silently produce a number.

**What is RAVEN's own code, and what is a transcribed driver:**

- **MCMC.** Real `Metropolis` / `AdaptiveMetropolis` objects; their own
  `_useRealization` (the acceptance log-ratio), `_checkAcceptance`,
  `tuneScalingParam`, `_updateAdaptiveParams` and
  `constructProposalDistribution` run, as do RAVEN's `Distributions.Normal`,
  `Uniform`, `MultivariateNormal` (PCA method) and `randomUtils`. Only the
  sampler loop (`localGenerateInput` / `localFinalizeActualSampling`), which
  needs the job handler and DataObjects, is transcribed, with upstream line
  numbers in the script. Every proposal draw and every acceptance uniform is
  recorded.
- **Polynomial chaos.** RAVEN's `Distributions`, `Quadratures`
  (`SmolyakSparseGrid`, `TensorGrid`, Gauss–Legendre and Gauss–Hermite sets),
  `IndexSets`, `OrthoPolynomials` and `GaussPolynomialRom._train` /
  `__evaluateLocal__` / `__mean__` / `__variance__` / `getSensitivities` /
  `getTotalSensitivities` run unmodified; the script wires them as
  `Samplers/SparseGridCollocation.py` does, with its DEFAULT quadrature and
  polynomial choice per distribution.
- **Gaussian process.** RAVEN's `GaussianProcessRegressor` ROM picks the
  kernel (its `pickKernel` / `customKernel`, over its vendored
  scikit-optimize kernels); sklearn's `GaussianProcessRegressor` does the
  numerics with RAVEN's defaults (alpha 1e-10, no restarts, `normalize_y`
  False, L-BFGS-B).

## What RAVEN has natively, and what it delegates (read from `devel` @ 8baaae3)

| Capability | RAVEN file(s) | Native or delegated |
|---|---|---|
| Metropolis, adaptive Metropolis | `Samplers/MCMC/{MCMC,Metropolis,AdaptiveMetropolis}.py` | **native** (numpy for arithmetic; proposals are RAVEN `Distributions`) |
| MVN proposal (adaptive Metropolis) | `DistributionsND.py` `MultivariateNormalPCA` | native, over **numpy SVD** (`mathUtils.computeTruncatedSingularValueDecomposition`) and `scipy.stats.norm.ppf` |
| 1-D distributions in the samplers | `Distributions1D.py` | **delegated** to `scipy.stats` |
| Polynomial chaos expansion | `SupervisedLearning/GaussPolynomialRom.py` | **native** (one `scipy.spatial.KDTree` lookup to match training rows to grid points) |
| Orthogonal polynomials | `OrthoPolynomials.py` | **delegated**: `scipy.special.eval_legendre`, `eval_hermitenorm`, ... ; RAVEN adds the normalisation |
| Gauss quadrature nodes and weights | `Quadratures.py` | **delegated**: `scipy.special.orthogonal.p_roots`, `he_roots`, ... |
| Smolyak / tensor sparse grids, index sets | `Quadratures.py`, `IndexSets.py` | **native** |
| HDMR | `SupervisedLearning/HDMRRom.py` | native, on top of `GaussPolynomialRom` |
| Gaussian process regression | `SupervisedLearning/ScikitLearn/GaussianProcess/GaussianProcessRegressor.py` | **delegated** to `sklearn.gaussian_process.GaussianProcessRegressor`; RAVEN owns the kernel menu, the custom-kernel string parser, the defaults and `evaluateGradients` |
| GP kernels | `contrib/SKOpt/GPKernels.py` | **vendored scikit-optimize** (BSD-3-Clause), subclasses of sklearn's kernels adding `gradient_x`; the kernel values are sklearn's |
| Bayesian optimisation | `Optimizers/BayesianOptimizer.py` | native loop; requires a RAVEN `GaussianProcessRegressor` ROM (line 226) and `scipy.optimize` |

### Data assimilation in RAVEN: none

Searched `ravenframework/` for Kalman, ensemble Kalman, EnKF, particle
filter, assimilation. **RAVEN has no data-assimilation capability**: no
Kalman filter, no ensemble filter, no particle filter. The hits are:

- `Models/PostProcessors/Validations/PhysicsGuidedCoverageMapping.py:322`:
  the docstring of `pcmTdep` says "uncertainty quantification and data
  assimilation", but the method is an SVD of the time series plus a k-nearest-
  neighbour regression of the coefficients, a validation post-processor, not
  a sequential filter.
- `SupervisedLearning/ARMA.py:1293` and `TSA/ARMA.py:589,600`: comments that
  borrow from `statsmodels.tsa.statespace.kalman_filter` to *simulate* a
  fitted VARMA, not to assimilate data.

So FilterPy (gh:#1191) and OpenDA (gh:#1192) carry that capability alone.

### Upstream behaviours found by reading, before any port was written

1. **Bounds are CLAMPED, not rejected** (`Metropolis.py:137-142`,
   `AdaptiveMetropolis.py:274-285`): a proposal outside a prior's bounds is
   moved onto the bound and evaluated there, while the acceptance ratio
   still treats the proposal as symmetric. That leaves an atom of
   probability on the bound, so the chain's stationary distribution is not
   the posterior when the posterior has mass near a bound.
2. **Adaptive Metropolis needs `<initial>`**: the ensemble mean is formed
   from the still-unset values (`AdaptiveMetropolis.py:215`) before the
   initial values are drawn (`:219-238`). The generator ran this case:
   RAVEN raises `TypeError: unsupported operand type(s) for -: 'float' and
   'NoneType'` (recorded in `meta.adaptive_no_initial`).
3. **With the default `burnIn = 0`, adaptive Metropolis tunes and adapts at
   once**: `_updateAdaptiveParams` only switches the scale tuning off at
   `counter == burnIn` (`:366`), which never happens for 0.
4. **At `counter == burnIn` the tuned scale replaces `lambda`, but the
   proposal is not rebuilt** until the next `adaptiveInterval` multiple
   (`:366-381`), so for up to 19 steps the proposal is `lambda_0 Σ_0` at
   scaling 1.
5. **RAVEN's GP returns no standard deviation by default.** `<multioutput>`
   defaults to True, wrapping the GP in sklearn's `MultiOutputRegressor`,
   whose `predict` takes no `return_std`; `ScikitLearnBase.__evaluateLocal__`
   catches the `TypeError` and returns the mean alone. The generator ran
   RAVEN's own `__evaluateLocal__`: keys `["y"]`, no `y_std`
   (`meta.raven_gpr_multioutput`).
6. **RAVEN's GP default kernel is not "Constant".** The input spec says "If
   None is passed, the kernel Constant is used as default"; RAVEN passes
   `None` to sklearn, whose default is `1.0 (fixed) * RBF(1.0, fixed)`, so
   the default GP has **no hyperparameter to optimise**.
7. `OrthoPolynomials.Hermite.setMeasures` passes `quad.type` (a string)
   where `OrthogonalPolynomial.setMeasures` expects the quadrature object, so
   Hermite polynomials on any non-Hermite quadrature raise. Not reached by
   the default choices; not ported.
8. `Quadratures.tensorGrid` calls `np.product`, removed in numpy 2.0, which
   is one reason RAVEN pins numpy 1.26.

## Method

### Routes compared, and the instrument for each

| Route | Instrument | Why this one |
|---|---|---|
| MCMC, deterministic part | **replay**: the Rust sampler consumes RAVEN's recorded proposal increments and acceptance uniforms, and every step's state, `alpha`, decision, acceptance rate, scaling (and for adaptive Metropolis `lambda`, ensemble mean and covariance) is compared | RAVEN draws through numpy's MT19937 and `scipy.stats.norm.ppf`; this crate draws through `petir::rng::lcg` (crate rule). The streams cannot match, so the arithmetic is separated from the randomness and compared exactly |
| MCMC, random part | analytic conjugate posteriors, run on the Rust LCG route | the only check of the Rust draws and colouring |
| Gauss rules | nodes and weights against `scipy.special.roots_legendre` / `roots_hermitenorm`, n = 1..20 | RAVEN delegates to these |
| Polynomials | `eval_legendre`, `eval_hermitenorm`, n = 0..12 at ten points including the `|x| < 1e-5` series branch | RAVEN delegates to these |
| PCE | index sets, grid points and weights (in RAVEN's insertion order), coefficients, mean, variance, Sobol and total indices, predictions | the whole of `GaussPolynomialRom` |
| GP numerics | at **fixed hyperparameters** (`optimizer=None`, or the default kernel): mean, std, log marginal likelihood and its gradient | optimiser-path-free |
| GP fit | fitted hyperparameters and LML against sklearn's L-BFGS-B | the Rust optimiser is NOT a port of L-BFGS-B, so this compares optima, not paths |

**Match criterion** (root `CLAUDE.md`, "Code-to-code"): bit-identical
preferred; otherwise below 1e-8 relative with the cause explained and
reported. Quantities that are differences near zero (`alpha`, near-zero PCE
coefficients, Sobol indices that are exactly zero) are compared absolutely,
on the scale of the quantity's neighbours, and say so.

### Cases

- **MCMC** (`mcmc.json`): `metropolis_conjugate_1d` (N(0, 2) prior, five
  N(θ, 1) observations, log likelihood, 400 steps, burn-in 50, tune interval
  100); `metropolis_bounded_linear_2d` (Uniform(0, 1) and N(1, 0.5) priors,
  a LINEAR likelihood so RAVEN takes `np.log` of it, initial values drawn
  from the priors, tune interval 25, 300 steps; exercises the clamp);
  `adaptive_correlated_2d` (N(0, 3) priors, a bivariate Gaussian likelihood
  with correlation 0.8, 600 steps, burn-in 100, tune interval 50, adaptive
  interval 20); `adaptive_no_burn_in_2d` (the same, burn-in 0, 300 steps).
- **PCE** (`pce.json`): `total_degree_poly3_uniform_normal` (Uniform(−1, 3) ×
  N(2, 0.5), total degree 3, Smolyak, a cubic); `tensor_product_sudret_3d`
  (Sudret's polynomial, tensor-product order 2, Smolyak);
  `hyperbolic_cross_ishigami` (order 8); `total_degree_ishigami_8` (2097
  points); `tensor_grid_anisotropic` (full tensor grid, importance weights
  2 : 1).
- **GP** (`gp.json`): `raven_default_kernel_1d`;
  `rbf_anisotropic_fixed_normalized_2d` (length scales 0.7, 1.6,
  `normalize_y`); `matern25_fixed_2d` (RAVEN's Matern is ν = 2.5);
  `constant_rbf_white_fixed_2d` (`Constant*RBF+WhiteNoise` at 2.0, 0.9,
  0.01); `rbf_fit_1d`; `constant_rbf_anisotropic_fit_2d`.

### Independent analytic checks (no upstream involved)

- **A1** Normal–Normal conjugate (the MCMC 1-D case's model): posterior
  N(5.3/5.25, 1/5.25) = N(1.0095238, 0.4364358²). Rust Metropolis on the
  LCG route, 200 000 steps.
- **A2** Bivariate Gaussian likelihood (mean (1, −1), unit variances,
  correlation 0.8) with independent N(0, 3²) priors: the posterior is
  Gaussian with precision Σ_L⁻¹ + I/9. Rust adaptive Metropolis, LCG route.
- **A3** The clamp: Uniform(0, 1) prior, Gaussian likelihood N(0.05, 0.1²).
  The exact posterior is a normal truncated to [0, 1], mean 0.100916.
- **A4** PCE polynomial reproduction: the cubic reproduced at test points,
  and Sudret's indices S_i = 25/91, S_ij = 5/91, S_123 = 1/91, mean 1,
  variance 0.728.
- **A5** GP noise-free interpolation: at the training points the mean equals
  the data and the standard deviation is near zero.

## Predictions (written before the port existed)

| # | Quantity | Prediction | Reason |
|---|---|---|---|
| P1 | MCMC accept/reject decisions, replay | identical in every step of all four cases | a flip needs `alpha` within an ulp of `ln u` |
| P2 | MCMC states (proposed and current θ) | **bit-identical** | `θ + d·s` and the clamp are the same IEEE operations on the same inputs |
| P3 | MCMC `alpha` | bit-identical in most steps; worst absolute difference ≤ 1e-12 | `np.log(scipy pdf)` against `ln(raffles pdf)`: the Gaussian pdf and `ln` may differ by an ulp |
| P4 | MCMC scaling, acceptance rate | bit-identical | depend only on the decisions and counters |
| P5 | adaptive `lambda`, ensemble mean and covariance | ≤ 1e-12 relative; bit-identical where every `alpha` is | `exp(alpha)` carries P3's ulps into `lambda` |
| P6 | Gauss–Legendre / Gauss–Hermite nodes and weights, n ≤ 20 | ≤ 4 ulp (≤ 1e-15 relative, nodes near 0 absolute ≤ 1e-16) | the eigensolver differs (GSL implicit QR in `petir` against LAPACK) before scipy's single Newton step; `exp`/`log` in the weight normalisation |
| P7 | `eval_legendre`, `eval_hermitenorm` | bit-identical on the recurrence; ≤ 2 ulp on the `|x| < 1e-5` Legendre series | same recurrence; the series uses cephes `beta` |
| P8 | index sets | identical, in the same order | integer rules |
| P9 | PCE grid points; weights | points bit-identical given equal nodes, else ≤ 1e-15 relative; weights ≤ 1e-14 relative | weights are sums of products of rule weights |
| P10 | PCE coefficients, mean, variance, Sobol, predictions | ≤ 1e-12 relative; coefficients that are zero in exact arithmetic ≤ 1e-13 absolute on the case's scale | sums of node-function products |
| P11 | GP mean at fixed hyperparameters | ≤ 1e-10 relative to max abs(y) | Cholesky operation order (LAPACK `dpotrf` via OpenBLAS against an unblocked loop), amplified by condition numbers up to ~1e10 at alpha = 1e-10 |
| P12 | GP std at fixed hyperparameters | ≤ 1e-9 absolute (unit prior variance) | `k(x,x) − vᵀv` cancels near training points |
| P13 | GP LML; its gradient | ≤ 1e-10 relative; ≤ 1e-8 relative | as P11 |
| P14 | GP fit, `rbf_fit_1d` | length scale within 1e-4 relative of sklearn's, LML within 1e-8 relative | a different optimiser reaching the same smooth optimum |
| P15 | GP fit, `constant_rbf_anisotropic_fit_2d` | LML at least sklearn's − 1e-6; hyperparameters may differ (sklearn went to amplitude 316², length scales ~24: a flat ridge) | path-dependent |
| A1 | conjugate posterior, Metropolis | mean within 4 batch-means standard errors of 1.0095238; sd within 3 % of 0.4364358 | |
| A2 | bivariate posterior, adaptive Metropolis | means within 4 batch-means standard errors; covariance entries within 8 % | |
| A3 | clamp | RAVEN's clamp leaves a positive fraction of samples exactly at 0 and a mean more than 4 SE below 0.100916; rejecting at the bound leaves none, and its mean within 4 SE | |
| A4 | PCE reproduction | cubic within 1e-12 relative; Sudret indices, mean, variance within 1e-13 absolute | the grid integrates the products exactly |
| A5 | GP interpolation | `abs(mean − y)` ≤ 1e-6 · max abs(y) at every training point; std ≤ 1e-3 | alpha = 1e-10 |

### Addendum to the predictions (written after the first run, before this case was generated)

The first run showed that `constant_rbf_anisotropic_fit_2d` ends where the
amplitude sits on its upper bound (1e5) with alpha 1e-10, so the Gram matrix
is singular to working precision and the LML there is round-off: this port's
LML at sklearn's own optimum is 10.858 where sklearn reports 10.926. That case
cannot test the fit (P15 is reported as missed, with that cause). One
well-conditioned fit case is therefore ADDED, chosen without running it:

- `matern_fit_2d`: RAVEN's `Matern` (ν = 2.5, isotropic, length scale from 1,
  bounds 1e-5..1e5), alpha 1e-6, the 2-D design of the other cases.
- **P16** (prediction): fitted length scale within 1e-4 relative of
  sklearn's; LML within 1e-8 relative.

**Gates versus predictions.** The tests gate on the criterion fixed before
the run: bit-identical, or below 1e-8 relative (or absolute, on the stated
scale) with the cause explained. The tighter numbers in the table above are
predictions, scored hit or miss below; the first draft of the tests asserted
the predictions themselves, and after P6 missed they were changed to assert
the criterion, so that a missed prediction is reported rather than either
hidden or turned into a moved gate.

## Results (measured 2026-11-04, `cargo test --release -p raffles`)

Numbers printed by `tests/raven_routes_gh1193.rs` and the unit tests named
in each row (`-- --nocapture`). rustc 1.99.1, x86_64 with AVX-512.

### Differential table (port against RAVEN / scipy / numpy / sklearn)

| Comparison | Bit-identical | Worst difference | Cause of the difference |
|---|---|---|---|
| MCMC decisions (4 chains, 1596 steps) | 1596 / 1596 | — | — |
| MCMC proposed + current states | **5600 / 5600** | 0 | — |
| MCMC `alpha` (absolute) | 1091 / 1596 | 7.1e-15 | `ln pdf`: numpy's AVX-512 `log`/`exp` against `petir`'s; measured separately, numpy's AVX-512 kernels differ from glibc's in ~24 % of arguments, so no libm choice here can match them |
| MCMC acceptance rate + scaling | **3196 / 3196** | 0 | — |
| adaptive `lambda` (relative) | 114 / 898 | 1.9e-14 | `exp(alpha)` carries the `alpha` ulps |
| adaptive ensemble mean + covariance | **12 / 12** | 0 | — |
| Gauss nodes, n = 1..20, Legendre + Hermite (relative to the rule's largest) | 330 / 420 | 2.2e-16 (1 ulp) | the eigensolver (below) |
| Gauss weights (relative to the rule's largest) | 33 / 420 | 1.3e-14 | scipy's weights use the derivative at the UNPOLISHED eigenvalues. Shown by substitution: fed scipy's own `eigvals_banded` eigenvalues, the port gives **every node bit-identical** and weights within 5e-16 (the residue is `log`/`exp`). The port's Sturm bisection is within 8e-16 of LAPACK's eigenvalues (GSL's QR, `petir::linalg::eigen_symm`, was within 2.1e-15 and gave 3.4e-14 on the weights, so it was replaced). Exact agreement needs LAPACK `dsterf`, not ported |
| `eval_legendre`, `eval_hermitenorm` (incl. the cephes series) | **260 / 260** | 0 | — |
| PCE index sets (5 cases) | all identical | — | — |
| PCE grid points | 6903 / 6919 | 8.3e-16 | node ulps |
| PCE grid weights (relative to largest) | 188 / 2321 | 1.8e-15 | rule weights |
| PCE coefficients (relative to largest) | 18 / 255 | 1.1e-14 | rule weights, summed |
| PCE mean + variance | 2 / 10 | 2.0e-15 | as above |
| PCE Sobol, total, partial/variance (absolute) | 8 / 81 | 1.8e-15 | as above |
| PCE predictions | 0 / 9 | 6.5e-13 | as above, at off-grid points of the order-8 Ishigami expansions |
| GP mean, fixed hyperparameters (relative to max abs y) | 1 / 19 | 4.4e-16 | unblocked Cholesky against LAPACK `dpotrf`/OpenBLAS; `exp` |
| GP std, fixed hyperparameters (absolute) | 1 / 19 | 1.1e-13 | the cancellation in `k(x,x) − vᵀv` |
| GP LML | 1 / 4 | 1.4e-15 | as the mean |
| GP LML gradient (relative to largest) | 0 / 6 | 1.8e-14 | as the mean |
| GP fit `rbf_fit_1d` | — | length scale 1.0158032028 vs 1.0158032027 (1.3e-10 rel); LML 5.3e-14 rel; mean 7.8e-11 | a different optimiser (bounded BFGS against L-BFGS-B) at the same optimum; sklearn's stopping tolerance is the looser |
| GP fit `matern_fit_2d` (added, P16) | — | length scale 2.0e-7 rel; LML 3.0e-14 rel; mean 1.2e-8 | as above |
| GP fit `constant_rbf_anisotropic_fit_2d` | — | LML 6.5e-3 rel apart; length scales 24.75, 21.10 vs 24.67, 23.16 | **not a valid comparison**: the amplitude ends on its 1e5 bound over alpha 1e-10, so the Gram matrix is singular to working precision; the two codes' LML at the SAME theta differ by 1.0e-2 (10.858 vs 10.926) |

Every code-to-code difference is below the 1e-8 criterion, and every one has
the cause stated. Flagged for the maintainer: the weights (1.3e-14) and the
GP std (1.1e-13) are the largest, both explained.

### Predictions scored

| # | Predicted | Measured | |
|---|---|---|---|
| P1 | decisions identical | 1596 / 1596 | hit |
| P2 | states bit-identical | 5600 / 5600 | hit |
| P3 | `alpha` ≤ 1e-12 abs | 7.1e-15 | hit |
| P4 | scaling, rate bit-identical | 3196 / 3196 | hit |
| P5 | ≤ 1e-12 rel | 1.9e-14; mean/cov bit-identical | hit |
| P6 | nodes/weights ≤ 4 ulp | nodes 1 ulp; **weights 1.3e-14 (~60 ulp)** | **missed** (weights): I did not foresee that the weights inherit the unpolished eigenvalues |
| P7 | recurrence bit-identical; series ≤ 2 ulp | all bit-identical | hit |
| P8 | index sets identical | identical | hit |
| P9 | points bit-identical given equal nodes; weights ≤ 1e-14 | points 8.3e-16 (nodes not all equal), weights 1.8e-15 | hit, conditionally |
| P10 | ≤ 1e-12 | coefficients 1.1e-14; **predictions 6.5e-13** | hit |
| P11 | mean ≤ 1e-10 | 4.4e-16 | hit |
| P12 | std ≤ 1e-9 | 1.1e-13 | hit |
| P13 | LML ≤ 1e-10, gradient ≤ 1e-8 | 1.4e-15, 1.8e-14 | hit |
| P14 | ls 1e-4, LML 1e-8 | 1.3e-10, 5.3e-14 | hit |
| P15 | LML ≥ sklearn's − 1e-6 | 10.855 vs 10.926 | **missed**; case uninformative (ill-conditioned), see the addendum |
| P16 | ls 1e-4, LML 1e-8 | 2.0e-7, 3.0e-14 | hit |

### Analytic checks

| # | Predicted | Measured (LCG route, Rust only) | |
|---|---|---|---|
| A1 | conjugate posterior: mean within 4 SE of 1.0095238, sd within 3 % of 0.4364358 | mean 1.008005 (z = −0.72, batch-means SE 0.0021), sd 0.435763 (−0.15 %); acceptance 0.356, final scale 3.68 | hit |
| A2 | adaptive, correlated posterior: means within 4 SE, covariance within 8 % | means 0.98728 / −0.96204 vs ±0.97826 (z = 1.05, 1.93); covariance entries −0.008 %, −0.37 %, −0.31 %; acceptance 0.235 (target 0.234) | hit |
| A3 | clamp: atom at 0 and mean > 4 SE low; reject: no atom, mean within 4 SE of 0.100916 | **clamp: 99.87 % of samples exactly at 0, mean 0.00012** (z = −3325); reject: none at 0, mean 0.10053 (z = −0.99) | hit, and far worse than predicted |
| A4 | Sudret ≤ 1e-13 abs; cubic ≤ 1e-12 rel | 2.0e-15; 7.9e-14 | hit |
| A5 | interpolation ≤ 1e-6 · max abs y, std ≤ 1e-3 | ≤ 9.9e-10, std 1.0e-5 (= √alpha) | hit |

**A3 is a finding about RAVEN, not about the port.** With the clamp, a
proposal past the bound lands ON it and is evaluated there; with the PyMC3
scale tuning, a chain that reaches the bound is accepted back onto it so
often that the acceptance rate stays high, the tuner keeps the scale large,
and the chain is absorbed. RAVEN's own run shows it: in
`metropolis_bounded_linear_2d`, whose posterior in x0 is a normal (0.1, 0.2)
truncated to [0, 1], **RAVEN's chain sits exactly at x0 = 0 in 136 of 300
steps** (from step 6; 40 of the last 100). `BoundHandling::Reject` is the
correct rule and is offered beside the faithful one.

### Typed interface (`raffles::fidelity`)

`SurrogateSpec::PolynomialChaos` and `SurrogateSpec::Kriging { route:
KrigingRoute::RavenSklearn }` give the same bits as calling the routes
directly (`fidelity::surrogate::tests::the_raven_routes_are_typed_end_to_end`);
kriging's `Prediction::std_dev` is `Some`, in the output's units, and its
`ValidationReport` carries 2σ coverage and a closed-form LOO RMSE.

### What this does not show

- The LCG route's draws are not compared with RAVEN's (different
  generators); only A1–A3 check them.
- The optimiser is not L-BFGS-B, so fits are compared at optima only;
  `n_restarts_optimizer` is not ported.
- Nothing here is validation, and no human has reviewed it.
