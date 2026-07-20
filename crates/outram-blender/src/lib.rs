//! # outram-blender
//!
//! A pure-Rust, headless **mesh-authoring frontend** for the OUTRAM PARK
//! multiphysics suite, inspired by the **architecture** of
//! [Blender](https://github.com/blender/blender) (GPLv2-or-later, which is
//! GPLv3-compatible). The eventual goal is to author and procedurally generate
//! geometry that feeds the OUTRAM PARK solvers — an `outram-foam-mesh`
//! `polyMesh` for CFD, or an `outram-mc-libs` CSG universe for Monte Carlo
//! neutron transport.
//!
//! > **⚠️ Scaffold, not a Blender port.** Blender is millions of lines of
//! > C/C++/Python; this crate borrows its *concepts and data-structure
//! > architecture* (the BMesh half-edge topology, the mesh-operator model, the
//! > modifier stack, geometry-nodes-style procedural generation) — it does
//! > **not** port Blender's code. Where a real algorithm is implemented
//! > (currently only [`primitives`]), it is written from first principles and
//! > unit-tested. Everything else is an honest, documented `TODO` stub.
//! >
//! > **Not affiliated with the Blender Foundation.** "Blender" names the
//! > upstream project whose architecture inspired this work; nothing here is
//! > endorsed by or sanctioned by the Blender Foundation. See the README's
//! > "Naming & trademark" section — the crate name itself is a pending
//! > maintainer decision.
//! >
//! > **Untrusted AI-generated draft** until a human reviews it, per the
//! > workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
//! > reactor control, safety-critical, or licensing decisions.
//!
//! ## Module map — what belongs where
//!
//! | Module | Blender analogue | Status |
//! |---|---|---|
//! | [`math`] | `blenlib` `BLI_math` vector types | **real** — a minimal pure-Rust [`math::Vec3`] |
//! | [`transform`] | `Object.matrix_world` affine placement | **real** — [`transform::Affine3`] per-vertex transform (CPU reference for the GPU kernel) |
//! | [`mesh`] | `bmesh` (`BMVert`/`BMEdge`/`BMLoop`/`BMFace`) | **real** — index-based half-edge topology |
//! | [`primitives`] | `editors/mesh/editmesh_add` primitive add-ops | **real** — cube / UV-sphere / cylinder / grid generators (unit-tested) |
//! | [`ops`] | `bmesh/operators/*` (`bmo_*`) mesh operators | **real** — extrude / midpoint-subdivide / vertex-bevel (flat or rounded multi-segment; boolean delegates to [`boolean`]) |
//! | [`subdivision`] | OpenSubdiv / `MOD_subsurf` | **real** — Catmull-Clark surface subdivision (local stencils) |
//! | [`boolean`] | `bmo_boolean` (Manifold upstream) | **real** — CSG entry point: exact convex-`Intersect` fast path, else delegates to [`boolean_general`] |
//! | [`boolean_general`] | `mesh_boolean.cc` / `mesh_intersect.cc` arrangement | **real** — general union/difference/intersect on non-convex closed meshes (arrangement + winding classification) |
//! | [`boolean_predicates`] | `blenlib` `math_boolean.cc` (Shewchuk) | **real** — robust `orient2d/3d`, `incircle`, `insphere` (adaptive f64 + double-double fallback) |
//! | [`boolean_classify`] | `mesh_boolean.cc` inside/outside classification | **real** — point-in-closed-mesh via generalized winding number (+ ray-parity cross-check) |
//! | [`modifiers`] | `modifiers/intern/MOD_*` modifier stack | **real** — subsurf / mirror / array |
//! | [`procedural`] | Geometry Nodes (`nodes/geometry/*`) | **real** — node-graph evaluator |
//! | [`export`] | I/O exporters (`io/*`) | **real** — OpenFOAM polyMesh text + CSG fitting (box/sphere/cylinder/convex-faceted) + DAGMC faceted-solid route |
//!
//! ## Design rules honoured here (workspace `CLAUDE.md`)
//!
//! - **Index-based topology, no lifetimes/pointers.** Every element is
//!   addressed by a newtype index ([`mesh::VertexId`], [`mesh::EdgeId`],
//!   [`mesh::LoopId`], [`mesh::FaceId`]) into a `Vec`, exactly as the workspace
//!   forbids `&'a`-linked graph nodes.
//! - **Enums for dispatch, never trait objects.** The operator, modifier, and
//!   procedural-node sets are closed and enumerated ([`ops::MeshOp`],
//!   [`modifiers::Modifier`], [`procedural::GeometryNode`]).
//! - **No `Box<T>`; `Arc<T>` for sharing.** Owned meshes are passed by value;
//!   shared read-only meshes use `std::sync::Arc`.
//!
//! ## Where to start reading
//!
//! [`primitives`] is the primary entry point — it is the only module with
//! runnable code. Read [`primitives::cube`] top-to-bottom, then the
//! [`mesh::Mesh`] type it builds on.
//!
//! ```
//! use outram_blender::primitives;
//!
//! // A unit cube centred at the origin: 8 vertices, 12 edges, 6 quad faces.
//! let cube = primitives::cube(1.0);
//! assert_eq!(cube.vertex_count(), 8);
//! assert_eq!(cube.edge_count(), 12);
//! assert_eq!(cube.face_count(), 6);
//! // Euler characteristic of a closed genus-0 surface: V - E + F = 2.
//! assert_eq!(cube.euler_characteristic(), 2);
//! ```

pub mod boolean;
pub mod boolean_classify;
pub mod boolean_general;
pub mod boolean_predicates;
pub mod export;
pub mod math;
pub mod mesh;
pub mod modifiers;
pub mod ops;
pub mod primitives;
pub mod procedural;
pub mod subdivision;
pub mod transform;

/// Heavy linear-algebra backend for the *large* mesh solves the advanced
/// operators will need — Laplacian mesh editing, ARAP deformation, and mesh
/// parameterization all build a sparse Laplacian over the mesh and solve
/// `A x = b`. Re-exports [`faer`], a pure-Rust, Android-safe dense **and**
/// sparse linear-algebra library (SIMD via `pulp`, no system BLAS).
///
/// **Division of labour:** per-element geometry math (positions, normals,
/// transforms) stays in the fixed-size [`math`] types — small, fast, no
/// allocation. `faer` is only for the big systems. For interactive editing
/// (same matrix, many right-hand sides) prefer `faer`'s sparse **Cholesky**
/// factorization over an iterative solve. An *optional* bridge to
/// `outram-foam-basic-lib`'s CG/GAMG iterative solvers is tracked separately for
/// large one-off sparse solves (see beads `op-hzs`). None of this is wired into
/// an operator yet — the dependency is staged for that work.
pub use faer;

/// Headless GPU compute via `wgpu`. Compiled **unconditionally on every desktop
/// target** (no cargo feature to opt in) so the GPU path is used as far as
/// possible; **absent only on Android** (`target_os = "android"`), which has no
/// system Vulkan/Metal loader and where the workspace Android rule forbids GPU
/// deps in the library build. Whether or not this module is present, callers get
/// a graceful CPU fallback: on Android the GPU attempt is compiled out entirely,
/// and on desktop [`gpu::probe`] returning `None` or a recoverable
/// [`gpu::GpuError`] routes to the CPU reference path. See
/// [`transform::Affine3::transform_points_best_effort`] for the unified
/// try-GPU-then-CPU entry point, and [`gpu`] for the fallback contract.
#[cfg(not(target_os = "android"))]
pub mod gpu;
