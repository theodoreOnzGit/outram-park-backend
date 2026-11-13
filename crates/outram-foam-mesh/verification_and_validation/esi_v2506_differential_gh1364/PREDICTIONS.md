# ESI v2506 differential: predictions (gh:#1364, sub-epics gh:#1458, gh:#1460)

Research, education and V&V only (`RESPONSIBLE_USE.md`). AI-produced, untrusted until
reviewed. **Written 2026-11-13 BEFORE any run of the new comparisons**; results go in
`README.md` beside this file with a verdict per row (held / MISS), never edited here.

Reference: OpenFOAM.com ESI **v2506** (`/home/user/upstream-build/OpenFOAM-v2506`,
rebuilt this session by `scripts/upstream/build-esi-v2506-and-genfoam.sh`, dummy
Pstream, `-O3`, `writePrecision 17`). Port flavour: ESI (`with_esi2506_flavour`,
`ProjectionFlavour::Esi2506`, `esi_*` check functions). Dev stays the default and is
not touched.

## Part A: polyDualMesh (gh:#1460)

The 2026-11-11 runs (before the container reset wiped the build) found the port's ESI
flavour byte-identical to ESI `polyDualMesh` on 12 of 12 case x variant duals. The
binary was lost with the reset.

| P | prediction |
|---|---|
| A1 | After rebuilding ESI `polyDualMesh` from a new script stage, the same 12 duals reproduce: 12 of 12 byte-identical in points, faces, owner, neighbour, patches. (Held on the build that existed; a rebuild of the same source must not change it.) |
| A2 | New primal meshes (the Part B dicts: multi-block graded, curved, 2D) give byte-identical duals in the ESI flavour: the dual is integer decisions plus centre arithmetic already bit-identical in the snappy work. A miss would be the first non-planar-centre or multi-block case where the port differs. |

## Part B: blockMesh features not yet differenced against a binary (gh:#1458)

Already compared against ESI v2506 bit for bit or to 1 ulp: cavity, T3A (arcs), pitz
(multi-block), five projection dicts. Open rows: `polyLine`, `spline`, `BSpline` edges,
`edgeGrading`, multi-section grading, several blocks sharing curved edges.

| P | dict | prediction |
|---|---|---|
| B1 | `bm_polyline`: two blocks, `polyLine` edges | topology identical; points 100 % bit-identical (linear interpolation along cumulative length, no libm call). |
| B2 | `bm_spline`: `spline` (Catmull-Rom) edges | topology identical; points bit-identical on >= 95 %, worst difference < 1e-14. Residual cause, if any: operation order in the spline basis. |
| B3 | `bm_bspline`: `BSpline` edges | topology identical; >= 90 % bit-identical, worst < 1e-13 (cubic B-spline evaluation order). |
| B4 | `bm_edgegrading`: 12 distinct edge ratios plus multi-section `((0.2 0.3 4)(0.6 0.4 1)(0.2 0.3 0.25))` | topology identical; points bit-identical on >= 99 %, worst < 1e-13. Cause if not: real `pow` of the ESI flavour against glibc `pow` (PETIR `pow`). |
| B5 | `bm_curved_graded`: three blocks, arcs plus grading on shared curved edges | topology identical; >= 90 % bit-identical (arc trig 1 ulp PETIR vs glibc, count pinned), worst < 1e-14. |
| B6 | `mergePatchPairs` | NOT predicted to work: the port returns `MeshError::NotImplemented` (`block_mesh.rs:1674`). The ESI run is made and the refusal recorded; porting `patchPointMerge`/`mergePolyMesh` is a deferral for the maintainer (gh:#1464). |

## Part C: checkMesh metrics (gh:#1364 row 6)

New instrument: an ESI probe application `dumpQuality` (C++, calls
`primitiveMeshTools::faceOrthogonality`, `faceSkewness`, `facePyramidVolume`,
`cellClosedness`, `faceFlatness`, `faceConcavity`, `cellDeterminant` on the case's
`polyMesh`, prints `%.17g`) plus ESI `checkMesh -allGeometry -allTopology` text for the
aggregate lines. The port side adds field-valued twins of those functions in
`check_mesh.rs` (`esi_*_field`), built on the ESI-matching geometry already in
`snappy_hex_mesh::esi_snap`.

| P | prediction |
|---|---|
| C1 | Face centres, face areas, cell centres, cell volumes (inputs to every metric): bit-identical on all test meshes (pinned by the snappy rounds on a flange mesh; hex and polyhedral meshes use the same code). |
| C2 | Face non-orthogonality (per-face `d.S/(|d||S|+rootVSmall)`): bit-identical on every internal face of every mesh, so the printed `Max` and `average` degrees agree (identical 6-digit text). |
| C3 | Face skewness, internal and boundary: bit-identical on hex meshes and duals. |
| C4 | Cell openness and aspect ratio (`cellClosedness`): bit-identical EXCEPT the `pow(v, 2/3)` term (glibc in ESI, PETIR in the port): 1 ulp on a minority of 3D cells, count pinned. 2D (cavity) meshes have no `pow` term and are bit-identical. |
| C5 | Face pyramid volumes (owner and neighbour): bit-identical. |
| C6 | `cellDeterminant` (checkMesh's own, normalised by mean face area, distinct from the snappy `checkCellDeterminant`): bit-identical, because the port copies the loop order. |
| C7 | Face flatness and concavity fields: bit-identical. |
| C8 | Aggregate `checkMesh` text lines (6 digits): identical on every mesh. |
| C9 | A port defect is NOT expected in Part C; the likeliest miss is `geometricD` (which directions are "empty"), which the port has no copy of today: any disagreement on the 2D cavity shows there first. |
