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

- The dual mesh's flat profile and high dp/dz (above): measure the port's
  corrected Laplacian and Gauss-linear gradient on the dual against the
  exact quadratic; compare against OpenFOAM on an exported mesh
  (`export_polymesh`).
- 2+ layers diverge; 1 layer runs.
- No feature-edge capture: bent end faces at the rim, chamfered wall.
- ~~`limited` snGrad with a coefficient in (0, 1) is not ported.~~ Ported
  2026-11-04. `cellLimited` gradients and `linearUpwind` / `limitedLinearV`
  convection, which the tutorials use, were not tried here.

## Reproduce

```bash
cargo test --release -p outram-foam-appbuilder-lib --test laminar_pipe_hagen_poiseuille \
    -- --ignored --nocapture measure_gh1155
cargo run  --release -p dhoby-ghaut --example foam_studio_web -- --render-mesh
```
