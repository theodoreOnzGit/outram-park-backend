# AGENTS.md

Guidance for Claude Code (and other AI assistants) working in the `outram-park-fork-cfmesh` crate.

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
~~**do not move that pin.**~~ **MOVED 2026-11-11 (gh:#1385, flagged for the maintainer):** all four fingerprints moved (cell counts did not) when the generators were made to follow upstream bit for bit on the sphere; causes in the "Rust-side differential" section of `verification_and_validation/upstream_cfmesh_gh1385/README.md`. The pin guards the anisotropic wiring against *wiring* changes; re-pin again only for a fix that follows upstream.

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
- **Two face-geometry formulas.** `foam_face_centre` / `ring_area_vector` are
  `face::centre` / `face::areaNormal` (the surface engine, the optimisers);
  `checks::pmg_face_centre_and_area` is the polyMeshGen addressing's own
  (`makeFaceCentresAndAreas`), used for cell centres. Do not swap them.
