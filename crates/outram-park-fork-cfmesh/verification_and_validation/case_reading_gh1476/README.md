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

### Reading pin (2026-11-11)

All eight tutorials pass `checkMeshDict` with no warning and read with no
unknown keyword. Read but not acted on (reported): `boundaryLayers/optimiseLayer`
in `cartesianMesh/multipleOrifices` and `cartesianMesh/ship5415Octree`, and
the ship's `optimisationParameters` (since commit `c90ea637e`, `optimiseLayer`
runs `optimizeBoundaryLayer`, but cartesianMesh's buffer layer is not
ported, so the row stays reported). The ship's
`optimisationParameters/reCalculateNormals` is recorded as not applicable:
upstream's `boundaryLayerOptimisation::readSettings` reads
`recalculateNormals`, so upstream ignores the tutorial's spelling too.

### End-to-end runs (2026-11-11, `--release`, 4-core machine shared with an OpenFOAM build)

Binaries at commit `39855ad7b` (`cutCube`, `hat`, `asmo`, `singleOrifice`,
`pMesh/multipleOrifices`) and `c90ea637e` (`elbow`,
`cartesianMesh/multipleOrifices`; the change between them only adds
`optimiseLayer`, which none of the first five uses). Time is the meshing
wall time (`ExecutionTime`) or `/usr/bin/time` wall time where marked.

| case | binary | cells | points | faces | time | prediction |
|---|---|---|---|---|---|---|
| `tetMesh/cutCubeOctree` | `tetMesh` | 205 182 | 40 498 | 420 782 | 19.0 s | **missed**: predicted 1 000–50 000 cells and under 10 s. The prediction counted octree leaves, not the ~10 tets each leaf yields, and did not allow for `minCellSize 0.1` switching on the automatic refinement |
| `cartesian2DMesh/hatOctree` | `cartesian2DMesh` | 18 676 | 38 090 | 75 072 | 1.7 s | met (10 000–40 000) |
| `cartesianMesh/asmoOctree` | `cartesianMesh` | 52 997 | 71 649 | 177 387 | 32.2 s | cells met (5e4–1e6); time faster than "minutes"; **patch types missed** (see below) |
| `cartesianMesh/singleOrifice` | `cartesianMesh` | 72 302 | 82 964 | 227 438 | 15.4 s | met: completes, patches `orificeRegion` (wall), `inlet`, `outlet` (patch), `fixedWalls` (wall); the layer refinement split 11 385 cells |
| `pMesh/multipleOrifices` | `pMesh` | 481 810 | 1 884 693 | 2 596 922 | 2 330 s wall, 2.4 GB | met: completes, patches `inlet`, `outlet`, `fixedWalls`; 807 low-quality faces left after `untangleBoundaryLayer` |
| `cartesianMesh/elbow_90degree` | `cartesianMesh` | 108 383 | 120 664 | 337 309 | 11.0 s wall | no prediction recorded (run added after the table was committed) |
| `cartesianMesh/multipleOrifices` | `cartesianMesh` | 238 337 | 266 730 | 743 131 | 381 s wall | no prediction recorded; `optimizeBoundaryLayer(true)` ran (43 938 hairs, 27 248 thinned, no buffer layer) |

Nothing was refused. `cartesianMesh/ship5415Octree` was not run: its
surfaces are not copied (`References.md`).

### Finding: surface patch types do not reach the output

Upstream copies each surface patch's `geometricType` to the mesh patch
(`edgeExtractor::updateMeshPatches`, `meshSurfaceEdgeExtractor2D`), and
`renameBoundaryPatches` turns `empty` into `wall` (3D). An STL's patches are
`empty` (`triSurface::setDefaultPatches`), the hat FMS's are `wall`, so
upstream writes `wall` for every unrenamed patch of these cases. The port's
generators take patch names only (`SurfaceRegions`) and write `patch`: on
`asmoOctree`, `cutCubeOctree` and `hatOctree` every unrenamed patch's type
differs from upstream. Renamed patches (`renameBoundary` with `type`) are
right. Not fixed here: it needs the surface's types passed into the four
generators, and a `symmetryPlane` surface type would also change what
`checkSymmetryPlanes` does.

### Not checked

No mesh was compared with upstream cfMesh (gh:#1385 does that); no image was
drawn. The reader's correctness is checked against the upstream source it
ports, entry by entry, not against an upstream run.
