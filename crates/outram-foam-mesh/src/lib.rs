//! # outram-foam-mesh
//!
//! OpenFOAM **mesh generation and conversion** utilities, translated to Rust on
//! top of [`outram_foam_basic_lib`]'s primitive + finite-volume layer (`FvMesh`,
//! `polyMesh` topology, points/faces/cells).
//!
//! > **Independent OUTRAM PARK fork, not the official OpenFOAM.** This crate is
//! > not affiliated with, endorsed by, or sanctioned by OpenCFD Ltd. / the
//! > OpenFOAM Foundation / ESI Group. "OpenFOAM" and the tool names
//! > (blockMesh, snappyHexMesh, …) are used only to identify the upstream
//! > algorithms this crate re-implements. See `TRADEMARKS.md`.
//! >
//! > **⚠️ Unverified until validated.** Everything here is a work-in-progress
//! > translation; use at your own risk. Not for nuclear facility operation,
//! > reactor control, safety-critical, or licensing decisions.
//!
//! ## What belongs here
//!
//! Mesh **construction** and **format conversion** — producing / importing a
//! `polyMesh` (points, faces, owner/neighbour, boundary patches) that the
//! Layer-1–4 crate and the solver crates then operate on. The four tools:
//!
//! - [`block_mesh`] — structured hexahedral block meshing from a `blockMeshDict`
//!   (the OpenFOAM `blockMesh` utility).
//! - [`snappy_hex_mesh`] — automatic split-hex meshing around STL surfaces:
//!   castellation (octree refinement), snapping, and boundary layers
//!   (`snappyHexMesh`).
//! - [`ideas_unv_to_foam`] — import an I-DEAS `.unv` (UNV) mesh into `polyMesh`
//!   (`ideasUnvToFoam`).
//! - [`poly_dual_mesh`] — construct the polyhedral dual of a mesh
//!   (`polyDualMesh`).
//!
//! Solver loops, turbulence models, and thermophysics do **not** belong here —
//! they live in the solver crates and `outram-foam-basic-lib`.

pub mod block_mesh;
pub mod ideas_unv_to_foam;
pub mod poly_dual_mesh;
pub mod snappy_hex_mesh;

/// Errors produced by the mesh utilities in this crate.
#[derive(Debug, thiserror::Error)]
pub enum MeshError {
    /// A dictionary / input file could not be parsed (bad syntax, missing key).
    #[error("mesh dictionary parse error: {0}")]
    DictParse(String),
    /// The requested feature is scaffolded but not yet implemented.
    #[error("not yet implemented: {0}")]
    NotImplemented(String),
    /// A geometric / topological inconsistency was detected while building the mesh.
    #[error("mesh construction error: {0}")]
    Construction(String),
}
