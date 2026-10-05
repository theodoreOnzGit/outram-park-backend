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

## Results at N_C/N_U = 600 (2026-10-05)

Binary built from `develop` at `f39501b8bd`. 5000 neutrons x [20 + 50] per
cell (sized for 2 shared cores; sigma ~250 pcm), seed 20 261 005 (+ 1 + index
for the lumps), 2 threads. Hardware: Intel Xeon @ 2.10 GHz, 2 threads pinned to
2 of 4 logical cores, 15 GB, Linux, CPU only, machine shared (load average up
to ~8). Run 02:07-03:16 UTC; 5-15 min per cell.

| r [cm] | R [cm] | k_inf | eta | f | p | epsilon |
|---|---|---|---|---|---|---|
| homogenised (control) | 13.882 | 0.77580 +/- 0.00208 | 1.33303 | 0.76483 | 0.74811 | 1.01241 |
| 0.1 | 0.694 | 0.93965 +/- 0.00254 | 1.33288 | 0.76000 | 0.91854 | 1.01206 |
| 0.3 | 2.082 | 0.95924 +/- 0.00211 | 1.33253 | 0.74922 | 0.94424 | 1.01542 |
| 1.0 | 6.941 | 0.93440 +/- 0.00288 | 1.33130 | 0.70584 | 0.96285 | 1.02887 |
| 2.0 | 13.882 | 0.85390 +/- 0.00239 | 1.32966 | 0.62863 | 0.96981 | 1.05332 |
| 3.0 | 20.823 | 0.76770 +/- 0.00295 | 1.32862 | 0.54765 | 0.97087 | 1.08605 |
| 4.0 | 27.764 | 0.68685 +/- 0.00252 | 1.32781 | 0.47341 | 0.96517 | 1.13058 |
| 6.0 | 41.646 | 0.55402 +/- 0.00195 | 1.32608 | 0.35050 | 0.93550 | 1.27094 |

Telescoping exact; `(k - k_factors)/k` -0.22 % .. +0.47 %, inside the band.

**Check 1 (homogenised cell): PASSES.** 0.77580 +/- 0.00208 against rung 2's
0.77153 +/- 0.00243 at N_C/N_U = 600 (`../tutorial_rung2/README.md`):
+427 +/- 320 pcm, 1.3 sigma < 3 sigma.

**Against the prediction (`43832d173`):** rise / peak / fall **held**; gain of
tens of per cent, nearly all in p **held** (+24 % at the peak); **maximum above
1, ~1.05 — refuted** (0.95924 +/- 0.00211, 19 sigma below 1); **at r ~ 1-3 cm —
refuted** (peak at 0.3 cm on this grid); **f falls a few per cent — refuted in
size** (0.76 -> 0.35 across the scan). Not predicted: epsilon 1.27 at r = 6 cm;
p falls again beyond 3 cm.

**OpenMC code-to-code: not run** (no OpenMC on this machine).
`openmc_inputs/lumped_openmc.py --r R` / `--control` runs these cells
unchanged; pending the maintainer.

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

**Result (2026-10-05 03:16-03:37 UTC, same binary, settings and hardware as
the 600 scan; `CU=200 RADII=0.3,1,2,3 CONTROL=0`, seed 20 261 005):**

| r [cm] | R [cm] | k_inf | eta | f | p | epsilon |
|---|---|---|---|---|---|---|
| 0.3 | 1.447 | 1.04795 +/- 0.00257 | 1.32800 | 0.89991 | 0.84795 | 1.03228 |
| 1.0 | 4.822 | 1.08986 +/- 0.00233 | 1.32738 | 0.88001 | 0.89753 | 1.03916 |
| 2.0 | 9.644 | 1.08002 +/- 0.00220 | 1.32653 | 0.84273 | 0.91759 | 1.05065 |
| 3.0 | 14.466 | 1.04409 +/- 0.00239 | 1.32604 | 0.79521 | 0.92777 | 1.06298 |

Telescoping exact; `(k - k_factors)/k` +0.04 % .. +0.40 %. Cells drawn:
`geometry/ws_cell_r*_cu200.png` (checked: lump and cell radii on the axes,
R/r = 4.8221, nothing outside R).

**Prediction HELD on both counts:** the maximum exceeds 1 (**1.08986 +/- 0.00233
at r = 1 cm**, 2 cm 1.08002 +/- 0.00220 within 3 sigma of it), at r of order
1-2 cm. The hypothesis behind it is supported: f stays at 0.80-0.90 here
against 0.35-0.75 at 600, while p still rises with the lump (0.848 -> 0.928).
**Not a validation**: no experiment on this cell; OpenMC not run (pending).
