# Tutorial rung 5: TRISO double heterogeneity, delta tracking, DH shortcuts

**Class: verification**, not validation. Both studies here compare one
treatment against another inside this code (explicit kernels against the same
atoms homogenised; approximate double-heterogeneity treatments against exact
delta tracking). No experiment corresponds to either problem. AI-assisted
record under `RESPONSIBLE_USE.md`; no human V&V sign-off. Education and
research only.

Tutorial rung 5 (GitHub #528, epic #520). Lesson page:
`crates/outram-mc-libs/docs/tutorial/src/triso.md`.

## Predictions (written 2026-10-05, before any run of this record)

Committed before the runs below were started, so that they predate the
results. Not to be edited after the results are in; a miss is reported as a
miss.

### P1. HTR-10 fuel-zone k_inf, ENDF route (`examples/htr10_fuel_zone_kinf.rs`)

The old record (2026-09-11) is LOW tier (embedded WMP + 10-group fast
fallback, Watt stand-in) with **free-gas** graphite: heterogeneous
1.59671 ± 0.00897, homogenised 1.45028 ± 0.00747, hom − het =
−14 643 ± 1167 pcm. The re-run changes the data to ENDF/B-VIII.0 read
directly (RECONR + BROADR, URR probability tables and DBRC on by default),
adds C-13 at its natural abundance, and puts the bound-graphite S(alpha,beta)
law (30 % porous reactor graphite, the HTR-10 choice in
`nee_soon::htr10_rmc::GraphiteLaw`) on the matrix carbon.

- **Sign (firm):** homogenised below heterogeneous, hom − het < 0. Lumping
  the kernels self-shields the U-238 resonances; this is the physics the
  example's gate exists for.
- **Magnitude of the difference:** same order as before, **−10 000 to
  −20 000 pcm**, central guess about **−16 000 pcm**, i.e. slightly larger in
  magnitude than LOW's −14 643. Reasoning: `examples/dh_keff_vv.rs` records
  that the LOW tier *understated* the cost of homogenisation by ~700-800 pcm
  on the FHR cell, because its coarse data had already smeared part of the
  U-238 resonance structure that homogenisation destroys. I expect the change
  to be inside about 1.5 sigma of the old number at the statistics run here,
  so not resolved.
- **Absolute k_inf (low confidence):** both arms lower than LOW's by
  **1000-4000 pcm**. Reasons: the bound-graphite law instead of free gas
  (this crate's own ~1700 pcm free-gas overestimate on a graphite spectrum,
  quoted in `dh_keff_vv.rs`), and the LOW tier's Watt/flat-nu stand-ins
  (about +500 pcm on Godiva). Sign stated, magnitude not trusted.

### P2. DH shortcuts on the FHR unit cell (`examples/dh_keff_vv.rs`)

Recorded 2026-09-14 (7200 × [15 + 40], single thread, this machine class):
CLS −3267, SCLS −3365, naive −4302, ring-RPT +37 pcm against delta tracking;
re-measured 2026-10-02 (delta, naive, ring-RPT only, i9-13900K): naive −4136,
ring-RPT +239. Since 09-14 the URR/DBRC defaults (2026-09-20), the SCLS
history-reset fix (2026-09-18) and the OpenMC-parity audit (2026-09-30) have
landed.

- **Delta tracking (exact arm):** within 2 sigma of the 2026-10-02 value
  1.38155 ± 0.00222 (no physics change since then that I know of).
- **Naive homogenisation:** a resolved negative bias, **−3800 to −4500 pcm**.
- **Ring-RPT:** not resolved from zero (|dk| < 3 sigma, expected within
  ±600 pcm).
- **CLS (whole-particle):** a resolved negative bias near the old value,
  **−2800 to −3800 pcm**. The bias is geometric (what the sampler is applied
  to), and the physics-default changes act on every arm alike.
- **SCLS (whole-particle):** within statistics of CLS or more negative;
  `docs/cls-scls-vv.md` (2026-09-18, 800 histories) had −4251 ± ~1000 pcm
  after the reset fix. Expect **−3000 to −4500 pcm**.
- **Kernel-level CLS / SCLS** (if run): not resolved from zero, as on
  2026-09-18.
- **Speed ratios** on this machine (2.1 GHz Xeon, the 09-14 hardware class):
  CLS and ring-RPT faster than delta by **1.5-2.5x**, naive **1.4-2x**; SCLS
  **slower than delta** (0.05-0.2x), because since the 2026-09-18 reset fix it
  measured 0.07x. Absolute seconds not predicted.

## Results

(To be filled in from the runs; see below.)
