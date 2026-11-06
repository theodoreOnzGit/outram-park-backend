# HTR-10 k_eff against control-rod position (gh:#1025)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** One seed per point. Research, education
> and V&V only; not for facility operation, licensing or safety decisions.
> AI-assisted run (Claude Code), not yet reviewed by the maintainer.

Predictions were written and committed before any production run:
[`PREDICTION.md`](PREDICTION.md) (commit `3eb8b455a`).

## Methodology

**What is computed.** k_eff of the HTR-10 model of `nee_soon::htr10_rmc`,
exactly the 2026-10-07 k-vs-height record's
(`crates/nee_soon/verification_and_validation/htr10_seker_2026_10_07_10k/`),
with the ten explicit control rods (B4C rings, steel sleeves, iron joints;
TECDOC-1382 § 4.1.1.5, printed p. 236) moved:

- **bank:** all ten together (`Htr10CoreDesign::with_bank`);
- **single rod:** rod 3 alone (the channel at 81°, labelled "S3" by
  assumption: TECDOC Fig. 4.7, p. 234, names one rod S3 but no source places
  it in a channel; under the model's symmetry any channel stands in for it,
  maintainer 2026-10-23), the other nine withdrawn.

Positions are the rods' lower end `z_T` (TECDOC's axial coordinate, downward
from the model top): 119.2 withdrawn and 394.2 fully in (p. 236), 171.2 (the
measured calibration's start, p. 252) and the seven B42 positions (p. 243).
In this model the bed top is at `z_T` 228.2 cm and the foot of its
cylindrical part at 351.8 cm.

**Inputs.** 14 rings x 12 Şeker layers (bed 123.576 cm, 16 681 balls),
ENDF/B-VIII.0 at 300.15 K (27 °C), helium, URR and DBRC on, every bound
thermal law, real rod metal (`Htr10DataConfig::default()`,
`Htr10MaterialConfig::benchmark_default`). Driver `run_keff_csg_hybrid`
through `keff_vs_height::run_core` (bed delta-tracked on the bounded
majorant, everything else surface-tracked), source box and 4x4x4 entropy
mesh over the whole fissile region, seed 20260917, 4 pinned threads.
Runner: `crates/nee_soon/examples/htr10_rod_curve.rs`.

**Worth.** `rho = 1/k_in - 1/k_out` (positive for an insertion) in % Δk/k, σ by first-order propagation of
two independent runs. TECDOC does not state its worth formula in the B4
section; this reading is stated, not sourced.

**References, kept separate (never mixed):**

| case | what | source | kind |
|---|---|---|---|
| B41 | ten rods fully in, initial core 126 cm, 27 °C helium, ORIGINAL benchmark | TECDOC-1382 Table 4-8, p. 248: VSOP 18.27 %, MCNP 19.36 % | calculation (INET; MCNP-4A, ENDF/B-V, p. 245) |
| B41 deviated | same, dummy balls 1.84 g/cm³ / 0.125 ppm (p. 249) | Table 4-14, p. 250: VSOP 17.23 %, MCNP 18.28 % | calculation; NOT this model's dummy balls (1.73 g/cm³, 1.3 ppm), shown for scale only |
| B42 | one rod, initial core, VSOP, worth at the seven positions | Table 4-9, p. 248 (original); Table 4-15, p. 250 (deviated) | calculation |
| S3 experiment | one rod, 17 000 balls (123.86 cm), lower end 171.2 → 394.2 cm, air | p. 252: integral 1.4693 %; p. 253 prints 1.4369 % for the same quantity (the source disagrees with itself) | measurement |
| Fig. 4.13 | S3 measured worth curve | AI digitisation, `crates/nee_soon/verification_and_validation/htr10_control_rod_worth_tecdoc1382_fig4_13/` | digitised measurement (untrusted until reviewed) |
| Lebenhaft (MIT 2001) Table 5-8 | MCNP4B k against insertion | thesis, DSpace handle 1721.1/28288 | NOT READ: dspace.mit.edu is denied by this environment's network policy; no value used |

**Known differences from the references, with expected sign, NOT applied:**

- Loading: ours 16 681 balls; B4 126 cm = 17 293-17 294 balls (p. 252);
  the S3 experiment 17 000 balls (p. 252). A smaller core leaks more into the
  side reflector, so rods are worth slightly MORE in ours. Size from TECDOC's
  own VSOP trend (B41 18.27 % at 126 cm against B31 15.24 % for the full core)
  is about +0.13 % Δk/k for the bank.
- Atmosphere: the S3 experiment was in air; TECDOC p. 250 states the humid-air
  effect on rod worth was studied and found negligible.
- Nuclear data: ENDF/B-VIII.0 against ENDF/B-V (INET MCNP) and VSOP's own
  libraries; sign not predicted.

## Results (2026-10-23)

All runs: N = 12, seed 20260917, 4 pinned threads, 0 lost particles. σ is the
within-run standard deviation of the mean over the active cycles (one seed;
seed-to-seed scatter not in it). Worth σ: first-order, independent runs.

### Source convergence (checked, not assumed)

The first S3 run's first-to-last entropy moved 5.512 → 5.462 bits, and only
those two values had been kept. A criterion was then fixed in
[`PREDICTION.md`](PREDICTION.md) BEFORE any trace was read, and a traced
probe (2000 × [20 + 60]; [`convergence/`](convergence/)) applied it: the
single-cycle entropy scatter on the 4 × 4 × 4 mesh is 0.025-0.040 bits, the
5-cycle average sits inside the tail band from cycle 1 in all three probed
cases (S3 at 394.2 and 171.2 cm, bank fully in), so n_inactive stays 20;
active-half entropies agree (z = +1.75, +0.24, −0.98). The production S3
fully-in run, traced, gives z = +2.37. The 5.512 → 5.462 move was
single-cycle noise (~2σ). **Not checked:** a finer mesh; a 4 × 4 × 4 mesh may
not see an azimuthal tilt from one rod.

### B41: all ten rods (bank)

| rods | `z_T` (cm) | k | histories | wall (transport) |
|---|---|---|---|---|
| all withdrawn | 119.2 | 0.99449 ± 0.00258 | 240 000 | 2119 s |
| all fully in | 394.2 | 0.84624 ± 0.00285 | 240 000 | 1982 s |

(2000 × [20 + 100], `b41_bank_n12.csv`, commit `3eb8b455a`; data 582 s;
wall times inflated by concurrent builds on the 4-core host.)

- Withdrawn against the 2026-10-07 record's N = 12 point, 0.995125 ±
  0.001055: −64 ± 279 pcm. **Prediction 1 holds.**
- **Bank worth 17.62 ± 0.48 % Δk/k.** Against the ORIGINAL B41 (Table 4-8,
  printed p. 248 = part-2 PDF p. 22; calculations at 126 cm, 27 °C, helium):
  VSOP 18.27 % → ours −0.65 (−1.4σ); MCNP 19.36 % → ours −1.74 (−3.6σ).
  Deviated values (Table 4-14, p. 250): VSOP 17.23 %, MCNP 18.28 %, for
  scale only. **Prediction 2 (17 ± 2 %) holds.**
- The MCNP gap is resolved at 3.6σ and is reported, not fitted. Candidate
  causes, signs stated, none tested here: nuclear data (ENDF/B-VIII.0 vs
  INET's ENDF/B-V), our bed 612 balls smaller (would make ours HIGHER, so it
  does not explain a low value), INET's rod and channel modelling (not
  published in detail).

### S3: rod 3 alone, nine withdrawn

| rod 3 lower end `z_T` (cm) | k | histories | wall (transport) |
|---|---|---|---|
| 119.2 (= all withdrawn, the bank run above) | 0.99449 ± 0.00258 | 240 000 | 2119 s |
| 171.2 (the experiment's start) | 0.98974 ± 0.00167 | 540 000 | 3506 s |
| 394.2 (fully in) | 0.97907 ± 0.00169 | 540 000 | 2644 s |

(2000 × [20 + 250], `single_s3_n12.csv`; the 394.2 run with its cycle trace,
`single_s3_n12_cycles.csv`, after the convergence check above.)

| travel | ours (% Δk/k) | reference | ours − reference |
|---|---|---|---|
| 171.2 → 394.2 | **1.10 ± 0.25** | MEASURED, rod S3, 17 000 balls, air: 1.4693 % (printed p. 252 = PDF p. 26) | −0.37 (−1.5σ) |
| | | same quantity as printed on p. 253 (PDF p. 27): 1.4369 % | −0.34 (−1.4σ) |
| | | INET calculation for this travel, 126 cm: 1.448 % (p. 253) | −0.35 (−1.4σ) |
| 119.2 → 394.2 (B42's travel) | **1.58 ± 0.32** | Table 4-8 (p. 248), original: VSOP 1.619 % | −0.04 (−0.1σ) |
| | | MCNP 1.793 % | −0.21 (−0.7σ) |
| 119.2 → 171.2 | 0.48 ± 0.31 | INET implied 1.619 − 1.448 = 0.17 % | not resolved |

- **Prediction (1.5 ± 0.4 % for 171.2 → 394.2): ours 1.10 ± 0.25 sits at
  its lower edge.** The measured value is 1.5σ above ours: consistent, but
  these statistics (±22 % on the worth) cannot say more. A 3σ statement
  either way needs about 4× the histories per end point (~4 h on this host).
- **Fig. 4.13** (the measured curve, AI-digitised on develop,
  `crates/nee_soon/verification_and_validation/htr10_control_rod_worth_tecdoc1382_fig4_13/`):
  only the integral can be compared, since only the ends were run. Its
  markers nearest full insertion (elevation 2.7 and 19.9 cm, i.e. `z_T` 391.5
  and 374.3 cm) read 1.462 and 1.451 %, the same as the printed integral.
- Known differences, NOT corrected for: 319 balls fewer than the experiment
  (sign: ours slightly HIGH); helium for the experiment's air (TECDOC p. 250:
  negligible for rod worth); "S3" is rod 3 by assumption (no source places
  it); ENDF/B-VIII.0.

### The bank curve (all ten rods together)

Interior points 2000 × [20 + 60] (`curve_bank_n12.csv`, every cycle in
`curve_bank_n12_cycles.csv`; entropy criterion met at every point, active
halves within |z| ≤ 1.34), ends from B41 above. Worth against all withdrawn;
fraction = worth / total. Transport 700-798 s per interior point
(uncontended, ~200 histories/s on 4 threads).

| `z_T` (cm) | k | worth (% Δk/k) | fraction of total | predicted fraction | VSOP one-rod fraction, Table 4-9 (p. 248), for shape only |
|---|---|---|---|---|---|
| 119.2 | 0.99449 ± 0.00258 | 0 | 0 | 0 | 0 |
| 171.2 | 0.98354 ± 0.00354 | 1.12 ± 0.45 | 0.064 | < 0.10 ✓ | (not tabulated) |
| 230.318 | 0.96591 ± 0.00318 | 2.98 ± 0.43 | 0.169 | 0.05-0.20 ✓ | 0.158 |
| 279.018 | 0.92679 ± 0.00354 | 7.34 ± 0.49 | 0.417 | 0.35-0.55 ✓ | 0.377 |
| 282.618 | 0.92497 ± 0.00333 | 7.56 ± 0.47 | 0.429 | 0.35-0.55 ✓ | 0.401 |
| 331.318 | 0.86651 ± 0.00349 | 14.85 ± 0.53 | 0.843 | 0.75-0.92 ✓ | 0.782 |
| 334.918 | 0.86989 ± 0.00360 | 14.40 ± 0.54 | 0.818 | 0.75-0.92 ✓ | 0.804 |
| 383.618 | 0.84968 ± 0.00358 | 17.14 ± 0.56 | 0.973 | > 0.95 ✓ | 0.994 |
| 394.2 | 0.84624 ± 0.00285 | 17.62 ± 0.48 | 1 | 1 | 1 |

- **Shape (prediction 3): an S-curve, as predicted**, steepest between the
  bed top (228.2 cm) and the bed's foot (351.8 cm); every point in its
  predicted band. The VSOP one-rod fractions are a different configuration
  (one rod, 126 cm) and are shown only to place the shape.
- **The 3.6 cm pairs (prediction 4): unresolved, as predicted.**
  279.018 → 282.618: +0.22 ± 0.68 %; 331.318 → 334.918: −0.45 ± 0.76 %
  (k reads higher further in, inside 1σ). Their differences are not read.
- No point is fitted, smoothed or dropped.

## What this does NOT establish

- **Not a validation of the bank against measurement:** B41 is a
  code-to-code comparison; TECDOC reports no measured ten-rod worth.
- **S3 against the measurement at ±22 %:** 1.5σ low; not resolved.
- **The Lebenhaft (MIT 2001) comparison** the maintainer asked for first was
  not made: the thesis could not be read (network policy), and its second-hand
  values were not used.
- **One seed per point**; a 4 × 4 × 4 entropy mesh only.
- The B41/B42 loadings (126 cm) and the experiment's (17 000 balls) are not
  this model's N = 12 bed (16 681 balls); the lattice cannot build them
  exactly. Expected sign stated above, not applied.

## Reproduce

```bash
# B41 ends (commit 3eb8b455a)
OUTRAM_ROD_CASES="12:119.2;12:394.2" OUTRAM_HTR10_HISTORIES=2000 OUTRAM_HTR10_INACTIVE=20 \
  OUTRAM_HTR10_ACTIVE=100 OUTRAM_HTR10_THREADS=4 OUTRAM_ROD_CSV=b41_bank_n12.csv \
  cargo run --release -p nee_soon --example htr10_rod_curve
# S3: add OUTRAM_ROD_SINGLE=3, cases 12:171.2;12:394.2, ACTIVE=250, CSV single_s3_n12.csv
# bank interior: cases 12:171.2;12:230.318;...;12:383.618, ACTIVE=60, CSV curve_bank_n12.csv
# convergence: python3 convergence/conv.py <csv>_cycles.csv
# review images: cargo run --release -p nee_soon --example htr10_rod_images
```

The logs, the commits each batch ran on (`logs/commit_*.txt`) and the start
and end times are in `logs/`. The web demo reads `b41_bank_n12.csv`,
`curve_bank_n12.csv` and `single_s3_n12.csv` at build time
(`examples/monte_carlo_web/htr10/rods.rs`).

## Geometry review images

[`geometry/`](geometry/): the drawing rule's slices of the assembled core
(bank out / half / in; rod 3 alone at 171.2, 282.618, 394.2 cm), y-z through
rod 3's axis and x-y at bed mid-height. Checked by eye: rod 3 at (16.0,
100.8) cm; its lower end at z = 290.03 − `z_T`; B4C sections, joints and
steel sleeves in place; at bed mid-height only rod 3's ring when it alone is
in. Not checked: the other nine rods one by one (their channels are pinned
by `reflector_geometry`'s tests; the per-rod placement by
`the_bank_is_ten_equal_rods`).

## Web demo

`?rung=htr10&mode=watch&view=rods` ("Control rods: k vs rod position"):
screenshots at 390 × 844 and 1280 × 900 were checked with Playwright
(Chromium 1194, software rendering). No-lag check (`lagcheck`): with the
worker pool processing the core's data, page frame intervals had a median of
200-230 ms against 150-200 ms for the same page with no pool, on the same
fully loaded 4-core host; every slider drag reached the page title within
0.1-0.6 s; the longest frame gap was 0.72 s. The UI thread was not blocked
by the workers; the absolute frame rate is this headless host's.

