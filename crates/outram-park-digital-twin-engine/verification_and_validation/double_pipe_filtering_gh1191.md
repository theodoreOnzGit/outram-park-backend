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
