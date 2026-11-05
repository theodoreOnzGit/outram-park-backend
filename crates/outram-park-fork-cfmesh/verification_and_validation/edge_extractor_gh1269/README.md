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

## Results (2026-11-05, `cargo test --release --features cell-centre-dual`)

Tests (`src/edge_extractor.rs`): `chain_and_map_order_helpers`,
`box_with_three_patches_every_face_on_its_side`,
`box_auxiliary_steps_and_planted_error`, `tet_and_p_mesh_run_the_edge_extractor`
(all pass); diagnostics `diagnose_foam_studio_pipe_differential` and
`diagnose_edge_extractor_in_tet_and_p_mesh` (`--ignored`, printed numbers
below). Pins re-run and passing: `back_off_only_layers_are_pinned_bit_for_bit`,
`dual_kinds_are_pinned_bit_for_bit` (these two cover the refactored
`surface_mapper` path), `octree_path_is_pinned_bit_for_bit`,
`layer_strategies_are_deterministic`, `correct_edges_is_pinned_bit_for_bit`,
`tet_mesh_stop_and_restart_reproduce_the_full_run`.

- **Prediction 1 held.** 96 boundary faces, 0 misassigned by either rule, no
  inverted point, no face moved; volume 1 to 1e-12. The constructor's data
  match hand counts: valence 2/3/4 (8 corners at 3), 48 single-cell edges,
  31 patch-border points.
- **Prediction 2 held.** The rim face goes back (`topology_proposed`
  `[1, 0]`); the island face stays in the wrong patch. That is upstream's
  behaviour as written: `findFaceCandidates` never lists a face whose
  neighbours are all in one patch, even another one. Recorded as a
  limitation, not a port defect.
- **Prediction 3 held.** Both undefined-read counters are 0 on every run.

### Differential A: Foam Studio pipe, `CellCentre` + `Corners`, bare

| h | faces that change patch | where | inlet faces (nearest / extractor) | inlet `|Σ S_z|/πR²` nearest | extractor | inlet rim edges nearest / extractor | `checkFacePatchesGeometry` inverted points | time |
|---|---:|---|---|---:|---:|---|---:|---:|
| 4 mm | 16 | 8 inlet→walls, 8 outlet→walls, all within h of an end | 61 / 53 | 0.98830 | 0.98765 | 48 / 32 | 948 | 0.6 s |
| 5 mm | 0 | — (normal alignment moved 16, topology moved the same 16 back) | 57 / 57 | 0.98760 | 0.98762 | 36 / 36 | 732 | 0.4 s |
| 6 mm | 5 | 3 outlet→walls, 2 walls→inlet, all within h of an end | 35 / 37 | 0.97508 | 0.97486 | 23 / 21 | 457 | 0.6 s |

The 48-gon's own ratio is 0.99715. Both assignments give valid meshes with
no negative cell after the mapping; max non-orthogonality nearest /
extractor: 74.1° / 74.5° (4 mm), 74.7° / 72.7° (5 mm), 93.7° / 91.2° (6 mm,
above 90° on both: the mapping alone, without the untangler, folds a face at
that h).

- **Prediction 4 held**: every change is at a rim, 0–16 per rim.
- **Prediction 5 missed.** The inlet's projected area is not closer to the
  48-gon with `edgeExtractor`: −0.00065 at 4 mm, +0.00002 at 5 mm, −0.00022
  at 6 mm. The faces it moves at 4 mm are the 8 chamfer quads at each end
  (|n_z| ≈ 0.68, centroid r 9.49 mm, z 0.48 mm, at θ ≈ ±39°, ±51°, ±129°,
  ±141°): their edges border the wall more than the inlet, so
  `checkFacePatchesTopology` moves them to the wall. With the nearest
  triangle they stay inlet and the mapping flattens them into the inlet
  plane; with `edgeExtractor` they become wall faces standing at the rim
  (`foam_studio_h4_rim_side.png`). Either way the rim chamfer is a property of
  the `Corners` dual, which no patch assignment removes: the rim deficit is
  ~1.2 % of πR² with both. At 6 mm a face with |n_z| = 0.977 goes to the wall
  and two with |n_z| = 0.34 go to the inlet, by the topology rule, against
  their normals.
- **The 5 mm oscillation.** `distributeBoundaryFacesNormalAlignment` moves
  16 faces, and `checkFacePatchesTopology` moves the same 16 back: the net
  assignment equals the nearest-patch one. Upstream's two steps disagree on
  these faces; nothing reconciles them.
- **`checkFacePatchesGeometry` moves points even when no patch changes.**
  Its first mapping finds 457–948 inverted points (the reflex corners of the
  dual's non-convex boundary polygons noted in gh:#1258) and untangles them
  (`untangleSurface`, one layer), smoothing the inlet's polygons
  (`foam_studio_h4_inlet_xy.png`: the right panel's polygons are rounder).
  No face then changes patch (`geometry_moves` `[0]`), so the loop ends
  after one pass.

### Differential B: `tet_mesh` and `p_mesh` on the pipe (48 sides, 2 mm)

| workflow | patch assignment | cells | volume error | non-positive | max non-orth | max skew | patches inlet / outlet / walls | inlet `|Σ S_z|/πR²` | inlet rim edges |
|---|---|---:|---:|---:|---:|---:|---|---:|---:|
| `tet_mesh` | `edgeExtractor` (default) | 31 136 | −0.291 % | 0 | 67.16° | 0.634 | 330 / 331 / 3 427 | 0.99361 | 46 |
| `tet_mesh` | nearest triangle | 31 136 | −0.289 % | 0 | 67.16° | 0.624 | 332 / 332 / 3 424 | 0.99480 | 52 |
| `p_mesh` | `edgeExtractor` (default) | 4 303 | −0.325 % | 0 | 55.29° | 0.577 | 121 / 121 / 1 408 | 0.99604 | 80 |
| `p_mesh` | nearest triangle | 4 303 | −0.324 % | 0 | 64.46° | 0.493 | 125 / 125 / 1 400 | 0.99609 | 88 |
| `p_mesh` | before gh:#1269 (nearest, map then optimise) | 4 303 | −0.35 % | 0 | 75.9° | — | — | — | — |

`edgeExtractor` on `tet_mesh`: 11 faces moved from the nearest-patch
distribution (normal alignment 5, topology 10 then 2; 7 inverted points at
the geometry check, no face moved). On `p_mesh`: 8 (topology). The sphere
(one patch) is unchanged (−1.362 % `tet_mesh`, −1.537 % `p_mesh`).

- **Prediction 6 held**: −0.291 %, all cells positive, closed, 67.16°
  (unchanged), three non-empty patches. The inlet's projected area moved
  away from the 48-gon (0.99480 → 0.99361), as in differential A.
- **Prediction 7 held**: −0.325 %, no negative cell. Ablation of the two
  `p_mesh` changes: `meshSurfaceEdgeExtractorFUN`'s order (smooth, then
  map; the stand-in mapped first) took the maximum non-orthogonality from
  75.9° to 64.46°; `edgeExtractor` took it on to 55.29°. The volume moved by
  0.03 points, nearly all from the order.

## Images (`examples/edge_extractor_images.rs`)

```text
cargo run --release -p outram-park-fork-cfmesh --features cell-centre-dual \
    --example edge_extractor_images -- crates/outram-park-fork-cfmesh/verification_and_validation/edge_extractor_gh1269
```

| file | shows |
|---|---|
| `foam_studio_h{4,5,6}_inlet_xy.png` | the inlet end along the axis, nearest (left) and `edgeExtractor` (right), after the mapping; changed faces red |
| `foam_studio_h{4,5,6}_rim_unrolled.png` | the inlet faces unrolled, angle across, radius 10.05 → 9 mm down (clamped): the rim's shape |
| `foam_studio_h{4,5,6}_rim_side.png` | the wall within 1.5 h of the inlet unrolled, angle across, z down: where the changed faces stand |

Checked in them: at 4 mm the 8 changed inlet faces are a red line in the
inlet plane with the nearest triangle and red wall quads at the rim with
`edgeExtractor`; the rim follows the 48-gon in both; the right-hand inlet
polygons are smoother (the untangler). At 6 mm the two walls→inlet faces are
slivers at z = 0 in both panels. Not checked: the outlet end (not drawn), and
the faces across the θ = 0 seam (dropped from the unrolled views).

## Upstream defects and limitations found (none filed)

- `checkCorners` calls `::exit(0)` part-way (`edgeExtractorCorners.C`
  line 1173): unfinished code that would end the process; no caller.
- `findEdgeCandidates` reads `sp[edges[seI].start()]` with `edges` the mesh's
  boundary edges and `seI` a surface edge label; no caller.
- `faceEvaluator::bestPatchAfterModification` dereferences
  `newOtherFacePatchPtr_`, which is null in a serial run (bound to a
  reference, never read there).
- `checkFacePatchesTopology`'s OpenMP loop writes `newBoundaryPatches` while
  the `faceEvaluator` reads it through a pointer: a race; the port runs the
  serial order.
- `checkFacePatchesGeometry` returns `false` when a later pass finds no
  inverted point, even if an earlier pass changed patches.
- `findFaceCandidates` cannot repair an isolated face whose neighbours are
  all in another patch (prediction 2).
- Unset reads (counted, 0 here): `neiPatches` over an edge without two faces,
  and the triangle of a failed region search.

## Not ported (see `src/edge_extractor.rs` docs)

`findEdgeCandidates`, `checkConcaveEdgeCells` (needs `triSurfaceClassifyEdges`),
`findCornerCandidates`, `checkCorners`, `cornerEvaluator` (no caller upstream
for any of them); the MPI branches; ~~`createFundamentalSheetsJFS` in
`meshSurfaceEdgeExtractorFUN` (needs `extrudeLayer`, item 26)~~
(**CORRECTED 2026-11-05:** ported and run since gh:#1280/#1281, record
`../fundamental_sheets_gh1281/`). The tet-dual
pipeline (`surface_to_tet_dual_mesh_multipatch`) still assigns by the
nearest triangle; the extractor is not wired there (not an upstream
workflow).
