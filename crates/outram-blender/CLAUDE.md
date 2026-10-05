# CLAUDE.md

Guidance for Claude Code (and other AI assistants) working in the `outram-blender` crate.

**What this crate is (2026-10-02, GitHub #486): geometry description +
meshing.** It owns the CSG geometry description and its pure navigation kernel
(`csg`), the OpenMC-port geometry plotter (`csg::plot`), the Monte Carlo tally
mesh description (`spatial_mesh`), and the mesh-authoring frontend (a GPL fork
of Blender's mesh architecture) with its OpenFOAM bridges. ~~It is planned to
become the meshing nexus for FV/FE too (#492).~~ **Since 2026-10-03 (#492) it
is the meshing nexus:** `unstructured::UnstructuredMesh` is the one mesh
description FV (`PolyMesh`/`FvMesh`), FE (`farrer-park`) and Monte Carlo
tallies (`MeshKind::Unstructured`) are built from; see "Meshing nexus"
below.

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
**MOVED 2026-10-02 (GitHub #486):** the plotter now lives in this crate as
`outram_blender::csg::plot` (re-exported unchanged as
`outram_mc_libs::geometry::plot`); its pixel-parity V&V stays in
`crates/outram-mc-libs/verification_and_validation/geometry_plotting/`.

## Cargo features (2026-10-02)

| feature | default | pulls | what it gates |
|---|---|---|---|
| `gpu` | **on** | `wgpu` (never on Android) | `src/gpu.rs`, the GPU attempt in `Affine3::transform_points_best_effort`, and (since 2026-10-05, gh:#587; also never on wasm32) `csg::gpu::render`, the GPU ray tracer of the assembled CSG geometry. `csg::gpu::flat`, its encoding, and `csg::gpu::index`, its grid index (2026-10-05), are ungated |
| ~~`mc-export`~~ | — | — | **RETIRED 2026-10-02** (#486): `to_mc_geometry`, `sim` and the `mc_godiva_keff` example moved to `nee_soon` |
| `foam-export` | off | `outram-foam-basic-lib` | polyMesh read/write bridge |
| `foam-mesh` | off | `outram-park-fork-cfmesh` | `foam_mesh` volume-meshing bridge |
| `gnn-graph` | off | `raffles` (~~`outram-mc-libs`~~, gone since #486 stage 3a) | `gnn_graph::cell_adjacency_graph` (CSG cells -> RAFFLES graph) |
| `block-mesh` (2026-10-03, #492) | off | `outram-foam-mesh` (+ `foam-export`) | `unstructured::convert::block_mesh`: blockMesh and the snappyHexMesh driver -> neutral mesh |
| `fem-export` (2026-10-03, #492) | off | `farrer-park` | `unstructured::convert::fem`: neutral <-> farrer-park `Mesh`, farrer-park's generators |

`foam-export` also gates `unstructured::convert::foam` (neutral <->
`PolyMesh`, -> `FvMesh`) and `foam-mesh` gates `unstructured::convert::cfmesh`
(cfMesh `VolumeMesh` -> neutral) since 2026-10-03.

- **`gpu` is a feature so dependents can drop `wgpu`** (GitHub issue #486):
  `default-features = false` gives the CPU path only. Every use of `crate::gpu`
  must sit under `#[cfg(all(feature = "gpu", not(target_os = "android")))]`,
  the module's own gate. Both gates are needed: the feature alone would try to
  compile wgpu code on Android, where `wgpu` is target-gated out.
- **Check both feature sets** before calling a change done:
  `cargo test --release -p outram-blender --lib --tests` and the same with
  `--no-default-features`.
- **`gnn-graph` lives here by maintainer decision** (2026-10-02): this crate
  owns geometry description under #486. ~~It needs `outram-mc-libs` only
  because `Cell` lives there today.~~ **CORRECTED 2026-10-02:** `Cell` is
  `crate::csg::cell::Cell` now, so the feature pulls only `raffles`.
  **`raffles` must not depend on this crate** (or on `outram-mc-libs`); the
  edge runs geometry -> RAFFLES only.
- **There is no `mesh` feature** (maintainer decision 2026-10-02): mesh
  authoring and `faer` are always compiled, even for `default-features =
  false` dependents such as outram-mc-libs. `faer` is target-split in
  `Cargo.toml` instead (no `rayon`/`rand` on wasm32) so the whole crate builds
  for `wasm32-unknown-unknown`; it is in `scripts/check-wasm.sh`.

## CSG description and tally meshes: what lives here, what does not (GitHub #486)

Moved here from `outram-mc-libs` on 2026-10-02 (maintainer decisions recorded
on GitHub #486):

| here (`outram_blender`) | stays in `outram-mc-libs` |
|---|---|
| `csg::{position, surface, cell, universe, lattice, geometry, triso_particle}` — the description and the **pure** navigation kernel: surface evaluate / sense / distance / normal / reflect, `Cell::contains` / `distance_to_boundary`, `Universe::find_cell`, `Geometry::locate` / `distance_to_boundary`, lattice indices / local positions / distances | transport-state work on `*Ext` traits in `geometry::crossing`: `GeometryExt` (`cross_surface*`, nudging, corner reflection, `distance_out_of_level`, `sigma_t_at`, `validate_boundary_conditions`), `SurfaceKindExt` (`diffuse_reflect`, virtual-lattice helpers); distribcell, virtual lattices, `volume_calc`, GPU encoders and WGSL |
| `csg::plot` — the OpenMC plotter (slices, ray traces, `ModelPlot`, PNG); materials enter through the `MaterialIdentity` trait | the plot-parity V&V tests (`tests/geometry_plot_openmc_parity.rs`, `python_plot_parity.rs`, `xs_and_tracks_plot_parity.rs`) and `impl MaterialIdentity for Material` |
| `spatial_mesh` — Regular / Rectilinear / Cylindrical / Spherical meshes and `MeshKind` (bounds, edges, bin counts, volumes, public `unflatten`) | bin lookup and scoring on `RegularMeshExt` & co. (`get_bin`, `bin`, `indices`, `surface_bins_crossed`, `count_sites`, `shannon_entropy`) |
| `export::to_csg_geometry` — fit a surface mesh and return a native CSG `Geometry` | — (`nee_soon::blender_bridge::to_mc_geometry` is a wrapper over it) |

Rules that follow:

- **This crate must never depend on `outram-mc-libs`**, not even optionally:
  outram-mc depends on this crate, and Cargo counts optional dependencies when
  it checks for cycles. A Monte Carlo bridge belongs in `nee_soon`.
- **`MeshKind` is deliberately NOT `#[non_exhaustive]`**: ~~the planned
  `Unstructured(Arc<..>)` variant (#492) must break every `match` site at
  compile time.~~ **`Unstructured(Arc<UnstructuredMesh>)` landed 2026-10-03
  (#492)** and every match site in outram-mc-libs was updated by hand; the
  next variant must force the same review.
- **Keep the documented raw-`f64`-cm API** in `csg` and `spatial_mesh` (no
  `uom`): it is the particle-tracking inner loop; see outram-mc-libs'
  `CLAUDE.md`, "Units: raw `f64`, not `uom`".
- **Hot-path functions carry `#[inline]`**, because outram-mc calls them
  across the crate boundary without LTO.
- **A change to `csg` or `spatial_mesh` is a change to outram-mc's
  transport.** Run outram-mc-libs' bit-identity gate
  (`tests/stats_move_fingerprints.rs`) and its plot-parity tests, and draw the
  geometry (HARD RULE above).
- The code is an OpenMC port (MIT): keep each file's provenance header; see
  `NOTICE` and `LICENSE.openmc`.

## Meshing nexus (GitHub #492, 2026-10-03)

`unstructured::UnstructuredMesh` is the **neutral mesh description**: points,
faces wound out of their owner, owner / neighbour, cells that carry both the
FV view (faces) and, when typed, the FE view (element kind + ordered nodes),
typed patches, cell zones and an explicit `LengthUnit`.

| here | elsewhere (never moved here) |
|---|---|
| the description, its geometry (OpenFOAM `primitiveMesh` port), the bounding-triangle cell definition and centroid decomposition, `CellLocator`, the 1-D mesher (`one_d`), the plotter (`unstructured::plot`) | the meshers: blockMesh / snappyHexMesh (`outram-foam-mesh`), cfMesh (`outram-park-fork-cfmesh`), farrer-park's FEM generators — **called** from `unstructured::convert`, behind features |
| converters: `convert::foam` (<-> `PolyMesh`, -> `FvMesh`), `convert::fem` (<-> farrer-park `Mesh`), `convert::cfmesh`, `convert::block_mesh` | point location and track-length scoring on the mesh: outram-mc-libs `tally::mesh_unstructured::UnstructuredMeshExt` |

Rules that follow:

- **Direction (GitHub #486 rule 1).** This crate depends on the FV/FE solver
  crates (optionally); **they must never depend on it**, or they could not be
  export targets. Monte Carlo is the other way round (outram-mc-libs depends
  on this crate).
- **A cell is the region bounded by its faces' fan triangles**
  (`for_each_bounding_triangle`), not its centroid decomposition. Some cfMesh
  dual cells are not star-shaped about their centroid (264 of 24751 on the
  first cylinder drawn, 2026-10-03); `decomposition_is_valid(c)` says whether
  the decomposition may be used (sampling only).
- **Units:** raw `f64` in the mesh's declared `LengthUnit` (the
  `csg`/`spatial_mesh` raw-`f64` exception); converters scale to metres for
  the FV/FE crates, Monte Carlo scales its cm positions into the mesh unit.
- **Draw every new mesh** (HARD RULE above) with `unstructured::plot`;
  `examples/meshing_nexus_images.rs` is the worked example and
  `docs/meshing_nexus/` holds its images and what was checked in them.
- **Not yet here** (maintainer default, 2026-10-03): OFFBEAT and Moltres
  meshes; the MGXS round trip lives in `nee_soon`.
