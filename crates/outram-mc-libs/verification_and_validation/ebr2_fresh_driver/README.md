# Fresh EBR-II MARK-II driver subassembly: k-infinity and the nuclear-data gap ablation (gh #1329)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** AI draft, unreviewed. Research, education
> and V&V only; not for facility operation, licensing or safety decisions.

Epic #1312, brief #1317 item 7 (fresh core only, no new ENDF work), data
#1313, builder #1327. The first fresh-core EBR-II step: one fresh MARK-II
driver subassembly in an infinite lattice.

**Every input is an AI transcription, not yet reviewed by a human.** The
inputs, each with document, table and printed page, are in
`examples/common/ebr2_fresh_driver_model.rs`. The EBR-II reports (ANL-ARC-228
Rev. 1, ANL-ARC-226 Rev. 1, IAEA-TECDOC-1819) are in the maintainer's private
corpus and are cited under the data-policy exception for published EBR-II
benchmark numbers (`reference-data/ebr2/README.md`).

## Methodology

**Model** (`examples/common/ebr2_fresh_driver_model.rs`, built with
`model::HexSubassembly`):

- 91 sodium-bonded U-5Fs pins (6 rings): slug 3.3655 mm, cladding OD
  4.4196 mm, wall 0.305 mm, 316SS wire 1.24 mm with a 152.4 mm lead,
  smeared into the coolant by volume (ARC-228 Table 3.1.1 p.17; lead ARC-226
  Table 4.1.1 footnote p.49).
- **Cladding wall 0.305 mm, not ARC-226's 0.315 mm**: ARC-228's own
  Table 3.3 (p.23) gives clad ID 0.3810 / OD 0.4420 cm, a 0.305 mm wall, so
  0.305 is consistent within the neutronics specification.
- **Pin pitch is not printed**: assumed `D + s` = 5.6596 mm (wire touching),
  P/D 1.2806, consistent with IAEA-TECDOC-1819's "P/D=1.28" for the EBR-II
  driver (TABLE 34 footnote, p.249).
- 316SS duct 56.134 / 58.166 mm across flats, inside the sodium hexagon of
  the 58.929 mm subassembly pitch (ARC-228 Table 3.1.2 p.18). Reflective on
  every face: an infinite lattice, axially infinite.
- U-5Fs: U 95.0, Zr 0.10, Mo 2.46, Ru 1.96, Rh 0.28, Pd 0.19, Nb 0.01 wt%
  (ARC-228 Table 3.2.2 p.21), 67 wt% U-235 (Table 3.1.1 p.17), 18.2 g/cm³
  (ARC-226 §4.2.1 p.100, "at 300 K"). No U-234/U-236 (no fresh isotopics
  are given).
- **316SS: PNNL-15870 Rev. 1 material 300** (8.00 g/cm³; wt% C 0.041,
  Si 0.507, P 0.023, S 0.015, Cr 17.0, Mn 1.014, Fe 66.9, Ni 12.0, Mo 2.5).
  Neither EBR-II specification gives a steel (#1313 A6). Read from the
  machine-readable copy PyNE distributes (`pyne/dbgen/materials_compendium.csv`,
  fetched 2026-11-10), not the PDF: **no page is cited**. Sodium 0.971 g/cm³,
  material 287 of the same copy.
- **Isothermal 20 °C (293.15 K)**, the temperature ARC-228 Tables 3.1.1,
  3.1.2 and 3.3 give dimensions at. Densities and nuclear data at the same
  temperature; the sodium is solid at 20 °C. Not an operating state.
- Continuous-energy ENDF/B-VIII.0 processed in the run at 293.15 K with the
  correct-physics defaults: **URR probability tables and DBRC on** (the run
  records URR tables on every nuclide with an unresolved range, DBRC on every
  nuclide, and no ablation).

**Nuclear data.** Tapes are taken by name from `reference-data/endf/` and
`reference-data/ace/endf/endf-b-viii.0/`. **The brief's premise that Ni, Zr
and most Mo are missing holds only for `reference-data/endf/`**: the ACE
submodule's `endf/` folder already holds ENDF/B-VIII.0 Ni-58/60/61/62/64,
Zr-90/91/92/94/96 and Mo-92/94/95/96/97/98/100 (added 2026-10-01 and
2026-11-09). Nothing was fetched. The only elements with no tape anywhere
in `reference-data/` are **Ru, Rh, Pd and Nb** (2.44 wt% of the fuel). The
release Mo-95 tape is used in every arm (not the beta one in
`reference-data/endf/`), so arms differ only in composition.

**Gap treatments** (`GapTreatment::arms()`), each named; omission leaves the
atoms out and renormalises nothing:

| arm | treatment |
|---|---|
| A | reference: Ni, Mo, Zr natural; Ru, Rh, Pd, Nb omitted |
| B | brief (a), `reference-data/endf` only: Ni, Mo, Zr, Ru, Rh, Pd, Nb omitted |
| C | brief (b), `reference-data/endf` only: Mo-95 atom for atom for all Mo; Ni, Zr, Ru, Rh, Pd, Nb omitted |
| D | A, Ni omitted |
| E | A, Mo-95 atom for atom for all Mo |
| F | A, Mo omitted |
| G | A, Ru/Rh/Pd/Nb atoms (IUPAC 2021 weights) replaced by natural Mo atoms |

**Run.** `examples/ebr2_fresh_driver_kinf.rs -- 2000 20 60`: 2000 neutrons
per cycle, 20 inactive + 60 active cycles (120 000 active histories per
arm), seed 1329 in every arm, multithread backend on 3 threads. Every
nuclide processed once and shared by the arms (one temperature).
Differences are quoted with `sqrt(sA² + sX²)`: the arms share a seed but
diverge at the first collision with a changed nuclide, so this treats them
as independent, which is conservative if anything.

**Reference.** There is **no measured k_inf** of a fresh MARK-II driver, and
ARC-226, ARC-228 and IAEA-TECDOC-1819 print **no computed one** (their text
searched for "k-inf", "kinf", "infinite" on 2026-11-10; nothing). The result
is **reported uncompared**. It is not a criticality result for EBR-II.

**Predictions** (written on #1329 before any run, from a one-group
absorption budget with order-of-magnitude cross sections from memory, not
cited inputs): A k_inf 1.6-1.8; B +1500 to +3500 pcm; C +500 to +2500;
D +400 to +1500; E -500 to -1500; F +600 to +2000; G -500 to -1500.

## Geometry pictures (crate HARD RULE), from the assembled geometry

Drawn by the example with the ported OpenMC plotter (cell lookup per pixel),
arm A (the arms differ only in composition):

| file | shows | checked |
|---|---|---|
| `ebr2_fresh_driver_xy.png` | the whole lattice cell, z = 0 | 91 pins in 6 rings, rows parallel to the flats; slug / bond / clad visible in every pin; coolant (wire smeared) between; duct; a thin sodium band to the cell's edge; no pin touches the duct |
| `ebr2_fresh_driver_pin_xy.png` | a corner pin (2.830, 0) cm, its neighbours, the duct corner and the sodium between ducts | slug r 0.1683, clad 0.1905-0.2210 cm as titled; duct 0.1016 cm; gap sodium 0.0382 cm to the pitch hexagon |
| `ebr2_fresh_driver_xz.png` | x-z through y = 0, the centre row of 11 pins, 34.29 cm drawn | every pin runs the full height; one pair of end planes. **Axes not to scale** (7 cm by 36 cm in a square image) |

Not checkable in the pictures: the wire (smeared, so not drawn), and the
reflective boundaries (a boundary condition, not a region).

## Results (2026-11-10, release build)

### Geometry and composition (`tests/model_ebr2_fresh_driver.rs`, 3 passed)

| quantity | ARC-228 Table 3.3 | analytic | sampled (1 081 801 points) | tolerance (set before) |
|---|---|---|---|---|
| fuel alloy | 26.92 % | 26.918 % | 26.916 % | 0.10 points (analytic), 0.15 (sampled) |
| 316SS (clad + wire + duct) | 22.29 % | 22.297 % | 22.295 % | as above |
| sodium (bond + coolant + gap) | 50.79 % | 50.785 % | 50.789 % | as above |

The Table 3.3 fractions reproduce **only with the sodium between ducts
counted**: they are over the lattice cell at the subassembly pitch. This is
an independent check of the built geometry: the fractions are a separate
table from the dimensions. The predicted analytic values (26.93 / 22.33 /
50.74 %) were off by up to 0.04 points because they used Table 3.3's rounder
cm dimensions. **The first sampling grid aliased** on the duct's flats
(316SS 22.20 % at 1201², 22.39 % at 2003²); the sampler now offsets points
by a row-dependent irrational fraction and gives 22.295 / 22.299 %. No
tolerance was changed. Mass densities rebuilt from the atom densities:
U-5Fs 17.75592 g/cm³ in arm A (18.2 × 0.9756, nothing renormalised), 316SS
8.00000 g/cm³. U-235 2.968053e-2 atoms/b-cm in the fuel.

### k_inf and the gap ablation

2000 × [20 + 60] cycles, seed 1329, 293.15 K. σ is the standard error over
active cycles; batch-means σ is the correlation-aware one.

| arm | treatment | k_inf | σ | batch-means σ | minus A [pcm] | predicted [pcm] | verdict |
|---|---|---|---|---|---|---|---|
| A | reference (Ru, Rh, Pd, Nb omitted) | **2.01400** | 0.00278 | 0.00377 | - | k 1.6-1.8 | **missed**: k higher than predicted |
| B | brief (a): Ni, Mo, Zr, Ru, Rh, Pd, Nb omitted | 2.04107 | 0.00274 | 0.00231 | **+2707 ± 390** | +1500 to +3500 | in range, 6.9 σ |
| C | brief (b): Mo-95 for Mo; Ni, Zr, Ru, Rh, Pd, Nb omitted | 2.01960 | 0.00336 | 0.00420 | +560 ± 436 | +500 to +2500 | in range; 1.3 σ, not resolved from 0 |
| D | Ni omitted | 2.03168 | 0.00323 | 0.00354 | **+1768 ± 426** | +400 to +1500 | sign right, 0.6 σ above the range |
| E | Mo-95 for all Mo | 2.00942 | 0.00259 | 0.00293 | -458 ± 380 | -500 to -1500 | sign right; 1.2 σ, not resolved |
| F | Mo omitted | 2.02939 | 0.00263 | 0.00276 | **+1539 ± 382** | +600 to +2000 | in range, 4.0 σ |
| G | Ru/Rh/Pd/Nb as natural Mo | 2.00962 | 0.00306 | 0.00271 | -438 ± 413 | -500 to -1500 | sign right; 1.1 σ, not resolved |

**Timing** (hardware: CPU only, 4 cores, Linux, Intel Xeon @ 2.10 GHz,
15.7 GiB RAM, 3 threads used; **shared and loaded**: other agents' builds,
and this change's own gates during arms D-G, ran on the same machine).
ENDF processing 265 s for 39 nuclides (U-235 166 s, U-238 100 s); transport
429-780 s per arm for 160 000 histories, i.e. 2.7-4.9 ms of wall time per
history on 3 threads. A smoke run of arm A (1000 × [5 + 10]) gave
2.00110 ± 0.01146.

## Interpretation

- **Which numbers to quote.** The reference k_inf of the fresh driver
  subassembly, on all nuclear data in `reference-data/`, is **2.014 ± 0.004**
  (batch-means σ, the larger). It still omits Ru, Rh, Pd and Nb, and G
  bounds that omission at **at least about -440 pcm** (1.1 σ): natural Mo
  captures less than Ru, Rh and Pd, so their true worth is likely larger.
  Quote A with that caveat; B and C are what the brief's data would have
  given and are not results.
- **The brief's treatments would have biased k high**: (a) by +2700 ± 390
  pcm, resolved. (b) by +560 ± 440 pcm, not resolved; it lands near A only
  because the Mo-95 over-capture (E) and the missing nickel (D) roughly
  cancel. That cancellation is luck, not a property of the treatment.
- **Consistency of the arms.** D + F = +3307 ± 570 against B = +2707 ± 390
  (Zr's 0.10 wt% is the only other difference), 0.9 σ apart. E - F =
  -1997 ± 533 (Mo-95 instead of nothing) against C - B = -2147 ± 435, 0.2 σ
  apart. The arms tell one story.
- **Predictions.** The absolute k_inf was missed high (2.01 against 1.6-1.8):
  the one-group budget used a fast U-235 fission cross section and ν that
  were too low for this hard spectrum. Signs were right in every arm; the
  nickel worth came out above its predicted range by 0.6 σ, the Mo worth in
  range.
- **Resolution.** σ ≈ 300-400 pcm per arm, chosen to keep each arm near
  10 minutes on this machine. That resolves B, D and F, not C, E or G; those
  three need about 4× the histories for 2-3 σ.
- **What this is not.** Not EBR-II criticality; not the SHRT core (which is
  burned); no thermal expansion, no axial structure, solid sodium. Inputs
  are AI transcriptions, unreviewed.

## Reproduce

```text
RAYON_NUM_THREADS=3 cargo run --release -p outram-mc-libs --example ebr2_fresh_driver_kinf -- 2000 20 60
cargo test --release -p outram-mc-libs --test model_ebr2_fresh_driver
```

Needs `reference-data/endf/` and the `reference-data/ace` submodule.
