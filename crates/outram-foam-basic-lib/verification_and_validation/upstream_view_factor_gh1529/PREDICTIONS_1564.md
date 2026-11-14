# Predictions for gh:#1564 (viewFactorsGen residual on non-convex / thin coplanar cases)

Written 2026-11-14 BEFORE any change to `view_factor_generation.rs`, after reading upstream
(ESI v2506) and running a source-level probe of `viewFactorsGen` on the lshape case.

## What the probe measured (before the fix)

`probe_1564/` holds diffs (printf additions only, no logic change, no upstream source copied) to
upstream's `viewFactorsGen.C`, `shootRays.H`, `searchingEngine.H`. The probe prints, at `%.17g`, the
coarse `Cf`/`Sf` of every face, every triangle given to the surface, `surfacesMesh`'s own triangles,
and for each of the 12,278 candidate rays the start, end, `findLine` hit flag, hit face (agglomeration)
and hit point. The ray lines are committed gzipped in `reference/probe_1564/lshape_rays.txt.gz`.

Against the Rust port (brute-force visibility), lshape:

| quantity | result |
|---|---|
| candidate rays, set of (i, j) | 12,278 = 12,278, identical sets |
| ray start and end points | bit-identical |
| coarse `Cf` (after nearest-snap) and `Sf` | bit-identical |
| wall triangles and their order (`surfacesMesh` does not reorder, 252/252) | bit-identical |
| first-trace nearest-hit face | **161 of 12,278 rays differ** |

So the candidacy test and the triangulation are NOT the cause (the caveat "coarse Sf = fine Sf" was
checked and holds). The difference is inside `findLine`.

## Hypothesis

`octree.findLine(start, end)` does not run the triangle test on the ray's own (start, end). Reading
`indexedOctree::findLine` / `traverseNode` (`indexedOctree.C`):

1. it walks leaf by leaf from the leaf containing `start`; inside a leaf it calls
   `fiOp(shape, start', nearestPoint, pt)` where `start'` is the point where the ray entered the leaf
   (pushed onto a leaf face by `pushPointIntoFace` / `pushPoint`, `perturbTol` = 10*SMALL scaled by the
   leaf span) and `nearestPoint` starts at the ray end and shrinks to each accepted hit;
2. a hit is accepted only if `octantBb.contains(pt)` (the hit lies in this leaf);
3. the walk stops at the first leaf that yields an accepted hit.

For a ray in the plane of a triangle (coplanar faces: the only differing pairs) the Moller-Trumbore
`det`, `u`, `v`, `t` are ~1e-17 noise and change with `dir` and `orig`. Brute force passes the full
(start, end) to every triangle and takes the smallest `t`; the octree uses another `dir`/`orig` per
leaf plus a leaf-membership filter, so the noise resolves differently. Axis-aligned convex cases have
no non-trivial in-plane rays, which is why they are byte-identical.

## Predicted effect of a faithful port of the octree build + `findLine` + `traverseNode`

(the existing `esi_octree` port of `outram-foam-mesh`, built for snappyHexMesh gh:#1467, moved to
`outram-foam-basic-lib` and extended with `findLine`)

| # | prediction | falsifier |
|---|---|---|
| 1 | for ALL 12,278 lshape rays (hit flag, hit face, hit point) equal the probe's bit for bit | any ray differs: hypothesis incomplete, find the next diverging step |
| 2 | lshape visible lists 126/126 rows (the 7 upstream-only pairs appear, the 1 Rust-only pair goes) | still 119/126 |
| 3 | lshape `F` bit-identical on all 10,886 pairs, three files byte-identical | any F differs |
| 4 | #1541 thin graded box: 250/250 rows; coupled step with Rust F falls from 1.7e-8 K / 2.5e-3 (qr) to the ESI-F level (~1e-11 K / ~1e-12) | rows still differ |
| 5 | cubeA, plate4/8/16, boxE stay byte-identical | any change |
| 6 | nothing is tuned: no tolerance, threshold or ordering changes; only the traversal and the arguments it hands the (unchanged) triangle test | n/a |

If 1 holds but 2-4 fail, the next suspect is the second pass (self-hit continuation).
