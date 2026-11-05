# cfMesh patch-wise `boundaryLayers`: V&V record (gh:#1270 remainder)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-written port, not yet reviewed by
a human.

**Upstream:** `github.com/DAFoam/cfmesh` at `e944f6ba695f` (cfMesh v1.1
line), `meshLibrary/utilities/boundaryLayers/boundaryLayers/`:
`boundaryLayers.C` (`findPatchesToBeTreatedTogether`, `addLayerForPatch`,
`addLayerForAllPatches`, `activate2DMode`, `createOTopologyLayers`),
`boundaryLayersCreateVertices.C` (`createNewVertices(labelList)`),
`boundaryLayerCells.C` (`createLayerCells`), `boundaryLayersI.H`
(`findNewNodeLabel`, `createNewCellFromEdge`, `createNewCellFromNode`),
`boundaryLayersWrapperLayer.C`, `boundaryLayersCheckTopologyOfBndFaces.C`;
`helperFunctionsGeometryQueriesI.H` (`isSharedEdgeConvex`,
`angleBetweenFaces`).

**Port:** `layers::boundary_layers::{BoundaryLayers,
find_patches_to_be_treated_together, create_new_vertex,
is_shared_edge_convex, angle_between_faces}`. `create_new_vertex` is now the
one copy of `createNewVertex` that both the one-group route
(`boundary_layers_add_layer`, unchanged in behaviour) and the patch-wise
route call.

## What upstream does that I checked before predicting

- `addLayerForAllPatches` is patch-wise by default (`patchWiseLayers_ =
  true`): every untreated patch gets a layer, each **group** of patches
  (`treatPatchesWithPatch_`) with its own layer vertex at a point where
  groups meet (`otherVrts_`), an extra hex cell along each feature edge
  between two groups (`createNewCellFromEdge`) and one at each point where
  three groups meet (`createNewCellFromNode`).
- Patches are grouped across **concave** edges (not convex by the tet test,
  or angle over 0.75 π) and at corners with more than three feature edges.
  Patches meeting only at convex 90° edges (box faces, pipe rims) are
  separate groups.
- The original boundary points are moved inward (to `p + Σ d_group`); new
  labels take `p`. So the wall stays where it was.
- `cartesianMeshGenerator::generateBoundaryLayers` calls
  `addLayerForAllPatches` always; `tetMesh`, `pMesh` and the
  `generateBoundaryLayers` utility call it with a `boundaryLayers` dict
  holding `nLayers > 0`, and `addLayerForPatch(name)` for each
  `patchBoundaryLayers` entry otherwise. `cartesian2DMesh` calls
  `activate2DMode` first (empty patches marked treated).
- `addWrapperLayer` and `checkTopologyOfBoundaryFaces` have no caller in
  cfMesh. In `addWrapperLayer` the new labels go to a local variable that
  shadows the member the face builder reads (upstream defect).
  `checkTopologyOfBoundaryFaces` stops with "Critical. Not tested" whenever
  it would decompose a cell.

## Predictions (written 2026-11-05, before any run)

Box B6: unit box carved at h 0.25 (64 hexes), one patch per side (`x0`,
`x1`, `y0`, `y1`, `z0`, `z1`). Box B3: the same box with `walls` on the
four y/z sides, `inlet` (x = 0), `outlet` (x = 1). Pipe P: the tet-dual
bare pipe R 10 mm, L 60 mm, h 6 mm (`walls`, `inlet`, `outlet`), as in the
gh:#1270 one-group tests.

- **P1 (B6).** Six groups of one patch. 64 + 96 face cells + 48 edge cells +
  8 corner cells = **216 cells**; every displacement is 0.125 (to 1e-12);
  every point's coordinates lie in {0, 0.125, 0.25, 0.5, 0.75, 0.875, 1}
  (1e-12), i.e. a 6 x 6 x 6 tensor grid; volume 1 (1e-12); closed; no
  non-positive cell; 0 unmatched boundary faces; each patch has 36 faces.
- **P2 (B3).** Three groups (`walls`, `inlet`, `outlet`; the walls' own
  edges are inside one patch). 64 + 96 face cells + 32 edge cells (4 x 4
  rim edges at each end), no corner cell (two groups at the box corners):
  **192 cells**; volume 1; closed; no non-positive cell. The O-topology
  route (`create_o_topology_layers`) gives 160 cells, bit for bit the
  one-group `boundary_layers_add_layer` on all three patches.
- **P3 (differential, P).** Three groups. For every group key and point,
  the patch-wise displacement equals the one-group route's for that group
  alone (`boundary_layers_add_layer(&[group])`) **bit for bit** (the same
  `create_new_vertex` with the same treated set). Cells: bare + face cells
  (walls + inlet + outlet) + one edge cell per rim edge, no corner cell; 0
  unmatched; closed. Volume conserved to 1e-5 relative (the end patches
  are not planar, as in the one-group record). Inverted cells: within
  ±20 % of the one-group's 56 (the wall displacement field is the same).
- **P4 (B6, `addLayerForPatch` in sequence).** `x0` then `y0`: 80 cells,
  then 100 (y0's 16 faces plus the 4 side quads x0's layer put in `y0`);
  volume 1; closed.
- **P5 (wrapper layer, B6).** 64 + 96 cells, the 96 new ones of zero volume
  (|V| < 1e-15), total volume 1, closed.
- **P6 (`checkTopologyOfBoundaryFaces`).** B6 with `x0`, `y0` forced into
  one group: no quad can be cut into two parts of more than two points, so
  nothing changes (mesh bit for bit). P with `walls` and `inlet` forced
  into one group: the dual's polygonal rim faces are cut, so the port stops
  with `CriticalNotTested` as upstream does.
- **P7 (`cartesian_mesh`, re-measuring gh:#1274).** The sphere has one
  patch: patch-wise = one-group, the final mesh **bit for bit** the
  gh:#1274 one. Pipe 60 mm at 2 mm: more cells (inlet/outlet layers, rim
  edge cells), closed, 0 non-positive cells at the end, volume error within
  0.05 % of the gh:#1274 −0.659 %. The rim layer is no longer pinched
  diagonally: separate wall and inlet columns meet at an edge cell
  (image).
- **P8 (`cartesian_mesh` with `nLayers` 3).** The rim edge cells are layer
  cells for two base faces, so `refine_boundary_layers` returns `NotPorted`
  (before gh:#1271's remainder lands; re-measured after).
- **P9 (`cartesian_2d_mesh`, the channel).** The empty patches are treated
  by `activate2DMode`; groups `walls`, `inlet`, `outlet`; 4 edge cells at
  the channel corners, each with its end faces in the empty patches. Still
  one cell thick, area L x H to 1e-12, closed.
- **P10 (`tet_mesh`, `p_mesh` with a dict).** With `nLayers` 1 on the
  sphere (one patch), one layer cell per boundary face; 0 non-positive cells
  after `meshOptimisation`; closed.

## Results (2026-11-05, `cargo test --release`)

Tests: `layers::boundary_layers::tests::{patch_wise_layer_on_the_six_patch_box,
patch_wise_against_o_topology_on_the_named_box,
patch_wise_displacements_equal_the_one_group_route_on_the_pipe,
add_layer_for_patch_in_sequence_on_the_box, wrapper_layer_has_zero_thickness,
check_topology_of_boundary_faces_on_forced_groups, edge_convexity_by_hand}`,
`pipeline::cartesian_mesh::tests::{cartesian_mesh_patch_wise_against_o_topology,
diagnose_cartesian_mesh_on_the_sphere, diagnose_cartesian_mesh_on_the_pipes,
cartesian_mesh_with_refined_layers}`,
`pipeline::cartesian_2d_mesh::tests::channel_2d_against_the_3d_mid_plane`,
`pipeline::tet_mesh::tests::tet_and_p_mesh_with_a_boundary_layers_dict`,
`mesh_utilities::tests::generate_boundary_layers_on_a_box`.

- **P1 held.** Six groups; 216 cells (96 face, 48 edge, 8 corner), 0
  unmatched; volume 1.000000000000000; closed; 0 non-positive; every
  displacement 0.125 and every coordinate on the 7-value grid (1e-12); 36
  faces per patch (`box6_patch_wise_yz.png`).
- **P2 held.** Groups `walls`, `outlet`, `inlet`; 192 cells (96 face, 32
  edge, 0 corner); volume 0.999999999999999; closed; 0 non-positive. The
  O-topology route gives 160 cells, equal to the one-group
  `boundary_layers_add_layer` on all three patches in points, faces and
  owners (`box3_patch_wise_xz.png`, `box3_o_topology_xz.png`).
- **P3 missed on the grouping, held on the differential.** Upstream's rule
  puts all three pipe patches in **one** group on the tet-dual pipe: of its
  72 rim edges, 24 are not convex by the tet test and have
  `angleBetweenFaces` above 135° (min/median/max 87.9/91.3/226.1°); 12
  per end survive the inverted-vertex filter, and one concave edge is
  enough to merge a pair. The dual's rim polygons fold, so upstream would
  build an O-topology layer here. With one group: 2 261 cells (605 + 1 656
  face cells, 0 edge cells), 0 unmatched, volume change 0 relative, **66**
  non-positive cells (the one-group layer on `walls` alone: 56; outside the
  ±20 % I predicted), max non-orthogonality 143.6°, skewness 51.3. Every
  displacement equals the one-group route's for the same group, **1 658 of
  1 658 bit for bit** (held). The three-group differential is on the
  cartesian pipe (P7).
- **P4 held.** 80 then 100 cells, volume 1, closed; a treated patch adds
  nothing; an unknown name is an error.
- **P5 held.** 96 layer cells, 160 cells, the 96 of zero volume, total 1,
  closed; a second call adds nothing (patch 0 treated).
- **P6 held.** The box: `Ok(0)`, mesh unchanged. The pipe with `walls` and
  `inlet` forced together: `CriticalNotTested { n_decomposed: 12 }`.
- **P7 held in substance; missed "bit for bit" on the sphere.** The sphere
  (one group) has the same 1 128 cells, −1.938 %, 15.60°, 0.470 as the
  O-topology ablation; the cell volumes agree to 2.9e-15 relative, but
  neither the labels nor the point bits are equal: the patch-wise route
  numbers new points as upstream (appended) and computes `p − dist n`, the
  one-group route renumbers as `build_layered_variable` does and builds
  `x + |d| d/|d|` (round-off). The 60 mm pipe at 2 mm, three groups
  (`inlet`, `outlet`, `walls`):

  | route | cells | layer cells (face / edge) | volume error | non-positive after the layer | final non-positive | max non-orth | max skew |
  |---|---:|---:|---:|---:|---:|---:|---:|
  | patch-wise (upstream) | 2 688 | 1 024 / 64 | −0.659 % | 32 | 0 | 30.18° | 0.481 |
  | O-topology (ablation; the gh:#1274 route) | 2 624 | 1 024 / 0 | −0.659 % | 32 | 0 | 35.69° | 0.487 |

  The rim is no longer pinched: `pipe_patchwise_layer_stage_xz_rim.png`
  shows the wall column, the inlet layer and the edge cell in the corner;
  `pipe_otopology_layer_stage_xz_rim.png` the diagonal pinch. **New:** the
  layer now changes the volume (with − without): 2.7e-6 (60 mm pipe),
  2.5e-6 (Foam Studio 5 mm), 1.2e-6 (1.25 mm), where the one-group layer
  changed it by exactly 0: the rim's new boundary points `p + d_inlet` lie
  on the wall facet's averaged plane, not on the facet.
- **P8 held.** With `nLayers` 3 the pipe's refinement is refused: "cell
  2624 is a layer cell for two base faces (7420, 8444)" (an edge cell), so
  the layer stays one cell thick. The sphere refines (528 split, 2 184
  cells).
- **P9 held.** The channel: groups `walls`, `outlet`, `inlet`; 48 layer
  cells (44 face, 4 edge); still one cell thick (120 of 120 cells with one
  bottom and one top face); area 0.25 to 2.2e-16; closed; 0 non-positive;
  the empty patches 72 → 120 faces each (the edge cells' end faces).
- **P10 held.** Sphere R 3 at 0.75: `tetMesh` 3 960 → 4 776 cells (816
  layer cells, one per boundary face; 471 non-positive right after the
  layer, 0 at the end); `pMesh` 489 → 767 (278; 0 and 0). `nLayers` 0
  gives the dict-less mesh bit for bit.
- **`generateBoundaryLayers` (other track's utility, re-measured):** the
  4 x 4 x 4 box now gives 216 cells (was 160 by the one-group stand-in);
  0 bad faces, volume 1 (its doc comment corrected).

### Re-measured `cartesian_mesh` (gh:#1274 record)

| case | cells | volume error | max non-orth | max skew | non-positive after the layer |
|---|---:|---:|---:|---:|---:|
| sphere 0.5 m | 1 128 | −1.938 % | 15.60° | 0.470 | 0 |
| pipe 60 mm, 2 mm | 2 688 | −0.659 % | 30.18° | 0.481 | 32 |
| Foam Studio pipe, 5 mm | 536 | −9.959 % | 12.01° | 0.489 | 16 |
| Foam Studio pipe, 5 / 1.25 mm | 22 320 | −0.156 % | 35.28° | 0.495 | 64 |

Every mesh is closed with 0 non-positive cells at the end.

## Images

Regenerate: `cargo test --release -p outram-park-fork-cfmesh --lib --
draw_patch_wise_layer_images --ignored`. Boundary faces cut by the plane are
coloured by patch (inlet red, walls blue, outlet green); faces of
non-positive cells orange.

| file | what |
|---|---|
| `box6_patch_wise_yz.png` | six-patch box, y-z at x 0.375: the 6 x 6 tensor grid |
| `box3_patch_wise_xz.png`, `box3_o_topology_xz.png` | walls/inlet/outlet box, x-z at y 0.375: patch-wise against O-topology |
| `pipe_{patchwise,otopology}_{layer_stage,final}_xz_rim.png` | cartesian pipe 60 mm at 2 mm, inlet rim, x-z at y 0.1 mm, right after `boundaryLayerGeneration` and at the end |
| `pipe_{patchwise,otopology}_{layer_stage,final}_xy.png` | the same, x-y at z 31.7 mm |

**What I checked:** the six-patch box is the predicted tensor grid; at the
pipe rim the patch-wise layer has a wall column, an inlet layer and a
square edge cell in the corner, where the O-topology layer's one rim cell
is pinched diagonally; the x-y section keeps a hex core with one layer
ring. **What I could not check:** the 32 non-positive cells right after the
layer lie in neither drawn plane (no orange), so I have not seen them; no
3D view.

## Upstream defects and quirks found

- `addWrapperLayer` writes the new labels to a local `newLabelForVertex`
  that shadows the member `newLabelForVertex_` read by
  `createNewFacesAndCells` (defect; the port follows the intent).
- `checkTopologyOfBoundaryFaces` and `addWrapperLayer` have no caller;
  the former stops with "Critical. Not tested" whenever it cuts a face
  into two real parts (ported as an error).
- `voronoiMeshGenerator::generateBoudaryLayers` constructs a second,
  unused `boundaryLayers` (harmless).
- Patch-wise `addLayerForAllPatches` does not mark the patches treated
  (ported as written).
- `terminateLayersAtConcaveEdges_` is set but never read (not ported).
- On tet-dual meshes the rim polygons fold, so upstream's concavity rule
  merges the pipe's patches into one group (P3).

# gh:#1271 remainder: `refineEdgeHexCell`, `refineCornerHexCell` and the two-direction face split

**Upstream:** `refineBoundaryLayersFaces.C` (`refineFace`, `generateNewFaces`),
`refineBoundaryLayersCells.C` (`generateNewCells`, `refineEdgeHexCell`,
`refineCornerHexCell::generateNewPoints`).

**What upstream does (read before predicting):** every quad face is split
into a grid: an internal face in the directions of its boundary edges (the
most layers of the boundary faces there), a boundary face in the direction
of an edge it shares with a face of another layer (that face's count). A
face split in two directions gets interior points by transfinite
interpolation, `u` and `v` the distance fractions along the two hairs at
the corner where both start. A cell is split by the number of its boundary
faces with more than one layer: 1 a prism split, 2 `refineEdgeHexCell` (an
`nI x nJ` grid, no new points), 3 `refineCornerHexCell` (`nI x nJ x nK`,
interior points by trilinear interpolation with `u, v, w` from the three
hairs at the cell's boundary corner). A face whose count differs from its
hairs' gets the extra hair points appended to its last piece.

## Predictions (written 2026-11-05, before the general split was written)

- **R1 (six-patch box, patch-wise layer, `nLayers` 3, ratio 1).** Face cells
  split in 3, edge cells in 9, corner cells in 27: 64 + 288 + 432 + 216 =
  **1 000 cells**, a 10 x 10 x 10 tensor grid (coordinates in {0, 1/24,
  2/24, 1/8, 1/4, 1/2, …} to 1e-12); volume 1 (1e-12); closed; 0
  non-positive.
- **R2 (walls/inlet/outlet box, patch-wise, `nLayers` 3).** 64 + 3 x 96 +
  9 x 32 = **640 cells**; volume 1; closed; 0 non-positive.
- **R3 (`cartesian_mesh`, 60 mm pipe at 2 mm, `nLayers` 3, ratio 1.2).** No
  `NotPorted`: 1 600 + 3 x 1 024 + 9 x 64 = **5 248 cells**, closed, 0
  non-positive at the end, volume error within 1e-4 (relative to the
  analytic value) of the unrefined −0.659 %.
- **R4 (pin).** The one-group prism refinement on the tet-dual pipe and the
  box keeps its output bit for bit (fingerprints recorded before the
  change: `prism_refinement_is_pinned_bit_for_bit`); the
  `build_layered_variable` differential still holds.
- **R5 (differential, R1's box).** Every hair's split points equal those of
  `build_layered_variable_with_columns` on a column of the same start,
  direction and length with `d = 0, 1, 2, 3` to 1e-15; the face interior
  points equal the tensor-grid points to 1e-15 (bilinear on a rectangle is
  exact).

## Results (2026-11-05, `cargo test --release`)

Tests: `layers::refine_boundary_layers::tests::{edge_and_corner_hexes_refine_the_six_patch_box,
edge_hexes_refine_the_named_box, prism_refinement_is_pinned_bit_for_bit,
split_points_match_build_layered_variable}` and
`pipeline::cartesian_mesh::tests::cartesian_mesh_with_refined_layers`.

- **R1 held.** 1 000 cells (152 split: 96 prisms, 48 edge hexes, 8 corner
  hexes); volume 1.000000000000002; closed; 0 non-positive; 100 faces per
  patch; every coordinate on the 10-value grid to 3.3e-16
  (`box6_refined_3_yz.png`).
- **R2 held.** 640 cells (32 edge hexes, no corner); volume 1; closed; 0
  non-positive.
- **R3 held.** The 60 mm pipe with `nLayers` 3, ratio 1.2: no refusal
  (1 088 cells split, 64 of them rim edge hexes), 5 248 cells, closed, 0
  non-positive, volume error −0.6590 % (unrefined −0.659 %); max skewness
  1.03 (0.48 unrefined). `pipe_refined_3_xz_rim.png`: three wall layers and
  three inlet layers meet in a 3 x 3 block at the rim.
- **R4 held.** Both prism fingerprints unchanged
  (`0x677a875f14ca046b`, `0x687256342d92ed50`); the
  `build_layered_variable` differential unchanged (4.6e-15 of the column,
  5 621 of 5 824 bit for bit).
- **R5 held, with a weaker instrument than predicted.** The test compares
  each hair's split points with the column `x + d (L j / 3)` written out in
  the test (the formula `layers::layer_columns` evaluates), not with a call
  to `build_layered_variable_with_columns`: equal bit for bit on every hair.
  Every interior face point lies on the tensor grid to 3.3e-16.

**Not ported (refused as `NotPorted`, none hit by these cases):** a face
whose split count differs from its hairs' point count (upstream appends
the extra points to the last piece), two- or three-base cells that are not
hexahedra, `activateSpecialMode`, the cell subset. The 2D mode is item 3.
**Port difference kept from gh:#1271:** non-quad faces holding a hair get
its points inserted; upstream leaves them unsplit.
