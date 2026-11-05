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
written in both crates (this one builds dependency-free); change one, change
the other. Comparison and recommendation: the gh:#1240 section of
`crates/outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155.md`.
Changing the default needs the maintainer.
