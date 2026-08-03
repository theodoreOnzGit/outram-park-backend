//! # outram-park-fork-cfmesh
//!
//! A pure-Rust **fork / port of [cfMesh](https://github.com/wyldckat/cfMesh)**
//! (Creative Fields' automatic unstructured mesh generator, **GPL-3.0**) for the
//! OUTRAM PARK multiphysics suite — the *volume-mesh-generation* layer that sits
//! between the [`outram-blender`](https://crates.io/crates/outram-blender)
//! surface-authoring frontend and the OpenFOAM-style solvers.
//!
//! ```text
//!   outram-blender  ──►  outram-park-fork-cfmesh  ──►  outram-foam-basic-lib
//!   (surface Mesh)       (tet / polyhedral cells        (PolyMesh → FvMesh,
//!                         + boundary layers)              solvable)
//! ```
//!
//! ## Why this crate exists
//!
//! The workspace already has the mesh **representation** and finite-volume
//! addressing (`outram-foam-basic-lib`'s `FvMesh` / `io::poly_mesh::PolyMesh`,
//! with `read`/`write`/`to_fv_mesh`), and the surface-**authoring** frontend
//! (`outram-blender`). What was missing everywhere is unstructured mesh
//! **generation** — turning a closed surface into a solvable *volume* mesh with
//! polyhedral cells and near-wall boundary layers. Open-source, pure-Rust,
//! GPLv3-clean tooling for this is genuinely lacking, so this crate ports the
//! proven cfMesh workflows rather than reinventing them.
//!
//! ## Goal
//!
//! Consume an `outram-blender` [`Mesh`](https://docs.rs/outram-blender) (a closed
//! watertight surface), generate a **tetrahedral** then **polyhedral** volume
//! mesh (à la cfMesh `tetMesh` / `cartesianMesh` + OpenFOAM `polyDualMesh`), with
//! optional **wall boundary/prism layers**, and emit a real-cell
//! `outram_foam_basic_lib::io::poly_mesh::PolyMesh` for the CFD/TH solvers and
//! `outram-mc-libs` geometry for neutronics — the mesh substrate for coupled
//! pebble-bed / molten-salt / light-water reactor simulations.
//!
//! ## Vendored upstreams (reference only, GPLv3-clean, never shipped)
//!
//! Both live under `upstream_source/` (gitignored, dev-only — see
//! `upstream_source/README.md`):
//!
//! - **cfMesh** — <https://github.com/wyldckat/cfMesh>, **GPL-3.0-only**. Primary
//!   port target: `meshLibrary/{cartesianMesh, tetMesh, utilities}` (Cartesian
//!   hex-dominant + tet meshing, surface tools, boundary-layer insertion).
//! - **voro++** — <https://github.com/chr1shr/voro>, modified-BSD (LBNL),
//!   GPLv3-compatible. Reference for the Voronoi / polyhedral-dual construction.
//!
//! Ported files carry the upstream provenance header block (project, source
//! file, commit, copyright, licence) per the workspace provenance rule; the
//! algorithms are re-implemented in Rust, not transcribed verbatim from C++.
//!
//! ## Status
//!
//! - **Milestone 1 — volume-mesh core + Cartesian block mesher.**
//!   [`volume_mesh::VolumeMesh`] (points + faces + owner/neighbour + patches,
//!   mirroring cfMesh `polyMeshGen` / OpenFOAM `polyMesh`) and
//!   [`cartesian::cartesian_box`] (a regular hex grid of an axis-aligned box).
//! - **Milestone 2 — castellated surface carve.** [`carve::carve_box`] overlays
//!   a uniform Cartesian grid on a closed triangle-soup surface and keeps the
//!   cells inside it (ray-parity inside test), producing a body-fitted
//!   *staircase* volume mesh with a `walls` boundary patch.
//!   [`carve::carve_region`] extends this to the region *inside* an outer
//!   surface but *outside* inner holes — the shell/annular pattern reactor
//!   geometry needs (coolant around fuel pins or pebbles).
//! - **Milestone 3a — boundary snapping.** [`snap::snap_to_surface`] projects
//!   every boundary point onto the closest point of the surface, turning the
//!   staircase into a body-fitted boundary.
//! - **Milestone 3b — foam bridge (feature `foam-export`).** [`foam::to_poly_mesh`]
//!   converts a [`volume_mesh::VolumeMesh`] into a real `outram-foam-basic-lib`
//!   `PolyMesh`, which yields a solvable `FvMesh` via `to_fv_mesh()` — closing
//!   the loop: surface → carve → snap → foam mesh (verified end-to-end).
//! - **Mesh quality checks.** [`checks::check_quality`] reports face
//!   non-orthogonality, skewness, cell aspect ratio, min face area / cell
//!   volume, and negative-volume cells (cfMesh `polyMeshGenChecks`) — the gate
//!   for trusting a generated mesh before it is solved.
//! - **Octree near-wall refinement.** [`octree::refine_near_boundary`] grades
//!   the mesh finer next to the surface, splitting each coarse transition
//!   face into its four fine sub-faces (hanging nodes) so the coarse cell
//!   becomes a genuine **polyhedron** — the mesh stays conforming (verified:
//!   exact volume, closed cells, > 6-face transition cells).
//! - **Prism boundary layers.** [`layers::add_boundary_layers`] inserts graded
//!   near-wall inflation layers at a wall patch (snappyHexMesh *addLayers* /
//!   cfMesh's boundary-layer step): the interior wall points move inward and
//!   the vacated shell is filled with `n_layers` stacked prism cells per wall
//!   face. It is a repartition, so it preserves the mesh volume exactly
//!   (verified: exact volume, closed, `+n_layers × wall_faces` cells). Flat
//!   (box) walls only — the fixed-thickness march self-intersects on curvature.
//!   [`layers::add_boundary_layers_adaptive`] handles **curved / polyhedral
//!   walls** (smoothed normals + per-point thickness limiting + validity
//!   back-off), verified valid + volume-conserving on snapped spheres/cylinders.
//! - **Polyhedral dual.** [`dual::polyhedral_dual`] turns a primal mesh into a
//!   **polyhedral** one — one cell per primal vertex — via the median
//!   (vertex-centred) dual, the equivalent of OpenFOAM's `polyDualMesh`. The
//!   dual tiles the same region (verified: exact volume, closed cells, every
//!   internal face shared by exactly two cells, genuinely > 6-face cells).
//!   Two variants: [`dual::polyhedral_dual`] (robust median, quad-fan faces)
//!   and [`dual::polyhedral_dual_min_faces`] (**face-minimal** — one polygon
//!   per primal edge via edge-star walking, ~40% fewer faces, verified to
//!   conserve volume and stay closed).
//! - **Tetrahedralization.** [`tet::tetrahedralize`] splits every cell into
//!   tetrahedra by centroid subdivision (the all-tet foundation; not a
//!   from-scratch Delaunay mesher). It conserves volume and triangulates the
//!   boundary surface (verified: positive-volume tets, boundary area == input
//!   surface, exact volume, every cell a 4-triangle tet).
//! - **Quality smoothing.** [`smooth::laplacian_smooth`] relaxes interior
//!   vertices toward their neighbour centroid (smart Laplacian — never inverts a
//!   cell, pins the boundary), improving cell shape while conserving volume
//!   exactly (verified: recovers perturbed tet quality, no inversions).
//! - **Flip-based Delaunay.** [`delaunay::flip_to_delaunay`] improves a tet mesh
//!   toward Delaunay by bistellar 2→3 / 3→2 flips (Shewchuk [`delaunay::orient3d`]
//!   / [`delaunay::insphere`] predicates). Volume- and boundary-preserving, with
//!   an *improve-or-noop* guard so it can never make a mesh worse and refuses
//!   tangled input (verified: fixes a non-Delaunay bipyramid exactly, conserves
//!   volume + bounded growth on a block, returns invalid input unchanged).
//! - **High-level pipeline (the recommended entry point).**
//!   [`pipeline::surface_to_tet_dual_mesh`] composes the stages above into one
//!   call: `carve → snap → tetrahedralize → Delaunay-improve → polyhedral dual
//!   → smooth → adaptive prism boundary layers`, driven by
//!   [`pipeline::TetDualOptions`] and returning a [`pipeline::TetDualReport`]
//!   (cell count, volume, quality, and a note for every stage skipped). Each
//!   optional stage is applied only if its result stays valid (closed + no
//!   inverted cells), otherwise it is **gracefully skipped** and the previous
//!   mesh kept, so the returned mesh is always valid and exportable. This is the
//!   coarse-grained meshing entry point the `outram-blender` "mesh studio" GUI
//!   calls; the [`pipeline::box_tet_dual`] / [`pipeline::sphere_tet_dual`] /
//!   [`pipeline::cylinder_tet_dual`] wrappers mesh the built-in primitives.
//!   Lengths in metres, layer thickness in metres, angles in degrees.
//!
//! Next on the `op-hzs` roadmap: exact/adaptive predicates + size-driven point
//! insertion (the rest of `op-38z`; gmsh — GPLv2+, GPLv3-compatible — is a
//! licence-clean reference) and multi-patch / feature-aware layer insertion.
//!
//! ## Design rules (workspace `CLAUDE.md`)
//!
//! Index-based topology (newtype indices into `Vec`, no lifetimes/pointers);
//! enums for dispatch, never trait objects; no `Box<T>` (own by value or share
//! with `Arc<T>`); pure Rust with no BLAS/C/Fortran so the crate builds on
//! Android/Termux.
//!
//! > **Not affiliated with Creative Fields or the cfMesh project**, and not the
//! > official cfMesh software — an independent GPL fork. **Untrusted
//! > AI-assisted draft** until human-reviewed, per the workspace
//! > `RESPONSIBLE_USE.md`. For education / research / V&V only; not for reactor
//! > operation, licensing, or safety-critical decisions.

pub mod cartesian;
pub mod carve;
pub mod checks;
pub mod delaunay;
pub mod dual;
pub mod layers;
pub mod math;
pub mod octree;
pub mod pipeline;
pub mod reactor;
pub mod shapes;
pub mod smooth;
pub mod snap;
pub mod tet;
pub mod volume_mesh;

/// Bridge to the real `outram-foam-basic-lib` `PolyMesh` (feature
/// `foam-export`) — the volume-mesh output path to the CFD/TH solver. Off by
/// default so the base crate stays dependency-free and Android-buildable.
#[cfg(feature = "foam-export")]
pub mod foam;
