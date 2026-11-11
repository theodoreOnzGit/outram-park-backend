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

(filled in below after the runs)
