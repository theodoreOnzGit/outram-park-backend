# gh:#1192 pilot: OpenDA's EnKF and DUD on the synthetic double pipe

**Status: AI-assisted draft, not reviewed by a human. Synthetic truth only**
(`RESPONSIBLE_USE.md`): a twin experiment for teaching and V&V, never an
estimate of a real exchanger.

Sections 1 and 2 (method and predictions) were committed before any pilot
code existed; section 3 holds the results, misses included.

## 1. Method

### (a) OpenDA's EnKF against FilterPy's EnKF (gh:#1191's twin experiment)

The problem is gh:#1191's, unchanged (`double_pipe/filtering.rs`, record
`double_pipe_filtering_gh1191.md`): the four experiments P-A (closures
known), P-B (textbook closures, model-form error), P-D (θ_h unknown, fixed
x), P-C (θ_h, θ_c unknown, x cycling); the same synthetic measurements
(seeds 11910, 11911), priors, process noise, σ_T = 0.05 K, 200 steps.

The filter is the port of OpenDA's `EnKF.analysis` (`raffles::openda`),
run on the same `StateSpaceModel` through `raffles::fidelity::Assimilator`
with N = 50 (FilterPy's EnKF also used 50):

- initial ensemble: x₀ + a realisation of `StochVector(0, P₀)` per member
  (OpenDA's `stochInit`);
- forecast: each member through the model's `propagate`, plus a realisation
  of `StochVector(0, Q)` (OpenDA's "system noise", what its models add
  inside `compute`);
- analysis: OpenDA's gain `K = X'·PREDᵀ·inv(PRED·PREDᵀ/(N−1) + R)/(N−1)`
  through JAMA's SVD inverse, perturbed observations, no localisation;
- reported estimate: the ensemble mean and the ensemble covariance with
  1/(N − 1) (OpenDA reports the mean and the standard deviation; FilterPy
  reports `P − K S Kᵀ`, a different quantity, see gh:#1191 §D7);
- draws: the workspace LCG (`petir::rng::lcg`, Box–Muller), seed 1192.

The two EnKFs share the algorithm (stochastic EnKF, perturbed observations)
but differ in the reported covariance, the draws and the inverse, so the
comparison is **statistical**, single run each, one seed: a cross-check that
the two ports behave alike, not a bit-for-bit test (that is done in raffles
against OpenDA itself).

### (b) DUD against the Kennedy–O'Hagan posterior (gh:#1180)

- **Calibrated:** p = (ln θ_h, ln θ_c, r_f), the same three as gh:#1180,
  against the **36 CFD training rows' ln UA**; the 8 held-out rows stay out.
- **Model:** TUAS's UA without the march (`one_d::ua`, gh:#1180's
  `TuasOneDUa`), **no discrepancy term**: DUD is the deterministic,
  model-form-blind counterpart.
- **Cost:** OpenDA's weighted cost, `J = ½ Σ ((ln UA_CFD − ln UA_1D)/σ)²`,
  with σ = 0.0105 on ln UA (gh:#1180's CFD error estimate, fixed before the
  run); optionally plus the background term `½ Σ ((p − p̄)/σ_p)²` with
  gh:#1180's priors as Gaussians: ln θ_h N(0, 0.2²), ln θ_c N(0, 0.3²), r_f
  N(5e-4, (2.89e-4)²) (the U[0, 1e-3] prior's mean and sd).
- **DUD settings:** start at the uncalibrated (0, 0, 0); initial search
  steps the prior sds (0.2, 0.3, 2.89e-4) (OpenDA takes them from the
  parameter uncertainty); `maxit` 10, every other setting OpenDA's default
  (absTol 0.01, relTol 0.01, maxInnerIter 6, maxStep 10).
- **Compared against gh:#1180's recorded posterior:** with δ, mean
  (+0.164, +0.336, 5.43e-4), median (+0.167, +0.331, 5.58e-4), sd (0.116,
  0.171, 2.50e-4); no δ, mean (−0.025, +0.777, 8.51e-4), median (−0.024,
  +0.795, 8.83e-4), sd (0.036, 0.086, 1.27e-4). **The posterior mode was not
  recorded by gh:#1180** and is not recomputed here (it needs the 20-minute
  KOH run again); the comparison is against the mean and the median.
- **Reported:** DUD's θ and its linearised standard deviations (OpenDA's
  error estimate), the evaluation count, the training ln UA RMSE, and the
  held-out UA relative RMSE of the 1-D model at DUD's θ (same metric as
  gh:#1180).

## 2. Predictions (written before the pilot code)

| id | prediction |
|---|---|
| OA1 | P-A: OpenDA EnKF r_f RMSE (steps 31–200) within ×1.5 of FilterPy EnKF's 8.73e-6, i.e. in [5.8e-6, 1.31e-5] |
| OA2 | P-A and P-C: mean NIS within ±0.3 of FilterPy EnKF's (1.852, 1.885) |
| OA3 | P-B: bias after burn-in within 10 % of FilterPy EnKF's −4.37e-4 (the bias is set by the wrong closures, not the filter) |
| OA4 | P-D: OpenDA's EnKF also loses consistency on the non-identified ridge, mean NEES > 3 (FilterPy EnKF 10.4) |
| OA5 | P-C: r_f RMSE within ×2 of FilterPy EnKF's 3.17e-5 |
| OA6 | P-A: mean NEES in [0.5, 2] (ensemble covariance, one dimension, N 50) |
| OB1 | DUD (no background) stops within 10 outer iterations without a model error |
| OB2 | DUD (no background) lands near KOH's **no-δ** posterior, not the with-δ one: ln θ_c within 0.15 of 0.777 and outside one with-δ sd of 0.336 (i.e. > 0.507); r_f within 3e-4 of 8.5e-4 (it may pass the 1e-3 range end: DUD is unbounded) |
| OB3 | DUD (no background): ln θ_h within 0.1 of −0.025 |
| OB4 | with the background term, ln θ_c moves toward the prior mean: strictly between 0 and the no-background value |
| OB5 | held-out UA relative RMSE of the 1-D at DUD's θ (no δ) lies between KOH with δ (1.28 %) and the 1-D at KOH's with-δ posterior mean (6.30 %) |
| OB6 | DUD's linearised sd of ln θ_c (σ 0.0105) is below KOH no-δ's 0.086, by about the noise ratio 0.0105/0.030: in [0.015, 0.06] |

## 3. Results

*To be added after the pilot runs.*
