# Octree remainders: automatic refinement, refinement sources, addressing (gh:#1278, #1264–#1266)

**Research, education and V&V only** (`RESPONSIBLE_USE.md`). AI-assisted
draft; untrusted until a human has reviewed it.

Upstream: DAFoam/cfmesh `e944f6ba695f` (cfMesh v1.1 line). Every number
below was produced by the test named beside it, run with
`cargo test --release -p outram-park-fork-cfmesh --lib` on 2026-11-05.
Predictions were written in the test doc comments and committed before
the first run (commits `9133e65e5` and, for the two cases added after a
miss, `gh:#1278: record the proximity miss` and the irregular-edge commit).

## What was ported

| Upstream | Rust | Default |
|---|---|---|
| `meshOctreeAutomaticRefinement` (`.C`, `.H`, `Ref.C`) | `mesh_octree_automatic_refinement` | off: runs only when `MeshDict::min_cell_size` is set, as upstream without `minCellSize` |
| `help::numberOfFaceGroups`, `numberOfEdgeGroups` | `mesh_octree_automatic_refinement::{number_of_face_groups, number_of_edge_groups}` | — |
| `meshOctreeCreator::refineBoxesIntersectingSurfaces` / `EdgeMeshes` | `MeshOctreeCreator::refine_boxes_intersecting_{surfaces, edge_meshes}`, `MeshDict::{surface,edge}_mesh_refinement`, `EdgeMesh` | off (empty lists) |
| `subsetCellSize`, subset `localRefinement` entries | `MeshDict::{subset_cell_size, local_refinement_subsets}` | off |
| `anisotropicSources` on the sources | `mesh_octree_creator::{source_surface_geometry, source_edge_mesh_geometry}` through gh:#1279's `anisotropic` (`MeshDict::anisotropic_sources`; first a pass-through hook, filled after merging gh:#1279; test `anisotropic_sources_move_the_refinement_sources`) | off |
| `meshOctreeAddressing`: `createOctreeEdges`, `faceEdges`, `nodeEdges`, `edgeFaces`, `edgeLeaves`, `leafEdges`, `leafLeaves`, `nodeFaces`, `isIntersectedFace`, `isIntersectedEdge`, `edgeIntersections` | `MeshOctreeAddressing::{edge_addressing, leaf_leaves, node_faces, is_intersected_face, is_intersected_edge, edge_intersections}`, `OctreeEdges`, `cartesian_mesh::do_face_and_triangle_intersect` | computed on demand |
| `checkGluedRegions` (`checkForGluedMesh`) | `MeshOctreeAddressing::check_glued_regions`, run by `from_dict` | off |
| `checkAndFixIrregularConnections` (no upstream caller) | `MeshOctreeAddressing::check_and_fix_irregular_connections` | not called |
| `findUsedBoxes`: `nonManifoldMeshing`, `keep`/`removeCellsIntersectingPatches` | `CellSelection`, `MeshOctreeAddressing::from_dict`, `cartesian_mesh_extractor_from_dict` | off |
| (test helper) | `shapes::box_surface_subdivided` | — |

`createOctreeBoxes` now calls the sources and the automatic refinement in
upstream's place (after `refineBoundary`, before inside/outside); with the
default `MeshDict` they do nothing. Every existing pin passed unchanged
(below).

## Results

### Proximity refinement: two plates closer than the cell size

`mesh_octree_automatic_refinement::tests::proximity_refines_two_close_plates_until_the_gap_is_resolved`.
The slab `[0, 1]^2 x [0, g]`, triangles of side 0.02, one patch; only
`proximityRefinement` runs after the boundary refinement. Upstream's rule:
a triangle leaf is refined while the triangles within `1.732 h` of its
centre form more than one connected group.

| case | `g` | `maxCellSize` | prediction | result |
|---|---|---|---|---|
| 1 | 0.1 | 0.25 | interior plate leaves at level 6 (`g / h` 3.2) | **MISS:** nothing refined, plate leaves stay at level 3 (`g / h` 0.4) |
| 2 (after the miss) | 0.05 | 0.125 | interior plate leaves at level 7, `g / h` 3.2 | **as predicted:** 1 352 interior leaves all at level 7, `g / h` 3.2; 2 224 boxes marked in 4 passes |

**Why case 1 missed.** The prediction ignored the side walls. At level 3
(`h` 0.25) every leaf centre on a 1 m plate is within 0.375 of a side wall,
inside the range `s` = 0.433, so the wall joins the two plates into **one**
group and the criterion sees one part of the surface. Later passes test
only the children of refined leaves, so nothing ever starts. This is the
criterion as upstream writes it: proximity refinement needs the parts to
be separate **within** `1.732 h`, which a narrow channel closed at its
sides within that range is not. Case 1 is kept as a pinned observation.

What the criterion guarantees, derived: refinement stops once the other
plate is beyond `1.732 h` of a centre at most `h / 2` from its plate, so at
least `g >= 1.232 h` (1.232 leaves across the gap) wherever it runs. Case 2
holds 3.2. Ablation (no automatic refinement): the plate leaves stay at the
global level (case 2: level 4, `g / h` 0.4). The stopping rule held on
every triangle leaf below `maxRefLevel_` in both cases. Not predicted: in
case 2 1 504 leaves outside the interior square reached level 8 (see
`slab_xz_level.png`; not investigated further).

### Curvature refinement: sphere

`curvature_refines_a_sphere_to_the_curvature_criterion`. Sphere R 3 (24 x
48), `maxCellSize` 1, `minCellSize` 0.1. Rule: refine while `h > 0.2835 /
κ`. Predicted: all triangle leaves move from level 4 (`h` 1.0) to level 5
(`h` 0.5), for any estimate `κ` in (0.2835, 0.567]. **As predicted:** 176
leaves at level 4 -> 680 at level 5 in one pass; per-leaf `κ` 0.3343–0.3354
(+0.3 % to +0.6 % against `1 / R` = 0.3333); the criterion holds on every
leaf.

Through `createOctreeBoxes` (`create_octree_boxes_runs_automatic_refinement_only_with_min_cell_size`):
the sphere gives the same 680 level-5 triangle leaves (5 104 leaves); with
no `minCellSize` the report is empty. On the 3-patch pipe (R 10 mm,
`maxCellSize` 4 mm, `minCellSize` 0.5 mm, `maxRefLevel_` 8): 1 576 ->
11 824 leaves; triangle leaves at levels 6 (1 516), 7 (264), 8 (518) and
**9 (64)**; curvature marked 229, proximity 4, corners 0, partitions 0. The
pipe has no corners (a corner needs three patch-boundary edges at a point),
so the corner criterion is reached but marks nothing. The 64 level-9
leaves are one past `maxRefLevel_`: upstream adds a layer of neighbours
(`markAdditionalLayers`) and the 1-irregularity closure without the level
test. Where the level-8 leaves sit was not examined; a candidate is the
`calculateEdgeCurvature` defect recorded by gh:#1278's first half (23.0/R
at the four rim points where the point numbering wraps).

### Refinement sources and `subsetCellSize` (differential)

| test | routes compared | prediction | result |
|---|---|---|---|
| `surface_source_matches_boundary_refinement_on_the_same_triangles` | sphere as its own `surfaceMeshRefinement` source (+2 levels) vs `boundaryCellSize` 0.25 | same leaves, same order | **as predicted**, 10 816 = 10 816 |
| same | source `cellSize` 0.25 | 3 levels (level 7), not 2: the sources' `cellSize` loop counts `cs <= s (1 + SMALL)` | **as predicted**: level 7, 40 944 leaves |
| `edge_mesh_source_refines_the_leaves_it_crosses` | segment in the cube, +2 levels; thickness 0 and 1 | crossed leaves at >= 6; thickness refines more | **as predicted**: 38 crossed leaves all at level 6; 2 024 / 2 892 / 9 220 leaves |
| `subset_cell_size_matches_patch_cell_size_on_the_same_facets` | `subsetCellSize` vs `patchCellSize`; subset vs patch `localRefinement` (pipe inlet) | identical leaves | **as predicted**: 27 672 = 27 672; 23 864 = 23 864; unknown subset = plain (20 952) |

### Octree addressing and the extractor options

| test | prediction | result |
|---|---|---|
| `octree_edges_match_the_extracted_mesh_edges` | octree edges = the extracted mesh's face edges | **as predicted**: sphere 13 356, pipe 27 762, pair for pair; `from_dict(default)` = `new` |
| `intersected_faces_and_edges_against_brute_force` (sphere, DATA boxes) | every face / edge a triangle crosses (brute force over all 2 208 triangles) is reported | **as predicted**: faces 5 268 crossed, 5 292 reported, 0 missed; edges 2 634 crossed, 2 652 reported, 0 missed, all with an intersection point |
| `glued_regions_are_separated` (two unit cubes 0.05 apart, `maxCellSize` 0.2, DATA boxes) | 1 component without the check, 2 with it, >= 50 leaves removed | **as predicted**: 432 cells / 1 component -> 360 cells / 2 components, 72 leaves made `BOUNDARY`, closed |
| `irregular_connections_check` (sphere, pipe) | node test never fires; ends (0, 0); counts recorded | one pass, (0, 0), on both |
| `irregular_edge_between_two_diagonal_cells_is_removed` | two diagonal cells: passes `[(1, 0), (0, 0)]`, no cell left | **as predicted** |
| `non_manifold_meshing_keeps_unknown_leaves` | = keepCellsIntersectingBoundary on the sphere; keeps an UNKNOWN leaf | **as predicted**: 5 896 = 5 896; 5 896 vs 5 895 |
| `keep_and_remove_cells_intersecting_patches` (pipe inlet) | remove drops, keep adds, exactly the 704 inlet leaves; subset name = patch name | **as predicted**: 12 864 -> 12 160, 6 584 -> 7 288; closed |

## Upstream defects and quirks found (nothing filed)

1. `refineBasedOnContainedCorners` marks a leaf holding two corners
   **without** the `maxRefLevel_` test: two corners closer than
   `minCellSize` are refined past it, and two coincident corners would never
   terminate. Transcribed; no test surface has such corners.
2. `markAdditionalLayers` and the regularity closure in the automatic
   refinement ignore `maxRefLevel_`: measured, 64 pipe leaves at
   `maxRefLevel_ + 1`.
3. `checkAndFixIrregularConnections`' node test is dead code: a face's edges
   away from the node never have another boundary face at the node, so
   `watertightSurface` is false at every node with a boundary face. The
   routine also has no caller at this commit.
4. `checkGluedRegions` runs inside the addressing constructor after the
   node and face addressing is built and does not clear it: the extractor
   reads faces built with the old box types (a removed leaf's faces to
   `MESHCELL` neighbours become their boundary faces). Transcribed; the
   result was closed on the test.
5. The sources' `cellSize` loop gives one more level than `patchCellSize` /
   `boundaryCellSize` at a cell size of exactly `maxCellSize / 2^k`
   (measured: 0.25 -> level 7 against 6).
6. Proximity refinement needs the parts to be separate within `1.732 h`:
   a channel closed at its sides within that range is never refined (case
   1 above). Behaviour as written, recorded rather than a defect.

## Images

Generated by
`cargo run --release -p outram-park-fork-cfmesh --example mesh_octree_images -- <dir> remainders`,
from the assembled octrees (each pixel asks `findLeafContainingVertex`; the
surface is the cut triangles; mesh edges are the cut mesh faces):

- `slab_xz_level_before.png`, `slab_xz_level.png`, `slab_xz_level_zoom.png`:
  the case-2 slab at `y = 0.5` before and after proximity refinement.
- `sphere_xz_level_before_curvature.png`, `sphere_xz_level_curvature.png`.
- `edge_source_xy_level.png`, `edge_source_thick_xy_level.png`: the
  edge-mesh source at `z = 4.1`, thickness 0 and 1.
- `two_boxes_xy_glued.png`, `two_boxes_xy_checked.png`: the cartesian mesh
  without and with `checkForGluedMesh`.
- `pipe_xz_remove_inlet.png`: `removeCellsIntersectingPatches` [inlet].

What was checked in them is listed in the section below.

## What the images show (checked by eye, 2026-11-05)

- `slab_xz_level.png` (vertical scale exaggerated 2.4x; the slab outline is
  the black rectangle): a level-7 band covers the slab and one leaf
  either side, level 6 and 5 outside it, as the proximity rule predicts.
  In the zoom the plates (z = 0 and 0.05) sit inside level-7 leaves with
  leaf planes at z = -0.00625 and 0.05625, and 3.2 leaves across the gap,
  as derived. **Not explained:** two level-8 columns at x ~ 0.2 and
  x ~ 0.8 (not at the walls, which are at 0 and 1), across the slab
  thickness; they lie outside the interior square the test checks.
- `sphere_xz_level_curvature.png`: every leaf in the slice is at level 5
  (0.5 m), the surface leaves by the curvature rule and, presumably, the
  rest by the added layer and the regularity closure (the slice is within
  one leaf of the surface almost everywhere; not checked leaf by leaf).
  Nothing finer than 5, as predicted.
- `edge_source_xy_level.png`: level-6 leaves follow the segment from
  (1.3, 2.1) to (6.7, 5.9) with a level-5 halo. The segment itself is not
  drawn. `edge_source_thick_xy_level.png` was generated but **not
  opened**; `slab_xz_level_before.png` and
  `sphere_xz_level_before_curvature.png` likewise (the tests count their
  levels).
- `two_boxes_xy_glued.png` / `two_boxes_xy_checked.png`: in the glued mesh,
  mesh edges (blue) cross the gap plane x = 1.025 in every row; in the
  checked one there is no mesh face on x = 1.025 between the gap-facing
  leaves, matching the two components the test counts. Note the colouring:
  a DATA leaf revised to `INSIDE` by `reviseDataBoxes` is drawn green
  (the legend takes the first flag set), so some green leaves in the gap
  column hold triangles.
- `pipe_xz_remove_inlet.png`: the DATA leaves holding inlet triangles
  (z = 0, the bottom rows, and the two rim leaves) carry no mesh edges.
  **Not resolved by eye:** whether the row z in [0, 0.001] keeps cells;
  the test counts the removed leaves exactly (704) instead.

## Pins

All existing pins passed unchanged: the full crate lib suite (197 passed before
merging the shared branch, 215 after it and the anisotropic hook; 0
failed, 16 ignored, `cargo test --release -p outram-park-fork-cfmesh
--lib`), including `back_off_only_layers_are_pinned_bit_for_bit`,
`octree_path_is_pinned_bit_for_bit`, `layer_strategies_are_deterministic`,
`correct_edges_is_pinned_bit_for_bit`,
`foam_studio_default_mesh_keeps_its_fingerprint`, the tet_mesh and
cartesian_mesh restart pins and
`extractor_matches_octree_rs_build_mesh_on_the_same_leaves`; and, with
`--features cell-centre-dual`, `dual_kinds_are_pinned_bit_for_bit` and the
p_mesh tests.
