# CLAUDE.md

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
`layers::refine_boundary_layers` (gh:#1270, #1271). Both are opt-in; no
pipeline option calls them. They also build through `layer_setup` and
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
(`edgeExtractor`, `boundaryLayers`, `preMapVertices`,
`meshSurfaceEdgeExtractorFUN`) call what exists and say so in
`TetMeshReport::notes`. When one of those lands, wire it in there and
**re-measure** the records in `verification_and_validation/tet_mesh_gh1272/`
and `p_mesh_gh1273/` (wiring the first two tracks moved `tet_mesh`'s sphere
volume error from +26.7 % to −1.36 %). The raw octree tet template has
non-manifold boundary points (upstream's own tets in coarse `BOUNDARY`
leaves), which `surfacePreparation` removes; `polyDualMesh` (`CellCentre`)
refuses the raw template on the sphere and the pipe.
