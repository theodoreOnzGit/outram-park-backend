# cfMesh `extrudeLayer`, the layer utilities, `createFundamentalSheets` and anisotropic meshing: V&V record (gh:#1280, #1281, #1279)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-written port, not yet reviewed by
a human.

**Upstream:** `github.com/DAFoam/cfmesh` at `e944f6ba695f` (cfMesh v1.1
line), read only. Upstream is not built or run here: every check below is
analytic, a differential against another route in this crate, or an
invariant. No result is compared with an upstream run.

## What was ported

| upstream | Rust | issue |
|---|---|---|
| `boundaryLayers/extrudeLayer` (serial) | `layers::extrude_layer` | gh:#1280 |
| `boundaryLayers/triangulateNonPlanarBaseFaces` | `layers::triangulate_non_planar_base_faces` | gh:#1280 |
| `meshOptimizer/symmetryPlaneOptimisation` | `symmetry_plane_optimisation` (called by `rename_boundary_patches`, as `checkSymmetryPlanes`) | gh:#1280 |
| utilities `improveMeshQuality`, `improveSymmetryPlanes`, `generateBoundaryLayers` (partial) | `mesh_utilities` | gh:#1280 |
| `createFundamentalSheets` (base, `JFS`, `FJ`) | `fundamental_sheets` | gh:#1281 |
| `meshSurfaceEdgeExtractorFUN::createBasicFundamentalSheets` | `mesh_surface_edge_extractor_fun` (now whole; `FunSheets::Without` is the ablation) | gh:#1281 |
| `anisotropicMeshing/*`, utilities `scaleMesh`, `scaleSurfaceMesh`, `surfaceGenerateBoundingBox` | `anisotropic` | gh:#1279 |

`patchesToSubsets` and `subsetToPatch` (item 26's list) were ported by the
surface track (`surface::features`, gh:#1276) before this work reached
them; not duplicated.

## Methodology and results

Every prediction was written in the test's doc comment before the test's
first run; misses are kept there, struck or labelled. Run:
`cargo test --release -p outram-park-fork-cfmesh --features cell-centre-dual --lib`.

### `extrudeLayer` (gh:#1280)

| test | case | prediction | result |
|---|---|---|---|
| `wrapper_sheet_on_a_box` | 3×3×3 unit box, every boundary face extruded into its owner | 27 + 54 cells; volume 1 to 1e-12; all cells positive; no edge/corner cell; ~~98~~ new points | **held**, except the new-point count: 56 (my count was wrong; 4³ − 2³ = 56 boundary points) |
| `internal_sheet_from_both_sides` | the 9 faces of the x = 1/3 plane, named from both sides | 18 layer cells, 32 new points, closed, all positive | **held** |
| `face_key_is_face_equality` | OpenFOAM's `face ==` (cyclic, either direction), `reverseFace` | | held |

The edge cells and corner cells of `createLayerCells` (layers that
self-intersect) were **not reached by any case here**: no test, the box
sheets or the pipe made one. They are transcribed, not verified.

### `triangulateNonPlanarBaseFaces` (gh:#1280)

`twisted_boundary_faces_are_fanned`: a 2×2×2 box, the corner (1, 1, 1)
lifted. Flat box: nothing marked (held). Lifted to z = 1.4: **missed**: I
predicted the top quad fanned on a hand margin of 0.186 against 0.184 with
the vertex mean as the centre; OpenFOAM's area-weighted `face::centre` moves
that margin and nothing is marked. Second prediction, written after the
miss and before its run, at z = 2.0: one face fanned, one cell into 9
pyramids (16 cells), volume unchanged to 1e-12, all positive: **held**
(volume 1.0625 both). No upstream caller exists at this commit (grep), so
no pipeline calls it.

### `symmetryPlaneOptimisation` (gh:#1280)

`perturbed_symmetry_planes_are_flattened`: 4×4×4 unit box, the `zMin`
points moved ±1e-3 in z. One `symmetryPlane`: the plane is z = 0 with
normal −z; every `zMin` point within 1e-12 of it; no other point moved.
Three planes (`xMin`, `yMin`, `zMin`): every patch point within 1e-12 of its
plane, the corner on all three. **All held.** `plane_intersections` checks
the OpenFOAM `plane` helpers analytically (held).

### Utilities (gh:#1280)

- `improve_mesh_quality_on_the_dented_box`: the utility against the same
  three `mesh_optimizer` calls made by hand (a **differential**, same
  route by construction: it checks the wiring and settings) — **bit for bit
  equal**; bad faces 3 → 0, matching item 1's untangle (0 left). Held.
- `improve_symmetry_planes_flattens`: held.
- `generate_boundary_layers_on_a_box`: 4×4×4 box, all patches: 160 cells,
  volume 1, 0 bad faces, all positive. Held. ~~**Partial port:**
  `optimizeBoundaryLayer` / `untangleBoundaryLayer` exist only inside the
  `CfMeshOptimise` strategy and are skipped (stated in the doc comment).~~
  **UPDATED 2026-11-05 (gh:#1280 remainder):** both are now callable on
  any mesh (`boundary_layer_optimisation::optimize_boundary_layer`, which
  finds the hairs with `detectBoundaryLayers` as upstream's
  `calculateHairEdges` does and runs the strategy's own `optimiseLayer`;
  `untangle_boundary_layer`) and `generate_boundary_layers` calls them
  where upstream does. `optimizeBoundaryLayer` does nothing unless
  `optimise_layer` is set (upstream's `optimiseLayer`, default off), so
  this test is unchanged. Its buffer layer (`activateSpecialMode`) is not
  ported.
- `generate_boundary_layers_with_optimise_layer_on_the_box` (predictions
  written before the run). ~~On the one-group layer: off = the recorded
  run bit for bit; on: 92 of 98 hairs thinned in one pass, thickness mean
  0.186 -> 0.146, max 0.267 -> 0.184, smallest cell 3.08e-3 -> 5.01e-3, 0
  bad faces, volume 1. Held.~~ **Re-measured after gh:#1270's remainder
  made `generateLayer` patch-wise (216 cells): the volume prediction
  missed.** On: 5 passes, 236 of 294 hairs thinned, thickness mean 0.138
  -> 0.106, min 0.125 -> 0.0225, smallest cell 2.10e-3 -> 1.12e-4, 0 bad
  faces at the end, **volume 1.00373**: the edge cells' boundary hairs
  thin to 2.5e-4, 262 faces go bad, and the untangles' boundary moves
  repair them off the box planes. Cause not established (upstream not
  run; first suspect: one hair kept per shared start point). Pinned as
  measured, not as a criterion; diagnostic
  `boundary_layer_optimisation::tests::diagnose_optimise_layer_on_the_patch_wise_box`.
- `generate_boundary_layers_with_optimise_layer_on_the_pipe` (R 10 mm, L
  60 mm, h 6 mm, `walls`): off 0 bad faces (**predicted some: missed**,
  `meshOptimisation`'s untangles repair them); ~~on: 1042 of 1456 hairs
  thinned, thickness mean 1.160e-3 -> 9.73e-4 (-16 %), max -16 %, 0 bad
  faces, smallest cell 6.61e-10 -> 3.11e-10 (halved), volume unchanged.~~
  re-measured on the patch-wise layer: 1241 of 1658 hairs thinned,
  thickness mean 1.264e-3 -> 1.062e-3 (-16 %), max -7 %, 0 bad faces,
  smallest cell 9.37e-10 -> 4.77e-10 (halved), volume unchanged.

### `createFundamentalSheets` (gh:#1281)

| test | case | prediction | result |
|---|---|---|---|
| `jfs_on_a_box` | 3×3×3 box, six patches, `JFS` | topology check false → wrapper (54 faces); closed, all positive, none unpatched, six patches; **volume below 1** | held except the volume: **1.12772 (missed)**. A new point on a feature edge moves against the summed normals of the copied faces at it, which for a boundary point is out through its cell's boundary face. FUN maps the boundary back afterwards. 207 cells; feature-edge sheet 126 faces, 168 new points, 72 new boundary quads |
| `fj_on_a_box` | same box, `FJ` with the wrapper | as JFS; **volume 1 to 1e-12** | held except the volume: **1.11451 (missed)**, the same mechanism (the non-patch cell at a feature edge is the next patch's wrapper cell). 275 cells |

### FUN in `pMesh`, differentially (gh:#1281)

Recorded in `../p_mesh_gh1273/README.md` (section "Re-measurement with the
fundamental sheets"): on the pipe (48 sides, 2 mm) the sheets take the mesh
from 4 303 to 9 781 cells, volume error −0.325 % → −0.320 %, max
non-orthogonality 55.29° → 58.24°, skewness 0.577 → 0.690, closed and
positive. The `FunSheets::Without` route reproduces the gh:#1269 numbers
exactly. Missed: the topology check passes (no wrapper sheet), where I
predicted a wrapper.

### Anisotropic meshing (gh:#1279)

`modify_then_revert_and_scaling`: `boxScaling` (centre (0.5, 0.5, 0.5),
lengths (0.4, 0.6, 1), scales (2, 0.5, 1)), `planeScaling` (origin
(0, 0, 0.2), normal z, distance 0.3, factor 3), and the two together, on an
11³ grid over [−0.5, 1.5]³: modify then revert returns every point to
**2.2e-16 / 1.1e-16 / 2.5e-16** (gate 1e-12; 1 111, 847 and 1 251 of 1 331
points moved). A box scaled ×2 in x has volume 2.000000000000001.
Overlapping and non-parallel combinations are refused. **All held.**
`surface_generate_bounding_box_appends_an_outward_box`: the 12 appended
triangles enclose +27 (outward, held). No generator calls the module yet
(`anisotropicSources` and `projectSurfaceAfterBackScaling` are not wired).

## gh:#1286 (the acceptance gate)

Recorded in
`crates/outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155.md`,
section "The pipeline's acceptance gate refuses zero-area faces".

## Images

Written by `examples/tet_mesh_images.rs` (command in
`../tet_mesh_gh1272/README.md`), from the assembled meshes.

| file | what |
|---|---|
| `box_xy_sheets_jfs.png`, `box_xy_sheets_fj.png` | the 3×3×3 box after JFS / FJ, x-y at z = 0.5 |
| `box_xy_z002_sheets_jfs.png`, `box_xy_z002_sheets_fj.png` | the same at z = 0.02 (inside the zMin wrapper layer) |
| `../p_mesh_gh1273/pipe_xy_p_mesh.png`, `pipe_xz_rim_p_mesh{,_without_sheets}.png` | `pMesh` on the pipe with and without the sheets |

**Checked by eye (2026-11-05):** `box_xy_sheets_jfs.png`: the wrapper
layer runs round the box; at the four vertical feature edges the new
points stand outside the box (the volume rise in the table), with a fan of
thin sheet cells in each corner; the interior cells are untouched.
`box_xy_sheets_fj.png`: the wrapper layer (about 0.09 thick: 0.4 × the
centre-to-vertex distance of a 1/3 face) and inside it a second ring, the
FJ sheets, of about the same thickness; the same outward corners. The
pipe images are described in the `p_mesh` record.

**Not checked:** the z = 0.02 images; 3D views; the anisotropic module (it
changes no mesh a pipeline makes, so nothing was drawn).

## Upstream defects found (not filed)

- `boxScaling::boundingPlanes` gives the x-max plane the normal
  `(1, 0, 1)` (typo). `checkForValidInverse` reads only the low planes'
  normals, so it has no effect. Kept as written.
- `extrudeLayer::addressingCalculator::faceSharingEdge` returns −1 when no
  other face shares the edge, and `createLayerCells` then indexes with it;
  here that is `ExtrudeLayerError::FaceSharingEdge`.
- `updateBoundary`'s `replaceBoundary` drops a one-cell face that gets no
  patch (the cell is then open); kept here with no patch and counted
  (`n_unpatched_boundary_faces`, 0 in every case run).
