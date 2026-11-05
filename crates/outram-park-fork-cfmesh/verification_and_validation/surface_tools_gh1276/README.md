# cfMesh triangulated-surface tools: V&V record (gh:#1276, #1277, #1278)

Research, education and V&V only (see `RESPONSIBLE_USE.md`). AI-drafted,
not yet reviewed by a human.

**What was ported.** The `surface` module (`src/surface.rs`,
`src/surface/{features,checks,cleanup,curvature}.rs`) transcribes, from
DAFoam/cfmesh `e944f6ba695f`:

| Issue | Upstream | Rust |
|---|---|---|
| gh:#1276 | `triSurf` (patches, feature edges, subsets, addressing) | `mesh_octree::TriSurf` (extended, not a second type) |
| gh:#1276 | `triSurfaceDetectFeatureEdges`, `triSurfacePatchManipulator`, `triSurfaceClassifyEdges`, `triSurfacePartitioner`; utilities `surfaceFeatureEdges`, `patchesToSubsets`, `subsetToPatch` | `surface::features` |
| gh:#1277 | `triSurfaceChecks`, `help::doTrianglesIntersect`/`doTrianglesOverlap`; utility `checkSurfaceMesh` | `surface::checks` |
| gh:#1277 | `triSurfaceCleanupDuplicates`, `…DuplicateTriangles`, `triSurfaceRemoveFacets`, `triSurfaceCopyParts`, `triSurfaceImportSurfaceAsSubset`; utilities `removeSurfaceFacets`, `copySurfaceParts`, `importSurfaceAsSubset`, `mergeSurfacePatches` | `surface::cleanup` |
| gh:#1278 | `triSurfaceCurvatureEstimator`, `quadricFitting`, `simpleMatrix::LUsolve` | `surface::curvature` |

Not ported: file I/O (FMS, FTR, STL), `meshOctreeAutomaticRefinement`, the
2D tools. No pipeline calls the module; no default changed.

## Methodology

All tests are in `src/surface/tests.rs`, run with
`cargo test --release -p outram-park-fork-cfmesh --lib surface::tests`.
Every prediction was written in the test's doc comment and committed before
the first run (commits `cf4147e01` for gh:#1276/#1277, `58ce7b780` for
gh:#1278); results were then recorded beside them, misses included.

Surfaces: `shapes::cylinder_surface` (R 1, L 4, 48 segments, fan caps, 192
facets; "the pipe"), `shapes::box_surface` ([-1, 1]³, one region per face),
an L-shaped prism (6-gon extruded by 1, caps fanned from the reflex vertex),
`shapes::sphere_surface` (R 3, 24 × 48 and 48 × 96), and the defect-seeded
box (below).

**Differential (gh:#1276):** feature edges of the port against
`outram-foam-mesh`'s `snappy_hex_mesh::snapping::detect_feature_edges`
(made `pub` for this), same angle, compared as sets of endpoint pairs.

**Differential (gh:#1277):** `checkOrientation` against the sign of
`shapes::surface_volume`, and the regions against
`patches::SurfaceRegions::validate`.

**Analytic (gh:#1278):** per-point curvature on the sphere against −1/R
(the surface is wound outward, so the fitted quadric opens downward).

## Results (2026-11-05)

### gh:#1276: features and patching

| Check | Predicted | Measured |
|---|---|---|
| unlabelled pipe, 45°: feature edges | 96 (the rims), 0 open | 96, 0 |
| unlabelled pipe, 45°: patches | 3: 48 / 48 / ~~192~~ | 3: 48 / 48 / 96 (**miss**: my arithmetic, 192 is the whole surface) |
| unlabelled pipe, 5°: patches | 50 | 50 |
| feature edges vs `outram-foam-mesh`, pipe 45° / 5° | equal, 96 / 144 | equal, 96 / 144 |
| box 45° | equal, 12 | equal, 12 |
| box, facet 4 flipped | port 12, foam-mesh 13 | 12, 13 |
| pipe with the bottom cap removed | port 96, foam-mesh 48 | 96, 48 |
| partitioner, box | 8 corners, 12 edge groups, 24 group pairs | as predicted |
| partitioner, labelled pipe | 0 corners, 2 groups | as predicted |
| classify, box outward / inward | 12 convex, 6 flat, 0 concave; orientation 1 / 2 | as predicted |
| classify, L prism | 17 convex, 1 concave, 12 flat | as predicted |
| `patchesToSubsets` → `subsetToPatch` | identity | identity |
| `symmetryPlane` split | refused | refused |

The differential disagrees exactly where upstream's rule differs from
`outram-foam-mesh`'s: upstream negates the cosine for an inconsistently
wound facet pair, and stores open and non-manifold edges as feature edges.

### gh:#1277: checks and clean-up

The seeded box: point 8 duplicates point 6 and replaces it in the first
facet using it; facet 0 is duplicated; facet 4 is flipped; one stray
triangle at (5, 5, 5).

| Step | Predicted | Measured |
|---|---|---|
| open-edge entries | 7 | 7 |
| non-manifold entries | 9 | 9 |
| disconnected parts | 2 | 2 |
| collocated points | [6, 8] | [6, 8] |
| orientation groups | 5 | 5 |
| `cleanup_duplicates` (1e-6) | 1 facet, 1 point, 0 degenerate | 1, 1, 0 |
| open-edge entries after | 3 | 3 |
| orientation flip (the port's step, not upstream) | 2 facets | 2 |
| remove `region_1` | 12 facets, 8 points | 12, 8 |
| final checks | all clean | all clean |
| **volume** | **exactly 8** | **8.0** (exact) |

`checkSurfaceMesh` on the clean box and pipe: no failures. **Miss:** I
predicted that upstream's `doTrianglesOverlap` would report every coplanar
facet pair sharing an edge (12 of 12 box facets); it reports 0, and also
reports no overlap for two same-wound coplanar triangles that do overlap
(`triangle_pair_helpers`): upstream keeps the outside of each clipping edge
of a counter-clockwise t1. That prediction was revised before the first run
for the hand-made pair; for the box it was not.

Other tools: `cleanup_duplicate_triangles` removes a copy and a reversed
copy; removing the −X patch leaves 10 facets, 8 points, 4 open edges and 12
stored feature edges; copying one patch gives 2 facets, 4 points; importing
the re-triangulated +Z face marks facets 2 and 3; merging patches 0 and 1
gives 5 patches and moves a subset with its facet (all as predicted).

### gh:#1278: curvature

Sphere R 3, relative error `|k R + 1|` per point:

| Mesh | mean: max | mean: median | min: max | max: max |
|---|---:|---:|---:|---:|
| 24 × 48 | 0.66 % | 0.37 % | 1.20 % | 0.12 % |
| 48 × 96 | 0.16 % | 0.087 % | 0.29 % | 0.025 % |

Predicted: median below 1 % and 0.3 %, max below 3 % and 1 %, about 4×
per doubling. Measured 4.1–4.3×. Labelled pipe rims: 92 of 96 rim points give
`(π/24)/(2 sin(π/48)) = 1.000714/R`; the 4 where the point numbering wraps
give 23.016/R, as predicted from upstream's use of stored edge directions
(an upstream defect, ported as written; my hand value 22.98 was a slip).
`LUsolve` on a 5 × 5 system needing a row swap: exact to 1e-12.

## Upstream defects found

Ported as the intent where the text cannot run or discards its result; each
is documented where the port meets it (`src/surface.rs` lists them):
`subsetToPatch` discards its new patches and renumbers subsets with a patch
map; `checkForNonManifoldEdges(triSurf&)` runs the hole check;
`triSurfaceCleanupDuplicates` reads stale `pointFacets` and leaves feature
edges and point subsets on old labels; `triSurfaceRemoveFacets` reads
whichever addressing is cached; `triSurfaceCopyParts` and
`triSurfaceClassifyEdges` read index −1; `mergeSurfacePatches` copies
subsets by loop index; `calculateEdgeCurvature` uses stored edge directions.

## Images

Drawn from the assembled surfaces by
`cargo run --release -p outram-park-fork-cfmesh --example surface_tools_images`:

- `pipe_patches_45deg.png`, `pipe_patches_5deg.png`: the pipe split by
  `surfaceFeatureEdges`, coloured by patch, from above and below. Checked:
  3 colours at 45° (two caps, wall), the black rim lines on the cap
  boundaries only; at 5° every side quad its own colour.
- `box_seeded_defects.png`: the seeded box by defect (red duplicated facet
  on −Z, seen from below; yellow flipped facet, drawn darker because it
  faces away; magenta stray, here moved to (1.6, 1.6, 1.6) to stay in
  frame; orange facets on open edges, red lines the open edges). Checked:
  the four open edges meet at the duplicated corner (1, 1, 1).
- `box_cleaned.png`: after cleaning, six colours, no dark facet.
- `l_prism_edge_types.png`: the classified L prism; one red (concave)
  vertical edge at the reflex corner, blue rims and verticals, grey
  diagonals.

Not checked in the images: the facet subsets other than those drawn, and
the curvature (no image; the test table is the record).
