# blockMesh projection: `geometry`, `project` vertices / edges / faces, `sweep` faces (gh:#1462, epic gh:#1458)

Upstream: OpenFOAM-dev `852527377c7e87e33d51b19ca8974b5db60af28b` (the
specification) and OpenFOAM.com v2506 (the binary at
`/home/user/upstream-build/OpenFOAM-v2506/platforms/*/bin/blockMesh`, the only
thing that can be *run* here). Code: `src/block_projection.rs`,
`src/block_edges.rs`, `src/block_mesh.rs`. Tests:
`tests/block_mesh_projection.rs`.

## Reading done before predicting (2026-11-11)

Dev sources read: `blockVertices/projectVertex`, `blockEdges/projectEdge`,
`projectCurveEdge`, `blockFaces/{blockFace,projectFace,sweepFace}`,
`blocks/block/blockCreate.C` (the three face-correction passes),
`blockDescriptor/{blockDescriptor.C,blockDescriptorI.H}` (`findCurvedFaces`,
`facePoints`, `correctFacePoints`, `flatFaceOrEdge`), `searchableSurfacesQueries::findNearest`
(single and multi surface), `searchableSurfaces::{sphere,cylinder,plane}`,
`plane::planeIntersect`, `linearInterpolationWeights::valueWeights`.

Differences between dev and v2506 found by diffing the two trees (so a v2506
run is NOT a differential of the dev port everywhere):

| routine | dev `852527377c7e` | v2506 | consequence |
|---|---|---|---|
| `projectVertex` | nearest point on the listed surfaces, search radius `magSqr(bb.span())` | to be diffed in the run | bit identity expected for one surface |
| `projectEdge::position(lambdas)` | identical algorithm and constants (10 iterations, relTol 0.1, absTol 1e-4) | same | bit identity expected |
| `projectFace::project` | projects, predicts along i (adds the i residual), predicts along j, **breaks before adding the j residual** when converged | projects, **breaks right after a projection** when the previous iteration's residual has converged, otherwise adds both residuals | converged face points differ: dev leaves them off the surface by the i residual, v2506 leaves them on it |
| multi-surface `findNearest` | alternates surfaces, intersects the two tangent planes, no step clipping | the same idea with the step clipped to `sqrt(distSqr)` and a `1e-6` collinearity test (dev: `rootSmall`) | agree where the iteration converges without clipping |
| `blockCreate.C` | face passes as quoted in gh:#1462 | same arithmetic (diff shows only renames) | bit identity expected given identical face points |
| `sweepFace` | present | **absent** (v2506 has only `projectFace`) | no binary differential for `sweep`; analytic check only |

The port implements the dev algorithm as the default (it is the specification)
and `ProjectFaceFlavour::Esi` (the v2506 loop) beside it so the binary can be
compared bit for bit on faces too, as `polyDualMesh` did (gh:#1472).

## Predictions, written before the first run

Cases (all dicts in `tests/fixtures/`, run on both codes):
`proj_vertex` (one block, 8 vertices projected onto a sphere, straight edges),
`proj_edges` (sphere7ProjectedEdges, no `faces`), `proj_faces`
(sphere7ProjectedEdges with its `faces`), `proj_cyl` (one block against a
cylinder with `project` edges and faces), `proj_two` (edges on a cylinder and a
plane at once).

P1. **Searchable surfaces, closed form.** Sphere: projected radius equals the
    sphere radius to 4e-16 relative; the centre maps to `centre + (1,0,0) R`.
    Cylinder wall: radial distance from the axis equals the radius to 4e-16;
    beyond an end cap the point lies on the cap disc. Plane: distance to the
    plane after projection is 0 to 4e-16 of the start distance.
P2. **`proj_vertex`, against v2506: every point bit-identical** (28 of 28 files
    fields; 0 differing points).
P3. **`proj_edges`, against v2506: every point bit-identical.** Edge nodes lie
    on the sphere (the loop leaves a projection as its last act on convergence):
    worst radial error below 1e-14 R, and at most 4 iterations on a 10-division
    edge.
P4. **`proj_faces`.** (a) `ProjectFaceFlavour::Esi` is bit-identical to v2506
    on all points. (b) The dev loop differs from v2506: face-interior points
    sit off the sphere by the last i residual, predicted between 1e-9 R and
    1e-4 R (absTol is 1e-4 summed over the face); the v2506 points sit on the
    sphere to 1e-15 R. (c) Volume interior points differ dev vs v2506 by no
    more than 1e-3 R. The dev flavour is NOT claimed verified against a binary.
P5. **`proj_cyl`**: edges and faces on a cylinder: v2506 bit-identical with the
    Esi flavour; face-interior nodes within 1e-4 of the radius for dev.
P6. **`proj_two` (edge on cylinder and plane).** Both flavours' nodes satisfy
    both surface equations to 1e-8 R (the dev loop is 20 alternations); the
    port and v2506 agree to 1e-6 R. A larger gap would be the clipping
    difference in the table above, not a port defect.
P7. **`sweep` faces, torus block of the dev `sweep` tutorial (arcs only).** The
    tutorial calls this "exact": every swept face node is on the torus
    `(sqrt(x'^2 + z'^2) - 2)^2 + y'^2 = 1` (centre `(6,0,0)`, axis y) to 1e-12
    for rotation factor 1; with factor 0 the *other* sweep direction is
    predicted to miss it (the tutorial's own comment), by more than 1e-3.
    The dev sphere block is "not exact": predicted miss between 1e-4 and 1e-1.
P8. **Existing outputs do not move.** `block_mesh_*` tests, T3A points
    (54568, bit-identical to the pre-change build), pitzDailySteady, cavity and
    the Foam Studio / appbuilder / outram-blender consumers are unchanged:
    projection only reads `geometry`, `faces` and `project` entries.
P9. **Nothing is silently skipped.** `extrudedCircle`, `plate`, `box`, `disk`,
    `searchableBox`, an unknown `type`, `triSurface` / `triSurfaceMesh` (until
    ported) return `NotImplemented` or a parse error naming the type; a
    `project` entry naming an undefined surface is an error (upstream
    `Cannot find surface`).

## Results

Measured 2026-11-11. `cargo test --release -p outram-foam-mesh --test
block_mesh_projection`: 13 pass (the three misses below are pinned as
observations, with the prediction quoted in each test). Differential: v2506
`blockMesh` run on the five `tests/fixtures/proj_*.dict` with
`writePrecision 17` and `convertToMeters 1`; compared with
`examples/compare_polymesh` (`PROJECT_FACE_FLAVOUR=esi` for the v2506 flavour).
The topology (faces, owner, neighbour, patches) was identical on every case in
both flavours.

### Differential against OpenFOAM.com v2506 (`points`, bit-identical count)

| case | points | dev flavour (default) | v2506 flavour |
|---|---|---|---|
| `proj_vertex` (8 projected vertices, sphere) | 210 | 210 / 210, worst 0 | 210 / 210, worst 0 |
| `proj_edges` (sphere7ProjectedEdges, no faces) | 7351 | 3169 / 7351, worst 1.1e-16 | **7351 / 7351** |
| `proj_faces` (sphere7ProjectedEdges, with faces) | 7351 | 1221 / 7351, worst **5.8e-4** | **7351 / 7351** |
| `proj_cyl` (cylinder edges and faces) | 567 | 126 / 567, worst 4.4e-16 | **567 / 567** |
| `proj_two` (edges on cylinder and plane) | 567 | 125 / 567, worst 6.7e-16 | **567 / 567** |
| T3A (8 arcs, gh:#1461 item, dev flavour irrelevant) | 54496 | 49965 / 54496, worst 1.1e-16 | not run |

The v2506 flavour reproduces the binary on every point of the five projection
cases (FNV-1a of the bit patterns is frozen in the test). T3A is the pending
upstream differential of the curved-edge work (`block_mesh_curved_edges.md`):
topology identical, 91.7 % of points bit-identical, worst difference one ulp
(1.1e-16), the PETIR `cos`/`sin`/`tan`/`acos` against glibc as predicted there.

### Predictions against results

| P | result | verdict |
|---|---|---|
| P1 | sphere radial error < 4e-16 R, centre case `(R,0,0)`, cylinder wall and cap, plane exact | held |
| P2 | `proj_vertex`: 210 / 210 bit-identical in both flavours | held |
| P3 | v2506 flavour: 7351 / 7351. Edge nodes on the sphere to < 1e-14, at most 4 passes on a 10-division edge (test). **The dev flavour is NOT bit-identical** (3169 / 7351, worst one ulp). | held for v2506 flavour, MISS for the dev default |
| P4a | v2506 flavour 7351 / 7351 | held |
| P4b | wall nodes: v2506 1.1e-16 R (held); dev **2.1e-4 R**, predicted 1e-9 to 1e-4 | MISS (upper bound by a factor 2) |
| P4c | dev vs v2506 worst point difference 7.9e-4 R, predicted <= 1e-3 | held |
| P5 | v2506 flavour 567 / 567; dev wall radial error 2.2e-16 (the first projection converges at once on a cylinder) | held |
| P6 | both flavours: projected vertices within 1e-8 of the circle; dev vs v2506 7.1e-16; v2506 flavour 567 / 567 | held |
| P7 torus, factor 0 on the major edges | exact: worst 2.0e-15; the 50 other nodes are the two flat end discs | held |
| P7 torus, factor 1 on the minor edges | **NOT exact: worst 1.2e-4** (predicted 1e-12) | MISS |
| P7 torus, factor 0 on the minor edges (wrong orientation) | off the torus (270 nodes, 220 beyond the discs) | held |
| P7 sphere block | **0.139** from the sphere (8 % of the radius), predicted 1e-4 to 1e-1 | MISS (upper bound) |
| P8 | `block_mesh`, `block_mesh_curved_edges`, `block_mesh_upstream_numbering` (cavity, T3A, pitz pinned against the v2506 run) and the 72 lib tests pass; `outram-blender --features block-mesh` compiles and its box test passes | held |
| P9 | every unported surface type, undefined surface, bad face refused | held |

### What the misses and the late findings are

1. **The dev flavour is not bit-identical to v2506, by two arithmetic
   differences the pre-run diff did not list.** v2506's sphere is
   `radius*n/magN` (dev `n/magN*radius`), and v2506's
   `linearInterpolationWeights` are `(t1-t)/dT, 1-w0` (dev `1-f, f`); both
   one-ulp effects. v2506's multi-surface loop also uses `SMALL = 1e-15` where
   dev's `small` is epsilon (found when `proj_two` stalled at 152 / 567 with
   the epsilon, then reached 567 / 567). All three are now in the table in
   `block_projection.rs` (`ProjectionFlavour`), and the v2506 flavour is what
   the frozen hashes pin. The dev code stays the default because it is the
   specification; **it is not verified against any dev binary** (none exists
   here).
2. **P4b**: with the dev loop the converged face points are left off the
   surface by the last i residual, 2.1e-4 R on the 10-division sphere face,
   twice my upper bound; the v2506 loop leaves them on it. This is upstream's
   behaviour difference, not a port defect, but it is a real accuracy
   difference between the two OpenFOAM lines (`projectFace` in dev ends a
   converged iteration before the j residual and before the next projection).
3. **P7, the torus.** The tutorial calls the minor-sweep factor-1 case
   "exact"; this port gives 1.2e-4 on a 6 x 6 x 12 block and 3.3e-3 against the
   equal-angle points in a direct test, **independent of the number of rows**
   (`p7b_sweep_twist_error_does_not_shrink_with_rows`: 12 rows 3.25e-3, 96 rows
   3.34e-3). The cause: each row adds the same dihedral increment (measured
   `-0.0926` for 12 rows, `-0.01157` for 96, both `phi * (y . chord)`), so the
   twist is linear in the row angle while the twist of a finite rotation about
   the major axis is not. The sign was tested (flipping it is far worse:
   0.1 to 0.5). The transcription was re-read line by line against
   `sweepFace.C`, the factor-0 major case is exact to 2e-15, and the
   transposed branch is exercised; so I judge this upstream behaviour, but it
   **cannot be confirmed without a dev `blockMesh`**, and the tutorial's word
   "exact" is not reproduced. **Flagged for the maintainer.**
4. **P7, the sphere block** falls short of the sphere by 0.139, consistent with
   the tutorial's "not exact ... falls short", at a larger size than I
   predicted.

## What this does and does not show

- v2506 is matched bit for bit on `project` vertices, edges and faces for the
  sphere and the cylinder, and on two-surface edges; that checks the port of
  the *algorithms* (the loops, the weights, the three correction passes). It
  does not check the dev-only differences (the dev face loop, the dev
  multi-surface step) against any binary.
- `sweep` has no binary differential at all (v2506 has no `sweepFace`); it is
  checked only analytically, with the miss above.
- Not ported, refused with `NotImplemented`: `triSurface` / `triSurfaceMesh`,
  `extrudedCircle` (and `projectCurve`'s special handling of it), `plate`,
  `box`, `disk`, planes given by equation or three points. The dev `pipe`,
  `sphere` and `sweep` (cone) tutorials need one of these or `#eval`/`#calc`/
  named vertices with projection, so they were not run.
- Dicts use the `searchableSphere` / `searchableCylinder` / `searchablePlane`
  names and `centre`; the port also reads the dev names (`sphere`,
  `cylinder`, `plane`) and `origin`.

## Drawings (`images/`)

Drawn from the built `PolyMesh` by `examples/draw_block_mesh.rs`
(`--cut Z` draws the section of the mesh by the plane `z = Z`, boundary faces
red; `--esi` selects the v2506 flavour).

- `proj_sphere7_edges_only_section.png`: sphere7ProjectedEdges, edges projected
  but no `faces`, section z = 0.0123: the outer boundary is a rounded square
  (the six outer block faces are flat between projected edges).
- `proj_sphere7_faces_dev_section.png`, `proj_sphere7_faces_v2506_section.png`:
  the same with the six `project` faces: the section is a circle of radius 1.
- `proj_cylinder_block_zmin.png`: one block with four cylinder-projected side
  faces, plane z = 0.2: a square block mapped to a disc.
- `sweep_sphere_block_section.png`, `sweep_torus_minor1_section.png`,
  `sweep_torus_major0_section.png`: swept blocks, sections z = 1.013 / 1.513.

Checked in the images: the section outlines are the expected curves (circle
for the faces-projected sphere, rounded square without faces, disc for the
cylinder block); the cells are all visible and none folds or crosses at this
resolution; the sphere7 inner block stays a rounded cube around the centre.
Not checked: cell quality below a pixel (only that `to_fv_mesh` accepts the
mesh in the dome test), the torus sections are oblique cuts through curved
cells so they show diagonal crossings and say nothing about the 1.2e-4 miss,
and the dev and v2506 sphere sections differ by 8e-4 R at most, which is
invisible at this size.
