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
