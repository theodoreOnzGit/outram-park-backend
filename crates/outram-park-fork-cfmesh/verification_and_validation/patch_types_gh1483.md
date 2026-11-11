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

Run 2026-11-11, release, shared target dir. Names and types (in order) of
the written `boundary` against the committed upstream `boundary.gz`
(`outram-foam-cli/tests/cfmesh_patch_types.rs`,
`tests/cfmesh_patch_types_sphere.rs`):

| case | generator | port after | upstream | equal |
|---|---|---|---|---|
| asmoOctree | cartesianMesh | 10 x `wall` (same names) | 10 x `wall` | yes |
| cutCubeOctree | tetMesh | 9 x `wall` | 9 x `wall` | yes |
| hatOctree | cartesian2DMesh | 5 x `wall`, 2 x `empty` | same | yes |
| sphere | cartesianMesh, tetMesh, pMesh | `sphere` `wall` | `sphere` `wall` | yes |
| elbow_90degree | cartesianMesh | `patch`, `patch`, `wall` | same | yes (unchanged) |

- P1 held on every row. The "port before" column was not re-run on the old
  code: it is read from the old source (`vec!["patch"; n]` at the edge
  extractor and after `edgeExtraction`), not measured.
- Incidental, printed by the CLI test and not asserted: face counts equal
  upstream's on asmoOctree (all 10 patches), cutCubeOctree (all 9) and
  hatOctree (all 7); the elbow's `fixedWalls` is 18384 against upstream's
  18477 (inlet and outlet equal). Not a #1483 finding; #1385's remit.
- P2 held: no pin moved. Run with `--features cell-centre-dual`:
  `back_off_only_layers_are_pinned_bit_for_bit`,
  `dual_kinds_are_pinned_bit_for_bit`, `octree_path_is_pinned_bit_for_bit`,
  `layer_strategies_are_deterministic` (`tet_dual_stages_are_deterministic`),
  `correct_edges_is_pinned_bit_for_bit`,
  `foam_studio_default_mesh_keeps_its_fingerprint`,
  `prism_refinement_is_pinned_bit_for_bit`,
  `generators_without_anisotropic_sources_are_pinned_bit_for_bit`, the three
  `*_stop_and_restart_reproduce_the_full_run` pins; and
  `cfmesh_vs_upstream_sphere` (4), `cfmesh_vs_upstream_cell_order` (1),
  `outram-foam-cli` `cfmesh_tools` (2). None hashes patch types.
- P3 held: Foam Studio's pin passes; the tet-dual pipeline does not call the
  four generators.
- P4 held: `a_surface_without_types_keeps_patch`.
- Gate `cargo test --release -p petir --test no_platform_libm`: 4 passed
  (no transcendental added).
- Not run: the full `--lib --tests` workspace suite (quick targeted runs
  only, disk and CPU).
