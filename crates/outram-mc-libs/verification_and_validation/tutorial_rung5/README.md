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

### R1. HTR-10 fuel-zone k_inf, ENDF route (2026-10-05)

**Methodology.**

- **Example:** `examples/htr10_fuel_zone_kinf.rs`, default route.
- **Data:** ENDF/B-VIII.0 read directly through RECONR and BROADR at 293.15 K, tolerance 1e-3.
  - URR probability tables and DBRC are on, applied by the constructor.
  - Carbon is C-12 and C-13 at 98.93 / 1.07 at.%.
  - The graphite S(alpha,beta) law is `tsl-reactor-graphite-30P.endf` (MAT 32), on both carbon isotopes. Its 296 K table is used, which is within NJOY's `T/1000 + 5` K tolerance.
  - The UO2 kernel is free gas.
- **Geometry:**
  - Atom densities from IAEA-TECDOC-1382 Table 4-38.
  - A reflective cube of half-width 1 cm holding 1018 RSA-packed kernels with r = 0.025 cm. Packing seed 20260811; realised f = 0.008328.
  - The homogenised case uses the identical inventory.
- **Transport:** 10 000 histories × [50 inactive + 200 active] per case, RNG seed 1, single-thread backend.
- **Majorant:** tabulated on the union of every nuclide's own grid plus a 131 073-point log backbone, with a 10 % margin.
  - It is audited at 2 000 001 log-spaced energies plus every grid node (2 241 288 energies in all).
  - The audit result: worst `Sigma_t/Sigma_maj` = **0.9091** (heterogeneous) and **0.9092** (homogenised), so it is bounded everywhere it was checked.
- **Pass criterion:** the example's gate, hom − het < 0 at more than 3 sigma. No absolute k is gated, because no reference exists for this problem.
- **Build and hardware:**
  - Built from `5eb40da10`.
  - Intel Xeon @ 2.10 GHz (KVM, 4 vCPU, 260 MiB L3, 15 GiB), pinned to one core with `taskset -c 2`. The machine was shared with another agent's work on cores 0-1.
  - Linux 6.18.44, rustc 1.95.0, `--release`.

Command:

```text
THREADS=1 taskset -c 2 target/release/examples/htr10_fuel_zone_kinf 10000 50 200
```

**Results.**

| case | k_inf | transport time |
|---|---|---|
| heterogeneous (kernels explicit) | **1.57136 ± 0.00088** | 703.6 s |
| homogenised (same atoms) | **1.44684 ± 0.00090** | 760.9 s |
| hom − het (k) | **−12 452 ± 126 pcm** (98.7σ) | |
| hom − het (rho) | −5477 ± 56 pcm | |

The previous record, 2026-09-11, used the LOW tier with free-gas graphite and fewer histories:

- het 1.59671 ± 0.00897;
- hom 1.45028 ± 0.00747;
- ~~−14 643 ± 1167 pcm~~, now superseded.

**Against P1.**

- **Sign: held.**
- **Magnitude: inside** the predicted −10 000 to −20 000 pcm.
  - The prediction said "slightly larger in magnitude than LOW". The result is **smaller**, by +2191 ± 1174 pcm (1.9σ), which is **not resolved**.
  - The direction of my reasoning (LOW understates homogenisation) is not supported here.
- **Absolute k_inf:**
  - Heterogeneous: −2535 ± 901 pcm from LOW. Predicted 1000-4000 pcm lower: **held**.
  - Homogenised: −344 ± 752 pcm from LOW. Predicted 1000-4000 pcm lower: **missed**, because the change is not resolved from zero.
  - Why the two arms moved differently was not investigated. The data route, the thermal law and the majorant all changed at once, so the move is not decomposed.

### R2. The majorant the example used before was not a bound (2026-10-05)

The first pilot (200 × [5 + 10]) stopped on the new audit:

- `Majorant::bounding(.., 1e-4, 2e7, 4096, 32, 0.1)`, the construction the example had used, exceeded `Sigma_t` by 18 %.
- Re-measured with the ablation `OUTRAM_HTR10_MAJORANT=bounding` on the same binary (`htr10_fuel_zone_kinf 50 1 2`):
  - heterogeneous: worst `Sigma_t/Sigma_maj` = **1.1839 at 1.6893 MeV, in the UO2 kernel**, which is an under-bound;
  - homogenised: 0.9411, which bounds.
- The bin sampling step at 1.69 MeV is about 340 eV, so a narrow resonance in the kernel's total cross section near 1.69 MeV falls between samples. Which nuclide carries it was not isolated.
- **Not measured:** how much k the under-bound moved. Its k at 50 histories is not a result.
- The fix changed the protocol, not the margin: the majorant is tabulated on the data's own grid nodes, where linear-linear data attains its maximum. See `build_majorant` in the example.
- The same construction at margin 0.3, as `DhUniverse::keff` uses it, was audited on the FHR unit cell of `dh_keff_vv.rs` (`OUTRAM_DH_VV_AUDIT=1`; each material against its own majorant; 200 001 log energies plus every grid node). Worst ratio **0.8970** (SiC, 0.772 MeV): bounded.
- `examples/triso_delta_tracking.rs` and other users of `bounding` were **not audited**.

### R3. DH shortcuts (`examples/dh_keff_vv.rs`): handed over, partial only

On 2026-10-05 the maintainer handed this re-measurement to another session (#582). The P2 predictions above stand for that session. Partial runs here were stopped. What finished:

- **Settings:** 7200 × [15 + 40], single thread, `taskset` to one core, binary with the per-material audit.
- **delta tracking:** **1.38155 ± 0.00222** (126.0 s). This is identical in every digit to the 2026-10-02 i9 record, as expected for a fixed seed on the single-thread backend.
- **CLS (whole-particle):** **1.34536 ± 0.00247** (70.4 s, 1.79×). That is −3619 ± 332 pcm against delta (10.9σ), inside P2's predicted −2800 to −3800.
- **Not finished:** naive, ring-RPT, SCLS, both kernel-level arms. They are not quoted.
