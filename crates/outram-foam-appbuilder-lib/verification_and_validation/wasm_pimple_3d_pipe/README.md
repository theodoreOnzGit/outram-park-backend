# Stage 0 feasibility gate: a 3-D laminar pipe, meshed and solved by `PimpleFoam`, natively and in a browser Web Worker

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Issue:** gh:#1154 (stage 0 of the Foam Studio epic gh:#1153)
**Started:** 2026-11-03, from commit `8d7b2d7fa` (branch `claude/digital-twin-engine-vfseuf`)

This is a **cost and feasibility measurement**, with a quick physics sanity
check. It is not the pipe-flow V&V case; that is stage 1. The question is:
**can a 3-D laminar pipe be meshed and solved by outram-foam in a
`wasm32-unknown-unknown` Web Worker fast enough for an interactive browser
demo?**

## Predictions (written 2026-11-03, before any run)

Everything in this section was written before the harness had been compiled
or run. The results section below is compared against it, misses included.

### The case

A straight circular pipe, D = 0.02 m, L = 0.2 m (L/D = 10), ν = 1e-6 m²/s
(water-like), uniform inlet U = 5e-3 m/s, so Re = U D / ν = 100. Walls
no-slip; outlet zeroGradient U and fixed kinematic p = 0; inlet zeroGradient
p. Laminar, first-order upwind convection, Euler time (the `PimpleFoam`
defaults), PISO with 2 correctors.

Reference values the sanity check is judged against:

- Hagen–Poiseuille centreline velocity, developed: u_c = 2 U = 0.010 m/s.
- Developed kinematic pressure gradient: dp/dz = 32 ν U / D² = 4.0e-4 m/s².
- Hydrodynamic entry length (the usual laminar estimate L_h ≈ 0.05–0.06 Re D)
  is 0.10–0.12 m. Only the downstream half of the pipe is developed, so the
  check is made in z ∈ [0.13, 0.17] m. **Prediction:** u_c/(2U) there is
  0.97–1.00 (the flow is still finishing its development, and a coarse mesh
  with upwind convection under-predicts the peak), and dp/dz is within ±10 %
  of 4.0e-4 m/s² on the O-grid and within ±25 % on the cfMesh mesh (see
  non-orthogonality below).

### Mesh route

- **cfMesh (tet → dual → layers) from the `outram-blender` cylinder.**
  The background carve is isotropic cubes, and the dual of a tet mesh
  multiplies the cell count by about 30 per background cube (24751 dual cells
  from 780 background cubes on the meshing-nexus cylinder). A pipe with
  L/D = 10 at 5k cells therefore gets only about 3 background cubes across
  its diameter. **Prediction:** cfMesh can produce a valid (closed, no
  negative cells) pipe mesh, but its radial resolution at 5k–50k cells is
  poor (≤ 8 cells across D at 5k), and its non-orthogonality is high
  (max > 70°, as the cfMesh tests record for every layered mesh). This
  port's `fvm::laplacian` has **no explicit non-orthogonal correction**, so I
  expect the pressure gradient on the cfMesh mesh to be off by more than on
  a structured mesh.
- **O-grid, built as a blockMeshDict.** blockMesh skips arc edges, but an
  O-grid whose outer ring has one block per circumferential cell needs no
  curved edges at all: every block edge is straight and every outer vertex is
  on the circle, which is exactly what any FV mesh of a circle is. That reuses
  the ported blockMesh (and its reproducible face ordering, gh:#903) instead
  of writing a new mesher. **Prediction:** max non-orthogonality < 45°, no
  negative cells, and it allows a longer axial cell than a radial one, which
  is what a pipe wants.

**Prediction of the choice:** the O-grid will be the recommended route for
the stage 1 pipe, and cfMesh will be kept for arbitrary user shapes.

### Cells against mesh time (native, release)

| target cells | cfMesh tet-dual (s) | O-grid blockMesh (s) |
|---|---|---|
| 5k | 0.3 | 0.02 |
| 20k | 1.5 | 0.08 |
| 50k | 5 | 0.2 |

wasm in a Worker: **1.1–1.6× native** for both, as the RhoCentralFoam record
measured, since meshing is integer- and float-arithmetic code with no host
calls.

### ms per PIMPLE step (native, release; 1 outer, 2 PISO correctors)

The cost is dominated by the PCG pressure solve (tolerance 1e-8 relative,
warm-started). PCG iterations grow as N^(1/3) in 3-D, so the step cost grows
as about N^(4/3).

| cells | native ms/step | wasm ms/step |
|---|---|---|
| 5k | 30 | 45 |
| 20k | 150 | 220 |
| 50k | 500 | 750 |

That is ~6 µs per cell-step at 5k, about 15× the explicit Sod cost per
cell-step (0.41 µs), because each step is several GS momentum sweeps and two
PCG solves of a few hundred iterations each.

### Time to steady state

Started from rest, the profile develops by viscous diffusion on the time
scale R²/(j₀² ν) ≈ 17 s (j₀ = 2.405), and the inlet disturbance is swept out
in L/U = 40 s. **Prediction:** the centreline velocity at z = 0.15 m and the
pressure drop settle to within 1 % of their final values after about
100–150 s of physical time.

At an axial Courant number of about 1 on the centreline (Δt = Δz / 2U), that
is 200–300 steps at 5k, 400–600 at 20k and 600–900 at 50k. Native wall time:
**~8 s at 5k, ~75 s at 20k, ~6 min at 50k**; wasm ×1.5.

### What is predicted to go wrong

- **The pressure reference.** `PimpleFoam::step` calls
  `p_eqn.set_reference(0, 0.0)` on every solve. Upstream OpenFOAM's
  `fvMatrix::setReference` acts only when `psi.needReference()`, which is
  false as soon as any patch fixes the value (`fixedValue` p at the outlet).
  So this port, on an open pipe, pins an interior cell (cell 0, at the inlet
  end) to p = 0 while the outlet is also p = 0. **Prediction:** that cell
  becomes a mass sink, the outlet flux falls below the inlet flux by far more
  than 1 %, and the pressure field is wrong. If so, the fix is to port the
  upstream `needReference` guard, with a test.
- **Host imports.** The RhoCentralFoam module had none. **Prediction:** this
  one has none too (no clock, no threads, no filesystem on the solve path),
  provided nothing calls `std::time::Instant`; a hidden `HashMap` gives no
  import on `wasm32-unknown-unknown`.
- **Memory.** About 5–10 kB per polyhedral cell for mesh, fields and the
  LDU matrices: **≤ 500 MB at 50k**, well under the 4 GB wasm32 limit. A
  phone tab may still object.

## Methodology (as run)

**Code.** `src/lib.rs` holds everything the two targets share: the two mesh
routes, the case set-up (`build_case`), the diagnostics and a C-ABI surface
(`pipe_mesh_ogrid`, `pipe_mesh_cfmesh`, `pipe_to_fv`, `pipe_quality`,
`pipe_case_new`, `pipe_steps`, `pipe_diag`) that keeps one case in a
`thread_local`. No filesystem, clock or threads on that path.

- **Native:** `src/bin/native.rs` calls the same C-ABI functions and times
  each with `Instant`.
- **Browser:** the `cdylib` built for `wasm32-unknown-unknown` (release,
  overflow checks on) is loaded by `worker.js` inside a **module-less Web
  Worker in headless Chromium 153** (Playwright's bundled build), started by
  `run_worker.mjs`, which serves the folder from a local HTTP server and
  starts **one Worker per case** so each memory figure is that case's peak.
  Timing is `performance.now()` around each call. No wasm-bindgen: the module
  is instantiated with an empty import object.

This follows `../wasm_worker_timing/` (same crate layout, same release
profile), except that the Worker runs in **Chromium**, not Node
`worker_threads`.

**Mesh routes.**

- **O-grid:** `ogrid_block_mesh_dict` writes a blockMeshDict with a central
  `n × n` block (half-width 0.55 R) and `4n` outer blocks of `nr × 1` cells, so
  the wall is a `4n`-gon with every vertex on the circle. It goes through the
  blockMesh port (`outram_blender::unstructured::convert::block_mesh`).
- **cfMesh:** `outram_blender::primitives::cylinder(48, R, L)`, checked by
  `outram_blender::foam_mesh::mesh_to_surface`, moved to z ∈ [0, L], each
  triangle labelled `inlet` / `outlet` / `walls` by position, then
  `outram_park_fork_cfmesh::pipeline::surface_to_tet_dual_mesh_multipatch`
  (snap, tet, Delaunay, face-minimal dual) and `convert::cfmesh`.
- Both end as the neutral `UnstructuredMesh` and reach the solver through
  `convert::foam::to_fv_mesh`. The images and the quality numbers are taken
  from that same neutral mesh.

**Quality instrument:** `outram_foam_mesh::mesh_quality::assess_quality`
(checkMesh-style) on the polyMesh the solver gets.

**Time step:** Δt = Δz / 2U (axial Courant number ≈ 1 on the centreline);
Δz = L / nz on the O-grid, the mean cell size (V/N)^(1/3) on cfMesh. Two
larger steps were run on the 20k O-grid (Δt = 1.0 s and 2.5 s, Co ≈ 4 and 10).

**Steady state:** each run stops when the largest cell velocity change, per
flow-through time L/U, falls below 1e-3 U, or at a step cap. The number to
quote is the **1 % settling time**: the time after which both the centreline
velocity at z = 0.15 m and the pressure drop (mean p of the cells with
z < 1 cm) stay within 1 % of their final values.

**Diagnostics** (`pipe_diag`): centreline u_z from the intercept of a
u_z = a + b r² fit over the cells with r < R/2 in a 2 cm slab (exact for a
parabola); dp/dz from a p-against-z fit over every cell in z ∈ [0.13, 0.17] m;
outlet/inlet flux from `phi`'s boundary values.

**Host:** Intel Xeon @ 2.10 GHz, 4 logical CPUs, shared cloud container. Runs
were made one at a time with no other build or run active. Native step times
are given as best and mean: the mean is what a user waits for, and the best
shows the shared-host noise (they differ by 25–50 %).

## Results (2026-11-03)

### 1. The pressure-reference prediction held, and two more boundary defects were found

The first open-pipe run on the unmodified `PimpleFoam` gave an outlet/inlet
flux of **0.94 on step 1, 144 on step 2, then negative**, with the inlet-end
pressure pinned at 0. Reading the solver against upstream then showed two more
differences on open boundaries, both on the boundary flux:

1. `set_reference(0, 0.0)` was unconditional (upstream: only when
   `p.needReference()`).
2. The pressure source took the boundary flux from the U boundary *values*,
   which are 0 on a zeroGradient outlet; upstream uses `fvc::div(phiHbyA)`,
   which carries `HbyA_P·Sf` there.
3. `phi`'s boundary flux was never set to the prescribed `U_b·Sf` on
   fixed-velocity patches (`constrainHbyA`), nor corrected by the boundary
   pressure flux on fixed-p patches (`phi = phiHbyA − pEqn.flux()`).

All three were ported into `src/solvers/pimple_foam/mod.rs` (module doc,
item 7). After the fix, mass is conserved to round-off on every mesh
(outlet/inlet flux = 1 to 1e-9 or better; worst cell continuity error
~1e-11 of the inlet flow).

- **New test** `tests/pimple_foam_open_channel.rs` (plane Poiseuille,
  fixed-p outlet): mass mismatch 2.3e-11, u_max / 1.5 U = 0.9975,
  dp/dx / HP = 0.9950. With fix 1 undone it fails (flux ratio 0.597).
- **The fix moved every closed-cavity number, toward its reference.** Only
  item 3 acts on the cavity (the moving lid's flux in `phi` is now exactly
  `U_lid·Sf = 0`). Measured before and after on the same machine:

| cavity quantity | before | after |
|---|---|---|
| max \|U − U_icoFoam\| / U_lid, Re = 10 | 0.0144 | **0.0062** |
| Ghia Re = 100, 20×20, upwind: max / RMS | 0.0634 / 0.0363 | **0.0367 / 0.0207** |
| Ghia Re = 100, 20×20, `Gauss linear`: max / RMS | 0.0426 / 0.0224 | **0.0127 / 0.0057** |
| Ghia Re = 100, 41×41: max / RMS | 0.0194 / 0.0113 | **0.0179 / 0.0095** |
| Ghia RMS grid study 20 → 40 → 80 | 0.0363 → 0.0120 → 0.00455 | **0.0207 → 0.0100 → 0.00432** |
| BDF2 vs Euler steady max \|ΔU\|, Δt = 8e-3 … 1e-3 (BDF2 ddtCorr) | 3.80e-3 … 8.04e-3 | **2.13e-3 … 5.71e-3** |

  The observed grid-convergence order fell (1.60 / 1.40 → 1.05 / 1.21)
  because the coarse grid gained most. All 6 cavity tutorial tests,
  `fv_scheme_selection` and `turbulence_coupling` pass unchanged; the two
  regression pins in `bdf2_temporal_order.rs` study 3 were re-pinned, with
  the old values struck through and the reason given.

### 2. Mesh route and quality

| mesh | cells | max non-ortho (°) | mean non-ortho (°) | max skewness | negative cells | volume / πR²L |
|---|---|---|---|---|---|---|
| O-grid 6/4/40 | 5280 | 19.7 | 2.2 | 1.99 | 0 | 0.9886 (24-gon) |
| O-grid 8/6/80 | 20480 | 26.0 | 2.6 | 2.13 | 0 | 0.9936 (32-gon) |
| O-grid 10/8/120 | 50400 | 29.7 | 2.8 | 2.21 | 0 | 0.9959 (40-gon) |
| cfMesh h = 4.0 mm, no layers | 6353 | 50.8 | 12.3 | 0.91 | 0 | 0.9787 |
| cfMesh h = 2.6 mm, no layers | 21134 | 73.0 | 10.4 | 1.02 | 0 | 0.9886 |
| cfMesh h = 1.8 mm, no layers | 59047 | 62.5 | 6.6 | 0.94 | 0 | 0.9935 |
| cfMesh h = 8 mm, 3 layers (studio default) | 8226 | 83.6 | 18.7 | 2.18 | 0 | **0.8716** |

The O-grid volumes equal the inscribed-polygon prism to round-off (pinned by
the crate's test `ogrid_has_the_polygon_volume`).

**cfMesh is not usable for this pipe at 5k–60k cells:**

- **The cell count is erratic in the cell size.** h = 4.0 mm gives 6353
  cells, h = 4.2 mm gives 1470 (the pipeline skipped tetrahedralisation:
  "it produced inverted cells"), h = 1.8 mm gives 59047, h = 1.9 mm gives
  214200 (the dual was skipped, leaving the all-tet mesh; the solve went to
  NaN within 5 steps, max non-orthogonality 89°). A UI cannot offer a
  cell-count slider on top of that.
- **The boundary layers dominate and spoil the default mesh.** With the
  studio's 3 layers, 7488 of 8226 cells are prisms (3 layers on all 2496
  boundary faces, inlet and outlet included); the section is a
  non-convex 10-gon holding 12.8 % less volume than the pipe, and the max
  non-orthogonality is 84–87° at every size tried.
- **Without layers the interior is a mix of large octahedral dual cells and
  small fillers**, with only ~5 large cells across the diameter at 6k
  (images below).

**Choice: the O-grid**, for the pipe and the stage 1 demo. It is built from
the existing blockMesh port with no new mesher, its quality is good and does
not degrade with refinement, its cell count is exact and predictable, and it
lets axial cells be longer than radial ones. cfMesh stays the route for
arbitrary user shapes, but it needs work before it is trusted for flow (see
"Unresolved").

### 3. Images (drawn from the neutral mesh the solver gets)

In `images/`, regenerate with `cargo run --release --bin images -- images`:

| file | what |
|---|---|
| `ogrid_5k_xy.png`, `ogrid_20k_xy.png` | cross-section at z = 10.13 cm |
| `ogrid_5k_xz_inlet.png`, `ogrid_20k_xz_inlet.png` | x-z slice at y = 0.013 cm, first 4.4 cm |
| `ogrid_5k_xz_full.png`, `ogrid_20k_xz_full.png` | x-z slice, full 20 cm |
| `cfmesh_5k_*`, `cfmesh_20k_*` | the same three views, cfMesh without layers |
| `cfmesh_layers_8k_*` | the same three views, cfMesh with the default 3 layers |

**Checked by eye (2026-11-03):** the O-grid sections are a closed 24-gon /
32-gon of radius 1 cm with the central square and the radial ring as
intended, no gaps and no overlaps; the axial slices show uniform 5 mm /
2.5 mm cells from z = 0 to 20 cm. The cfMesh 6k section is not a 48-gon: its
boundary has long flat runs at x, y = ±0.98 cm (the snapped hex carve), and
its cells range from ~3.5 mm octahedra to sub-millimetre fillers. The layered
cfMesh section is an irregular 10-gon. **Not checked:** the faces between the
slices (only two planes were cut), and the patch assignment (patches are not
coloured; their face counts were printed instead: cfMesh 6k inlet 268,
outlet 268, walls 11968 faces).

### 4. Timing

Native = x86_64 release; wasm = Chromium 153 Worker, release. "To steady" is
to the stop criterion (1e-3); "1 % settle" is the number to quote for a demo.

| case | cells | mesh native / wasm (s) | →FvMesh native / wasm (s) | ms/step native best / mean | ms/step wasm best / mean | wasm/native (mean) | steps to steady | 1 % settle: t (steps) | wall to steady native / wasm (s) | wasm memory |
|---|---|---|---|---|---|---|---|---|---|---|
| O-grid, Δt 0.5 | 5280 | 0.039 / 0.147 | 0.003 / 0.014 | 16 / 21 | 26 / 46 | 2.2 | 65 | 15 s (30) | 1.4 / 3.0 | 10 MiB |
| O-grid, Δt 0.25 | 20480 | 0.112 / 0.248 | 0.008 / 0.021 | 92 / 128 | 162 / 227 | 1.8 | 122 | 14.5 s (58) | 15.6 / 27.6 | 35 MiB |
| O-grid, Δt 0.167 | 50400 | 0.313 / 0.343 | 0.025 / 0.039 | 301 / 438 | 435 / 638 | 1.5 | 178 | 14.3 s (86) | 78 / 114 | 83 MiB |
| O-grid, Δt 1.0 (Co ≈ 4) | 20480 | 0.116 / 0.163 | — | 121 / 168 | 214 / 267 | 1.6 | 34 | 14 s (14) | 5.7 / 9.1 | 35 MiB |
| O-grid, Δt 2.5 (Co ≈ 10) | 20480 | 0.098 / 0.153 | — | 144 / 220 | 232 / 341 | 1.6 | 114 | 170 s (68) | 25 / 39 | 35 MiB |
| cfMesh, Δt 0.214 | 6353 | 0.54 / 0.62 | 0.023 / 0.034 | 40 / 61 | 39 / 58 | 0.95 | 69 | 5.4 s (25) | 4.2 / 4.0 | 32 MiB |
| cfMesh, Δt 0.138 | 21134 | 2.28 / 2.69 | 0.099 / 0.095 | 167 / 255 | 189 / 277 | 1.1 | not reached in 3000 | 6.5 s (47) | — (400 steps: 102 / 111) | 106 MiB |
| cfMesh, Δt 0.102 | 59047 | 7.65 / 7.46 | 0.39 / 0.28 | 675 / 937 | 722 / 1078 | 1.15 | cap 400 | 7.5 s (74) | — (400 steps: 375 / 431) | 278 MiB |

**Host imports: none** (`WebAssembly.Module.imports` returned `[]` for every
case). **Wasm size: 817 916 bytes, 259 558 gzipped (-9).** Every diagnostic
the wasm runs returned agrees with the native run to all printed digits
(5–6 significant); bitwise agreement was not checked.

**Wasm/native is 0.95–2.2× on the mean step**, about 1.5× at 20k–50k. The
largest ratios are on the first, smallest case in each Worker (5k O-grid
mesh 3.8×, step 2.2×), consistent with Chromium running baseline (Liftoff)
code before optimised code is ready; this was not isolated.

### 5. Physics sanity check (not the V&V; that is stage 1)

| mesh | u_c / 2U at z = 0.15 m | at 0.17 m | dp/dz / (32 ν U / D²) | outlet / inlet flux |
|---|---|---|---|---|
| O-grid 5k | 0.9902 | 0.9924 | 1.0316 | 1.000000000 |
| O-grid 20k | 0.9940 | 0.9959 | 1.0318 | 1.000000000 |
| O-grid 50k | 0.9957 | 0.9975 | 1.0319 | 1.000000000 |
| O-grid 20k, **Re = 25** (ν = 4e-6) | 0.9983 | 0.9983 | 1.0239 | 1.000000000 |
| cfMesh 6k | 0.631 | 0.631 | 1.061 | 1.000000000 |
| cfMesh 21k | 0.687 | 0.687 | 1.194 | 1.000000000 |
| cfMesh 59k | 0.754 | 0.754 | 1.265 | 1.000000000 |

- **O-grid: within the prediction.** The centreline rises toward 2U with
  refinement and downstream; dp/dz is 3.2 % above Hagen–Poiseuille.
- **The entry-length assumption was tested and is not the main cause.** At
  Re = 25 (entry length ~0.03 m, so the slab is fully developed) u_c reaches
  0.998 × 2U, but dp/dz is still 2.4 % high. Part of the excess is the
  polygonal wall: at the same bulk velocity a 24-, 32- or 40-gon of the same
  circumradius has less area, which raises dp/dz by 1.15 %, 0.64 % or 0.41 %.
  The rest, **~1.7–2.8 %, does not shrink with refinement.** ~~Hypothesis, not
  tested: this port's `fvm::laplacian` has no non-orthogonal correction~~
  **TESTED 2026-11-03 (gh:#1161):** it was the missing non-orthogonal
  correction. With upstream's `Gauss linear corrected` ported, the Re = 25
  value is 1.0072 (0.08 % above the polygon value) and the Re = 100 values
  are 1.0175 / 1.0151 / 1.0136 (the slab is still developing at Re = 100);
  record: `../non_orthogonal_correction_gh1161.md`. The numbers in this
  README are the pre-gh:#1161 solver's, kept as run. The plane-channel test,
  on an orthogonal mesh, is within 0.5 %.
- **cfMesh fails the sanity check:** the profile is far too flat (u_c / 2U =
  0.63–0.75) and dp/dz is 6–27 % high, getting *worse* with refinement. Mass
  is conserved, so this is the discretisation on these meshes, not a leak.
  Not investigated further (see "Unresolved").
- Also: the mean pressure in the first centimetre settles at 9.2e-5 m²/s²
  (20k), against a fully developed 8.0e-5 for the whole length; the excess is
  of the size of the usual laminar entrance pressure loss, but no reference
  value was checked.

### 6. Predictions against results

| prediction | result | verdict |
|---|---|---|
| pressure pin makes cell 0 a sink | flux ratio 0.94, 144, then negative | **held** (and 2 more defects found) |
| O-grid non-ortho < 45°, no negative cells | 19.7–29.7°, 0 | held |
| cfMesh valid but poor; non-ortho > 70° | valid; 51–73° without layers, 84–87° with | held, and worse than predicted (erratic counts, sanity fails) |
| mesh time O-grid 0.02 / 0.08 / 0.2 s | 0.039 / 0.112 / 0.313 s | missed, 1.4–2× slower |
| mesh time cfMesh 0.3 / 1.5 / 5 s | 0.54 / 2.28 / 7.65 s | missed, 1.5–1.8× slower |
| wasm mesh 1.1–1.6× native | 0.98–2.2× (3.8× on the very first call) | mostly held |
| ms/step native 30 / 150 / 500 | mean 21 / 128 / 438 | held |
| ms/step wasm 45 / 220 / 750 | mean 46 / 227 / 638 | held |
| 1 % steady after 100–150 s of flow | **14–15 s** | **missed, 7–10× too pessimistic.** The L/U = 40 s "sweep" I added does not exist: in an incompressible start-up the plug flow is set everywhere on the first step, and only the local viscous development (R²/(j₀²ν) ≈ 17 s) remains. |
| wall to steady native ~8 s / 75 s / 6 min | 1.4 / 15.6 / 78 s | missed (pessimistic), from the line above |
| u_c/2U 0.97–1.00; dp/dz ±10 % (O-grid), ±25 % (cfMesh) | O-grid 0.990–0.996, +3.2 %; cfMesh 0.63–0.75, +6 to +27 % | O-grid held; cfMesh **missed** |
| no host imports | none | held |
| memory ≤ 500 MB at 50k | 83 MiB (O-grid), 278 MiB (cfMesh 59k) | held |

## Recommendation (for the maintainer to choose before stage 1)

1. **3-D is interactive up to ~20k cells, batch above.** In the browser, the
   20k O-grid reaches a 1 %-steady pipe in **14 steps × 0.27 s ≈ 4 s** at
   Co ≈ 4, and the 5k O-grid in **30 steps × 46 ms ≈ 1.4 s** at Co ≈ 1 (a
   larger Δt would be faster still; not measured). The 50k O-grid needs
   **86 steps × 0.64 s ≈ 55 s** at Co ≈ 1: a progress-bar job, not a slider.
2. **Cell cap: 20k by default, 50k as the maximum** with a "this will take a
   minute" warning. Memory is not the limit (83 MiB at 50k).
3. **Steady-first, by pseudo-transient PISO at Co ≈ 4.** There is no
   SIMPLE/simpleFoam port; a large Δt on `PimpleFoam` gives the same steady
   answer to all printed digits at Co ≈ 4 in 3.6× fewer steps (34 against
   122 to the stop criterion). Co ≈ 10 oscillates and is slower (170 s of
   flow to settle). Offer the transient (Co ≈ 1) as a second mode for
   watching the profile develop; it costs ~3× more wall time.
4. **Mesh the pipe with the O-grid**, promoted from this harness into a
   library (outram-foam-mesh or outram-blender) in stage 1, with its own
   test. **DONE 2026-11-03 (gh:#1155):** `outram_foam_mesh::block_primitives::PipeOGrid`
   (same blockMeshDict); the case set-up and fits are
   `outram_foam_appbuilder_lib::cases::laminar_pipe`. This harness's own copies
   (`ogrid_block_mesh_dict`, `build_case`, the fits) are superseded and kept
   only so the stage 0 numbers stay reproducible as run. Do not put cfMesh behind a pipe demo until its flow problems are
   understood.
5. **Before stage 1 counts as V&V:** decide on the non-orthogonal correction
   (the +1.7–2.8 % dp/dz residual) and the large-viscous-number instability
   below.

## Unresolved

- **cfMesh meshes give wrong flow** (u_c 0.63–0.75 of 2U) although mass is
  conserved and no cell is negative. Candidates, none tested: the
  missing non-orthogonal and skewness corrections on strongly non-uniform
  dual cells; under-resolution by the large octahedral cells; the cell-centre
  definition of non-star-shaped dual cells (196 of 6353 at 6k).
- **cfMesh cell count jumps** with small cell-size changes because whole
  stages (tet, dual) are skipped by the graceful-degradation gate.
- **dp/dz +1.7–2.8 % on the O-grid** that does not converge away (above).
- **`PimpleFoam` is unstable at large viscous number.** The plane-channel test
  first ran at ν Δt / Δy² = 8 and diverged (|U|max 0.11 → 0.30 m/s in 10
  steps, then NaN, with mass conserved at every step); at 0.8 it is stable.
  The pipe cases ran at about 2 and were stable. Not investigated.
- The Worker ran in Chromium on a desktop-class x86 host; Firefox, Safari and
  phones are unmeasured. Run-to-run noise on this shared host is 25–50 %.
- `rayon` (`parallel`) was not enabled; the solve is single-threaded in both
  targets.

## Reproduce

```bash
cd crates/outram-foam-appbuilder-lib/verification_and_validation/wasm_pimple_3d_pipe
cp ../../../../Cargo.lock .            # pin the workspace's versions
cargo test  --release                  # O-grid volume, fit, settling, mass
cargo run   --release --bin native -- ogrid 8 6 80 0.25 3000
cargo run   --release --bin native -- cfmesh 0.0026 0 48 0.138 400
cargo run   --release --bin native -- ogrid 8 6 80 0.25 3000 4e-6   # Re = 25
cargo run   --release --bin images -- images
cargo build --release --lib --target wasm32-unknown-unknown
node run_worker.mjs target/wasm32-unknown-unknown/release/foam_wasm_pipe.wasm
rm -rf Cargo.lock target
```

`run_worker.mjs` needs `require('playwright')` to resolve and its bundled
Chromium. This folder is **not a workspace member**; it declares its own
empty `[workspace]`, as `../wasm_worker_timing` does.
