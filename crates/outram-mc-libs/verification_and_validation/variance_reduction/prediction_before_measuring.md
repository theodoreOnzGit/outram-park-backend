# Survival biasing + Russian roulette — the prediction, recorded BEFORE the run

GitHub **#258**. Written 2026-09-22, **before**
`examples/variance_reduction_ablation.rs` was executed. Unedited since; the
measured results are in `ablation_2026_09_22.md`.

## The case

Bare HEU sphere at Godiva's critical radius (8.7407 cm), U-235 + U-238 from
`reference-data/endf/` at 293.6 K, 2000 histories × [15 inactive + 40 active],
paired over seeds. Two arms differing **only** in `KeffSettings::variance_reduction`:

- **ANALOG** — the default. Capture kills the particle.
- **SURVIVAL** — `survival_biasing: true`, `weight_cutoff: 0.25`,
  `weight_survive: 1.0` (upstream's defaults). Capture reduces the weight by
  `Σ_a/Σ_t`; Russian roulette is played when the weight falls below the cutoff.

## The predictions

1. **Unbiased: the two arms must give the SAME mean `k`.** These are two
   estimators of one eigenvalue, not two physics models. A resolved difference
   is a **bug**, not a variance saving, and would be reported as one. Predicted
   difference: **zero**, i.e. consistent within the combined standard error of
   the two ensemble means.

2. **Variance down.** Survival biasing removes one source of randomness (which
   collisions happen to capture) and replaces it with a deterministic weight
   reduction. A single-seed run already seen on this case gave per-generation
   spread `4205 → 3121 pcm`. Predicted **seed-to-seed standard deviation of
   `k` reduced by a factor 1.2–1.5**, i.e. variance down 1.4–2.2×. The gain is
   bounded because the dominant variance in a k-eigenvalue run is
   fission-source noise, which survival biasing does not touch.

3. **Cost up, and the figure of merit could go EITHER WAY.** A neutron that
   analog transport would have captured keeps flying until roulette kills it.
   With `Σ_a/Σ_t ≈ 0.15–0.25` on Godiva and a cutoff of 0.25, a history
   survives roughly `ln(0.25)/ln(1 − Σ_a/Σ_t) ≈ 5–9` extra collisions.
   Predicted **wall-clock per history up 1.5–3×**.

   So the figure of merit `FOM = 1/(σ² t)` is the product of a 1.4–2.2× gain
   and a 1.5–3× loss. **The predicted FOM change is between ×0.5 and ×1.5 —
   i.e. this technique is predicted NOT to pay on this problem**, and might
   cost. That is stated here in advance precisely so that a null or negative
   result is reported as the finding rather than quietly dropped in favour of
   a case where it flatters.

   Survival biasing earns its keep on **deep-penetration shielding**, where
   analog histories die long before reaching the detector. A bare fast
   critical sphere is close to the worst case for it, and this ablation is
   chosen for that reason: it measures the unbiasedness, which is what matters
   for correctness, on a problem where the variance argument cannot carry the
   result.

---

# Addendum — a second prediction, for the residual the first run left

Written 2026-09-22 **after** the 96-seed run and **before** the `N = 8000`
run's output was read. (The 8000-particle job was already in flight when this
was written; nothing below is derived from its result, which had not been
looked at. The reasoning is from theory and from the 96-seed numbers alone.)

## What the first run left unresolved

| | 24 seeds | 96 seeds |
|---|---|---|
| survival − analog | +212 ± 130 pcm (1.63 σ) | **+122 ± 62 pcm (1.97 σ)** |

Prediction 1 said the difference should be **zero**. At 1.97 σ it is not
resolved either way, and the significance went *up*, not down, when the
statistics improved. Per this crate's own history — the hybrid-tracking gate
that was "inside 4 σ and equally consistent with a real 1200 pcm bias" — that
is a result that cannot be left as "consistent with zero" and called done.

## The hypothesis

**Fission-bank population control.** Power iteration with a fixed-size fission
bank carries a well-known eigenvalue bias of order `1/N` in the bank size,
with a coefficient proportional to the **variance of the per-history fission
production**. Both arms carry it. Survival biasing changes that variance by
construction — that is what it is for — so the two arms legitimately sit at
slightly different points of the same `1/N` curve at finite `N`. This is a
property of power iteration, not a defect in the estimator.

Both runs above used `N = 2000` histories per generation.

## The falsifiable consequence, stated before measuring

Run the identical ablation at **`N = 8000`**, four times the bank.

- **If the residual is population control**, it scales as `1/N` and must fall
  to **≈ +31 pcm**, i.e. consistent with zero at the ~45 pcm sensitivity a
  32-seed run at that `N` provides.
- **If it stays near +122 pcm**, it is *not* population control, the residual
  is real, and there is a defect in the survival-biasing estimator to find —
  which would be reported as such, and the technique would not be recommended
  for use until it was found.

Anything in between (say +60 to +90 pcm) is a partial result and would be
reported as one, with the next step being `N = 32000` rather than a
declaration either way.

**The scaling is the test, not the size.** A single number consistent with
zero at wider error bars would prove nothing, which is exactly the trap the
first run fell into.

---

# Addendum 2 — the N = 8000 result, and why it settles nothing

Measured 2026-09-22, 32 paired seeds at `N = 8000`:

    survival - analog = +83 +/- 64 pcm (1.30 sigma)

Addendum 1 predicted **≈ +31 pcm** if the residual is population control, and
**≈ +122 pcm** if it is real.

**It is +83, which is the band Addendum 1 named in advance as a partial
result** — and the honest reading is worse than "partial". Against the
`N = 2000` value of `+122 ± 62`, this is a difference of `39 ± 89 pcm`: the
two measurements are **statistically indistinguishable**. The central value
fell by a factor 1.47 where `1/N` predicts 4, but with these error bars the
measurement cannot tell a factor of 4 from a factor of 1.

So the test was **underpowered**, and saying "consistent with population
control" would be exactly the failure this crate's own history warns about —
the hybrid-tracking gate that was "inside 4 sigma and equally consistent with
a real 1200 pcm bias".

Addendum 1 pre-registered the next step for this case, and it is being taken
rather than reinterpreted: **`N = 32000`, 48 seeds**.

| hypothesis | predicted at N = 32000 |
|---|---|
| population control (`1/N` from the `N = 2000` value) | **≈ +8 pcm** |
| a real estimator bias | **≈ +83 to +122 pcm** |

At 48 seeds the combined standard error is ~20 pcm, so the two predictions are
~4 sigma apart and the run can actually decide. **If the residual stays near
+83 pcm, it is not population control, and survival biasing in this port has a
defect that must be found before the technique is used anywhere.** That is
stated here, before the run, so the conclusion is not chosen after seeing the
number.

## What is not in question

Everything except the bias check already met its prediction and has tightened
with statistics rather than moved:

| | predicted | N=2000 (96 seeds) | N=8000 (32 seeds) |
|---|---|---|---|
| sd ratio | 1.2–1.5x | 1.49x | 1.46x |
| cost ratio | 1.5–3x | 1.45x | 1.45x |
| FOM ratio | 0.5–1.5x | 1.55x | 1.47x |

The cost ratio came in slightly **below** the predicted floor (1.45 against
1.5), so that part of Addendum 1's prediction was marginally wrong and is
recorded as such.
