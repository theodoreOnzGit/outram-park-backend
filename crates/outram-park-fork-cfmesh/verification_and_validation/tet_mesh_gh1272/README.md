# cfMesh `tetMesh` as upstream: V&V record (gh:#1272)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-written port, not yet reviewed by
a human.

**Upstream:** `github.com/DAFoam/cfmesh` at `e944f6ba695f` (cfMesh v1.1 line).

## Predictions (written 2026-11-05, before any run)

Setup A, sphere: R 3 m, 24 x 48 UV surface (analytic 113.0973 m^3),
`maxCellSize` 0.5 (the octree root is 16 m, so leaves are exactly 0.5 m),
no `boundaryCellSize`. Setup B, pipe: R 10 mm, L 60 mm, 48 sides, patches
walls/inlet/outlet, `maxCellSize` 2 mm. Baseline at equal cell size: the
crate's `carve_box` + `snap` + `tetrahedralize` + `flip_to_delaunay`
(`surface_to_tet_dual_mesh` with `dual: false`, no layers).

1. `tetCreatorOctree` on a uniform octree (setup A): no sub-nodes, no face
   centres, no split edges; every tet is "around an edge" (two leaf centres
   and an octree edge: the BCC lattice). Every tet has positive volume. No
   read upstream leaves undefined (all `TetCreatorDefects` zero).
2. The extracted tet mesh is closed (`validate()`), every face in at most
   two tets. Its volume before mapping equals the volume of the `MESHCELL`
   leaves minus one pyramid `h^3/6` per leaf face on the region boundary,
   to 1e-12 relative.
3. On the graded pipe (B with `boundaryCellSize` 1 mm) the split-edge and
   face-centre tets appear, still all positive, defects zero.
4. Full workflow (A): all tets positive and the mesh closed. Volume error
   against 113.0973: I predict a **miss** of the README's uniform 0.5 m
   figure (1.72 %), because `surfaceMorpherCells` / `topologicalCleaner`
   (topology track) and `meshSurfaceOptimizer::untangleSurface` (surface
   track) are not ported: the stand-in reverts tangled boundary moves, so
   part of the boundary stays on the octree, outside the surface. Expected
   between 1.72 % and 8 %.
5. Tet count (A) between 1.0 and 1.6 times the baseline's.
6. Before mapping, the octree tets' max non-orthogonality is below the
   baseline's (BCC tets are near-regular; centroid-subdivision tets are not).
7. Run time: `tet_mesh` slower than the baseline (the optimiser with
   boundary moves and the LP untangler runs; the baseline has no optimiser).
8. `renumberMesh` leaves the internal faces upper-triangular; the patches
   keep their faces.

## Results (2026-11-05, `cargo test --release`)

Tests: `pipeline::tet_mesh::tests::{tet_creator_on_the_sphere_and_the_pipe,
diagnose_tet_mesh_against_the_crate_tets,
tet_mesh_stop_and_restart_reproduce_the_full_run}`,
`renumber_mesh::tests::renumber_mesh_is_upper_triangular_and_depth_first`,
`rename_boundary_patches::tests::*`, `workflow_controls::tests::*`.

### The tet creator and extractor (predictions 1–3)

| case | leaves | nodes | tet points | tets | dropped by `checkAndAppendTet` | negative | defects |
|---|---|---|---|---|---|---|---|
| sphere, 0.5 m | 2 584 | 2 277 | 4 877 | 13 968 | 144 | 0 | 0 |
| pipe 48, 2 mm / 1 mm | 21 288 | 21 449 | 55 595 | 195 136 | 2 496 | 0 | 0 |

- **Prediction 1 missed (its premise was wrong).** The octree is not
  uniform. The `MESHCELL` leaves are all level 5, but 96 of the 360
  `BOUNDARY` leaves next to them are coarser (level 4, `OUTSIDE`).
  Sub-nodes appear at their corners (4 877 points, not 3 557). Positivity
  and zero defects held.
- **Prediction 2 missed, then explained exactly.** The tet volume is
  145.5 m^3, not 142.5 (the `MESHCELL` leaves minus `h^3/6` per
  region-boundary face, 840 faces).
  - The extra 3.0 m^3 = 24 h^3 is 288 tets whose centroids lie in the
    coarse `BOUNDARY` leaves.
  - `createTetsFromFacesWithCentreNode` builds the four tets
    `(node, sub-node, sub-node, face centre)` without a cube label, so they
    pass `checkAndAppendTet` even where the leaf is not a cell. That is
    upstream's code as written.
  - These tets hang off the mesh at single points (the diamonds in
    `sphere_xz_tet_template.png`). They make 96 non-manifold boundary points
    on the sphere and 2 240 on the pipe.
  - Pinned: 13 968 tets, 288 outside, 144 dropped. The volume equals the
    expected value plus the outside tets to 1e-12.
- Prediction 3 held. On the graded pipe the split-edge and face-centre tets
  appear (55 595 points against 21 449 nodes), all positive, with no defect.
- Every face lies in at most two tets, and the mesh is closed in both cases.

### The workflow against the crate's tets (predictions 4–7)

Both routes use the same cell size: 0.5 m for the sphere (`maxCellSize`)
and 2 mm for the pipe. The baseline is `carve_box` + `snap` +
`tetrahedralize` + `flip_to_delaunay`. Volume error is against 113.0973 m^3
(sphere) and the 48-gon prism, 1.8796e-5 m^3 (pipe).

| case | route | cells | volume error | non-positive | max non-orth | max skew | time |
|---|---|---|---|---|---|---|---|
| sphere | `tet_mesh` | 13 968 | **+26.69 %** | 0 | 57.09° | 1.640 | 0.12 s |
| sphere | template only | 13 968 | +28.65 % | 0 | 0.00° | 0.125 | — |
| sphere | baseline | 21 888 | −1.72 % | 0 | 80.23° | 0.402 | 2.66 s |
| pipe | `tet_mesh` | 35 416 | **+22.33 %** | 0 | 66.15° | 1.198 | 1.40 s |
| pipe | template only | 34 560 | +22.58 % | 0 | 0.00° | 0.125 | — |
| pipe | baseline | 57 600 | −0.54 % | 0 | 68.42° | 0.412 | 0.34 s |

- **Prediction 4 missed badly.** All cells are positive and the meshes are
  closed (held). But the volume error is +26.7 % (sphere) and +22.3 %
  (pipe), far outside the 1.72–8 % I predicted.
  - The cause is visible in `sphere_xz_tet_mesh.png`: the boundary stays on
    the octree, outside the surface.
  - The stand-in for `untangleSurface` reverts the moves of every cell the
    mapping tangles. On this template that is 1 238 boundary points on the
    sphere and 3 539 on the pipe: nearly all of them.
  - The template's boundary is the DATA leaves' outer faces, each with a
    pyramid cut to the leaf centre, so the leaf centres are boundary points
    too. Projecting both the corners and the centres onto the surface
    flattens the boundary tets.
  - Upstream removes those cells first (`surfaceMorpherCells`,
    `topologicalCleaner`: the topology track), and untangles instead of
    reverting (`meshSurfaceOptimizer`: the surface track).
  - **This number measures the missing steps, not the ported ones.**
    Re-measure once they land.
- **Prediction 5 missed.** The cell ratio is 0.64 (sphere) and 0.61 (pipe),
  not 1.0–1.6.
- Prediction 6 held, more strongly than predicted: the template's
  non-orthogonality is exactly 0. Its tets are the BCC lattice's congruent
  disphenoids, whose centroid is their circumcentre.
- **Prediction 7 missed on the sphere** (0.12 s against 2.66 s: the
  optimiser found no bad face). It held on the pipe (1.40 s against
  0.34 s), mostly spent in `correctEdgesBetweenPatches`, which made 156
  pyramids.
- Prediction 8 held (`renumber_mesh` test): the internal faces are
  upper-triangular and the patches keep their faces.

### Workflow controls and restart

`tet_mesh_stop_and_restart_reproduce_the_full_run` (pipe, 24 sides, 4 mm):

- Stopped after `surfaceProjection`, the mesh is neither renumbered nor
  renamed (one patch, `defaultFaces`).
- Restarted from that mesh and its meta data, the run resumes at
  `patchAssignment` and gives the uninterrupted run's mesh **bit for bit**.
- Two runs give the same mesh bit for bit.

## Images (`examples/tet_mesh_images.rs`)

```text
cargo run --release -p outram-park-fork-cfmesh --example tet_mesh_images -- crates/outram-park-fork-cfmesh/verification_and_validation/tet_mesh_gh1272
```

The example also writes three Voronoi images; they were moved to
`../p_mesh_gh1273/`. Each image shows the mesh edges cut by the slice
(blue), the surface (black) and the octree leaf types underneath.

| file | what |
|---|---|
| `sphere_xz_tet_template.png` | sphere, the extracted octree tets, x-z at y = 0.01 |
| `sphere_xz_tet_mesh.png`, `sphere_xy_tet_mesh.png` | the `tet_mesh` result, x-z and x-y |
| `pipe_xy_tet_mesh.png`, `pipe_xz_tet_mesh.png` | pipe, `tet_mesh` at 2 mm, mid-length x-y and axial x-z (the x-z image is stretched along z) |
| `pipe_xy_graded_tet_template.png` | pipe, the graded template (1 mm boundary cells), by level |

**What I checked in them:**

- The template fills the INSIDE and DATA leaves with the BCC pattern (two
  diagonals per leaf in the slice). It stops at the DATA leaves' outer
  faces, outside the surface.
- The isolated diamonds outside the band are the 288 tets in coarse
  BOUNDARY leaves (finding above). In the `tet_mesh` result they are
  stretched toward the surface.
- In the `tet_mesh` images the boundary is still on the octree: the
  +26.7 % / +22.3 % volume error is the DATA band.
- The graded template refines one level at a time, and the tets follow the
  levels.

**What I could not check:** 3D views; the inside of a cell (the slices
show cut edges only); the patch of each boundary face (not coloured).

## Not ported, called through what exists, or skipped

See the `pipeline::tet_mesh` docs for the step-by-step table.

- ~~`surfaceMorpherCells`, `topologicalCleaner` (topology track): skipped.~~
  **CORRECTED 2026-11-05:** called since gh:#1267 landed (re-measurement
  below).
- ~~`meshSurfaceOptimizer::untangleSurface` / `optimizeSurface` and
  `meshOptimizer::optimizeSurface` (surface track): skipped. Tangled mapping
  moves are reverted instead.~~ **CORRECTED 2026-11-05:** called since
  gh:#1259 landed; nothing is reverted any more.
- `edgeExtractor` (patch assignment): the nearest triangle stands in.
- `boundaryLayers`, `refineBoundaryLayers` (layers track): not offered. The
  workflow runs as upstream does without a `boundaryLayers` dict.
- ~~`symmetryPlaneOptimisation` in `renameBoundaryPatches`: reported, not run.~~
  **CORRECTED 2026-11-05 (gh:#1280):** run (`symmetry_plane_optimisation`);
  no case here has a `symmetryPlane` patch, so no number above moves.
- `anisotropicSources` (~~not ported~~ the classes are ported since
  gh:#1279, `anisotropic`, ~~but not wired into the workflow~~ **wired since
  gh:#1279's remainder, `../anisotropic_gh1279/`**), `triSurfacePatchManipulator`, and `meshDict` and
  mesh file I/O (a restart holds the mesh in memory).

## Upstream defects found (none filed; for the maintainer)

- `renumberMesh` is documented as Cuthill–McKee, but it pops the last
  element (`LongList::remove`): it is a depth-first walk. Kept.
- `tetCreatorOctree` reads `VRWGraph` rows that may be empty, and
  `nodeLabels` that may be `-1`, without a guard (undefined in a release
  build). The port counts such reads: zero on every run here.
- `createTetsFromFacesWithCentreNode` creates tets inside non-cell
  (`BOUNDARY`) leaves (finding above). Kept as upstream.
- `workflowControls` records each step twice in `completedSteps`, and a
  `stopAfter` of the last step never triggers. Kept.

## Re-measurement after the topology and surface tracks landed

gh:#1267 (`surfaceMorpherCells`, `topologicalCleaner`) and gh:#1259
(`meshSurfaceOptimizer`: `untangleSurface`, `optimizeSurface`,
`meshOptimizer::optimizeSurface`) merged into the branch after the run
above. The workflow now calls them in upstream's places, and
`remapBoundaryPoints` maps without reverting (upstream has no revert; the
untangler follows it). The table above stays as the record of the run with
the gaps.

**Predictions (written 2026-11-05, before the re-run):**

- R1. `surfacePreparation` removes the 288 tets in BOUNDARY leaves and the
  boundary cells with all points on the boundary; after it, no
  non-manifold boundary point is left.
- R2. All cells positive and the mesh closed on both cases.
- R3. Volume error within ±3 % on the sphere and ±2 % on the pipe (the
  boundary now reaches the surface).
- R4. `untangleSurface` leaves 0 inverted points after the projection.

**Results (2026-11-05, same tests and setups):**

| case | route | cells | volume error | non-positive | non-manifold boundary points | max non-orth | max skew | time |
|---|---|---|---|---|---|---|---|---|
| sphere | `tet_mesh` | 12 840 | **−1.36 %** | 0 | 0 | 51.56° | 0.437 | 0.19 s |
| sphere | baseline | 21 888 | −1.72 % | 0 | — | 80.23° | 0.402 | 2.56 s |
| pipe | `tet_mesh` | 31 136 | **−0.29 %** | 0 | 0 | 67.16° | 0.624 | 1.53 s |
| pipe | baseline | 57 600 | −0.54 % | 0 | — | 68.42° | 0.412 | 0.32 s |

- R1 held: one `surfacePreparation` pass. The morpher removed 1 128 cells
  (sphere) and 3 424 (pipe); no non-manifold boundary point is left.
- R2 held.
- R3 held, inside both bounds: −1.36 % and −0.29 %. `tetMesh` as upstream is
  now closer to the surface than the crate's tets at the same cell size,
  with 41–46 % fewer cells and a lower maximum non-orthogonality on the
  sphere (51.6° against 80.2°).
- R4 held: `untangleSurface` took the pipe from 27 inverted points to 0;
  the sphere had none.
- Pipe patches after renaming: inlet 332, outlet 332, walls 3 424 faces.
  `correctEdgesBetweenPatches` changed nothing on this mesh.
- Upstream steps still not in the workflow: `edgeExtractor` (the nearest
  triangle stands in), `boundaryLayers` / `refineBoundaryLayers` (no dict),
  ~~`symmetryPlaneOptimisation`~~ (run since gh:#1280; no `symmetryPlane`
  patch here).

The images above were regenerated after this re-run.
~~"In the `tet_mesh` images the boundary is still on the octree"~~
**CORRECTED 2026-11-05:** the boundary now lies on the surface trace all
round in `sphere_xz_tet_mesh.png`, `sphere_xy_tet_mesh.png` and
`pipe_xy_tet_mesh.png` (checked), with no fold and no detached tets. The
template images are unchanged.

## Re-measurement with `edgeExtractor` wired in (gh:#1269, 2026-11-05)

`patchAssignment` now runs the ported `edgeExtractor`
(`edge_extractor::extract_patches`); the nearest triangle is the ablation
`PatchAssignment::NearestTriangle`. ~~"Upstream steps still not in the
workflow: `edgeExtractor` (the nearest triangle stands in)"~~ **CORRECTED
2026-11-05 (gh:#1269)** for `edgeExtractor`. Predictions and the full
differential: `../edge_extractor_gh1269/README.md`.

| case | patch assignment | cells | volume error | max non-orth | max skew | patches inlet / outlet / walls |
|---|---|---:|---:|---:|---:|---|
| sphere | `edgeExtractor` (one patch) | 12 840 | −1.362 % | 51.56° | 0.437 | — |
| pipe | `edgeExtractor` (default) | 31 136 | **−0.291 %** | 67.16° | 0.634 | 330 / 331 / 3 427 |
| pipe | nearest triangle | 31 136 | −0.289 % | 67.16° | 0.624 | 332 / 332 / 3 424 |

11 pipe faces change patch; the inlet's projected area moves from 0.99480 to
0.99361 πR² (the 48-gon is 0.99715), away from the polygon. The images were
regenerated; only `pipe_xz_tet_mesh.png` changed (and `pipe_xy_p_mesh.png`
in `../p_mesh_gh1273/`). Checked in it: the boundary follows the surface
trace, the four rim corners are filled, no fold.
