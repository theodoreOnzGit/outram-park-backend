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
//! | [`mesh`] | `bmesh` (`BMVert`/`BMEdge`/`BMLoop`/`BMFace`) | **real** — index-based half-edge topology |
//! | [`primitives`] | `editors/mesh/editmesh_add` primitive add-ops | **real** — cube / UV-sphere / cylinder / grid generators (unit-tested) |
//! | [`ops`] | `bmesh/operators/*` (`bmo_*`) mesh operators | **stub** — extrude / subdivide / bevel / boolean TODOs |
//! | [`modifiers`] | `modifiers/intern/MOD_*` modifier stack | **stub** — subsurf / mirror / array TODOs |
//! | [`procedural`] | Geometry Nodes (`nodes/geometry/*`) | **stub** — node-graph sketch |
//! | [`export`] | I/O exporters (`io/*`) | **stub** — bridges to `outram-foam-mesh` polyMesh + `outram-mc-libs` CSG |
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

pub mod export;
pub mod math;
pub mod mesh;
pub mod modifiers;
pub mod ops;
pub mod primitives;
pub mod procedural;

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
