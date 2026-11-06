# HTR-10 k_eff against control-rod bank position: predictions (gh:#1025)

**Written 2026-10-23, before any production run.** Only a 5000-history
timing probe had been run (withdrawn 0.982 ± 0.005, fully in 0.868 ± 0.012;
far too few histories to inform anything below beyond "the run works").

**Research, education and V&V only.** Not for facility operation, licensing
or safety decisions.

## The case

- Model: `nee_soon::htr10_rmc` exactly as the 2026-10-07 k-vs-height record
  (`crates/nee_soon/verification_and_validation/htr10_seker_2026_10_07_10k/`),
  14 rings x 12 Şeker layers (bed 123.576 cm, 16 681 balls), ENDF/B-VIII.0 at
  300.15 K (27 °C), helium, every default physics term on. The only change is
  `Htr10CoreDesign::rod_insertion`: all ten explicit rods moved together.
- Positions (rod lower end `z_T`, cm; TECDOC-1382's axial coordinate, downward
  from the model top): 119.2 withdrawn and 394.2 fully inserted (§ 4.1.1.5,
  printed p. 236), 171.2 (the measured calibration's start, § 4.2.1.5,
  p. 252) and B42's seven positions (§ 4.1.2.4, p. 243).
- In this model's coordinates the bed top is at `z_T` 228.2 cm and the bottom
  of the bed's cylindrical part at `z_T` 351.8 cm (the conus below). At full
  insertion the rod top is at `z_T` 129.5 cm, so B4C spans the whole bed.

## Which benchmark numbers this is compared with

- **B41** (ten rods fully inserted, initial core, 126 cm loading, helium,
  calculations at 27 °C), **original** benchmark, IAEA-TECDOC-1382 Table 4-8,
  printed p. 248: **VSOP 18.27 %, MCNP 19.36 %** (INET; MCNP-4A with ENDF/B-V,
  p. 245). These are CALCULATIONS, not measurements.
  - Original, not deviated: the deviated benchmark (Tables 4-13 to 4-15,
    p. 250) changes the dummy balls to 1.84 g/cm³ and 0.125 ppm boron
    (p. 249). This model's dummy balls are the original specification's
    1.73 g/cm³ graphite with 1.3 ppm (`outram-mc-libs`
    `pebble_beds::htr10::RHO_GRAPHITE`), so the original column is the
    like-for-like one.
- Worth is ~~`rho = 1/k_out - 1/k_in`~~ **CORRECTED 2026-10-23 (a sign slip in the formula as typed, found when the panel printed a negative worth; the predicted magnitudes below are unchanged):** `rho = 1/k_in - 1/k_out`, positive for an insertion, in % Δk/k. TECDOC does not define its
  "reactivity worth" formula in the B4 section; this reading is stated, not
  sourced.
- **Not compared:** B42 and the measured single-rod S3 worth (1.4693 %,
  p. 252) are ONE rod; this curve moves ten. Their positions are shared only
  so the shapes can be laid side by side.
- **Lebenhaft (MIT 2001) Table 5-8:** not readable in this environment
  (dspace.mit.edu denied). Not used.

## Predictions

1. **Withdrawn k equals the record's N = 12 point** (ENDF/B-VIII.0,
   0.995125 ± 0.001055, 2026-10-07): same geometry, data and seed, different
   statistics, so within 2σ combined. A larger gap is a defect in this
   runner, not physics.
2. **Fully inserted:** k about 0.84-0.86; total bank worth
   **17 ± 2 % Δk/k**, i.e. a little below INET's MCNP 19.36 % and close to
   VSOP 18.27 %. Reasons: the 2026-10-05 preview (1000 x [10 + 30], same
   model) gave 16.0 ± 1.0 %; explicit 5-segment rods with joints and the
   B4C self-shielded in its own ring cannot be worth more than a smear.
   Sign of the known differences, NOT applied as corrections:
   - our bed is 2.4 cm shorter than B4's 126 cm (16 681 balls against
     17 293-17 294, p. 252): a smaller core leaks more into the side
     reflector, so a slightly HIGHER worth. Size from TECDOC's own VSOP trend
     (B41 18.27 % at 126 cm, B31 15.24 % for the 180 cm full core) is about
     +0.13 % Δk/k, below this run's resolution;
   - ENDF/B-VIII.0 against ENDF/B-V: sign not predicted.
3. **Shape: an S-curve in `z_T`.** Little worth while the lower end is in the
   top reflector and cavity (119.2 to 228.2), steep through the bed (228.2 to
   351.8), flat again below the bed. As fractions of the total:
   - 171.2 cm: < 10 %;
   - 230.318 cm (lower end just below the bed top): 5-20 %;
   - 279.018 / 282.618 cm: 35-55 %;
   - 331.318 / 334.918 cm: 75-92 %;
   - 383.618 cm: > 95 %; 394.2 cm: 100 % by definition.
   For reference only (one rod, VSOP, not comparable in size): Table 4-9's
   single-rod fractions are 16, 38, 40, 78, 80, 99 % at the same positions.
4. **The 3.6 cm pairs (279.018 / 282.618 and 331.318 / 334.918)** move a
   36 mm steel joint: their difference is below this run's σ. Predicted
   unresolved, i.e. within 2σ of each other.
5. **Lost particles: 0** in every case; Shannon entropy flat after the
   inactive cycles.

## Addendum, 2026-10-23, before any single-rod run: rod 3 ("S3") alone

Added after the maintainer approved the per-rod model (gh:#1025). Written
before any single-rod transport; the only result known at writing is the
bank's withdrawn point (0.99449 ± 0.00258, which is also the single-rod
reference: all ten withdrawn is one configuration).

**The case.** Rod 3 (the channel at 81°) alone, the other nine withdrawn;
"S3" is a label under the model's symmetry, not a source's placement. Same
model, loading (N = 12, 16 681 balls) and data as the bank.

**References, separate cases:**

- **Experiment** (TECDOC-1382 § 4.2.1.5, printed p. 252): rod S3, 17 000
  balls (123.86 cm), lower end 171.2 → 394.2 cm, air: integral worth
  **1.4693 %** (p. 252); the same document prints **1.4369 %** for the same
  quantity on p. 253. Both are quoted; neither is chosen.
- **Fig. 4.13** (printed p. 252, part-2 PDF p. 26), the measured curve: AI digitisation on develop
  (`crates/nee_soon/verification_and_validation/htr10_control_rod_worth_tecdoc1382_fig4_13/`),
  `marker` rows only (the `trace` rows are the drawn curve, not
  measurements). Elevation = 394.2 − `z_T`; worth zero at `z_T` 171.2.
- **Calculation at the experiment's travel** (p. 253): INET's
  1.448 % for 171.2 → 394.2 cm (initial core, 126 cm).
- **B42** (one rod, 119.2 → 394.2 cm, 126 cm loading): VSOP 1.619 %, MCNP
  1.793 % (original, Table 4-8, p. 248); VSOP differential values Table 4-9
  (original, p. 248).

**Predictions.**

1. Integral worth 171.2 → 394.2 cm: **1.5 ± 0.4 % Δk/k**. Reasons: one rod
   of ten in an annulus of ten interacting rods is worth more than a tenth of
   the bank (shadowing), and TECDOC's own calculations and the measurement
   all sit at 1.4-1.8 %; our bank-to-B41 comparison (pending) will tell
   whether the model runs high or low. Our core is 319 balls smaller than the
   experiment's (sign: slightly MORE worth).
2. Integral worth 119.2 → 394.2 cm (B42's travel): 0.02-0.1 % above the
   171.2 → 394.2 value (VSOP: 1.619 against INET's 1.448 at 171.2, i.e. the
   top 52 cm of travel is worth ~0.17 %, an upper bound since our rod's
   lowest 52 cm then still sit in the top reflector and cavity).
3. Shape: the same S-curve in `z_T` as the bank, steepest across the bed.
4. **Resolution, stated now:** a single-rod worth of ~1.5 % from two
   independent runs needs σ_k ≈ 150 pcm per end point for a ±15 % integral.
   Points 3.6 cm apart (the B42 pairs) and the shape near the ends will NOT
   be resolved at the statistics affordable here; such differences will be
   reported as unresolved, not read.

## Source-convergence criterion, stated 2026-10-23 before the probe traces were read

After the S3 run at `z_T` 171.2 cm ended with a first-to-last entropy of
5.512 → 5.462 bits (only those two values were kept), a probe records every
cycle (2000 × [20 + 60], S3 at 394.2 and 171.2 cm, bank fully in). The
inactive count for the re-runs is chosen from the traces by this rule, fixed
now, before any trace is seen:

- reference band: the mean and standard deviation σ_H of the single-cycle
  entropy over the probe's last 40 cycles;
- converged at cycle c: the 5-cycle moving average of the entropy is inside
  mean ± 2σ_H at c and at every later cycle;
- n_inactive for a case = max(20, 2 c), rounded up to a multiple of 10 (a
  factor 2 margin, because the probe is one seed);
- the active cycles are then checked for a trend: the entropy of the first
  and second halves of the active cycles must agree within 3 standard errors.
  A trend is reported, not cut away.

The criterion is on the entropy (the source shape), never on k.
