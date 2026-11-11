# cfMesh patch types reach the output mesh (gh:#1483)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-run; not yet reviewed by a human.

## Upstream's flow (read 2026-11-11, cfMesh `e944f6ba695f`)

- `boundaryPatch` / `replaceBoundary` create a patch of type `patch`
  (`polyMeshGenModifierReplaceBoundary.C:190`), **except** when the patch
  count is unchanged: then names, starts and sizes are updated and every
  type is kept (`:170`).
- The octree generators (`cartesianMesh`, `tetMesh`, `pMesh`) set the types
  in `edgeExtractor::updateMeshPatches` (`edgeExtractor.C:2619`):
  `boundaries[patchI].patchType() = surfPatches[patchI].geometricType()`.
  `cartesian2DMesh` does it in
  `meshSurfaceEdgeExtractor2D::distributeBoundaryFaces`
  (`...DistributeFaces.C:81`), then adds `bottomEmptyFaces` and
  `topEmptyFaces` of type `empty`.
- The surface's `geometricType()` is what the file holds: `.fms` / `.ftr`
  carry it per patch; an STL read through OpenFOAM's `triSurface` has
  `empty` on every patch (the port's `from_stl_zoned` already says so).
- `renameBoundaryPatches` (`renameBoundaryPatches.C`) runs last: a renamed
  entry gets `type` (default `wall`), the default patch gets `defaultType`
  (default `wall`), a patch no entry touches keeps the mesh's type;
  `checkEmptyPatches` turns `empty` into `wall` unless empty patches are
  allowed (`cartesian2DMesh` allows them).

## The defect in the port

`SurfaceRegions` held names only, so `edge_extractor::update_mesh_patches`
returned `patch` for every patch and `edgeExtraction` reset the types to
`patch` again. A patch `renameBoundary` did not rename was therefore written
as `patch`.

## Predictions (written before any run of the fixed code)

P1. Boundary files, port against the committed upstream runs
(`upstream_cfmesh_gh1385/runs/*/constant/polyMesh/boundary.gz`):

| case | generator | surface types | upstream types | port before | port after |
|---|---|---|---|---|---|
| asmoOctree | cartesianMesh | STL: `empty` | 10 x `wall` | `patch` | 10 x `wall` |
| cutCubeOctree | tetMesh | STL: `empty` | 9 x `wall` | `patch` | 9 x `wall` |
| hatOctree | cartesian2DMesh | `.fms`: `wall` | 5 x `wall`, 2 x `empty` | `patch` x 5, `empty` x 2 | 5 x `wall`, 2 x `empty` |
| sphere | cartesianMesh, tetMesh, pMesh | STL: `empty` | `sphere` `wall` | `patch` | `wall` |
| elbow_90degree | cartesianMesh | STL, `renameBoundary` | `patch`, `patch`, `wall` | unchanged | unchanged |

P2. Points, faces, owners and neighbours do not move: the types are carried
beside the geometry. Every fingerprint that hashes mesh geometry keeps its
value; a fingerprint that hashes patch types moves (checked below).

P3. Foam Studio is unaffected: it uses the tet-dual pipeline
(`pipeline.rs`, `assign_patches_by_region`) with its own patch names and
does not call the four generators.

P4. Callers that build `SurfaceRegions` without types (`single`,
`from_labels`) keep `patch`: no API user sees a change.

## Results

Filled in below after the run.
