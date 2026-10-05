# Tutorial rung 3: natural-uranium lumps in graphite, k_inf against lump radius

**Class: verification**, not validation. The checks are the homogeneous limit
(rung 2's own result at the same atom ratio) and the algebra of the factors.
An OpenMC code-to-code comparison is prepared but **not yet run** (see the
end). No experiment exists for this cell. AI-assisted record under
`RESPONSIBLE_USE.md`; no human V&V sign-off. Education and research only.

Tutorial rung 3 (GitHub #525, epic #520). Example:
`crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs` (compositions in
`src/vv/ugraphite.rs`, shared bookkeeping with rung 2 in
`examples/common/ugraphite_common.rs`). Lesson page:
`crates/outram-mc-libs/docs/tutorial/src/lumped.md`. The example's `///` doc
comment carries the same results.

## Methodology

- **Cell.** A Wigner-Seitz sphere: natural uranium metal (19.05 g/cm3, IUPAC
  atom fractions) of radius `r` at the centre of graphite (1.73 g/cm3) of
  radius `R = r / v^(1/3)`, with `v` fixing the cell-average
  `N_C/N_U = 600` (`R = 6.94 r`). Built with `fhr_pebble_geometry` (ball, two
  graphite shells) and a **white** outer boundary (`diffuse_reflect`, OpenMC
  port verified in `../white_boundary/`). Not specular: see the `vv_gate` doc
  comment of `lump_self_shielding_scan.rs`.
- **Data and physics** exactly as rung 2 (`../tutorial_rung2/README.md`):
  ENDF/B-VIII.0 at 296 K, tolerance 0.001, URR + DBRC on, crystalline-graphite
  S(alpha,beta) on C-12 and C-13.
- **Factors** from `run_keff_reactor_physics`; "fuel" is the lump material
  (the library's definition, which is the textbook `f` here).
- **Check 1 (can fail): homogenised cell.** The same geometry at `r = 2 cm`
  with both regions filled with the cell-average mixture must reproduce rung
  2's homogeneous `k_inf` at `N_C/N_U = 600` (white is exact in an infinite
  homogeneous medium, and `k_inf` depends only on atom ratios). The
  criterion, fixed before the run: the two agree within 3 combined sigma.
- **Prediction** written in the example's doc comment before any run
  (commit `43832d173`), unedited since.
- **Geometry images** of every radius, drawn from the assembled CSG:
  [`geometry/`](geometry/) (`MODE=images`).

## Results

Running (2026-10-05); recorded in the next commit.

## Follow-up added AFTER seeing the N_C/N_U = 600 scan (prediction written before its run)

Written 2026-10-05 ~03:15 UTC, after the 600 scan had peaked **below 1**
(about 0.96 at r = 0.3 cm, then falling) and before any run at another ratio.
It does not change the pre-registered comparison above; it is a second
question the first result raised, labelled as such.

**Hypothesis.** At 600 carbon atoms per uranium atom the homogeneous thermal
utilisation is already low (f = 0.765, rung-2 sweep), and lumping lowers it
further, so the gain in p cannot carry k above 1. With less carbon,
N_C/N_U = **200** (homogeneous f = 0.907 but p = 0.58), lumps should raise p
a lot while f stays high.

**Prediction:** at N_C/N_U = 200, scanning r = 0.3, 1, 2, 3 cm, the maximum
k_inf **exceeds 1**, at r of order 1-2 cm (larger than at 600, because the
lumps sit closer together). Same settings as the 600 scan
(`CU=200 RADII=0.3,1,2,3 CONTROL=0`, 5000 x [20 + 50]).
