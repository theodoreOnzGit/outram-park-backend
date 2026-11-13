# ESI v2506 differential: blockMesh, polyDualMesh, checkMesh (gh:#1364)

Research, education and V&V only (`RESPONSIBLE_USE.md`). AI-produced, untrusted
until a human has reviewed it. Date of runs: 2026-11-13, release, x86_64.
Predictions (committed before any run): [`PREDICTIONS.md`](PREDICTIONS.md).
Sub-epics: gh:#1458 (blockMesh), gh:#1460 (polyDualMesh); snappyHexMesh is
gh:#1467 and is not touched here.

**Reference.** OpenFOAM.com ESI v2506 (`/home/user/upstream-build/OpenFOAM-v2506`,
dummy Pstream, `-O3`, `writePrecision 17`), rebuilt this session by
`scripts/upstream/build-esi-v2506-and-genfoam.sh`. New stage `polydual` builds
`polyDualMesh` (scratch copy, committed patch as before) and the probe
`dumpQuality` (this directory). Port flavour: ESI (`ProjectionFlavour::Esi`,
`UpstreamOptions::with_esi2506_flavour`, the `esi_*` check functions). Dev stays
the default; every ESI form below is an option beside the dev form.

## Results at a glance

- **blockMesh, ESI flavour: 7 of 7 new and old dicts have identical topology
  (faces, owner, neighbour, patches); points are bit-identical on 5** (`polyLine`,
  `spline`, `BSpline`, `edgeGrading` + multi-section grading, `pitzDailySteady`);
  the two arc dicts differ at 1 ulp in 21 of 459 and 215 of 54496 points, all
  from PETIR `cos`/`sin`/`acos` against glibc (cause proven by experiment).
- **Three port defects found and fixed** (all in the ESI form, dev untouched):
  the flavour was dropped for any dict without a `geometry` section; ESI's
  `spline`/`BSpline` use no distance re-parameterisation (worst 4.3e-2 off the
  binary before the fix); ESI's mid-point `arc` is a different construction
  from dev's (`cylindrical` coordinate system).
- **polyDualMesh, ESI flavour: 27 of 27 case x variant duals byte-identical**
  (points, faces, owner, neighbour, patches): the four older cases (cavity,
  annulus, warped, L-shape) and five new multi-block, curved or graded ones, each
  with the base, `-splitAllFaces` and `-concaveMultiCells` variants.
- **checkMesh metrics: every field bit-identical on 33 meshes** (11 primal, 18
  dual, 4 3D variants, plus T3A and pitzDailySteady): face centres and areas, cell
  centres and volumes, face orthogonality, skewness, owner and neighbour pyramid
  volumes, cell openness, aspect ratio, face flatness, concavity and cell
  determinant. The aggregate lines of `checkMesh -allGeometry` (17 digits)
  reproduce exactly.
- **mergePatchPairs: not ported, deferred** (see "Deferred for the maintainer").

## Part A: polyDualMesh

`polyDualMesh 60` (and `-splitAllFaces`, `-concaveMultiCells`) on the primal
`polyMesh` written by the ESI `blockMesh`; the port reads the same primal and
runs `poly_dual_mesh_upstream` in the ESI flavour; `compare_polymesh` (dual mode)
compares index by index. Result for every row: `points: bit-identical n of n`,
`faces: identical loops n of n`, `owner`, `neighbour` equal, `equal: true`.

| primal | dual points / faces (base) | split | concave |
|---|---|---|---|
| cavity | 1832 / 3475 | 4321 / 6154 | same as base |
| annulus (4 arcs) | 924 / 1670 | 2288 / 3478 | same |
| warped hex | 480 / 833 | 1133 / 1750 | same |
| L-shape (3 blocks) | 522 / 830 | 1000 / 1516 | 531 / 857 |
| `bm_polyline` | 647 / 1136 | 1555 / 2396 | same |
| `bm_spline` | 647 / 1136 | 1555 / 2396 | same |
| `bm_bspline` | 647 / 1136 | 1555 / 2396 | same |
| `bm_edgegrading` | 1452 / 2746 | 4045 / 6276 | same |
| `bm_curved_graded` | 996 / 1792 | 2446 / 3718 | same |

27 of 27 identical. Pinned in-tree by `tests/poly_dual_mesh_esi_v2506.rs` (12
cases whose primal this crate reproduces bit for bit: points hash of the binary's
output and a topology hash). The `bm_curved_graded` primal has 21 PETIR-trig
points, so its dual cannot be rebuilt from the port's own primal bit for bit; it
is compared with the binary's primal as input (`compare_polymesh`) and not pinned.
**A1 and A2 held.**

## Part B: blockMesh, ESI flavour against the binary

Dicts in `tests/fixtures/bm_*.dict` (generator: scratch; the files are the
record). All have `convertToMeters 1`, two or three blocks, default patch.

| dict | what it exercises | points identical | topology | note |
|---|---|---|---|---|
| `bm_polyline` | `polyLine` edges, 2 blocks | 288 / 288 | identical | B1 held |
| `bm_spline` | Catmull-Rom `spline`, shared curved edge | 288 / 288 (after fix; 121 before) | identical | B2: first run MISS, see defect 2 |
| `bm_bspline` | `BSpline` | 288 / 288 (138 before) | identical | B3: first run MISS |
| `bm_edgegrading` | `edgeGrading` with 12 ratios, two multi-section gradings, 3 blocks | 728 / 728 (388 before) | identical | B4: first run MISS, see defect 1 |
| `bm_curved_graded` | 8 `arc` edges, grading on shared curved edges | 438 / 459, worst 2.2e-16 (58 before, worst 2.9e-15) | identical | B5 held after defect 3 |
| `T3A` (tutorial, 8 arcs) | arcs, `defaultPatch` | 54281 / 54496, worst 1.1e-16 (49965, 1.1e-16 in dev form) | identical | |
| `pitzDailySteady` (`units` replaced by `convertToMeters 1`, ESI ignores `units`) | 5 blocks, grading | 25012 / 25012 (24797, worst 5.7e-14 before) | identical | |

**Defects (all fixed in the ESI form; the dev form and every existing pin are unchanged):**

1. **The flavour was dropped for dicts without a `geometry` section.**
   `BlockMeshDict::parse_with_flavour` started from `Geometry::default()` (dev),
   and the flavour reached `line_divide_weights` only through that struct, so
   ESI's real-`pow` `lineDivide` was never selected for ordinary dicts. This is
   why the 2026-11-11 table for pitzDailySteady against v2506 showed 24797 of
   25012. Fixed in `block_mesh.rs` (`Geometry { surfaces: vec![], flavour }`).
   Pinned by `esi_flavour_reaches_dicts_without_geometry`.
2. **ESI `spline` / `BSpline` have no distance iteration.** Dev's `Spline<Derived>`
   re-parameterises so that `lambda` is the straight-line distance fraction
   (16 iterations); v2506's `splineEdge::position` is
   `CatmullRomSpline::position(mu)` straight on the `polyLine` chord-length
   parameter (`localParameter`, then the segment polynomial). The port had only
   the dev form: worst 4.3e-2 and 4.4e-2 against the binary. New
   `EdgeCurve::CatmullRomEsi` / `BSplineEsi` (`block_edges.rs`), chosen when the
   flavour is ESI; operation order copied (and the end tests use v2506's
   `SMALL = 1e-15`, not dev's epsilon).
3. **ESI's mid-point `arc` is a different construction.** v2506 builds
   `cylindrical(centre, r1 ^ r3, r1)` and positions with
   `(R & (r cos t, r sin t, 0)) + origin`, with the rotation tensor columns
   `(ax2, ax1 ^ ax2, ax1)`; dev uses the angle-and-axis form. New
   `ArcEdgeEsi` (`through_point`, `position`) and `EdgeCurve::ArcEsi`. Effect on
   `bm_curved_graded`: 58 to 438 of 459 identical, worst 2.9e-15 to 2.2e-16.
   **Cause of the remaining 21 points, proven:** with `std` `cos`/`sin`/`acos` in
   `ArcEdgeEsi` instead of PETIR's, `bm_curved_graded` is **459 of 459**
   bit-identical (a throwaway experiment, reverted: this crate must not call the
   platform libm). The residual is the accepted PETIR-vs-glibc 1 ulp (under the
   1e-8 allowance, flagged for the maintainer): `bm_curved_graded` 21 points, T3A
   215 points, worst 2.2e-16 and 1.1e-16. Pinned exactly by
   `esi_block_mesh_arc_dicts_differ_only_at_pinned_trig_points` (the binary's bit
   patterns of the differing points are in `tests/fixtures/esi_trig_ulp_points.txt`,
   built by `esi_diff_points.py`; the port's points with those replaced hash to
   the binary's points hash).

The ESI `arc ... angle axis` and `origin` forms were not exercised (v2506 has
only the mid-point and `origin` forms; the dev `angle axis` form is dev-only and
unchanged). `origin` is not ported in either flavour; not covered here.

Pins: `tests/block_mesh_esi_v2506.rs` (points hashes of the binary, topology
hashes, the trig table). Images (the mesh the solver sees; red = boundary):
[`images/bm_polyline_esi.png`](images/bm_polyline_esi.png),
[`bm_spline_esi.png`](images/bm_spline_esi.png) and
[`bm_spline_dev.png`](images/bm_spline_dev.png) (the dev iteration, for contrast),
[`bm_bspline_esi.png`](images/bm_bspline_esi.png),
[`bm_edgegrading_esi.png`](images/bm_edgegrading_esi.png),
[`bm_curved_graded_esi.png`](images/bm_curved_graded_esi.png). **Checked in them:**
curved edges bend where the dict puts the knots (the spline and BSpline walls
are smooth through the interior knots), the shared face between blocks is
conforming, the grading clusters toward the inner radius and the ratio side in
`bm_curved_graded`, no cell is inverted. **Not checked:** the z extrusion (the
pictures are the lowest-z plane); the 3D checks are the numeric ones in Part C.

## Part C: checkMesh metrics

Instrument: `dumpQuality/dumpQuality.C` (calls `primitiveMeshTools`, prints
`%.17g`) and ESI `checkMesh -allGeometry -allTopology` (it prints 17 digits when
`writePrecision 17`). Port: new in `check_mesh.rs`, additive:
`esi_geometric_d`, `esi_quality_fields` (13 fields), `esi_check_summary`;
`examples/compare_check_mesh.rs` compares index by index. **Result: every value
bit-identical, 33 meshes**, e.g. T3A (26820 cells, 3D, aspect ratio 3232.46):
`faceCentres 323121/323121, ortho 53213/53213, skew 107707/107707, aspect
26820/26820, determinant 26820/26820`, and the 2D cavity (`meshD = (1 1 -1)`),
the warped hex (`meshD = (-1 -1 -1)` through default `empty` faces: aspect ratio
`-1`, reproduced), and polyhedral duals with concave cells.

| P | verdict |
|---|---|
| C1 geometry bit-identical | held |
| C2 non-orthogonality field, Max/average text | held |
| C3 skewness | held |
| C4 aspect ratio: 1-ulp differences through `pow(v, 2/3)` expected | **MISS in the good direction**: 0 differences (T3A 26820 cells, three 3D meshes) |
| C5 pyramid volumes | held |
| C6 `cellDeterminant` | held |
| C7 flatness, concavity | held |
| C8 aggregate checkMesh numbers | held (exact at 17 digits) |
| C9 `geometricD` the likeliest miss | held (cavity `(1 1 -1)`, warped `(-1 -1 -1)`, 3D variants `(1 1 1)`) |

Pins: `tests/check_mesh_esi_v2506.rs` (field hashes of the probe, the printed
aggregate numbers, four meshes committed in `tests/fixtures/esi_checkmesh/`).

### Ledger: the EXISTING dev-form `check_quality` against ESI's numbers

`compare_check_mesh` also prints the existing `check_quality` (`CheckOptions::default()`
and `foam_mesh_legacy()`) on the same ESI geometry. Not changed (reviewed
behaviour is closed to modification; each is a different statistic or definition,
now documented):

| quantity | dev-form value | ESI value | meaning |
|---|---|---|---|
| max non-orthogonality | warped 16.4314054628126094, T3A 43.8097410641263991 | 16.431405462812609, 43.809741064126406 | same up to 1 ulp (`to_degrees` vs `rad*180/pi`) |
| mean non-orthogonality | warped 5.4368 | average 6.3796 | **different statistic**: mean of angles vs `acos(mean of cosines)` |
| max skewness | internal only 8.39e-3 (warped) | 0.37332 | ESI takes the maximum over internal AND boundary faces; the dev-form `max_boundary_skewness` (default options) equals ESI's value, the `foam_mesh_legacy` mirrored boundary form is 0 |
| max aspect ratio, "cmpt" | T3A 74.12 | 3232.46 | ESI takes `max(maxCmpt/minCmpt, (1/6) sum/V^(2/3))`; the existing function is only the second term. ~~doc: "OpenFOAM's `checkClosedCells`"~~ **CORRECTED** in the doc comment of `cell_aspect_ratio_cmpt`. Dev's own form not re-checked (no dev source here) |
| min cell determinant | warped 0.8429 | 0.1023 | **different definition**: `n (x) n` weighted by area over the area sum (snappy's) vs `checkMesh`'s `cellDeterminant` (`sqr(S/avgArea)`, `det/8`) |

Use `esi_quality_fields` / `esi_check_summary` when the number must equal ESI's
`checkMesh`.

## Deferred for the maintainer

- **mergePatchPairs** (gh:#1464): still `MeshError::NotImplemented`. ESI
  `blockMesh` supports it (`blockMesh::calcMergeInfo` / `patchPointMerge` and
  `polyTopoChange` stitching). A bit-level port needs the stitching and
  point-numbering order of ESI's `mergePolyMesh`; not started (size: several
  hundred lines plus a binary differential on at least two dicts). Decision
  needed: port against ESI (dev has a different merge, `blockMeshMergeFast`) or
  leave refused.
- **PETIR trig 1 ulp** (21 and 215 points above): the standing maintainer call
  on accepting the PETIR-vs-glibc difference.
- **dev `checkClosedCells` aspect ratio**: not re-checked (no OpenFOAM-dev
  source in this checkout; the dev build was wiped). One run on a rebuilt dev
  tree would settle whether the existing `max_aspect_ratio_cmpt` is the dev form.
- **ESI `arc ... origin`**, `projectCurve`/`sweep` ESI forms beyond the existing
  projection pins, `units` handling: ESI silently ignores `units`, the port reads
  it; the pitz row above uses `convertToMeters 1` for both.
- Not touched: snappyHexMesh (gh:#1467), `checkMesh` topology checks (upper
  triangular order, zip-up, point usage) and `-allTopology` set outputs.

## Reproduce

```bash
scripts/upstream/build-esi-v2506-and-genfoam.sh polydual   # polyDualMesh + dumpQuality (~25 s on top of a v2506 build)
# generate dicts / cases: the dicts are tests/fixtures/bm_*.dict; mkcases.sh of
# ../poly_dual_mesh_upstream_gh1460 makes the four older primal cases
blockMesh; dumpQuality; checkMesh -allGeometry -allTopology      # in each case, via with-of.sh
PROJECT_FACE_FLAVOUR=esi compare_polymesh DICT CASE/constant/polyMesh    # blockMesh
compare_polymesh PRIMAL/constant/polyMesh DUAL/polyMesh 60 esi [splitAllFaces|concaveMultiCells]
compare_check_mesh CASE                                          # checkMesh fields
```
