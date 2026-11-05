# cfMesh `pMesh` (Voronoi) as upstream: V&V record (gh:#1273)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-written port, not yet reviewed by
a human.

**Upstream:** `github.com/DAFoam/cfmesh` at `e944f6ba695f` (cfMesh v1.1 line).

## Predictions (written 2026-11-05, before any run)

On the **same** `tetCreatorOctree` tets (gh:#1272), extracted to a tet
`VolumeMesh` (sphere R 3 at 0.5 m; pipe 48 at 2 mm with 1 mm boundary
cells):

1. `voronoiMeshExtractor` makes exactly one cell per tet vertex with no
   boundary edge, i.e. per interior vertex of the tet mesh; the mesh is
   closed and every cell has positive volume.
2. Against `DualKind::CellCentre` (`polyDualMesh`, feature
   `cell-centre-dual`): the same interior cells. For every interior vertex
   the two cells have the same number of faces and the same volume to 1e-12
   relative (same corners: tet centroids; OpenFOAM's tet cell centre equals
   the vertex average up to round-off).
3. Against `MedianPerCorner`: the same set of interior vertices has cells,
   but different volumes (edge midpoints and face centroids as corners).
4. The Voronoi total volume is smaller than the tet mesh's (no cells at the
   boundary vertices): the missing band is about one tet layer.
5. The full `pMesh` workflow keeps the mesh closed with no negative cell;
   the volume error is larger than `tetMesh`'s (the boundary is further
   from the surface before mapping, and `preMapVertices` is not ported).

## Results (2026-11-05, `cargo test --release`)

Tests: `pipeline::p_mesh::tests::{voronoi_extractor_cells_are_the_interior_tet_vertices,
voronoi_cells_equal_the_cell_centre_dual_inside, diagnose_p_mesh_workflow}`.
The second needs `--features cell-centre-dual`.

### The extractor on the same tets

| case | tets | tet-mesh points | interior points | Voronoi cells | boundary edges | volume, Voronoi / tets | non-positive |
|---|---|---|---|---|---|---|---|
| sphere 0.5 m | 13 968 | 3 215 | 1 749 | 1 749 | 4 248 | 109.31 / 145.50 m^3 | 0 |
| pipe 2 mm / 1 mm | 195 136 | 41 501 | 26 587 | 26 587 | 45 456 | 1.8185e-5 / 2.1213e-5 m^3 | 0 |

- Prediction 1 held: one cell per interior tet vertex, closed, all
  positive.
- **Prediction 2 held where it could be tested.**
  - `polyDualMesh` refuses the sphere and pipe templates. They have 96 and
    2 240 non-manifold boundary points (see the `tetMesh` record: tets in
    coarse BOUNDARY leaves).
  - On a box (`[0.05, 0.95]^3`, `maxCellSize` 0.1), which is manifold,
    every Voronoi cell equals the `CellCentre` cell of its vertex: the same
    face count on all 1 241, and the volume to **9.8e-15** relative.
- **Prediction 3 missed on the sphere.** On the uniform BCC interior the
  median-dual cells have the same volumes as the Voronoi cells (3.8e-15),
  though their corners differ. On the graded pipe they differ by up to
  41 %, as predicted.
- Prediction 4 held: the Voronoi volume is smaller (one tet layer missing).

### The workflow

| case | cells | volume error | non-positive | max non-orth | max skew | time | projection reverts |
|---|---|---|---|---|---|---|---|
| sphere 0.5 m | 1 749 | **−0.97 %** | 0 | 28.0° | 0.536 | 0.34 s | 0 |
| pipe 2 mm | 4 303 | **−0.05 %** | 0 | 24.5° | 0.746 | 1.63 s | 900 |

- **Prediction 5 missed, the other way.** `pMesh` is far closer to the
  surface than `tetMesh` (−0.97 % against +26.7 % on the sphere). It is
  also closer than the crate's uniform 0.5 m baseline (−1.72 %).
  - Its boundary starts one tet layer inside the tet template's boundary.
    That puts it close to the surface, partly inside and partly outside
    (`sphere_xz_voronoi_template.png`; template volume 109.31 m^3,
    −3.3 %). Mapping it onto the surface tangles almost nothing (0 reverts
    on the sphere).
  - `tetMesh`'s boundary starts outside, on the DATA leaves' outer faces.
  - The optimiser repaired 16 bad faces (sphere) and 112 (pipe).
- `pMesh` never calls `workflowCompleted`, so the meta data stays on the
  mesh. This is asserted, and is upstream as written.

### Re-measurement after the topology and surface tracks landed

The table above was measured with the gaps (no `surfaceMorpherCells`, and
tangled moves reverted instead of untangled). Since gh:#1267 and gh:#1259
merged, `pMesh` calls `surfaceMorpherCells`, `untangleSurface` and
`optimizeSurface` in upstream's places. **I wrote no prediction for this
re-run** (the `tetMesh` predictions R1–R4 were written; I did not extend
them to `pMesh`), so these numbers are a measurement, not a test of a
prediction.

| case | cells | volume error | non-positive | max non-orth | max skew | time |
|---|---|---|---|---|---|---|
| sphere 0.5 m | 1 749 | **−1.54 %** | 0 | 18.1° | 0.508 | 0.14 s |
| pipe 2 mm | 4 303 | **−0.35 %** | 0 | **75.9°** | 0.861 | 2.05 s |

- `surfaceMorpherCells` removed no cell on either case (the Voronoi
  template already has one boundary face per cell's side), and
  `untangleSurface` found no inverted point.
- The volume errors moved from −0.97 % / −0.05 % to −1.54 % / −0.35 %.
  The only steps that changed are the surface optimiser's, so the movement
  is theirs; which moves cause it was not traced. Recorded, not tuned.
- **The pipe's maximum non-orthogonality rose from 24.5° to 75.9°**, and
  `optimizeLowQualityFaces` left its 16 low-quality faces at 16. Where the
  bad faces are has not been located; it needs a look before this mesh is
  used for a solve.
- The images were regenerated after this re-run and show the same picture
  as described below.

## Images

Written by `examples/tet_mesh_images.rs` (command in `../tet_mesh_gh1272/`).

| file | what |
|---|---|
| `sphere_xz_voronoi_template.png` | sphere, the Voronoi template, x-z at y = 0.01 |
| `sphere_xz_p_mesh.png` | the `p_mesh` result |
| `pipe_xy_p_mesh.png` | pipe at 2 mm, mid-length x-y |

**What I checked:**

- Inside, the cells show the truncated-octahedron pattern of the BCC dual
  (squares and octagons in the slice).
- Before mapping, the outer cells straddle the surface trace; after mapping
  they reach it all round, with no fold.

**Not checked:** 3D views, and the boundary patches.

## Not ported (see the `pipeline::p_mesh` docs)

~~`surfaceMorpherCells`,~~ `preMapVertices`, `meshSurfaceEdgeExtractorFUN`,
~~`meshSurfaceOptimizer` (`optimizeSurface`, `untangleSurface`),~~
`edgeExtractor`, `boundaryLayers`, `refineBoundaryLayers`.
**CORRECTED 2026-11-05:** `surfaceMorpherCells` (gh:#1267) and
`meshSurfaceOptimizer` (gh:#1259) are called since they landed.
**CORRECTED again 2026-11-05 (gh:#1269):** `edgeExtractor` is called, and
`meshSurfaceEdgeExtractorFUN` is, without its fundamental sheets (see the
next section).

## Re-measurement with `edgeExtractor` and `meshSurfaceEdgeExtractorFUN` (gh:#1269, 2026-11-05)

`patchAssignment` runs the ported `edgeExtractor`; `edgeExtraction` runs
`meshSurfaceEdgeExtractorFUN` in upstream's order (`optimizeSurface`, then
`mapVerticesOntoSurfacePatches`; the stand-in mapped first) without its
fundamental sheets (`createFundamentalSheetsJFS` needs `extrudeLayer`, not
ported).

| case | route | cells | volume error | max non-orth | max skew |
|---|---|---:|---:|---:|---:|
| pipe | before gh:#1269 | 4 303 | −0.35 % | 75.9° | — |
| pipe | nearest triangle + FUN order | 4 303 | −0.324 % | 64.46° | 0.493 |
| pipe | `edgeExtractor` + FUN order (default) | 4 303 | **−0.325 %** | **55.29°** | 0.577 |
| sphere | default | 1 749 | −1.537 % | 18.06° | 0.508 |

The 75.9° maximum, not located in the run above, came down to 64.5° with
the order alone and to 55.3° with `edgeExtractor` (still not located).
Record: `../edge_extractor_gh1269/README.md`. Images regenerated:
`pipe_xy_p_mesh.png` changed (the other two are bit-identical), not
re-inspected beyond the boundary meeting the surface trace all round.
