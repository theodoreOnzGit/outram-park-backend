# blockMesh curved edges, units and defaultPatch: V&V record (gh:#1461, gh:#1463, epic gh:#1458)

Upstream: OpenFOAM-dev `852527377c7e87e33d51b19ca8974b5db60af28b`
(`crates/outram-foam-mesh/upstream_source/README.md`). Code:
`src/block_edges.rs`, `src/block_mesh.rs`. Tests: `tests/block_mesh_curved_edges.rs`.

## Status of the upstream-run differential

No upstream `blockMesh` binary exists on this machine
(`find /home/user/upstream-build -name blockMesh -type f` returned nothing on
2026-11-11; that tree has cfMesh's libraries only). So the `points`/`faces`
comparison against a real `blockMesh` run on T3A, sphere and pitzDailySteady is
**not done** and stays a remaining item on gh:#1364. What is checked instead is
each port against upstream's own analytic definitions (below).

## Predictions, written before the first run (2026-11-11)

P1. **Quarter-annulus block, two `arc` edges (through-point form).** Vertices
    (1,0,0) (2,0,0) (0,2,0) (0,1,0), arcs 1-2 through (2cos45, 2sin45) and 0-3
    through (cos45, sin45), cells (n, 4, 1), uniform grading. Upstream's
    transfinite blend places interior nodes on circles of radius
    r1 + s(r2 - r1) at angle t*pi/2. Predicted: every node has
    |r - r(s)| < 1e-14 and angle error < 1e-14 rad, and the mesh volume is the
    closed form (n/2) sin(pi/(2n)) (r2^2 - r1^2) with z thickness 1, relative
    error < 1e-12. The same dict with the `edges` list removed (what the port
    did before) is predicted to miss by O(1): a straight-sided trapezoid has
    area 1.5 against 2.356 for the sector.
P2. **Arc from angle and axis**, 90 degrees about z through (1,0,0)->(0,1,0):
    centre at the origin, radius 1 to < 1e-15; with p1 = (0,1,1) a helix whose
    z is linear in lambda to < 1e-15.
P3. **polyLine** passes through every knot at its cumulative-length parameter
    to < 1e-15.
P4. **spline (Catmull-Rom) and BSpline.** Upstream's `Spline::position`
    iterates until |lambda - lambdaDist| <= rootSmall (1.5e-8) or 16
    iterations, so (a) collinear equally spaced knots give a straight line with
    straight-line-distance fraction = lambda to <= 1.5e-8 of the length;
    (b) a Catmull-Rom spline evaluated at lambda_k = |p_k - p_0| /
    (|p_k - p_0| + |p_k - p_end|) returns knot k to <= 1e-6 of the length
    (Catmull-Rom interpolates its knots; BSpline does not, and is only
    checked for the end points).
P5. **units.** `units [mm]` is read as 1e-3: a 1 x 1 x 1 cube in mm has
    volume 1e-9 m^3 (before: 1.0). `units` together with `convertToMeters` is
    an error (upstream FatalIOError). `[in]` -> 0.0254.
P6. **defaultPatch.** Absent: the unassigned-faces patch is named
    `defaultFaces` and has kind `Empty` (before: `Patch`). `defaultPatch {
    name x; type patch; }` gives name `x`, kind `Patch`. A `defaultPatch`
    without `type` is an error.
P7. **No silent skips.** `geometry`, `faces`, `namedVertices`, non-empty
    `mergePatchPairs`, `$var` and `#calc` return `NotImplemented`; `project`
    edges return `NotImplemented`.
P8. **T3A tutorial dict** (8 arcs, `defaultPatch { type empty; }`) builds a
    valid closed mesh, and each of its arcs' interior block-edge nodes lies on
    the circle through its three dictionary points (< 1e-12 relative).

## Results

(filled in after the run)
