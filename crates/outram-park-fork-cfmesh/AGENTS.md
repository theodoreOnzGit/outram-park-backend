# AGENTS.md

Guidance for Claude Code (and other AI assistants) working in the `outram-park-fork-cfmesh` crate.

## `no_std` + `alloc`, with a default `std` feature (gh:#1639, 2026-11-17)

Without its default `std` feature this crate is `no_std` + `alloc`, checked on
a bare-metal target by `scripts/check-nostd.sh`. Keep it that way:

- New code uses `core::` / `alloc::` paths and the `crate::nostd_prelude::*`
  glob; maps come from `crate::hash` (std's with `std`, so the fingerprint pins
  hold; `hashbrown` without).
- Clocks are `crate::time::Instant`, trace switches `crate::env::var`, and a
  thread pool is `#[cfg(all(feature = "std", not(target_arch = "wasm32")))]`
  with a serial path beside it.
- Files and printing are `#[cfg(feature = "std")]` (one attribute per
  `println!`), and so is any test or example that reads files.

## Reactor geometry is DRAWN for a human to check before it is trusted (HARD RULE)

**Maintainer direction, 2026-09-25.** Binds this crate. The same rule is in the
`CLAUDE.md` of `outram-mc-libs`, `nee_soon`, every `outram-foam-*` crate and
every crate downstream of them; a crate that newly depends on one of those
takes the rule into its own `CLAUDE.md` (check with `cargo metadata`).

**Whenever you build or change a complex reactor geometry** — CSG cells and
surfaces, lattices, pebble beds, TRISO particles, reflector zones, control-rod
bands, a CFD/FEM mesh, anything a solver will transport or integrate through —
**draw it, as images a human can open (PNG, or JPG/SVG), and hand them over**
before any result computed on it is reported as more than tentative.

- **Draw what the solver sees, not what you meant.** Render from the ASSEMBLED
  geometry (cell / material lookup at each pixel, or the mesh itself), never
  from the named constants. A picture of the constants hides exactly the
  defects this rule exists to catch.
- **Minimum set:** an axial slice (R-Z / x-z) of the whole model; radial (x-y)
  slices at the heights that matter; and, for nested geometry, zoomed slices at
  every level down to the smallest (pebble, TRISO particle). Colour by
  material, with a legend and the key dimensions marked.
- **Commit the images with the change** (beside the V&V record or the
  manuscript package) and point the human at them by path in your summary.
  Regenerate them whenever the geometry changes.
- **Say what you checked in them, and what you could not** — an image nobody
  was told to look at checks nothing.

**Why.** On 2026-09-24/25 the HTR-10 model carried, at once: a bottom reflector
mirrored from the top and up to 107 cm short; a core cavity that grew with the
bed; pebbles interpenetrating by 1.1 cm with 4.8 % of core carbon clipped away
(gh:#309, #310); and a TRISO lattice holding 8240 particles while reporting
8340 (gh:#316, +353 pcm). Every run completed with green diagnostics. Each was
found by looking at the built geometry, not by the eigenvalue.

**Tools.** ~~`outram_mc_libs::geometry::plot` samples a slice of an assembled
CSG geometry; OpenMC-parity image output (PNG/JPG) is the preferred path once
it lands.~~ **UPDATED 2026-09-25 — it has landed (gh:#268):**
`outram_mc_libs::geometry::plot` is a port of OpenMC's plotter that writes PNG
directly — slices, wireframe and solid ray traces — verified pixel-for-pixel
against `openmc --plot`
(`crates/outram-mc-libs/verification_and_validation/geometry_plotting/`).
`render_material_slice` draws a material-coloured slice with a legend and cm
axes in one call; `crates/nee_soon/examples/htr10_geometry_images.rs` is the
worked example. For meshes, plot the mesh itself (cells, patches, zones).

## Meshing nexus: outram-blender exports into this crate (GitHub #492, 2026-10-03)

`outram-blender`'s neutral `unstructured::UnstructuredMesh` is the one mesh
description the FV, FE and Monte Carlo solvers are built from. Its `foam-mesh` feature **calls** this crate's tet -> dual -> layers pipeline (through `outram_blender::foam_mesh`) and converts the `VolumeMesh` (`convert::cfmesh`). Note from the first run (2026-10-03): 264 of 24751 dual cells of a 0.1 m cylinder mesh are not star-shaped about their centroid; the neutral mesh accepts them (cells are defined by their bounding faces), but a consumer that decomposes cells about the centroid must check `decomposition_is_valid`.

**This crate must never depend on `outram-blender`** (GitHub #486 dependency
rule 1): blender depends on this crate, optionally, to export into it, and
Cargo counts optional dependencies when it looks for cycles. Mesh types stay
defined here; the converters live in `outram_blender::unstructured::convert`.

## Layer quality: three strategies, kept side by side (gh:#1215, 2026-11-04)

`TetDualOptions::layer_strategy` picks how the layer stage meets upstream's
mesh-quality limits: `BackOffOnly` (default; this crate's global uniform
back-off), `CfMeshOptimise` (port of cfMesh v1.1 `boundaryLayerOptimisation`
and `meshOptimizer`, from `github.com/DAFoam/cfmesh` `e944f6ba695f`, because
`wyldckat/cfMesh` v1.0 has no `boundaryLayerOptimisation`) and
`SnappyCollapse` (port of OpenFOAM-dev `852527377c7e` `snappyLayerDriver`'s
per-point `extrudeMode` loop). They share `layers::layer_setup` and
`layers::build_layered_variable`; never give one strategy its own copy of the
column geometry. Keep all three callable: the maintainer's differential-testing
rule (`AGENTS.md`) compares them on the same inputs, and the comparison table
lives in the gh:#1215 section of
`crates/outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155.md`.
Changing the default needs the maintainer, and re-measuring every number that
depends on it.

cfMesh's own **creation** route (`boundaryLayers::addLayerForPatch`, one
layer at half the local edge) is `layers::boundary_layers`, and its split
(`detectBoundaryLayers`, `refineBoundaryLayers`) is
`layers::refine_boundary_layers` (gh:#1270, #1271). ~~Both are opt-in; no
pipeline option calls them.~~ **UPDATED 2026-11-05 (gh:#1270 remainder):**
the tet-dual pipeline still does not call them; the four upstream workflows
do, as upstream (below). The patch-wise route
(`layers::boundary_layers::BoundaryLayers`, edge and corner cells where
patch groups meet) builds its cells as upstream lists them, not through
`build_layered_variable`, whose columns cannot hold edge or corner cells;
the one-group route and `addWrapperLayer` still build through it.
`create_new_vertex` is the one `createNewVertex`: both routes call it. ~~They also build~~ The one-group route and the split build through `layer_setup` and
`build_layered_variable`. Upstream's layer is not quality-checked: on the
pipe it inverts cells. The record is in the gh:#1270 and gh:#1271 sections
of the same V&V file.

`SnappyCollapse` builds layer cells by upstream's `nFaceLayers` rule
(`layers::build_layered_faces`, `FaceLayerCells::Upstream`, gh:#1241); the
gh:#1215 rule (`MostOfItsPoints`, cells pinched at an unextruded point) is
a port defect kept callable for the comparison. cfMesh's
`correctEdgesBetweenPatches` is `correct_edges.rs`, opt-in through
`TetDualOptions::correct_edges` (gh:#1241: it does not repair the
`Corners` rim, and costs 13–330 s on the demo pipe).

**Reproducibility is pinned across processes, not just within one.** A test
that runs the pipeline twice in one process cannot see `HashMap` iteration
order (seeded per process); `pipeline::tests::back_off_only_layers_are_pinned_bit_for_bit`
asserts fingerprints recorded once. ~~`octree.rs` still numbers cells in
`HashSet` order (the `refinement_levels > 0` path): not yet fixed.~~
**FIXED 2026-11-05 (gh:#1241):** the octree's leaves are a `BTreeSet`;
`pipeline::tests::octree_path_is_pinned_bit_for_bit` pins it (before the
fix three processes gave three fingerprints).

## The dual's boundary faces: three dual kinds, kept side by side (gh:#1240, 2026-11-04)

`TetDualOptions::dual_kind` picks the dual: `MedianPerCorner` (default,
unchanged: one boundary face per boundary-triangle corner, 4–8 per wall
cell, median 69° to the owner), `MedianPerPoint` (`dual::polyhedral_dual_per_point`:
OpenFOAM-dev `852527377c7e` `polyDualMesh`'s boundary rule on the median
dual, one face per point per feature-bounded sector) and `CellCentre`
(`cell_centre_dual`, feature `cell-centre-dual`: `outram-foam-mesh`'s
`poly_dual_mesh` itself; this crate may depend on it, it does not depend on
this crate). `dual_feature_points` picks upstream's two rules
(`OnFeatureEdge`, the utility's; `Corners`, the class's); with
`OnFeatureEdge` no back-off layer grows on a pipe (cfMesh's border cap is 0
at a rim vertex collinear with its neighbours). The ~40-line sector walk is
written in both crates (~~this one builds dependency-free~~ **CORRECTED
2026-11-05, gh:#1255:** this crate now depends on `outram-foam-basic-lib`
for OpenFOAM's `eigenValues`, but still not on `outram-foam-mesh` by
default); change one, change the other. Comparison and recommendation: the gh:#1240 section of
`crates/outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155.md`.
Changing the default needs the maintainer.

## The mesh surface: one engine, never another boundary map (gh:#1257, 2026-11-05)

`surface_engine::SurfaceEngine` is cfMesh's `meshSurfaceEngine` (serial):
boundary faces, points, edges, `pointFaces`, `pointPoints`, `edgeFaces`,
`faceEdges`, point/face normals and face centres, and the modifier.
`SurfacePartitioner` is `meshSurfacePartitioner` (corners, edge points,
feature edges, `pointPatches`). `layers::layer_setup`, `correct_edges`,
`surface_mapper` and `boundary_layer_optimisation`'s hairs read their
boundary maps from it; **a new stage that needs boundary addressing uses
the engine, it does not build its own map.** `surface_optimizer` holds
`partTriMesh`, `surfaceOptimizer`, `meshSurfaceCheckInvertedVertices`
(gh:#1258) and `meshSurfaceOptimizer` (gh:#1259), opt-in through
`TetDualOptions::untangle_surface` and
`BoundaryLayerOptimisationControls::optimise_surface` (both off by
default; changing that needs the maintainer). Note (gh:#1258): by cfMesh's
inverted-vertex test 40 % of the median/cell-centre duals' boundary points
are reflex corners of non-convex boundary polygons before any mapping.

## `optimiseFinalMesh` and the boundary-move projection (gh:#1283, 2026-11-05)

`final_mesh` holds upstream's `optimiseFinalMesh` for a mesh with no
`CfMeshOptimise` layer (`TetDualOptions::optimise_final_mesh`) and
`BoundaryProjection` (`EachPass`, `AfterUntangle`), which maps the points
`untangleMeshFV`'s boundary loop moved back onto their patches. **The
projection is not upstream.** Upstream's `untangleMeshFV` keeps points on
the surface only through `optimiseBoundaryVolumeOptimizer(true)`'s tangent
constraint and calls no mapper afterwards. All three switches are off by
default, and
`final_mesh::tests::foam_studio_default_mesh_keeps_its_fingerprint` pins
~~Foam Studio's default mesh~~ Foam Studio's pre-gh:#1298 default mesh
(**UPDATED 2026-11-05, gh:#1298:** Foam Studio now turns all three on
itself, with `CellCentre` + `Corners` and `CfMeshOptimise`
(`engine::CfMeshRoute::UpstreamNoLaplace`); this crate's
`TetDualOptions::default()` is unchanged, so its pins and the other
consumers keep the old pipeline).

Use the projection only with `untangle_surface`. On the `revert_tangled`
capture it re-maps the points that capture left off the surface on
purpose and tangles the mesh (h 4 and h 6 mm on the pipe). Record: the
gh:#1283 section of `laminar_pipe_gh1155.md`.

**The stage gate refuses degenerate faces (gh:#1286, 2026-11-05).**
`pipeline::acceptable` (what `keep_if_ok` applies to every stage) checks
closure, cell volumes, and now cfMesh's `checkFaceAreas(VSMALL)` and
finite points. Upstream has no such gate; it is the port's. A new stage
check goes there, not into a stage of its own.

## cfMesh's octree, ported beside `octree.rs` (gh:#1262–#1266, 2026-11-05)

`mesh_octree` (cubes, slot, surface intersections), `mesh_octree_queries`,
`mesh_octree_modifier`, `mesh_octree_creator` (`MeshDict`, inside/outside),
`object_refinement` and `cartesian_mesh` (`meshOctreeAddressing`,
`cartesianMeshExtractor`) transcribe DAFoam/cfmesh `e944f6ba695f`. They do
**not** replace `octree.rs`, whose fingerprint pin must not move. Keep both
routes callable: on the same leaves, `cartesian_mesh_extractor` and
`octree::build_mesh` build the same faces ring for ring (pinned). The
`decomposeCells` step is shared with `correct_edges` (`decompose_cells`;
**MOVED 2026-11-05, gh:#1267:** it now lives in `topology::decompose_cells`,
which `correct_edges`, `cartesian_mesh` and the `topology` checks all call,
pinned by `correct_edges::tests::correct_edges_is_pinned_bit_for_bit`);
change it once. Upstream's `findNearestSurfacePoint` is not exact;
`NearestSearch::Exact` is the port's exact variant. Record and images:
`verification_and_validation/cartesian_octree_gh1262/`.

## cfMesh's `tetMesh` and `pMesh`, beside the tet-dual pipeline (gh:#1272, #1273, 2026-11-05)

`pipeline::tet_mesh` (`tetMeshGenerator`) and `pipeline::p_mesh`
(`voronoiMeshGenerator`) are new entry points that share one generator
body. Their templates are `tet_creator_octree` + `tet_mesh_extractor_octree`
and `voronoi_mesh_extractor`; `workflow_controls`, `renumber_mesh` and
`rename_boundary_patches` are the workflow's own steps. They do **not**
change `surface_to_tet_dual_mesh`. They call the topology track's
`surfaceMorpherCells` / `topologicalCleaner` and the surface track's
`meshSurfaceOptimizer` in upstream's places. Steps still missing
(~~`edgeExtractor`,~~ ~~`boundaryLayers`~~ (**with a `boundaryLayers` dict since gh:#1270's remainder**, `TetMeshOptions::boundary_layers`), ~~`preMapVertices`~~ (**ported 2026-11-05, gh:#1274**, as `premap_vertices`; ~~but not yet called by `p_mesh`~~ **called by `p_mesh` since gh:#1274's remainder**),
~~`meshSurfaceEdgeExtractorFUN`~~ ~~`createFundamentalSheetsJFS`~~) call what
exists and say so in `TetMeshReport::notes`. **UPDATED 2026-11-05
(gh:#1280, #1281):** `extrudeLayer` (`layers::extrude_layer`) and
`createFundamentalSheets` (`fundamental_sheets`) are ported, and `pMesh`'s
FUN runs `createFundamentalSheetsJFS` by default
(`TetMeshOptions::fun_sheets`; `FunSheets::Without` is the ablation, the
gh:#1269 route); `renameBoundaryPatches` runs `symmetryPlaneOptimisation`.
Record: `verification_and_validation/fundamental_sheets_gh1281/`. **UPDATED 2026-11-05
(gh:#1269):** `patchAssignment` runs `edge_extractor` (cfMesh's
`edgeExtractor`, default) and `pMesh`'s `edgeExtraction` runs
`mesh_surface_edge_extractor_fun` (~~without the fundamental sheets, which
need `extrudeLayer`~~ with them since gh:#1281); `PatchAssignment::NearestTriangle` keeps the old
nearest-triangle rule callable for the differential. The tet-dual pipeline
still names patches by the nearest triangle. Record:
`verification_and_validation/edge_extractor_gh1269/`. When one of those lands, wire it in there and
**re-measure** the records in `verification_and_validation/tet_mesh_gh1272/`
and `p_mesh_gh1273/` (wiring the first two tracks moved `tet_mesh`'s sphere
volume error from +26.7 % to −1.36 %). The raw octree tet template has
non-manifold boundary points (upstream's own tets in coarse `BOUNDARY`
leaves), which `surfacePreparation` removes; `polyDualMesh` (`CellCentre`)
refuses the raw template on the sphere and the pipe.

## cfMesh's triangulated-surface tools (gh:#1276–#1278, 2026-11-05)

`surface` (`features`, `checks`, `cleanup`, `curvature`) ports
`triSurfaceTools/` and the surface utilities. **`mesh_octree::TriSurf` is
the crate's one triangulated-surface type** (it now carries upstream's
patches, stored feature edges, subsets and addressing): extend it, never
add a second one. A tool that changes points or facets builds a new
`TriSurf` (`from_parts`, `surface::rebuild`), so the addressing cannot go
stale as upstream's cached addressing does. No pipeline calls the module.
Several upstream tools are defective (listed in `src/surface.rs`); where the
text cannot work (`subsetToPatch`), the port follows the intent and says
so. ~~`meshOctreeAutomaticRefinement` is not ported.~~ **CORRECTED
2026-11-05:** it is ported (`mesh_octree_automatic_refinement`, next
section but one). Record and images:
`verification_and_validation/surface_tools_gh1276/`.

## cfMesh's `cartesianMesh` (gh:#1274, 2026-11-05)

`pipeline::cartesian_mesh` is `cartesianMeshGenerator`, a new entry point
beside `tet_mesh`. It shares `create_octree`, `SavedMesh`,
`PatchAssignment` and `face_regions` with it. Its template is
`cartesian_mesh::cartesian_mesh_extractor`, and its `surfaceProjection`
runs `premap_vertices` (`preMapVertices`, ported for it).

**Upstream adds one boundary layer to every cartesianMesh, with no dict**
(`addLayerForAllPatches`). ~~Here that is `layers::boundary_layers` on all
patches as **one group**. Upstream's patch-wise groups are not ported, so
on multi-patch surfaces the rim layer differs from upstream.~~ **UPDATED
2026-11-05 (gh:#1270 remainder):** patch-wise, as upstream
(`boundary_layer_generation`, shared by the four workflows);
`LayerTopology::OTopology` keeps the one-group layer callable for the
differential. `without_boundary_layer` is the explicit ablation. Record:
`verification_and_validation/patch_wise_layers_gh1270/`.

The record (the differential against `carve_box` + `snap` and the
tet-dual) and the images are in
`verification_and_validation/cartesian_mesh_gh1274/`.

## The octree remainders: automatic refinement, sources, addressing (gh:#1278, #1264–#1266, 2026-11-05)

`mesh_octree_automatic_refinement` is `meshOctreeAutomaticRefinement`
(curvature, corners, partitions, proximity). `createOctreeBoxes` runs it,
and the `surfaceMeshRefinement` / `edgeMeshRefinement` sources, in
upstream's place; each is **off unless the `MeshDict` asks**
(`min_cell_size`, `surface_mesh_refinement`, `edge_mesh_refinement`,
`subset_cell_size`, `local_refinement_subsets`). `MeshDict::anisotropic_sources`
moves the sources through gh:#1279's `anisotropic`
(`mesh_octree_creator::source_surface_geometry` /
`source_edge_mesh_geometry`); ~~the generators do not read it~~ the
generators read it too since gh:#1279's remainder (next section).

`MeshOctreeAddressing::from_dict` and `cartesian_mesh_extractor_from_dict`
read `nonManifoldMeshing`, `keep`/`removeCellsIntersectingPatches` and
`checkForGluedMesh`; with a dict that sets none of them they are `new` and
`cartesian_mesh_extractor` (tested field for field). The octree edges are
`MeshOctreeAddressing::edge_addressing` (equal to the extracted mesh's
edges). Upstream's proximity criterion needs the parts to be separate
within `1.732 h`: a channel closed at its sides within that range is never
refined. Record, the misses and the images:
`verification_and_validation/octree_remainders_gh1278/`.

## `anisotropicSources` in the four generators (gh:#1279 remainder, 2026-11-05)

`cartesian_mesh`, `tet_mesh`, `p_mesh` and `cartesian_2d_mesh` apply
`MeshDict::anisotropic_sources` where upstream's generators do:

- **Constructor.** The octree is built on the modified surface
  (`anisotropic::modified_surface_points`). Every step up to the revert
  maps, extracts and optimises against that surface.
- **3D generators.** At the end of `meshOptimisation`'s first
  `optimiseFinalMesh` they revert, then run
  `projectSurfaceAfterBackScaling` (`anisotropic::revert_and_project`,
  then `optimiseFinalMesh` again on the original surface).
- **2D generator.** The revert is at the end of `boundaryLayerGeneration`,
  followed by a new quadtree on the original surface
  (`cartesian_2d_mesh::refined_quadtree`), `remapBoundaryPoints` and
  `optimiseMeshSurface`.

A run stopped before the revert returns the mesh in the modified space,
as upstream does. Without sources, all four outputs are pinned to their
pre-wiring fingerprints
(`anisotropic::tests::generators_without_anisotropic_sources_are_pinned_bit_for_bit`):
~~**do not move that pin.**~~ **MOVED 2026-11-11 (gh:#1385, ~~flagged for the maintainer~~ accepted by the maintainer 2026-11-12, gh:#1336):** all four fingerprints moved (cell counts did not) when the generators were made to follow upstream bit for bit on the sphere; causes in the "Rust-side differential" section of `verification_and_validation/upstream_cfmesh_gh1385/README.md`. The pin guards the anisotropic wiring against *wiring* changes; re-pin again only for a fix that follows upstream. **Moved again 2026-11-11 (gh:#1385 round 4, ~~flagged~~ accepted by the maintainer 2026-11-12, gh:#1336): the 2D fingerprint, then the `pMesh` one (fifth move)** (`preMapVertices` 2 iterations; the 2D template and `distributeBoundaryFaces` carry the cell lists through `replaceBoundary`); cartesian, tet and p did not move.

**Accepted by the maintainer 2026-11-12 (gh:#1336):** every move of this pin (round 2 cartesian/tet, round 4 2D and pMesh; fingerprints only, cell counts unchanged) is accepted; the pin may still not move except for a fix that follows upstream.

**UPDATED 2026-11-13 (gh:#1385 round 5, `cartesianMesh` only):** the back-scaling's
`projectSurfaceAfterBackScaling` builds a **new octree on the original surface**
(`createOctreeWithRefinedBoundary(20, 30)`), maps every boundary point with its
`findNearestSurfacePoint` and runs the second `optimiseFinalMesh` with that octree
(`anisotropic::revert_and_project_octree`); ~~the port mapped to the exact nearest point
and optimised with brute-force searches~~ (`anisotropic::revert_and_project`, which
`tetMesh`, `pMesh` and the 2D generator still call: not yet compared with a staged
upstream run). Pinned by `tests/cfmesh_vs_upstream_single_orifice.rs`
(`cartesianMesh/singleOrifice`, final mesh equal to upstream's up to the pow ulp).

Record and images: `verification_and_validation/anisotropic_gh1279/`.

## cfMesh's `cartesian2DMesh` and the quadtree (gh:#1275, 2026-11-05)

`pipeline::cartesian_2d_mesh` is `cartesian2DMeshGenerator`. Its input is
a ribbon surface (`tri_surface_2d::extrude_2d_edges`). Its octree is the
**quadtree**: `MeshOctree::new_quadtree`, a root cube at `posZ = -10`
whose every cube spans the root box in z, with 4 children and the slots
4–7 set to `NO_CUBE`. **Any code that walks `sub_cubes` must skip
`NO_CUBE`**, or use `existing_sub_cubes`.

The 2D addressing is `poly_mesh_gen_2d_engine`, and the 2D mapping is
`surface_mapper_2d`.

~~`optimizeSurface2D`, `untangleSurface2D` and the 2D `refineBoundaryLayers`
are not ported; the report says so at each step.~~ **UPDATED 2026-11-05
(gh:#1275 remainder):** all three are ported
(`surface_optimizer::{optimize_surface_2d, untangle_surface_2d}`,
`refine_boundary_layers_2d`) and called where upstream calls them; the
layer is patch-wise. Record:
`verification_and_validation/cartesian_2d_mesh_gh1275/`.

## Every mesh leaves in OpenFOAM's upper-triangular face order (gh:#1285, 2026-11-05)

`renumber_mesh::upper_triangular` (steps 2–3 of upstream's `renumberMesh`
with the cell labels kept) is the **last step** of the tet-dual pipeline
(`pipeline::run_pipeline`) and of `cartesian::cartesian_box`. One final
step, not one per builder: every stage rebuilds its mesh from cell face
lists, so an order set inside a stage does not survive the next. A new
public route that returns a `VolumeMesh` ends with it too
(`poly_mesh_gen_checks::check_upper_triangular` must pass). The four
upstream workflows end with the full `renumberMesh` (cell walk too). The
geometry is unchanged by it (`upper_triangular_order_moves_no_geometry`);
fingerprints that hash face order moved once. Record:
`verification_and_validation/upper_triangular_gh1285/`.

## The `meshDict` reader: every entry gets a row (gh:#1476, 2026-11-11)

`mesh_dict::read_mesh_dict` reads an upstream `meshDict` (parsed by
`outram_foam_basic_lib::io::dict`, the one dictionary parser) into the
typed `MeshDict` and generator options; `MeshDictReading::resolve` matches
patch names as upstream's `findPatches` does (full regular-expression match,
`rename_boundary_patches::regex_full_match`). Rules that follow:

- **No entry is skipped silently.** Each gets a `KeywordStatus`: honoured,
  reported (read, not acted on, with the reason), not applicable (upstream's
  generator does not read it) or unknown. When a generator starts honouring a
  reported entry (for example `optimiseLayer`, once `optimizeBoundaryLayer` is
  wired), change its row in the same change and update the pin
  `mesh_dict::tests::tutorials_read_with_no_unhandled_keyword`.
- **The typed structs stay the interface**; the reader only fills them.
- Surfaces: `tri_surf_io` (FMS, FTR; STL through `outram-foam-mesh`'s port of
  OpenFOAM's `STLReader`, feature `surface-file`). Never a second STL reader.
- ~~**Known gap:** the generators take patch *names* only
  (`patches::SurfaceRegions`), so a surface's geometric types (`wall` in an
  FMS, `empty` from an STL, which upstream's `renameBoundaryPatches` turns into
  `wall`) do not reach the output; unrenamed patches come out as `patch`.~~
  **CORRECTED 2026-11-11 (gh:#1483):** `SurfaceRegions::types` carries each
  surface patch's geometric type (`TriSurf::surface_regions()` fills it from
  the file); `edgeExtractor::updateMeshPatches` and the 2D
  `distributeBoundaryFaces` put it on the mesh, and later steps keep it as
  upstream's `replaceBoundary` does. A surface built without types
  (`SurfaceRegions::single`, `from_labels`) keeps `patch`. Pinned against the
  committed upstream `boundary` files by
  `crates/outram-foam-cli/tests/cfmesh_patch_types.rs` and
  `tests/cfmesh_patch_types_sphere.rs`; record
  `verification_and_validation/patch_types_gh1483.md`.
- Tutorial inputs live in `test_data/cfmesh_tutorials/` (provenance in its
  `References.md`, excluded from the package).

## Avoid writing macros (workspace rule, 2026-11-05)

Maintainer direction: macros make code harder to read and review. Do not
write new `macro_rules!` or proc macros in this crate; use functions,
generics, enums and plain `match` arms. Using std and dependency macros
(`vec!`, `format!`, derives) is fine. Full rule: root `CLAUDE.md`, "Avoid
writing macros".

## Differential against real upstream output (gh:#1385, 2026-11-11)

`tests/cfmesh_vs_upstream_sphere.rs` pins `cartesianMesh`, `tetMesh` and `pMesh`
stage by stage against cfMesh's own output
(`verification_and_validation/upstream_cfmesh_gh1385/`): ~~bit-identical points
and faces through `boundaryLayerGeneration`; `meshOptimisation` within the
last bit, the cause proven in `tests/cfmesh_vs_upstream_cell_order.rs` (cell
face order, which `VolumeMesh` does not carry)~~ **UPDATED 2026-11-11
(gh:#1385, round 2):** all seven steps and the final meshes of the three
generators are bit-identical on the sphere (`VolumeMesh::cells` carries cfMesh's
cell face lists, see the V&V README), and asmoOctree is bit-identical through
`boundaryLayerGeneration` (`outram-foam-cli/tests/cfmesh_vs_upstream_stages.rs`).
Rules this taught:

- **OpenFOAM divides.** `v /= s` and `v/s` divide each component;
  `v.scale(1.0 / s)` differs in the last bit. Use `Vec3::div`. About 140
  `scale(1.0 / x)` remain in the crate; fix the ones on a path you compare.
- **The mapper, the untangler, the edge extractor and `optimiseMeshSurface`
  take the octree** (`SurfaceQueries::with_octree`,
  `optimise_mesh_surface_octree`,
  `map_vertices_onto_surface_patches_given_octree`): upstream's
  `findNearestSurfacePoint` is not an exact nearest point, and a brute-force
  one differs in the last bit and near shared edges by more.
- **`meshSurfaceEdgeExtractorNonTopo` / `...FUN` run on every surface**, also
  one of a single region; `regions == None` is not a reason to skip them.
- **Face order is part of the answer.** Template: octree-face order plus
  `reorder_boundary_faces`; layers: `add_cells_upstream`. A stage built from
  cell lists (`from_cell_faces`) does not come out in upstream's order.
- **Cell face lists are part of the answer too** (`VolumeMesh::cells`, gh:#1385
  round 2): the centre of a cell sums its faces in list order. A stage that
  rebuilds a mesh must carry the lists the way upstream's modifier does
  (`reorderBoundaryFaces` relabels in place; `replaceBoundary` keeps the internal
  faces and appends the new boundary faces in the order of the boundary list,
  labels by patch; `removeCells` / `addCells` / `decomposeFaces` as written) or
  drop them (`None`): a carried list is used only while consistent with owner /
  neighbour. Set `cells: None` in a new `VolumeMesh` literal unless you carry them.
- **Multi-patch routines were checked on asmoOctree, not only the sphere:**
  `mapEdgeNodes` / `mapCorners`, `decomposeCells` and `createNewVertex` now follow
  upstream line by line (the approximation loop breaks before replacing the
  estimate; `distanceOfPointFromTheEdge` is the distance to the infinite line).
- **Round 3 (2026-11-11): the optimiser, its checks and the part tet mesh
  follow upstream operation by operation** (V&V README, "round 3"). Rules it
  taught: `findBadFaces` / `findLowQualityFaces` are cfMesh's checks on the
  **addressing's** geometry (`checks::pmg_geometry`, the `*_g` functions of
  `poly_mesh_gen_checks`), never on vertex-average centres; the part tet mesh
  takes the addressing's centres, cfMesh's cell-list order and `tetMatcher`;
  `partTetMesh::updateOrigMesh` marks **every** tet-mesh node as changed, moved
  or not; `scale(1.0 / x)` is wrong wherever upstream writes `/ x` (the
  volume optimiser, Knupp, the LP untangler, `generateNewVertices`).
- **An OpenFOAM `HashSet` is iterated in hash order, and that can be the
  answer, not round-off.** `triSurfaceCurvatureEstimator` fits a quadric over
  `otherLabels` (`labelHashSet`); with five to seven points the fit is
  near-singular and the order moves the curvature by O(1).
  `surface::curvature::FoamLabelHashSet` emulates `HashTable::setEntry`
  (capacity 128, head insertion, rehash at 0.8). Use it, or an equivalent,
  wherever upstream iterates a hash container into a sum or a list.
- **The one known residual is `pow`, not an algorithm.** glibc's `pow` on an
  FMA CPU differs from PETIR's no-FMA ARM `pow` by one ulp on some inputs; the
  `pow(stab, 3)` of `surfaceOptimizer::evaluateGradients` makes asmoOctree
  `meshOptimisation` 71649/71650 and the elbow differ from `edgeExtraction`
  on. Do not "fix" it by calling the platform `powf` (PETIR's gate forbids it);
  `asmo_octree_mesh_optimisation_is_within_one_pow_ulp` pins it from above.
  A difference **bigger** than a last-bit cascade is not this: look at the
  traces (`GH1385_TRACE`, patches `gh1385-trace-instrumentation-*.patch`).
- **C++ default arguments bind a `bool` to the first parameter (gh:#1385 round 5).**
  `untangleMeshFV` calls `tmo.optimiseBoundaryVolumeOptimizer(true)` and `(false)` against
  `(const label nIterations = 3, const bool nonShrinking = false)`: one unconstrained
  iteration below two global iterations, zero from five on. Upstream's comments describe an
  intent the code does not carry out; `mesh_optimizer::BoundaryMoveCalls::Upstream`
  (default) follows the code, `AsDocumented` the comments. Read the **signature**, not the
  comment, before porting a call.
- **Two face-geometry formulas.** `foam_face_centre` / `ring_area_vector` are
  `face::centre` / `face::areaNormal` (the surface engine, the optimisers);
  `checks::pmg_face_centre_and_area` is the polyMeshGen addressing's own
  (`makeFaceCentresAndAreas`), used for cell centres. Do not swap them.

## The staged tet-dual route: every stage kept for viewing (gh:#1765, 2026-11-26)

`pipeline::staged` (`surface_to_tet_dual_mesh_staged`,
`surface_to_tet_dual_mesh_multipatch_staged`) runs the same pipeline as the
plain entry points through `run_pipeline_observed`, keeping a
`StageSnapshot` of the mesh after every stage, and the
`boundary_layer_index` (core 0, k = k-th layer from the wall). It is for
showing the pipeline (Mesh Studio, `dhoby-ghaut/examples/mesh_studio/`),
not for solving.

- **It must stay bit-identical to the plain route**:
  `staged::tests::staged_route_is_bit_identical_to_the_plain_route`
  compares the fingerprint and the report (timings blanked). The plain
  route calls `run_pipeline_observed` with `None`, so a new stage is
  recorded by one `staged::record` call after it, never by a second copy
  of the pipeline.
- `checks::cell_quality` gives per-cell non-orthogonality and skewness
  whose maxima equal the `checkMesh` report bit for bit (pinned).

## Multi-region meshes with fitted interfaces (2026-11-26, NOT a port)

`pipeline::multi_region` meshes several regions in one mesh with every
interface a set of faces: a `MultiRegionSurface` (outer and interface
triangles, patch markers, one seed per region component) goes to TetGen
`-pq..A`; `multi_region_tet_mesh` returns the tetrahedra,
`multi_region_dual_mesh` duals each region on its own and stitches the
duals at the interfaces (points merged within 1e-9 of the bounding-box
diagonal; an unpaired interface face is an error). Needs
`TetSource::TetGen`; it does not touch `surface_to_tet_dual_mesh*`.

- **The dual's centroid winding can leave a non-convex cell open** (a
  vertex on a re-entrant edge). `rewind_dual` re-winds from the primal
  geometry, which is exact for a tet primal; `dual.rs` is unchanged.
- **`validate` checks closure only**: a cell wound inside out passes. Both
  multi-region routes also refuse a non-positive cell volume.
- Only the per-corner boundary rule is stitched; the interface cells carry
  the worst quality (HTR-10 at 10 cm: 83.6° against 63.8° elsewhere).
  Record and images:
  `crates/dhoby-ghaut/verification_and_validation/htr10_cht_mesh/`.

## TetGen as the tet source (gh:#1796, 2026-11-26)

`TetDualOptions::tet_source` picks where the tet → dual → layers route
gets its tetrahedra: `TetSource::CarvedCentroid` (~~default, unchanged~~
the default until 2026-11-26: carve, snap, centroid split, Delaunay
flips) or `TetSource::TetGen {
radius_edge, min_dihedral, max_volume }` (`pipeline/tet_source.rs`: the
surface becomes a PLC through `PlcInput::from_triangle_soup` with the
region index as marker, `tetrahedralize_plc` under `-pq<r>/<d>[a<V>]`,
`to_volume_mesh` names the patches). Changing the default needs the
maintainer.

**The default is TetGen since 2026-11-26** (maintainer: "set the default
crate wide to tetgen"): `TetSource::TetGenAtCellSize { 1.414, 3.5 }`,
resolved when the pipeline starts to `-a h^3/(6 sqrt 2)` from the run's
own `cell_size` (Mesh Studio's rule), so `..Default::default()` callers are
sized by their cell size. `refinement_levels`, `snap` and `max_flips` do
nothing on it. **Every test fixture written before that date builds its
options from `TetDualOptions::carved()`**, so the carved route's pins and
recorded numbers still pin what they pinned; the default has its own
tests (`tet_source::tests::the_default_*`). Open: on the TetGen default
`SnappyCollapse`'s layer index (541 cells) disagrees with its layer stage
(598) on the test pipe (`staged::tests`).

- **Carve, snap and the Delaunay flips are not run** on TetGen's tets; the
  staged log records carve and snap with `applied: false` and an empty
  mesh, Delaunay with `applied: false` and the TetGen mesh. The flips are
  skipped on principle: TetGen's `-q` output is already a constrained
  Delaunay refinement, and `flip_to_delaunay` (plain `f64`, blind to the
  constraining subfaces) could only undo a constrained face.
- **Cavities need hole points** (`tet_source::hole_points`): a closed
  surface component with negative signed volume (wound into what it
  encloses, e.g. the pebble in its channel) gets one. Without it TetGen
  meshed the pebble's inside and the `pebble` patch disappeared into
  internal faces.
- **Measured (Mesh Studio, 2026-11-26):** no layer grows on TetGen's
  **tetrahedra** with back-off or `CfMeshOptimise` on any of pipe, fuel
  channel, pebble (some wall tets have all four vertices on the boundary;
  `tet_source::tests::diagnose_boundary_tets_of_both_sources`). On their
  **cell-centre dual** (`Corners`) back-off grows 3 layers on the pipe and
  fuel channel (12–17 % of the asked thickness, against 6–9 % on the carved
  dual) and none on the pebble; `CfMeshOptimise` grows all 3 on all three
  (31–75 %) and 20 on the pipe. The cell-centre dual with corner feature
  points was kept by measurement against `MedianPerPoint` (same layers,
  7–18× slower under `CfMeshOptimise`) and `MedianPerCorner` (4–6× the wall
  faces, max non-orthogonality 80–88°).
- **TetGen output differs between native and wasm** on the pebble (35 347
  tetrahedra natively, 35 200 in the browser): the port calls `acos`,
  `cos`, `ln` and `powf` through the platform's `libm`, not PETIR, so a
  last-bit difference changes a refinement decision (gh:#1823). Pins are
  native.

## The `tetgen` module: TetGen 1.6.0, ported stage by stage (epic gh:#1789, 2026-11-26)

`src/tetgen/` ports Hang Si's TetGen 1.6.0 (`github.com/libigl/tetgen`,
commit `e05aca7df74e`, AGPL-3.0-or-later; AGPL is acceptable here since the
maintainer's 2026-10-20 direction) as a **module of this crate**, not a new
crate. Ported files carry `SPDX-License-Identifier: AGPL-3.0-or-later` and
the upstream attribution header (and, for `predicates.rs`, Shewchuk's
public-domain notice).

**Differential rule (binds every stage).** A stage is done when its output
is **bit-identical** (`to_bits()` / exact integer equality, no tolerance)
to upstream's, from the upstream `tetgen` binary or from a C harness
compiled against the upstream source (`-ffp-contract=off`, no fast-math),
on committed vectors the Rust test reads without a compiler. A residual is
reported with its cause, never absorbed in a tolerance. Port line for
line: TetGen uses the predicates' **magnitudes**, so a value-different but
sign-equal substitute (`delaunay.rs`, `outram-blender`'s
`boolean_predicates.rs`) is not a port.

| Stage | Issue | Content | Status |
|---|---|---|---|
| 1 | gh:#1790 | exact predicates (`predicates.cxx`) | done: 84 000 / 84 000 bit-identical, `verification_and_validation/tetgen_predicates_gh1790/` |
| 2 | gh:#1791 | point-set Delaunay (`transfernodes`, BRIO/Hilbert sort, `incrementaldelaunay`, ~~`insertpoint`~~ `insert_vertex_bw`) and the mesh data structure | done: 12 / 12 cases identical to `tetgen -zQ` (165 124 tets, order and vertex order), `verification_and_validation/tetgen_delaunay_gh1791/` |
| 3 | gh:#1792 | PLC input (`.poly`/`.smesh`/`.edge`, `PlcInput`) and surface meshing (`meshsurface`) | done: 13 / 13 cases identical to an instrumented upstream (39 207 dump lines: every subface, subsegment, tet and point slot with its links, the dead-item stacks, the random state), `verification_and_validation/tetgen_meshsurface_gh1792/` |
| 4 | gh:#1793 | boundary recovery (`recoverboundary`, `tetrahedralize_plc`) | done: 38 / 38 runs (19 cases x `-p`, `-pY`) identical to an instrumented upstream (86 291 dump lines, 55 reach counters), `verification_and_validation/tetgen_recoverboundary_gh1793/` |
| 5 | gh:#1794 | the rest of `-p`: `carveholes`, `suppresssteinerpoints`, `recoverdelaunay`, `smooth_vertices`, `improve_mesh`, `jettisonnodes`, output, `to_volume_mesh` | done: 48 / 48 runs (21 cases x `-pz`, `-pzY`; 3 x `-pzA`, `-pzAY`), 192 / 192 output files byte-identical to the unmodified upstream binary, 34 reach counters equal, `verification_and_validation/tetgen_plc_gh1794/` |
| 6 | gh:#1795 | Delaunay refinement (`-q`, `-a`); ~~and mesh improvement~~ (mesh improvement is stage 5: plain `-p` runs it) | done: 41 / 41 runs (21 cases x `-pqz`; subsets x `-pq1.2z`, `-pq1.414/12z`, `-pqa<V>z`; `-pqAz`; Mesh Studio pipe, fuel channel, pebble x `-pq1.414a0.0026z`), 164 / 164 files byte-identical to the unmodified upstream binary, 64 reach counters equal, `-V` statistics identical, `verification_and_validation/tetgen_quality_gh1795/` |
| 7 | gh:#1796 | TetGen as the tet source for the tet → dual → layers route, and in Mesh Studio | done: `pipeline::TetSource::TetGen` (`pipeline/tet_source.rs`, default still `CarvedCentroid`), Mesh Studio's default; measurements in the gh:#1796 section of `dhoby-ghaut/verification_and_validation/mesh_studio_gh1771/README.md` |

`tetgen::predicates::Predicates` holds what `exactinit` sets (the C
globals of `predicates.cxx`) in place of upstream's file-scope statics.

**Stage 2 (gh:#1791)** adds `tetgen::mesh` (the `tetgenmesh` object later
stages extend: `TetRecord` with upstream's twelve words, the subface and
subsegment slots `seg`/`sub` reserved; `PointRecord`; `TriFace` with `ver`
in `0..12` and every orientation table transcribed one for one, including
the subface ones stages 3 on need) and `tetgen::delaunay`
(`delaunay_tetrahedralize`). ~~`insertpoint`~~ **CORRECTED 2026-11-26:**
upstream's point-set path inserts with `insert_vertex_bw` (Bowyer-Watson),
not `insertpoint`; `insertpoint` and its flips are first used by ~~stage 3~~
stage 4 (~~`delaunizesegments`~~ **CORRECTED 2026-11-26, gh:#1793:**
`recoverboundary`'s Steiner points; `delaunizesegments` is the `-D` path and
is not ported; **CORRECTED 2026-11-26, gh:#1792:** `meshsurface` calls
neither) and ~~are not ported yet~~ are ported in stage 4 (`locate` was
already). Rules it taught:

- **Pool order is part of the answer.** `.ele` order is upstream's pool
  traversal (allocation slot order, dead records skipped) and a freed
  record is reused LIFO first. Create and free tetrahedra and points only
  through `MemoryPool` (`maketetrahedron2`, `tetrahedrondealloc`,
  `makepoint`, `pointdealloc`), never with a plain `Vec::push`.
- **The C library's `rand()` is part of the answer** (the vertex
  permutation, `locate_dt`'s face choice, the new `recenttet`).
  `mesh::GlibcRand` is glibc's; upstream built against another libc gives
  another valid mesh. TetGen's own `randomnation` (seed in the mesh) is
  separate; both are ported.
- **Raw-pointer reads are emulated, not "fixed".** `insert_vertex_bw` tests
  `((point *) point2tet(p))[6] == insertpt` on a handle that may carry a
  version (the first tet's four points); upstream's read then lands
  mid-record and never matches, and the port says so in
  `point2tet_apex_is`.

**Stage 3 (gh:#1792)** adds `tetgen::plc` (`PlcInput`: `read_poly`,
`read_smesh`, `read_edge`, `read_poly_with_node` follow `tetgenio`'s
scanning token for token; `from_triangle_soup` is the entry for cfMesh and
Blender surfaces, one marker per triangle) and `tetgen::surface`
(`plc_meshsurface`, `TetgenMesh::from_plc`, `meshsurface` and everything it
calls), with the subface and subsegment pools in `mesh.rs` (`ShellRecord`,
`Face`, the encoded `ShellFace`). Rules it taught:

- **Compare the whole state, not the output.** `meshsurface` writes no
  file, so the reference is an instrumented upstream build that dumps every
  pool slot (dead ones too) with its links and versions, the dead-item
  stacks and the random state (`meshsurface_dump.patch` in the V&V folder;
  the port's `dump_meshsurface` writes the same text). Stage 4 starts from
  exactly this state; extend the dump, do not narrow it.
- **Inputs must reach the paths.** The first eight cases (boxes, the Mesh
  Studio soups, a sphere) were identical but never flipped an edge or
  recovered a segment; `comb_prism`, `grid_box`, `jitter_soup`, `slit_box`
  and `fin_box` were added to reach `flip22`, `lawsonflip`,
  `sscoutsegment`'s flips, `ONEDGE` insertion and `facedihedral`. Count the
  branches a case reaches (a temporary trace) before calling it covered.
- **Upstream's dead-item link is written into the record** (`sh[0]` of a
  freed subface or subsegment, `memorypool::dealloc`); `shellfacedealloc`
  emulates it. A loop variable whose version upstream carries from one
  record to the next (`mergefacets`' `shloop.shver`) is carried here too.
- **Where upstream dereferences `NULL`, the port returns
  `TetgenError::InternalError`.** `fin_crash.poly` segfaults upstream in
  `unifysegments` (a fin edge inside a facet, not on a segment); the port
  errors at the same read (pinned).

**Stage 4 (gh:#1793)** adds `tetgen::flip` (the tet-subface / tet-segment
links and their `tet2subpool` / `tet2segpool`, `flip23`, `flip32`,
`flip41`, `flip31`, `flipnm`, `flipnm_post`, `tri_edge_test`,
`checkflipeligibility`, `removeedgebyflips`, `removefacebyflips`),
`tetgen::recover` (`recoverboundary`, `recoversegments`,
`recoveredgebyflips`, `recoversubfaces`, `recoverfacebyflips`,
`finddirection`, `getedge`, `makesegmentendpointsmap`, the dump, and the
entry `tetrahedralize_plc`) and `tetgen::steiner` (`insertpoint`, the
`add_steinerpt_*` routines, `smoothpoint`, `removevertexbyflips`,
`sremovevertex`, `lawsonflip3d`, `recoverdelaunay`). Rules it taught:

- **`recoverboundary` reaches stage 5's `recoverdelaunay`** (after every
  Steiner point), so it is ported here; stage 5 reuses it. The CDT routines
  (`-D`: `delaunizesegments`, `formregion`, `formcavity`, `fillcavity`,
  `scoutsubface`, `scoutsegment`, ...) are **not** reached by
  `recoverboundary`; only `scoutsegment` and `scoutsubface` are ported
  (unit-tested), the cavity routines are not.
- **Reference runs are without `-Q`.** Upstream wraps a few
  `terminatetetgen` calls inside its `!b->quiet` warning blocks, so `-Q`
  changes control flow; the port follows the default and collects the
  warnings in `TetgenMesh::warnings`.
- **Count branches, not just routines.** The instrumentation's `tg_reach`
  counters (55: every recovery routine plus the Steiner and flip branches)
  are part of the comparison; a case that does not move a counter does not
  test that path. Not reached by any committed case (say so if you rely on
  them): `removevertexbyflips`' 4-to-1 (`INTETRAHEDRON`) and
  shift-into-volume branches, duplicated facets (upstream segfaults on the
  overlapping-facet input tried), `splitsliverflag`, `lawsonflip3d`'s
  non-convex hull branch (`nonconvex` is set by `carveholes`, stage 5), and
  the `unflip` branches of `flipnm` / `flipnm_post` (mesh improvement).
- **Where upstream reads a `NULL` list**, the port errors at the same read:
  `create_a_shorter_edge`'s facet maps are built only by `-D` and `-q`.
- **`flipstack` is one list upstream** (surface `flipshpush` and tet
  `flippush` items); here `flipstack` and `tflipstack`, never both non-empty
  on the paths ported. Keep it so, or merge them.

**Stage 5 (gh:#1794)** adds `tetgen::carve` (`carveholes`,
`suppresssteinerpoints`, `suppressbdrysteinerpoint`, the general
`getvertexstar_with`), `tetgen::optimize` (`get_tetqual`, the bad-tet
priority queue, `improve_mesh`, `repair_badqual_tets`, `repair_tet`,
`flip_edge_to_improve`, `add_steinerpt_to_repair`, `smooth_vertices`,
`move_vertex`, the Laplacian centres), `tetgen::output` (`TetgenOutput`,
the four file writers with C's `%.17g`, `to_volume_mesh`) and
`tetgen::tetrahedralize` (`TetgenMesh::finish_plc`, the `Switches`
builder); `tetrahedralize_plc` runs the whole `-p` route. Rules it taught:

- **Read the switch post-processing, not only the defaults.**
  `parse_commandline` turns `optmaxdihedral` 177 into 179.9 when `-q` is
  off (`tetgen.cxx:3744`); with 177, 32 of 192 files differed.
  `finish_plc` applies it, so a hand-built `Behavior` gives what the
  command line gives.
- **Plain `-p` improves the mesh.** `improve_mesh` runs under `b->plc`;
  on `pebble -pzY` it repairs 1 782 tetrahedra and adds 16 Steiner points.
  Its reach counters (`TetgenMesh::reach5`, `stage5_reach.patch`) are
  compared with the files. Not reached by any committed case: segment
  deletion in `carveholes`, `lawsonflip3d`'s hull-vertex removal,
  successful boundary-Steiner suppression, the skinny-tet branch and
  `create_a_shorter_edge` from improvement, a failed `insertpoint` in
  `add_steinerpt_to_repair`, `get_surf_laplacian_center` (no facet Steiner
  point survives to smoothing).
- **Hull slivers are peeled across facets.** On `sphere -pz`
  `lawsonflip3d` removes 534 slivers between boundary triangles of
  different facets and flips those triangles (upstream does too): the mesh
  encloses the PLC volume less 3.4e-6. `TetgenMesh::hull_sliver_volume`
  (not upstream) records it.
- Reference outputs come from the **unmodified** binary run as
  `tetgen -<sw> <file>` (the `# Generated by` line holds the command line,
  with a trailing space), without `-Q`.

**Stage 6 (gh:#1795)** adds `tetgen::refine` (`delaunayrefinement`, the
three repair loops, the encroachment tests, the three splits,
`locate_on_surface`, `locate_point_walk`, the facet and segment maps,
`RefineState` for upstream's refinement members), `tetgen::quality`
(`qualitystatistics`, the `-V` block) and, in existing files,
`insertpoint`'s `rejflag`, `check_insert_radius`, `smlenflag` and
`chkencflag & 1 / 2`, the flips' and `flip22`'s queues
(`flip22_chk`), `Switches::quality` / `max_volume`. Rules it taught:

- **Loop variables carry versions across iterations** (again, as stage 3's
  `mergefacets`): `delaunayrefinement`'s subface pass keeps the `shver`
  `check_subface` left for the next subface. A fresh version per subface
  gave the right nodes and tets and the `.face` file in another order.
- **`decode` is not `fsymself`.** `locate_point_walk` steps with
  `decode(tet[ver & 3])`, keeping the neighbour's stored version; the
  version decides the `ONEDGE` spin, the cavity order and `encshlist`.
- **Localise with state dumps at phase boundaries.** `stage6_reach.patch`
  adds `TG_DUMP6S` / `6F` / `6Q` (after the segment pass, the subface
  pass, refinement); comparing them with the port's `dump_recoverboundary`
  and then tracing one routine's inputs found both faults.
- **`-a` is a target, not a bound,** and `-q` leaves tetrahedra above the
  ratio near sharp segments and corners: upstream's own output does
  (`refined_meshes_meet_their_bounds` checks every such tetrahedron is
  upstream's). Do not "fix" it in the port.
- **A pointer comparison in `qualitystatistics`** (`tetloop.tet <
  neightet.tet`) is emulated by block order (a later `mmap`'d block lies
  lower); slot order put two face angles in the wrong histogram bin.
- Not reached by any committed case: `check_enc_subface`'s same-facet
  `flippush`, `locate_on_surface`'s `rand()` coin flip. Not ported: `-r`
  element lists, `-m` sizes, `-aa`, `-S`, `-c`.
