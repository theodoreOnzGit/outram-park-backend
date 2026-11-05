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

## 3. Results (2026-11-04)

Command: `cargo test --release -p outram-park-digital-twin-engine --lib
double_pipe::openda -- --nocapture` (code `double_pipe/openda.rs`).

### (a) OpenDA's EnKF beside FilterPy's EnKF (N 50 each; 8 runs, 2.4 s)

FilterPy's EnKF reproduces gh:#1191's recorded numbers exactly (same code,
same seed). r_f metrics after burn-in (steps 31–200):

| exp. | filter | RMSE | bias | NEES | NIS | 95 % cover | final σ(r_f) |
|---|---|---|---|---|---|---|---|
| P-A | FilterPy | 8.73e-6 | −3.18e-6 | 1.33 | 1.852 | 0.895 | 1.10e-5 |
| P-A | **OpenDA** | **8.49e-6** | −3.27e-6 | **1.25** | **1.833** | 0.915 | 8.42e-6 |
| P-B | FilterPy | 4.374e-4 | −4.373e-4 | 2048 | 1.860 | 0.000 | 1.10e-5 |
| P-B | **OpenDA** | 4.375e-4 | **−4.374e-4** | 1996 | 1.836 | 0.000 | 8.42e-6 |
| P-D | FilterPy | 3.73e-4 | −3.68e-4 | 10.4 | 1.777 | 0.250 | 9.70e-5 |
| P-D | **OpenDA** | 4.89e-4 | −4.86e-4 | **23.5** | 1.735 | 0.085 | 9.44e-5 |
| P-C | FilterPy | 3.17e-5 | +2.58e-5 | 3.83 | 1.885 | 0.965 | 2.50e-5 |
| P-C | **OpenDA** | **2.31e-5** | −1.86e-5 | 1.83 | **1.866** | 0.985 | 2.55e-5 |

| id | prediction | measured | |
|---|---|---|---|
| OA1 | P-A RMSE in [5.8e-6, 1.31e-5] | 8.49e-6 | **met** |
| OA2 | NIS within ±0.3 of FilterPy (P-A, P-C) | 1.833 vs 1.852; 1.866 vs 1.885 | **met** |
| OA3 | P-B bias within 10 % | −4.374e-4 vs −4.373e-4 (0.02 %) | **met** |
| OA4 | P-D NEES > 3 | 23.5 | **met** |
| OA5 | P-C RMSE within ×2 | 2.31e-5 vs 3.17e-5 | **met** |
| OA6 | P-A NEES in [0.5, 2] | 1.25 | **met** |

**Reading.** Two independent ports of the stochastic EnKF agree on the
twin within their single-seed noise. The cross-check also repeats gh:#1191's
two warnings with a second code: with the textbook closures (P-B) both are
confidently wrong by more than the whole fouling range while NIS looks
perfect, and on the non-identified θ_h–r_f ridge (P-D) both lose
consistency (OpenDA's worse, NEES 23.5 against 10.4, one seed each). The
P-C difference (OpenDA's NEES 1.83, FilterPy's 3.83) is in the reported
covariance: OpenDA's estimate is the ensemble's own covariance, FilterPy's
is `P − K S Kᵀ`. Single runs, one seed per filter: the seed-to-seed spread
is not measured.

### (b) DUD against the Kennedy–O'Hagan posterior (0.1 s)

| | ln θ_h | ln θ_c | r_f [m² K/W] | training ln UA RMSE | held-out UA RMSE |
|---|---|---|---|---|---|
| uncalibrated (0, 0, 0) | 0 | 0 | 0 | 0.0948 | 8.95 % (gh:#1180) |
| **DUD, no background** (10 iterations, 24 evaluations) | **−0.061** ± 0.0095 | **+2.161** ± 1.08 | **1.67e-3** ± 1.1e-4 | **0.0279** | **1.49 %** |
| **DUD, background** (10 iterations, 31 evaluations) | −0.028 ± 0.015 | +1.128 ± 0.089 | 1.25e-3 ± 9.0e-5 | 0.0303 | 1.78 % |
| KOH with δ, posterior mean | +0.164 ± 0.116 | +0.336 ± 0.171 | 5.43e-4 ± 2.5e-4 | 0.0587 | 6.30 % (1-D alone) |
| KOH no δ, posterior mean | −0.025 ± 0.036 | +0.777 ± 0.086 | 8.51e-4 ± 1.3e-4 | 0.0329 | not recorded |

(± are DUD's linearised sds and KOH's posterior sds; held-out = the 1-D
model at that θ, without δ, gh:#1180's metric.)

| id | prediction | measured | |
|---|---|---|---|
| OB1 | stops within 10 outer iterations, no model error | no model error, but it stopped **on the 10-iteration cap**, not on a convergence test | **missed** (in substance) |
| OB2 | ln θ_c within 0.15 of 0.777 and > 0.507; r_f within 3e-4 of 8.5e-4 | ln θ_c **2.161** (θ_c 8.7); r_f **1.67e-3** | **missed** (> 0.507 holds; the rest does not) |
| OB3 | ln θ_h within 0.1 of −0.025 | −0.061 | **met** |
| OB4 | background pulls ln θ_c strictly between 0 and the no-background value | 1.128 ∈ (0, 2.161) | **met** |
| OB5 | held-out RMSE in (1.28 %, 6.30 %) | 1.49 % (1.78 % with background) | **met** |
| OB6 | sd(ln θ_c) in [0.015, 0.06] | 1.08 (0.089 with background) | **missed** |

**Reading.** Deterministic calibration without δ does not land on KOH's
no-δ posterior: it runs along the θ_c–r_f ridge to physically implausible
values (θ_c 8.7 against the CFD's own annulus Nu ratio 1.26–1.58; r_f 1.7×
the top of gh:#1180's prior range), and DUD's linearised sd of ln θ_c (1.08)
says plainly that θ_c is not identified by ln UA alone. The KOH no-δ
posterior is held back by its priors (ln θ_c N(0, 0.3²), r_f ≤ 1e-3) and its
inflated ε; the background term pulls DUD the same way (ln θ_c 1.13) but
not as far. Both DUD runs fit the training rows better than KOH's θ does
(0.028 / 0.030 against 0.033 and 0.059) and predict the held-out UA well
(1.49 %, close to KOH-with-δ's 1.28 %), **with a θ that is wrong**: a fit of
ln UA is not evidence for θ. This is gh:#1180's identifiability finding,
reached by a second, deterministic route. KOH's posterior **mode** was not
recorded by gh:#1180 and was not recomputed (it needs the 20-minute KOH run).

Not done: DUD with bounds (OpenDA's constraint QP is not ported); DUD from
other starting points; the seed-to-seed spread of (a).
