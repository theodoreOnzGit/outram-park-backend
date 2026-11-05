# cfMesh `edgeExtractor` as upstream: V&V record (gh:#1269)

Port: `src/edge_extractor.rs` (`edgeExtractor.{C,H}`, the `faceEvaluator`
of `edgeExtractorCorners.C`) and `src/mesh_surface_edge_extractor_fun.rs`
(`meshSurfaceEdgeExtractorFUN` without its fundamental sheets), from
DAFoam/cfmesh `e944f6ba695f`. Wired into `pipeline::tet_mesh` and
`pipeline::p_mesh` in upstream's order (`patchAssignment`, then
`edgeExtraction`); the nearest-triangle rule stays callable as the ablation
`PatchAssignment::NearestTriangle`.

## Methodology

- **Reference.** Upstream is the specification; this is a code-to-code port
  without an upstream build, so the checks are the plan's box test, planted
  errors whose upstream outcome is worked out by hand from the source, and a
  differential against the nearest-triangle rule it replaces.
- **Differential A (Foam Studio pipe).** R 10 mm, L 200 mm, 48 sides,
  patches walls / inlet / outlet; `surface_to_tet_dual_mesh_multipatch` with
  `CellCentre` + `Corners`, bare (`feature_edges: false`, no layers), at
  h 4, 5, 6 mm. On that mesh, upstream's `tetMesh` order: `mapMeshToSurface`
  (`surface_optimizer::map_mesh_to_surface`), then the patch assignment by
  (a) the nearest triangle to each face's vertex average and (b)
  `edgeExtractor` (octree built with `maxCellSize` h), then the patch
  mapping `mapVerticesOntoSurfacePatches` on each assignment. Recorded: the
  boundary faces that change patch (b against a, and where they are), the
  inlet's projected area `|Σ S_z| / πR²` after mapping (the 48-gon is
  0.99715), the inlet rim (points, mean and minimum radius), validity.
- **Differential B (`tet_mesh` / `p_mesh` on the pipe).** R 10 mm, L 60 mm,
  48 sides, `maxCellSize` 2 mm, `PatchAssignment::EdgeExtractor` (default)
  against `NearestTriangle`: volume error against the 48-gon prism
  (1.8796e-5 m³), cells, max non-orthogonality, patch face counts, inlet
  `|Σ S_z| / πR²`.
- **Images.** `examples/edge_extractor_images.rs`: close-ups of the inlet rim
  of differential A, both assignments.

## Predictions (written 2026-11-05, before any run)

1. Box (three patches, carve 0.25): both rules put every boundary face on
   its side; the extractor moves no point; no inverted point.
2. Planted errors on the box: a rim face planted in the wrong patch is
   restored by `checkFacePatchesTopology`; an interior face whose four
   neighbours are all in one other patch is **not** (upstream's
   `findFaceCandidates` does not list it).
3. The undefined-read counters (`n_unset_neighbour_patch_reads`,
   `n_failed_region_searches`) are 0 on every run.
4. Differential A, faces that change patch: only at the two rims (no face
   whose centre is more than one h from z = 0 or z = L), between 0 and 30
   faces per rim at each h.
5. Differential A, inlet `|Σ S_z| / πR²` after mapping: closer to 0.99715
   with `edgeExtractor` than with the nearest triangle at every h (a face
   whose normal is radial goes to the wall, one whose normal is axial to the
   inlet).
6. Differential B, `tet_mesh`: volume error within ±0.5 % (it was −0.29 %),
   all cells positive, mesh closed; max non-orthogonality within ±10° of
   67.16°; three non-empty patches.
7. Differential B, `p_mesh` (with `meshSurfaceEdgeExtractorFUN`'s order,
   smooth then map): volume error within ±0.6 % (it was −0.35 %), no
   negative cell.

## Results

(to be filled from the runs; misses kept)
