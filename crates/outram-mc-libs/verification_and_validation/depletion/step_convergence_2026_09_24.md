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
   nominal 4** ✘ (2026-09-24; **3.930 ✔ since 2026-10-17**, see the correction below).

## CORRECTED 2026-10-17 (gh:#950): the cause was a defect in the CF4 port, and CF4 is 4th order here

**What was wrong.** Upstream's `CF4Integrator` docstring writes the final
update as `exp(A1/4 + A2/6 + A3/6 - A4/12) exp(-A1/12 + A2/6 + A3/6 + A4/4) n`;
read as an operator product it applies the second factor first, and this
crate's CF4 did exactly that. Upstream's **code** applies the first factor
first (`openmc/deplete/integrators.py:180-183`, `cf4_f3` then `cf4_f4`).
Swapping the pair flips the sign of the commutator term the pair is built to
reproduce, which costs two orders. The port also skipped upstream's clip of the
first stage (`abc.py:765-766`).

**How it was established.** The integrators moved to PETIR and now follow
upstream's code; PETIR's integrator logic is bit-identical to OpenMC's own
running `integrate()` on a fixed-rate problem
(`crates/petir/tests/depletion_vs_openmc.rs`). On PETIR's exact test problem
(composition-dependent flux, non-commuting matrices, closed-form answer) the
docstring order measures **2.000** and the code order **3.98**
(`crates/petir/verification_and_validation/depletion/petir_integrators_2026_10_17.md`).
`tests/depletion_move_fingerprints.rs` replays the removed CF4 verbatim on the
moved CRAM and reproduces its old fingerprint exactly, so nothing else changed.

**Re-measured on this history, 2026-10-17** (reference: the new Cf4 at
h = 0.0078 d, `k_inf = 1.781684744358`), fine band:

| h [d] | Predictor `\|err\|` | p | CeCm `\|err\|` | p | Cf4 `\|err\|` | p |
|---|---|---|---|---|---|---|
| 0.2500 | 1.309e-5 | 0.472 | 1.186e-5 | 1.435 | 1.401e-6 | 3.190 |
| 0.1250 | 8.460e-6 | 0.543 | 4.015e-6 | 1.808 | 1.483e-7 | 3.740 |
| 0.0625 | 5.124e-6 | 0.768 | 1.115e-6 | **1.947** | 1.100e-8 | **3.930** |
| 0.0313 | 2.833e-6 | | 2.870e-7 | | 7.174e-10 | |
| 0.0156 | 1.488e-6 | | 7.228e-8 | | **4.267e-11** | |

Cf4 is **1700×** more accurate than CeCm at the finest step, for twice the
solves. The predicted outcome ("well above 2, toward 4") was written down
before this run, in the PETIR record's section 1. The CeCm and predictor
columns are unchanged in method; their errors moved only because the
reference did. `examples/depletion_integrator_order_scan.rs` now prints all
eight integrators; on this history CE/LI, LE/QI, SI-CE/LI and SI-LE/QI also
come out near 4 on `k_inf` (3.78 to 4.00 at h = 0.0625 d) and EPC-RK4 at 1.95.
That is a measurement on one functional of one history, **not** a statement of
their orders; their orders were measured on PETIR's exact problem (2, 3, 2, 3
and 2 respectively).

The struck section below is kept because it shaped the 2026-09-24 record.

## ~~The finding: CF4 does not achieve 4th order here, and CeCm beats it~~ (superseded, see above)

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
3. ~~**Five of eight integrators are absent**~~ **CORRECTED 2026-10-17:
   all eight are present (PETIR, gh:#950)** — CELI, EPCRK4, LEQI, SICELI,
   SILEQI. Nothing here says what they would be worth.
4. ~~**The CF4 cause is unconfirmed**, as stated above.~~ **CORRECTED
   2026-10-17:** confirmed and fixed — the docstring exponential order (see
   the correction at the top of the finding).
5. **No transfer-rate convergence study.** `TransferRate` and
   `apply_transfer_rates` exist with unit tests (including a pebble-recirculation
   case), but their interaction with step size is not measured here.
