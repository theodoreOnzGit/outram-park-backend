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

## Results

(to be filled from the runs)
