# viewFactor surface radiation vs OpenFOAM ESI v2506 (gh:#1529)

Child of gh:#1379 (epic gh:#1376, upstream parity gh:#1336). Research, education and V&V only
(`RESPONSIBLE_USE.md`). Date of every number below: 2026-11-13, ESI OpenFOAM-v2506 built by
`scripts/upstream/build-esi-v2506-and-genfoam.sh` (partial dummy-Pstream build), Rust 1.99.1.

Predictions were committed before any run: `PREDICTIONS.md` (commit `a8750a4fc`; prediction 8
added in `d2c476cea` after the cube/plate runs and before the cylinder run, flagged there).

## What was built

* **Upstream build stage** `viewfactors` (new, in `scripts/upstream/build-esi-v2506-and-genfoam.sh`):
  `libpairPatchAgglomeration`, `faceAgglomerate`, `viewFactorsGen`. **CGAL is not needed and not
  installed**: `viewFactorsGen` has a `-DNO_CGAL` path (its own `searchingEngine.H`/`shootRays.H`,
  `distributedTriSurfaceMesh`), which upstream's own `Allwmake` selects when `have_cgal` fails. The
  newer `createViewFactors` (voxel ray engine, no CGAL either) is NOT built. The three pieces
  built in 24 s with 2 jobs. Note for the next reader: ESI v2506 `viewFactorsGen` is NOT ray
  Monte Carlo; it is deterministic quadrature (double area integral far, double line integral near),
  rays only decide visibility.
* **Rust port** `outram_foam_basic_lib::view_factor_radiation` (additive, opt-in): see its module
  table. The LU is the crate's existing `SquareMatrix::lu_decompose`/`lu_back_substitute`, called
  unchanged (it carries a review stamp context; nothing in it was edited).
* **Driver** `driver/vfDump.C` (calls upstream only) and `scripts/run.sh` (builds the cases with
  upstream `blockMesh`, `faceAgglomerate`, `viewFactorsGen`, then dumps).

### ~~Decision: `viewFactorsGen` is NOT ported (recorded, with line counts)~~ **SUPERSEDED 2026-11-14 (gh:#1540)**: the serial, un-agglomerated generator IS ported, see "viewFactorsGen port" at the end. Line counts below are for the whole family; `faceAgglomerate`, `pairPatchAgglomeration` and `singleCellFvMesh` remain unported.

| piece | lines |
|---|---|
| `viewFactorsGen.C` + `shootRays.H` + `searchingEngine.H` | 1,321 + 186 + 54 |
| `faceAgglomerate.C` | 234 |
| `pairPatchAgglomeration` (`.C`, `.H`, templates) | 771 + 264 + 77 |
| `singleCellFvMesh.C` | 651 |
| `viewFactor.C`/`.H` (ported here, direct branch only) | ~1,470 total, ~450 used |

About 3,800 lines on `distributedTriSurfaceMesh`, `triSurface`, `mapDistribute` and an octree
search, none of which exist in the crate. The model reads upstream's output files (ASCII, with
`writePrecision 17` so they round-trip exactly). Generating F in Rust is a separate issue (below).
Only the first-appearance ordering of `singleCellFvMesh`'s coarse faces was reproduced
(`coarse_patch`), because the model needs it to index the matrix.

## Differential vs ESI (every value bit for bit)

Seven closed cases, each with TWO `calculate()` calls (second after scaling every wall T by
1.1, which exercises the cached-LU path), `qr` compared on every boundary face:

| case | faces / coarse | options | qr call 1 | qr call 2 |
|---|---|---|---|---|
| cubeA | 96 / 96 | variable emissivity, qro 1000 W/m2 on zmax | 96/96 | 96/96 |
| cubeB | 96 / 24 (`nFacesInCoarsestLevel 4`) | smoothing, constantEmissivity | 96/96 | 96/96 |
| cubeC | 96 / 96 | cubeA with constantEmissivity | 96/96 | 96/96 |
| cubeD | 96 / 96 | cubeA with qro = 0 | 96/96 | 96/96 |
| plate4, plate8, plate16 | 160 / 160 | variable emissivity | 160/160 | 160/160 |

**1,344 of 1,344 values bit-identical, worst relative difference 0.** No residual to prove: the
LU order (Crout with scaled pivoting, `>=` tie rule) is the same code path. The parsed F,
`globalFaceFaces`, `finalAgglom` (real ASCII files of cubeB, committed under
`reference/generated_cubeB/`) equal upstream's in-memory values on all 24 rows. cubeA == cubeC
bit for bit (prediction 4: cached-LU and variable path agree). Pinned by
`tests/view_factor_vs_openfoam.rs`. Scope: wall T 500-1000 K, emissivity 0.5-1.0; not claimed beyond.

## Prediction scorecard

| # | prediction | result |
|---|---|---|
| 1 | F parse bit-exact | yes, 24/24 rows (cubeB files) and the dump rows |
| 2 | coarse averages bit-identical | yes (cubeB, 24 coarse faces, all qr match) |
| 3 | qr bit-identical | yes, 1,344/1,344 |
| 4 | const vs variable emissivity identical | yes |
| 5 | radiosity identity <1e-12 | yes, 3.6e-16. Energy balance sum(A q)/sum(A abs q): **1.9 %** on the unsmoothed cube (F row sums are 1.0099, F not reciprocal). The smoothed balance was NOT measured (`smoothing` destroys reciprocity, so the "1e-10 with smoothing" part of my prediction is unsupported; not scored) |
| 6 | cube F within 5 % | yes: opposite 0.200047 vs exact 0.19982 (+0.11 %), adjacent 0.202467 vs 0.200045 (+1.2 %), row sum 1.0099 (+1.0 %) |
| 7 | plates, from below, <15 % at L/d=8 | **direction wrong**, magnitude right: see below |
| 8 | cylinders <2 % at N=32, shrinking | yes, far better: see below |

## Analytic checks

**Unit cube** (Modest, Radiative Heat Transfer, 3rd ed., catalogue; exact via Howell C-11
`parallel_squares_f(1,1)` = 0.199825): numbers in row 6 above. These are the mesh-dependent
`viewFactorsGen` errors for a 4x4 face grid (~~Gauss tolerance 0.1~~ **CORRECTED 2026-11-14** the CODE default is `GaussQuadTol 0.01` (`viewFactorsGen.C`: `getOrDefault<scalar>("GaussQuadTol", 0.01)`); the 0.1 is only in upstream's header comment; these runs used the 0.01 default): +0.11 % opposite,
+1.2 % adjacent, +1.0 % row sum.

**Parallel plates** (1 m square plates at distance d in a closed box, four black sides at 0 K,
8x8 plate faces, hot plate 1000 K eps 0.8, cold 500 K eps 0.5; infinite-plate value
`sigma(T1^4-T2^4)/(1/e1+1/e2-1)` = 23,626.7 W/m2 with ESI's sigma). Centre 2x2 faces:

| L/d | F12 ESI | F12 exact (Howell C-11) | F12 error | hot centre / inf | cold centre / inf |
|---|---|---|---|---|---|
| 4 | 0.630384 | 0.632036 | -0.26 % | 1.379 | 0.761 |
| 8 | 0.789542 | 0.789953 | -0.05 % | 1.130 | 0.924 |
| 16 | 0.886727 | 0.886715 | +0.001 % | 1.033 | 0.980 |

The hot plate also loses heat to the cold sides, so its flux is above the infinite-plate value
and the cold plate's is below; both converge to it as d/L falls (edge effect, not a numerical
error: the F error is 0.26 % or less). Plate-average fluxes are further away (1.55, 1.34, 1.20)
because edge faces see the sides; those are not the infinite-plate quantity and are not gated.

**Concentric cylinders** (`tests/view_factor_concentric_cylinders.rs`; Siegel & Howell; Modest
Eq. 5.28; inner 0.5 m, 1000 K, eps 0.8; outer 1.0 m, 500 K, eps 0.5; inscribed N-gon, 2-D
midpoint quadrature M=40, radii from the polygon perimeters). This test needs no ESI; it checks
the ported solve in a configuration with concave facets:

| N | q model (W/m2) | q exact | error | balance | worst F row sum error |
|---|---|---|---|---|---|
| 16 | 30373.61 | 30377.19 | -0.012 % | 2.0e-4 | 3.1e-4 |
| 32 | 30378.45 | 30377.19 | +0.004 % | -7.3e-5 | 1.2e-4 |
| 64 | 30377.19 | 30377.19 | -0.0000 % | -3.3e-7 | 1.3e-6 |

Caveat: using the polygon's own perimeter radii removes the polygon-vs-circle error from the
reference by construction, so this isolates quadrature plus the solve, not the discretisation of
the circle. A first version of the test produced a 32 % error; the cause was a rounding-noise
visibility hit of a point on a facet against its own polygon (rays from inner facets were being
tested against the inner polygon). Fixed by testing only outer-to-outer rays; the number moved
from 33 % to 0.012 % at N=16.

## Defects and notes found

* The prediction text for plate exchange had the wrong direction (above); kept, not edited.
* ESI quirk reproduced and documented: `constantEmissivity true` freezes the LU (and so the
  emissivities) at the first `calculate()`.
* ESI quirk reproduced: `T4ave[coarseI]` is filled from `coarseToFine[coarsePatchFace[coarseI]]`,
  so coarse numbering is first-appearance order, not label order. Identity when faces are already
  in label order (cubeB is not; it passes).
* `viewFactorsDict` must not hold a bare `".*"` entry: `viewFactorsGen` reads top-level keys such
  as `GaussQuadTol` with `getOrDefault`, and a wildcard patch dictionary matches them and aborts.
  The cases use `"(xmin|xmax|...)"`.
* blockMesh does not emit `inGroups`: the case builder edits the `boundary` file to put the walls
  in `viewFactorWall`.

## Not done / follow-ups

* ~~Port of `viewFactorsGen` + `faceAgglomerate` + `singleCellFvMesh` (3,800 lines): follow-up issue.~~ **PARTLY DONE 2026-11-14 (gh:#1540)**: `viewFactorsGen` ported (serial, 3-D, no agglomeration); `faceAgglomerate` + `pairPatchAgglomeration` + `singleCellFvMesh`, the 2-D branch and parallel remain.
* Iterative `useDirectSolver false` branch, `nBands > 1`, `solarLoad`, parallel `mapDistribute`.
* ~~The appbuilder coupling (`with_view_factor_radiation` beside the P1 option) is NOT done: it
  needs a cavity-surface-to-patch mapping and a per-face emissivity source, which is not small.~~ **CORRECTED 2026-11-14 (gh:#1541)**: `EnergyEquation::with_view_factor_radiation` exists (`outram_foam_appbuilder_lib::solvers::energy_view_factor`), per-patch emissivity and `qro`, F from the Rust generator; coupled differential in `outram-foam-appbuilder-lib/verification_and_validation/upstream_view_factor_gh1541/`. Still not done: the htr10_rz cavity itself (needs a cavity mapping and a 2-D/axisymmetric generator).
* The 2-D `htr10_rz` cavity test the issue is for (RPV to RCCS) was not run: needs the generator
  (or ESI's `viewFactorsGen` on the HTR-10 cavity mesh) and the coupling.
* Geometry: `images/geometry.svg`, drawn by `scripts/draw_geometry.py` from the upstream
  polyMesh (514 polygons = 96 + 96 + 160 + 160 + 2 cylinders; count checked, XML valid).
  ~~**I could not render or look at the image in this environment** (no SVG rasteriser), so it is
  unreviewed; the patch/face counts are the only check made.~~ **CORRECTED 2026-11-13**: rendered
  with the pre-installed Chromium (Playwright) to `images/geometry.png`. The first render showed the
  cubes clipped above the top edge (the SVG viewBox started at y = 0 while the oblique projection
  draws above it); the viewBox was widened to `0 -110 1250 700` and nothing else changed. Checked by
  looking at the PNG: cubeA/cubeB show 4x4 faces on each of 6 patches (96 boundary faces), plate4 and
  plate16 show 8x8x1 cells with the expected thickness ratio (0.25 vs 0.0625 m), and the cylinders
  show 16 facets at radii 0.5 and 1.0 m. Not checkable from the picture: the 24-face agglomeration of
  cubeB (drawn fine, as the label says). Known cosmetic defect: the cube and plate labels overlap the
  drawings and each other.
* `docs/outram-foam-basic-lib-api.md` mirror not regenerated (`kovan-cli api-docs` not run).

## Reproduce

`scripts/upstream/build-esi-v2506-and-genfoam.sh viewfactors` then
`nice -n 10 crates/outram-foam-basic-lib/verification_and_validation/upstream_view_factor_gh1529/scripts/run.sh`;
then `cargo test --release -p outram-foam-basic-lib --test view_factor_vs_openfoam --test view_factor_concentric_cylinders`.

## viewFactorsGen port (gh:#1540), 2026-11-14

Module `outram_foam_basic_lib::view_factor_generation`; test `tests/view_factor_generation_vs_openfoam.rs`;
predictions `PREDICTIONS_viewFactorsGen.md` (commit `f6cdb522f`, before any comparison). Upstream: ESI
v2506 `viewFactorsGen` built `-DNO_CGAL` (octree ray engine), serial, no `finalAgglom`, code defaults
(`GaussQuadTol 0.01`, `distTol 8`, `alpha 0.21`), `writePrecision 17`. Reference = the files upstream
wrote (`F`, `globalFaceFaces`, `mapDist`) plus its `polyMesh`, committed gzipped in `reference/vfgen/`.
Rust regenerates them from the mesh alone. Rust 1.99.1, glibc 2.39, CPU with FMA.

| case | faces | pairs (upstream) | visible lists equal | F entries bit-identical | F / globalFaceFaces / mapDist bytes |
|---|---|---|---|---|---|
| cubeA 4x4x4 unit cube | 96 | 7,680 | 96/96 | 7,680/7,680 | identical / identical / identical |
| plate4, plate8, plate16 (8x8x1 plates, closed box; 2AI far pairs) | 160 each | 17,152 each | 160/160 each | 17,152/17,152 each | identical x3 |
| boxE 2x1x0.5, 5x3x2 (non-square faces) | 62 | 3,132 | 62/62 | 3,132/3,132 | identical x3 |
| lshape (L enclosure, 3 blocks of 3x3x3, rays blocked at the re-entrant corner) | 126 | 10,886 | ~~119/126~~ **126/126** (CORRECTED 2026-11-14, gh:#1564) | ~~10,879/10,879 of the pairs both list~~ **10,886/10,886** | ~~`mapDist` identical; F and globalFaceFaces differ (7 upstream-only pairs, 1 Rust-only)~~ **identical x3** |

Worst relative difference of any common `F` entry: **0**. The `log` of `petir::mathf::RealMath::r_ln` (ARM
optimized-routines port) agreed with glibc's FMA-ifunc `log` on every argument in these runs
(prediction 3's feared 1-ulp residual did not occur; not a proof for other arguments).

Prediction scorecard (`PREDICTIONS_viewFactorsGen.md`):

| # | prediction | result |
|---|---|---|
| 1 | visible lists identical, 100 % of rows | **wrong first time**: boxE 60/62 rows. Cause (found by reading upstream, not by tuning): the candidate rays use the coarse-face centre `uindirectPrimitivePatch::faceCentres()` = `face::centre`, which differs in the last bit from the mesh's `makeFaceCentresAndAreas` centre; faces on the same plane then pass or fail `(d & fA) < 0` on rounding noise. After using `face::centre`: 100 % on all convex cases. lshape: see below |
| 2 | 2AI bit-identical | yes (plates) |
| 3 | >= 99.5 % F bit-identical, 60 % on exactly 100 % | yes, exactly 100 % |
| 4 | quadrature order same | implied by 3 |
| 5 | files byte-identical | yes on all convex cases, after fixing my row separator (each row ends with a blank line) |
| 6 | reciprocity within 2 % | not measured |
| 7 | boxE as the others | yes, after the face-centre fix |

~~**lshape residual (not fixed, reported).** [text kept in the git history: 119/126 rows, 7 upstream-only pairs, 1 Rust-only, "the octree traversal ... is not ported: brute force replaces it"]~~ **FIXED 2026-11-14 (gh:#1564)**, see the section below. The original diagnosis that the differing pairs are "rounding-noise" candidacy was wrong twice: the candidacy test is bit-identical to upstream (probe, below), and upstream's (106, 108) carries F = -1.3878e-4, not noise (gh:#1541).

Caveats: (1) the coarse-face `Sf` is taken as the fine `Sf` (true for un-agglomerated faces: the
`edgeLoops` of a single face keep the face's vertex order, checked in `PrimitivePatchEdgeLoops.C`; **re-verified 2026-11-14 by the gh:#1564 probe: 126/126 `Cf` and `Sf` bit-identical on lshape**);
(2) ~~the octree search is replaced by brute force over all triangles~~ **CORRECTED 2026-11-14 (gh:#1564)** the octree search is ported (`esi_octree`); (3) 2-D (`nSolutionD == 2`),
agglomeration, parallel, `dumpRays`, `viewFactorField` are not ported; (4) geometry images for boxE and
lshape were NOT drawn (AGENTS.md rule): the meshes are `inputs/blockMeshDict`, `inputs/blockMeshDictL`;
(5) `docs/outram-foam-basic-lib-api.md` not regenerated.

## gh:#1564: the octree `findLine`, root cause of the lshape / thin-box residual, 2026-11-14

Predictions: `PREDICTIONS_1564.md` (commit `2cc5f3615`, before any change to the generator).

**Method.** Upstream was read first (`viewFactorsGen.C`, `shootRays.H`, `searchingEngine.H`,
`distributedTriSurfaceMesh::findLine`, `triSurfaceSearch`, `indexedOctree::findLine/traverseNode/
walkToNeighbour/pushPointIntoFace`, `treeBoundBox::intersects`, `treeDataPrimitivePatch::findIntersection`,
`triangle::intersection`). Then an instrumented copy of the real `viewFactorsGen` (`probe_1564/`: the
diffs, printf only) was run on lshape and compared with the Rust values in IEEE bits.

| quantity (lshape, 126 faces) | Rust brute force vs upstream |
|---|---|
| candidate rays (12,278), start, end | identical sets, bit-identical |
| coarse `Cf` (nearest snap) and `Sf` | bit-identical (so candidacy `(d & fA) < 0` is NOT the cause) |
| the 252 wall triangles and their order (`surfacesMesh` does not reorder) | bit-identical |
| first-trace `findLine` hit | **161 of 12,278 rays differ** |

**Root cause.** `indexedOctree::findLine` does not test a ray's own (start, end) against every
triangle (`indexedOctree.C`, `traverseNode`, the `findAny == false` branch). It walks the leaves in ray
order; in each leaf it calls `fiOp(shape, start, nearestPoint, pt)` where `start` is the point the ray
entered the leaf at (`pushPointIntoFace`, shifted by `perturbTol = 10*SMALL` times the leaf span) and
`nearestPoint` is the end, shrunk to each hit found so far; a hit counts only if
`octantBb.contains(pt)`; the walk stops at the first leaf with a hit. For a ray in the plane of a
triangle the Moller-Trumbore `det`, `u`, `v`, `t` are 1e-17 noise and change with `dir`/`orig`, so
brute force (full segment, smallest `t`) resolves them differently. That is also why every convex,
axis-aligned case was already byte-identical.

**Fix (matching upstream, nothing tuned).** The workspace already held a translation of
`indexedOctree` (build, `findNearest`) in `outram-foam-mesh` for snappyHexMesh (gh:#1467). It was
moved to `outram_foam_basic_lib::esi_octree` (re-exported from `outram-foam-mesh`; the 34 `esi_*` unit
tests and `snappy_hex_mesh_esi_feature` / `_flange` pass unchanged) and extended with `find_line`,
`traverse_node`, `walk_to_neighbour`, `push_point*`, `treeBoundBox::intersects` with the face bits.
`view_factor_generation` builds the tree as `triSurfaceSearch::tree()` does (depth 10, leaf 10,
duplicity 3, `Random(65431)` inflate (1e-4, ROOTVSMALL)) and hands each ray to `find_line`. The
triangle test, `planarTol`, the self-hit continuation and the candidate rule are unchanged.

**Results (re-measured 2026-11-14, Rust 1.99.1).**

| case | before (brute force) | after (octree) |
|---|---|---|
| cubeA, plate4/8/16, boxE | byte-identical | byte-identical (unchanged) |
| lshape visible lists | 119/126 rows (7 upstream-only, 1 Rust-only pair) | **126/126** |
| lshape F entries | 10,879 of 10,886 pairs listed by both, bit-identical | **10,886/10,886 bit-identical**, worst 0 |
| lshape F / globalFaceFaces / mapDist bytes | differ / differ / identical | **identical x3** |
| lshape first-trace hits vs the probe (flag, face, hit point, bits) | 161/12,278 differ | **0/12,278 differ** |
| lshape chain qr (`view_factor_chain_vs_openfoam`) | 0/126 bit-identical, worst 2.09e-5 | **126/126, worst 0** (both calls) |

Prediction scorecard (`PREDICTIONS_1564.md`): 1 (all rays equal) yes; 2 (126/126) yes; 3 (F and files
identical) yes; 4 (thin box) see the appbuilder record `upstream_view_factor_gh1541/README.md`;
5 (convex unchanged) yes; 6 (nothing tuned) yes.

Pinned by `tests/view_factor_generation_vs_openfoam.rs`
(`lshape_non_convex_case_is_bit_and_byte_identical_to_viewfactorsgen`,
`lshape_every_candidate_ray_first_hit_equals_upstream_find_line` against
`reference/probe_1564/lshape_rays.txt.gz`) and `tests/view_factor_chain_vs_openfoam.rs`.
Caveats: serial (the parallel `distributedTriSurfaceMesh` path is not ported); `findLineAny`,
`findLineAll` not ported; the octree `findLine` port is verified bit-for-bit on lshape (12,278 rays) and
on the thin box through F, not proven for arbitrary geometry.


## Bookkeeping status

| axis | status |
|---|---|
| Implementation | [x] direct-solver path ported; [x] serial un-agglomerated generator (gh:#1540); [ ] agglomeration, 2-D, parallel, iterative path, appbuilder coupling |
| Review | [ ] none (AI-assisted draft, untrusted until a human reviews it) |

## viewFactor chain: Rust generator -> Rust solve -> qr vs ESI (gh:#1541), 2026-11-14

Test `tests/view_factor_chain_vs_openfoam.rs`; predictions `PREDICTIONS_chain.md` (commit `29b51d01d`, before
the first run). Start from the upstream polyMesh only; `generate_view_factors` makes F / globalFaceFaces,
`|Sf|` is the Rust face area (`face_centre_and_area(..).mag()`), `ViewFactorModel::solve` (direct LU) runs
twice (T, then T x 1.1) exactly as `vfDump.C`; compared with the upstream `qr1` / `qr2` bit for bit. T,
emissivity and qro are the case inputs (read from the upstream dump, where they were set; not tuned).
Rust 1.99.1, release.

| case | faces | options | qr call 1 | qr call 2 | worst rel. diff |
|---|---|---|---|---|---|
| cubeA | 96 | variable e, qro 1000 on zmax | 96/96 | 96/96 | 0 |
| cubeC (cubeA mesh) | 96 | constantEmissivity | 96/96 | 96/96 | 0 |
| cubeD (cubeA mesh) | 96 | qro 0 | 96/96 | 96/96 | 0 |
| plate4, plate8, plate16 | 160 each | variable e | 160/160 | 160/160 | 0 |
| boxE | 62 | non-square faces | 62/62 | 62/62 | 0 |
| lshape (non-convex) | 126 | | ~~**0/126**~~ **126/126** | ~~**0/126**~~ **126/126** | ~~**2.0902e-5**~~ **0** (CORRECTED 2026-11-14, gh:#1564: octree `findLine` port; re-measured) |

**1,660 of 1,660 convex-case values bit-identical** (prediction 1 held; the area risk did not occur: the Rust
`|Sf|` is bit-identical to `mesh.magSf()`). Not run in this chain: the agglomerated cubeB (needs
`faceAgglomerate`, not ported).

Scorecard: prediction 1 (convex 100 %) **held**; prediction 3 (no glue defect) held. Prediction 2 (lshape: not
all identical AND worst < 1e-10) **half right, half wrong**: not identical, yes (0/126), but the worst
difference is **2.09e-5, not < 1e-10**, because the "noise-level" assumption about the 8 differing F pairs
was wrong (the (106,108) entry is -1.39e-4, see the correction above). The lshape test therefore pins the
known residual (0 identical; worst between 1e-6 and 1e-4) instead of gating at 1e-8; ~~it fails when gh:#1564
is fixed~~ **CORRECTED 2026-11-14 (gh:#1564)**: it did fail when the generator was fixed, and is now the bit-identity gate
`lshape_chain_is_bit_identical_to_esi_qr` (126/126, worst 0, both calls). No tolerance was relaxed.

Scope: fixed wall T (500-1000 K), emissivity 0.5-1.0, closed convex enclosures; no coupling to a conduction
solve in this slice.
