# polyDualMesh parity (gh:#1460: #1471, #1472, #1473)

Port: `src/poly_dual_mesh.rs` (default, surface-preserving) and
`src/poly_dual_mesh_upstream.rs` (new, explicit upstream mode). CLI:
`crates/outram-foam-cli/src/bin/poly_dual_mesh.rs`. Upstream read:
OpenFOAM-dev `852527377c7e87e33d51b19ca8974b5db60af28b` (sparse clone in the
gitignored `upstream_source/OpenFOAM-dev`) and OpenFOAM.com v2506
(`/home/user/upstream-build/OpenFOAM-v2506`).

## What was read before anything was written (upstream first)

`polyDualMesh.C` (`simpleMarkFeatures`, `main`), `meshDualiser.C` (all of
`setRefinement`, `createFacesAroundEdge`, `createFaceFromInternalFace`,
`createFacesAroundBoundaryPoint`), `edgeFaceCirculator`, `polyTopoChange`
(`compact`, `getFaceOrder`, `makeCells`), `primitiveMeshEdges.C`
(`calcEdges`: edges numbered upper-triangular by point pair),
`PrimitivePatchAddressing.C`, `PrimitivePatchCheck.C`, `HashTable.C`
(`set`, `resize`, `toc`), `Hash<label>` (identity), `faceTemplates.C`,
`primitiveMeshCellCentresAndVols.C`; v2506 `primitiveMeshTools.C`.
Findings that shaped the port, all checked in the source:

1. The utility puts a dual point at **every boundary-face centre**
   (`featureFaceSet` is seeded with all boundary faces, "B. face zones" adds
   more). Only edge midpoints are restricted to feature edges (plus, in
   `setRefinement`, every boundary edge at a feature point). So #1460's
   "deviation: a point at every boundary face centre" is not a deviation in
   faces; it is one in **edge midpoints** only.
2. Dual cell, point and face numbering come from `polyTopoChange` with
   `orderCells = false, orderPoints = false`: additions in order, internal
   faces re-ordered upper-triangular by dual cell, then patches. The feature
   sets come from `labelHashSet::toc()`: bucket order of an identity-hash,
   power-of-two table (so insertion order matters through collisions).
3. OpenFOAM-dev and v2506 use the same algorithm in `meshDualiser` and
   `simpleMarkFeatures`; v2506 additionally carries faceZone ids and fields.
4. The two lines' face-centre routines differ (gh:#1473): dev weights by area
   projected on the face normal; v2506 by `mag(n)`; triangle centres are
   `(1/3)*(p0+p1+p2)` in both; cell centres (`pc = 3/4 fc + 1/4 cEst`) are
   identical.

## Predictions (written before any run of the new code)

Hand-derived from the source, committed before the first test run.

| id | case | prediction |
|---|---|---|
| P1 | `FoamLabelHashSet`, size 4, insert 1, 5, 2, 6 | resizes to 8 on the 4th insert; `toc` = `[1, 2, 5, 6]` |
| P1b | size 2, insert 0, 2 | `toc` = `[0, 2]` after the resize; `[2, 0]` is the pre-resize chain order |
| P2 | 3x3x3 block (27 cells, 64 points, 6 patches), `featureAngle` 60, upstream mode | 64 dual cells; **197 dual points** (27 cell + 54 boundary-face + 32 feature points + 84 boundary-edge points: the 108 boundary edges minus the 24 whose two ends are both face-interior points); internal dual faces = 144 (one per primal edge); sum of dual volumes 27 (planar faces) to 1e-9; closure residual under 1e-12; Euler characteristic 2 in every cell; internal faces upper-triangular |
| P3 | same block, `splitAllFaces` | every edge and every face gets a dual point: 27 + 108 + 32 + 144 = **311 points**; internal faces = 324 (36 interior edges x 4 cells + 72 boundary non-seam edges x 2 + 36 seam edges x 1), several between the same two dual cells; volume 27 |
| P4 | L-shape (2x2x1 block minus one cell, one patch), angle 60 | without `concaveMultiCells`: 16 dual cells; with it: **20** (the two inner-corner points each become 3 cells); volume 3 in both; closure under 1e-12 |
| P5 | two cells in a row (2x1x1) with the middle face as a face zone | preserved: 12 + 4 = **16** dual cells (the 4 zone points become 2 each); `-doNotPreserveFaceZones`: 12; volume 2 |
| P6 | default `poly_dual_mesh_with` on any of the above | **byte-identical output before and after this change** (pins below) |
| P7 | v2506 `polyDualMesh 60` on blockMesh cavity (20x20x1), port in upstream mode with `Esi2506` centres | points / faces / owner / neighbour / boundary identical to the bit; if not, the first difference is recorded below |

Note on P2: an earlier draft of this table said 233 (it double-counted the seam edges); 197 is the figure committed before any run.

## Results


Dates: 2026-11-11. Toolchain Rust 1.99.1, `--release`. All runs below are
from the committed code; the primal meshes are `blockMesh` output of
OpenFOAM.com v2506 at `writePrecision 17`.

### Predictions P1 to P5 (in-tree tests, `tests/poly_dual_mesh_upstream.rs`)

All five hit on the first run: hash-set order `[1, 2, 5, 6]` and `[0, 2]`;
3x3x3 block 64 dual cells, **197** points, 144 internal faces, volume 27 to
1e-9, closure and Euler satisfied, upper-triangular internal faces;
`splitAllFaces` **311** points and **324** internal faces with several faces
between one cell pair; L-shape 16 cells plain and **20** with
`concaveMultiCells`; face zone 16 cells preserved, 12 without. (Nothing was
tuned: the numbers were derived from the source before the first run, see
the table above.)

### P7, the differential against OpenFOAM.com v2506 `polyDualMesh`

v2506's `polyDualMesh` application was built from
`applications/utilities/mesh/manipulation/polyDualMesh` with two lines
removed (`polyDualMeshApp.local.patch`: `processorMeshes.H` and its
`removeFiles` call, which need the unbuilt `reconstruct` library and only
act on parallel cases; the algorithm is untouched). Four primal cases, three
option variants each, `polyDualMesh 60 [-splitAllFaces | -concaveMultiCells]`;
recipe and scripts in `poly_dual_mesh_upstream_gh1460/`. Compared with
`examples/compare_polymesh.rs` (dual mode): `points` bit for bit, `faces`
vertex loops exactly, `owner`, `neighbour`, `boundary` exactly.

| case (primal cells) | variant | v2506 flavour: points bit-identical | faces identical | owner, neighbour, boundary |
|---|---|---|---|---|
| cavity 20x20x1 (400) | base | 1832 / 1832 | 3475 / 3475 | identical |
| cavity | `-splitAllFaces` | 4321 / 4321 | 6154 / 6154 | identical |
| cavity | `-concaveMultiCells` | 1832 / 1832 | 3475 / 3475 | identical |
| annulus, arc edges (240) | base | 924 / 924 | 1670 / 1670 | identical |
| annulus | split | 2288 / 2288 | 3478 / 3478 | identical |
| annulus | concave | 924 / 924 | 1670 / 1670 | identical |
| warped hex 6x5x4 (120) | base | 480 / 480 | 833 / 833 | identical |
| warped | split | 1133 / 1133 | 1750 / 1750 | identical |
| warped | concave | 480 / 480 | 833 / 833 | identical |
| L-shape, 3 blocks (96) | base | 522 / 522 | 830 / 830 | identical |
| L-shape | split | 1000 / 1000 | 1516 / 1516 | identical |
| L-shape | concave (531 points: the concave edge is live) | 531 / 531 | 857 / 857 | identical |

**12 of 12 cases are byte-for-byte identical in all five files** with
`UpstreamOptions::with_esi2506_flavour()` (v2506's face-centre formula and
hash-set sizing).

#### The same comparison with the OpenFOAM-dev flavour (the port's default)

Identical connectivity (faces, owner, neighbour, boundary) everywhere, and
the differences in `points` are exactly the two dev/v2506 differences the
source reading found, nothing else:

| case | points bit-identical (dev flavour vs v2506 run) | worst difference | cause |
|---|---|---|---|
| cavity | 1832 / 1832 (all variants) | 0 | planar axis-aligned faces: both formulas agree exactly |
| annulus | 665 / 924 | 8.9e-16 | face-centre formulas differ at round-off on curved faces |
| warped hex | 242 / 480 | **1.1e-6** (a real, warp-level difference) | dev's projected-area weights against v2506's `mag(n)` |
| L-shape base / concave | 362 / 522, 371 / 531 | index-level (the dual points are the same set, numbered differently) | dev's `labelHashSet(n)` capacity `n` against v2506's `reserve(n)` capacity `2n` wraps the feature-face labels: different `toc()` order, so different numbering |

~~The dev flavour was **not** run against an OpenFOAM-dev binary (none was
built; the dev source is only read).~~ **CORRECTED 2026-11-11 (gh:#1364): it was run; 12 of 12 case x variant duals are bit-identical to OpenFOAM-dev 852527377c7e `polyDualMesh`** (`upstream_dev_852527377c7e/README.md`, pinned in `tests/upstream_dev_852527377c7e.rs`). The paragraph below is the state before that run. Its two differences from v2506 are
therefore source-derived and checked here only through the v2506 binary
(the v2506 flavour matches it; the dev flavour moves exactly where the two
sources differ). ~~Not verified: that dev's own binary reproduces the dev
flavour bit for bit.~~ Verified 2026-11-11, see above.

#### Misses on the way (recorded, not hidden)

The first differential run (before the two fixes below) **missed**: cavity
base 1675 / 1832 points bit-identical, L-shape 362 / 522. owner, neighbour
and boundary were already identical. Both causes were found by reading
upstream, not by tuning:

1. **Edge numbering.** The port numbered mesh edges lexicographically.
   `primitiveMesh::reset` (both lines) checks the point order and, when the
   points are ordered (internal points first; trivially so when every point
   is on the boundary), `calcEdges` groups edges (internal-internal,
   internal-boundary, boundary-boundary, boundary-face edges last).
   A `printEdges` probe of v2506 gave `E 0 = (22, 463)` for the cavity, not
   `(0, 1)`. The port now reproduces `calcEdges` for both cases, pinned by
   `edge_numbering_follows_calc_edges_for_ordered_points` against that
   probe's values. Edge numbers set the `toc()` order of the feature-edge
   set.
2. **Feature-face hash set capacity** (dev `labelHashSet(n)`, v2506
   `reserve(n)` = capacity `2n`), above.

### Default surface-preserving mode against upstream mode (#1472 measure)

`examples/compare_dual_modes.rs`, feature angle 60, enclosed volume against
the primal's and `assess_quality`:

| case | mode | dual points | boundary faces | volume error vs primal | max / mean non-ortho | max skew |
|---|---|---|---|---|---|---|
| cavity | default | 3200 | 1050 | +4.9e-15 | 0.00 / 0.00 deg | 0.000 |
| cavity | upstream | 1832 | 1354 | +8.5e-15 | 0.00 / 0.00 deg | 0.000 |
| annulus (curved) | default | 1316 | 430 | **-4.91e-4** | 1.12 / 0.17 deg | 0.067 |
| annulus | upstream | 924 | 598 | **-3.27e-4** | 1.12 / 0.10 deg | 0.067 |
| warped hex | default | 620 | 214 | +1.10e-3 | 20.69 / 6.13 deg | 0.007 |
| warped | upstream | 480 | 310 | +2.86e-4 | 20.17 / 6.10 deg | 0.008 |
| L-shape | default | 646 | 244 | 0 | 8.75 / 0.18 deg | 0.308 |
| L-shape | upstream | 522 | 364 | 0 | 8.75 / 0.18 deg | 0.308 |

Interpretation, against what #1460 assumed: on **flat** patches the default
and upstream modes enclose the same volume (round-off), so the default's
extra edge midpoints buy nothing there. On **curved or warped** boundaries
the upstream mode's volume error is *smaller* in all three measured cases
(annulus -3.3e-4 against -4.9e-4; warped +2.9e-4 against +1.1e-3), the
opposite of what "the surface-preserving default reproduces the surface
better" suggests: the extra midpoints lie on the straight primal edges, and
the faces through them bulge differently from upstream's. Quality figures are
equal to the displayed precision except the mean non-orthogonality on the
annulus (0.17 against 0.10 deg). Three small meshes; this is a measurement,
not a verdict, and the default was **not** changed (maintainer decision).

### Pins (default mode must not move)

The default code path is untouched (only `///` and `//!` text in
`src/poly_dual_mesh.rs` changed, the stale doc claims of gh:#1471).
Run after the change: `cargo test --release -p outram-foam-mesh --test
poly_dual_mesh` (8 passed), cfMesh `dual_kinds_are_pinned_bit_for_bit` and
`foam_studio_default_mesh_keeps_its_fingerprint` with
`--features cell-centre-dual` (2 passed), `cargo test --release -p petir
--test no_platform_libm` (4 passed; it caught a `log10` in the CLI's
`time_name` on the first run, replaced by `{:e}` formatting). Foam Studio's
`headless_unstructured_default_is_pinned` (a `dhoby-ghaut` example target
with the egui/wgpu stack) was **not run** here: it reaches the default only
through `poly_dual_mesh_with`, which this change does not touch.

### Drawings (AGENTS.md: meshes are drawn)

`images/poly_dual_{lshape,annulus,cavity}_gh1460.png`
(`examples/draw_poly_dual_mesh.rs`): four panels of the lowest-z boundary
faces seen from +z: primal, default dual, upstream-mode dual, upstream mode
with `-concaveMultiCells`. Checked by eye on the L-shape: the dual cells
tile the L with no overlaps or gaps; the default has a point on every
boundary edge (the dense red dots along the outline) and upstream only at
seams and feature edges; the inner re-entrant corner is closed in all.
Not checked in the drawings: the interior (only the bottom plane is drawn),
and the split of the concave edge's two points into three cells (it lies
along z and is only visible as a different face count, 531 points, 204
cells).

### What is not done (maintainer decisions, left open)

- Default behaviour: unchanged (maintainer, gh:#1472).
- gh:#1473 unification: the five existing copies of the face/cell centre
  routine are untouched. This change adds a sixth, upstream-exact routine
  pair (`face_area_and_centre`, `cell_centres_and_volumes`, both lines) used
  only by the upstream mode; switching the five callers moves FV solver
  results at round-off level and is the maintainer's call. The new pair
  lives in `outram-foam-mesh` (not `outram-foam-basic-lib` as #1473 asks) so
  no other crate's API or review stamps move; relocating it is mechanical.
- The boundary-rule duplicate in cfMesh `dual.rs` and the
  `FeaturePoints` / `FeaturePointRule` twin enum: not touched (dependency
  decision).
