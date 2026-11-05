# cfMesh `cartesian2DMesh`: V&V record (gh:#1275)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-written port, not yet reviewed by
a human.

**Upstream:** `github.com/DAFoam/cfmesh` at `e944f6ba695f`.

## What is ported

| upstream | crate |
|---|---|
| `cartesian2DMeshGenerator`, `executables/cartesian2DMesh` | `pipeline::cartesian_2d_mesh` |
| the quadtree in `meshOctree` (`isQuadtree_`, `refineCube2D`, negative `posZ`, the 2D neighbour searches, `findEdgeCentre`) | `mesh_octree`, `mesh_octree_queries`, `cartesian_mesh` |
| `cartesianMeshExtractor`'s empty patches | `cartesian_mesh::create_empty_patches` |
| `polyMeshGen2DEngine` | `poly_mesh_gen_2d_engine` |
| `meshSurfaceMapper2D` (`adjustZCoordinates`, `preMapVertices`, `mapVerticesOntoSurface`, `mapVerticesOntoSurfacePatches`, `mapCorners(edges)`, `findMappingDistance`) | `surface_mapper_2d` |
| `meshSurfaceEdgeExtractor2D` (`distributeBoundaryFaces`, `remapBoundaryPoints`) | `surface_mapper_2d` |
| `triSurfaceExtrude2DEdges`, `triSurface2DCheck::is2DSurface` | `tri_surface_2d` |

**Reused from the 3D workflow, as upstream reuses it:**
`cartesianMeshExtractor`, the four connectivity checks, `boundaryLayers`
(the one-group route, with the empty patches untreated as 2D mode leaves
them), `renumberMesh`, `renameBoundaryPatches` and `workflowControls`.

**Not ported (each is a note in the report):**

- `meshSurfaceOptimizer::optimizeSurface2D` and `untangleSurface2D` (the
  `optimiseMeshSurface` step, which runs twice);
- `refineBoundaryLayers::activate2DMode`;
- the patch-wise `boundaryLayers` groups;
- `triSurfacePatchManipulator` and `triSurfaceCleanupDuplicateTriangles`;
- `anisotropicSources`;
- the no-argument `mapCorners()` (nothing calls it);
- MPI;
- `utilities/extrudeEdgesInto2DSurface` (a `main` around the extruder);
- `triSurface2DCheck::createSubsets`.

## Predictions (written 2026-11-05, before any run)

Channel: outline `[0, 1] x [0, 0.25]`, extruded by `triSurfaceExtrude2DEdges`
(height 0.1 x the outline's diagonal, 0.10308 m). Regions: walls (y = 0,
y = 0.25), inlet (x = 0) and outlet (x = 1). `maxCellSize` 0.05. The 3D
comparison is the same channel as a closed box with z in `[0, 0.5]` and the
front and back in `frontAndBack`, meshed by `cartesian_mesh`.

- **D1.** The extruded rectangle passes `is2DSurface`. A sphere and a
  tilted ribbon fail it.
- **D2.** Every quadtree leaf has `posZ = -10` and spans the root box in z.
  The template is one cell thick: each cell has exactly one face in each
  empty patch. `polyMeshGen2DEngine` builds (half the points at z min).
- **D3.** The final mesh is closed. Every point lies **exactly** on z = 0
  or z = 0.10308 (`adjustZCoordinates`), half on each. 0 non-positive
  cells. Patches: walls, inlet, outlet, bottomEmptyFaces, topEmptyFaces,
  with the last two `empty`.
- **D4.** The template's x-y layout is **identical** (every node to 1e-9)
  to the 3D template's nodes on its mid-plane z = 0.25. The root boxes
  have the same x-y extent (both are set by L = 1), and the 3D leaves
  are the same in x-y at mid-height.
- **D5.** The final meshes' x-y layouts are **not** identical. The 3D
  layer is grown on all six sides as one group, the 2D layer only on the
  side patches, and the 3D mesh is also optimised. Predicted: at least
  half the 2D nodes still coincide with 3D mid-plane nodes (the interior
  ones).
- **D6.** The area (volume / thickness) is within 1e-9 of L x H = 0.25:
  the outline is axis-aligned, so mapping is exact, and the layer
  conserves the volume.

## Results (2026-11-05, `cargo test --release`)

Tests: `pipeline::cartesian_2d_mesh::tests::{channel_2d_against_the_3d_mid_plane,
quadtree_leaves_span_the_root_in_z}`,
`tri_surface_2d::tests::extruded_outline_is_a_2d_surface`.

| | 2D (`cartesian_2d_mesh`) | 3D (`cartesian_mesh`, box z in [0, 0.5]) |
|---|---:|---:|
| template cells | 72 | 576 (8 layers of 72) |
| template nodes in x-y (2D) / on z = 0.25 (3D) | 95 | 95, **identical to 1e-9** |
| final cells | 116 (72 + 44 layer cells) | — |
| final nodes, z = 0 / z = dz | 139 / 139 | 139 within 0.01 of z = 0.25 |
| area (volume / dz) against L x H = 0.25 | +4.4e-16 relative | — |
| non-positive cells | 0 | 0 |
| max non-orthogonality, skewness | 34.26°, 0.817 | — |

Patches: walls 36, outlet 4, inlet 4, bottomEmptyFaces 116,
topEmptyFaces 116 (types patch, patch, patch, empty, empty).

### Predictions against results

- ~~**D1 missed on the rectangle; held otherwise.**~~ **CORRECTED
  2026-11-05 (gh:#1288): D1 holds.** `eigen_values_symm` lacked upstream
  ESI v2306's effectively-diagonal branch; with it the rectangle's
  covariance gives `(0, 4, 4)` exactly and `is2DSurface` says 2D
  (`extruded_outline_is_a_2d_surface` now asserts it).
  - ~~Its normal covariance is `diag(4, 4, 0)`. `outram_foam_basic_lib`'s
    `eigen_values_symm` returns `(-1.68e-8, -0, 4)` for it instead of
    `(0, 4, 4)`, so `is2DSurface` reports "3D space". The double root is
    lost in the cubic solve; the defect is in that crate, not in this port.~~
  - The channel ribbon, whose covariance is the same `diag(4, 4, 0)`,
    passed in the workflow run. Round-off in the unit normals decides
    which way the check goes.
  - An irregular quadrilateral passes. The sphere and the tilted ribbon
    fail, as predicted.
  - In the workflow the check only writes a note, as upstream's only
    writes a warning, so the mesh is not affected.
- **D2 held.** Every quadtree leaf has `posZ = -10` and spans the root box
  in z, with at most 8 face neighbours. Each of the 72 template cells has
  one face in each empty patch. Each point pairs with exactly one point
  across the thickness.
- **D3 held.** The mesh is closed. All 278 points lie exactly on z = 0 or
  z = 0.10308 (139 each), and every cell has one bottom and one top face.
  0 non-positive cells. Patches as predicted.
- **D4 held.** The 2D template's 95 x-y nodes are, to 1e-9, the 3D
  template's 95 nodes on z = 0.25. This is the issue's "x-y layout
  identical to item 20's mid-plane". The root boxes share their x-y
  extent; in z they differ (the ribbon's z centre is 0.0515, the box's
  0.25).
- **D5 missed.** I predicted at least half the final nodes would still
  coincide. Only 4 of 139 do, to 1e-9.
  - The 3D mid-plane still has 139 nodes within 0.01 of z = 0.25 (the
    largest deviation is 4.0e-7). In x-y they are a mean 8.2 mm and a
    maximum 28.5 mm (0.6 cell) from the nearest 2D node.
  - The cause is that the two finished workflows differ.
    - 3D: `meshOptimisation` runs `optimizeMeshFV`, whose Laplacian moves
      every interior node. Its layer also covers the front and back.
    - 2D: upstream's `meshOptimisation` is only `optimiseMeshSurface`
      (2D), which is not ported here. Nothing moves inside.
  - Compare `channel_2d_xy.png` with `channel_3d_midplane_xy.png`.
- **D6 held:** the area is 0.25 to +4.4e-16.

### Where the 2D workflow meets the known caveats

1. **The morpher's −22 %:** not met. `cartesian2DMeshGenerator` does not
   call `surfaceMorpherCells`.
2. **The one-group layer:** used, over walls, inlet and outlet as one
   group with the empty patches untreated. 0 cells inverted on this
   straight channel. At the four corners the corner cell is pinched
   diagonally (`channel_2d_xy.png`): this is the same multi-patch
   difference from upstream's patch-wise layer as in 3D.
3. **`refine_boundary_layers` `NotPorted`:** not reached. The 2D
   refinement (`activate2DMode`) is not ported at all, so a
   `boundaryLayers` dict gives a note and no split.

## Images (`examples/cartesian_mesh_images.rs`, second argument)

```text
cargo run --release -p outram-park-fork-cfmesh --example cartesian_mesh_images -- crates/outram-park-fork-cfmesh/verification_and_validation/cartesian_mesh_gh1274 crates/outram-park-fork-cfmesh/verification_and_validation/cartesian_2d_mesh_gh1275
```

| file | what |
|---|---|
| `channel_2d_xy.png` | the 2D mesh, x-y at mid-thickness, over the quadtree's leaf types |
| `channel_2d_xz.png` | x-z at y = 0.1: one cell thick; every leaf spans the root box in z |
| `channel_3d_midplane_xy.png` | the 3D `cartesian_mesh` of the box channel, x-y at z = 0.2501 |

**What I checked in them:**

- The 2D mesh fills the outline, and its boundary lies on the outline.
- The quadtree leaves are columns over the whole root height
  (`channel_2d_xz.png`), and the mesh is one band from z = 0 to 0.103.
- The 2D and 3D cores share the same x-y grid.
- The 3D layout is smoothed (curved lines near the inlet and the outlet),
  where the 2D layout is not.
- The corner cells are pinched diagonally in both.

**What I could not check:** 3D views; patch colours.

## Upstream defects and quirks found (none filed)

- `cartesianMeshExtractor`'s empty-patch test reads
  `mag(n.z) > mag(n.x + n.y)`. It sums the components, where a magnitude
  was probably meant: a face with `n = (1, -1, 0.1)` counts as
  z-dominated. Ported as written; it does not matter on axis-aligned
  quadtree faces.
- `meshSurfaceMapper2D::mapCorners()` (no arguments) looks up mesh point
  labels in `corners()`, which holds boundary-point labels. Nothing calls
  it.
- `meshSurfaceEdgeExtractor2D::distributeBoundaryFaces` leaves the patch
  of an inactive face that is at neither z unset. The port returns `Err`
  there.
- ~~`outram_foam_basic_lib::primitives::eigen::eigen_values_symm` returns
  `(-1.68e-8, -0, 4)` for `diag(4, 4, 0)` (D1). This is a defect in
  another crate.~~ **FIXED 2026-11-05 (gh:#1288)** for a diagonal
  covariance. A covariance with a repeated root and off-diagonals above
  `ROOTSMALL` still loses a root, as upstream ESI's cubic does.

## Not done in this issue

- ~~`meshSurfaceOptimizer::optimizeSurface2D` and `untangleSurface2D`
  (about 270 upstream lines). With them, the 2D `edgeExtraction` and
  `meshOptimisation` steps would smooth the side.~~ **Done in the
  gh:#1275 remainder, below.**
- ~~`refineBoundaryLayers` 2D mode.~~ **Done, below.**
- ~~The patch-wise layer.~~ **Done in gh:#1270's remainder**
  (`patch_wise_layers_gh1270/README.md`, P9).
- ~~A 2D stop and restart test.~~ **Done, below.**

# gh:#1275 remainder: `optimizeSurface2D`, `untangleSurface2D`, the 2D `refineBoundaryLayers`

**Upstream:** `meshSurfaceOptimizerOptimizeSurface.C` (`optimizeSurface2D`,
`untangleSurface2D`), `meshSurfaceMapper2DMapVertices.C`
(`mapVerticesOntoSurfacePatches(edgesToMap)`), `refineBoundaryLayers.C`
(`activate2DMode`), `detectBoundaryLayersFunctions.C` and
`refineBoundaryLayersFunctions.C` (the 2D branches),
`cartesian2DMeshGenerator.C` (`optimiseMeshSurface` in `edgeExtraction`
and `meshOptimisation`; `refBoundaryLayers` with `untangleSurface2D`).

**Port:** `surface_optimizer::{optimize_surface_2d, untangle_surface_2d}`,
`surface_mapper_2d::map_edges_onto_surface_patches_2d` (the selected-edge
form; the all-edge function now calls it),
`layers::refine_boundary_layers::{refine_boundary_layers_2d,
detect_boundary_layers_mode}`.

**What upstream does (read before predicting):** `optimizeSurface2D`
smooths the z-min outline points along the outline and maps the edges
across the thickness back onto their patches, then smooths the z-min points
inside the bottom patch, copying every move to the z-max point across.
`untangleSurface2D` smooths, up to 20 times, the z-min points of the bad
faces. The 2D refinement only takes the empty faces out of every layer; the
rest is the 3D split.

## Predictions (written 2026-11-05, before the 2D steps were wired or run)

The channel `[0, 1] x [0, 0.25]` at 0.05, as above.

- **S1.** In `edgeExtraction` the template is a uniform grid with a straight
  outline: `optimizeSurface2D` moves no point by more than 1e-12. In
  `meshOptimisation` (after the patch-wise layer, whose cells are half a
  cell thick) the smoothing moves the points next to the layer, so the
  final layout changes; the outline stays on the straight sides (area L x H
  to 1e-12), one cell thick, closed, 0 non-positive cells.
- **S2.** `untangleSurface2D` on the finished channel finds 0 bad faces at
  its first check and stops.
- **S3.** A finished channel with one interior z-min point (and its z-max
  partner) moved past its neighbour: at least one bad face, which
  `untangle_surface_2d` reduces to 0.
- **S4.** With a `boundaryLayers` dict of `nLayers` 3: the 44 layer cells
  over the outline split into 3, the 4 corner edge cells into 9: 120 − 48 +
  132 + 36 = **240 cells**; one cell thick; area L x H to 1e-12; closed; 0
  non-positive.
- **S5.** Stopped after `edgeExtraction`, and after
  `boundaryLayerGeneration`, then restarted: the uninterrupted mesh bit for
  bit; two runs bit for bit.
- **S6 (re-measuring D5).** With the 2D smoothing the final 2D layout is
  closer to the 3D mid-plane: the maximum distance to the nearest 2D node
  falls below the 28.5 mm (0.6 cell) of D5.

## Results (2026-11-05, `cargo test --release`)

Tests: `pipeline::cartesian_2d_mesh::tests::{channel_2d_surface_optimisation_and_refined_layers,
cartesian_2d_mesh_stop_and_restart_reproduce_the_full_run,
channel_2d_against_the_3d_mid_plane}`.

- **S1 missed on the first half, held on the second.** On the mapped
  template (after `remapBoundaryPoints`) `optimizeSurface2D` moves 40
  outline and 51 bottom points by up to **3.6e-2** (an outline point) and
  3.4e-2 (an inner point), against the predicted 1e-12: the cell is 0.047,
  so this is most of a cell. No outline point leaves its straight side, the
  area stays 0.25 to the last digit, and no cell is non-positive. My
  premise, that the mapped template is a uniform grid, is wrong where
  `remapBoundaryPoints` stretches the boundary cells across the gap
  between the template and the surface; I have not checked the spacing
  point by point. The finished channel: 120 cells, area 0.25 (+2.2e-16),
  one cell thick, closed, 0 non-positive, max non-orthogonality 4.00°
  (9.94° before), max skewness 0.012 (0.051).
- **S2 held.** `untangleSurface2D` on the finished channel: bad faces `[0]`.
- **S3 held.** One bottom point (and its top partner) dragged 0.08 in x:
  5 bad faces, `untangle_surface_2d` iterations `[5, 0]`, 0 bad faces and 0
  non-positive cells after.
- **S4 held.** `nLayers` 3: 48 cells split (44 prisms, 4 edge hexahedra
  across the thickness), 240 cells, area 0.25, one cell thick, closed, 0
  non-positive; `untangleSurface2D` after it found `[0]`.
- **S5 held.** Stopped after `edgeExtraction` (resumed at
  `boundaryLayerGeneration`) and after `boundaryLayerGeneration` (resumed
  at `meshOptimisation`): the uninterrupted mesh bit for bit, with the
  `nLayers` 3 dict; two runs bit for bit.
- **S6 held.** The 2D layout against the 3D box channel's mid-plane (both
  now with the patch-wise layer, the 3D one in four groups): 147 2D nodes,
  nearest 3D slab node mean 1.7 mm, **max 6.9 mm (0.15 cell)**, was 28.5 mm
  (0.6 cell) in D5. Only 4 nodes coincide to 1e-9, as before.

**D5 re-measured:** ~~mean 8.2 mm, max 28.5 mm (0.6 cell)~~ mean 1.7 mm,
max 6.9 mm (0.15 cell), with `optimiseMeshSurface` (2D) and the
patch-wise layer in both routes. The record above D5 is kept as it was
measured.

### Images (regenerated)

`cargo run --release -p outram-park-fork-cfmesh --example cartesian_mesh_images
-- crates/outram-park-fork-cfmesh/verification_and_validation/cartesian_mesh_gh1274
crates/outram-park-fork-cfmesh/verification_and_validation/cartesian_2d_mesh_gh1275`

| file | what |
|---|---|
| `channel_2d_xy.png`, `channel_2d_xz.png` | regenerated: the patch-wise layer and the 2D surface smoothing |
| `channel_2d_corner_xy.png` | the inlet corner: wall and inlet layers with the edge cell between them |
| `channel_2d_refined_3_xy.png`, `channel_2d_refined_3_corner_xy.png` | `nLayers` 3: three layers on each side, a 3 x 3 block at the corner |

**What I checked:** the refined corner shows three inlet and three wall
layers meeting in a 3 x 3 block; the outline stays on the surface trace.
**What I could not check:** the octree overlay makes the whole-channel
image hard to read; I checked the corners in the zooms only.

