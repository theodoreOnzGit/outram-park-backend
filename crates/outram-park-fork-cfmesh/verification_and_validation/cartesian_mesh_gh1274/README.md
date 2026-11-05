# cfMesh `cartesianMesh` as upstream: V&V record (gh:#1274)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-written port, not yet reviewed by
a human.

**Upstream:** `github.com/DAFoam/cfmesh` at `e944f6ba695f` (cfMesh v1.1 line),
`cartesianMeshGenerator.C` (`generateMesh`) and `executables/cartesianMesh`.

**Port:** `pipeline::cartesian_mesh` (the workflow, step table in its module
docs) and `premap_vertices` (`meshSurfaceMapper::preMapVertices` and
`faceMetricInPatch`, ported here because `mapMeshToSurface` calls it).
`layers::refine_boundary_layers::points_in_bnd_layer` is new
(`pointsInBndLayer`). Everything else is the earlier tracks' code.

## What upstream does that I checked before predicting

- `cartesianMeshGenerator` does **not** call `surfaceMorpherCells` (grep of
  `meshLibrary/`: only `tetMeshGenerator` and `voronoiMeshGenerator` do). Its
  `surfacePreparation` is the four connectivity checks (gh:#1268). So the
  morpher's −22 % volume on the carved sphere (gh:#1267) is **not on this
  route**.
- `generateBoundaryLayers` runs `boundaryLayers::addLayerForAllPatches`
  **unconditionally**: every cartesianMesh gets one layer, with or without a
  `boundaryLayers` dict. Only `refBoundaryLayers` needs the dict.
- `addLayerForAllPatches` is patch-wise (`patchWiseLayers_ = true`): patches
  separated by feature edges are separate groups with their own vertices
  (`otherVrts_`). The port has the one-group route only (gh:#1270). On the
  sphere (one patch) the two are the same; on the pipe they are not.
- `optimizeBoundaryLayer` acts only with `optimiseLayer` in the dict
  (default off). `projectSurfaceAfterBackScaling` returns at once without
  `anisotropicSources`.
- The template keeps only `INSIDE` leaves that hold no triangle
  (`keepCellsIntersectingBoundary` off), so its boundary lies inside the
  surface by up to one leaf.

## Predictions (written 2026-11-05, before any run)

Setup S: sphere R 3 m, 24 x 48 UV (analytic 113.0973 m³), `maxCellSize`
0.5, no other settings. Setup P: pipe R 10 mm, L 60 mm, 48 sides,
walls/inlet/outlet, `maxCellSize` 2 mm. Setup F: the Foam Studio pipe,
L 200 mm, `maxCellSize` 5 mm. Baselines at the same cell size:
`carve_box` + `snap_to_surface`, and `surface_to_tet_dual_mesh` (defaults,
no layers).

- **C1.** S template: every cell a hex; volume below the analytic value by
  10–25 % (the triangle-holding leaves are dropped).
- **C2.** `surfaceTopology` on S removes at most 2 % of the cells; a
  uniform octree on a smooth convex surface has few irregular connections.
- **C3.** `preMapVertices` (3 iterations) moves every boundary point (a
  patch is found everywhere) and leaves the mean distance to the surface
  between 1/8 and 1/2 of its start (`premap_vertices` test).
- **C4.** The final S mesh is closed (`validate()`), and every cell that
  touches no boundary point is a hex (the plan's test).
- **C5.** Right after the layer (before the optimiser), some S cells are
  non-positive: the caveat that the cfMesh one-group layer inverts cells on
  curved walls (56 / 304 on the pipes, gh:#1270). I predict 1–200 here, and
  0 non-positive cells after `meshOptimisation`.
- **C6.** Volume error on S: |error| < 2 % (at most the tet-dual's
  uniform-0.5 m 1.72 % plus a little). The layer itself moves the volume by
  less than 1e-6 relative (ablation `without_boundary_layer`).
- **C7.** `carve_box` + `snap` gives a larger |volume error| than
  `cartesian_mesh` on S, and non-positive cells or worse skewness (the snap
  has no untangling).
- **C8.** P and F: closed, 0 non-positive cells at the end, |volume error|
  < 1 %. Patches inlet / outlet / walls after `edgeExtractor`.
- **C9.** With a `boundaryLayers` dict (`nLayers` 3, ratio 1.2):
  `refineBoundaryLayers` splits the S layer (every layer cell has one base
  face) and the mesh stays closed. On P, the rim cells are layer cells for
  two base faces, so the port refuses (`NotPorted`, the known caveat) and
  the layer stays one cell thick; the workflow still finishes.
- **C10.** Stop after `edgeExtraction`, restart: the uninterrupted mesh bit
  for bit; two runs bit for bit.

## Results (2026-11-05, `cargo test --release`)

Tests: `pipeline::cartesian_mesh::tests::{diagnose_cartesian_mesh_on_the_sphere,
diagnose_cartesian_mesh_on_the_pipes, cartesian_mesh_with_refined_layers,
cartesian_mesh_stop_and_restart_reproduce_the_full_run}`,
`premap_vertices::tests::pre_map_vertices_halves_the_distance_on_the_sphere`.
`edgeExtractor` (gh:#1269) was merged in before the run and is the
`patchAssignment` step; `PatchAssignment::NearestTriangle` is its ablation.

### Differential

Volume error against the analytic value: 113.0973 m³ (sphere), the 48-gon
prism (pipes). The faceted sphere surface itself is −0.712 % (112.2922 m³);
the pipes' faceted surface equals the prism to 1e-15. "pyr ≤ 0": cells with a
non-positive face pyramid. Every mesh in the table passes `validate()` and
has 0 non-positive cells.

| case | route | cells | volume error | pyr ≤ 0 | max non-orth | max skew | time |
|---|---|---:|---:|---:|---:|---:|---:|
| sphere 0.5 m | `cartesian_mesh` | 1 128 | −1.938 % | 0 | 15.60° | 0.470 | 0.18 s |
| sphere 0.5 m | `cartesian_mesh`, `without_boundary_layer` | 600 | −1.938 % | 0 | 33.38° | 0.260 | 0.13 s |
| sphere 0.5 m | `carve_box` + `snap` | 912 | −1.719 % | 0 | 24.08° | 0.237 | 0.04 s |
| sphere 0.5 m | tet-dual (no layers) | 5 269 | −1.719 % | 0 | 72.46° | 1.095 | 2.89 s |
| pipe 60 mm, 2 mm | `cartesian_mesh` | ~~2 624~~ **2 688** | −0.659 % | 0 | ~~35.69°~~ **30.18°** | ~~0.487~~ **0.481** | 0.31 s |
| pipe 60 mm, 2 mm | without layer | 1 600 | −0.659 % | 0 | 51.62° | 0.644 | 0.16 s |
| pipe 60 mm, 2 mm | `carve_box` + `snap` | 2 400 | −0.544 % | 0 | 17.12° | 0.201 | 0.01 s |
| pipe 60 mm, 2 mm | tet-dual (no layers) | 13 411 | −0.544 % | 0 | 59.67° | 0.894 | 0.83 s |
| Foam Studio pipe, 5 mm | `cartesian_mesh` | ~~520~~ **536** | **~~−9.960 %~~ −9.959 %** | 0 | ~~38.47°~~ **12.01°** | ~~0.485~~ **0.489** | 0.06 s |
| Foam Studio pipe, 5 mm | without layer | 200 | −9.960 % | 0 | 28.12° | 0.576 | 0.05 s |
| Foam Studio pipe, 5 mm | `carve_box` + `snap` | 480 | −3.073 % | 0 | 10.83° | 0.270 | 0.00 s |
| Foam Studio pipe, 5 mm | tet-dual (no layers) | 3 113 | −3.073 % | 0 | 47.92° | 0.905 | 0.12 s |
| Foam Studio pipe, 5 / 1.25 mm | `cartesian_mesh` | ~~22 208~~ **22 320** | −0.156 % | 0 | ~~34.81°~~ **35.28°** | ~~0.493~~ **0.495** | 1.37 s |
| Foam Studio pipe, 5 / 1.25 mm | without layer | 13 000 | −0.156 % | 0 | 56.54° | 0.664 | 1.19 s |
| Foam Studio pipe, 5 / 1.25 mm | `carve_box` + `snap` (1.25 mm) | 33 280 | −0.082 % | 0 | 14.08° | 0.223 | 0.08 s |
| Foam Studio pipe, 5 / 1.25 mm | tet-dual (1.25 mm, no layers) | 177 249 | −0.082 % | 0 | 66.47° | 0.953 | 16.6 s |

The volumes of `carve_box` + `snap` and the tet-dual are equal on every case:
the tet-dual tetrahedralises the same snapped carve, so this is expected.

**RE-MEASURED 2026-11-05 (gh:#1270 remainder):** the `cartesian_mesh`
rows were taken with the one-group layer (`addLayerForAllPatches` as one
group, the port's stand-in). The patch-wise layer is now ported and is the
default, as upstream; the struck numbers are the old ones, the bold the new.
The sphere (one patch) is unchanged. The old route is callable as
`LayerTopology::OTopology`. Record:
`verification_and_validation/patch_wise_layers_gh1270/README.md`.

**Which number to quote:** the `cartesian_mesh` row with the layer is the
upstream workflow. The "without layer" row is the ablation.

### Predictions against results

- **C1 held in part.** Every template cell is a hex (600 on the sphere, 0
  decomposed). **The volume missed:** −33.7 % (75.0 m³), not the
  predicted −10 to −25 %. At 0.5 m on R 3 the dropped triangle-holding
  band is a whole leaf thick all round.
- **C2 held:** one pass, 0 cells removed (sphere and both pipes).
- **C3 held, just:** every boundary point found a patch. The mean distance
  went from 0.355 m to 0.0450 m, a ratio of 0.127 (the bound is 1/8 =
  0.125). On the pipe it went from 1.83 mm to 0.230 mm (0.126).
- **C4 held on the sphere:** the mesh is closed, and all 600 cells away from
  the wall are hexes (asserted).
  - On the pipe, 1 432 of the 1 600 cells away from the wall are hexes.
  - The other 168 are split hexes, with hanging points at octree level
    changes. Upstream keeps these as polyhedra unless
    `decomposePolyhedraIntoTetsAndPyrs` is set.
- **C5 missed on the sphere, held on the pipes.**
  - The one-group layer inverted **no** cell on the sphere (predicted
    1–200).
  - It inverted 32 cells on the 60 mm pipe, 64 on the Foam Studio pipe at
    1.25 mm, and 0 at 5 mm. **RE-MEASURED 2026-11-05 with the patch-wise
    layer:** 32, 64 and ~~0~~ **16** (5 mm). This is the known caveat (gh:#1270: 56 / 304
    on the tet-dual pipes).
  - After `meshOptimisation` every mesh has 0 non-positive cells. The
    untangler's own count reads 0 bad faces on entry, so the repair
    happened earlier in that step: in `optimizeSurface` or in
    `optimizeMeshFV`'s Laplacian pass. Which of the two did it is **not
    isolated**.
- **C6 held:** −1.938 % on the sphere, inside 2 %. Against the faceted
  surface the error is −1.23 %. The layer changes the volume by **exactly
  0** in floating point (with minus without, on all three cases;
  **CORRECTED 2026-11-05:** with the patch-wise layer, 0 on the sphere but
  2.7e-6, 2.5e-6 and 1.2e-6 relative on the three pipes, because the rim's
  new boundary points lie on the wall's averaged plane, not on its facet): the wall
  points do not move, and the old points move inward with their cells.
- **C7 missed.** `carve_box` + `snap` is closer in volume (−1.719 %), with 0
  non-positive cells and lower skewness (0.237 against 0.470).
  - The cartesian route has lower non-orthogonality only with the layer
    (15.6° against 24.1°).
  - The volume gap comes from the template. `cartesianMesh` keeps only the
    leaves that hold no triangle, which lie about one leaf inside the
    surface. Mapping that boundary across the gap leaves the outer cells
    longer radially, and the surface faceted by fewer points
    (`sphere_xz_cartesian_mesh.png`).
- **C8 held on the 60 mm pipe:** −0.659 %, closed, 0 non-positive cells.
  Patches: inlet 64, outlet 64, walls 896 faces.
- **C8 missed on the Foam Studio pipe at 5 mm:** −9.96 %. This is a
  resolution miss, not a defect.
  - The 5 mm request gives 4.69 mm leaves, and the triangle-holding ring is
    dropped. The template is 2 x 2 cells across the 20 mm bore.
  - Its boundary has 8 points around the circle, and the inscribed octagon
    is −10.0 %.
  - With `boundaryCellSize` 1.25 mm the error is −0.156 %.
- **C9 missed on the pipe** (**RE-MEASURED 2026-11-05: now held**: with
  the patch-wise layer the rim edge cells are layer cells for two base
  faces and the port returns `NotPorted`, as predicted; then gh:#1271's
  remainder, below in `patch_wise_layers_gh1270`). I predicted `NotPorted` at the rim. Instead
  the pipe refined (1 024 cells split, 0 non-positive, closed), as did the
  sphere (528 cells split, 2 184 cells).
  - The reason: the port's **one-group** layer gives every layer cell
    exactly one base face.
  - Upstream's patch-wise layer (not ported) puts rim cells between two
    base faces. That is where `refineEdgeHexCell` and
    `refineCornerHexCell` are needed.
  - So the `NotPorted` caveat is **not hit on this route**, because a
    second gap (the patch-wise layer) hides it. Both remain gaps.
- **C10 held:** stopped after `edgeExtraction` and restarted, the run gives
  the uninterrupted mesh bit for bit. Two runs agree bit for bit.

### Where the workflow meets the known caveats

1. **The morpher's −22 %: not met.** Upstream's `cartesianMeshGenerator`
   does not call `surfaceMorpherCells`, and the four checks change nothing
   on these templates.
2. **The one-group layer inverts cells on curved walls.**
   - Met on both pipes (32 and 64 cells), not on the sphere. All were
     repaired by `meshOptimisation` (C5 above).
   - On multi-patch surfaces the layer is still not upstream's, because
     patch-wise groups are not ported.
   - In `pipe_xz_rim_cartesian_mesh.png` the rim layer cell is pinched
     diagonally into the corner. Upstream's separate wall and inlet
     columns would not do that. **RE-MEASURED 2026-11-05:** with the patch-wise layer (now the default) the rim is not pinched; the regenerated image shows separate columns.
3. **`refine_boundary_layers` returns `NotPorted` for two-direction
   cells: not met**, for the reason in C9.

## Images (`examples/cartesian_mesh_images.rs`)

```text
cargo run --release -p outram-park-fork-cfmesh --example cartesian_mesh_images -- crates/outram-park-fork-cfmesh/verification_and_validation/cartesian_mesh_gh1274
```

| file | what |
|---|---|
| `sphere_xz_cartesian_template.png` | the sphere's `cartesianMeshExtractor` template, x-z at y = 0.01 |
| `sphere_xz_cartesian_mesh.png`, `sphere_xy_cartesian_mesh.png`, `sphere_xz_cartesian_mesh_zoom.png` | the `cartesian_mesh` result |
| `sphere_xz_carve_snap.png` | the baseline `carve_box` + `snap` |
| `pipe_xy_cartesian_mesh.png`, `pipe_xz_rim_cartesian_mesh.png` | pipe 60 mm at 2 mm: mid-length section and the outlet rim |
| `foam_studio_pipe_xy_cartesian_mesh.png`, `foam_studio_pipe_xz_rim_cartesian_mesh.png` | Foam Studio pipe at 5 mm (the 2 x 2 core of finding C8) |

**What I checked in them:**

- The template stops at the INSIDE leaves, one leaf inside the surface.
- In the result the boundary lies on the surface trace all round (sphere,
  pipe), with no fold.
- One layer of thin cells follows the wall, with radial spokes from the
  former template corners.
- The core stays a hex grid.
- ~~At the pipe rim the layer cell is pinched diagonally (caveat 2).~~ **REGENERATED 2026-11-05 (gh:#1270 remainder):** the rim images now show the patch-wise layer (wall and inlet layers with an edge cell); the one-group pinch is in `patch_wise_layers_gh1270/pipe_otopology_layer_stage_xz_rim.png`.

**What I could not check:** 3D views, and the patch of each face (faces are
not coloured by patch).

## Upstream quirks recorded

- `addLayerForAllPatches` runs in `cartesianMesh` with no `boundaryLayers`
  dict, so every cartesian mesh has one layer. Kept. The ablation
  `without_boundary_layer` is explicit and off by default.
- `workflowControls` numbers `meshOptimisation` (32) before
  `boundaryLayerGeneration` (64), but the generator calls them in the
  opposite order. The restart logic uses membership, not the values, so
  this has no effect; the restart test passes.
