# HTR-10 k_eff on a DEM random bed at the lattice's ball count, 10 000 × [5 + 135] (2026-10-08)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** One pour, one seed. Research, education
> and V&V only; not for any operational use. AI-assisted run and write-up
> (Claude Opus 5.5 in Claude Code), not yet reviewed by the maintainer.

**Generated:** the run went from 2026-10-07 20:47 to 21:56 UTC, which is
2026-10-08 04:47 to 05:56 SGT (`logs/start_utc.txt`, `logs/end_utc.txt`).
**Commit:** `713cd5bb60`, a worktree branch off `develop` at `ac0adb5084`.
**Approved as:** option A of gh:#787, 2026-10-08.

**Files:**
- `logs/run_dem_e8_N12.log`: the run log.
- `logs/smoke_1000x2p3.log`: the smoke run.
- [`PREDICTION.md`](PREDICTION.md): written and committed before either run.
- [`RUN_PARAMETERS.md`](RUN_PARAMETERS.md): everything needed to rebuild the run.
- `run.sh`: the exact launch.

## Headline

| | k ± 1σ | against RMC (0.999419) | against the lattice |
|---|---|---|---|
| **DEM random bed**, 16 681 balls (this record) | **0.989293 ± 0.000962** | **−1013 ± 96 pcm (−10.5σ)** | **−583 ± 143 pcm (−4.1σ)** |
| Şeker lattice, N = 12, 16 681 balls ([2026-10-07 record](../htr10_seker_2026_10_07_10k/README.md)) | 0.995125 ± 0.001055 | −429 ± 106 pcm (−4.1σ) | — |
| prediction ([`PREDICTION.md`](PREDICTION.md)) | 0.9960 | about −330 pcm | +100 pcm (range −200 to +400) |

**The prediction failed.** The sign is wrong, and the size is outside the
predicted range: −583 pcm against +100 (−200 to +400). By the prediction's own
criterion, |Δk| > 450 pcm is a surprise. **On one pour, the random bed sits
583 pcm below the lattice at equal ball count, resolved at 4.1σ of within-run
statistics.** It also falls outside the crate's ±1000 pcm band against RMC,
at −1013 ± 96 pcm, where the lattice sits inside it.

## Methodology

- **Computed:** k_eff of the HTR-10 first-criticality core with its pebble
  bed replaced by a DEM pour. Everything else is the model of the 2026-10-07
  lattice record.
- **Bed:**
  - **Source:** the gh:#216 `GranularSystem` bed at µ = 0.1, µ_r = 0
    (`reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv`, 27 554
    pebbles). It was settled by the LIGGGHTS port into the published conus
    and verified byte-identical on a re-run on 2026-10-07
    (`crates/dhoby-ghaut/verification_and_validation/htr10_dem_bed_bake/`).
  - **Cut:** to the lattice's **16 681** balls above the floor at N = 12.
    Every pebble at or below the floor is kept, plus the lowest 16 681 above
    it (`explicit_bed::trim_to_core_balls`). This is the web beds view's
    rule, and a test asserts the two cuts select the same balls.
  - **Identity:** `paper_fuel_assignment`, 57:43, giving 9 508 fuel balls.
  - **Build:** `assemble_explicit_triso_from_centres`. The geometry is drawn
    in [`../htr10_dem_bed_images/`](../htr10_dem_bed_images/README.md); those
    images are for the 16 890-ball cut, built by the same code.
- **Driver:** the record's own `examples/htr10_rmc_keff.rs`, through the new
  `OUTRAM_HTR10_DEM_BED` knob. Data, materials, the bounded majorant (#589),
  the source box and entropy mesh (sized from this bed), and the
  equal-ball-count reference are all unchanged.
- **Statistics and physics:**
  - 10 000 × [5 + 135], seed 20260917, ENDF/B-VIII.0 with 30P graphite;
  - URR, DBRC and S(α,β) on as defaults, every ablation off;
  - 5 threads; within-run 1σ.
- **Reference:**
  - **RMC:** Li, Yu & Wei (2014), ENDF/B-VII.0, read at 122.091 cm, where
    Şeker's model holds 16 681 balls: **0.999419**.
  - **The lattice point:** the 2026-10-07 record at N = 12 on VIII.0. **It was
    not re-run** on this commit. The only transport-side change since
    `dbb9e26e` is `7ee55f283a`, pinned bit-identical.
- **Pass criterion:** none was set for the DEM bed. The crate's band against
  RMC is ±1000 pcm. The question asked was the size of Δk against the lattice
  and whether it resolves.

## Results

### k and the comparisons

From `logs/run_dem_e8_N12.log`:

```text
k_eff        = 0.989293 +/- 0.000962
dk height-matched = -1013 pcm   (against 0.999419, not 1.004288)
GAUGE (not a reference): MCNP at this height, Table 3 0.998250 (-896 pcm), Table 4 0.999476 (-1018 pcm)
histories    = 1400000 (planned 1400000)
lost locate  = 0   stuck events = 0   neg distance = 0
leak vacuum  = 205 (0.015 %)   [lattice N = 12: 211, 0.015 %]
collisions   = 315.97 per history   [lattice: 313.73]
virtual coll = 12 668 341 782 = 9 049 per history   [lattice: 8 926]
```

- **Against the lattice:** Δk = 0.989293 − 0.995125 = **−583 pcm**. The σ of
  the difference is √(96.2² + 105.5²) = 143 pcm, so this is **−4.1σ**.
- **Against RMC:** −1013 ± 96 pcm (−10.5σ). The lattice's −429 ± 106 pcm was
  inside the ±1000 band; this point is 13 pcm outside it.
- **Against the MCNP gauges** (ENDF/B-VI, an independent model; not a
  reference): −896 pcm from Table 3 and −1018 pcm from Table 4.
- **Against the smoke run:** the smoke's 0.995483 ± 0.007653 at 1000 × [2 + 3]
  agrees (+0.8σ).

### Source convergence

- **Shannon entropy**, printed at generations 0, 11, 22, …, 132 and 139
  (4 × 4 × 4 mesh over the fissile region, ceiling 6 bits): 5.596, 5.599, 5.575, 5.595, 5.572, 5.602, 5.584,
  5.606, 5.576, 5.596, 5.604, 5.589, 5.601, 5.589. That is flat within ±0.017
  bits from generation 0, with no trend.
- **The lattice's trace** on its own mesh was 5.580 → 5.529, in a 5.516–5.580
  band. The two meshes are sized from different beds, so the absolute values
  are not comparable.
- **The first five generations:** k = 0.969, 0.992, 0.982, 0.977, 0.992.
- **Limit:** the trace was printed, not stored, at every 11th generation. A
  slow drift between the printed points cannot be excluded from this log
  alone. The lattice record has the same limitation.

### Cost

- **Nuclear data:** 89.5 s.
- **Transport:** **4 034.6 s** for 1.4 M histories on 5 threads, which is
  **2.9 ms wall and 14.4 ms CPU per history**, or 5.6 CPU-h. Total wall
  1 h 09 min.
- **Against the prediction:** 1.6–4.7 h was predicted, and revised to 1.2 h
  after the smoke run (14.4 ms against the smoke's 15.5 ms per history). The
  revised figure held. The first estimate was too high because it rested on
  the slower, shared Xeon.
- **Host:** i9-13900K, shared with other agents' jobs, load average 4–16.

## Interpretation

- **What is measured:** on this one pour, at equal ball count, equal fuel
  count and equal whole-core φ (0.5994 against 0.5999), the core built on the
  DEM bed is **583 ± 143 pcm less reactive** than the core built on Şeker's
  lattice. The difference is resolved at 4.1σ of within-run statistics.
- **What is not measured: pour-to-pour scatter.** One pour is one
  arrangement. The 583 pcm is (packing effect) + (this realisation's
  deviation from it) + (the non-packing differences below). This run cannot
  separate them. **Option B of the approval, three pours at other seeds, is
  the natural follow-up. It was not run here.** Each new pour needs a DEM
  bake (about 6 min on 3 threads) and a run like this one (about 1.2 h on 5).
- **Non-packing differences,** listed in [`PREDICTION.md`](PREDICTION.md):
  - 154 more dummy balls in the conus;
  - a different tube, with an ordered filler below a 25 cm DEM stub;
  - a flat-cut top;
  - the lens split (1.3e-5 of the ball volume);
  - wall caps up to 0.032 cm.

  The conus term was predicted positive, so it cannot explain a negative
  shift unless the prediction of its sign was also wrong. The others were
  each expected to be worth under 50 pcm. None of this was measured
  separately.
- **Hypotheses, none tested:**
  1. Fuel pebbles shadow one another more in the random arrangement (a
     different pebble-level Dancoff factor at equal φ).
  2. The poured wall layering puts fuel at different radii from the lattice,
     changing how fuel couples to the side reflector.
  3. This realisation sits low within the pour-to-pour scatter.

  Option B tests the third. An ablation building the lattice's conus and
  tube under the DEM core would test the non-packing terms.
- **Against RMC,** the random bed moves the model **away** from the
  reference, from −429 to −1013 pcm. RMC and MCNP model the lattice, so this
  is the expected direction for a model that is no longer like for like with
  them. It says nothing on its own about which bed is closer to the reactor.
  The reactor's bed is random, but its first-criticality k is not the
  reference here.
- **The prediction was wrong, and is left as written.** In particular, the
  claim that the packing term at equal φ would be "under ~200 pcm in either
  direction" is refuted for this pour, unless the realisation scatter turns
  out to be large.

## What was not done

- No second pour (option B), no second seed, no ablation of the conus or tube
  difference.
- No re-run of the lattice point on this commit (forbidden by the brief).
- No new geometry images. The 16 890-ball images in
  `../htr10_dem_bed_images/` were built by the same code. This cut differs
  only in stopping 209 balls lower, which the test above checks by count, not
  by eye.
