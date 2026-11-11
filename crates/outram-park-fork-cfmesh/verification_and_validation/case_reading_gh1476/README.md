# Reading upstream cfMesh cases (gh:#1476)

**Status: AI draft, not human reviewed.** Research, education and V&V only.

## What this checks

That the port's executables (`outram-foam-cli`: `cartesianMesh`, `tetMesh`,
`pMesh`, `cartesian2DMesh`) read an **unmodified upstream cfMesh case**
(DAFoam/cfmesh `e944f6ba695f`, inputs in
`crates/outram-park-fork-cfmesh/test_data/cfmesh_tutorials/`, provenance in
its `References.md`), so the port and upstream can be run on the same inputs
(gh:#1385). It does **not** compare meshes with upstream: no upstream cfMesh
run is part of this record.

## Methodology

1. **Reading pin** (`mesh_dict::tests::tutorials_read_with_no_unhandled_keyword`):
   each of the eight tutorials' `meshDict` passes the `checkMeshDict` port
   with no warning and is read with no unknown keyword; the entries read but
   not acted on are listed exactly; each copied surface is read and every
   patch pattern matches a patch.
2. **End-to-end runs**: copy the case to a scratch directory, run the
   binary (`--release`, one thread), record cells, faces, points, patches,
   wall time of the meshing and anything refused.

## Predictions (written 2026-11-11, before any binary was run)

| case | binary | prediction |
|---|---|---|
| `tetMesh/cutCubeOctree` | `tetMesh` | completes in under 10 s; between 1 000 and 50 000 cells (2 m cube, `maxCellSize` 0.2, two patches at 0.05) |
| `cartesian2DMesh/hatOctree` | `cartesian2DMesh` | completes; 1.5 m² at 0.01 m is ~15 000 background cells, so between 10 000 and 40 000 cells with the 6-layer refinement; one cell thick in z |
| `cartesianMesh/asmoOctree` | `cartesianMesh` | completes, in minutes rather than seconds (local cells down to 0.003 125 m on a 5.3 m box); between 5e4 and 1e6 cells; the surface's ten solids `INLE10000`, `OUTL20001`, `SYMP30002`, `defaultFaces0003`–`0009` as patches, all of type `wall` (STL patches are type `empty`, which `renameBoundaryPatches` turns into `wall`; no `renameBoundary`) |
| `cartesianMesh/singleOrifice` | `cartesianMesh` | completes or is refused by a layer step the port reports (`refine_error`); patches `fixedWalls`, `orificeRegion`, `inlet`, `outlet` after `renameBoundary` |
| `pMesh/multipleOrifices` | `pMesh` | completes; patches `fixedWalls`, `inlet`, `outlet` after `renameBoundary` |

A run that exceeds 60 minutes is stopped and recorded as such, not as a
failure of the reader.

## Results

To be filled in from the runs (below this line, after the predictions were
committed).
