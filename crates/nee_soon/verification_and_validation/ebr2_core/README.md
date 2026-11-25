# Simplified EBR-II full core, fresh fuel: geometry and k_eff (gh:#1762)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** A SIMPLIFIED FRESH CORE, not the Run
> 138B burnup state; its k is not a prediction of EBR-II's critical state.
> Research, education and V&V only; not for facility operation, licensing or
> safety decisions. AI-assisted model, run and write-up (Claude Code), not
> reviewed by the maintainer.

**Files:**
- [`PREDICTION.md`](PREDICTION.md): the prediction, committed before the run
  (6d603be57).
- `ebr2_core_*.png`: the assembled geometry, drawn by
  `examples/ebr2_core_images.rs`.
- `keff_default.log` and `.err`: the run.

## Methodology

- **Model:** `nee_soon::ebr2_core::Study::default()`. The module docs give
  every input, its source and every assumption. In short:
  - Run 138B's 637 positions, from the gh:#1313 transcription;
  - a CSG hex lattice at the 5.8929 cm pitch;
  - each subassembly homogenised at ARC-228 Table 3.3's volume fractions;
  - fresh U-5Fs at 67 wt% U-235, 316SS and sodium, built as the fresh driver
    builds them (arm A);
  - the control rods' fuel 21.06 cm below the drivers', with B4C above it;
  - 293.15 K everywhere.
- **Data:** ENDF/B-VIII.0 (41 nuclides), processed by the workspace's NJOY
  port at 293.15 K, with the library's defaults (URR, RVS).
- **Run:** surface tracking, 4000 × [20 + 80], seed 1, every thread of a
  4-vCPU container (`--keff 4000 20 80 1`).
- **Pass criterion:** none. The result is reported against the prediction.

## Geometry check (the drawing rule)

`ebr2_core_xy_midplane.png`, `ebr2_core_xy_rows1to7.png`, `ebr2_core_xz.png`
and `ebr2_core_xz_rods.png` are drawn from the assembled CSG.

**What I checked in them:**
- **Rows 1-7:** drivers, the half-worth drivers (ring 1 and three in ring 2,
  among others) and the SST dummies in ring 2.
- **Rods:** seven high-worth control rods in row 5, showing B4C at the
  mid-plane (their fuel has dropped 21.06 cm); the two safety rods in row 3
  and XX09 as rod fuel.
- **Outer rings:** the reflector rows; the blanket from about row 11 out,
  with its scattered reflector positions; sodium in the truncated corners
  inside the 96 cm cylinder.
- **x-z section:** 34.29 cm of driver fuel, and the rod and safety-rod fuel
  sitting lower.

**What I could not check:** that the orientation matches ARC-228 Fig. 3.1
sector by sector. I compared the pattern by eye only.

The tests pin the rest:
- `ebr2_core::tests::the_table_is_the_transcription` pins the table to the
  CSV and the counts: 87 fuelled drivers, 8 control rods, 2 safety rods, 330
  blanket subassemblies.
- `every_position_has_its_own_tile` pins each position to its own lattice
  tile, at its centre.

## Results

| Run | k | Wall time |
|---|---|---|
| default study, 4000 × [20 + 80], seed 1 | **1.09526 ± 0.00203** | 421 s (data and transport) |

**Against the prediction** (k = 1.08, range 1.02 to 1.15): **inside the
range**, 1.5 % above the point value.

**Interpretation:** the core is supercritical by about 9 % Δk, about what
fresh fuel against a partly burnt critical core leads one to expect. The
number carries the assumptions, though: the half-worth fuel, the B4C
volume, the reflector make-up and the axial centring are each guesses worth a
few percent. It is quoted as this model's k, not as EBR-II's.

**Which number to quote:** this one run, with its σ. No seed study has been
made; σ is the run's own standard error.

## Known gaps

- Burnup: the ARC-228 Appendix B/C compositions are not held (gh:#1313 A6).
- Homogenised subassemblies, with no pins.
- The assumed fractions and make-ups listed in the module docs.
- No thermal expansion.
- One seed.
