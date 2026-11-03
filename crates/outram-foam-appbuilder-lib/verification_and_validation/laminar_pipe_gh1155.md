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

## Reproduce

```bash
cargo test --release -p outram-foam-appbuilder-lib --test laminar_pipe_hagen_poiseuille \
    -- --ignored --nocapture measure_gh1155
cargo run  --release -p dhoby-ghaut --example foam_studio_web -- --render-mesh
```
