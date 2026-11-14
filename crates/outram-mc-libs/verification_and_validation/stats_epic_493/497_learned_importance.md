# #497 — variance reduction from learned importance: V&V protocol (stub)

**Issue:** [gh:#497](https://github.com/theodoreOnzGit/outram-park-backend/issues/497), epic #493.
**Code:** `src/stats/learned_importance.rs` (`learned_weight_windows`,
`VrArm`, `FomComparison`); `raffles::surrogate::{PolynomialSurrogate,
validation::LeaveOneOut}`.

> **Status: NOT YET MEASURED (testing deferred by maintainer, 2026-10-03).**
> Nothing below has been run. No number on this page is a result.

## Off by default — and why

This is an **estimator**, not physics: weight windows play fair split /
roulette games that leave every expected score unchanged. So, as for
`physics/variance_reduction.rs`, analog is the correct default and the root
`CLAUDE.md` "correct physics is the default" rule does not apply. Nothing here
runs unless a caller builds the windows and attaches them with
`VarianceReduction::with_weight_windows`.

## What is being checked

1. **Unbiasedness (the gate).** The tally mean with learned windows agrees
   with the analog mean. A biased answer fails regardless of FOM.
2. **Figure of merit.** `FOM = 1/(R²T)`, `R` the relative error of the tally,
   `T` the wall-clock, compared against (a) analog and (b) plain MAGIC
   windows (`WeightWindows::update_magic`) built from the **same** early
   tally. Ratio reported with its uncertainty
   (`FomComparison::fom_ratio_sigma`).
3. **Fill quality.** Per energy group: resolved / filled / clamped / empty
   cell counts, and the surrogate's leave-one-out RMSE in `ln φ`.

## Methodology

- Case: the shielded room of `tests/openmc_notebooks/shielded_room_weight_window.rs`
  (31 × 38 mesh, the case `variance_reduction/fom_2026_09_24.md` measured
  MAGIC on), so the MAGIC arm has a recorded precedent. Same geometry,
  materials, source and mesh in all three arms.
- Early tally: the analog arm's first batches (count fixed before the run);
  windows built once, then held fixed for the measured run.
- All three arms on one machine, hardware recorded (crate `CLAUDE.md`).

## Pass criteria (fixed in advance, 2026-10-03)

- **Unbiased:** `|z| ≤ 2` (`UNBIASED_Z_BAND`), `z = Δmean / sqrt(σ_a² + σ_v²)`,
  for the headline tally and for every mesh cell both arms resolve (with the
  expected ~5 % of cells outside 2σ by chance stated, not hidden).
  **Open question for the maintainer:** #497 says "within combined σ"; read
  literally as 1σ, a correct estimator fails 32 % of the time. 2σ is used
  here and flagged, decided before any result exists.
- **FOM:** reported, with uncertainty, against analog and MAGIC. No FOM
  threshold is a pass/fail gate — the issue asks for a measured ratio.

## Protocol for the 2026-11-14 measurement (fixed before any run)

Test: `shielded_room_learned_importance` in
`tests/openmc_notebooks/shielded_room_weight_window.rs`. Command:
`cargo test --release -j 2 -p outram-mc-libs --no-default-features --test openmc_notebooks shielded_room_learned_importance -- --nocapture --ignored`.

- **Seeds.** N = 5 independent seeds per arm; each seed is one
  `run_fixed_source` call of `n` particles in 10 batches. The estimate is the
  mean over seeds of the per-run headline; its sigma is the seed-to-seed sample
  standard deviation / sqrt(N). `VrArm.n_batches` = N = 5.
- **Headline tally.** Flux per source particle summed over mesh cells whose
  centre lies 900-1450 cm (xy distance) from the source: the "far field" band
  of `fom_2026_09_24.md`, chosen because it is the band where MAGIC was
  previously seen to pay. The 0-400 and 400-900 cm bands are reported too; all
  three were fixed here, not after seeing results.
- **Time.** T = total wall-clock of the N runs of that arm (transport only);
  the early-tally generation run (shared by MAGIC and learned) is reported
  separately and the FOM is given both without and with it charged.
  FOM = 1/(R^2 T), R = sigma/|mean| of the N-seed mean.
- **Early tally.** One analog run, 2000 particles / 10 batches, seed distinct
  from every measured seed. MAGIC (`update_magic`, threshold 1.0, ratio 5, as
  in the 2026-09-24 precedent) and learned (`learned_weight_windows`, same
  threshold/ratio, other settings default) are built once from that same tally
  and held fixed.
- **Particle counts per seed** are set from a cost probe (time per particle),
  never from tallies, so that all arms finish in well under an hour on this
  4-core box; they are printed in the results.
- **Unbiasedness**: z = (mean_vr - mean_analog)/sqrt(s_a^2 + s_v^2), reported
  against BOTH bands, |z| <= 1 and |z| <= 2. No pass/fail is chosen. Per-cell
  z is computed over cells with a nonzero score in every seed of both arms.

## Prediction (written 2026-11-14 BEFORE any run)

1. **Bias.** The argument says none. Each VR arm's headline z against analog
   is expected inside |z| <= 2 (probability about 0.95 each if unbiased) and
   inside |z| <= 1 with probability about 0.68 each, so about 0.47 that both
   pass the 1-sigma reading by chance alone. A |z| > 2 would be a real finding
   (a defect in the game), not noise to explain away. Per-cell: about 32 % of
   cells outside 1 sigma and 5 % outside 2 sigma expected by chance.
2. **FOM vs analog.** From `fom_2026_09_24.md` (MAGIC far-field ratio 0.252x,
   cost ratio 5.7x) I predict MAGIC FOM/analog in **0.1-0.5, point 0.25**:
   below 1, i.e. no net FOM gain on the cells the analog arm resolves, with the
   early tally here being 25x smaller (2000 vs 50000 particles) so fewer windows.
3. **Learned vs MAGIC.** I predict **learned/MAGIC in 0.5-1.5, point 0.8**, and
   more likely below 1 (about 60 %) than above. Reason: a degree-2 polynomial
   in ln(flux) over (x, y) cannot represent an L-shaped bunker with 200 cm
   concrete walls and a corridor; filled cells beyond the walls get windows
   from a smooth extrapolation that is wrong in level, and the extra windowed
   cells add splitting cost (the cost pathology recorded for deep MAGIC sets).
   Learned/analog is therefore predicted **0.05-0.5, point 0.2**. A learned
   arm that beats analog (ratio > 1) would surprise me (about 10 %).
4. **LOO RMSE in ln(flux)** is predicted large (> 1, i.e. worse than a factor
   e) because of the wall discontinuities.

## Results

| arm | date | mean | R | T (s) | FOM | FOM / analog | z vs analog | hardware |
|---|---|---|---|---|---|---|---|---|
| analog | — | NOT YET MEASURED | | | | | | |
| MAGIC | — | NOT YET MEASURED | | | | | | |
| learned | — | NOT YET MEASURED | | | | | | |
