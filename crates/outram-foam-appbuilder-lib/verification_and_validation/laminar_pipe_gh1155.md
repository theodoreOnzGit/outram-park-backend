# Laminar pipe flow on the O-grid: Hagen–Poiseuille, f = 64/Re and the development length (gh:#1155)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Issue:** gh:#1155 (Foam Studio stage 1, epic gh:#1153). Gated by gh:#1161
(non-orthogonal correction), recorded in `non_orthogonal_correction_gh1161.md`.
**Date:** 2026-11-03. **The first resolved-pipe case in the workspace.**

## Question

Does OUTRAM PARK's `PimpleFoam` (the pimpleFoam port), on the O-grid pipe
mesh the Foam Studio demo uses, reproduce (1) the developed Hagen–Poiseuille
profile and pressure gradient, i.e. f = 64/Re, and (2) the hydrodynamic
development length of a uniform inlet, and do both converge with the grid,
at what observed order?

## Methodology

**Case** (`src/cases/laminar_pipe.rs`, `PipeFlow`): a straight pipe,
D = 20 mm, **L = 400 mm (L/D = 20)**, ν = 1e-6 m²/s, uniform inlet
U = 5 mm/s, **Re = 100**; walls no-slip, outlet fixed kinematic p = 0 and
zeroGradient U, inlet zeroGradient p. From rest. PISO, 1 outer / 2
correctors, Euler, upwind convection, `Gauss linear corrected` Laplacians
with **one** non-orthogonal corrector (gh:#1161 §5: with none the march at
Co ≈ 4 is slow or diverges). L/D = 20 rather than stage 0's 10 so that the
check slab is well past the development length (gh:#1161 §4).

**Meshes:** `outram_foam_mesh::block_primitives::PipeOGrid::level(R, L, m)`,
the geometrically similar family `(n, nr, nz) = (2m, 2m, 32m)`, m = 3, 4, 5:
17 280, 40 960, 80 000 cells; the wall is a `8m`-gon inscribed in the circle,
so the geometry error is part of the discretisation and vanishes as m → ∞.
Mesh quality (`fvm::max_non_orthogonality_deg` on the solver's mesh, L/D = 10
members): 28.0° at m = 4, 31.0° at m = 5, at the square's corners; m = 3 not
measured.

**Time step:** pseudo-transient, Co ≈ 4 capped so that νΔt/Δr² ≤ 2
(stage 0's stable range): Co = 2.70 / 2.02 / 1.62. Run to the stage 0
steady criterion (max |ΔU|/U per flow-through time < 1e-3).

**Instruments** (fixed before the runs; `cases::laminar_pipe`):

- **Developed pressure gradient:** `−dp/dz` from a volume-weighted
  p-against-z fit over every cell with z ∈ [0.25, 0.35] m (12.5–17.5 D),
  against Hagen–Poiseuille `32νU/D²`; then `f·Re = 64 × that ratio`.
  This is the quantity f = 64/Re states.
- **Developed centreline velocity:** per axial layer, the intercept of a
  `u_z = a + b r²` fit over the cells with r < R/2 (exact for a parabola);
  the mean over the slab's layers, against 2U.
- **Profile:** the volume-weighted RMS of `(u_z − 2U(1 − r²/R²)) / 2U` over the
  slab's cells.
- **Development length:** the first z (linear between layers) where the
  layer centreline velocity reaches 99 % of the *numerical* developed value
  (above). Durst et al. define it at 99 % of the developed centreline
  velocity; using the numerical one keeps the discretisation offset of the
  developed value (0.04–0.3 %) out of a quantity that is sensitive to it
  near the 99 % crossing.
- **Reference:** Durst, Ray, Ünsal & Bayoumi (2005), *J. Fluids Eng.* 127,
  1154–1160, doi:10.1115/1.2063088, `L/D = [0.619^1.6 + (0.0567 Re)^1.6]^(1/1.6)`
  = **5.772** at Re = 100 (stated accuracy: within 3 % of their simulations).
  **Provenance, honestly:** the original paper was not read in this session.
  The constants were checked against an open secondary source that quotes the
  correlation for a Newtonian pipe flow (arXiv:2409.00842, "A revisit of the
  development of viscoplastic flow in pipes and channels"), as seen in a web
  search result; its PDF could not be fetched (arxiv.org is not reachable
  from this session). A reviewer should check the constants against the
  original.
- **Grid convergence:** with h ∝ 1/m and three grids of ratios 4/3 and 5/4,
  solve `(f₃ − f₄)/(f₄ − f₅) = (3^-p − 4^-p)/(4^-p − 5^-p)` for the observed
  order p, then extrapolate `f∞ = f₅ − C 5^-p`.

Test: `tests/laminar_pipe_hagen_poiseuille.rs::measure_gh1155_grid_study`
(`--ignored`, ~6 min). The headless demo regression
(`dhoby-ghaut/examples/foam_studio_web/engine.rs`,
`headless_pipe_reaches_steady_hagen_poiseuille`) runs the 2.6k demo pipe.

**Predictions.** The only prediction written before these runs is gh:#1161's
(dp/dz falls toward the equal-area polygon value once the correction is in);
no prediction for the development length or the observed orders was written
first. That is a process miss, recorded rather than back-filled.

## Results (2026-11-03)

| m | cells | Co | steps | (−dp/dz)/HP | f·Re | u_c,dev / 2U | profile RMS | L_99 / D | wall (s) |
|---|---|---|---|---|---|---|---|---|---|
| 3 | 17 280 | 2.70 | 38 | 1.01076 | 64.69 | 0.9973 | 0.0116 | 6.200 | 8.1 |
| 4 | 40 960 | 2.02 | 65 | 1.00815 | 64.52 | 0.9990 | 0.0066 | 6.024 | 49 |
| 5 | 80 000 | 1.62 | 137 | 1.00664 | 64.42 | 0.9996 | 0.0043 | 5.952 | 280 |
| extrapolated | — | — | — | **1.0014** | **64.09** | **1.0002** | 0.0003 | **5.854** | — |
| observed order p | | | | 1.14 | | 3.0 | 2.0 | 2.5 | |
| reference | | | | 1 | 64 | 1 | 0 | 5.772 (Durst) | |

Outlet/inlet flux 1 to 1e-10, worst cell continuity error ≤ 4e-12 of the
inlet flow, in all three. Wall times on a shared host with other builds
running: indicative only.

**Reading it.**

- **f = 64/Re is reproduced in the grid limit:** f·Re falls from 64.69 to
  64.42 over the three grids and extrapolates to **64.09 (+0.14 %)**. The
  observed order is **1.1**, not the 2 a smooth second-order scheme would
  give. Not investigated; candidates are the wall-adjacent half-cell
  gradient (first order on the polygon's flat faces) and the polygon's
  shape effect, which is not only an area change (a polygon's Poiseuille
  number differs from an equal-area circle's at O(1/N²)). The extrapolated
  value is within the extrapolation's own uncertainty of 64 (three grids,
  non-constant ratio; no safety factor applied).
- **The developed profile is Hagen–Poiseuille:** u_c / 2U → 1.0002, profile
  RMS 1.2 % → 0.43 %, falling at second order.
- **Development length: 1.4 % above Durst et al. in the grid limit**
  (5.854 against 5.772 D; 7.4 / 4.4 / 3.1 % above on the grids). Durst's
  correlation is a fit to simulations, quoted within 3 %, so this agrees. The
  coarse grids over-predict it, as upwind convection's numerical diffusion
  would, and the miss shrinks at order 2.5.

**What misses.** The f·Re order of 1.1 is unexplained (above). Nothing was
tuned: every coefficient is the solver default or the upstream default, the
reference was fixed before the runs, and the one protocol choice (Co capped
by the viscous number, one non-orthogonal corrector) was made for stability
and is stated with its reason.

## The demo's meshes and solve settings (not the V&V grids)

The Foam Studio demo meshes the cylinder with stage 0's family
`(n, n − 2, n·L/D)` (`foam_studio_web/engine.rs::demo_ogrid`; the default is
the 8/6/80 grid, 20 480 cells, the same grid as gh:#1161's 20k row), not the
`level` family above: on the `level` family the 20k budget needs Co ≈ 2 for
stability, and the march took 136 steps instead of 34 (measured 2026-11-03).
So the demo's default mesh is covered by gh:#1161's table (Re = 100: dp/dz
1.0151 × HP in the still-developing slab; Re = 25: 1.0072), and by this grid
study only as a member of a neighbouring family. The demo runs one
non-orthogonal corrector and caps Δt at a viscous number νΔt/h² of 0.5 (h the
mean cell size): both measured-stable 20k runs sit at 0.48, and with a cap of
2 the Re = 10 run diverged (gh:#1162); the Re slider therefore starts at 20.

Native headless runs of the demo engine (`--headless`, Re = 100 unless
stated, to the steady criterion; shared host):

| cells | mesh time | steps | wall in the solve | u_c/2U at 0.75 L | (−dp/dz)/HP over 0.65–0.85 L |
|---|---|---|---|---|---|
| 1 920 (test) | 0.01 s | 31 | 0.4 s | — | — |
| 20 480 | 0.08 s | 34 | 9.3 s | 0.9940 | 1.0151 |
| 20 480, Re = 20 | — | 46 | 12.7 s | — | 1.0073 |
| 42 000 | 0.16 s | 52 | 40 s | 0.9953 | 1.0137 |

(The 1 920-cell row is the regression test's pipe; its numbers are pinned in
`engine.rs`. At 42k the viscous cap sets Co ≈ 3.25; an earlier build without
it, at Co ≈ 4, needed 171 steps for the same answer.)

## The browser demo (checks, not V&V)

Full wasm build (`web/foam_studio/build.sh`; wasm 11.5 MB, 3.7 MB gzipped),
served locally, driven by `laminar_pipe_gh1155/browser_check.js` in headless
Chromium (Playwright's bundled build, SwiftShader software GL) on the shared
4-CPU host. `?autostart&cells=N` meshes and runs the default pipe (Re = 100,
steady mode). While the worker solves, the script taps "+" and drags on the
view six times, and records every animation frame.

| run | viewport | mesh ready | to steady (wall) | steps · last ms/step (worker) | frame interval while solving: median / p95 / max | after |
|---|---|---|---|---|---|---|
| 20k | phone 390×844 (touch, DPR 2) | 1.0 s | **13.6 s** | 34 · 261 ms | 16.7 / 100 / 450 ms | max 50 ms |
| 20k | desktop 1280×800 | 0.7 s | **13.1 s** | 34 · 188 ms | 16.7 / 67 / 200 ms | max 17 ms |
| 42k (batch) | phone | — | **56 s** | 52 · 847 ms | 16.7 / 17 / 450 ms | max 17 ms |
| 42k (batch) | desktop | — | **56 s** | 52 · 714 ms | 16.7 / 17 / 250 ms | max 17 ms |

- **It runs, and converges in the browser to the native answer** (same step
  count; the title, the status line and the profile plot show it).
- **Phone layout:** the panel opens folded ("Controls »"); + / − / Reset and
  the Drag: orbit / pan toggle sit top right; the convergence dock (residual
  history, probe, profile against Hagen–Poiseuille) opens at the bottom.
  Screenshot: `laminar_pipe_gh1155/browser_phone_390_solving.png` (after
  three "+" taps, mid-solve); desktop:
  `laminar_pipe_gh1155/browser_desktop_1280_steady.png`.
- **Responsiveness:** the median frame stays at 16.7 ms throughout and the
  zoom taps took effect mid-solve (visible in the screenshot). The slow
  frames (p95 67–100 ms at 20k, single frames of 200–450 ms) come during the
  solve only. Software GL and the worker share four CPUs here, and that
  contention was not separated from main-thread work; the page itself only
  drains messages and paints. On a hardware GPU it was not measured.
- **Against the stage 0 estimate (about 4 s to a 1 %-steady 20k pipe):** the
  full steady criterion takes 34 steps here, not stage 0's 14 steps to 1 %,
  and each step carries the non-orthogonal correction and one corrector
  (gh:#1161, +43 % per step natively), so **13 s, not 4 s**. The 42k batch
  run is the minute the maintainer's decision allowed.
- **Found and fixed during the check:** the batch progress bar showed for
  the 20 480-cell default (the cap test compared with 20 000 exactly); the
  colour bar sat under it. Both moved (the screenshots predate the fix).

## Mesh images (the geometry-drawing rule)

In `laminar_pipe_gh1155/`, written by
`cargo run --release -p dhoby-ghaut --example foam_studio_web -- --render-mesh`,
from the `PolyMesh` the solver's `FvMesh` is built from (converted to
`outram_blender`'s neutral mesh only for the plotter), one colour per cell:

| file | what |
|---|---|
| `demo_20k_xy.png`, `demo_20k_xz_inlet.png` | the demo's default pipe (8/6/80, L/D = 10, 20 480 cells): cross-section at mid-length, x-z slice of the first 4.4 cm |
| `vv_m3_xy.png`, `vv_m3_xz_inlet.png` | the V&V grid m = 3 (17 280 cells) |
| `vv_m5_xy.png`, `vv_m5_xz_inlet.png` | the V&V grid m = 5 (80 000 cells) |

**Checked by eye (2026-11-03):** each cross-section is a closed regular
polygon with every outer vertex on the 1 cm circle (32-gon for the demo,
24-gon for m = 3, 40-gon for m = 5), a central square of half-width 0.55 cm
holding n × n cells (8, 6, 10) and a ring of nr radial cells (6, 6, 10), no
gaps or overlaps; the ring cells are thinnest at the square's corners (the
cells that set the max non-orthogonality). The axial slices show uniform
cells starting at z = 0: 2.5 mm long in the demo (16 in 4 cm), 4.17 mm for
m = 3 and 2.5 mm for m = 5, as `nz` says. The printed volumes equal the
inscribed-polygon prisms to the printed 7 digits (6.242890e-5, 1.242331e-4,
1.251476e-4 m³). **Not checked:** the faces between the two cut planes, and
the patch assignment (patches are not coloured; the solver's inlet/outlet
fluxes, equal to 1e-10, show the end patches are where they should be).

## The unstructured mesh: Hagen–Poiseuille on cfMesh's polyhedral mesh with wall layers (gh:#1202)

**Issue:** gh:#1202 (under the Foam Studio epic gh:#1153). **Started:**
2026-11-04. The demo gained a mesh choice for the pipe: **Structured** (the
O-grid above, the default, unchanged) or **Unstructured**: `outram-blender`'s
cylinder (48 facets) through the cfMesh port's carve → snap → tet → Delaunay
flips → face-minimal polyhedral dual → **wall-only** prism layers
(`outram_blender::foam_mesh::mesh_to_tet_dual_multipatch`,
`TetDualOptions::layer_patches = ["walls"]`), converted through the neutral
mesh to the solver's `FvMesh`.

### Question

On the polyhedral mesh the demo now offers, does the same `PimpleFoam` case
reproduce Hagen–Poiseuille (centreline velocity, profile, pressure gradient,
wall shear) as well as on the O-grid at a matched cell count, and if not, by
how much does it miss?

### What was changed in the mesher first (and what was not)

- **Layers on the wall only.** Before gh:#1202 the multipatch pipeline grew
  prisms over the whole boundary, inlet and outlet included (stage 0 found
  7488 of 8226 cells were prisms). Upstream cfMesh selects patches per
  patch (`boundaryLayers::addLayerForPatch`, `patchBoundaryLayers` in
  `meshDict`); its border rules (`boundaryLayersCreateVertices.C`,
  `createNewVertex`) were ported into
  `outram_park_fork_cfmesh::layers::add_boundary_layers_adaptive_on_patches`.
  That port is small (one function plus its assembly), so it was done rather
  than worked around. Tests and the before/after pictures:
  `outram-park-fork-cfmesh` `layers::tests::wall_only_layers_on_a_box_keep_the_ends`,
  `pipeline::tests::wall_only_layers_end_on_the_pipe_ends`.
- **Determinism.** The pipeline gave a different cell numbering on every
  run (the face-minimal dual and the layerers iterated hash maps); now
  pinned by `pipeline::tests::tet_dual_stages_are_deterministic`.
  **CORRECTED 2026-11-04 (gh:#1215):** ~~now pinned~~ that test compares two
  calls within one process; between processes the Delaunay flipper still
  swept faces in randomly seeded `HashMap` order and the h 6 mm pipe came
  out different on every run. Fixed (`BTreeMap` order) and pinned across
  processes by `back_off_only_layers_are_pinned_bit_for_bit`; see the
  gh:#1215 section.
- **Patch types.** The neutral mesh typed every cfMesh patch as a generic
  `patch`; `walls` now arrives as a `wall` (the cfMesh writer's own rule).
  The solver's boundary conditions go by name (`inlet`, `outlet`, anything
  else a no-slip wall), so this changed no number.
- **Not changed: no feature-edge capture.** cfMesh's
  `meshSurfaceEdgeExtractor` is not ported, so the dual's boundary faces at
  the rim wrap round the corner: on the 4 mm test mesh, end-patch points
  reach 1.0 mm up the wall, and the bare mesh holds 1.8 % less volume than
  the 24-gon prism. The inlet condition is applied on those bent faces.

### Predictions (written 2026-11-04, before any flow solve on the unstructured mesh)

The case is the demo's: D = 20 mm, L = 200 mm (L/D = 10), Re = 100, uniform
inlet, steady by pseudo-transient PISO (Co ≤ 4, viscous number ≤ 0.5), the
instruments of the demo's headless run over the slab 0.65–0.85 L
(130–170 mm, past Durst's 115 mm development length). Matched resolution:
the unstructured settings whose cell count is closest to the O-grid's, at
about 5k and about 20k cells.

| quantity | O-grid (structured) | unstructured, wall-only layers |
|---|---|---|
| converges to the steady criterion | yes, 30–40 steps at 20k (recorded: 34) | yes, but slower: 40–150 steps |
| outlet / inlet flux | 1 to 1e-9 | 1 to 1e-8 (the scheme is conservative on any mesh) |
| u_c / 2U at 0.75 L | 0.99–1.00 (recorded 0.994) | **0.90–1.00**. Stage 0 measured 0.63–0.75 on cfMesh meshes without layers *before* the non-orthogonal correction (gh:#1161) existed; I expect the correction to recover most of it, not all |
| (−dp/dz) / HP | 1.01–1.03 (recorded 1.015) | within ±10 % |
| profile RMS of (u_z − u_HP)/2U | 0.005–0.02 | 0.02–0.08 |
| τ_w / (4νU/R), one-sided `ν u_P/d` | 0.93–1.00 (d ≈ 0.4 mm on the ring) | 0.85–1.05 (d ≈ half the first layer) |
| max non-orthogonality | 26° (recorded) | 80–90° |
| max skewness (checkMesh) | ≈ 2.1 | 2–15 |
| mesh time, native | ≈ 0.1 s | 1–5 s at 20k |
| ms per step, native, per cell | baseline | 1.5–3× the O-grid's (more faces per cell, 2 non-orthogonal correctors) |

If u_c falls below 0.90 on the unstructured mesh, the candidates, in the
order I would test them: the missing skewness correction (the port
interpolates linearly to the face centre along `d`, as `Gauss linear`
does, and dual faces are skewed); the cell centre of non-star-shaped dual
cells; the bent inlet faces at the rim; plain under-resolution by the
large octahedral cells.

**Non-orthogonal correctors.** Upstream practice on meshes above ~70° is
more correctors and, above ~75–80°, `laplacian Gauss linear limited
corrected 0.33–0.5`. ~~The `limited` snGrad is not ported
(`non_orthogonal::LaplacianCorrection` refuses it), so the unstructured
case runs `corrected` with **2** correctors (the O-grid keeps 1, so its
numbers do not move).~~ **Superseded the same day:** the first runs used
`limited 0` with no correctors on the layered meshes (they diverged with
`corrected`); the maintainer then directed that the unstructured mesh get
non-orthogonal correctors and upstream's tutorial settings, `limitedSnGrad`
was ported, and every unstructured number was re-measured. Both sets are in
the results below; see "Settings from the OpenFOAM tutorials".

### Methodology (as run)

Native headless runs of the demo engine (`foam_studio_web --headless N`
for the O-grid, `--headless-unstructured h_mm layers first_mm expansion Re`
for cfMesh), release, Re = 100, steady mode, on the shared 4-CPU host one at
a time. The instruments are `cases::laminar_pipe`'s, the same on both
meshes, over the slab 0.65–0.85 L: `centreline_velocity` (u_z = a + b r²
fit, r < R/2, ±0.05 L about 0.75 L), `pressure_gradient`, `profile_rms`,
`wall_shear_stress` (one-sided `ν u_P / d` on the wall faces, OpenFOAM's
`wallShearStress` for a laminar wall), and the **binned profile**
(`radial_profile_binned`: cell centres within one cell size of the 0.75 L
plane, binned in 12 bins of r/R, volume-weighted) that the demo plots on
both meshes. "Matched resolution" became matched *cell count*: the O-grid
family jumps (7 920 → 13 230), so the pairs are 7 920 / 6 713 and 13 230 /
10 745. The structured default was re-run to check it did not move.

### Results (2026-11-04)

**The structured default did not move:** 20 480 cells, 34 steps, u_c/2U
**0.9940**, (−dp/dz)/HP **1.0151**, out/in 1.000000000 — the recorded
numbers to every printed digit.

**Mesher, `--mesh-sweep` (native, R 10 mm, L 200 mm, first layer 0.3 mm,
expansion 1.2; time is meshing only):**

| h (mm) | layers | cells | wall faces | max non-ortho (°) | max skewness | mesh (s) | note |
|---|---|---|---|---|---|---|---|
| 6 | 0 / 1 / 2 / 3 | 1939 / 6713 / 11487 / 16261 | 4774 | 56 / 86 / 89 / 89 | 1.0 / 1.3 / 1.7 / 1.7 | 0.5–0.8 | Delaunay flips skipped |
| 5 | 0 / 1 / 2 / 3 | 3113 / 10745 / 18377 / 26009 | 7632 | 48 / 83 / 88 / 90.5 | 0.9 / 1.1 / 1.9 / 8.9 | 0.2–0.7 | |
| 4 | 0 / 1 / 2 / 3 | 6353 / 6353 / 30289 / 42257 | 11968 | 51 / 51 / 90.0 / 86 | 0.9 / 0.9 / **251** / 1.2 | 0.4–2.0 | 1 layer: none added (back-off to zero) |
| 3.3 | 0 / 1 / 2 / 3 | 9055 / 25675 / 33524 / 46656 | 16620 | 61 / 87 / 87 / 87 | 0.9–1.5 | 0.6–1.8 | |
| 2.5, 2 | — | erratic | fewer than at 3.3 | | | | 2 mm: the dual skipped (all-tet) |

The demo therefore offers h ∈ [R/3, R/1.5], at most 3 layers, and refuses
settings estimated above 50k cells; meshing takes 0.2–2 s natively.

**Flow (Re = 100, slab 0.65–0.85 L):**

| mesh | cells | max non-ortho (°) | skew | Laplacian | steps | u_c/2U | (−dp/dz)/HP | profile RMS | τ_w/HP | out/in | ms/step |
|---|---|---|---|---|---|---|---|---|---|---|---|
| O-grid | 7 920 | 19.7 | 1.99 | corrected, 1 | 27 | **0.9905** | **1.0171** | 0.0112 | 1.0042 | 1.000000000 | 44 |
| cfMesh h 6 mm, 1 layer | 6 713 | 85.9 | 1.33 | limited 0 | 15 | **0.6105** | **1.2452** | 0.1713 | 1.1976 | 1.000000000 | 34 |
| O-grid | 13 230 | 23.3 | 2.07 | corrected, 1 | 31 | **0.9928** | **1.0161** | 0.0082 | 1.0057 | 1.000000000 | 85 |
| cfMesh h 5 mm, 1 layer (~~demo default~~, see the re-run) | 10 745 | 83.4 | 1.05 | limited 0 | 53 | **0.6118** | **1.2527** | 0.1733 | 1.2244 | 1.000000000 | 62 |
| O-grid (demo default) | 20 480 | 26.0 | 2.13 | corrected, 1 | 34 | 0.9940 | 1.0151 | 0.0063 | 1.0063 | 1.000000000 | 182 |
| cfMesh h 5 mm, **2 layers** | 18 377 | 87.7 | 1.88 | either | — | **diverged** | | | | | |
| cfMesh h 5 mm, no layers | 3 113 | 47.6 | 0.91 | corrected, 2 | 10 | 0.6272 | 1.4757 | 0.1665 | 1.4426 | 0.999999999 | 46 |
| cfMesh h 4 mm, no layers | 6 353 | 50.8 | 0.91 | corrected, 2 | 11 | 0.7022 | 1.6431 | 0.1334 | 1.6187 | 1.000000000 | 116 |
| cfMesh h 3.3 mm, no layers | 11 177 | 57.4 | 0.92 | corrected, 2 | 10 | 0.7544 | 1.6895 | 0.1074 | 1.6721 | 1.000000000 | 278 |

Binned profiles at 0.75 L (r/R : u/U): O-grid 13k `0.000:1.985 0.314:1.791
0.608:1.245 0.877:0.443 0.956:0.150` (Hagen–Poiseuille 2(1 − r²): 2.000,
1.803, 1.261, 0.462, 0.172); cfMesh 11k default `0.000:1.227 0.438:1.154
0.678:1.036 0.878:0.611 0.956:0.149`. The cfMesh profile is a plug with a
wall layer.

**Against the predictions:**

| prediction | result | verdict |
|---|---|---|
| unstructured converges, 40–150 steps | 1 layer: 15–53 steps; **2 or 3 layers: diverged on all 7 meshes tried**, at Co 4, at a 10× smaller Δt, with 0, 1 or 2 correctors, corrected or uncorrected | **missed** |
| mass to 1e-8 | 1 ± 1e-9 | held |
| u_c/2U 0.90–1.00 | **0.61–0.75** | **missed badly**; the non-orthogonal correction did not recover stage 0's flat profile |
| dp/dz within ±10 % | **+25 % (1 layer), +48 to +69 % (bare)**, and the bare meshes get *worse* with refinement (1.48 → 1.64 → 1.69) | **missed** |
| profile RMS 0.02–0.08 | 0.11–0.17 | missed |
| τ_w/HP 0.85–1.05 | 1.20–1.67 | missed |
| max non-ortho 80–90° (layered) | 83–90° (bare 48–61°) | held |
| skewness 2–15 | 0.9–1.9 (251 on one setting, flagged by the page) | held, mostly below |
| mesh 1–5 s at 20k | 0.2–2 s over the offered range | held (faster) |
| ms/step per cell 1.5–3× the O-grid | about 1–1.4× with limited 0 (no correctors); about 4× with 2 correctors on the bare meshes | missed both ways |

**The unstructured result is much worse than the structured one, and is
reported as it is.** The demo offers it, defaulting to the one-layer mesh
that runs, and the page says that 2 or more layers diverge.

### Settings from the OpenFOAM tutorials, and the re-run (maintainer direction, 2026-11-04)

The table above was run with `limited 0` and no correctors on the layered
meshes, a setting chosen because `corrected` diverged. The maintainer
directed that the unstructured mesh get non-orthogonal correctors and that
the settings follow the OpenFOAM tutorials. OpenFOAM-dev was cloned
shallowly (`--filter=blob:none`, sparse `tutorials/` and
`limitedSnGrad/`, commit `852527377c7e`, 2026-11-03) and these were read:

| tutorial (OpenFOAM-dev) | mesh | `laplacianSchemes` / `snGradSchemes` | `nNonOrthogonalCorrectors` | `gradSchemes` | `div(phi,U)` |
|---|---|---|---|---|---|
| `legacy/incompressible/icoFoam/elbow` | unstructured (Fluent `elbow.msh`), laminar PISO | `Gauss linear corrected` / `corrected` | **2** | `Gauss linear` | `Gauss limitedLinearV 1` |
| `legacy/basic/laplacianFoam/flange` | unstructured (ANSYS) | `Gauss linear corrected` / `corrected` | **2** | `Gauss linear` | — |
| `incompressibleFluid/propeller` (pimpleFoam) | snappyHexMesh, `maxNonOrtho 65`, layers to 75 | `Gauss linear limited corrected 0.33` / `limited corrected 0.33` | 0 (nOuter 2, nCorr 1) | `cellLimited Gauss linear 1` on U | `Gauss linearUpwind grad(U)` |
| `incompressibleFluid/turbineSiting` (simpleFoam) | snappyHexMesh | `limited corrected 0.33` | 0 | `Gauss linear` | `bounded Gauss upwind` |
| `incompressibleVoF/cavitatingBullet` | snappyHexMesh | `limited corrected 0.5` | 0 (3 for `pcorr`) | `Gauss linear` | `Gauss linearUpwind grad(U)` |
| `motorBikeSteady`, `drivaerFastback`, `windAroundBuildings` | snappyHexMesh, `maxNonOrtho 65` | `corrected` | 0 (10 only in `potentialFlow`) | `cellLimited Gauss linear 1` | `bounded Gauss linearUpwind…` |
| `multiRegion/CHT/misalignedDuct` (fluid) | — | `corrected` | 1 | `cellLimited Gauss linear 1` on U | `Gauss linearUpwind grad(U)` |

No tutorial runs on a `polyDualMesh` or cfMesh mesh in this tree (grep:
none). **Chosen, and why:** `nNonOrthogonalCorrectors 2` (elbow and
flange, the two unstructured-mesh cases, elbow being laminar PISO like
this one); `Gauss linear corrected` up to 70° max non-orthogonality (the
snappy cases meshed to 65°) and `Gauss linear limited corrected 0.33` above
it (propeller and turbineSiting, whose layers are allowed to 75°). The
`limitedSnGrad` limiter, `min(ψ |snGrad_uncorrected| / ((1 − ψ)
|correction| + small), 1)`, was ported from OpenFOAM-dev
`limitedSnGrad.C` into `outram_foam_appbuilder_lib::solvers::non_orthogonal`
(`LaplacianCorrection::Limited`, `limit_scalar_correction`,
`limit_vector_correction`; test `limited_correction_follows_upstream_limiter`)
and applied to the pressure, momentum and energy Laplacians. **Not
followed:** convection stays `Gauss upwind` on both meshes (the demo's
scheme; changing it would change the structured default), and `cellLimited`
gradients are not ported. The structured mesh keeps its settings
(`corrected`, 1 corrector); re-run: 34 steps, 0.9940, 1.0151, unchanged.

**Re-run with those settings (Re = 100, slab 0.65–0.85 L):**

| mesh | cells | max non-ortho (°) | settings | steps | u_c/2U | (−dp/dz)/HP | profile RMS | τ_w/HP | out/in | ms/step |
|---|---|---|---|---|---|---|---|---|---|---|
| cfMesh h 6 mm, 1 layer (**demo default now**) | 6 713 | 85.9 | limited corrected 0.33, 2 correctors | 64 | **0.6179** | **1.3355** | 0.1683 | 1.2782 | 1.000000000 | 89 |
| cfMesh h 5 mm, 1 layer | 10 745 | 83.4 | limited corrected 0.33, 2 | 273 | **0.6216** | **1.3921** | 0.1684 | 1.3417 | 1.000000000 | 148 |
| cfMesh h 5 mm, 1 layer | 10 745 | 83.4 | limited corrected **0.5**, 2 | not steady in 5000 (metric oscillates at 0.2) | 0.6268 | 1.4495 | 0.1655 | 1.4057 | 1.000000000 | 165 |
| cfMesh h 5 mm, 2 layers | 18 377 | 87.7 | limited corrected 0.33, 2 (and 3) | **diverged** | | | | | | |
| cfMesh h 4 / 3.3 mm, no layers | 6 353 / 11 177 | 50.8 / 57.4 | corrected, 2 | 11 / 10 | 0.7022 / 0.7544 | 1.6431 / 1.6895 | 0.133 / 0.107 | 1.62 / 1.67 | 1.000000000 | 120 / 266 |

**Reading it.** The tutorial settings run the one-layer meshes and change
the answer by little: u_c/2U 0.61 → 0.62, dp/dz 1.25 → 1.34–1.39 × HP (the
limited correction adds back part of the non-orthogonal flux, and the
miss grows), at 4–5× the steps. Two or more layers still diverge, with 2 or
3 correctors. So the missing correctors were not the cause of the miss;
the dual-mesh diagnosis above stands. The demo's default became the 6 713-
cell mesh (64 steps, ~6 s natively) because the 10 745-cell one now takes
273 steps (43 s) for the same answer to 1 %. The headless regression
`headless_unstructured_pipe_runs_and_is_pinned` pins 0.6179 / 1.3355.

**What was tested about the cause (`--ignored`
`diagnose_dual_mesh_geometry_and_laplacian` in `foam_studio_web/engine.rs`,
and the `FOAM_CFMESH_STAGES` / `FOAM_NON_ORTH` / `FOAM_LAPLACIAN` /
`FOAM_MAX_VISCOUS` knobs of the native engine):**

- **Not the geometry bookkeeping.** On the 6 353-cell dual every interior
  cell closes to 1e-16 and the Gauss volume Σ x_f·S_f / 3 matches the
  solver's cell volume to 3e-14.
- **The two-point Laplacian is biased low on the dual.** For φ = x² + y²
  (∇²φ = 4) the uncorrected `|S|²/(S·d)` operator reads 0.885 × 4 on
  average over interior dual cells (worst cell off by 171 %), the
  orthogonal `|S|/|d|` 0.783; on the O-grid 0.983 and 0.980. Too little
  interior diffusion keeps a plug core, which is what the profiles show.
  The explicit correction should remove the mean bias; with it (bare
  meshes) the profile is still flat, so either the Gauss-linear gradient
  in the correction is itself poor on these cells or something else
  dominates. **Not resolved.**
- **It is the dual, not cfMesh's earlier stages.** The snapped hex carve
  alone (`FOAM_CFMESH_STAGES=carve`, 1 050 cells) gives dp/dz 0.88 × HP and
  RMS 0.07, on the right side of the reference, while the dual of the
  same carve gives 1.64; the tetrahedral mesh (`=tet`) diverges.
- **Radially the dual is very coarse.** The cross-section at the default
  (`unstructured_h5_1layer_wall_only_xy.png`) has 4–5 cells across the
  diameter (the O-grid at 8k has 14), and the snapped wall is a ~16-gon,
  not the 48-faceted surface. Part of the miss is resolution, but the bare
  meshes' dp/dz grows with refinement, so not all of it.
- **The layered meshes' divergence is spatial, not temporal:** it happens
  at every Δt and corrector count tried. Not investigated further.

### Isolating the miss: geometry, orientation, a manufactured Poisson problem (maintainer direction, 2026-11-04)

The non-orthogonal correction did not move the answer, so the maintainer
asked for the geometry and the discretisation's geometric terms to be
checked, cheapest first. Diagnostic: `foam_studio_web/engine.rs`
`diagnose_geometry_orientation_and_poisson` (`--ignored`), on the O-grid
7 920, the dual h 4 mm without layers, and the dual h 6 mm with 1 layer.

**Predictions (written 2026-11-04, before running it):**

1. *Closure and volume.* Closure already measured 1e-16 and the Gauss
   volume 3e-14 per interior cell (the first diagnostic). Σ V: O-grid
   0.9936 πR²L (the 32-gon), dual 0.97–0.99 (the ~16-gon snapped wall and
   chamfered rim). Wall area: O-grid 0.9984, dual 0.97–1.0. Non-planarity:
   O-grid ~1e-16 (planar quads), dual 1e-3–1e-1 (median-dual faces are
   non-planar), layer prism side faces non-planar too. The port's
   face and cell centres follow OpenFOAM's triangle decomposition about the
   face's average point (`face_centre_and_area` / `cell_geometry` in
   `outram-foam-mesh`, read before writing this), so non-planarity should
   bias nothing beyond OpenFOAM's own.
2. *Orientation.* 0 misoriented internal faces and 0 boundary faces on all
   three meshes (the layer back-off rejects folded faces).
3. *Poisson.* O-grid: b = 1.00 ± 0.01, RMS < 1 %. Dual: **if b differs from
   1 by more than 5 %, the Laplacian (or the mesh it sees) is the cause, not
   PISO**; the two-point check measured the uncorrected operator 12 % low,
   so I predict the uncorrected solve gives b ≈ 1.1 and the corrected one
   b ≈ 1.0–1.05. If corrected b ≈ 1, the Laplacian is cleared and the miss
   is in the momentum/pressure coupling on these meshes.
4. *Wall snGrad.* Read, not run: `DeltaCoeff::NonOrthogonal.boundary_delta`
   is the projection n·d (as OpenFOAM's `fvPatch::delta()`), and the
   momentum matrix is restamped to it on fixed-value/no-slip walls
   (`non_orthogonal::restamp_vector_delta`). Predicted correct.
5. *Development length and station.* Durst: 115 mm at Re = 100; the slab
   is 130–170 mm and the O-grid reads 0.99 there. On the dual the same
   station; the binned profile is flat at every station, so not the cause.

**Results (2026-11-04)** (`diagnose_geometry_orientation_and_poisson`,
`diagnose_unstructured_reynolds_dependence`,
`diagnose_unstructured_linear_refinement`, all `--ignored` in
`foam_studio_web/engine.rs`):

| check | O-grid 7 920 | dual h 4 mm, no layer | dual h 6 mm, 1 layer | verdict against the prediction |
|---|---|---|---|---|
| 1. Σ V / πR²L | 0.98862 | 0.97866 | **0.94155** | O-grid: the 24-gon of this grid (0.9886), not the 32-gon I wrote; dual as predicted, the layered mesh lower |
| 1. wall area / 2πRL | 0.99715 | 0.99254 | 0.98018 | held |
| 1. max non-planarity / √\|S\| | 7e-14 | 0.31 | **1.05** | held in kind, larger than predicted on the layered mesh |
| 2. misoriented internal / boundary faces | 0 / 0 | 0 / 0 | 0 / **19** | **missed on the layered mesh**: 19 boundary faces whose normal points back past their cell centre |
| 3. Poisson, uncorrected: b (exact 1), a/R² | 1.0008, 0.969 | 0.959, 1.060 | 1.018, 1.112 | dual within 4–11 %, **not** the 30–40 % flow miss |
| 3. Poisson, corrected (20 passes): b, a/R², RMS/a | 1.012, 0.989, 0.004 | 0.983, 0.992, 0.015 | 1.077, 1.010, 0.018 | held: **the Laplacian and the mesh it sees are within 2–8 %** |
| 4. wall snGrad distance | n·d (read) | n·d | n·d | held (read, not measured) |
| 5. development, u_c/2U at z/L 0.2 / 0.45 / 0.75 / 0.9 | 0.880 / 0.969 / 0.990 / 0.993 | — | 0.617 / 0.618 / 0.618 / 0.618 | the dual flow is **developed** by 0.2 L, to the wrong profile: not a development-length or station problem |

(The first Poisson run built Γ with `SurfaceScalarField::uniform`, which sets
the internal faces only; the walls got no Dirichlet term and every solve
diverged, the O-grid's too. The harness was fixed to interpolate Γ; the
numbers above are from the fixed run.)

**So none of checks 1–5 explains the flow miss; the Re dependence did.**
At Re = 20 the 6 713-cell dual gave u_c/2U 0.73 and dp/dz 0.99 × HP, at
Re = 100 0.62 and 1.34: the error grows with convection. On the dual's
faces, oblique to the flow, **first-order upwind adds cross-stream false
diffusion** of order U·h·sin 2θ / 4 (Patankar 1980, *Numerical Heat
Transfer and Fluid Flow*, §5.6; De Vahl Davis & Mallinson 1976), here about
U·h/4 ≈ 1.5e-5 m²/s against ν = 1e-6: it carries momentum from the core to
the wall, flattens the developed profile and raises the wall shear and dp/dz.
On the O-grid the faces are aligned with the flow and the term vanishes,
which is why upwind is harmless there. Test: the same meshes with `Gauss
linear` convection (second order, no false diffusion):

| mesh (Re = 100, `Gauss linear`, corrected, 2 correctors) | cells | steps | wall (s, native) | u_c/2U | (−dp/dz)/HP | profile RMS | τ_w/HP |
|---|---|---|---|---|---|---|---|
| dual h 6 mm, no layer | 1 939 | 70 | 2.2 | 0.8623 | 0.8549 | 0.0858 | 0.8185 |
| dual h 5 mm, no layer | 3 113 | 55 | 3.0 | 0.8856 | 0.8868 | 0.0715 | 0.8665 |
| dual h 4 mm, no layer (**demo default**) | 6 353 | 61 | 8.6 | **0.9416** | **0.9414** | 0.0395 | 0.9257 |
| dual h 3.3 mm (R/3), no layer | 11 177 | 54 | 16.2 | **0.9685** | **0.9754** | 0.0192 | 0.9634 |
| dual h 6 mm, 1 layer (limited corrected 0.33) | 6 713 | 146 | 13.7 | 0.8503 | 0.8292 | 0.0872 | 0.7957 |
| dual h 5 mm, 1 layer (limited corrected 0.33) | 10 745 | 75 | 11.4 | 0.8726 | 0.8572 | 0.0751 | 0.8266 |
| O-grid 7 920, upwind (unchanged) | 7 920 | 27 | — | 0.9905 | 1.0171 | 0.0112 | 1.0042 |
| O-grid 7 920, `Gauss linear` (control) | 7 920 | 29 | — | 0.992 (z = 0.75 L) | 1.0146 | — | — |

(At h 4 mm "1 layer" gave the bare mesh: the layerer added none, as the
sweep recorded.) **With central convection the unstructured pipe converges
toward Hagen–Poiseuille with refinement, from below:** the miss falls from
14 % (h 6 mm) to 6 % (h 4 mm) to 3 % (h 3.3 mm) on u_c and dp/dz alike,
while the O-grid control barely moves (1.0171 → 1.0146). The wall layer does
not help at these resolutions (0.85 against 0.86 bare at 6 mm). The demo now
runs the unstructured pipe with `Gauss linear` convection (the tutorials'
`limitedLinearV 1` / `linearUpwind grad(U)`, bounded second-order schemes, are
not ported; `Gauss linear` is the second-order scheme the port has, and the
coil demo already uses it), 2 non-orthogonal correctors and `corrected`
(the bare meshes are at 51–57°) or `limited corrected 0.33` above 70°, and
its default is the bare h 4 mm mesh. The structured mesh keeps upwind, and
its recorded numbers (34 steps, 0.9940, 1.0151 at 20k) did not move.

**Does the unstructured pipe "match" Hagen–Poiseuille?** Not to the O-grid's
1 %: 6 % low at the default, 3 % low at the finest cell the mesher handles
(h = R/3; below that the cfMesh pipeline skips stages, see the sweep). The
trend is monotone toward 1 but three points on an irregular mesh family
give no reliable order or extrapolation. **That is the maintainer's call
for the merge.** Still open: the 19 inward boundary faces of the layered
mesh (check 2), the 2+ layer divergence, the mesher's degradation below
R/3, and a bounded second-order convection scheme.

### `linearUpwind` and `cellLimited`, the tutorials' convection (gh:#1202, merge work, 2026-11-04)

**Maintainer's merge bar:** the unstructured demo default within **2 %** of
Hagen–Poiseuille on both u_c and dp/dz, after porting `linearUpwind`, fixing
the 19 inward boundary faces and fixing the 2+ layer divergence; reached by
fixing physics and numerics, not by tuning.

**What was ported.** From OpenFOAM-dev `852527377c7e` (2026-11-03):
`linearUpwind<vector>::correction` (`linearUpwind.C`) and
`cellLimitedGrad<vector, minmod>` (`cellLimitedGrad.{C,H}`,
`minmodGradientLimiter.H`), into
`outram_foam_appbuilder_lib::solvers::schemes` (`linear_upwind_correction`,
`cell_limited_grad_vec`, `grad_vec_scheme`; `DivScheme::GaussLinearUpwind`
now runs instead of returning `UnsupportedScheme`; new
`GradScheme::CellLimitedGaussLinear(k)` and the `grad(U)` entry
`FvSchemes::grad_u`). Searched first: the appbuilder had the `fvSchemes`
enum variant (unimplemented), `fvc::grad_vec` (Gauss linear, reused as the
base gradient) and no gradient limiter. Upstream's
`correctedSnGrad::fullGradCorrection` takes its gradient from the
`grad(U)` scheme too, so the momentum Laplacian's non-orthogonal
correction now uses `grad_u` as well (with the default `Gauss linear`
nothing changes, bit for bit). Unit tests against hand values:
`schemes::tests::linear_upwind_matches_hand_values_and_is_exact_for_linear_fields`,
`cell_limited_gradient_matches_hand_values`.

**Chosen for the demo, from the tutorials, before any run:**
`div(phi,U) Gauss linearUpwind grad(U)` with
`grad(U) cellLimited Gauss linear 1`: `incompressibleFluid/propeller`
(pimpleFoam, snappy mesh with layers) and
`multiRegion/CHT/misalignedDuct` use exactly this pair. `Gauss linear`
(the previous setting) and unlimited `linearUpwind` are measured beside it
as controls; the choice does not depend on which comes out closer.

**Predictions (written 2026-11-04, before running `linearUpwind` on the
pipe):**

| quantity | prediction | reasoning |
|---|---|---|
| u_c/2U, dp/dz at h 6 / 5 / 4 / 3.3 mm, `linearUpwind`, Gauss linear grad | within ±0.02 of the `Gauss linear` row at each h (h 4 mm: 0.92–0.96) | both are second order; `Gauss linear` already removed upwind's cross-stream false diffusion, so the scheme should not be what is left |
| the same with `cellLimited Gauss linear 1` | 0–0.03 below the unlimited `linearUpwind` | the limiter flattens the gradient at the wall and centreline extrema, adding some diffusion |
| trend with h | monotone toward 1 from below, as `Gauss linear` | |
| demo default within 2 % with the scheme alone | **no** | u_c and dp/dz miss by the same 6 % at h 4 mm with `Gauss linear`; a scheme change would move the profile shape, not both together. I expect the cause elsewhere (step 4 candidates) |
| steps to steady at h 4 mm | 40–80 (Gauss linear: 61) | upwind-biased, so at least as stable as central |

**Results (2026-11-04, native release, Re = 100, bare duals, 2 correctors,
`corrected`; `diagnose_unstructured_linear_refinement` with `FOAM_DIV_U`;
the `Gauss linear` rows reproduce the earlier record to every digit):**

| h (mm) | cells | `Gauss linear`: steps, u_c/2U, dp/dz, RMS, τ_w | `linearUpwind`, Gauss linear grad | `linearUpwind`, `cellLimited … 1` (**demo setting**) |
|---|---|---|---|---|
| 6 | 1 939 | 70, 0.8623, 0.8549, 0.0858, 0.8185 | 20, 0.8182, 0.9520, 0.1035, 0.9111 | 22, 0.7333, 0.9989, 0.1184, 0.9575 |
| 5 | 3 113 | 55, 0.8856, 0.8868, 0.0715, 0.8665 | 18, 0.8349, 0.9704, 0.0903, 0.9488 | 22, 0.8287, 0.9336, 0.0918, 0.9128 |
| 4 (default) | 6 353 | 61, 0.9416, 0.9414, 0.0395, 0.9257 | 26, **0.9205**, **1.0005**, 0.0472, 0.9853 | 29, **0.8534**, **0.9983**, 0.0574, 0.9832 |
| 3.3 | 11 177 | 54, 0.9685, 0.9754, 0.0192, 0.9634 | 32, 0.9675, 1.0114, 0.0225, 1.0005 | 37, 0.9585, 0.9933, 0.0246, 0.9809 |

| prediction | result | verdict |
|---|---|---|
| `linearUpwind` within ±0.02 of `Gauss linear` | u_c −0.02 to −0.05 (h ≥ 4 mm), dp/dz **+0.04 to +0.10** | **missed**: the scheme moves dp/dz much more than predicted, and u_c and dp/dz no longer move together |
| `cellLimited` 0–0.03 below unlimited | u_c −0.07 at h 4 mm, −0.09 at 6 mm, −0.01 at 3.3 mm; dp/dz ±0.04 | **missed** at h 4–6 mm (more diffusive than predicted) |
| monotone toward 1 | `linearUpwind` u_c monotone; dp/dz overshoots 1 at 3.3 mm; limited u_c monotone, dp/dz not | partly |
| default within 2 % with the scheme alone | demo setting: u_c **0.853**, dp/dz **0.998** | held (it is not): u_c misses by 15 % |
| 40–80 steps at h 4 mm | 26–29 | missed (faster) |

**Reading it.** dp/dz is within 0.2 % of Hagen–Poiseuille at the default
under both `linearUpwind` variants, but the centreline velocity is 8–15 %
low; the profile is flatter than parabolic while the wall shear is right
(τ_w 0.98). The schemes disagree with each other by more than their
truncation error should allow at h = 4 mm, and the limiter (which only
acts where the gradient extrapolation overshoots a neighbour) changes u_c
by 7 %: the gradient on these cells is suspect. That is the "gradient
reconstruction" candidate of step 4, examined below. The demo takes the
tutorial pair regardless, as decided before the run.

### Is the u_c instrument biased on the dual? (coordinator's check, 2026-11-04)

**Question (coordinator).** If u_c came from cell centres binned near the
axis, a bin of radius b over the parabola reads u_c(1 − b²/2R²), about 8 %
low for b ≈ 0.4 R, on the dual but not on the O-grid (which has a cell on
the axis). Check the instrument before changing numerics.

**What the instrument is (read from `cases::laminar_pipe`, before
measuring).** The headline u_c is **not** binned:
`centreline_velocity` is a volume-weighted least-squares fit u_z = a + b r²
over every cell with r < R/2 in the slab |z − 0.75 L| < 0.05 L, u_c = a,
which is exact for a parabola wherever the cells sit. That is instrument (b)
already. Only the plotted profile (`radial_profile_binned`) bins, and its
first bin is what the binning bias would affect.

**Predictions (written before the run), at the demo default (h 4 mm,
6 353 cells, `linearUpwind` + `cellLimited`):**

| instrument | prediction |
|---|---|
| binned first bin (r/R < 1/12), u/2U | ≤ the fit by u_c b²/2R² with b ≈ the bin's mean r, i.e. 0–2 % below the fit (the bin is 0.08 R wide, not 0.4 R) |
| (b) fit u = a − b r² (r < R/2), a/2U | 0.853 (the recorded number: this *is* the headline instrument) |
| (b′) the same fit over r < 0.9 R | within 0.02 of (b) if the profile is parabolic |
| (a) RMS over the slab of (u − u_HP(r))/2U at each cell's own centre, u_HP from the **nominal** U | 0.05–0.06 (recorded 0.057) |
| (c) slab flow Σ u_z V / Δz against the inlet flux | 1 to 1e-6 (mass is conserved to 1e-9) |
| (c′) inlet flux against U πR² | **0.92–0.96.** My hypothesis, from a geometry check made just before the coordinator's message (cfMesh's own 48-sided pipe, `diagnose_inward_boundary_faces_on_layered_pipes`): the inlet patch's projected area is 0.9396 πR² at h 4 mm (0.8656 at 6 mm, 0.8915 at 5 mm, 0.9718 at 3.3 mm), because the rim is chamfered (no feature-edge capture) and the chamfer faces are `walls`. `Gauss linear`'s u_c and dp/dz tracked these ratios to 0.3–1 %. If so, the miss is the flow rate the case carries, not the profile |
| (a′) the RMS against u_HP built from the **measured** flow rate and section area (U_b = Q/A, R_eq = √(A/π)) | below (a) |

**Results (2026-11-04, `diagnose_pipe_instruments` in
`foam_studio_web/engine.rs`, `--ignored`; Re = 100 nominal, to steady).**
Q is the inlet face flux (exact: out/in = 1 ± 1e-9); A the exact area of
the section the 0.75 L plane cuts (Σ of the cut polygons).

| instrument | dual h 4 mm, `linearUpwind` + `cellLimited` (demo) | dual h 4 mm, `Gauss linear` | O-grid 7 920, upwind (control) |
|---|---|---|---|
| binned first bin, u/2U (bin's mean r/R) | 0.8491 (0.000) | 0.9279 (0.000) | 0.9740 (0.130) |
| (b) fit r < R/2, a/2U (**the headline**) | 0.8534 | 0.9418 | 0.9905 |
| (b′) fit r < 0.9 R | 0.8862 | 0.9423 | 0.9932 |
| (a) RMS (u − u_HP)/2U at cell centres, nominal U, R | 0.0574 | 0.0395 | 0.0112 |
| (a′) the same with U_b = Q/A, R_eq = √(A/π) | 0.0385 | 0.0120 | 0.0056 |
| (c) Σ u_z A over the cut / Q | 1.0088 | 1.0133 | 1.0000 |
| inlet Q / (U πR²) | **0.9396** | **0.9396** | 0.9886 |
| cut area / πR² | 0.9796 | 0.9796 | 0.9886 |
| u_c / 2U_b, U_b = Q/A | **0.8897** | **0.9820** | 0.9905 |
| (−dp/dz) / (8νU_b/R_eq²) | **1.0196** | **0.9616** | 1.0055 |
| nominal u_c/2U, dp/dz/HP | 0.8534, 0.9983 | 0.9418, 0.9414 | 0.9905, 1.0171 |

**Against the predictions.** The binning bias is not what moved u_c: the
headline is a fit, and the first bin reads within 0.4–1.5 % of it (held).
(b′) is within 0.02 of (b) on `Gauss linear` and the O-grid, but **0.033
above** on the demo setting: its profile is not parabolic (held / missed).
(c) closes to 1–1.3 % on the dual (midpoint quadrature over cut cells of
a curved profile; exact on the O-grid) (held, looser). The inlet carries
**0.9396 U πR²** on the demo mesh, as predicted from the cfMesh-only
geometry, while the section is 0.9796 πR²: **the case runs at 4.1 % less
flow than U over its own section**, because the chamfered rim's faces are
`walls`, not `inlet` (feature-edge capture is not ported). (a′) < (a)
(held).

**Which instrument the 2 % bar should use, from the physics.**
Hagen–Poiseuille relates the centreline velocity and the gradient to the
flow rate through the section: u_c = 2Q/A, −dp/dz = 8μQ/(πR⁴). The
nominal normalisation assumes Q = U πR², which is a property of the
mesh's inlet patch, not of the solution; on the dual it is false by 6 %.
~~So, for "does the solver reproduce Hagen–Poiseuille", the right
instrument is **(b), the fit intercept, normalised by the flow the case
carries: u_c/2U_b and dp/dz/(8νU_b/R_eq²) with U_b = Q/A** (both exact
quantities), with (a′) beside it as the whole-profile check. The nominal
numbers stay reported, because they are what the demo shows and they say
the demo runs at Re ≈ 94, not 100: **that is a defect of the case**
(the inlet does not span the pipe) and it is listed to fix, not normalised
away. On the O-grid the two normalisations differ by the 24-gon's area
(1.1 %), so the structured record keeps its nominal numbers.~~
**CORRECTED (maintainer direction via the coordinator, 2026-11-04): the
bar's instrument is the nominal one**, u_c and dp/dz against
Hagen–Poiseuille for the U and R the user set; the flow-normalised numbers
are diagnostics. The case must carry the flow it says it does, so the
chamfer was fixed at its source (below), not normalised away.

**Verdict.** The instruments agree that the demo setting's u_c really is
low: **11 % against its own flow**, with an RMS three times
`Gauss linear`'s. `Gauss linear` on the same mesh is −1.8 % / −3.8 %
against its own flow. So the `linearUpwind` + `cellLimited` profile is a
numerics problem, and the remaining 4 % of the nominal miss is the inlet.

### Port defect: the linear interpolation weights (found 2026-11-04, step 4 "gradient reconstruction")

**Upstream is the specification.** OpenFOAM-dev `852527377c7e`
`surfaceInterpolation::makeWeights` sets the owner weight of an internal
face to `w = |S·(C_N − C_f)| / (|S·(C_f − C_O)| + |S·(C_N − C_f)|)`
(projections on the face normal; the distance form `dNei/(dOwn + dNei)`
only as a fallback when the projections vanish). The port
(`outram_foam_basic_lib::fvc::interpolate::linear_weight`, and a copy,
`non_orthogonal::owner_weight`, used by the explicit non-orthogonal
correction) uses `|C_N − C_f| / |C_N − C_O|`, which is neither: on any
face whose centre is off the line between the cell centres it is a
different number. Every `fvc::interpolate`, so every Gauss gradient, the
`Gauss linear` convection, `rAU` and `HbyA` interpolation and the face
flux, uses it. **Measured** (`diagnose_interpolation_weights`, geometry
only): the structured default O-grid 20 480: 30 720 of 59 904 internal
faces differ, mean |Δw| 3.9e-3, max 0.157; the O-grid 7 920: mean 4.4e-3,
max 0.139; the dual h 4 mm: 32 136 of 33 636 differ, mean 1.8e-2, max
0.150. No port weight falls outside [0, 1]. The same copy is in
`outram-park-fork-coolprop` and `tampines-steam-tables`
(`openfoam_source/fv_operators/fvc/interpolate.rs`), not changed here.

**Predictions (written before the fix is run):**

| case | prediction |
|---|---|
| structured default 20 480 (34 steps, 0.9940, 1.0151) | **moves**, by less than 0.3 % on each (the faces that differ are the square/ring transition, mean Δw 4e-3); step count ±3 |
| dual h 4 mm, `Gauss linear`, measured-flow u_c, dp/dz (0.9820, 0.9616) | each moves by less than 0.01 |
| dual h 4 mm, `linearUpwind` + `cellLimited` (0.8897, 1.0196) | u_c up by 0–0.03: **not** the main cause. My leading explanation of its low u_c is now the limiter itself: at a smooth extremum (the centreline maximum of u_z) minmod `cellLimited` sets the limiter to 0 (the neighbours are all below the cell, so `max − U = 0`), the gradient vanishes, and `linearUpwind` falls back to first-order upwind on the axis cells, whose false diffusion flattens the peak. That is upstream's behaviour, not a port defect, and the unlimited `linearUpwind` (u_c 0.9597, dp/dz 1.0218 against its own flow, from the sweep's nominal numbers) shows the size of it |

**Results after the fix (2026-11-04; `linear_weight` is upstream's,
unit test `fvc::interpolate::tests::linear_weight_projects_on_the_face_normal_as_upstream`):**

| case | before | after | verdict |
|---|---|---|---|
| structured default 20 480 (nominal; `--headless`) | 34 steps, 0.9940, 1.0151, RMS 0.0063, τ_w 1.0063 | 34 steps, **0.9940, 1.0154**, RMS 0.0063, τ_w 1.0066 | held: moved by +0.03 % on dp/dz only |
| O-grid 7 920 | 27, 0.9905, 1.0171 | 27, 0.9905, 1.0173 | |
| dual h 4 mm, `Gauss linear`, nominal / own flow | 0.9418, 0.9414 / 0.9820, 0.9616 | 0.9397, 0.9353 / 0.9797, 0.9553 | held (moved < 0.01), slightly further |
| dual h 4 mm, `linearUpwind` unlimited, nominal / own flow | 0.9205, 1.0005 / — | 0.9188, 0.9852 / 0.9580, 1.0062 | |
| dual h 4 mm, `linearUpwind` + `cellLimited` (demo), nominal / own flow | 0.8534, 0.9983 / 0.8897, 1.0196 | **0.8540, 0.9915** / 0.8904, 1.0127 | held: not the cause |

The fix is kept because it is upstream's formula, not for what it does to
the numbers (which is little).

### Feature-edge capture: the chamfered rim at its source (coordinator's direction, 2026-11-04)

**Upstream read first.** cfMesh (`6cf1d211f9cf`)
`tetMeshGenerator::generateMesh` runs `mapMeshToSurface` (every boundary
vertex to the surface), then `mapEdgesAndCorners`
(`meshSurfaceEdgeExtractorNonTopo`: `edgeExtractor` distributes the
boundary faces into patches, then
`meshSurfaceMapper::mapVerticesOntoSurfacePatches`: a vertex of one patch
goes to the nearest point of that patch, a vertex between two patches onto
the feature edge between them (`mapEdgeNodes`), a vertex of three or more
to the nearest corner (`mapCorners`), each move capped by
`findMappingDistance`; then `correctEdgesBetweenPatches`), and only then
the boundary layers. The port's pipeline snapped the *carved* mesh and
then built the dual, whose boundary vertices are averages, and never
mapped again: hence the chamfer and the ~16-gon. Ported as
`outram_park_fork_cfmesh::surface_mapper::map_vertices_onto_surface_patches`
(the face distribution is the crate's nearest-triangle rule, not
`edgeExtractor`'s optimisation; `correctEdgesBetweenPatches` and the
surface untangler are not ported) and run in the multipatch pipeline
after the dual, before the layers (`TetDualOptions::feature_edges`, on by
default; `false` is the ablation). The single-patch entry points do not
run it.

**Predictions (written before the first run), pipe R 10 mm, 48 facets:**

| quantity | before | prediction |
|---|---|---|
| inlet projected area / πR², h 4 mm | 0.9396 | 0.99–0.998 (the 48-gon is 0.9971) |
| section area / πR², h 4 mm | 0.9796 | 0.99–0.998 |
| the stage is kept (mesh valid) at h 6 / 5 / 4 / 3.3 mm | — | kept at all four; if one is refused, it is a face with all its vertices on the rim (`correctEdgesBetweenPatches` not ported) |
| max non-orthogonality, bare h 4 mm | 51° | rises, 55–75° (wall cells stretched outward) |
| inward boundary faces with 1 layer, h 6 mm | 20 | fewer, 0–10: the tangled prisms sat on the chamfer |
| nominal u_c/2U, dp/dz at the demo default | 0.854, 0.992 | u_c 0.88–0.93 (the +6 % flow, but the `cellLimited` axis flattening stays), dp/dz 1.03–1.08 |

**Mesh results (2026-11-04, cfMesh's own 48-sided pipe, 200 mm;
`diagnose_inward_boundary_faces_on_layered_pipes`).** First run (mapping
only): inlet 0.9901 / 0.9936 / 0.9936 πR² at h 6 / 4 / 3.3 mm, section
0.998 / 0.9955 / 0.9977, the stage kept everywhere, **but** 2 inward
outlet faces at h 6 mm and 6 at 3.3 mm (zero-area faces whose every vertex
landed on the rim: the `correctEdgesBetweenPatches` case). So the mapping
now undoes itself locally (`map_vertices_onto_surface_patches_untangled`:
the boundary points of any cell left with a non-positive face pyramid go
back, repeated up to 10 passes) and the pipeline notes how many points
stay unmapped. Second run:

| h (mm), layers | inlet / πR² (before → after) | section / πR² | unmapped points | inward boundary faces (before → after) | max non-ortho (°) | negative pyramids, all faces |
|---|---|---|---|---|---|---|
| 6, 0 | 0.8656 → 0.9901 (outlet 0.9391) | 0.9547 → 0.9980 | 34 | 0 → 0 | 56.2 | 0 |
| 4, 0 (demo default) | 0.9396 → **0.9936** | 0.9795 → **0.9955** | 0 | 0 → 0 | 50.6 | 0 |
| 3.3, 0 | 0.9718 → 0.9833 | 0.9864 → 0.9977 | 126 | 0 → 0 | 57.4 | 0 |
| 6, 1 | | | 34 | **20 → 0** | 77.0 | 0 |
| 5, 1 | 0.8915 → 0.9872 | 0.9653 → 0.9911 | 0 | 31 → 0 | 74.7 | 0 |
| 5, 2 | | | 0 | 66 → 0 | 86.8 | **105 (internal)** |
| 4, 2 | | | 0 | 34 → 0 | 83.5 | 0 (1 602 before) |

Against the predictions: inlet and section areas held at h 4 mm (0.9936,
0.9955); held at 6 and 5 mm on the inlet, **missed at 3.3 mm** (0.9833,
where 126 points stayed unmapped) and on the 6 mm outlet (0.9391). The
stage is kept everywhere (held, after the local undo). Max non-orthogonality
did not rise (missed: 50.6° at h 4 mm). **The inward boundary faces are
gone on every layered pipe measured** (held, better than predicted): they
sat on prisms extruded from the chamfer, so step 2's fix is this port, not
a change in the layerer. Test:
`pipeline::tests::every_boundary_face_points_outward_on_layered_pipes_and_ducts`
(pipes h 6 / 4 mm with 1–2 layers and a 20 × 20 × 60 mm box duct with 1–2
layers: 0 inward faces). **Open:** on the coarse 24-sided test pipe of
`wall_only_layers_end_on_the_pipe_ends` (facet 2.6 mm under a 4 mm cell),
105 points stay unmapped, the boundary creases, and the layerer then
finds a 108° fold at every thickness and adds no layer; that test now
runs with `feature_edges: false`, its premise (the layer port on the
unmapped dual) kept and said so.

**Flow predictions on the mapped mesh (written before the runs), nominal
instrument, Re = 100:** `Gauss linear` at h 4 mm u_c/2U 0.97–0.99, dp/dz
0.96–1.00; unlimited `linearUpwind` u_c 0.95–0.97, dp/dz 1.02–1.05; the
demo pair (`cellLimited`) as in the table above (0.88–0.93, 1.03–1.08).
Layers (step 3): h 4 mm with 2 layers (no negative pyramid left) **runs**;
h 5 mm with 2 layers (105 internal negative pyramids) still diverges.

**Flow results on the mapped mesh (2026-11-04, Re = 100 nominal, demo
cylinder; steps, u_c/2U, dp/dz/HP, profile RMS, τ_w/HP; nominal
instrument):**

| h (mm), layers, cells | `Gauss linear` | `linearUpwind`, Gauss linear grad | `linearUpwind` + `cellLimited … 1` (demo pair) |
|---|---|---|---|
| 6, 0, 1 939 | 75, 0.9499, 0.9054, 0.0227, 0.9032 | 19, 0.9029, 1.0010, 0.0474, 0.9991 | 22, 0.8058, 1.0692, 0.0739, 1.0677 |
| 5, 0, 3 113 | 58, 0.9556, 0.9342, 0.0219, 0.9316 | 18, 0.9029, 1.0118, 0.0478, 1.0101 | 24, 0.9005, 0.9925, 0.0486, 0.9908 |
| **4, 0, 6 353 (default)** | 63, **0.9811, 0.9673**, 0.0126, 0.9638 | 27, **0.9598, 1.0210**, 0.0214, 1.0193 | 29, **0.8925, 1.0326**, 0.0356, 1.0312 |
| 3.3, 0, 11 177 | 54, 0.9669, 0.9632, 0.0161, 0.9596 | 33, 0.9661, 0.9932, 0.0198, 0.9911 | 37, 0.9606, 0.9838, 0.0214, 0.9802 |
| 6, 1, 6 713 | 76, 0.9453, 0.8882, 0.0222, 0.8840 | 20, 0.9000, 0.9809, 0.0476, 0.9763 | 23, 0.8071, 1.0565, 0.0733, 1.0514 |
| 5, 1, 10 745 | 63, 0.9440, 0.9011, 0.0262, 0.8978 | 21, 0.8954, 0.9704, 0.0509, 0.9677 | 28, 0.8954, 0.9705, 0.0509, 0.9678 |
| 4, 1, **18 321** (a layer now grows; before, the back-off gave none) | 95, 0.9748, 0.9511, 0.0152, 0.9418 | 74, 0.9522, 1.0015, 0.0248, 0.9932 | 73, 0.8907, 1.0301, 0.0372, 1.0215 |

Instruments at the default (`diagnose_pipe_instruments`): inlet Q now
**0.9936** U πR², cut section 0.9964 πR²; nominal and own-flow numbers now
agree to 0.3 % (demo pair 0.8925 / 1.0326 nominal, 0.8951 / 1.0317 own
flow; `Gauss linear` 0.9813 / 0.9673, 0.9841 / 0.9665; unlimited
`linearUpwind` 0.9597 / 1.0210, 0.9624 / 1.0201). The first bin reads
0.8883 against the fit's 0.8925 (no binning bias of note).

**Against the predictions:** `Gauss linear` u_c 0.981 (held, 0.97–0.99),
dp/dz 0.967 (held, 0.96–1.00); unlimited `linearUpwind` 0.960 (held,
low edge) / 1.021 (held); demo pair 0.893 (held, 0.88–0.93) / 1.033
(held, 1.03–1.08). **None of the three is within 2 % on both u_c and dp/dz
at the default.** The chamfer is fixed (the flow is right to 0.6 %), so
what is left is the discretisation on this mesh: the refinement series is
not monotone at 3.3 mm for `Gauss linear` (0.981 → 0.967), and the
schemes differ from each other by 2–9 % at h = 4 mm, i.e. the mesh is not
resolved to 2 % by any of them. Wall layers do not help at fixed h (they
lower `Gauss linear`'s dp/dz by 2–4 %).

**`cellLimited` revisited (coordinator: once the geometry is right),
against the tutorials, not by the numbers.** The tutorials that pair
`cellLimited Gauss linear 1` with `linearUpwind grad(U)` (`propeller`,
`misalignedDuct`, `motorBikeSteady`) are turbulent, high-Re snappy cases
where the limiter's job is to bound the gradient at sharp fronts; the
laminar, smooth tutorial on an unstructured mesh (`icoFoam/elbow`) uses
`Gauss linear` gradients with `Gauss limitedLinearV 1` convection, no
cell limiter. A minmod cell limiter clips the gradient to zero at every
smooth extremum, here the whole centreline, which turns `linearUpwind`
back into first-order upwind on the axis cells: the measured 7 % u_c
deficit against unlimited `linearUpwind` at h = 4 mm is that, and it
shrinks with h (0.961 against 0.966 at 3.3 mm), as a clipped-extremum
error should. The laminar-tutorial reading (no cell limiter for a smooth
laminar flow) is the one that matches this case; changing the demo on
that ground is a decision for the maintainer, so the demo keeps the pair
it was given until then, and the record shows all three.

### Step 3: two or more layers (2026-11-04)

**On the mapped mesh, 2 layers run; 3 diverge** (Re = 100, demo cylinder):

| h, layers, cells | demo pair: steps, u_c/2U, dp/dz, RMS, τ_w | `Gauss linear` |
|---|---|---|
| 6, 2, 11 487 | 39, 0.8079, 1.0883, 0.0729, 1.0653 | 80, 0.9483, 0.9029, 0.0222, 0.8902 |
| 5, 2, 18 377 | 39, 0.8983, 0.9892, 0.0500, 0.9862 | 76, 0.9485, 0.9131, 0.0254, 0.9096 |
| 4, 2, 30 289 | 45, 0.8926, 1.0280, 0.0365, 1.0262 | 80, 0.9754, 0.9516, 0.0157, 0.9482 |
| 5, 3, 26 009 | **diverged** | **diverged** |
| 4, 3, 42 257 | **diverged** | **diverged** |

So the earlier 2-layer divergence went with the chamfer (prediction
"h 5 mm, 2 layers still diverges": **missed**, it runs). The 3-layer
meshes have no negative volume, no non-positive pyramid and max
non-orthogonality 82–84° (no worse than the 2-layer ones that run), but
their **max skewness is 6.6 and 7.2** (2 layers: 2.4 and 4.4; bare and
1 layer: 0.2–0.8) and their minimum cell determinant 6.6e-3 (cfMesh's
48-sided pipe, `diagnose_inward_boundary_faces_on_layered_pipes`).
Upstream's mesh-quality limits for layer addition
(`etc/caseDicts/mesh/generation/meshQualityDict`, OpenFOAM-dev
`852527377c7e`, which snappyHexMesh applies while adding layers and
removes the layer locally where they fail): `maxInternalSkewness 4`,
`minTetQuality 1e-15` (no non-positive face pyramid),
`minDeterminant 0.001`, `maxNonOrtho 65` (relaxed for layers to 75 in the
tutorials). The port's layer back-off checked only closure, negative
volume and non-orthogonality < 90°.

**Fix:** the back-off now also requires max skewness ≤ 4, no
non-positive face pyramid and min determinant ≥ 0.001 (upstream's values;
the non-orthogonality bound stays < 90° because the dual itself sits at
50–57° and the tutorials' layers are allowed to 75°, so 65° would refuse
every layer; that choice is this crate's, and said so).

**Predictions (before the run):** the 3-layer meshes back off to thinner
layers with skewness ≤ 4 and then run; the 2-layer h 5 mm mesh loses its
105 bad pyramids; the 1-layer meshes are unchanged (skew ≤ 0.8); h 4 mm
with 2 layers (skew 4.37) backs off slightly. If skewness does not fall
with thickness, the layerer returns no layers and the pipeline says so,
and the divergence is then not fixed.

**Result: missed.** With those limits **every 2- and 3-layer pipe backed
off to no layer at all** (h 5 and 4 mm; the 1-layer ones unchanged): the
skewness did not fall with thickness. Reading why turned up a second
port defect. The crate's `checks::QualityReport::max_skewness` divides the
face-centre offset by the **cell-centre distance |d|**, while OpenFOAM's
`primitiveMeshTools::faceSkewness` divides it by the face's own extent in
the offset's direction, floored at 0.2 |d| (already ported in this
workspace as `outram_foam_mesh::mesh_quality::face_skewness`). For a thin
prism stacked on another, |d| is the layer thickness and the face is a
wall-face wide, so the port's number scales like width/thickness and no
amount of thinning brings it under 4. The doc claim "OpenFOAM flags > 4"
held only for upstream's metric.

**Prediction for the corrected metric (before the run):** the 3-layer
meshes' max skewness falls to 0.5–3 and they are accepted with the full
thickness again; whether they then run is open: if they still diverge, the
skewness was not the cause and the next suspects are the layer cells'
aspect ratio against the explicit non-orthogonal correction.

**Result:** held on the metric (3-layer max skewness 0.98–1.01 with
upstream's definition; the 2-layer h 5 mm mesh backs off slightly and
loses its 105 bad pyramids), and the 3-layer meshes are the same meshes as
before, so **they still diverge: skewness was not the cause.**

**Isolation of the 3-layer divergence (h 5 mm, 3 layers, 26 009 cells,
demo pair, Re = 100, `--headless-unstructured 5 3 0.3 1.2 100`):**

| change | result |
|---|---|
| none | diverged |
| `FOAM_LAPLACIAN=uncorrected` (no explicit non-orthogonal term) | diverged |
| `FOAM_NON_ORTH=0` | diverged |
| Re = 20 | diverged |
| viscous-number cap 0.2 (default 0.5) | diverged |
| viscous-number cap 0.1 | (not finished in 25 min) |
| viscous-number cap 0.05 (Δt 10× smaller) | did not diverge in 15 min (killed by the time limit, not steady) |

So it is temporal after all, and not the non-orthogonal correction.
Instrumented (a temporary print of each linear solve's iterations and
residual): the pressure PCG converges throughout (60–170 iterations to
1e-8) until the end; the **momentum predictor's Gauss–Seidel needs 33
sweeps at first, then 100, 190, 716, then hits its 1 000-sweep cap
unconverged (residual 2e-2 … 2.0)**, and three steps later the pressure
solve blows up. The port solves `UEqn` **cold-started from U = 0**
(`FvVectorMatrix::solve`), so every step it must rebuild the whole field
by Gauss–Seidel sweeps; on the stretched layer cells (aspect 5–7.5,
first layer 0.04–0.08 mm) the sweep count to 1e-7 grows past the cap.
OpenFOAM solves `UEqn == -grad(p)` for U **starting from the current U**
(every `fvMatrix::solve` iterates on `psi` in place), so its smoother only
has to remove the change over one step.

**Fix (prediction written before the run):** warm-start the momentum
solve from the current U (`FvVectorMatrix::solve_with_guess`, used by
`PimpleFoam`'s predictor). Predicted: the 3-layer meshes run to steady at
the default Δt; Gauss–Seidel sweeps per step stay below 100; the
structured default moves by at most the solver tolerance (u_c, dp/dz to
4 digits unchanged, 34 ± 1 steps).

**Result: the structured default is unchanged (34 steps, 0.9940, 1.0154,
RMS 0.0063, τ_w 1.0066), but the 3-layer meshes still diverge: the
prediction missed, and the sweep counts were a symptom.** With the warm
start, the largest |U| per step was 3.9e-3, 1.5e-2, 0.146, 0.30, 0.80,
52 m/s (U = 5e-3 m/s) before the Gauss–Seidel cap was ever reached, and
from the second step on it sat in **one cell**: a shrunk dual cell next to
the layers near the inlet rim (r 9.29 mm, z 2.0 mm, 9 faces, 4.9e-9 m³),
growing radially. The warm start is kept (it is upstream's behaviour and
changes no recorded number), but it is not the fix.

The cells with the worst non-orthogonality on these meshes are exactly
the shrunk dual cells over the layer stack (`diagnose_layer_quality_per_layer`:
max non-orthogonality in the layers 54°, at the top-layer/dual interface
75–84°). snappyHexMesh adds layers against its *relaxed* quality controls,
`maxNonOrtho 75` (the tutorials' `snappyHexMeshDict` `relaxed` block,
used while layers are added), and collapses the layer where that fails;
the port allowed anything under 90°.

**Fix (prediction before the run):** the layer back-off uses upstream's
relaxed `maxNonOrtho 75`. Thinner layers bring the prism centres toward
the wall and the interface angle down, so the back-off should find a
thickness: predicted max non-orthogonality ≤ 75° on all layered pipes,
the 3-layer pipes run to steady, the layers end thinner (first layer
0.02–0.06 mm).

**Result: missed, and reverted.** With the 75° bound **every 2- and
3-layer pipe, and even the 1-layer h 4 mm pipe, backed off to no layer at
all**: the angle at the top-prism/dual face does not fall with a uniform
thinning (it is set by how far the dual cell's centre sits sideways from
its wall face, which the bare mesh already has as boundary
non-orthogonality). snappyHexMesh meets the bound by collapsing the layer
*locally*; this layerer's prism columns span the whole wall, so it cannot.
The bound stays < 90°, as before. **Status of step 3: 1 and 2 layers run
at every h measured; 3 layers diverge**, the instability growing in one
shrunk dual cell over the layer stack near the inlet rim. What would fix
it, both upstream practice and not ported: local layer collapse
(snappy `addLayers`' per-face termination) or cfMesh's
`refineBoundaryLayers` + `meshOptimizer::untangleMeshFV` after the layers.
Not tried: a smaller time step for 3 layers (a viscous-number cap of 0.05
did not diverge in 15 min, but did not finish either), which would hide
the cell, not fix it.

**Per-layer quality of the layered pipes (2026-11-04, cfMesh's 48-sided
pipe, 200 mm, `diagnose_layer_quality_per_layer`; OpenFOAM's skewness and
aspect ratio; requested first layer 0.3 mm, ×1.2):**

| h, layers, cells | region | max non-ortho (°) | max skewness | max aspect ratio | mean thickness |
|---|---|---|---|---|---|
| 5, 1, 10 745 | dual / layer 1 | 74.7 / 74.7 | 1.14 / 0.85 | 2.7 / 2.4 | — / 0.290 mm |
| 5, 2, 18 377 | dual / layer 1 / layer 2 | 79.4 / 54.0 / 79.4 | 1.00 / 0.80 / 0.80 | 2.4 / 5.2 / 4.6 | — / 0.082 / 0.099 mm |
| 5, 3, 26 009 | dual / 1 / 2 / 3 | 81.7 / 53.9 / 54.2 / 81.7 | 1.01 / 0.80 / 0.80 / 0.80 | 2.5 / 7.1 / 6.3 / 5.5 | — / 0.053 / 0.064 / 0.076 mm |
| 4, 1, 18 321 | dual / layer 1 | 78.2 / 78.2 | 1.38 / 1.38 | 4.6 / 2.4 | — / 0.284 mm |
| 4, 2, 30 289 | dual / 1 / 2 | 83.5 / 54.2 / 83.5 | 0.98 / 0.59 / 0.74 | 3.0 / 5.5 / 4.9 | — / 0.070 / 0.084 mm |
| 4, 3, 42 257 | dual / 1 / 2 / 3 | 84.4 / 54.1 / 54.3 / 84.4 | 0.98 / 0.59 / 0.59 / 0.63 | 3.0 / 7.5 / 6.7 / 6.0 | — / 0.043 / 0.051 / 0.061 mm |

(A face counts for both cells it joins, so the top layer and the dual
share the interface's angle.) Inside the stack the layers are good
(54°, skewness < 1, aspect ≤ 7.5). The bad faces are the interface to the
dual (75–84°). With 2 or 3 layers the total thickness is capped at half
the shortest wall edge at each point (upstream's `boundaryLayers` rule,
`boundaryLayersCreateVertices.C`: penetration distance 0.5 × the nearest
neighbour), so the first layer comes out 0.04–0.08 mm, not the 0.3 mm
asked; one layer is not capped (0.29 mm).

### Layers against no layers at matched cell count (step 3, second half)

**Predictions (written before the run; from the earlier, pre-warm-start
rows, which these runs repeat on the current code):** wall layers do
**not** improve τ_w or dp/dz at matched cell count on this mesh family:
the pairs below differ by less than 3 % in τ_w/HP and dp/dz/HP, in either
direction, and the bare mesh at the smaller h is the closer one. Reason:
at h = 4–6 mm the error is in the core (the profile shape), not in the
wall gradient, and a layer costs a whole wall's worth of cells that the
bare mesh spends on the core.

**Results (2026-11-04, current code: weights, feature-edge capture, layer
quality limits, warm-started momentum; Re = 100; nominal instrument;
`FOAM_SWEEP=matched`):**

| cells | mesh | demo pair: steps, u_c/2U, **dp/dz/HP**, RMS, **τ_w/HP** | `Gauss linear`: steps, u_c/2U, **dp/dz/HP**, RMS, **τ_w/HP** |
|---|---|---|---|
| 6 353 | h 4 mm, no layer (**default**) | 29, 0.8925, **1.0326**, 0.0356, **1.0312** | 63, 0.9811, **0.9673**, 0.0126, **0.9638** |
| 6 713 | h 6 mm, 1 layer | 23, 0.8071, 1.0565, 0.0733, 1.0514 | 76, 0.9453, 0.8882, 0.0222, 0.8840 |
| 11 177 | h 3.3 mm, no layer | 37, 0.9606, **0.9838**, 0.0214, **0.9802** | 54, 0.9669, **0.9632**, 0.0161, **0.9596** |
| 10 745 | h 5 mm, 1 layer | 28, 0.8954, 0.9705, 0.0509, 0.9678 | 63, 0.9440, 0.9011, 0.0262, 0.8978 |
| 11 487 | h 6 mm, 2 layers | 39, 0.8079, 1.0883, 0.0729, 1.0653 | 80, 0.9483, 0.9029, 0.0222, 0.8902 |
| 18 321 | h 4 mm, 1 layer | 73, 0.8907, 1.0301, 0.0372, 1.0215 | 95, 0.9748, 0.9511, 0.0152, 0.9418 |
| 18 377 | h 5 mm, 2 layers | 117, 0.8990, 0.9946, 0.0500, 0.9785 | 93, 0.9509, 0.9225, 0.0253, 0.9088 |

The warm start changed none of the rows it shares with the earlier table
to the printed digits (the h 5 mm, 2-layer mesh is new: the pyramid gate
thinned it).

**Against the prediction.** "Layers do not improve τ_w or dp/dz at matched
count": **held**. At ~6.5k and ~11k cells the bare mesh is closer to
Hagen–Poiseuille in τ_w and dp/dz under both schemes; at ~18k, h 5 mm
with 2 layers beats h 4 mm with 1 under the demo pair (τ_w 0.979 against
1.022) and loses under `Gauss linear` (0.909 against 0.942). "Within 3 %
of each other": **missed**: under `Gauss linear` the layered meshes read
τ_w 6–8 % lower than the bare one at the same count, and under the demo
pair the 6 mm meshes read 2–8 % higher. Adding layers at fixed h *does*
move τ_w (h 6 mm `Gauss linear`: 0.903 bare → 0.884 one layer → 0.890
two), but not toward 1: the layers resolve a wall gradient that is
already right to a few per cent, while the coarse core sets the error.
**On this mesh family wall layers are not the lever for the remaining
gap**; they are offered in the demo because they run (1–2 layers), not
because they help here.

### Mesh images after this work (drawn from the `PolyMesh` the solver gets)

New in `laminar_pipe_gh1155/unstructured_gh1202/` (2026-11-04,
`--render-unstructured`; every older image there was regenerated from the
current mesher too, and now shows the mapped rim):
`mapped_h4_bare_*` (the demo default, 6 353 cells),
`mapped_h4_2layers_*` (30 289), `mapped_h4_3layers_*` (42 257), each as
`xy` (mid-length), `xz_inlet`, `xz_corner` (inlet rim) and `xy_wall` (a
3.5 mm wall zoom showing the layer stack).

**Checked by eye:** `mapped_h4_bare_xz_corner.png`: the rim is a square
corner at (1.0, 0) cm, the inlet flat at z = 0 out to the wall, the wall
straight at x = 1.0 cm down to the inlet (before: a 2 mm × 5 mm chamfer,
`unstructured_default_7k_xz_corner.png` in its pre-2026-11-04 version).
`mapped_h4_3layers_xy_wall.png`: three thin prism layers along the wall,
total about 0.15 mm, under large dual cells (about 1.2 mm across); the
layer faces meet the dual cells at a visibly oblique angle where a dual
cell's centre sits sideways of its wall face, which is the 75–84°
interface non-orthogonality of the table above. **Not checked:** the
layer stack at the rim in 3-D (the slices cut it in two planes only). **The structured default moved:**
~~dp/dz 1.0151~~ **1.0154**; u_c 0.9940 and the 34 steps are unchanged.

### Mesh images (drawn from the `PolyMesh` the solver gets)

In `laminar_pipe_gh1155/unstructured_gh1202/`, written by
`cargo run --release -p dhoby-ghaut --example foam_studio_web -- --render-unstructured`,
one colour per cell:

| file | what |
|---|---|
| `unstructured_h5_1layer_wall_only_{xy,xz_inlet,xz_corner}.png` | h 5 mm, 1 layer, wall only (10 745 cells): cross-section at mid-length, x-z slice of the first 4.4 cm, zoom on the inlet corner — **after** |
| `unstructured_h5_1layer_whole_boundary_{xy,xz_inlet,xz_corner}.png` | the same settings with layers on the whole boundary (the pre-gh:#1202 behaviour) — **before** |
| `unstructured_default_7k_{xy,xz_inlet,xz_corner}.png` | the 6 713-cell V&V mesh (h 6 mm, 1 layer; ~~the demo default~~ until the convection fix, after which the default is the bare h 4 mm mesh, whose cells are those of the "after" images without the wall column) |

**Checked by eye (2026-11-04):** in the corner zooms, *after* has a thin
layer column along the wall (x ≈ 0.98–1.0 cm) that stops at z = 0 with no
layer on the inlet; *before* has the same wall column plus a row of thin
cells along the whole inlet (z = 0–0.03 cm). The cell counts differ by
336 = 2 × 168, the end faces' prisms. The cross-section is a ~16-sided
polygon of radius 1 cm with 4–5 large polyhedra across and a ring of thin
wall prisms; the requested 0.3 mm first layer came out about 0.2 mm (the
layerer's cap at half the local wall-edge length and its validity
back-off). **Not checked:** the faces between the cut planes; the patch
assignment was checked by the test (`inlet` / `outlet` / `walls`, walls
typed `wall`), not drawn.

### Unresolved (gh:#1202 follow-ups)

**UPDATED 2026-11-04 (end of the merge work).** Status of each item, and
what is still open:

- ~~The dual mesh's flat profile and high dp/dz~~: the upwind false
  diffusion (fixed by second-order convection) and the chamfered inlet
  (fixed by the feature-edge capture) explained most of it. **Still open:
  a few-per-cent gap at the demo default** (nominal instrument, the bar's:
  u_c/2U **0.8925**, dp/dz **1.0326** with the demo's `linearUpwind` +
  `cellLimited`; 0.9811 / 0.9673 with `Gauss linear`; 0.9597 / 1.0210
  with unlimited `linearUpwind`). The 2 % bar on both is **not met** by
  any of them. The `cellLimited` choice is explained against the
  tutorials above (a laminar smooth flow is the `icoFoam/elbow` case,
  which has no cell limiter); changing it is the maintainer's call.
  Comparing against OpenFOAM on an exported mesh (`export_polymesh`) is
  the next check that would separate the port from the mesh.
- ~~2+ layers diverge; 1 layer runs.~~ **1 and 2 layers run; 3 diverge**
  (one shrunk dual cell over the layer stack, 75–84° interface); ~~local
  layer collapse is not ported.~~ **UPDATED 2026-11-04 (gh:#1215):** both
  upstream repairs are ported and compared (section above): 3 layers run
  under cfMesh's optimiser at h 5 and 4 mm; snappy's collapse removes every
  layer. The 75–84° is inherited from the bare dual's wall faces (several
  per wall cell, median 69° to the owner centre), which the default
  back-off keeps.
- ~~No feature-edge capture~~: ported (`surface_mapper`), without
  `correctEdgesBetweenPatches` (points that would land a face on the rim
  stay unmapped: 34 at h 6 mm, 126 at 3.3 mm, 0 at the default).
- ~~19 inward boundary faces~~: 0 on every layered pipe and duct measured
  (test `every_boundary_face_points_outward_on_layered_pipes_and_ducts`).
- ~~`limited` snGrad with a coefficient in (0, 1) is not ported.~~ Ported
  2026-11-04. ~~`cellLimited` gradients and `linearUpwind` /
  `limitedLinearV` convection, which the tutorials use, were not tried
  here.~~ `linearUpwind` and `cellLimited` ported and measured;
  `limitedLinearV` (elbow) is not ported.
- Port defects found and fixed on the way: the linear interpolation
  weights (`fvc::linear_weight`, now upstream's `makeWeights`), cfMesh's
  skewness metric (now `faceSkewness`), the cold-started momentum solve
  (now warm-started, as upstream). The same weight defect is in
  `outram-park-fork-coolprop` and `tampines-steam-tables`'
  `openfoam_source/fv_operators/fvc/interpolate.rs`, not changed here.
- **Structured default re-checked after all of it:** 34 steps, u_c/2U
  0.9940, ~~dp/dz 1.0151~~ **1.0154** (the weights fix), RMS 0.0063,
  τ_w 1.0066.

## Layer quality: cfMesh thinning against snappyHexMesh collapse, differentially (gh:#1215)

**Issue:** gh:#1215, the follow-up of gh:#1202's step 3 (3 layers diverge,
one shrunk dual cell over the layer stack, 75–84° at the top-prism/dual
interface; uniform thinning does not lower it). **Maintainer direction:**
port both upstream mechanisms and test them differentially against the
crate's own global back-off on the same inputs.

### What was ported (2026-11-04)

| strategy (`TetDualOptions::layer_strategy`) | module | upstream ported (file, commit) |
|---|---|---|
| `BackOffOnly` (**default**, unchanged) | `layers::add_boundary_layers_back_off` | this crate's own: every column at the per-point cap, then all scaled by 0.6 until upstream's limits pass |
| `CfMeshOptimise` | `boundary_layer_optimisation`, `mesh_optimizer` | cfMesh v1.1 (`github.com/DAFoam/cfmesh` `e944f6ba695f`; `boundaryLayerOptimisation` is not in the `wyldckat/cfMesh` v1.0 the rest of the crate cites): `boundaryLayerOptimisation::optimiseLayer` (hair normals at the boundary and inside, `nSmoothNormals 5`; `optimiseThicknessVariation` with `relThicknessTol 0.15`, `featureSizeFactor 0.3`, `maxNumIterations 5`), `meshOptimizer::untangleMeshFV`, `optimizeLowQualityFaces`, `optimizeBoundaryLayer`, `untangleBoundaryLayer`, `partTetMesh`, `volumeOptimizer`, `knuppMetric`, `polyMeshGenChecks::findBadFaces` / `findLowQualityFaces`, in `tetMeshGenerator`'s order (optimise one layer, then split it) |
| `SnappyCollapse` | `snappy_layers`, `checks::faces_failing_mesh_quality` | OpenFOAM-dev `852527377c7e`: `snappyLayerDriver` `extrudeMode`, `handleNonManifolds`, `handleFeatureAngle`, `growNoExtrusion`, `truncateDisplacement`, `setupLayerInfoTruncation`, `checkAndUnmark`, the `nLayerIter` / `nRelaxedIter` loop; `meshCheck::checkMesh` with `meshQualityDict`'s values (65° strict, 75° relaxed, skewness 4 / 20, tet sign, twist 0.02, determinant 0.001, face weight 0.05, volume ratio 0.01) |

Shared: `layers::build_layered_variable`, a per-point layer count (0 at a
collapsed point; a face takes the most of its points, `addPatchCellLayer`'s
rule; cells collapse to wedges where a layer ends).

**Not ported, and why it matters to the comparison.** cfMesh: the LP
`meshUntangler`, boundary-point moves in `untangleMeshFV`,
`meshSurfaceOptimizer` inside `optimiseLayer`, the Laplace pass of
`optimizeMeshFV`. snappy: `medialAxisMeshMover` (only the wall points move
here, so the per-point cap, half the shortest wall edge, stands in for the
medial-axis limit), `handleWarpedFaces` (runs only with `relativeSizes`),
`maxConcave`, the butterfly strings. Upstream's `handleFeatureAngle` only
looks at edges between *two* layer faces, so it cannot stop layers at the
wall/inlet rim (an edge of one layer face), whatever `featureAngle` is;
the brief's expectation that it would is not upstream's behaviour.

**Reuse decision.** `outram-foam-mesh`'s `snappy_hex_mesh/layers.rs` was
read in full first. It cites `snappyLayerDriver.C:217` for "the
`EXTRUDE`/`NOEXTRUDE` unextrusion loop", but its code reduces the layer
count *globally* (`for n in (1..=n_surface_layers).rev()`, interior insert
then outward extrusion) and its own "Honest scope" says "no per-face layer
termination"; it has no per-point mode. What it does share with cfMesh's
layerer is the grading arithmetic (`layer_thicknesses`), which this crate
already had. It works on `outram_foam_basic_lib` `PolyPatchMesh` /
`CastellatedMesh`; this crate builds without that dependency by default.
So there was nothing to factor out, and the collapse loop was ported here
(its doc comment cites both). The overclaim in that module's header was
corrected with a dated note.

**Found on the way: the tet-dual pipeline was not reproducible between
processes.** `delaunay::flip_to_delaunay` swept faces in `HashMap` order,
seeded per process: three runs of the h 6 mm pipe gave three meshes (same
cell count). gh:#1202's `tet_dual_stages_are_deterministic` compares two
calls inside one process and could not see it. The sweep is now in
`BTreeMap` order (`smooth`'s neighbour sets too); the fingerprints of the
h 4 mm and box-duct meshes did not change, the h 6 mm ones did. Every
gh:#1202 number on an h 6 mm mesh (and any other mesh whose flips depend on
the order) came from one draw of that randomness.

### Predictions (written 2026-11-04, before any differential run)

Inputs: the demo pipe (R 10 mm, L 200 mm, 48 facets, wall-only layers,
first 0.3 mm, × 1.2) at h 4 / 5 / 6 mm with 1–4 layers; a tet-dual box duct
20 × 20 × 200 mm at h 5 mm with 2 and 3 layers (its four 90° long edges are
the sharp-feature case, run also with snappy's `featureAngle 60` of the
`propeller` tutorial); a Cartesian (hexahedral) duct for the sanity test.

| quantity | `BackOffOnly` | `CfMeshOptimise` | `SnappyCollapse` |
|---|---|---|---|
| layer coverage (wall faces with all N layers) | 100 % at 1–3 layers, as gh:#1202 (or 0 where the back-off finds nothing) | **100 % always** (cfMesh never removes a layer) | 1 layer: 60–95 %; 2–3 layers: **0–50 %**: I expect `minFaceWeight 0.05` and `minVolRatio 0.01` at the top-prism/dual face (layer 0.05–0.08 mm against dual cells 1–3 mm deep), not the non-orthogonality, to be what collapses most points |
| collapsed wall points | 0 | 0 | the complement: 50–100 % at 2–3 layers |
| total thickness (mean, layered points) | as gh:#1202 (≈ 0.15 mm at 3 layers, h 5 mm) | 0.8–1.0 × the back-off's: on a pipe the inward hairs converge only by 7.5° per facet, so `featureSizeFactor` binds nowhere and `relThicknessTol 0.15` only near the rim | the back-off's where kept (same cap, no global scaling) |
| max non-orthogonality | 82–84° (3 layers) | **lower, 65–78°**: `optimizeLowQualityFaces` targets every face above 65° by moving the dual points around it | ≤ 75° when converged (the relaxed bound), else as the back-off |
| faces above 65° / 75° | hundreds / tens | fewer than the back-off, not zero | 0 above 75° on layer cells when converged |
| max skewness (OpenFOAM) | ≈ 1 | ≈ 1 | ≈ 1 |
| min determinant (crate's) | 6.6e-3 at 3 layers | similar or higher | higher (bad cells removed) |
| bad faces (cfMesh `findBadFaces`) after | 0 | 0 | — |
| converges / steps (Re 100, demo pair), 3 layers | **diverges** (gh:#1202) | **runs** at h 5 mm (55 % confidence): the shrunk dual cell is reshaped | **runs** (70 %): the layers at the bad cells are gone |
| u_c/2U, dp/dz/HP, τ_w where a run converges | as gh:#1202 for 1–2 layers | within ±2 % of the back-off at the same h and N (the core sets the error, gh:#1202) | within ±2 % on u_c and dp/dz; τ_w within ±5 % (fewer layers where collapsed) |
| determinism | identical across runs | identical | identical |
| meshing time | 0.2–2 s | 2–20× the back-off (tet-region volume optimiser) | ≤ nLayerIter × the back-off's build + check; 1–20 s |
| Cartesian duct (sanity) | full layers | interior points bit-identical, thickness on the side middles unchanged | first iteration accepted, mesh identical to the back-off's |
| tet-dual duct, `featureAngle` 100 vs 60 | — | — | 100: no edge flagged (90° edges); 60: the four long edges' points unextruded at the start |

Where A and B should differ, and why: A keeps every layer and moves
interior points; B keeps the geometry and removes layers. So A wins on
coverage and B on worst-face quality; on the solve both should remove the
3-layer divergence if its cause is the one shrunk cell, and neither changes
the Hagen–Poiseuille accuracy by more than the 2 % the core's resolution
already costs.

### Methodology (as run)

- **Mesh level:** `outram-park-fork-cfmesh`
  `pipeline::tests::diagnose_layer_strategies_differential` (`--ignored`;
  `CFMESH_CASE` filters, `CFMESH_DET=1` adds a second run per case and
  compares fingerprints): cfMesh's own 48-sided pipe and the tet-dual duct
  through the multipatch pipeline under each strategy. Quality is the
  crate's `check_quality` (OpenFOAM `faceSkewness`, the crate's
  determinant) plus counts of faces failing snappy's `meshQualityDict`
  (`checks::faces_failing_mesh_quality`, strict 65° / relaxed 75°), and,
  for the back-off's full-layer meshes, which single check the layer
  cells' faces fail. Times are native release on the shared 4-core host,
  **with other jobs running** (the build and the solves below), so they are
  upper bounds; the clean h 5 mm, 3-layer run took 0.77 s / 47.3 s / 0.67 s.
- **Solve level:** Foam Studio `--headless-unstructured h n 0.3 1.2 100`
  with `FOAM_LAYER_STRATEGY=backoff|cfmesh|snappy` (the demo's cylinder,
  48 facets; the demo pair `linearUpwind grad(U)` + `cellLimited Gauss
  linear 1`, 2 correctors), Re = 100, to the steady criterion; nominal
  instruments, as the gh:#1202 rows.
- **Sanity:** `pipeline::tests::every_strategy_leaves_a_good_box_duct_alone`
  (Cartesian duct). **Determinism:** `layer_strategies_are_deterministic`
  (two calls, and fingerprints recorded once and asserted, so a different
  process must give the same bits), `back_off_only_layers_are_pinned_bit_for_bit`,
  and `CFMESH_DET=1` on every case of the h 6 mm rows (all `det true`).

### Results (2026-11-04)

**Mesh level** (pipe R 10 mm, L 200 mm; requested first layer 0.3 mm,
× 1.2; "first" and "total" are means over the wall points that carry the
full count; NO = non-orthogonality; fails = faces failing snappy's strict
/ relaxed controls; coverage = wall faces with all N layers):

| case | strategy | cells | time (s) | max NO (°) | > 65° / > 75° | max skew | min det | fails strict / relaxed | coverage | first / total (mm) |
|---|---|---|---|---|---|---|---|---|---|---|
| h 6, 1 | BackOff | 6 713 | 1.0 | 77.0 | 1 956 / 260 | 1.88 | 9.5e-3 | 2 116 / 424 | 1.000 | 0.299 / 0.299 |
| | CfMesh | 6 713 | 186 | 73.4 | 6 / 0 | 0.85 | 0.167 | 18 / 12 | 1.000 | 0.960 / 0.960 |
| | Snappy | 2 195 | 0.9 | 61.1 | 0 / 0 | 0.89 | 5.1e-2 | 6 / 6 | 0.054 (98.7 % of points collapsed) | 0.300 / 0.300 |
| h 6, 2 | BackOff | 11 487 | 0.8 | 84.2 | 2 308 / 1 112 | 2.04 | 2.2e-3 | 3 924 / 2 738 | 1.000 | 0.149 / 0.328 |
| | CfMesh | 11 487 | 203 | 74.9 | 18 / 0 | 1.19 | 5.5e-2 | 36 / 18 | 1.000 | 0.440 / 0.968 |
| | Snappy | 1 939 | 0.9 | 56.2 | 0 / 0 | 0.89 | 0.141 | 6 / 6 | 0 | — |
| h 6, 3 | BackOff | 1 939 (**no layer**: the back-off found none in 10 tries) | 1.4 | 56.2 | 0 / 0 | 0.89 | 0.141 | 6 / 6 | 0 | — |
| | CfMesh | 16 261 | 212 | 75.8 | 24 / 4 | 1.47 | 2.3e-2 | 74 / 54 | 1.000 | 0.267 / — |
| | Snappy | 2 419 | 1.0 | 63.0 | 0 / 0 | 0.89 | 0.141 | 6 / 6 | 0 (97.5 % collapsed; 118 points kept, tapered) | — |
| h 6, 4 | BackOff | 1 939 (no layer) | 1.7 | 56.2 | 0 / 0 | 0.89 | 0.141 | 6 / 6 | 0 | — |
| | CfMesh | 21 035 | 171 | 76.3 | 134 / 4 | 1.64 | 1.1e-2 | 810 / 684 | 1.000 | 0.181 / — |
| | Snappy | 1 939 | 0.8 | 56.2 | 0 / 0 | 0.89 | 0.141 | 6 / 6 | 0 | — |
| h 5, 1 | BackOff | 10 745 | 0.4 | 74.7 | 4 672 / 0 | 1.14 | 5.5e-2 | 6 072 / 1 400 | 1.000 | 0.294 / 0.294 |
| | CfMesh | 10 745 | 27 | 63.4 | 0 / 0 | 0.77 | 0.132 | 0 / 0 | 1.000 | 0.765 / 0.765 |
| | Snappy | 3 113 | 0.6 | 47.0 | 0 / 0 | 0.84 | 0.203 | 152 / 152 | 0 | — |
| h 5, 2 | BackOff | 18 377 | 0.7 | 79.4 | 4 720 / 3 088 | 1.00 | 1.7e-2 | 7 496 / 5 864 | 1.000 | 0.077 / — |
| | CfMesh | 18 377 | 46 | 65.1 | 8 / 0 | 0.80 | 0.104 | 1 248 / 1 240 | 1.000 | 0.349 / 0.768 |
| | Snappy | 3 113 | 0.7 | 47.0 | 0 / 0 | 0.84 | 0.203 | 152 / 152 | 0 | — |
| h 5, 3 | BackOff | 26 009 | 0.7 | 81.7 | 4 720 / 3 911 | 1.01 | 6.8e-3 | 7 792 / 6 983 | 1.000 | 0.048 / 0.176 |
| | CfMesh | 26 009 | 48 | 66.2 | 24 / 0 | 0.80 | 4.6e-2 | 1 272 / 1 248 | 1.000 | 0.211 / 0.769 |
| | Snappy | 3 113 | 0.7 | 47.0 | 0 / 0 | 0.84 | 0.203 | 152 / 152 | 0 | — |
| h 5, 4 | BackOff | 33 641 | 0.9 | 82.1 | 4 720 / 4 016 | 1.01 | 3.3e-3 | 8 644 / 7 988 | 1.000 | 0.033 / 0.176 |
| | CfMesh | 33 641 | 85 | 66.7 | 32 / 0 | 0.81 | 2.4e-2 | 1 280 / 1 248 | 1.000 | 0.143 / 0.769 |
| | Snappy | 3 113 | 0.7 | 47.0 | 0 / 0 | 0.84 | 0.203 | 152 / 152 | 0 | — |
| h 4, 1 | BackOff | 18 321 | 0.8 | 78.2 | 6 320 / 3 464 | 1.38 | 5.1e-3 | 7 782 / 4 926 | 1.000 | 0.286 / 0.286 |
| | CfMesh | 18 321 | 722 | 66.9 | 16 / 0 | 0.60 | 0.108 | 76 / 60 | 1.000 | 0.624 / 0.624 |
| | Snappy | 6 353 | 1.4 | 50.6 | 0 / 0 | 0.84 | 0.142 | 192 / 192 | 0 | — |
| h 4, 2 | BackOff | 30 289 | 1.3 | 83.5 | 6 736 / 5 872 | 0.98 | 1.7e-2 | 11 848 / 10 984 | 1.000 | 0.064 / 0.141 |
| | CfMesh | 30 289 | 772 | 69.6 | 16 / 0 | 0.62 | 9.1e-2 | 1 406 / 1 390 | 1.000 | 0.285 / 0.627 |
| | Snappy | 6 353 | 1.1 | 50.6 | 0 / 0 | 0.84 | 0.142 | 192 / 192 | 0 | — |
| h 4, 3 | BackOff | 42 257 | 1.6 | 84.4 | 6 752 / 6 288 | 0.98 | 6.6e-3 | 12 456 / 12 008 | 1.000 | 0.039 / 0.141 |
| | CfMesh | 42 257 | 853 | 70.5 | 16 / 0 | 0.62 | 3.8e-2 | 1 762 / 1 746 | 1.000 | 0.172 / 0.627 |
| | Snappy | 6 353 | 1.1 | 50.6 | 0 / 0 | 0.84 | 0.142 | 192 / 192 | 0 | — |
| h 4, 4 | BackOff | 54 225 | 1.8 | 84.8 | 6 752 / 6 304 | 0.98 | 3.2e-3 | 13 048 / 12 608 | 1.000 | 0.026 / 0.141 |
| | CfMesh | 54 225 | 720 | 70.9 | 16 / 0 | 0.65 | 1.9e-2 | 1 852 / 1 836 | 1.000 | 0.117 / 0.627 |
| | Snappy | 6 353 | 1.1 | 50.6 | 0 / 0 | 0.84 | 0.142 | 192 / 192 | 0 | — |
| duct h 5, 2 | BackOff | 19 281 | 0.6 | 80.2 | 2 560 / 2 560 | 0.88 | 7.3e-2 | 2 560 / 2 560 | 1.000 | 0.147 / 0.323 |
| | CfMesh | 19 281 | 24 | 62.7 | 0 / 0 | 0.58 | 0.148 | **0 / 0** | 1.000 | 0.332 / 0.731 |
| | Snappy (`featureAngle` 100 and 60) | 3 921 | 0.6 | 25.6 | 0 / 0 | 0.69 | 0.304 | 0 / 0 | 0 | — |
| duct h 5, 3 | BackOff | 26 961 | 0.8 | 80.4 | 2 560 / 2 560 | 0.77 | 1.8e-2 | 7 632 / 7 632 | 1.000 | 0.057 / 0.207 |
| | CfMesh | 26 961 | 24 | 64.9 | 0 / 0 | 0.56 | 0.118 | **0 / 0** | 1.000 | 0.202 / 0.734 |
| | Snappy (100 and 60) | 3 921 | 0.6 | 25.6 | 0 / 0 | 0.69 | 0.304 | 0 / 0 | 0 | — |

Which of snappy's checks the back-off's full-layer meshes fail, counted on
faces of layer cells, one check at a time (the reason snappy collapses):

| case | NO > 65 | NO > 75 | tet sign | twist | det | face weight | vol ratio |
|---|---|---|---|---|---|---|---|
| pipe h 6, 1 | 1 956 | 260 | 14 | 4 | 24 | 4 | 2 |
| pipe h 5, 1 | 4 672 | 0 | 0 | 0 | 0 | 0 | 1 232 |
| pipe h 5, 3 | 4 720 | 3 911 | 0 | 8 | 352 | 2 560 | 2 536 |
| pipe h 4, 3 | 6 752 | 6 288 | 192 | 0 | 744 | 4 800 | 4 752 |
| duct h 5, 2 | 2 560 | 2 560 | 2 560 | 0 | 0 | 0 | 0 |
| duct h 5, 3 | 2 560 | 2 560 | 0 | 0 | 0 | 1 328 | 4 368 |

(Skewness 4 / 20 failed nowhere.) Snappy's first pass runs the strict
65°: on the pipe, 4 720 of the 7 632 wall faces at h 5 mm sit above it
already with one layer.

**The cause of the 75–84° (found by this comparison, `diagnose_wall_faces_per_dual_cell`).**
On the bare tet dual each wall-adjacent cell owns **4 or 7+ wall faces**
(h 5 mm pipe: 7 632 wall faces on 1 288 cells; 648 cells with 4, 624 with
7 or more; the duct the same, 672 / 624), one piece per boundary-triangle
corner, and the line from a piece's centre to its owner's centre leaves
the piece at a median 69° from its normal (90th percentile 74.6°, max
75.1°; 4 720 pieces above 65°, exactly the 4 720 of the layered rows; duct
median 58°, 2 560 above 65°). A prism stacked on a piece inherits that
angle at its top face; no thinning changes it, which is why the back-off
cannot and why snappy removes everything. OpenFOAM's `polyDualMesh` gives a
boundary point's dual cell one boundary face per patch (the polygon around
the point), not one per triangle corner; this crate's face-minimal dual
does not merge the pieces even where they are coplanar (the flat duct). Not
changed here (recorded as the lead for the next step; see "Open").

**Solve level** (Re = 100, demo pair, nominal instruments; steps, u_c/2U,
(−dp/dz)/HP, profile RMS, τ_w/HP; meshing time):

| h (mm), N | BackOffOnly | CfMeshOptimise | SnappyCollapse |
|---|---|---|---|
| 5, 3 | **diverged** (as gh:#1202) | **ran: 425 steps**, 0.9215, 0.9977, 0.0321, 0.9935 (mesh 79 s) | = the bare mesh (all collapsed): 24, 0.9005, 0.9925, 0.0486, 0.9908 |
| 4, 3 | **diverged** | **ran: 54 steps**, 0.8935, 0.9968, 0.0305, 0.9903 (mesh 888 s; 198 bad faces left) | = bare (demo default): 29, 0.8925, 1.0326, 0.0356, 1.0312 |
| 5, 2 | 117, 0.8990, 0.9946, 0.0500, 0.9785 (gh:#1202's row, reproduced) | 31, 0.9253, 0.9976, 0.0340, 0.9935 (76 s) | = bare: 24, 0.9005, 0.9925, 0.0486, 0.9908 |
| 4, 2 | 45, 0.8926, 1.0280, 0.0365, 1.0262 (gh:#1202's row) | 51, 0.8927, 0.9918, 0.0309, 0.9895 (869 s) | = bare: 29, 0.8925, 1.0326, 0.0356, 1.0312 |
| 6, 2 | 39, 0.8079, 1.0883, 0.0729, 1.0653 (gh:#1202's row) | 31, 0.8522, 1.0168, 0.0502, 1.0125 (186 s) | = bare: 22, 0.8058, 1.0692, 0.0739, 1.0677 |
| 6, 3 | no layer (the back-off found none) = bare: 22, 0.8058, 1.0692, 0.0739, 1.0677 | **5 000 steps, not steady** (the step cap; it did not diverge): 0.8537, 1.0378, 0.0495, 1.0136 (204 s) | 2.5 % of the wall points kept (wedge cells where layers end): 22, 0.8033, **1.1212**, 0.0723, 1.1112 |
| 5, 1 | 28, 0.8954, 0.9705, 0.0509, 0.9678 (gh:#1202's row) | 24, 0.9179, 0.9999, 0.0350, 0.9964 (50 s) | = bare: 24, 0.9005, 0.9925, 0.0486, 0.9908 |

Reading it: under cfMesh every case that meshed also ran except h 6 mm with
3 layers (not steady in 5 000 steps); its dp/dz and τ_w sit within 0.5 % of
Hagen–Poiseuille at h 5 mm and h 4 mm (thick layers at the wall, but u_c
still 8–11 % low: the core). Snappy's 3 % of kept layers at h 6 mm *raise*
dp/dz by 5 % over the bare mesh: a layer that ends inside the wall leaves
wedge cells, the worst place for them.

The structured default was re-run: 20 480 cells, 34 steps, u_c/2U
**0.9940**, dp/dz **1.0154**, RMS 0.0063, τ_w 1.0066, unchanged. The
unstructured default (h 4 mm, no layer) is unchanged by the determinism fix
(its fingerprint did not move) and by the strategies (it has no layer):
0.8925 / 1.0326, as `headless_unstructured_pipe_runs_and_is_pinned` pins.

**Sanity and determinism.** On the Cartesian duct every strategy leaves
the mesh alone, as predicted (`every_strategy_leaves_a_good_box_duct_alone`:
snappy converges in one iteration, bit-identical to the back-off; cfMesh
flags no low-quality face, moves no interior point, keeps the side-middle
thickness to 1e-12). All three strategies are reproducible (pinned
fingerprints; `det true` on every `CFMESH_DET=1` case).

### Against the predictions

| prediction | result | verdict |
|---|---|---|
| CfMesh coverage 100 % | 100 % everywhere | held |
| Snappy coverage 1 layer 60–95 %, 2–3 layers 0–50 % | **0–5 %**: 5.4 % at h 6 mm with 1 layer, 0 elsewhere | **missed** (lower than the low edge at 1 layer) |
| what collapses snappy: face weight and volume ratio, not non-orthogonality | **non-orthogonality first** (4 672 of 7 632 faces above 65° with one layer at h 5 mm; the strict 65° pass removes them before the relaxed 75° could apply); weight and volume ratio fail too from 2–3 layers | **missed** |
| CfMesh thickness 0.8–1.0 × the back-off's | **2.5–4.4 × thicker** (h 5 mm, 3 layers: total 0.77 mm against 0.18; first layer 0.21 against 0.05; with 1 layer 0.77 mm against the 0.3 mm asked): the optimisers move the layer's inner points freely, and cfMesh's thickness control (`refineBoundaryLayers`' `maxFirstLayerThickness`) is not ported | **missed** |
| CfMesh max NO 65–78° | 63.4–76.3° | held |
| Snappy max NO ≤ 75° | 47–63° (nothing left to be bad) | held, trivially |
| skewness ≈ 1 | 0.6–2.0 | held |
| CfMesh 3 layers runs at h 5 mm (55 %) | **runs** (425 steps) and at h 4 mm (54 steps); at h 6 mm not steady in 5 000 steps (no divergence) | held at 5 and 4 mm |
| Snappy 3 layers runs (70 %) | runs, but only because no layer is left | held in letter, not in substance |
| u_c, dp/dz within ±2 % of the back-off where both run | CfMesh against BackOff at the same h: N = 2, dp/dz +0.3 % (5 mm), −3.5 % (4 mm), −6.6 % (6 mm), u_c +2.9 %, 0.0 %, +5.5 %; N = 1 at 5 mm, dp/dz +3.0 %, u_c +2.5 % | **missed** at h 4 and 6 mm on dp/dz, and at 5 and 6 mm on u_c: the thicker cfMesh layers change the near-wall resolution, and at h 6 mm they move both toward Hagen–Poiseuille |
| determinism | identical | held |
| meshing time: CfMesh 2–20 × | **30–900 ×** (27 s to 15 min) | **missed** |
| Cartesian duct sanity | as predicted | held |
| `featureAngle` 60 unextrudes the duct's long edges | no visible effect: the strict quality pass already removes every layer | not testable here |

### Where A and B agree and where they differ, and why

- **Agree:** both repair what the back-off cannot: the 3-layer pipes run
  under both, and both end with max non-orthogonality below 76°.
- **Differ, completely, on coverage:** cfMesh keeps 100 %, snappy 0 %. The
  reason is upstream's design, not a port difference: snappy only removes
  (a point once unextruded stays so) and its first 20 iterations hold 65°,
  which most faces of this dual's wall cells exceed *before any layer is
  added*; cfMesh only moves points and never removes a layer.
- **Differ on thickness:** snappy keeps the requested (capped) thickness
  where it keeps a layer; cfMesh's layers end 2.5–4.4 × thicker than the
  back-off's and up to 2.5 × the request (module docs).
- **Differ on cost:** cfMesh's volume optimiser on tetrahedra of the
  polyhedral cells is 30–900 × the back-off; snappy costs a few rebuilds.

### Recommendation (for the maintainer; the default is not changed)

Keep **`BackOffOnly` as the default** for the demo's offered settings
(it is what the recorded numbers use, and for 1–2 layers it runs), and
**do not adopt either new strategy as the default yet**:
`SnappyCollapse` removes every layer on this mesh family, so it is
equivalent to asking for none; `CfMeshOptimise` makes 3 layers run but
costs minutes at h 4 mm (too slow for the browser demo, which meshes on the
reader's phone) and does not hold the requested thickness. The comparison
points at the cause instead: the dual's wall cells carry 4–8 wall faces
each, one per triangle corner, where OpenFOAM's `polyDualMesh` would give
one. **Recommended next step:** merge each dual cell's wall pieces into one
face per patch (port `polyDualMesh`'s boundary faces), predicted to bring
the interface non-orthogonality to the bare interior's 50–57° and let all
three strategies keep their layers; then re-run this table.

### Images (drawn from the `PolyMesh` the solver gets)

In `laminar_pipe_gh1155/layer_strategies_gh1215/`, written by
`cargo run --release -p dhoby-ghaut --example foam_studio_web -- --render-layer-strategies`
(the demo pipe, h 5 mm, 3 layers, one colour per cell), for each of
`backoff`, `cfmesh`, `snappy`: `layers_<s>_h5_3_xz_rim.png` (x-z zoom on
the inlet rim), `_xy.png` (cross-section at mid-length), `_xy_wall.png`
(3.5 mm wall zoom of that section) and `_wallmap.png` (every wall point,
angle against z, inlet at the top: red no layer, orange fewer than asked,
blue all three, darker the thicker, 0–1 mm).

**Checked by eye (2026-11-04):** `backoff` wall zoom: three thin prisms,
total 0.25–0.4 mm along the section, under the dual cell; rim: the same thin stack ends on
the inlet at z = 0 and its columns lean (the uniform back-off to 0.36 of
the cap). `cfmesh` wall zoom: three layers about 0.27, 0.33 and 0.40 mm
from the wall in (total about 1 mm here), the dual cell above them shrunk to make room; rim:
the stack is 0.7–1 mm thick down to the inlet, columns straight, the dual
cells behind it reshaped (the interior points moved, compare the rim of
`backoff`). `snappy`: no layer anywhere (the bare dual), and its wall map
is all red; cfMesh's all blue, thickness varying in bands along the
48 facets. **Not checked:** the 3-D shape of the reshaped dual cells
between the slices; the wall map draws points, not faces.

### Open (gh:#1215)

- ~~The dual's boundary faces (above): the lead.~~ **Done 2026-11-04, gh:#1240** (section below): ported as two dual kinds, default unchanged.
- CfMesh's thickness: port `refineBoundaryLayers`' `maxFirstLayerThickness`
  / `thicknessRatio`, so the requested first layer is honoured.
- CfMesh's cost: the volume optimiser runs on up to 2 point-layers of
  cells around ~1 000–5 000 low-quality faces for 10 passes; it is threaded
  natively (Jacobi, results independent of the thread count) but would
  need to be bounded before a browser could use it.
- Not ported (module docs): cfMesh's LP `meshUntangler`, boundary moves and
  `meshSurfaceOptimizer`; snappy's medial-axis mover and `handleWarpedFaces`.
- `octree.rs` still numbers cells in `HashSet` order (only with
  `refinement_levels > 0`).
- Upstream (OpenFOAM, cfMesh) is not installed here, so neither port was
  checked against its upstream on a shared case; the differential above is
  between the ports and the back-off.

## The dual's boundary faces: per corner, per point, cell-centre, differentially (gh:#1240)

**Issue:** gh:#1240, the lead of gh:#1215. The median dual
(`outram-park-fork-cfmesh::dual`) gives every wall cell one boundary face
per boundary-triangle corner (4–7+ per cell); a face's centre-to-owner line
leaves it at a median 69°, 4 720 of 7 632 wall faces above 65° on the bare
h 5 mm pipe, and that angle is inherited by every prism stacked on it.

### What was built (2026-11-04)

| `TetDualOptions::dual_kind` | code | upstream (OpenFOAM-dev `852527377c7e`) |
|---|---|---|
| `MedianPerCorner` (**default**, unchanged) | `dual::polyhedral_dual_min_faces` | none: this crate's median dual, per-corner quads `[v, em, fc, em]` |
| `MedianPerPoint` (Route A) | `dual::polyhedral_dual_per_point` | the boundary rule of `polyDualMesh`: feature edges from `simpleMarkFeatures` (`applications/utilities/mesh/manipulation/polyDualMesh/polyDualMesh.C`: patch seams always, normals more than the feature angle apart; every point on a feature edge a feature point), faces from `meshDualiser::createFacesAroundBoundaryPoint` (`meshDualiser.C`: one ring per point, closed at every feature edge), `polyDualMesh::splitFace` (`src/conversion/polyDualMesh/polyDualMesh.C`: fewer than two feature edges, no split) |
| `CellCentre` (Route B) | `cell_centre_dual` (feature `cell-centre-dual`) → `outram-foam-mesh::poly_dual_mesh` | the same boundary rule, now in the existing port too; its interior (cell-centre rings around interior edges) is unchanged and was already a port of `polyDualMesh::calcDual` |

Both new kinds name the tet boundary into the input regions before the dual
(`assign_patches_by_region`), so the inlet/wall and outlet/wall seams are
feature edges, as upstream's patch edges are. Feature angle 45° (the
`polyDualMesh` CLI default of `outram-foam-cli`; upstream's utility takes it
as a required argument).

**`outram-foam-mesh::poly_dual_mesh` was changed to upstream's rule** (it
emitted one quad per boundary face at a feature point, and tested points,
not edges, against the angle; patch seams were not features): the block V&V
tests still pass, and the 120° corner-cutting test now runs on a one-patch
block, since a six-patch block keeps its edges at any angle (upstream's
rule). New: `boundary_faces_one_per_point_per_sector`.

**Sharing decision.** `outram-foam-mesh` does not depend on this crate, so
this crate may depend on it, optionally (`cell-centre-dual`), with no cycle:
Route B *is* the outram-foam-mesh code, converted (`cell_centre_dual.rs`
only reorders faces and maps types). Route A cannot call it: the cfMesh
crate builds with no dependency by default, and Route A's interior is the
median dual's. The ~40-line sector walk is therefore written twice
(`dual::boundary_faces_per_point`, `poly_dual_mesh::build_boundary_faces_for_point`),
each citing the other and upstream.

**Deviation kept in both:** upstream puts a dual vertex at the midpoint of
*feature* edges only; both routes put one on every boundary edge (the
median dual needs it to close its boundary-edge faces; in outram-foam-mesh
it reproduces a flat patch exactly).

### Predictions (written 2026-11-04, before any differential run)

Inputs: the gh:#1215 meshes (the demo pipe R 10 mm, L 200 mm, 48 facets, at
h 4 / 5 / 6 mm; the tet-dual box duct 20 × 20 × 200 mm at h 5 mm), wall-only
layers first 0.3 mm × 1.2. Baseline (`MedianPerCorner`) numbers are
gh:#1215's.

| quantity | `MedianPerCorner` | `MedianPerPoint` (A) | `CellCentre` (B) |
|---|---|---|---|
| cells | as gh:#1215 (h 5 bare 3 113) | **= corner, exactly** (one per primal point) | = corner exactly, **if** the dual passes the gate (60 %: the centroid rings of slivers may invert a cell; on failure the pipeline falls back to corner with a note) |
| internal faces | — | **the corner dual's rings exactly** (same interior code); geometry differs only where the feature-edge capture moves boundary vertices differently | one per interior edge as corner, but rings of tet centroids only (about half the vertices) |
| wall faces per wall cell | 4 or 7+ (h 5: 648 / 624) | **1 for every wall cell** (rim points: 1 wall + 1 inlet face) | 1, as A |
| wall faces | 7 632 (h 5) | = wall cells ≈ 1 288 | ≈ 1 288 |
| wall face-to-owner angle | median 69°, p90 74.6°, max 75.1° | **median 15–30°, p90 < 45°, max < 65°**; > 65°: 0–20 | similar to A (±10°) |
| internal NO (whole mesh) | max 47–57° bare (gh:#1215 snappy rows) | = corner ± 3° | mean within ±5° of corner, max up to +10° (non-planar centroid rings) |
| max faceSkewness | 0.84–0.89 | within ±0.2 of corner | within ±0.5 |
| closure | valid | valid | valid if accepted |
| volume / πR²L (pipe) | (measured here) | **0.5–2 % below corner** at h 5 (the dropped vertex: a ring of radius ≈ 0.4 h on the cylinder sags ≈ (0.4h)²/2R inside); larger at h 6, smaller at h 4 | within 0.3 % of A (same boundary tiling; different interior only) |
| volume (duct) | exact | **= corner to 1e-6** (flat sides, the 90° edges are features) | = corner to 1e-6 |
| inlet area / πR² | (measured here) | = corner within 0.5 % (rim points kept, faces planar) | = corner within 0.5 % |
| determinism | pinned | identical across processes (BTreeMap walk) | identical (HashMaps there are lookups only) |
| **3 layers, BackOffOnly, h 5**: max NO / > 65° | 81.7° / 4 720 | **60–72° / < 300** | as A ± 5° |
| 3 layers, SnappyCollapse, coverage | 0 % | **40–100 %** (1 layer 60–100 %) | as A ± 20 points |
| 3 layers, CfMeshOptimise | 100 %, 48 s | 100 %, **2–10× faster** (fewer low-quality faces) | as A |
| duct, 2–3 layers | as gh:#1215 | all strategies keep layers; snappy ≥ 80 % | as A |
| flow, bare h 4 (demo default) u_c/2U, dp/dz/HP | 0.8925, 1.0326 | u_c within ±0.02 of corner; **dp/dz +2 to +5 %** (the smaller section from the volume loss) | as A ± 2 % |
| flow, h 5, 3 layers BackOffOnly | diverged | **runs** (65 %): u_c 0.90–0.93, dp/dz 0.98–1.04, τ_w within 5 % of HP | runs (50 %), as A ± 3 % |

Where A and B should differ, and why: only in the interior (median corners
at face and cell centroids against tet centroids), so mesh-level boundary
quantities (wall-face count, wall angle, volume, inlet area) should agree
closely and interior non-orthogonality, skewness and validity may not.
Where A and corner should differ: only at the boundary faces; any change in
the interior quality beyond the boundary vertex moves is a finding.

### First runs, a defect found, and a second variant (2026-11-04)

The bare-mesh rows and the BackOffOnly / SnappyCollapse layer rows were run
first (results below). **BackOffOnly grew no layer on either new route**, at
any h and any N. The cause, found with `diagnose_back_off_on_dual_kinds`
(the back-off's candidates at the cap × 0.6^k on the bare h 5 mm pipe):
**16 faces with a non-positive pyramid at every thickness down to 1 % of
the cap**, and a per-point cap of 0 m somewhere. They sit on the inlet and
outlet rims. With `FeaturePointRule::OnFeatureEdge` (OpenFOAM's
`polyDualMesh` *utility*: every point on a feature edge keeps its vertex),
a rim point's inlet face is `[v, em₁, fc, …, em₂]` with `em₁`, `v`, `em₂`
on the rim; on a straight stretch of the 48-gon they are collinear, and the
layer stage's border limit (cfMesh `boundaryLayers::createNewVertices`,
"limit distances": 0.9 × the distance from the point to the segment between
its two neighbours in the non-layer face, ported as upstream has it) is
then **0**. A zero-thickness column next to full ones folds the layer cell
there. cfMesh's rule assumes the vertex is not collinear with its
neighbours; nothing in it is wrong for cfMesh's own meshes.

OpenFOAM has a second `polyDualMesh`: the **class**
(`src/conversion/polyDualMesh/polyDualMesh.C`, the one the brief names).
Its `calcFeatures` keeps only points with **more than two** feature edges;
a point on a feature line is dropped and its ring split along the line
(`splitFace`), so the inlet and wall faces of a rim point meet on the chord
`em₁–em₂`, which on a straight stretch passes through the dropped point.
Both rules are now callable (`TetDualOptions::dual_feature_points`:
`OnFeatureEdge`, the default, and `Corners`; the same enum in
`outram-foam-mesh` as `FeaturePoints`), and both are run below.

**Predictions for `Corners`, written before its differential rows (one
diagnostic already run, said here so the prediction is not mistaken for a
blind one):** the diagnostic showed the h 5 mm back-off candidates with no
bad pyramid and max non-orthogonality 55° (1 layer) and 54–66° (3 layers,
× 0.6 and × 1). Predicted for the table: bare cell counts, wall faces per
wall cell, internal NO and the wall face-to-owner angle as `OnFeatureEdge`
(± 1° on the medians); volume and inlet area within 2e-4 of `OnFeatureEdge`
(the rim chords cut only at the 48-gon's corners); BackOffOnly now grows
full layers at 1–3 layers at h 4 / 5 / 6 with max NO < 70°; SnappyCollapse
coverage ≥ 80 % at 1 layer and ≥ 50 % at 3 layers; the 3-layer h 5 mm solve
runs, u_c/2U 0.90–0.94, dp/dz 0.98–1.04 × HP.

### Methodology (as run)

- **Mesh level:** `outram-park-fork-cfmesh`
  `pipeline::tests::diagnose_dual_kinds_differential` (`--ignored
  --features cell-centre-dual`; filters `CFMESH_CASE`, `CFMESH_DUAL`,
  `CFMESH_LAYERS`, `CFMESH_STRATS`, `CFMESH_FP=corners`; `CFMESH_DET=1`
  re-runs each bare mesh in the same process). Internal non-orthogonality
  as `check_quality` measures it (owner-to-neighbour centre line against
  the face normal), per face; the **wall face-to-owner angle** is the
  same instrument on a wall face with the face centre in place of the
  neighbour (what a prism stacked on the face inherits at its top);
  faceSkewness is OpenFOAM's (`check_quality`); volume and inlet area
  against πR²L and πR² (also against the 48-gon). For full-coverage
  back-off meshes, which of snappy's `meshQualityDict` checks the layer
  cells' faces fail, one at a time. `diagnose_back_off_on_dual_kinds`
  prints the back-off's candidates at cap × 0.6^k.
- **Solve level:** Foam Studio `--headless-unstructured h n 0.3 1.2 100`
  with `FOAM_DUAL_KIND=corner|point|centre`, `FOAM_DUAL_FP=corners` and
  `FOAM_LAYER_STRATEGY`, the demo pair, Re 100, to the steady criterion
  (5 000-step cap); nominal instruments as the gh:#1202 / #1215 rows.
- **Determinism:** each bare mesh was fingerprinted in three separate
  processes (two differential runs and a direct run of the test binary):
  identical every time; pinned by `dual_kinds_are_pinned_bit_for_bit`.
- Times are native release on the shared 4-core host with other jobs
  running (upper bounds).

### Results (2026-11-04)

**Bare meshes** (no layer). `E` = `FeaturePointRule::OnFeatureEdge`
(the default of the new kinds), `C` = `Corners`. Wall faces per wall cell
as counts of cells with 1 / 2 / 3 / 4 / 7+; NO = internal
non-orthogonality (mean, max); wall angle = face-to-owner angle (median,
p90, max; count above 65°).

| case | dual | cells | wall faces (cells) | per wall cell | NO mean / max (°) | wall angle p50 / p90 / max (°), > 65 | skew | V/πR²L | inlet/πR² |
|---|---|---|---|---|---|---|---|---|---|
| pipe h 4 | corner | 6 353 | 11 968 (2 020) | 0 / 8 / 16 / 1 016 / 980 | 11.4 / 50.6 | 68.3 / 77.0 / 77.6, 6 720 | 0.84 | 0.99545 | 0.99355 |
| | point E | 6 353 | 2 036 | 2 004 / 16 | 11.4 / 50.6 | 26.7 / 48.9 / 74.4, 16 | 0.85 | 0.99217 | 0.99355 |
| | centre E | 6 353 | 2 036 | 2 004 / 16 | 11.9 / 52.4 | 26.9 / 48.2 / 77.0, 16 | 0.91 | 0.99217 | 0.99355 |
| | point C | 6 353 | 2 036 | 2 004 / 16 | 11.5 / 50.6 | 26.7 / 48.9 / 78.7, 16 | 0.85 | 0.99122 | **1.00816** |
| | centre C | 6 353 | 2 036 | 2 004 / 16 | 12.0 / 52.3 | 27.6 / 48.2 / 81.7, 16 | 0.92 | 0.99122 | **1.00816** |
| pipe h 5 | corner | 3 113 | 7 632 (1 288) | 0 / 0 / 16 / 648 / 624 | 16.3 / 47.0 | 69.3 / 74.6 / 75.1, 4 720 | 0.84 | 0.99387 | 0.99056 |
| | point E | 3 113 | 1 320 | 1 256 / 32 | 16.3 / 47.0 | 25.6 / 44.3 / 67.4, 16 | 0.86 | 0.98892 | 0.99145 |
| | centre E | 3 113 | 1 320 | 1 256 / 32 | 16.7 / 47.7 | 26.2 / 43.8 / 70.5, 16 | 0.92 | 0.98892 | 0.99145 |
| | point C | 3 113 | 1 304 | 1 272 / 16 | 16.3 / 47.0 | 25.6 / 43.9 / 45.6, **0** | 0.86 | 0.98887 | 0.98287 |
| | centre C | 3 113 | 1 304 | 1 272 / 16 | 16.7 / 47.7 | 26.2 / 43.3 / 45.5, **0** | 0.92 | 0.98887 | 0.98287 |
| pipe h 6 | corner | 1 939 | 4 774 (807) | 0 / 8 / 0 / 403 / 386 (+10 with 6) | 11.1 / 56.2 | 60.1 / 74.5 / 81.4, 2 026 | 0.89 | 0.99056 | 0.99008 |
| | point E | 1 939 | 828 | 790 / 13 / 4 | 11.1 / 56.5 | 11.6 / 40.6 / 85.6, 8 | 0.96 | 0.98216 | 0.98956 |
| | centre E | 1 939 | 828 | 790 / 13 / 4 | 12.0 / 57.7 | 11.0 / 39.4 / 86.6, 10 | 1.02 | 0.98216 | 0.98956 |
| | point C | 1 939 | 827 | 793 / 11 / 4 | 11.2 / 56.7 | 11.6 / 40.6 / 77.7, 4 | 0.90 | 0.98081 | 0.98572 |
| | centre C | 1 939 | 827 | 793 / 11 / 4 | 12.0 / 57.9 | 11.0 / 39.4 / 80.9, 2 | 1.09 | 0.98081 | 0.98572 |
| duct h 5 | corner | 3 921 | 7 680 (1 296) | 0 / 0 / 0 / 672 / 624 | 6.0 / 25.6 | 58.4 / 72.6 / 72.6, 2 560 | 0.69 | 1.00000 | 1.00000 |
| | point E / C | 3 921 | 1 460 | 1 132 / 164 | 6.0 / 25.3 | 0.0 / 8.4 / 11.4 (C: 14.3), 0 | 0.74 | 1.00000 | 1.00000 |
| | centre E / C | 3 921 | 1 460 | 1 132 / 164 | 7.1 / 32.9 | 0.0 / 7.9 / 11.0 (C: 13.8), 0 | 0.88 | 1.00000 | 1.00000 |

All closed and valid; fingerprints identical across processes for every
row. **CORRECTED 2026-11-05 (gh:#1241):** the "inlet/πR²" column is
Σ|S_f| over the inlet faces, which counts a bent face's tilt as area; the
projected section |Σ S_f·ẑ|/πR² of the `C` rows is 0.95583 (h 4, not
1.00816), 0.98287 (h 5, planar, the same) and 0.94786 (h 6, not
0.98572). See the gh:#1241 section. (The h 6 mm corner row carries gh:#1215's note "34 boundary points
left unmapped" by the feature-edge capture; the C rows at h 4 and h 6 mm
carry 128 and 64.)

**Layers, `Corners` rule** (wall-only, first 0.3 mm × 1.2; coverage = wall
faces with all N layers; NO max; faces > 65°; minDet). The corner rows
are gh:#1215's, reproduced in the same run.

| case | dual | BackOffOnly N = 1 | N = 2 | N = 3 | SnappyCollapse N = 1 / 2 / 3 coverage |
|---|---|---|---|---|---|
| pipe h 5 | corner | 1.0, 74.7°, 4 672, 5.5e-2 | 1.0, 79.4°, 4 720 | 1.0, 81.7°, 4 720, 6.8e-3 | 0 / 0 / 0 |
| | point C | **1.0, 55.2°, 0**, 5.4e-2 | 1.0, 66.0°, 747, 1.1e-3 | **1.0, 66.0°, 747**, 1.1e-3 | **0.914** / 0 / 0 |
| | centre C | **1.0, 55.2°, 0**, 2.2e-2 | **1.0, 55.0°, 0**, 1.5e-2 | **1.0, 55.0°, 0**, 5.6e-3 | 0.626 / 0 / 0 |
| pipe h 4 | corner | 1.0, 78.2°, 6 320 | 1.0, 83.5°, 6 736 | 1.0, 84.4°, 6 752 | 0 / 0 / 0 |
| | point C | 1.0, 63.7°, 0 | 1.0, 68.1°, 32 | 1.0, 69.8°, 32, 9.3e-3 | 0.953 / 0 / 0 |
| | centre C | 1.0, 71.8°, 24 | 1.0, 72.2°, 32 | 1.0, 74.3°, 32, 5.1e-3 | 0.580 / 0 / 0 |
| pipe h 6 | corner | 1.0, 77.0°, 1 956 | 1.0, 84.2°, 2 308 | none (back-off found none) | 0.054 / 0 / 0 |
| | point C | 1.0, 73.6°, 3 | 1.0, 74.1°, 5 | 1.0, 73.2°, 8 | 0.363 / 0 / 0 |
| | centre C | 1.0, 77.3°, 5 | 1.0, 78.0°, 9 | 1.0, 77.1°, 10 | 0.363 / 0 / 0 |
| duct h 5 | corner | 1.0, 71.2°, 2 560 | (gh:#1215) 80.2° | 80.4° | 0 / 0 / 0 |
| | point C | 1.0, 45.7°, 0 | 1.0, 62.1°, 0 | 1.0, 62.1°, 0 | 0.989 / 0 / 0 |
| | centre C | 1.0, 50.8°, 0 | 1.0, 53.4°, 0 | 1.0, 53.4°, 0 | 0.989 / 0 / 0 |

**Layers, `OnFeatureEdge` rule** (the first runs): BackOffOnly grew **no
layer on any pipe** of either new kind at any h and N (the duct, with no
rim on a curve, took full layers: point N = 3 at 62.1°); SnappyCollapse
covered 96–97 % (point) and 70–97 % (centre) at N = 1, 65 % at h 6 mm, and
0 at N = 2 and 3. The cause is above (first runs): 16 rim faces with a
non-positive pyramid at every thickness, from the zero border cap at a
collinear rim vertex.

Which of snappy's checks the layer cells' faces fail (back-off, full
coverage, h 5 mm, `Corners`), one at a time:

| case | NO > 65 | tet quality | twist | det | face weight | vol ratio |
|---|---|---|---|---|---|---|
| pipe corner N = 1 / 3 | 4 672 / 4 720 | 0 / 0 | 0 / 8 | 0 / 352 | 0 / 2 560 | 1 232 / 2 536 |
| pipe point N = 1 / 3 | **0 / 0** | 29 / 132 | 8 / 29 | 0 / 0 | 0 / 156 | 0 / 0 |
| pipe centre N = 1 / 3 | **0 / 0** | 29 / 141 | 8 / 30 | 0 / 144 | 0 / 164 | 0 / 0 |
| duct point N = 1 / 3 | 0 / 0 | 16 / 744 | 0 / 32 | 0 / 0 | 0 / 0 | 0 / 0 |
| duct centre N = 1 / 3 | 0 / 0 | 16 / 744 | 0 / 16 | 0 / 0 | 0 / 0 | 0 / 0 |

**CfMeshOptimise, `Corners`, h 5 mm:** point N = 1: 100 %, 279 s, mean
total 0.89 mm, NO max 56.1°, 29 bad faces left; N = 3: 100 %, 249 s,
0.89 mm, 57.7°, 77 left. Centre N = 1: 100 %, 135 s, 1.06 mm, 56.1°;
N = 3: 100 %, 140 s, 1.07 mm, 57.2°, 77 left. (gh:#1215's corner rows:
27 s and 48 s, NO 63.4° and 66.2°.)

**Solve level** (Re 100, demo pair; steps, u_c/2U, (−dp/dz)/HP, profile
RMS, τ_w/HP):

| h, N, strategy | corner | point E | centre E | point C | centre C |
|---|---|---|---|---|---|
| 4, bare (demo default) | 29, 0.8925, 1.0326, 0.0356, 1.0312 | 29, 0.9003, 1.0579, 0.0322, 1.0586 | 27, 0.8819, 1.0621, 0.0372, 1.0628 | 29, 0.8677, 1.0152, 0.0485, 1.0158 | 28, 0.8502, 1.0190, 0.0526, 1.0197 |
| 5, bare | 24, 0.9005, 0.9925, 0.0486, 0.9908 | 23, 0.9145, 1.0340, 0.0436, 1.0357 | 28, 0.9081, 1.0227, 0.0446, 1.0240 | 24, 0.9068, 1.0246, 0.0471, 1.0263 | 27, 0.9004, 1.0134, 0.0482, 1.0147 |
| 5, 1, BackOff | 28, 0.8954, 0.9705, 0.0509, 0.9678 | (no layer: = bare) | (= bare) | 23, 0.9070, 1.0391, 0.0466, 1.0411 | 29, 0.9005, 1.0331, 0.0479, 1.0351 |
| 5, 1, Snappy | = bare | — | — | 23, 0.9070, 1.0391, 0.0466, 1.0411 (91 % covered) | 29, 0.9004, 1.0136, 0.0485, 1.0298 (63 %) |
| 5, 2, BackOff | 117, 0.8990, 0.9946, 0.0500, 0.9785 | — | — | **diverged** | 32, 0.9001, 1.0294, 0.0481, 1.0313 |
| 5, 3, BackOff | **diverged** | (= bare) | (= bare) | **diverged** | **diverged** |
| 4, 3, BackOff | **diverged** | — | — | **diverged** | **diverged** |
| 6, 3, BackOff | no layer | — | — | **diverged** | **diverged** |
| 5, 3, CfMesh | 425, 0.9215, 0.9977, 0.0321, 0.9935 (mesh 79 s) | — | — | **runs: 22, 0.9182, 1.0068, 0.0361, 1.0109** (mesh 373 s) | **runs: 26, 0.9314, 1.0050, 0.0322, 1.0056** (mesh 167 s) |

The structured default was re-run: 34 steps, u_c/2U **0.9940**, dp/dz
**1.0154**, RMS 0.0063, τ_w 1.0066, unchanged. The unstructured default
(per-corner, h 4 mm, no layer) is unchanged: 0.8925 / 1.0326, as
`headless_unstructured_pipe_runs_and_is_pinned` asserts (it passes), and
its cfMesh-pipeline fingerprint (L 200 mm, `pipe_surface`)
18084991279620725399 is the same in all three processes; the per-corner
code path is untouched (`back_off_only_layers_are_pinned_bit_for_bit`,
`layer_strategies_are_deterministic` pass with their old pins).

### Against the predictions

| prediction | result | verdict |
|---|---|---|
| cells = corner exactly | equal at every h, both kinds, both rules | held |
| centre passes the gate (60 %) | passed everywhere, no fall-back | held |
| internal faces = corner's rings (A) | NO distribution equal to 0.1° (mean, p50, p90, p99, max) at every h | held |
| 1 wall face per wall cell | 97–99 % of wall cells have 1, the rest 2 (3 for 4 cells at h 6 mm): the rim (1 wall + 1 inlet) is as predicted, the 2-face cells are where the feature angle (45°) splits a wall point's ring on a fold of the snapped tet surface, and on the duct the long-edge points (2 sides) | held in substance; "1 for every cell" missed by 1–3 % |
| wall angle median 15–30°, p90 < 45°, max < 65° | median 11–28°, p90 39–49°; max 67–87° with `E`, 45.5° at h 5 with `C` but 78–82° at h 4 / h 6 | median held; p90 missed at h 4 (48.9°); max missed except h 5 `C` |
| B wall angle ± 10° of A | within 1° on the medians | held |
| internal NO of B: mean ± 5°, max up to +10° | mean +0.4–0.9°, max +0.7–1.7° | held |
| skewness ± 0.2 / ± 0.5 | A +0.01–0.07, B +0.07–0.20 | held |
| volume 0.5–2 % below corner at h 5 | −0.50 % (h 5), −0.33 % (h 4), −0.85 % (h 6) | held at h 5 and 6; h 4 below the low edge |
| B volume within 0.3 % of A | **identical** to 5 digits: the boundary tiling and the mapped boundary vertices are the same in both, and only boundary faces enter the volume | held |
| duct volume = corner | 1.00000 | held |
| inlet area within 0.5 % of corner | `E`: +0.09 %, 0.0, −0.05 %; `C`: **+1.5 % (h 4), −0.8 % (h 5), −0.4 % (h 6)** | held with `E`; **missed** with `C` (predicted 2e-4) |
| deterministic | identical across three processes | held |
| BackOff 3 layers h 5: max NO 60–72°, > 65° < 300 | `E`: **no layer at all**; `C`: point 66.0° / 747, centre 55.0° / 0 | `E` **missed**; `C` held for centre, point missed on the count |
| Snappy coverage 40–100 % at 3 layers | **0 everywhere** (N = 1: 36–99 %) | **missed** |
| CfMesh 2–10× faster | 3–6× **slower** (135–279 s against 27–48 s) | **missed** |
| duct, all strategies keep layers | BackOff and CfMesh yes; snappy only at N = 1 | missed for snappy at N ≥ 2 |
| flow bare h 4: u_c ± 0.02, dp/dz +2 to +5 % | `E`: u_c +0.008 / −0.011, dp/dz +2.5 / +2.9 %; `C`: u_c −0.025 / −0.042, dp/dz −1.7 / −1.3 % | `E` held; `C` missed on both |
| h 5, 3 layers BackOff runs (65 %) | **diverged** on both kinds (with `C`; with `E` no layer grew) | **missed** |
| (not predicted) 3 layers under CfMeshOptimise | **run on both kinds with `C`**: point 22 steps, centre 26 (corner: 425); centre u_c 0.9314, dp/dz 1.0050, τ_w 1.0056 | — |

### Where the routes agree and differ, and why

- **A against corner:** same cells and the same interior rings (NO
  distribution equal); they differ only at the boundary, as intended: one
  face per point instead of 4–8, wall angle median 69° → 26° (h 5). The
  volume is 0.3–0.9 % lower because the merged face drops the vertex `v`
  on the curved wall and the face centroid sits inside the cylinder by
  the sagitta of a ring of radius ≈ 0.4 h. The flow changes follow the
  section: at h 4 mm dp/dz rises 2.5 % (0.33 % less area, and the near-wall
  cells now centred on the wall points), at h 5 mm by 4.2 %.
- **B against A:** identical boundary (volume, inlet area, wall-face
  counts to the face); they differ in the interior only, where the
  cell-centre rings (tet centroids) are slightly less orthogonal (+0.4–0.9°
  mean) and more skewed (+0.07–0.2) than the median rings, and the dual
  cells' centres sit elsewhere, which moves u_c by 0.7–2 % against A at the
  same h. With layers, B is the better of the two on the pipe at h 5 mm
  (55° against 66° at N = 2–3, no face above 65°) and the worse at h 4 and
  6 mm (72–78° against 64–74°); its 2-layer h 5 mm solve runs where A's
  diverges.
- **`E` against `C`:** `E` keeps a vertex at every rim point; on a straight
  stretch of the 48-gon it is collinear with its two feature-edge
  neighbours, and cfMesh's border cap is then zero: no back-off layer can
  grow. `C` drops those vertices (the `polyDualMesh` class) and every
  back-off grows, but the rim becomes a chain of chords that the
  feature-edge capture then maps less well: 128 / 64 boundary points left
  unmapped at h 4 / 6 mm (none with `E`), and the inlet area off by +1.5 %
  / −0.8 % / −0.4 %. That is the likely cause of `C`'s 3.6 % lower u_c at h 4 mm (not isolated).
- **Snappy at N ≥ 2:** with the non-orthogonality gone, what still fails
  is snappy's `minTetQuality` and `minTwist` on 100–750 layer faces (the
  side faces of a prism on a 10–14-vertex, non-planar wall polygon), and
  its unextrusion loop then removes every point within 2–3 iterations.
  That the whole wall goes from a few hundred failing faces is the
  port's loop as run (each removal leaves wedge cells whose faces fail in
  turn); it was not traced face by face here. **Open.**
- **3-layer divergence** with BackOff on the new duals, whose mesh passes
  the layer checks (NO ≤ 66°, minDet ≥ 1e-3): not explained here. The
  corner dual's 3-layer divergence was attributed (gh:#1202) to one
  shrunk dual cell over the stack; on the per-point duals the smallest
  determinant is 1.1e-3 (point, 2–3 layers) and 5.6e-3 (centre, 3 layers),
  against 1.5e-2 on the centre 2-layer mesh that runs. **Open.**

### Recommendation (for the maintainer; the default is not changed)

**Keep `MedianPerCorner` as the default for now.** The boundary-face defect
is real and the port fixes it (wall angle 69° → 26°, the 65° failures gone
from every layer face), and **three layers now run**, but only through
cfMesh's optimiser: `CellCentre` + `Corners` + `CfMeshOptimise` at h 5 mm
reaches steady in 26 steps with u_c/2U 0.9314, dp/dz 1.0050 and τ_w 1.0056
× Hagen–Poiseuille, the closest to HP of any unstructured row in this
record, but its meshing took 167 s (with other jobs running), too slow for
the browser demo. (Its total layer thickness, 1.07 mm, is the requested
0.3 × (1 + 1.2 + 1.44) = 1.09 mm here; the first layer's thickness was not
measured.) With the back-off,
three layers still diverge and snappy still collapses at two. **The
candidate to adopt next is `CellCentre` with `Corners`**: at h 5 mm its
back-off layers keep every face ≤ 55° and 2 layers run (u_c 0.900, dp/dz
1.029). Before a default change: (1) fix the rim mapping of the chord chain
(port cfMesh's `correctEdgesBetweenPatches`, already open), (2) trace the
3-layer back-off divergence, (3) understand snappy's tet-quality cascade,
(4) bound cfMesh's optimiser cost. Changing the default re-pins
`headless_unstructured_pipe_runs_and_is_pinned` and re-measures every
unstructured number in this record.

### Images (drawn from the mesh the solver gets)

In `laminar_pipe_gh1155/dual_kinds_gh1240/`, written by
`cargo run --release -p dhoby-ghaut --example foam_studio_web -- --render-dual-kinds`
(and again with `FOAM_DUAL_FP=corners`, files `dual_<kind>_corners_*`):
`dual_<kind>_h5_wall_cell.png` (the wall faces of the wall cell at
mid-length, unrolled, each face's face-to-owner angle at its centre, the
owner centre marked), `_bare_xy.png` and `_bare_xy_wall.png` (cross-section
at mid-length and its wall zoom), `_3layers_xz_rim.png` (inlet rim, three
back-off layers).

**Checked by eye (2026-11-04):** `corner` wall cell: 8 wall faces around
the point (52–57° by this picture's owner centre, a vertex-weighted
estimate; the table's 69° median is over all wall faces with the
volume-weighted centres); `point` and `centre`: one octagon-like face
around the point, 1° from its owner. Corner 3-layer rim: three thin
prisms under the wall cells down to the inlet (gh:#1215's picture).
Point-`C` 3-layer rim: three layers 0.26–0.43 mm, ending cleanly on the
inlet at z = 0, the rim cell's column leaning inward; with `E` the rim
picture is the bare mesh (no layer grew). `centre` bare wall zoom: one
large wall cell spanning the zoom, its wall side a polyline of the face
chords. **Not checked:** the 3-D shape of the rim cells, and where on the
rim the unmapped points of the `C` meshes sit.

### Open (gh:#1240)

- The 3-layer divergence on the per-point duals (above), and snappy's
  collapse at N ≥ 2.
- `Corners` and the feature-edge capture: port cfMesh's
  `correctEdgesBetweenPatches` (gh:#1202's open item), then re-measure the
  inlet area.
- cfMesh's border cap on a vertex collinear with its neighbours (zero):
  upstream's rule as written; with `OnFeatureEdge` it stops every back-off
  layer. A change to it would be a deviation from cfMesh, for the
  maintainer.
- Upstream OpenFOAM `polyDualMesh` is not installed here, so neither route
  was checked against it on a shared case.
- `octree.rs`'s `HashSet` numbering (refinement path): untouched.

## Follow-ups of gh:#1240: rim repair, the 3-layer divergence, snappy's cascade, cfMesh's cost, octree order (gh:#1241)

**Issue:** gh:#1241. Five items, worked in order; each has its
predictions written (and committed) before its runs.

### Item 1: cfMesh's `correctEdgesBetweenPatches` and the `Corners` rim

**Upstream read first (2026-11-05).** cfMesh v1.1 (`DAFoam/cfmesh`
`e944f6ba695f`), `meshLibrary/utilities/surfaceTools/correctEdgesBetweenPatches/`
(`correctEdgesBetweenPatches.C`, `…DistributeFaces.C`), called from
`meshSurfaceEdgeExtractorNonTopo::decomposeBoundaryFaces`, which
`tetMeshGenerator::mapEdgesAndCorners` runs after `extractPatches` and
before `mapVerticesOntoSurfacePatches`. It does **not** move points. It
changes topology, in two steps (a third, `decomposeProblematicFaces`, is
commented out upstream):

1. `decomposeConcaveFaces`: an internal face touching a concave (or
   undetermined) boundary point and having boundary edges in two or more
   patches is split into a triangle fan about its centre
   (`decomposeFaces`), and both its cells are marked.
2. `patchCorrection`: a boundary face with a vertex that is an *edge
   point* (exactly two feature edges, `meshSurfacePartitioner`) whose two
   edges in the face are **both** feature edges is split: a quad into two
   triangles at that vertex, a larger face into a fan about its centre;
   triangles are left alone. Its owner is marked.
3. Every marked cell is split into pyramids, one per face, apex at the
   average of the cell's vertices (`decomposeCells::decomposeCellIntoPyramids`,
   `findTopVertex`), after splitting any face that meets another face of
   the same cell on more than one edge (`checkFaceConnections`).

**Measured before any change** (`diagnose_corners_rim`, bare pipes,
`Corners`, both per-point kinds give the same boundary; with and without
the feature-edge capture). This replaces a misreading in the gh:#1240
table: its "inlet/πR²" is Σ|S_f| over the inlet faces, which counts a
non-planar face's tilt as area. The projected section |Σ S_f·ẑ| is what
the flow sees:

| h | Σ\|S\|/πR² | \|Σ S·ẑ\|/πR² | inlet points off the plane | rim vertices (at a 48-gon corner) | reverted by the capture | `patchCorrection` candidates (inlet) |
|---|---|---|---|---|---|---|
| 4 | 1.00816 | **0.95583** | 24 (up to 1.0 mm) | 48 (0) | 128 | 72 (16) |
| 5 | 0.98287 | 0.98287 | 0 | 44 (0) | 0 | 32 (4) |
| 6 | 0.98572 | **0.94786** | 6 (up to 1.5 mm) | 23 (3) | 64 | 24 (4) |

So the `Corners` rim has **two** errors, not one: (a) at h 4 and 6 mm the
capture reverts the points of cells it tangles, and the inlet is left
bent (projected section −4.4 % and −5.2 %); (b) at every h the rim is a
chain of chords between edge midpoints, none at a 48-gon corner (the
48-gon itself is 0.99715 πR²), which at h 5 mm costs −1.7 % with a planar
inlet. `correctEdgesBetweenPatches` addresses (a) at most: it adds no
rim vertex, so it cannot address (b).

**Predictions (written 2026-11-05, before the port's first run).** Port
as a stage between the patch naming and the capture, off by default
(`TetDualOptions::correct_edges`), on the same h 4 / 5 / 6 mm `Corners`
pipes, both per-point kinds:

| quantity | h 4 | h 5 | h 6 |
|---|---|---|---|
| `decomposeConcaveFaces` faces | 0 (every feature edge of a pipe is convex) | 0 | 0 |
| `patchCorrection` faces | = the candidates above (72 / 32 / 24), ± 10 % (the fan of one face can change its neighbour's count) | | |
| reverted by the capture | 128 → **0–20** | 0 | 64 → **0–10** |
| inlet points off the plane | 24 → 0–4 | 0 | 6 → 0–2 |
| \|Σ S·ẑ\|/πR² (predicted inlet area) | 0.956 → **0.985–0.993** (48 rim chords at r ≈ 9.98 mm; evenly spaced at 48-gon edge midpoints would give 0.99288) | **0.983 ± 0.002, unchanged** (no rim vertex added) | 0.948 → **0.970–0.988** (23 chords: a regular 23-gon is 0.98774) |
| Σ\|S\| − \|Σ S·ẑ\| | → < 2e-3 | 0 | → < 5e-3 |
| V/πR²L | 0.99122 → 0.992–0.996 | 0.98887 ± 0.0005 | 0.98081 → 0.984–0.990 |
| cells | + the pyramids: each marked cell of F faces becomes F cells, about +1 000–2 000 at h 4 | +300–800 | +200–600 |
| max internal non-orthogonality | rises (pyramids on a large dual cell; 55–75°) | | |
| closure, determinism | closed; identical across processes | | |

Not expected: any change at all with `correct_edges` off (every
existing pin unchanged), or on the per-corner default.

**What was built.** `outram-park-fork-cfmesh::correct_edges` (the three
steps above, upstream file and commit in its header) and
`TetDualOptions::correct_edges` (off by default). Upstream runs it on a
boundary already mapped onto the surface (`tetMeshGenerator::
mapMeshToSurface`: every boundary point to its nearest surface point,
then `untangleSurface`), with the patches `edgeExtractor` assigned, and
the patch mapper reads those patches. The port does the same in that
order: `surface_mapper::map_vertices_onto_surface_untangled` (the
pre-map; `untangleSurface` is not ported, so a cell it tangles keeps its
points), the nearest-triangle patches, `correct_edges_between_patches`
(children keep the parent's patch), then the mapper with those patches
(`map_vertices_onto_surface_patches_untangled_given`). The first run
showed pyramids inverted (the apex, the vertex average of a non-convex
rim cell, outside it); upstream leaves that to the final optimiser
(`optimiseFinalMesh`, `untangleMeshFV` 10 / 50 / 2), which the port runs
right after the capture (only interior points move: the LP untangler and
the boundary moves are not ported).

**Results (2026-11-05,** `diagnose_corners_rim` with
`CFMESH_STEPS=1`; per-point and cell-centre give the same boundary):

| h | `correct_edges` | concave faces / faces split / fanned / cells → pyramids | cells (point, centre) | reverted | \|Σ S·ẑ\|/πR² | Σ\|S\|/πR² | V/πR²L | meshing (s) |
|---|---|---|---|---|---|---|---|---|
| 4 | off | — | 6 353 | 128 | 0.95583 | 1.00816 | 0.99122 | 0.4 |
| 4 | on | 32 / 56 / 272 (point), 128 (centre) / 40 | 6 353 (**the capture is rejected**: 16 / 11 cells still inverted after `untangleMeshFV`, so the mesh is the uncaptured one) | — | 0.92496 | 0.96468 | 0.97207 | 330 / 232 |
| 5 | off | — | 3 113 | 0 | 0.98287 | 0.98287 | 0.98887 | 0.1 |
| 5 | on | 0 / 8 / 128, 48 / 8 | 4 305, 3 593 | 0 | **0.98094** | 0.98094 | 0.98888 | 0.2 |
| 6 | off | — | 1 939 | 64 | 0.94786 | 0.98572 | 0.98081 | 0.5 |
| 6 | on | 4 / 20 / 148, 12 / 72 / 15, 19 | 3 348, 2 701 | 56, 60 | **0.95138** | 0.95205 | 0.98155 | 32 / 13 |

**Against the predictions:** concave faces 0 — **missed** (4–32: the
pre-mapped rim is a zigzag of wall faces folded onto the inlet, whose
feature edges are partly concave); faces split = candidates ± 10 % —
missed low (16–56 of 24–72: the pre-map changes the patches, and the
concave step splits some first); reverted 128 → 0–20 and 64 → 0–10 —
**missed** (h 4: the capture is rejected outright, worse than before;
h 6: 56–60); inlet 0.985–0.993 (h 4) and 0.970–0.988 (h 6) — **missed**
(0.925 and 0.951); h 5 unchanged ± 0.002 — missed by 0.0019 low; cells
up — held where the capture is kept; max non-orthogonality up — held
(h 6: 57° → 74–82°).

**Reading it.** The port is upstream's algorithm (the unit tests pin
`patchCorrection` by hand on a cube, and a box is left alone), and it
does what upstream's code says; it does not repair this rim, for two
reasons that are not in `correctEdgesBetweenPatches`: (a) at every h the
`Corners` rim is a chain of chords between tet-edge midpoints that
misses the 48-gon's corners (h 5: 44 chords, all vertices at r 9.979–
9.993 mm, the inlet planar and still −1.7 %), and no topology step adds
a rim vertex; `OnFeatureEdge` keeps the rim points and its inlet is
0.991 at h 5; (b) at h 4 and 6 mm the mapped rim cells tangle, and what
upstream untangles them with (`untangleSurface`, the LP `meshUntangler`
and `untangleMeshFV`'s boundary moves) is not ported. Kept off by
default (it also costs 13–330 s); `TetDualOptions::correct_edges`
keeps it callable. **The gh:#1240 table's "inlet/πR²" for `Corners` is
corrected above**: it is Σ|S_f| (1.00816 at h 4), the projected section
the flow sees is 0.95583.

Tests: `correct_edges::tests::{patch_correction_splits_a_face_with_two_feature_edges_at_an_edge_point,
a_box_is_left_alone_and_pyramids_conserve_volume, face_centre_matches_openfoam_on_simple_faces}`,
`pipeline::tests::correct_edges_runs_in_the_pipeline_and_is_off_by_default`
(L 60 mm, h 6 mm: 605 → 1 727 cells, closed, none inverted).

### Item 2: the BackOffOnly 3-layer divergence

**Predictions (written 2026-11-05, before the diagnostic's first run).**
Meshes: `CellCentre` and `MedianPerPoint` with `Corners`, h 5 mm, 3
back-off layers (diverge in gh:#1240); the same with 2 layers on
`CellCentre` (runs) as the control. The solver's Δt is set from the
*mean* cell size (`pseudo_time_step`: Co ≈ 4 on the cell length, the
viscous number ν Δt / h² ≤ 0.5 on h = (V/N)^⅓), so a thin layer cell sees
a local viscous number ν Δt / Δ² far above 0.5.

1. *Where it starts.* The first cells to blow up are layer cells (a
   prism with a wall face, or the top prism of a stack), not dual cells
   of the core: 80 %.
2. *Local numbers.* The diverging meshes' largest local viscous number
   (Δ = V / max |S_f| of the cell) is 10–100 × the running 2-layer mesh's;
   the blow-up cell is within the 1 % of cells with the largest one: 60 %.
3. *Frozen mesh, pure diffusion* (the gh:#1202 manufactured Poisson
   problem, −∇²u = 4, u = 0 on the wall): the uncorrected operator gives
   b within 10 % of 1 on every mesh (the layer checks pass, and the
   Laplacian was within 2–8 % on the gh:#1202 meshes); the 20 deferred
   non-orthogonal correction passes converge on every mesh too, because
   the face non-orthogonality is ≤ 66°. So the Laplacian is **not** the
   cause: 70 %.
4. *Cause, if 1–3 hold:* the explicit parts of the PISO step (the lagged
   non-orthogonal correction of the momentum Laplacian and the explicit
   `linearUpwind` correction) on cells whose local viscous number is
   O(10²–10³); OpenFOAM keeps Δt from a Courant number on every cell
   (`CourantNo.H`, max over cells of ½ Σ|φ_f| Δt / V) and recommends
   `limited` snGrad on such meshes. Expected fix: limit the
   non-orthogonal correction on the layer faces, or the time step by the
   *local* cell size; to be decided by what the run shows, not before.

**Results of the diagnostic (2026-11-05,** `foam_studio_web/engine.rs`
`diagnose_three_layer_divergence`, Re 100, the demo pair, `Corners`):

| mesh | cells | Δt (s) | thin min (m) | local ν Δt/Δ² max | min face weight / vol ratio | Poisson b uncorr. / corr. (20 passes, change of pass 20) | blow-up (\|U\| > 20 U) |
|---|---|---|---|---|---|---|---|
| centre h 5, N 3 | 7 025 | 2.0 | 4.7e-5 | 899 | 0.008 / 0.0135 | 0.931 / 0.988 (2.6e-14) | step 13 |
| point h 5, N 3 | 7 025 | 2.0 | 6.5e-5 | 468 | 0.018 / 0.0120 | 0.909 / 0.988 (1.2e-16) | step 16 |
| centre h 5, N 2 (runs) | 5 721 | 2.0 | 7.8e-5 | 330 | 0.017 / 0.0135 | 0.931 / 0.988 (2.6e-14) | none in 300 |
| centre h 4, N 3 | 12 461 | 1.46 | 2.4e-5 | 2 644 | 0.006 / 0.0117 | 0.960 / 0.983 (6.0e-9) | step 21 |
| centre h 6, N 3 | 4 420 | 2.4 | 2.9e-5 | 2 776 | 0.024 / 0.0060 | 1.020 / 1.084 (2.3e-14) | step 24 |

Where it starts (centre h 5, N 3): the first cell above 3 U (step 8) is
a **dual cell at the inlet rim** (r 9.33, z 1.8 mm; the same cell at the
outlet rim in the mirrored runs), not a layer cell; it has no wall face,
local viscous number 4.4, and one face (|S| 7.7e-6 m², the largest of
its seven) to the **top prism of the stack** (Δ 7e-5 m, volume 0.11 of
its own, face weight 0.064); its other neighbours are the rim's dual
cells with face weights 0.008–0.06 to their own top prisms. At Δt / 10
the run holds 300 steps but that cell carries **u_r = −4.75 U** (radially
inward, five times the bulk velocity) with the face fluxes around it
small and continuity closed to 1e-10: the cell velocity is decoupled
from the fluxes. The pressure there: −7.7e-5 against 1.0e-4 in the top
prism next to it and ≈ 1e-5 in the other neighbours; a Gauss gradient
whose face values come 94 % from the prism carries that jump into the
cell's velocity.

Ablations on centre h 5, N 3 (the first cell above 3 U; blow-up step):
`linearUpwind` + `cellLimited` (base) 8; 13 · `Gauss upwind` 8; 13 ·
`Gauss linear` 6; 10 · no limiter 7; 13 · `uncorrected` laplacian 5; 8 ·
no non-orthogonal corrector (same as base) · **Δt / 4: 8; 13** ·
**Δt / 10: none in 300 (the 4.8 U rim mode stays)** · **Δt / 30: none,
max |U| 1.80 U on the axis** · Δt / 100: none. OpenFOAM's Courant number
(`CourantNo.H`, ½ Δt Σ|φ_f| / V) at the demo's Δt is **37–41** (max, in
the outlet cells by the axis; mean 2.3–2.9) on the 3-layer *and* the
2-layer mesh; Δt / 30 is Co ≈ 1.

**Against the predictions.** 1, layer cells first: **missed** (a rim dual
cell next to the top prism). 2, local viscous number: the diverging
meshes are 1.4–8 × the running one, not 10–100 ×, and the blow-up cell is
not among the 1 % largest (rank 4 883 of 7 025): **missed**. 3, the
Laplacian is not the cause: **held** (b within 10 % uncorrected except
h 6 corrected 1.084; every correction pass sequence converges). 4, the
cause in the explicit terms: **missed** (no scheme or corrector ablation
changes it); the time step does.

**Cause.** A pressure–velocity decoupled mode at the rim cell next to the
thinnest top prism, where the mesh fails what the back-off does not
check: OpenFOAM's `minFaceWeight 0.05` (0.008 here) and nearly
`minVolRatio 0.01` (0.0135). It grows when Δt is set as it is now, from a
nominal length (`pseudo_time_step`), which on these meshes is a Courant
number of 37–41 by OpenFOAM's definition; the PISO step that upstream
runs under `adjustTimeStep` / `maxCo` never sees that. With 2 layers the
top prism is thicker (weights ≥ 0.017) and the same Co 40 is survived.

**Predictions for the fix (written 2026-11-05, before its first run).**
Port OpenFOAM's `CourantNo.H` and `setInitialDeltaT.H` / `setDeltaT.H`
(`maxDeltaFactor` 1.2, `1 + 0.1 maxCo/Co`) as an opt-in of Foam Studio's
pipe (`FOAM_MAX_CO`, off by default, so both defaults are unchanged).
Centre h 5, N 3 at `maxCo 1`: runs to the steady criterion within the
5 000-step cap (70 %), u_c/2U 0.89–0.93 and dp/dz 0.98–1.04 × HP (the
2-layer row: 0.900, 1.029), no cell above 2.1 U. At `maxCo 4`: bounded,
with the rim mode (|U| 3–6 U at that cell), not steady-clean (60 %).

**The fix as built.** `foam_studio_web/engine.rs`: `courant_numbers`
(`CourantNo.H`) and `adjusted_delta_t` (`setInitialDeltaT.H` and
`setDeltaT.H`, `maxDeltaT` = the case's fixed step), applied after each
pipe step when `Engine::max_co` is set (natively `FOAM_MAX_CO`); off by
default, so both Foam Studio defaults keep their fixed step. Test:
`adjust_time_step_follows_openfoam` (hand values; with `maxCo 1` the
small O-grid's Co stays ≤ 1.2 and Δt falls; off, Δt stays).

**Results (2026-11-05,** `FOAM_MAX_CO=… FOAM_DUAL_KIND=centre
FOAM_DUAL_FP=corners foam_studio_web --headless-unstructured 5 3 0.3 1.2
100`, 7 025 cells):

| maxCo | steps to steady | solve (s) | final Δt (s) | u_c/2U | (−dp/dz)/HP | profile RMS | τ_w/HP | rim cell (r 9.33 mm, z 198.2 mm) |
|---|---|---|---|---|---|---|---|---|
| fixed step (Co ≈ 40) | diverged at 13 | — | 2.0 | — | — | — | — | blows up |
| 4 | 155 | 27.6 | 0.186 | 0.8998 | 1.0271 | 0.0482 | 1.0290 | \|U\| 1.87 U, u_r = **+1.80 U** |
| 1 | 519 | 74.1 | — | 0.8998 | 1.0271 | 0.0482 | 1.0290 | (same steady state) |

Against the predictions: runs at `maxCo 1` within the cap — **held**;
u_c/2U and dp/dz in range — **held** (and equal to the 2-layer mesh's
0.9001 / 1.0294 within 0.3 %); no cell above 2.1 U — held, narrowly
(1.87 U); `maxCo 4` bounded but not steady-clean — **missed**: it reaches
the same steady state to four digits in 155 steps. The rim cell still
carries a radial velocity of 1.8 U in the steady state (where u_r ≈ 0
physically): the mesh's decoupled mode, now bounded, not removed. So
the time step was why it *diverged*; the face-weight defect at the rim
is still there and is a mesher matter (the back-off checks no
`minFaceWeight`; thinner layers make it worse, cfMesh's thicker ones
avoid it).

Both defaults re-checked after the change: see the end of this
section.

### Item 3: snappy's cascade at 2+ layers

**Upstream read first (2026-11-05).** OpenFOAM-dev `852527377c7e`:
`snappyLayerDriver::checkAndUnmark` runs `meshCheck::checkMesh` on the
whole layered mesh and unextrudes *every point* of a patch face whose
added cells use a failing face; `meshCheck::checkFaceTets` and
`checkFaceTwist` (`polyMeshCheckQuality.C`) are the formulas the port has
(tet sign with the cell and face centres exchanged; twist against
`fc − cc` on a boundary face), so no formula defect is expected there.
Two things upstream has that the port does not:

1. `addPatchCellLayer::setRefinement` gives a patch face **`nFaceLayers`
   cells** (from `setupLayerInfoTruncation`: 1 on a face touching an
   unextruded point with `nBufferCellsNoExtrude 0`), the first spanning
   the rest of each column (`offset = addedPoints.size() − nFaceLayers`).
   The port computes `nFaceLayers` and then ignores it: every face gets
   as many cells as its most-layered point, so a face next to one
   unextruded point gets N cells **each pinched to zero thickness** at
   that point (a collapsed edge per layer).
2. The medial-axis mover shrinks the mesh under the quality checks
   (`medialAxisMeshMover::shrinkMesh` → `motionSmootherAlgo::scaleMesh`,
   `errorReduction 0.75`, `nRelaxIter` tries, then the displacement
   is zeroed only where faces still fail) and feeds the honoured
   displacement back (`patchDisp = oldPatchPos − localPoints`). The port
   keeps the full cap at every extruded point.

**Predictions (written 2026-11-05, before the diagnostic's first run),**
`CellCentre` + `Corners`, h 5 mm, 2 and 3 layers:

- iteration 1 (every point extruded, uniform stacks): 100–750 failing
  faces, tet quality and twist, as gh:#1240 found; the wall faces they
  unmark are a minority (< 40 %) of the wall;
- iteration 2 onward: **most failing faces are in the pinched stacks**
  next to unextruded points (> 70 % of the wall faces unmarked from
  iteration 2 on have a pinched cell), and the unmarked set grows ring by
  ring until coverage is 0: that is the cascade (80 %);
- with upstream's `nFaceLayers` (one wedge cell per face where a layer
  ends), the cascade stops: coverage at 2 and 3 layers ≥ 50 % (60 %),
  and N = 1 unchanged to the bit (no face of a 1-layer stack has fewer
  layers than its points: `nFaceLayers` = 1 = the count);
- BackOffOnly and CfMeshOptimise unchanged to the bit (they never mix
  counts).

**What was changed.** `layers::build_layered_faces` (upstream's
`nFaceLayers` cells per face, the last spanning the rest of each column,
its side split at the neighbour's levels), used by `SnappyCollapse`
under `SnappyLayerControls::face_cells = FaceLayerCells::Upstream` (the
new default of that strategy); `FaceLayerCells::MostOfItsPoints` keeps
the gh:#1215 builder callable. `setup_layer_info_truncation` now returns
the face counts it computed. Each layer iteration writes a note (faces
unextruded by `truncateDisplacement`, failing faces, wall faces using one
and how many of them are pinched, points still extruded).

**Results (2026-11-05,** `diagnose_snappy_cascade`, h 5 mm, `Corners`;
wall-face non-planarity of the bare mesh p50 0.064, p90 0.144 mm; the
per-point total (cap) p10 0.26, p50 0.42 mm, so the first layer is
0.45 × cap at N = 2 and 0.27 × cap at N = 3, p50 0.115 mm):

| case | rule | iteration 1: failing faces / wall faces unmarked (of 1 304) | then | coverage | cells |
|---|---|---|---|---|---|
| point N 1 | both | 205 / 22 | 4 more iterations, 8–23 faces each | 0.914 (bit-identical) | 4 305 |
| point N 2 | gh:#1215 | 1 877 / 40 | 38 faces a step for 20 steps, every one pinched | **0** | 3 113 |
| point N 2 | **upstream** | 1 877 / 40 | 10–24 a step, every one next to an unextruded point, converged at 22 | **0.429** | 4 233 |
| point N 3 | both | 2 056 / 188 | iteration 2: `truncateDisplacement` unextrudes 468 pinched faces at once | 0 | 3 113 |
| centre N 1 | both | 37 / 22 | 22 iterations | 0.626 (bit-identical) | 3 929 |
| centre N 2 | gh:#1215 | 2 917 / 344 | 336 pinched faces in iteration 2 | 0 | 3 113 |
| centre N 2 | **upstream** | 2 917 / 344 | 172 by `truncateDisplacement`, then 24–48 a step for 20 steps | 0 | 3 113 |
| centre N 3 | both | 3 096 / 492 | 468 by `truncateDisplacement` | 0 | 3 113 |

**Against the predictions.** Iteration 1 a minority of the wall (< 40 %):
held (3–38 %; the failing *faces* are 1 877–3 096, not 100–750, because a
failing face fails every layer cell around it). Most failing wall faces
pinched from iteration 2 on: held (100 % under the gh:#1215 rule). The
cascade stops with upstream's rule, coverage ≥ 50 % at 2 and 3 layers:
**missed** (point N 2: 0 → 0.429; centre N 2 and both N 3 stay 0). N = 1
bit-identical: held. BackOff and CfMesh unchanged: held (their pins pass).

**The explanation, in three parts.** (1) *A port defect, fixed:* the
gh:#1215 builder gave a face next to an unextruded point N cells each
pinched to zero thickness there; they fail the tet check and unextrude
their neighbours, ring by ring (the 2-layer cascade on the per-point
dual). (2) *Upstream's own rule on these faces:* `truncateDisplacement`
unextrudes every face with two separate unextruded runs; on 10–14-vertex
wall polygons with a scattered unextruded set that is most faces, so once
iteration 1 leaves 27–65 % of the points scattered, one pass empties the
wall (N = 3, and centre N = 2). Upstream applies the same rule
(`snappyLayerDriver.C` `nPinched`, read); on snappy's own hex wall faces
two separate runs on a quad are rare. (3) *Why iteration 1 fails so
much:* the layers are thinner than the wall faces are warped (at N = 3
the median first layer 0.115 mm against a p90 non-planarity 0.144 mm),
and a thin prism on a warped polygon fails `minTetQuality` and
`minTwist`; the check formulas are upstream's (`checkFaceTets`,
`checkFaceTwist` read side by side with `checks.rs`), the port's
difference is the vertex-average face centre against upstream's
area-weighted one (not isolated). Not ported, and the next lead:
`medialAxisMeshMover::shrinkMesh` (`scaleMesh` under the quality checks,
`errorReduction 0.75`) and its thickness smoothing; upstream feeds the
honoured displacement back instead of unextruding.

Tests: `layers::tests::per_face_layer_counts_conform_and_repartition`
(a box with one unextruded corner: closed, no inverted cell, volume 1 to
1e-12, 27 + 3 × 51 + 3 cells; equal counts give the old builder bit for
bit); `pipeline::tests::layer_strategies_are_deterministic` and
`every_strategy_leaves_a_good_box_duct_alone` pass unchanged.

### Item 4: the cost of `CfMeshOptimise`

**Upstream read first (2026-11-05).** cfMesh v1.1 `meshOptimizer::
untangleMeshFV` (`optimizeMeshFV.C`): per global iteration, an inner loop
(Knupp, the LP `meshUntangler`, the volume optimiser on the cells around
the bad faces, `nGlobalIter/2 + 1` layers) until no bad face or five
passes past the fewest; then, **if bad faces remain, a loop that moves
boundary points** (`optimiseBoundaryVolumeOptimizer(true)` for the first
two global iterations, `optimiseBoundarySurfaceLaplace` up to the fifth,
`optimiseBoundaryVolumeOptimizer(false)` after), and only then the next
global iteration. `optimizeLowQualityFaces`, `optimiseUsingVolumeOptimizer
(10)`, `partTetMesh`'s growth and `findBadFaces` with `changedFace` match
the port line for line (read side by side). The port has no LP untangler
and no boundary moves: a bad face held by boundary points cannot be fixed,
and the global loop then runs all its iterations on a growing region.

**Predictions (written 2026-11-05, before profiling):** on `CellCentre` +
`Corners`, h 5 mm, 3 layers (140–370 s in gh:#1240): ≥ 60 % of the
meshing time is in the three `untangleMeshFV(10, 50, 2)` calls, and in
them the volume optimiser's per-node search (not the region builds or the
checks); those calls run ≥ 8 of their 10 global iterations and end with
bad faces left; the faces left touch boundary points.

**Profile (2026-11-05,** `diagnose_dual_kinds_differential`, centre,
`Corners`, h 5 mm, 3 layers, cfMesh; each optimiser call now reports its
passes, global iterations, nodes smoothed and seconds in smoothing /
region builds / checks; native release, 4 cores, alone on the host):
110.5 s in all. The first `untangleMeshFV(10, 50, 2)`: bad faces
2 861 → 29 in 61 passes over **all 10** global iterations, 184 956 nodes
smoothed, **54.5 s** smoothing, 0.8 s regions, 0.2 s checks. The final
`untangleMeshFV(10, 50, 2)`: **29 → 29** in 60 passes over 10 global
iterations, 168 996 nodes, **50.8 s** smoothing. Everything else: 3 s.
So 95 % of the time is the volume optimiser inside the two full untangles,
and the second achieves nothing. Against the predictions: ≥ 60 % in the
untangles — held (95 %); smoothing, not regions or checks — held; ≥ 8 of
10 global iterations with faces left — held (10, 29 left).

**The bound, and its predictions (written 2026-11-05, before its first
run).** Upstream's boundary moves (`tetMeshOptimisation::
optimiseBoundaryVolumeOptimizer`, `optimiseBoundarySurfaceLaplace`) are
what make a later global iteration differ; without them a global
iteration that removes no bad face is followed by more of the same.
`mesh_optimizer::untangle_mesh_fv_bounded(…, stop_on_stall)` ends the
global loop there; `BoundaryLayerOptimisationControls::
stop_untangling_on_stall` (default `true`; `false` is the old run, kept)
applies it to the cfMesh strategy's two full untangles. The quality
targets (`findBadFaces`, `findLowQualityFaces`) are untouched. Predicted
on the same mesh: meshing 110 s → 10–30 s; bad faces at the end 77 ± 10
(the stall happens after the count stops falling, so what is left is the
same kind of face); max non-orthogonality within ±2°, mean total
thickness within ±2 %; the h 5 mm, 3-layer solve within ±0.5 % on u_c,
dp/dz and τ_w of 0.9314 / 1.0050 / 1.0056; the back-off and snappy
pins unchanged; the `CfMeshOptimise` pin in
`layer_strategies_are_deterministic` changes only if its box duct needs
a second global iteration.

### Item 5: `octree.rs` determinism

**Prediction (written 2026-11-05, before the fix's first run):** the
octree path (`refinement_levels > 0`) numbers its leaves in `HashSet`
order, seeded per process: three processes give three fingerprints
(90 %: a `HashSet` of a few hundred tuples practically never iterates in
the same order twice). With `BTreeSet` / `BTreeMap` (cells in
(level, i, j, k) order) three processes give one fingerprint, the cell
count is unchanged (the leaf *set* does not depend on the order), and the
default meshes (`refinement_levels = 0`) are untouched, the unstructured
default's fingerprint included.

**Results (2026-11-05,** `pipeline::tests::octree_path_is_pinned_bit_for_bit`,
three separate processes each): before the change, a 1 m box at 0.25 m
with 2 levels (2 528 cells) and the L 60 mm pipe at h 6 mm with one level
(664 cells) gave **three different fingerprints each** (box
7249328437174530281, 11578423670580479500, 9835399971238429003); after,
the same pair three times (16070505555120789497, 24335298658873933),
now pinned. Cell counts equal before and after. Held. The defaults: see
the end of this section.

## Reproduce

```bash
cargo test --release -p outram-foam-appbuilder-lib --test laminar_pipe_hagen_poiseuille \
    -- --ignored --nocapture measure_gh1155
cargo run  --release -p dhoby-ghaut --example foam_studio_web -- --render-mesh
# gh:#1215, the layer strategies
cargo test --release -p outram-park-fork-cfmesh --lib -- --ignored --nocapture diagnose_layer_strategies_differential
cargo test --release -p outram-park-fork-cfmesh --lib -- --ignored --nocapture diagnose_wall_faces_per_dual_cell
FOAM_LAYER_STRATEGY=cfmesh cargo run --release -p dhoby-ghaut --example foam_studio_web -- --headless-unstructured 5 3 0.3 1.2 100
cargo run  --release -p dhoby-ghaut --example foam_studio_web -- --render-layer-strategies
# gh:#1240, the dual kinds
cargo test --release -p outram-park-fork-cfmesh --features cell-centre-dual --lib -- --ignored --nocapture diagnose_dual_kinds_differential
CFMESH_FP=corners cargo test --release -p outram-park-fork-cfmesh --features cell-centre-dual --lib -- --ignored --nocapture diagnose_dual_kinds_differential
cargo test --release -p outram-park-fork-cfmesh --features cell-centre-dual --lib -- --ignored --nocapture diagnose_back_off_on_dual_kinds
FOAM_DUAL_KIND=centre FOAM_DUAL_FP=corners FOAM_LAYER_STRATEGY=cfmesh cargo run --release -p dhoby-ghaut --example foam_studio_web -- --headless-unstructured 5 3 0.3 1.2 100
cargo run  --release -p dhoby-ghaut --example foam_studio_web -- --render-dual-kinds
FOAM_DUAL_FP=corners cargo run --release -p dhoby-ghaut --example foam_studio_web -- --render-dual-kinds
```
