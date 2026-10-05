# Tutorial rung 2: homogeneous uranium in graphite, four factors and the natural-uranium sweep

**Class: verification**, not validation. The checks are against algebra
(telescoping), against an energy-only run of the same collision physics, and
against another code (OpenMC). No experiment exists for an infinite homogeneous
mixture of uranium and graphite. AI-assisted record under `RESPONSIBLE_USE.md`;
no human V&V sign-off. Education and research only.

Tutorial rung 2 (GitHub #524, epic #520). Example:
`crates/outram-mc-libs/examples/ugraphite_four_factor.rs` (compositions in
`src/vv/ugraphite.rs`, shared bookkeeping in `examples/common/ugraphite_common.rs`).
Lesson page: `crates/outram-mc-libs/docs/tutorial/src/ugraphite.md`. The
example's `///` doc comment carries the same results; this file collects them
with the methodology.

## Methodology

- **Model.** A reflective cube (half-width 50 cm) filled with one homogeneous
  material: `k` is `k_inf`, nothing leaks.
- **Data and physics (the defaults, nothing ablated).** ENDF/B-VIII.0 U-234,
  U-235, U-238, C-12, C-13 from `reference-data/endf/`, RECONR + BROADR at
  296 K and tolerance 0.001 by the workspace NJOY port; URR probability tables
  and DBRC on (constructor defaults); crystalline-graphite S(alpha,beta)
  (`tsl-crystalline-graphite.endf`, MAT 30, 296 K) on both carbons. No WMP.
- **Factors.** One power iteration (`run_keff_reactor_physics`, i.e.
  `transport_csg::run_keff_csg_reactor_physics`) with track-length tallies in
  three groups (thermal < 0.625 eV <= resonance < 100 keV <= fast) and a
  fine-group flux (12 300 bins, 1e-5 eV .. 20 MeV). Thermal absorption is
  split by nuclide from that fine-group flux (the library's material-based
  "fuel" would make `f = 1` in a homogeneous medium) and the factors are
  assembled by the library's `assemble_six_factors`.
- **Pass criteria.** `eta f p epsilon` must equal `P_total / A_total`
  (exact algebra); the power-iteration `k` must agree with the product inside
  `CONSISTENCY_BAND` (-1 % .. +5 %, the band existing because `(n,2n)` adds
  neutrons without fission). The comparisons with the energy-only `p` and with
  OpenMC are reported with their sigma, not gated.
- **Predictions** for both the main case and the sweep were written in the
  example's doc comment **before** any production run (commit `c199b7dc1`) and
  have not been edited since.

## Main case: one HTR-10 pebble's U and C (17 wt%), N_C/N_U = 767.2

Recorded 2026-10-04 on `develop` at `c199b7dc1f`. 20 000 neutrons x [20 + 100],
seed 20 261 004, 2 threads. Hardware: i9-13900K (16 logical cores, 62 GB,
Linux, CPU only), shared with other jobs. Data 90 s, transport 819 s.

| quantity | value |
|---|---|
| k_inf (generation mean) | **1.56777 +/- 0.00081** |
| eta | 2.02809 +/- 0.00132 |
| f (nuclide split) | 0.97470 +/- 0.00063 |
| p (3-group) | 0.71398 +/- 0.00044 |
| epsilon (3-group) | 1.11144 +/- 0.00093 |
| eta f p epsilon | 1.56867 +/- 0.00256, = P/A to 2e-16 |
| (k - k_factors)/k | -0.00057, inside the band |
| two-group view | eta 2.02809, f 0.97470, p 0.71052, epsilon 1.11686 |
| binning check fold(Sigma_a)/tally | 0.999989 |

Prediction (hand four-factor estimate): `k_inf ~ 1.4-1.6`, `eta ~ 2.0`,
`f ~ 0.98`, `p ~ 0.70`, `epsilon ~ 1.0-1.1`. **Held.**

**p against the energy-only walk** (`common/energy_only_slowing_down.rs`, the
kernel of `u238_resonance_escape.rs`, 400 000 histories from 100 keV, same
mixture and physics): 0.71274 +/- 0.00072 vs the transport's 0.71398 +/- 0.00044,
+0.00124 (1.5 sigma). Not the same quantity (the transport's resonance group
also holds the ~1 % of fission neutrons born below 100 keV), so agreement at
this level is what is expected.

**OpenMC code-to-code.** OpenMC 0.16.1-dev25 (`d7d3284a1`) on the openmc.org
ENDF/B-VIII.0 HDF5 library (294 K neutron data by nearest-temperature lookup,
`c_Graphite` at 296 K), ptables and DBRC on, same histories, seed 1, 1 thread,
316 s (same i9-13900K). Deck: [`openmc_inputs/ugraphite_openmc.py`](openmc_inputs/ugraphite_openmc.py)
(`--case pebble`).

| | outram-mc | OpenMC | difference |
|---|---|---|---|
| k (gen. mean) | 1.56777 +/- 0.00081 | 1.56711 +/- 0.00134 | +66 +/- 157 pcm (0.4 sigma) |
| k (OpenMC combined) | | 1.56807 +/- 0.00066 | -30 +/- 104 pcm (0.3 sigma) |
| eta | 2.02809 | 2.02786 | +0.01 % |
| f | 0.97470 | 0.97471 | -0.00 % |
| p | 0.71398 +/- 0.00044 | 0.71301 +/- 0.00084 | +0.14 % (1.0 sigma) |
| epsilon | 1.11144 | 1.11272 | -0.12 % |

Agreement within statistics on `k` and on every factor. The 2 K temperature
difference is stated, not corrected.

## Step 7 sweep: natural uranium, k_inf against N_C/N_U

Running (2026-10-05); recorded in the next commit.
