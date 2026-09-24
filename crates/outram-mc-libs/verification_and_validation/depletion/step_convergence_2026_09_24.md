# Depletion step-size convergence and integrator order — measured

**Date:** 2026-09-24
**Issue:** [gh:#266](https://github.com/theodoreOnzGit/outram-park-backend/issues/266).
Acceptance: *"The convergence study, with methodology and measured results,
recorded per the workspace V&V rule"* and *"Each new integrator reproduces the
predictor answer in the small-step limit and converges faster in step size, with
the observed order reported as a measured number."*
**Code:** `src/depletion/integrators.rs` (`Integrator`, `observed_order`),
`src/depletion/operator.rs` (`deplete_with`, new).
**Runners:** `tests/depletion_step_convergence.rs` (2 tests),
`examples/depletion_step_convergence_scan.rs`,
`examples/depletion_integrator_order_scan.rs` (new).

## Problem

One-group burnup of 3 % enriched UO2 on `DepletionChain::simple` (the 9-nuclide
`chain_simple.xml` chain), 1 MW in 1000 cm³, 40 days total held fixed while the
step is halved. Beginning-of-life densities in **atoms/barn·cm**: U-235
7.0e-4, U-238 2.2e-2. Sanity: **15.7630 %** of the U-235 burns over the 40 days,
checked *before* any convergence claim.

> **A near-miss worth keeping.** The first draft used `1.0e21` for those
> densities, reading the field as atoms/cm³ when it is atoms/barn·cm — 23 orders
> of magnitude out. `Sigma_f` then becomes enormous, the flux needed for 1 MW
> collapses to 5.5e-11 n/cm²/s, **nothing burns**, and every step size returns a
> bit-identical answer. The study would have "passed" against a solution that
> never moved. The burned-fraction check exists because of it.

## What had to be built first

`Integrator::CeCm` and `Integrator::Cf4` existed as **single-step** methods over
a matrix-rebuild closure, unit-tested on synthetic matrices — but **no burnup
driver took an `Integrator`**: `deplete_predictor` hard-coded the predictor. So
their order could not be measured on a real history, and acceptance bullet 2 was
not merely untested but unevaluable. `deplete_with(integrator, …)` closes that.

The flux is rebuilt from **each stage's own densities** rather than frozen at
begin-of-step. That matters: with a frozen matrix every method integrates the
same constant-coefficient system and CF4 is exact for the wrong reason. The
constant-matrix case is pinned separately by
`every_integrator_is_exact_when_the_matrix_is_constant`.

## The instrument, and the two ways it was wrong first

**1. The reference must not be the predictor's own finest run.** A first version
used `predictor at h = 0.039 d` as truth, and the data said it was wrong: CeCm's
and Cf4's errors came out **non-monotonic** (2.95e-5 → 4.99e-5 → 5.03e-5 →
3.51e-5 → 9.31e-6). That is the signature of a higher-order method *crossing* a
biased reference, not converging to it — the predictor's finest answer still
carries first-order error, so no error measured against it can fall below that
bias. The reference is now **Cf4 at h = 0.0078 d**, `k_inf = 1.781684708134`.

**2. Order cannot be measured across the Xe-135 transient.** Xe-135's half-life
is 9.14 h = **0.3808 d**. A step longer than that does not resolve its
transient and **no method is in its asymptotic regime**. This is not a guess: the
same study measures the predictor's Xe-135 order as **−3.701** at 5 d steps and
**+1.229** once the step is at or below the half-life. Mixing the two regimes
produces the nonsense orders above (`p = 5.754`, `p = −5.329`). They are reported
separately.

## Results — coarse band (step > Xe-135 half-life)

What a method is *worth* at a step someone would actually use. `|err|` against
the reference.

| h [d] | Predictor | CeCm | Cf4 |
|---|---|---|---|
| 5.000 | 4.602e-4 | 2.605e-5 | **7.491e-6** |
| 2.500 | 2.003e-4 | 4.645e-5 | 4.822e-5 |
| 1.250 | 7.741e-5 | 4.683e-5 | 6.773e-5 |
| 0.625 | 3.066e-5 | 3.163e-5 | 5.299e-5 |

**At h = 5 d, CeCm is 17.7× and Cf4 61.4× more accurate than the predictor**, for
2× and 4× the matrix solves. That is the practical statement and it is real.

The **truncation error of the step sizes used elsewhere in this crate** —
`depletion_coupled_to_transport.rs` and `depletion_one_group_collapse.rs` — is
therefore about **4.6e-4 in `k_inf`, i.e. ~46 pcm**, at a 5 d predictor step.
Previously unknown rather than small, which is what #266 said.

Note the non-monotonicity *within* this band for the higher-order methods. It is
not a defect in them: outside the asymptotic regime the error has no reason to be
monotone in `h`, and the reference is itself far better than any coarse-band
point.

## Results — fine band (step < Xe-135 half-life), where order is measurable

| h [d] | Predictor `\|err\|` | p | CeCm `\|err\|` | p | Cf4 `\|err\|` | p |
|---|---|---|---|---|---|---|
| 0.2500 | 1.312e-5 | 0.472 | 1.182e-5 | 1.435 | 2.230e-5 | 1.351 |
| 0.1250 | 8.497e-6 | 0.543 | 3.978e-6 | 1.808 | 7.851e-6 | 1.783 |
| 0.0625 | 5.160e-6 | 0.768 | 1.079e-6 | **1.947** | 2.184e-6 | **1.941** |
| 0.0313 | 2.869e-6 | | 2.507e-7 | | 5.373e-7 | |
| 0.0156 | 1.524e-6 | | **3.605e-8** | | 1.084e-7 | |

**Against acceptance bullet 2:**

1. **Small-step agreement** — all three errors fall towards zero
   (1.5e-6 / 3.6e-8 / 1.1e-7 at the finest step), so all three converge to the
   same limit. Relative agreement with the reference at h = 0.0156 d is
   **8.6e-7, 2.0e-8, 6.1e-8**. ✔
2. **Converges faster in step size** — CeCm is **42×** and Cf4 **14×** more
   accurate than the predictor at the same finest step. ✔
3. **Observed order as a measured number** — Predictor **0.77 and rising towards
   its nominal 1**; CeCm **1.947 against a nominal 2** ✔; Cf4 **1.941 against a
   nominal 4** ✘.

## The finding: CF4 does not achieve 4th order here, and CeCm beats it

Cf4's observed order is **1.94**, not 4, and CeCm — half the cost — is **3×
more accurate** at the finest step. That is a measured result and it is reported
rather than smoothed.

**Leading explanation, reasoned and not measured.** CF4 is a commutator-free
exponential integrator for a **linear non-autonomous** system `dn/dt = A(t) n`.
This problem is not that: `flux_for_power` renormalises the flux at every stage
so the power stays at 1 MW, which makes the reaction rates — and hence `A` — a
**function of `n` itself**. For a nonlinearly-coupled `A(n)`, commutator-free
exponential methods lose their high order, while a midpoint method like CE/CM
retains second order. Upstream's `CF4Integrator` is coupled to a
power-normalised flux in the same way.

**What would discriminate it**, and has *not* been done: re-run with the flux
held fixed across the step (making `A` genuinely independent of `n` within the
step) and see whether Cf4 recovers 4th order. If it does, the order loss is the
power normalisation and the implementation is right; if it does not, the stage
construction is suspect. Until that runs, the 1.94 is a measurement without a
confirmed cause, and the explanation above is a hypothesis.

## Limitations

1. **One problem, one chain.** A 9-nuclide chain with one dominant short-lived
   fission product. A full chain has a spectrum of half-lives and its own
   stiffest timescale, which would move the regime boundary.
2. **`k_inf` only.** The order is measured on the end-of-life eigenvalue. Xe-135
   density is reported by the test but not order-analysed in the fine band.
3. **Five of eight integrators are absent** — CELI, EPCRK4, LEQI, SICELI,
   SILEQI. Nothing here says what they would be worth.
4. **The CF4 cause is unconfirmed**, as stated above.
5. **No transfer-rate convergence study.** `TransferRate` and
   `apply_transfer_rates` exist with unit tests (including a pebble-recirculation
   case), but their interaction with step size is not measured here.
