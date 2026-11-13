# HF → LF rung 3: Kennedy–O'Hagan calibration of the TUAS 1-D double pipe (gh:#1180)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Issue:** gh:#1180 (rung 3 of epic gh:#1177). **Date:** 2026-11-04.
**This rung calibrates, by intention** (`crates/raffles/AGENTS.md`).

- **What was calibrated:** the TUAS 1-D exchanger's tube-side Nusselt
  multiplier θ_h, annulus-side multiplier θ_c and wall resistance r_f.
- **Against what:** ln UA of the 36 CFD **training** rows of gh:#1178's
  committed dataset.
- **Uncalibrated value:** θ = (1, 1, 0) (`Theta::nominal()`).
- **Kept out of the calibration:** the 8 held-out CFD rows, the CFD's own
  Nusselt numbers, the literature annulus ratio and the marched duty Q.
  ε-NTU was not used.

**Methodology, settings, priors and the predictions K1–K12** were fixed
before the run in
`crates/raffles/verification_and_validation/koh_calibration_gh1180.md` §4
(the SEPIA translation and its differential tests are there too). Code:
`src/multifidelity/double_pipe/koh.rs` (`koh_spec`, `TuasOneDUa`) and
`one_d::ua`, TUAS's UA without the time march, which
`march_free_ua_is_the_marched_ua` pins **bit-identical** to the marched UA
at 9 (x, θ) pairs.

Run: `cargo test --release -p outram-park-digital-twin-engine --lib -j 2
koh_calibration_of_the_double_pipe -- --nocapture`, 1 160 s on the shared
4-CPU host (calibration 714 s). Full output, unedited apart from removed
compiler warnings: `double_pipe_koh_gh1180/run_2026-11-04.txt`.

## Results

**Emulator** (plug-in, highest-posterior particle of the simulation-only
TMCMC): m = 264 low-fidelity runs, ln UA standardised by mean −0.1383,
sd 0.5238; λ_Uz 0.313, λ_Ws 12 578, λ_WOs 7 370. n = 36 observations.

### θ posterior (with δ)

| θ coordinate | prior | posterior mean | sd | 95 % interval |
|---|---|---|---|---|
| ln θ_h | N(0, 0.2²) | **+0.164** (θ_h 1.18) | 0.116 | [−0.069, +0.383] |
| ln θ_c | N(0, 0.3²) | **+0.336** (θ_c 1.40) | 0.171 | [+0.027, +0.683] |
| r_f [m² K/W] | U[0, 1e-3] (sd 2.9e-4) | **5.43e-4** | 2.50e-4 | [5.7e-5, 9.6e-4] |

Correlations: (ln θ_h, ln θ_c) **−0.70**, (ln θ_h, r_f) +0.17,
(ln θ_c, r_f) +0.43. Observation error on ln UA: mean 0.0073, 95 %
[0.0054, 0.0103] (prior centred on 0.0105).

### δ(x) at the 36 training x

Posterior mean δ on ln UA ranges from **−0.092 (T23: Re_hot 30, Re_cold 45,
k_wall 100, L 0.1)** to **+0.090 (T28: Re_hot 90, Re_cold 5, k_wall 100,
L 0.3)**, with posterior sd 0.027–0.043. |δ| < 0.05 at **19 of 36**.
δ is negative at 29 of 36 points (mean about −0.035): the posterior θ puts
the 1-D UA **above** the CFD almost everywhere and δ brings it back down.
The positive δ sits at Re_cold = 5 with Re_hot ≥ 30, the corner where
gh:#1178 found the series-UA / `UA_local` mismatch largest (T28 was the
−21.4 % uncalibrated point). Per point: the run output.

### Held-out predictive (KOH, with δ and ε)

| id | CFD UA [W/K] | median | error | 95 % interval | z |
|---|---|---|---|---|---|
| H1 | 1.14336 | 1.14229 | −0.09 % | [1.11116, 1.17463] | +0.06 |
| H2 | 1.15001 | 1.14177 | −0.72 % | [1.10951, 1.17341] | +0.51 |
| H3 | 1.29950 | 1.31644 | +1.30 % | [1.28006, 1.35661] | −0.88 |
| H4 | 1.27261 | 1.27673 | +0.32 % | [1.23945, 1.31250] | −0.21 |
| H5 | 0.76961 | 0.76711 | −0.32 % | [0.74607, 0.78785] | +0.24 |
| H6 | 1.62369 | 1.66034 | +2.26 % | [1.61566, 1.71095] | −1.55 |
| H7 | 1.19173 | 1.19626 | +0.38 % | [1.16857, 1.22345] | −0.31 |
| H8 | 1.12149 | 1.11051 | −0.98 % | [1.08432, 1.13613] | +0.83 |

**Held-out UA relative RMSE 1.28 %; 95 % coverage 8/8; 2σ coverage 8/8.**
For comparison on the same 8 points and metric: SMT MFK (gh:#1190) 3.0 %;
KRG on the CFD alone 13.9 %; the degree-1 polynomial (gh:#1194) 11.1 %; the
uncalibrated 1-D model 8.95 %; **the 1-D model alone at the posterior-mean
θ (no δ) 6.30 %**.

### Ablation (each component reset to its derived value in every sample)

| reset | 1-D alone at the posterior mean | change | KOH with δ | change | 95 % coverage |
|---|---|---|---|---|---|
| none | 6.30 % | — | 1.28 % | — | 8/8 |
| θ_h → 1 | 4.90 % | **−1.40** points | 1.22 % | −0.05 | 8/8 |
| θ_c → 1 | 8.57 % | +2.27 | 1.70 % | +0.42 | 7/8 (H6 out) |
| r_f → 0 | 17.33 % | **+11.03** | 1.14 % | −0.14 | 8/8 |

### No-δ contrast (SEPIA's supported univariate model, same priors and emulator)

ln θ_h −0.025 ± 0.036; **ln θ_c +0.777 ± 0.086** (θ_c 2.17, near the
design range's upper end 0.9); **r_f 8.5e-4 ± 1.3e-4** (near the 1e-3
upper end). ε on ln UA inflates to 0.030 (95 % [0.022, 0.040]). Held-out
UA RMSE 1.80 %, coverage 8/8 with 95 % intervals about 2.2 times as wide (e.g. H1: 0.143 W/K against 0.063).

### Independent checks (not in the calibration)

- **The CFD's own Nusselt ratios** (CFD Nu_1D / TUAS textbook Nu over all
  44 points): tube 0.877–1.078, annulus 1.257–1.581. Posterior median
  **θ_c 1.392: inside** the annulus range (and inside the literature
  developed-annulus ratio 1.28–1.64). Posterior median **θ_h 1.182:
  outside** the tube range, 10 % above its top.
- **Marched duty Q at the posterior-mean θ** (θ = (1.186, 1.420, 5.43e-4),
  no δ; `CorrectedModel { TuasOneD, ThetaEstimate::Posterior }`):

| id | CFD Q [W] | calibrated 1-D | uncalibrated 1-D |
|---|---|---|---|
| H1 | 20.603 | +1.52 % | −3.51 % |
| H2 | 22.991 | +1.85 % | −2.58 % |
| H3 | 29.545 | +1.02 % | −5.31 % |
| H4 | 38.584 | +4.72 % | −3.52 % |
| H5 | 19.593 | +2.81 % | −2.57 % |
| H6 | 32.128 | +0.66 % | −4.91 % |
| H7 | 28.946 | +3.03 % | −3.78 % |
| H8 | 42.149 | +6.66 % | −2.38 % |

## Predictions against results

| # | prediction | measured | |
|---|---|---|---|
| K1 | ln θ_c mean in [0.15, 0.45], sd in [0.05, 0.25] | 0.336, sd 0.171 | **met** |
| K2 | ln θ_h mean in [−0.15, 0.15], sd > 0.10 | 0.164, sd 0.116 | **missed** (mean just above); sd met |
| K3 | r_f mean < 2.5e-4, piled at 0 | 5.43e-4 ± 2.50e-4 | **missed badly**: the posterior is close to the prior (mean 5e-4, sd 2.9e-4) and correlates with both multipliers. r_f is not identified by UA with δ present |
| K4 | corr(ln θ_h, ln θ_c) < −0.3 | −0.70 | **met** |
| K5 | \|δ\| < 0.05 at ≥ 30/36, largest at L 0.3, Re_hot 10 | 19/36; largest at T23 (−0.092) and T28 (+0.090) | **missed** on both counts |
| K6 | held-out RMSE 1.5–6 %, within ×2 of MFK's 3.0 % | **1.28 %** (MFK / KOH = 2.3) | **missed, on the good side**: better than predicted, and better than MFK by more than ×2 |
| K7 | 95 % coverage 7–8/8 | 8/8 | **met** |
| K8 | 1-D alone: θ_c ≥ +5 points, θ_h ≤ +3, r_f < +0.5 | +2.27, −1.40, **+11.03** | θ_c **missed**, θ_h met (and it improved), r_f **missed badly** |
| K9 | with δ, each reset moves < half its 1-D-alone change | −0.05 / +0.42 / −0.14 against −1.40 / +2.27 / +11.03 | **met** |
| K10 | no-δ: ln θ_c higher and sd < half of KOH's | 0.777 > 0.336; sd 0.0860 vs half 0.0855 | higher **met**; sd **missed by 0.6 %** of the bound |
| K11 | θ_c inside [1.26, 1.58], θ_h inside [0.88, 1.08] | 1.392 inside; **1.182 outside** | θ_c **met**, θ_h **missed** |
| K12 | calibrated 1-D Q within 2.5 % at all 8 | within at 4/8; +0.66 … **+6.66 %** | **missed** |

## Interpretation, with identifiability stated plainly

1. **The KOH predictive is the number to quote: held-out UA 1.28 % RMSE,
   8/8 inside 95 %.** It beats MFK's 3.0 % on these 8 points with 36 CFD
   rows and no extra low-fidelity grid. Eight points cannot separate
   1.3 % from 3 % with much confidence; the coverage is 8/8 for both
   intervals, so the intervals are, if anything, generous.
2. **θ is only partly identified, and the posterior θ is not a standalone
   correction.** UA depends on θ only through two series conductances,
   and δ is free to absorb any smooth offset. The data pin θ_c (posterior
   sd 0.17 against the prior's 0.30, median inside the CFD's own annulus
   ratio range), but not θ_h or r_f: θ_h is pushed 10 % above anything the
   CFD's tube Nusselt numbers support, r_f (whose true value in the CFD is
   **0**) is not pulled toward 0, and the two compensate each other. That
   is why the ablation of r_f wrecks the 1-D-alone UA (+11 points) yet
   barely moves the KOH predictive (−0.14): **δ re-absorbs any θ change**,
   the confounding Brynjarsdóttir & O'Hagan (2014) describe. It is also why
   the marched Q at the posterior-mean θ (K12) is **worse** than
   uncalibrated at H8 (+6.7 % against −2.4 %): the posterior-mean θ alone
   over-predicts UA (δ is negative at 29/36 points), so a caller who drops
   δ inherits an over-correction.
3. **Without δ** the calibration is over-confident and pushed to the edges
   of the design range (θ_c 2.17, r_f 8.5e-4, both implausible against the
   CFD's own Nusselt numbers), the observation error inflates fourfold to
   absorb the misfit, and the predictive is wider and worse (1.80 %).
4. **What to use downstream (rung 5's demo):** the KOH predictive
   `η(x, θ) + δ(x)` with its interval for UA; not `CorrectedModel` with the
   posterior mean and no δ. For Q a δ on Q (or on ε) would be needed: not
   done here.

## What this does not show

- One output (UA). Q, ε, the outlet temperatures and Δp are not calibrated;
  the friction multipliers are not identifiable from UA and stay at 1.
- The emulator's hyperparameters are a plug-in (KOH 2001 §4.5), not
  sampled; the emulator's own accuracy at the held-out x was not checked
  separately from the predictive.
- A single TMCMC run per form (seeds 1180, 1181): no seed-to-seed spread
  was measured on the double pipe (the toy showed 0.01-level θ differences
  between seeds at too few moves, and agreement with SEPIA at 20).
- r_f was calibrated because the issue asked for a wall/contact resistance;
  the CFD has none, and the result shows UA cannot say so.
- No experimental data; the CFD carries about 1 % mesh error (gh:#1178).
