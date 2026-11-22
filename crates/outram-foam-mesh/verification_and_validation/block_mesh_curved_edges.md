# blockMesh curved edges, units and defaultPatch: V&V record (gh:#1461, gh:#1463, epic gh:#1458)

Upstream: OpenFOAM-dev `852527377c7e87e33d51b19ca8974b5db60af28b`
(`crates/outram-foam-mesh/upstream_source/README.md`). Code:
`src/block_edges.rs`, `src/block_mesh.rs`. Tests: `tests/block_mesh_curved_edges.rs`.

## Status of the upstream-run differential

~~No upstream `blockMesh` binary exists on this machine
(`find /home/user/upstream-build -name blockMesh -type f` returned nothing on
2026-11-11; that tree has cfMesh's libraries only). So the `points`/`faces`
comparison against a real `blockMesh` run on T3A, sphere and pitzDailySteady is
**not done** and stays a remaining item on gh:#1364.~~ **CORRECTED 2026-11-11
(gh:#1462):** an OpenFOAM.com v2506 `blockMesh` binary exists at
`/home/user/upstream-build/OpenFOAM-v2506/platforms/*/bin/blockMesh` (built later
the same day). T3A has now been run on it (`writePrecision 17`) and compared with
`examples/compare_polymesh`: 54496 points on both sides, faces, owner,
neighbour and patches identical, **49965 of 54496 points bit-identical, worst
difference 1.1e-16 (one ulp)**, the PETIR `cos`/`sin`/`tan`/`acos` against glibc
as predicted below. pitzDailySteady (needs `units [mm]`, unknown to v2506) and
the `sphere*` tutorials (need `#eval`) were not run. Besides that, each port is
checked against upstream's own analytic definitions (below).

## Predictions, written before the first run (2026-11-11)

P1. **Quarter-annulus block, two `arc` edges (through-point form).** Vertices
    (1,0,0) (2,0,0) (0,2,0) (0,1,0), arcs 1-2 through (2cos45, 2sin45) and 0-3
    through (cos45, sin45), cells (4, n, 1), uniform grading. Upstream's
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

Measured 2026-11-11, `cargo test --release -p outram-foam-mesh --test
block_mesh_curved_edges`: 13 pass. Every prediction held at the stated
tolerance, with one wrong-test episode recorded below.

| P | measured | verdict |
|---|---|---|
| P1 | n = 8: worst radius error 4.4e-16, worst angle error 3.5e-16 rad; volume 2.3410838641935396 against closed form 2.341083864193539, relative 1.9e-16. Without `edges`: 1.5 (the old output) | held |
| P2 | circle radius error < 1e-15, helix z linear < 1e-15, end point exact to < 1e-15 | held |
| P3 | polyLine knots hit to < 1e-15 | held |
| P4 | collinear spline/BSpline within 1.5e-8 x length; Catmull-Rom knots within 1e-6 x length; BSpline end points exact | held |
| P5 | `units [mm]` cube volume 1e-9 (relative 1e-12); `[in]` = 25.4 * 1e-3; `units` + `convertToMeters` error; `[kg]` NotImplemented | held |
| P6 | default `defaultFaces`/`Empty`; `defaultPatch { name rest; type patch; }` -> `rest`/`Patch`; missing `type` is an error | held |
| P7 | all listed keywords error, **except `$name` of an earlier top-level entry, which is now expanded** (see deviation) | held, scope extended |
| P8 | T3A: 8 arcs parsed, `defaultPatch` empty, 54568 points / 26820 cells, valid `FvMesh`, 20 nodes on arc 1-3 on its circle | held |

**Wrong-test episode.** The first P1 run failed (volume 1.915 against 2.341).
The mesh was not at fault: the test dict listed arcs on the bottom face only,
so the top face was straight-sided and the prism was skewed. Listing the top
arcs (as T3A does for both z levels) gave the 1e-16 agreement above. The
prediction and the tolerance were not changed.

**Deviation from the predictions (scope).** `pitzDailySteady` cannot be read
without `$posY`/`$negY`, so a simple expansion of `$name` to an earlier
top-level `name value;` entry was added (`expand_variables`). `#calc`,
`#include`, `${x}` and `$:a.b` still return `NotImplemented`. Measured:
`pitzDailySteady` (`units [mm]`) builds 25012 points and 12225 cells, which
are the counts OpenFOAM's tutorial is known to give (quoted from memory of
upstream's `checkMesh` log, **not re-run here**: no upstream binary), maximum
x = 0.29 m (before this change the dict was read with scale 1 and the macros
skipped, giving 290 m).

## What this does and does not show

- The curve functions follow upstream line by line (operation order kept, so
  the same last-ulp behaviour is intended), but **bit-identity to an upstream
  run is not demonstrated**: it needs an upstream `blockMesh` binary. When one
  exists, compare `points` and `faces` on T3A, `mesh/blockMesh/sphere*` (needs
  `#calc`, not done) and pitzDailySteady; expected differences are PETIR
  `cos`/`sin`/`tan`/`acos` against glibc, a few ulp (gh:#1455 rule).
- `lineDivide`'s `expansionRatio == 1.0` test is `|r - 1| < 1e-12` in the
  existing `graded_positions_multi`, upstream's is exact; they differ only for
  ratios within 1e-12 of 1.
- Straight-edge blocks still use the older trilinear fast path (gh:#1458 notes
  it is not upstream's 12-edge blend bit for bit); only blocks that have a
  listed edge use the full blend with the curved-edge correction.
- Not ported here: block faces / projection (gh:#1462), `cellZones`,
  `mergePatchPairs`, named vertices/blocks, `#calc` (gh:#1463, gh:#1464).

## Drawings

`images/t3a_before_straight_zoom.png` and `images/t3a_after_curved_zoom.png`:
the T3A leading-edge region (x 0..0.09 m, y 0..0.03 m), z-min plane, cell edges
grey, boundary edges coloured by patch, drawn from the built `PolyMesh` by
`examples/draw_block_mesh.rs`. Before: the three blocks around the leading
edge are bounded by straight chords. After: the plate and the block interfaces
follow the 8 arcs.

Checked in the images: before, the leading-edge corner is a chamfered
polygon and the fan of cells around it has kinked rows; after, the leading
edge is a smooth rounded nose, the fan rows are smooth arcs that meet the
plate (coloured edge, bottom right) tangentially, and no cell is folded or
crossed. Not checked: cell quality below a pixel (only that `to_fv_mesh`
accepts the mesh), the other two-thirds of the domain (straight blocks,
unchanged), and the z structure (one cell thick).
