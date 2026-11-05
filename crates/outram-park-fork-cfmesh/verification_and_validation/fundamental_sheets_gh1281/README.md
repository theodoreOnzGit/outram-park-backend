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
  repair them off the box planes. ~~Cause not established (upstream not
  run; first suspect: one hair kept per shared start point). Pinned as
  measured, not as a criterion~~ **FIXED 2026-11-05 (gh:#1293, section
  below):** a port defect in `optimiseThicknessVariation`'s neighbour
  hairs; now volume 1 to 1e-12, every boundary point on its plane, gated;
  diagnostic
  `boundary_layer_optimisation::tests::diagnose_optimise_layer_on_the_patch_wise_box`.
- `generate_boundary_layers_with_optimise_layer_on_the_pipe` (R 10 mm, L
  60 mm, h 6 mm, `walls`): off 0 bad faces (**predicted some: missed**,
  `meshOptimisation`'s untangles repair them); ~~on: 1042 of 1456 hairs
  thinned, thickness mean 1.160e-3 -> 9.73e-4 (-16 %), max -16 %, 0 bad
  faces, smallest cell 6.61e-10 -> 3.11e-10 (halved), volume unchanged.~~
  re-measured on the patch-wise layer: 1241 of 1658 hairs thinned,
  thickness mean 1.264e-3 -> 1.062e-3 (-16 %), max -7 %, 0 bad faces,
  smallest cell 9.37e-10 -> 4.77e-10 (halved), volume unchanged.

### Hairs at shared start points (gh:#1293)

**What upstream does** (read 2026-11-05, DAFoam/cfmesh `e944f6ba695f`,
`boundaryLayerOptimisationThickness.C`, `optimiseThicknessVariation`): for
hair `heI` and each boundary face `bf` at its start point that does not
hold its end, it takes **the hairs of `bf`'s owner cell**
(`hairEdgesAtBndFace(cell, bf)`: at each point of `bf`, the cell edge
leaving `bf` there) and compares `heI` only with the hairs in
`hairEdgesAtBndPoint_` that **equal** those cell edges. Every hair at a
point is kept (`detectBoundaryLayers::generateHairEdges` drops only
duplicate edges).

**Where the port departs:** `Hairs::optimise_thickness_variation` took, at
each point of `bf`, `Hairs::at_point[p]`: the *first* hair starting at
`p`, whatever cell it belongs to. On the patch-wise box a box-edge point
starts two or three hairs (one per patch's layer; 76 of 294 hairs share a
start point), so an edge hair was compared with the other patch's hair at
the **same** point (`|dv| = 0`, which `calculateThickness` turns into
"take the shorter length") or with a hair of another cell, and not with
the hair of its own cell. `hairEdgesNearHairEdge` (from_mesh), the normal
smoothing and `calculateNormalVectors` already follow upstream.

**Predictions (written 2026-11-05, before the fix was run):**

1. `CfMeshOptimise` is unchanged **bit for bit**: its one-layer mesh has
   one hair per wall point, the only boundary faces at a wall point that do
   not hold the hair are its prisms' wall faces, and their cell edges are
   exactly the `at_point` hairs. `layer_strategies_are_deterministic` and
   every other pin pass. No gate is then needed (if this misses, the old
   rule stays the default and the upstream rule becomes a named variant).
2. Box (`generate_boundary_layers_with_optimise_layer_on_the_box`): the
   edge cells' `BOUNDARY | ATEDGE` hairs stop collapsing: smallest hair
   **above 1e-2** (was 2.5e-4 on the diagnostic, 0.0225 at the end),
   fewer bad faces after `optimiseLayer` than the 262 recorded (sign
   certain if the departure is the cause, magnitude not: I predict
   under 122, the first-pass count), no boundary point off its plane at
   any stage, and **volume 1 to 1e-12**. Mean thickness rises from 0.106
   toward the off-run's 0.138 (still at most it). Confidence is moderate:
   the geometric decay (×0.1–0.35 per pass) looks like the
   `featureSizeFactor` limit, which this departure feeds only through the
   wrong neighbour; if the box still moves, the remaining cause is
   elsewhere and is recorded as such.
3. Pipe (`..._on_the_pipe`): the pipe's layer has no box edges, but its
   end caps make patch-border points with more than one hair; thinned
   count and mean thickness move by under 5 %, 0 bad faces stays.

**Results (2026-11-05, after the fix; `--release`):**

| prediction | result |
|---|---|
| 1. `CfMeshOptimise` bit for bit | **held.** `layer_strategies_are_deterministic` passes on its 2026-11-04 fingerprints (7725 cells), as do `back_off_only_layers_are_pinned_bit_for_bit`, `octree_path_is_pinned_bit_for_bit`, `correct_edges_is_pinned_bit_for_bit`, `foam_studio_default_mesh_keeps_its_fingerprint`, `dual_kinds_are_pinned_bit_for_bit` (`--features cell-centre-dual`) and both `*_stop_and_restart_reproduce_the_full_run`. No pin moved, so the upstream rule is applied unconditionally: no gate, no named variant. |
| 2. box | **held, and better than predicted.** `optimiseLayer` stops after **1** pass (was 5), 48 of 294 hairs thinned (was 236); thickness min **0.125** (was 0.0225; prediction >1e-2) / mean 0.1289 (off-run 0.1382) / max 0.1475; **0 bad faces after every stage of every pass** (was 122 -> 262; prediction <122); 0 boundary points off their planes at every stage; smallest cell 1.95e-3 (was 1.12e-4); **volume 1.000000000000** (gate 1e-12, now asserted with the planes in `generate_boundary_layers_with_optimise_layer_on_the_box`). |
| 3. pipe | **held trivially:** the pipe's layer has **no** hairs sharing a start point (`n_shared_start` 0), so it is unchanged to the digit: 1241 of 1658 thinned, mean 1.062e-3, 0 bad faces, smallest cell 4.77e-10. My premise (end caps give shared starts) was wrong: the layer is on `walls` only. |

Upstream was not run (not installed here); by its source, its
`optimiseThicknessVariation` uses exactly the neighbours the port now
uses, so the box should not move under upstream either.

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
triangles enclose +27 (outward, held). ~~No generator calls the module yet
(`anisotropicSources` and `projectSurfaceAfterBackScaling` are not wired).~~
**UPDATED 2026-11-05 (gh:#1279 remainder):** the four generators call it;
record in `../anisotropic_gh1279/`.

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
