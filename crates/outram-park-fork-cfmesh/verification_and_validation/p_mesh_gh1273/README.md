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

~~`surfaceMorpherCells`,~~ ~~`preMapVertices`~~ (wired, gh:#1274 remainder), `meshSurfaceEdgeExtractorFUN`,
~~`meshSurfaceOptimizer` (`optimizeSurface`, `untangleSurface`),~~
`edgeExtractor`, ~~`boundaryLayers`, `refineBoundaryLayers`~~ (with a `boundaryLayers` dict since gh:#1270/#1271 remainders).
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

## Re-measurement with the fundamental sheets (gh:#1281, 2026-11-05)

`meshSurfaceEdgeExtractorFUN` now runs `createFundamentalSheetsJFS`
(`crate::fundamental_sheets`, on the `extrudeLayer` port of gh:#1280)
before its smoothing and mapping, as upstream; `TetMeshOptions::fun_sheets
= FunSheets::Without` keeps the gh:#1269 route for the differential. The
sphere case passes no regions, so FUN does not run on it (unchanged).

**Predictions (written 2026-11-05, before the run).** Pipe 48, 2 mm:

- JFS's topology check fails on the Voronoi template (rim cells have
  faces on two patches), so the wrapper sheet is built: one cell per
  boundary face, then the feature-edge sheet. Cells rise from 4 303 by
  at least the boundary face count (several thousand).
- The mesh stays closed with no non-positive cell after the workflow's
  `meshOptimisation`; the volume error stays within ±0.5 points of
  −0.325 % (the sheets move boundary points, and FUN maps them back).
- The sheets are thin (0.4 × the centre-to-vertex distance of the copied
  faces), so max non-orthogonality and skewness rise above the `Without`
  route's 55.3° / 0.577.
- The rims: with the sheets, each wall cell at a rim has one boundary
  face, so the mapped rim edge is the cell edge between two sheet cells
  (sharper than without). Checked on the image, not measured.

**Results (2026-11-05,** `cargo test --release -p outram-park-fork-cfmesh
--features cell-centre-dual --lib -- diagnose_p_mesh_workflow --nocapture`;
the test runs both routes on the pipe).

| case | route | cells | volume error | non-positive | max non-orth | max skew | time |
|---|---|---:|---:|---:|---:|---:|---:|
| pipe | `FunSheets::Without` (gh:#1269) | 4 303 | −0.325 % | 0 | ~~55.29°~~ 55.30° | ~~0.577~~ 0.574 | 0.37 s |
| pipe | **JFS (default)** | **9 781** | **−0.320 %** | 0 | **~~58.24°~~ 58.23°** | **~~0.690~~ 0.686** | 0.52 s |
| sphere | (no regions: FUN not run) | 1 749 | ~~−1.537 %~~ −1.484 % | 0 | ~~18.06°~~ 18.21° | 0.508 | 0.24 s |

**RE-MEASURED 2026-11-05 (gh:#1274 remainder):** the struck numbers were taken without `preMapVertices`; the others with it (section below).

The `Without` row reproduces the gh:#1269 row exactly, so the route kept
for the differential is the old one.

- **Missed: the topology check passes** on the Voronoi template (no cell
  has two boundary faces), so JFS builds **no wrapper sheet**. My
  prediction assumed rim cells with faces on two patches; that is not a
  question `isTopologyOk` asks (it counts boundary faces per cell, any
  patch), and no cell here has two.
- The feature-edge sheet extrudes 5 478 faces (every internal face of a
  boundary cell whose other cell is not a boundary cell of the same
  patch), 8 648 new points, no edge or corner cell. Cells 4 303 → 9 781
  (+5 478, one per extruded face). The "several thousand" held, through
  the feature-edge sheet, not the wrapper.
- Closed, no non-positive cell: **held**. Volume error within ±0.5 points
  of −0.325 %: **held** (−0.320 %).
- Max non-orthogonality and skewness above the `Without` route: **held**
  (58.24° against 55.29°, 0.690 against 0.577).
- The workflow's optimiser found 0 bad faces on both routes.

**Images** (`examples/tet_mesh_images.rs`, regenerated; the sphere and
`tetMesh` images are bit-identical to the committed ones, `cmp`):
`pipe_xy_p_mesh.png` (now with the sheets), `pipe_xy_p_mesh_without_sheets.png`,
`pipe_xz_rim_p_mesh.png`, `pipe_xz_rim_p_mesh_without_sheets.png` (x-z at
y = 0.1 mm, the inlet rim at x = 10 mm, z = 0).

**Checked by eye (2026-11-05):**

- `pipe_xy_p_mesh.png`: a ring of thin sheet cells runs inside the wall
  cells all round; the outer boundary meets the 48-gon trace all round,
  with no fold.
- `pipe_xz_rim_p_mesh.png` against `_without_sheets`: without the sheets
  one cell spans the rim corner (a face on the inlet and one on the wall
  meet in one cell); with them, a sheet layer runs under the inlet and
  inside the wall, and a cell edge runs into the rim point. Both reach
  the rim at (10 mm, 0).

**Not checked:** the outlet rim; the 3D shape of the sheets; a solve on
this mesh.

## Re-measurement with `preMapVertices` wired in (gh:#1274 remainder, 2026-11-05)

`voronoiMeshGenerator::mapMeshToSurface` calls
`meshSurfaceMapper::preMapVertices()` (3 iterations) before
`mapVerticesOntoSurface` and `untangleSurface`; `p_mesh` skipped it (noted)
since `premap_vertices` (gh:#1274) had not been wired here. It is now.
`tetMesh` does not call it upstream and is unchanged.

**Predictions (written 2026-11-05, before the run).** Sphere 0.5 m and
pipe 48 at 2 mm, the `diagnose_p_mesh_workflow` setups:

- **Q1.** `preMapVertices` finds a patch for every boundary point and leaves
  the mean distance to the surface between 1/8 and 1/2 of its start (as on
  `cartesianMesh`, C3 of gh:#1274).
- **Q2.** Cell counts unchanged (sphere 1 749, pipe 9 781): it moves points
  only.
- **Q3.** Volume errors move by less than 0.2 points (sphere −1.537 %,
  pipe −0.320 %).
- **Q4.** Max non-orthogonality rises by no more than 5° (18.06°, 58.24°).
- **Q5.** Closed, 0 non-positive cells.

**Results (2026-11-05,** `cargo test --release -p outram-park-fork-cfmesh
--features cell-centre-dual --lib -- diagnose_p_mesh_workflow --nocapture`**).**

| case | cells | volume error | non-positive | max non-orth | max skew | mean distance before / after `preMapVertices` |
|---|---:|---:|---:|---:|---:|---|
| sphere 0.5 m | 1 749 | −1.484 % (was −1.537 %) | 0 | 18.21° (18.06°) | 0.508 (0.508) | 0.1734 / 0.02170 m (ratio 0.125) |
| pipe 2 mm, JFS (default) | 9 781 | −0.320 % (−0.320 %) | 0 | 58.23° (58.24°) | 0.686 (0.690) | 0.875 / 0.110 mm (0.125) |
| pipe 2 mm, `FunSheets::Without` | 4 303 | −0.325 % (−0.325 %) | 0 | 55.30° (55.29°) | 0.574 (0.577) | 0.875 / 0.110 mm (0.125) |

- **Q1 held on the distance**, ratio 0.1251 (sphere) and 0.1254 (pipe), just
  inside the 1/8 bound. Whether every point found a patch is **not
  checked** (the report carries no count).
- **Q2, Q3, Q4, Q5 held.** Cells unchanged; the volume error moved by
  0.053 points on the sphere and by less than 0.001 on the pipe;
  non-orthogonality within 0.2°; closed, 0 non-positive.
- The pipe barely moves because its boundary is mapped and untangled after
  the premap anyway; the sphere's error improves by 0.05 points.

Images regenerated (`examples/tet_mesh_images.rs`).

