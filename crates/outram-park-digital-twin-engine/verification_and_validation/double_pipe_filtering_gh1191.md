# Double pipe: tracking a drifting wall resistance with the FilterPy ports (gh:#1191)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Issue:** gh:#1191 (epic gh:#1177). **Date:** 2026-11-04. **Synthetic
truth only:** no plant, no live data, no operational use. This is a
teaching and V&V twin experiment.

Sections 1–3 were written and committed **before the pilot code existed
and before any run**. Results follow in their own section, with the
predictions left unedited.

## 1. The question

gh:#1180's Kennedy–O'Hagan calibration found the wall resistance r_f
**poorly identified from UA**. r_f trades off against the tube-side Nusselt
multiplier θ_h and against δ, and its posterior stayed prior-like although
r_f is 0 in the CFD. Can a filter on **noisy outlet temperatures** track a
fouling-like r_f(t) that drifts in time, and when does it do better than
KOH?

## 2. Construction

**The truth (CFD-derived closures, stated):** at a training design point x,
take the CFD's own mean Nusselt numbers, `Nu_hot^CFD(x)` and
`Nu_cold^CFD(x)`, read off the committed gh:#1178 dataset
(`Closures::from_cfd`; read off, not fitted). Fold the fouling into the
tube side exactly as TUAS's series path does, with
`one_d::fold_fouling(Nu_hot · θ_h, r_f)`, and build TUAS's component with
those Nusselt numbers. UA then follows from TUAS's two nodal conductances,
march-free as in `one_d::ua`. The outlet temperatures come from the
counter-current ε-NTU relation (dwsim-libs, `one_d::eps_ntu`) at that UA
and x's capacity rates, with the inlets at 350 / 300 K.

The fouling follows the Kern–Seaton asymptotic form, the textbook fouling
model:

    r_f(k) = r_∞ (1 − exp(−k/τ)),  r_∞ = 4 × 10⁻⁴ m² K/W, τ = 60 steps, k = 1 … 200,

with r_∞ inside Incropera's Table 11.1 range (the same source as #1194's
θ bounds). Measurement noise: independent N(0, 0.05² K²) on each outlet,
drawn from the workspace LCG with a fixed seed. **The CFD itself has
r_f = 0**: the drift is imposed on the CFD-derived closures, not measured.
Steady state is assumed at each step (quasi-static fouling).

**What is not from the CFD:** ε-NTU in place of TUAS's time march for the
outlet temperatures. The march's ε is lower by the 1-D layer error only
(`nominal_theta_is_tuas_own_correlation_path` bounds it within 5 %). It is
used for truth and filter alike, so it is not a source of model error
here.

**The filter's model** (`fidelity::StateSpaceModel`, implemented in
`double_pipe::filtering`): the state is a random walk,
`propagate(x) = x`, and `observe` is the truth's own map from (closures,
θ, r_f) to the outlets. Process noise is fixed **a priori** from the
expected drift rate, not tuned:

- r_f: `q = (r_∞/τ)²` (σ = 6.7e-6 m² K/W per step, the steepest slope of
  the drift);
- ln θ: `q = 1e-6` (σ 1e-3 per step: slow adaptation).

Priors: r_f ~ N(0, (5 × 10⁻⁴)²); ln θ_h ~ N(0, 0.2²) and
ln θ_c ~ N(0, 0.3²), which are gh:#1180's priors.

**The experiments** (each run with UKF (Merwe, α 0.5, β 2, κ 3 − n), EKF
(central-difference Jacobians) and EnKF (N 50, own draws, fixed seed)):

| | Filter's closures | State | Operating point | Kind |
|---|---|---|---|---|
| **P-A** | the CFD's (= truth's) | r_f | fixed x_A = (Re_hot 30, Re_cold 15, k_wall 100, L 0.1), a training point | identical twin |
| **P-B** | TUAS's textbook (uncalibrated, θ nominal) | r_f | x_A | fraternal twin: model-form error |
| **P-D** | the CFD's, scaled by unknown θ_h | ln θ_h, r_f | x_A | identifiability at fixed x (the KOH situation) |
| **P-C** | the CFD's, scaled by unknown θ_h, θ_c | ln θ_h, ln θ_c, r_f | cycling through the 9 training (Re_hot, Re_cold) pairs at k_wall 100, L 0.1, one per step | identifiability with a moving operating point |

**Metrics.** Tracking error of r_f (RMSE over steps 31–200, after
burn-in, and over all steps); the mean error (bias); NEES of the full
state against the truth, averaged over steps (P-B: r_f only); 95 %
coverage of the true r_f; and the NIS of the 2-vector innovation, averaged
over steps. The bands assume independent steps (errors are
autocorrelated, so they are indicative only). For 200 steps:

- NEES, 1 degree of freedom: [0.82, 1.20];
- NIS, 2 degrees of freedom: [1.73, 2.29].

## 3. Predictions (before any run)

| # | Prediction |
|---|---|
| F1 | P-A, UKF: r_f RMSE (31–200) in [5e-6, 4e-5] m² K/W (signal ≈ 1 K at r_∞ against 0.05 K noise) |
| F2 | P-A, UKF: NEES mean in [0.3, 1.5], **below** the band's centre (q from the steepest slope is conservative after the knee); 95 % coverage in [0.90, 1.00] |
| F3 | P-A: NIS mean inside [1.73, 2.29] for all three filters |
| F4 | P-A: EKF RMSE within 10 % of the UKF's; EnKF (N 50) within 30 % |
| F5 | P-B: r_f biased **negative** (textbook UA is low, gh:#1180's θ_c = 1.40), mean error (31–200) in [−6e-4, −1e-4]; coverage < 0.2; NEES > 10 |
| F6 | P-B: NIS mean nevertheless in [1.5, 3]: both outlets carry only UA, so the innovations cannot show the model-form error |
| F7 | P-D: at the last step, \|corr(ln θ_h, r_f)\| > 0.9; r_f RMSE (31–200) > 1e-4 (the r_f–θ_h ridge, as in KOH); NEES in [0.5, 2] (consistent, just uninformative) |
| F8 | P-C: r_f RMSE (31–200) < 5e-5; \|corr(ln θ_h, r_f)\| < 0.9 at the last step; NEES in [0.5, 2]; final ln θ_c within 0.05 of 0 |
| F9 | The KOH cross-check: at a **fixed** operating point, filtering on outlet temperatures does **not** identify r_f better than KOH did (P-D, F7). It does better only when the closures are known (P-A) or the operating point moves (P-C) |
| F10 | All runs together take < 5 min in release |

## 4. Results (2026-11-04, added after the run; sections 1–3 unedited)

Code: `src/multifidelity/double_pipe/filtering.rs`. Test:
`multifidelity::double_pipe::filtering::tests::pilot_four_experiments_three_filters`
(`GH1191_WRITE=1` writes the files below). **Picture first:**
`double_pipe_filtering_gh1191/r_f_tracking.svg` has one panel per
experiment: the true r_f, the UKF's estimate and its 95 % band. Every step
of every run is in `double_pipe_filtering_gh1191/traces.csv`.

**Signal at x_A:** the CFD's Nusselt numbers are 5.598 (tube) and 6.674
(annulus); TUAS's textbook ones are 5.981 and 5.011. The clean outlets are
327.440 / 314.100 K. At r_∞ the hot outlet rises by 0.969 K and the cold
outlet falls by 0.605 K, against 0.05 K noise.

| Exp. | Filter | r_f RMSE 31–200 [m² K/W] | bias 31–200 | NEES mean | NIS mean | 95 % coverage | final σ(r_f) | final corr(ln θ_h, r_f) |
|---|---|---|---|---|---|---|---|---|
| P-A | UKF | 8.65e-6 | −3.29e-6 | 1.140 | 1.807 | 0.935 | 1.01e-5 | — |
| P-A | EKF | 8.66e-6 | −3.31e-6 | 1.171 | 1.829 | 0.930 | 1.01e-5 | — |
| P-A | EnKF | 8.73e-6 | −3.18e-6 | 1.329 | 1.852 | 0.895 | 1.10e-5 | — |
| P-B | UKF | 4.38e-4 | −4.37e-4 | 1938 (r_f only) | 1.813 | 0.000 | 1.01e-5 | — |
| P-B | EKF | 4.38e-4 | −4.38e-4 | 1948 | 1.855 | 0.000 | 1.01e-5 | — |
| P-B | EnKF | 4.37e-4 | −4.37e-4 | 2048 | 1.860 | 0.000 | 1.10e-5 | — |
| P-D | UKF | 9.05e-4 | −9.05e-4 | 99.5 | 1.613 | 0.120 | 1.70e-4 | 0.998 |
| P-D | EKF | 7.59e-5 | +7.47e-5 | 1.056 | 1.775 | 1.000 | 2.44e-4 | 0.999 |
| P-D | EnKF | 3.73e-4 | −3.68e-4 | 10.39 | 1.777 | 0.250 | 9.70e-5 | 0.993 |
| P-C | UKF | 1.42e-5 | −7.42e-6 | 1.445 | 1.850 | 1.000 | 2.93e-5 | 0.656 |
| P-C | EKF | 1.35e-5 | +1.82e-6 | 1.588 | 1.870 | 0.985 | 2.93e-5 | 0.656 |
| P-C | EnKF | 3.17e-5 | +2.58e-5 | 3.827 | 1.885 | 0.965 | 2.50e-5 | 0.681 |

The final P-C ln θ_c is 0.0056 (UKF), 0.0067 (EKF) and 0.0084 (EnKF); its
truth is 0. Wall time for all 12 runs: **2.1 s** (release).

| # | Verdict |
|---|---|
| F1 | **met**: 8.65e-6, inside [5e-6, 4e-5] |
| F2 | **half missed**: NEES 1.140 and coverage 0.935 are inside their predicted ranges, but NEES is **above** the band's centre, not below. The steepest-slope q was not conservative. The deterministic drift is not a random walk: the filter lags the knee, and its −3.3e-6 bias adds to the NEES |
| F3 | **met**: NIS 1.807, 1.829, 1.852 |
| F4 | **met**: EKF +0.06 %, EnKF +0.9 % on RMSE |
| F5 | **met**: bias −4.37e-4, coverage 0.000, NEES ≈ 2000 |
| F6 | **met**: NIS 1.81–1.86 with an r_f off by more than its whole range. **The innovation check cannot see model-form error** when the observations carry one degree of freedom (UA), which both outlets do here |
| F7 | **mostly missed.** \|corr\| > 0.9 holds (0.993–0.999). The RMSE > 1e-4 holds for the UKF (9.1e-4) and the EnKF (3.7e-4) but **not the EKF (7.6e-5)**. NEES in [0.5, 2] holds **only for the EKF** (1.06); the UKF's is **99** and the EnKF's **10**. Looking at the figure, the UKF jumps around step 15 to ln θ_h ≈ −0.23, r_f ≈ −5e-4, and stays there with a band that excludes the truth. The sigma-point and ensemble filters lost consistency on the curved, non-identified ridge. The EKF stayed consistent and near the truth only because its θ_h stayed at its prior mean, which is the truth here. Its final σ(r_f), 2.4e-4, says r_f is **not** identified |
| F8 | **met for the UKF and the EKF** (RMSE 1.4e-5, \|corr\| 0.656, NEES 1.45 / 1.59, ln θ_c ≤ 0.007). The **EnKF misses on NEES** (3.83; the rest is inside). N = 50 under-disperses over 3 dimensions, as D8 in raffles also shows (P ≈ 0.93 of the KF's) |
| F9 | **met in substance.** At a fixed point, the final σ(r_f) with θ_h unknown is 1.0–2.4e-4, the size of KOH's posterior σ (2.5e-4, gh:#1180): no better. With the closures known it is 1.0e-5 (P-A), and with the operating point moving it is 2.9e-5 (P-C), 25× and 8× tighter |
| F10 | **met**: 2.1 s |

### What this does and does not show

- **The KOH cross-check, stated fairly.** P-C identifies r_f because it is
  an identical twin: the filter's model is the truth's model, so nothing
  plays the role of KOH's δ(x). KOH's r_f problem was the θ_h–r_f trade
  **plus** δ absorbing what θ could not. Filtering on outlet temperatures
  is not a cure for model-form error: P-B shows a filter confidently wrong
  by 4.4e-4 m² K/W (more than r_∞) while its NIS looks perfect. With the
  textbook closures, the r_f a filter reports is the closure error, not
  the fouling.
- **Consistency on a non-identified problem is filter-dependent** (P-D):
  the UKF and EnKF became overconfident along the ridge. Before quoting an
  estimate, check identifiability: the posterior correlation, and σ against
  the prior.
- Single runs: one noise seed per schedule, one EnKF seed. The NEES and NIS
  bands assume independent steps, which they are not. Seed-to-seed spread
  was not measured.
- The truth uses ε-NTU on TUAS conductances, not the TUAS time march or the
  CFD itself (section 2). It is a twin experiment on CFD-derived closures.

### Tests run

`cargo test --release -p outram-park-digital-twin-engine --lib --tests -j 2`
(2026-11-04): **137 passed, 0 failed, 0 ignored**, including the two pilot
tests and the long MFK and KOH tests (lib 1273 s).
