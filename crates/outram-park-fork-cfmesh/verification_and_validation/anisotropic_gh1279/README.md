# cfMesh's anisotropic workflow in the four generators: V&V record (gh:#1279 remainder)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-written port, not yet reviewed by
a human.

**Upstream:** `github.com/DAFoam/cfmesh` at `e944f6ba695f` (cfMesh v1.1
line): `cartesianMeshGenerator.C`, `tetMeshGenerator.C`,
`cartesian2DMeshGenerator.C`, `voronoiMeshGenerator.C` (the `pMesh`
executable), and `utilities/anisotropicMeshing/`.

**Port:** `pipeline::{cartesian_mesh, tet_mesh, p_mesh, cartesian_2d_mesh}`
(the wiring), `crate::anisotropic` (the modifier, ported earlier in
gh:#1279).

## What upstream does (read before predicting)

All four generators' **constructors** do the same thing: with
`anisotropicSources` in `meshDict`,
`surfaceMeshGeometryModification(surf, meshDict).modifyGeometry()` makes
the modified surface (`modSurfacePtr_`), and the octree is built on **it**
(`meshOctree(*modSurfacePtr_)`, `createOctreeBoxes`). Every step up to the
revert works in the modified space, against that octree.

Where the mesh goes back to real space differs:

| generator | revert (`polyMeshGenGeometryModification::revertGeometryModification`) | after the revert |
|---|---|---|
| `cartesianMesh` | end of `optimiseFinalMesh` (step `meshOptimisation`), after `optimizeMeshFV`, `optimizeLowQualityFaces`, `optimizeBoundaryLayer(modSurfacePtr_ == nullptr)` (so `false` on this pass), `untangleMeshFV` | `projectSurfaceAfterBackScaling`: new octree on the **original** surface, `createOctreeWithRefinedBoundary(20, 30)`, `meshSurfaceMapper::mapVerticesOntoSurface` (nearest point, every boundary point), then `optimiseFinalMesh` **again** (now with `modSurfacePtr_` null: no second revert, `optimizeBoundaryLayer(true)`) |
| `tetMesh` | end of `optimiseFinalMesh` (same place) | `projectSurfaceAfterBackScaling`, as `cartesianMesh` |
| `pMesh` (`voronoiMeshGenerator`) | end of `optimiseFinalMesh` (same place) | `projectSurfaceAfterBackScaling`, as `cartesianMesh` |
| `cartesian2DMesh` | inside `generateBoundaryLayers`, **after** `addLayerForAllPatches` | new quadtree on the original surface (`meshOctree(surf, true)`, `createOctreeWithRefinedBoundary(20)`, i.e. 15 triangles per leaf), `mapEdgesAndCorners` (`remapBoundaryPoints`), `optimiseMeshSurface`; the later steps use the new quadtree. **No `projectSurfaceAfterBackScaling`** in 2D. |

Conditions: the revert is guarded by `modSurfacePtr_` (set only by the
constructor), `projectSurfaceAfterBackScaling` by
`meshDict_.found("anisotropicSources")`. Both are outside the
`runCurrentStep` checks only in the sense that they sit inside the step that
calls them: a run stopped before `meshOptimisation` (3D) or
`boundaryLayerGeneration` (2D) returns a mesh **still in the modified
space**, and a 2D restart that resumes after `boundaryLayerGeneration`
keeps the constructor's modified quadtree for its later steps. The port
keeps both, as upstream.

`projectSurfaceAfterBackScaling` is a **generator** method, not a
`meshOptimizer` / `meshSurfaceOptimizer` one (grep of the whole tree: it
exists only in the three 3D generators). `meshSurfaceMapper::
mapVerticesOntoSurface` (`meshSurfaceMapperMapVertices.C`) is
`findNearestSurfacePoint` for every boundary point, no patch, no limit: the
crate's `snap::snap_to_surface_with(.., SurfaceSearch::Octree)` (gh:#1263,
bit-identical to the brute-force nearest point) is that.

The search octrees: the port's surface searches (`SurfaceQueries`, the
surface optimiser, `snap`) take the surface's points and triangles, not
the octree, so the octree that upstream rebuilds in
`projectSurfaceAfterBackScaling` has no counterpart in 3D; the points are
what changes. In 2D the quadtree **is** what the 2D steps read, so it is
rebuilt as upstream.

## Predictions (written 2026-11-05, before the wiring was run)

- **W0.** Without `anisotropicSources` every generator's output is
  bit-identical to the code before the wiring
  (`anisotropic::tests::generators_without_anisotropic_sources_are_pinned_bit_for_bit`,
  fingerprints committed before the wiring), and the named pins and the
  three restart tests hold.

Setup S: sphere R 3 m, 24 x 48 UV (analytic 113.0973 m³, faceted surface
−0.712 %), `maxCellSize` 0.5, and one `boxScaling` "slab": centre (0, 0,
0), lengths (8, 8, 2), scales (1, 1, 0.5). The forward map stretches
`|z| <= 1` by 2 (and shifts everything above it by +2), so the octree's
cubes become 0.5 x 0.5 x 0.25 boxes in the slab once reverted.

- **W1 (`cartesian_mesh`).** Every hexahedron touching no boundary point
  and lying wholly in the slab has `dz / dx = 0.5` to 1e-9; every one
  wholly outside it (`|z| >= 1`) has `dz / dx = 1` to 1e-9. Straddling
  cells lie between. The slab holds 54.45 m³ of the sphere's 113.1 m³, so
  the modified sphere is 1.48 times as large: about 1.48 x 1 128 ≈ 1 670
  cells (±20 %).
- **W2.** Closed (`validate`), 0 non-positive cells.
- **W3.** Volume error against 113.0973 m³ within 1 percentage point of the
  isotropic control run (−1.938 % recorded in gh:#1274).
- **W4.** After the run every boundary point is within 1 % of the cell size
  (5 mm) of the original surface: the projection puts them on it and the
  second `optimiseFinalMesh` keeps them there unless it finds bad faces.
- **W5 (`tet_mesh`).** Same setup. Cells touching no boundary point: the
  ratio of the mean z-extent to the mean x-extent of the cells wholly in
  the slab is 0.5 ± 5 %; of those wholly outside it, within 5 % of the same
  ratio on the isotropic control run. Closed, 0 non-positive. Volume error
  within 1 percentage point of the isotropic control.
- **W6 (`p_mesh`).** Same setup: closed, 0 non-positive; the z/x extent
  ratio in the slab 0.5 ± 10 % (the Voronoi cells are less regular).
- **W7 (`cartesian_2d_mesh`).** The channel ribbon `[0, 1] x [0, 0.25]`
  at 0.05 with a `boxScaling` centre (0.25, 0.125, 0), lengths (0.5, 1,
  1), scales (0.5, 1, 1): `x <= 0.5` is stretched by 2. Interior cells
  wholly in `x <= 0.5` have `dx / dy = 0.5` to 1e-9, the others `1`.
  Closed, 0 non-positive, one cell thick, area 0.25 to 1e-9 (the walls are
  straight, so mapping onto them is exact).
- **W8.** An anisotropic `cartesian_mesh` run stopped after
  `edgeExtraction` (mesh still in the modified space) and restarted gives
  the uninterrupted run's mesh bit for bit.

## Results

Not yet run.
