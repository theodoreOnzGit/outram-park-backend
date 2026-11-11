# polyDualMesh parity (gh:#1460: #1471, #1472, #1473)

Port: `src/poly_dual_mesh.rs` (default, surface-preserving) and
`src/poly_dual_mesh_upstream.rs` (new, explicit upstream mode). CLI:
`crates/outram-foam-cli/src/bin/poly_dual_mesh.rs`. Upstream read:
OpenFOAM-dev `852527377c7e87e33d51b19ca8974b5db60af28b` (sparse clone in the
gitignored `upstream_source/OpenFOAM-dev`) and OpenFOAM.com v2506
(`/home/user/upstream-build/OpenFOAM-v2506`).

## What was read before anything was written (upstream first)

`polyDualMesh.C` (`simpleMarkFeatures`, `main`), `meshDualiser.C` (all of
`setRefinement`, `createFacesAroundEdge`, `createFaceFromInternalFace`,
`createFacesAroundBoundaryPoint`), `edgeFaceCirculator`, `polyTopoChange`
(`compact`, `getFaceOrder`, `makeCells`), `primitiveMeshEdges.C`
(`calcEdges`: edges numbered upper-triangular by point pair),
`PrimitivePatchAddressing.C`, `PrimitivePatchCheck.C`, `HashTable.C`
(`set`, `resize`, `toc`), `Hash<label>` (identity), `faceTemplates.C`,
`primitiveMeshCellCentresAndVols.C`; v2506 `primitiveMeshTools.C`.
Findings that shaped the port, all checked in the source:

1. The utility puts a dual point at **every boundary-face centre**
   (`featureFaceSet` is seeded with all boundary faces, "B. face zones" adds
   more). Only edge midpoints are restricted to feature edges (plus, in
   `setRefinement`, every boundary edge at a feature point). So #1460's
   "deviation: a point at every boundary face centre" is not a deviation in
   faces; it is one in **edge midpoints** only.
2. Dual cell, point and face numbering come from `polyTopoChange` with
   `orderCells = false, orderPoints = false`: additions in order, internal
   faces re-ordered upper-triangular by dual cell, then patches. The feature
   sets come from `labelHashSet::toc()`: bucket order of an identity-hash,
   power-of-two table (so insertion order matters through collisions).
3. OpenFOAM-dev and v2506 use the same algorithm in `meshDualiser` and
   `simpleMarkFeatures`; v2506 additionally carries faceZone ids and fields.
4. The two lines' face-centre routines differ (gh:#1473): dev weights by area
   projected on the face normal; v2506 by `mag(n)`; triangle centres are
   `(1/3)*(p0+p1+p2)` in both; cell centres (`pc = 3/4 fc + 1/4 cEst`) are
   identical.

## Predictions (written before any run of the new code)

Hand-derived from the source, committed before the first test run.

| id | case | prediction |
|---|---|---|
| P1 | `FoamLabelHashSet`, size 4, insert 1, 5, 2, 6 | resizes to 8 on the 4th insert; `toc` = `[1, 2, 5, 6]` |
| P1b | size 2, insert 0, 2 | `toc` = `[0, 2]` after the resize; `[2, 0]` is the pre-resize chain order |
| P2 | 3x3x3 block (27 cells, 64 points, 6 patches), `featureAngle` 60, upstream mode | 64 dual cells; **197 dual points** (27 cell + 54 boundary-face + 32 feature points + 84 boundary-edge points: the 108 boundary edges minus the 24 whose two ends are both face-interior points); internal dual faces = 144 (one per primal edge); sum of dual volumes 27 (planar faces) to 1e-9; closure residual under 1e-12; Euler characteristic 2 in every cell; internal faces upper-triangular |
| P3 | same block, `splitAllFaces` | every edge and every face gets a dual point: 27 + 108 + 32 + 144 = **311 points**; internal faces = 324 (36 interior edges x 4 cells + 72 boundary non-seam edges x 2 + 36 seam edges x 1), several between the same two dual cells; volume 27 |
| P4 | L-shape (2x2x1 block minus one cell, one patch), angle 60 | without `concaveMultiCells`: 16 dual cells; with it: **20** (the two inner-corner points each become 3 cells); volume 3 in both; closure under 1e-12 |
| P5 | two cells in a row (2x1x1) with the middle face as a face zone | preserved: 12 + 4 = **16** dual cells (the 4 zone points become 2 each); `-doNotPreserveFaceZones`: 12; volume 2 |
| P6 | default `poly_dual_mesh_with` on any of the above | **byte-identical output before and after this change** (pins below) |
| P7 | v2506 `polyDualMesh 60` on blockMesh cavity (20x20x1), port in upstream mode with `Esi2506` centres | points / faces / owner / neighbour / boundary identical to the bit; if not, the first difference is recorded below |

Note on P2: an earlier draft of this table said 233 (it double-counted the seam edges); 197 is the figure committed before any run.

## Results

(filled in below after the runs)
