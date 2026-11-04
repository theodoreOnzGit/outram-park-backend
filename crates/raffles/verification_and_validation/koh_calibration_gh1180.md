# Kennedy–O'Hagan calibration with model discrepancy (gh:#1180)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Issue:** gh:#1180 (rung 3 of epic gh:#1177). **Date:** 2026-11-04.
**Calibration is the stated purpose** (`crates/raffles/AGENTS.md`): the
1-D TUAS exchanger's θ is calibrated against the double-pipe CFD's training
rows. The held-out CFD rows, the CFD's own Nusselt numbers, the literature
annulus limit and ε-NTU are **not** calibration data and stay checks.

Sections 1–4 (search, upstream findings, design, predictions) were written
and committed **before any port code and before any run**, with the
reference generator. Results were added afterwards in their own section,
without editing the predictions.

## 1. What was searched before writing anything

| candidate | licence | KOH with a discrepancy GP? | verdict |
|---|---|---|---|
| RAVEN (`idaholab/raven`) | Apache-2.0 | no (checked in gh:#1193: MCMC samplers only) | no route |
| `surmise` 1.0.0 (BAND, PyPI wheel, `calibrationmethods/`) | MIT | **no**: `directbayes`, `directbayeswoodbury`, `mlbayeswoodbury`, `simulationpost` calibrate θ with an emulator and an observation-error covariance; none has a model-discrepancy term (searched for `bias`, `discrep`, `delta`: no hits) | rejected |
| **SEPIA** (`lanl/SEPIA`, commit `309890423a121919dc09a678c65a21748bbefb4b`, 2023-10-02) | **BSD-3-Clause** (`License-BSD-3`, © 2020 Triad National Security) | **yes**: the Python successor of LANL's GPMSA (Higdon, Kennedy, Cavendish, Cafeo & Ryne 2004, *SIAM J. Sci. Comput.* 26:448; Higdon, Gattiker, Williams & Rightley 2008, *JASA* 103:570): y_obs = η(x, θ) + δ(x) + ε, a GP emulator for η over (x, t), a GP for δ over x | **translated** |
| Dakota's Bayesian calibration with discrepancy | LGPL | yes | not built: a C++ framework with QUESO/GPMSA dependencies, too heavy for this host; SEPIA is the same GPMSA formulation in 1 k lines of Python |
| R `calibrator` (BACCO, Hankin), `SAVE`, `RobustCalibration` | GPL | yes | no R on this host |
| raffles itself | — | TMCMC / TEMCMC / MH / RAVEN MCMC (gh:#1193), sklearn-route GP and SMT KRG/MFK (gh:#1190); **no KOH** | reused: TMCMC samples the translated posterior |

BSD-3 flows into AGPL-3.0; the ported files carry SEPIA's notice
(`crates/raffles/NOTICE`).

## 2. Upstream findings (SEPIA is the specification)

1. **SEPIA's univariate-output model carries no discrepancy.**
   `SepiaData.create_D_basis` returns early for scalar output ("Model has
   univariate output, skipping discrepancy..."), so `pv = 0` and the model
   is `set_params_noD` (η + ε only). Forcing `obs_data.D` crashes
   `SepiaModel.__init__` at `self.num.v = v.reshape((n*pv, 1))` (checked:
   `ValueError: cannot reshape array of size 0 into shape (5,1)`).
2. **Yet the scalar KOH branch survives in the numerics**, inherited from
   GPMSA: `SepiaLogLik.compute_log_lik` ("for scalar output: SigVUgW =
   [SigV+SigUgW] + SigObs/lamOs") and the dense predictor `uvPred` ("if
   num.scalar_out: if not no_D: SigCross = [SigVVx SigUUx; 0 SigWUx]").
3. **`uvPred` is guarded by `raise RuntimeError('uvPred should not normally
   be invoked; uvPredSep is the correct method')`**, and `uvPredSep` (the
   default) cannot run scalar + discrepancy (it adds the n × n `SigObs` to a
   2n × 2n block). For the supported no-discrepancy model the two are the
   same algebra.
4. **The reference generator therefore runs upstream code in three
   modes** (`koh_sepia_gh1180/generate_references.py`): NO_D (SEPIA's
   supported model, both predictors); WITH_D (the model set up by
   `SepiaModel` as NO_D, then the four fields the crash leaves unset filled
   by hand — `num.pv = 1`, `num.lamVzGroup = [0]`, `num.lamVzGnum = 1`, and
   `set_params_full(0, 0, 0, 0)`, the prior corrections `SepiaModel`
   computes for scalar output being all zero — after which upstream's
   `compute_log_lik` runs unchanged; predictions from `uvPred`'s own body
   with only the guard line removed); and SIM_ONLY (the emulator alone).
   **Upstream defect, found when the generator first ran:** in that
   scalar + discrepancy branch of `uvPred` (`SepiaPredict.py` line 704),
   `SigCross[n*pv:,:npred*pv]=SigUUx` assigns an n-row block to an m-row
   slice (`ValueError` on every call), where upstream's own GPMSA comment
   — "SigCross=[SigVVx SigUUx; zeros(m*pu,npred*pv) SigWUx]" — places
   `SigUUx` in the first n rows and the u columns. The generator corrects
   that one line to the comment (`SigCross[:n*pv,npred*pv:]=SigUUx`); it is
   the only other change to upstream's body, and the port follows the
   comment.
   An **independent** dense check (not SEPIA) is generated beside them: the
   multivariate-normal log density of the stacked (u, w) under the joint
   covariance must equal SEPIA's decomposed log-likelihood plus
   (n + m)/2 ln 2π.
5. SEPIA's log prior is **unnormalised** (no Gamma/Beta/Normal constants),
   evaluates the ρ = exp(−β/4) Beta prior with ρ capped at 0.999, and
   bounds are open intervals. Its MCMC steps θ uniformly, ρ uniformly
   ("BetaRho") and the precisions by a variable-width proposal with a
   Hastings correction ("PropMH"), so its target is a density in
   (θ, ρ, λ). The port keeps all of this.

## 3. What is ported, and how it is tested

**Ported** (`crates/raffles/src/bayesian/koh/`): SEPIA's scalar-output
model — the unit-cube transform of x and t and the y standardisation
(`SepiaData.transform_xt`, `standardize_y`, numpy's pairwise summation and
`ddof = 1`), the squared-exponential covariance in the ρ parameterisation
(`SepiaDistCov.compute_cov_mat`), the log-likelihood (`compute_log_lik`,
scalar branch, with and without δ, and simulation only), the log prior
(`SepiaPrior.compute_log_prior` with the defaults of `set_params_sim_only`,
`set_params_noD`, `set_params_full`), and the predictive mean and
covariance of (v, u) at new x (`uvPred`). **Not ported:** multivariate
output (K/D bases, PCA), separable designs, categorical inputs, mean bases,
hierarchical/shared θ, SEPIA's own MCMC and step tuning (TMCMC samples the
same target instead), plotting.

**Differential protocol.** Deterministic numerics are compared with the
committed SEPIA outputs (bit-identical preferred; otherwise below 1e-8
relative with the cause stated). The sampler is compared statistically,
separately, with the criterion fixed here: for each θ component, |mean_Rust
− mean_SEPIA| ≤ 3 √(se_SEPIA² + se_Rust²), se_SEPIA from 40 batch means of
SEPIA's chain, se_Rust = sd_Rust √(5/N) (an assumed effective sample size
of N/5 for the TMCMC population), and the ratio of posterior standard
deviations within [0.8, 1.25].

**Toy problem** (all inputs in the JSON): η(x, t) = x (1 + 2 t₁) +
0.5 sin(2πx + 3 t₂); 40 simulator runs on a seeded Latin hypercube in
(x, t₁, t₂) ∈ [0, 1]³; 10 observations at x = 0.05 … 0.95 of
η(x, 0.3, 0.7) + 0.25 x² + N(0, 0.02²). Five parameter sets (SEPIA's
initial values, three perturbations, one with ρ_U above the 0.999 cap);
predictions at x = 0, 0.27, 0.5, 0.83, 1 for three parameter samples;
SEPIA MCMC 12 000 steps (2 000 burn), seed 1180.

### Predictions for the differential tests (written before the port)

| # | quantity | prediction | why |
|---|---|---|---|
| D1 | x/t transforms, y standardisation, u, w | **bit-identical** | the same arithmetic in the same order once numpy's pairwise sum (8-way unrolled below 128 elements) is reproduced |
| D2 | log-likelihood, NO_D / WITH_D / SIM_ONLY, 5 sets each | < 1e-10 relative, not bit-identical | the Cholesky and the inverse (`np.linalg.inv` via LAPACK `dgesv`) are LAPACK's blocked routines upstream and unblocked loops here: same algebra, different rounding |
| D3 | log prior | bit-identical, including the ρ cap | elementwise `ln`, `exp`, products and sums of ≤ 3 terms, summed in SEPIA's order |
| D4 | dense joint MVN check (independent) | SEPIA's decomposition = dense + (n+m)/2 ln 2π to < 1e-9 relative, and the port within 1e-10 of both | the W / (VU \| W) factorisation is exact algebra |
| D5 | predictive mean and covariance of (v, u) | mean < 1e-8 relative; covariance < 1e-8 of the largest diagonal entry | the dense solve `scipy.linalg.solve(..., sym_pos=True)` (Cholesky) against a Cholesky solve here |
| D6 | NO_D: `uvPred` against `uvPredSep` (upstream against upstream) | agree to < 1e-9 | same conditional Gaussian, conditioned in one step or in two |
| D7 | sampler: TMCMC (population 2 000) against SEPIA's MCMC on WITH_D, all 11 parameters free | the criterion above holds for θ₁, θ₂ | both sample one target |

## 4. The double pipe (rung 3 of gh:#1177)

**Low fidelity.** `TuasOneD` (`outram-park-digital-twin-engine`,
`multifidelity::double_pipe`), θ = (Nusselt multiplier tube side,
Nusselt multiplier annulus side, wall resistance r_f on the tube's outer
area, two friction multipliers). **Calibration output: ln(UA / 1 W/K)**, UA
the overall conductance. Why UA only: θ's three heat-transfer components
act on the steady state only through the two nodal conductances, and UA
is their series combination, so UA is what θ governs; the 1-D UA is
computed from TUAS's own nodal conductances at (x, θ) **without the time
march** (identical to the march's UA, pinned by a test), which makes the
simulator design affordable. Why the logarithm: the uncalibrated
disagreement is relative (UA 1.1–21.4 % low, gh:#1178) and θ acts
multiplicatively on conductances, so an additive δ on ln UA is a relative
discrepancy. **The friction multipliers are not calibrated**: they do not
enter UA at all (they are unidentifiable from it); they stay at 1.

**θ calibrated, and the priors (fixed here, from outside the comparison).**
Each prior is stated on the transformed coordinate that the emulator sees.

| θ | coordinate t | design range of t | prior | justification |
|---|---|---|---|---|
| tube Nu multiplier θ_h | ln θ_h | [−0.6, 0.6] | N(0, 0.2²), truncated to the range (±3σ) | TUAS's Gnielinski laminar correlation is a constant-flux mean; the conjugate counterflow wall sits between constant flux and constant temperature, whose developed Nusselt numbers differ by ln(4.36/3.66) = 0.175 (gh:#1178 P2). σ = 0.2 covers that gap at 1σ. Centred on the derived (TUAS) value 1 |
| annulus Nu multiplier θ_c | ln θ_c | [−0.9, 0.9] | N(0, 0.3²), truncated to the range (±3σ) | TUAS has no annulus correlation and uses the pipe correlation on D_h; the literature developed inner-wall-heated annulus at r* = 0.6 has Nu_ii ≈ 5.6–6.0 against the pipe's 4.36 / 3.66 (Kays & Crawford / Shah & London, gh:#1178 P1), a ratio 1.28–1.64. Centred on the derived value 1 so that a posterior near the literature ratio is the data's doing, with σ = 0.3 wide enough not to exclude it (ln 1.64 = 1.65σ) |
| wall/contact resistance r_f | r_f [m² K/W] | [0, 1e-3] | uniform on the range (SEPIA's N(0.5, 10²) on the unit scale) | the upper end of Incropera & DeWitt Table 11.1's water-service fouling factors. The CFD has **no** fouling or contact resistance, so its true value is 0; it is calibrated (as the issue asks) to see whether δ and θ_h let it absorb something it should not |

**The KOH model.** SEPIA's scalar formulation above, with δ. Inputs x
as (ln Re_hot, ln Re_cold, ln k_wall, L), SEPIA's unit-cube transform over
the simulator design (the closures were interpolated in the same log
coordinates in gh:#1178). **Simulator design:** the 44 design x (36
training and 8 held-out: running the cheap 1-D model at a held-out x uses
no held-out CFD output) × 6 t-points each, from a seeded Latin hypercube in
the three t coordinates (a different one per x): m = 264 runs. **Field
data:** the 36 CFD training rows' ln UA_local (n = 36). **Discrepancy
hyperparameters:** SEPIA's defaults (ρ_V ~ Beta(1, 0.1), λ_Vz ~ Gamma(1,
1e-3)). **Observation precision λ_Os:** Gamma(shape 2, mean 1/σ_std²),
σ_std = 0.0105 / sd(standardised scale), i.e. a CFD error of about 1 % in
UA — gh:#1178's mesh check moved UA by −1.05 % at the most demanding point.
**Emulator hyperparameters** (β_U, λ_Uz, λ_Ws, λ_WOs) are fixed at the
highest-posterior particle of a TMCMC run on SEPIA's simulation-only
posterior (SEPIA's default priors), then the KOH posterior over
(θ, ρ_V, λ_Vz, λ_Os) is sampled by raffles' TMCMC (population 2 000).
**This is a deviation from SEPIA**, which samples every parameter jointly:
it is Kennedy & O'Hagan's own plug-in of the emulator's hyperparameters
(KOH 2001, §4.5; the "modular" approach of Liu, Bayarri & Berger 2009),
taken here for cost (each joint evaluation would factor a 264 × 264
matrix). **Sampler settings, added after D7's first run showed 3 moves per
stage under-mixing and before any double-pipe run:** emulator stage TMCMC
population 600 × 10 moves per stage (seed 1180), calibration stage 2 000 ×
10 (seed 1181).

**What is reported:** the posterior on θ (and its correlations); δ(x) over
the design; the predictive of ln UA at the 8 held-out x (a mixture over 400
posterior samples of the conditional Gaussians, plus 1/λ_Os for the CFD's
own error), its median as the point prediction, its 95 % interval and
coverage, the held-out relative RMSE of UA (RMSE / mean |UA|, the metric of
gh:#1190's MFK 3.0 %); the ablation; a no-δ calibration (SEPIA's supported
NO_D model, same priors) as the identifiability contrast; and three
**independent checks** that the calibration never sees: the CFD's own
Nusselt ratios on each side (CFD / TUAS, from the committed dataset), the
literature developed-annulus ratio 1.28–1.64, and the calibrated 1-D duty Q
(a full TUAS march at the posterior-mean θ, no δ) against the held-out CFD
Q. ε-NTU is not used.

**Ablation:** with the posterior samples, set one θ component to its
derived value (θ_h = 1, θ_c = 1, r_f = 0) in every sample, the others as
sampled; report (a) the 1-D model alone at the posterior-mean θ so
modified (held-out UA RMSE), and (b) the KOH predictive with δ (RMSE and
coverage). The **uncalibrated** values are θ = (1, 1, 0).

### Predictions for the double pipe (written before any run)

| # | quantity | prediction | why |
|---|---|---|---|
| K1 | posterior ln θ_c | mean in [0.15, 0.45] (θ_c 1.16–1.57); sd in [0.05, 0.25] | the annulus correlation is the dominant uncalibrated error (26–58 %), and UA is 1–21 % low everywhere |
| K2 | posterior ln θ_h | mean in [−0.15, 0.15]; sd above 0.10 (half the prior's) | the tube side is within −12…+8 %; UA cannot separate the two sides well |
| K3 | posterior r_f | mean below 2.5e-4 m² K/W, mass piled at 0 | it can only lower a UA that is already too low |
| K4 | corr(ln θ_h, ln θ_c) | below −0.3 | series resistances: raising one side's h does what raising the other's does |
| K5 | posterior mean δ at the 36 training x | \|δ\| < 0.05 (5 % in UA) at ≥ 30 of 36; largest at L = 0.3 m, Re_hot = 10 | after θ_c takes the mean offset, what is left is the series-UA vs UA_local mismatch of gh:#1178 P9 |
| K6 | held-out UA relative RMSE (KOH predictive median) | 1.5 % to 6 %; within a factor 2 of MFK's 3.0 % either way | 36 rows and a δ GP over 4 inputs, against MFK's 375 low-fidelity rows |
| K7 | held-out coverage of the 95 % interval | 7 or 8 of 8 | an honest posterior with a plug-in emulator |
| K8 | ablation, 1-D alone | resetting θ_c raises the held-out UA RMSE by ≥ 5 points; θ_h by ≤ 3; r_f by < 0.5 | K1–K3 |
| K9 | ablation, with δ | each reset moves the RMSE by less than half its 1-D-alone change | δ absorbs a θ reset: the confounding |
| K10 | no-δ calibration | ln θ_c higher than with δ and its sd below half the KOH sd | without δ, θ must explain all the misfit and is over-confident |
| K11 | independent: CFD Nu ratios | the KOH posterior median θ_c lies inside the CFD's own annulus ratio range [1.26, 1.58]; θ_h inside the tube's [0.88, 1.08] | if θ is physically identified, it recovers what the CFD's Nusselt numbers say |
| K12 | independent: calibrated 1-D Q at the held-out points (no δ) | \|ΔQ/Q\| < 2.5 % at all 8 (uncalibrated −2.4 … −5.3 %) | Q follows UA, damped (gh:#1178 P4) |

## 5. Results: differential tests (2026-11-04)

Run with `cargo test --release -p raffles --test koh_sepia_gh1180 -j 2`
(4 tests, 53 s; 4 CPUs shared). References from
`koh_sepia_gh1180/generate_references.py` (SEPIA `309890423a12`, Python
3.11.17, numpy 1.26.4, scipy 1.10.1; SEPIA's MCMC run took 46 s).

| # | quantity | prediction | measured | |
|---|---|---|---|---|
| D1 | transforms, u, w | bit-identical | **bit-identical** in every number | met (after reproducing numpy's zero-started pairwise sum, checked on 1 800 random arrays) |
| D2 | log-likelihood / log posterior, 15 sets | < 1e-10 relative | worst **6.8e-10** (no_d set 4), 1.4e-10 (no_d set 1); every with_d and sim_only set ≤ 2.6e-11 | **missed at 2 of 15 sets.** Cause: SEPIA's own round-off. At set 4 SEPIA is 5.9e-10 from the independent dense density and the port 9.7e-11; the gap is upstream's LU inverse of an ill-conditioned `SigW` (λ_WOs, λ_Ws ≥ 1000). Inside the workspace 1e-8 code-to-code allowance, which the test asserts. **Flagged for the maintainer** |
| D3 | log prior | bit-identical | bit-identical at **9 of 15**, ≤ 1e-14 relative at the rest | **missed.** numpy 1.26.4's vectorised `exp`/`log` differ from glibc's by one ulp on 20–24 % of arguments (measured on 200 000 of each); Rust uses glibc's. Bit identity would need numpy's SIMD kernels ported |
| D4 | dense check | SEPIA = dense to < 1e-9; port within 1e-10 of both | SEPIA–dense worst 5.9e-10; **port–dense worst 9.7e-11** | met (the port is closer to the exact density than SEPIA) |
| D5 | predictive mean / covariance | < 1e-8 | mean worst 5.0e-13 (no δ) / 5.5e-13 (with δ); covariance worst 1.0e-12 / 2.6e-13 of the largest diagonal | met |
| D6 | upstream `uvPred` vs `uvPredSep` | < 1e-9 | 4.9e-13 | met; `uvPredSep` with δ fails upstream (`ValueError: operands could not be broadcast together with shapes (20,20) (10,10)`, asserted) |
| D7 | TMCMC vs SEPIA MCMC, 11 parameters free | criterion of §3 | **first run (3 moves/stage) MISSED**: θ₁ 0.3615 vs 0.3823, \|Δ\| 0.0208 > bound 0.0058. A diagnostic showed the population's mean log posterior still rising with the move count (101.1 / 104.4 at 3 moves, 107.8 / 106.9 at 10, 108.9 at 20 with population 4 000). **Protocol fixed, criterion kept:** at 20 moves θ₁ 0.38416 ± 0.03093 vs SEPIA 0.38232 ± 0.03082 (\|Δ\| 0.0019, bound 0.0054, sd ratio 1.004); θ₂ 0.69681 ± 0.01664 vs 0.69755 ± 0.01705 (\|Δ\| 0.0007, bound 0.0030, ratio 0.976) | met after the protocol fix |

The toy's truth is θ = (0.3, 0.7); both samplers put θ₁ near 0.38 ± 0.03
because δ = 0.25 x² is partly absorbed by θ₁ (which scales a term linear in
x): the KOH confounding, on a problem small enough to see it.

**The typed route** (`raffles::fidelity::koh`, unit test
`the_typed_route_calibrates_an_analytic_pair`): LF 1 + k a against HF
1 + 1.4 a + 0.05 a², 9 training rows: posterior median k = 1.438
(prediction [1.2, 1.6], met); both held-out values inside the 95 %
interval (HF 1.42450 in [1.40595, 1.45658]; 2.15200 in [2.11358, 2.19203]).

## 6. Results: the double pipe (2026-11-04)

Full record, tables and interpretation:
`crates/outram-park-digital-twin-engine/verification_and_validation/double_pipe_koh_gh1180.md`
(run output committed beside it). In one paragraph: the KOH predictive
gives the 8 held-out CFD UAs with **1.28 % relative RMSE and 8/8 inside
the 95 % interval** (MFK, gh:#1190: 3.0 %). The θ posterior is
ln θ_h +0.164 ± 0.116, ln θ_c +0.336 ± 0.171, r_f 5.4e-4 ± 2.5e-4 m² K/W
(uncalibrated 0, 0, 0), corr(h, c) −0.70. **θ is only partly identified**:
θ_c's median (1.39) falls inside the CFD's own annulus Nusselt ratio range,
but θ_h (1.18) lies above the tube range and r_f (truly 0 in the CFD) stays
prior-like; δ re-absorbs any θ reset (ablation with δ moves the RMSE by at
most 0.42 points, without δ by up to 11), and the posterior-mean θ used
without δ over-predicts the marched duty by up to 6.7 %. Predictions met:
K1, K4, K7, K9; missed: K2, K3, K5, K6 (better than predicted), K8, K10
(by 0.6 % of its bound), K11 (θ_h half), K12.
