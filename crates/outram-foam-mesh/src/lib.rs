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
//! ## Start here
//!
//! If you have a geometry and want a mesh, call one function —
//! [`mesh_from_surface`] — and grade the result with
//! [`assess_quality`](mesh_quality::assess_quality):
//!
//! ```no_run
//! use outram_foam_mesh::{mesh_from_surface, MeshingControls, MeshingPhases};
//! use outram_foam_mesh::snappy_hex_mesh::TriangleSoup;
//! use outram_foam_basic_lib::primitives::Vector3;
//!
//! let surface = TriangleSoup::uv_sphere(Vector3::ZERO, 1.0, 24, 24);
//! let controls = MeshingControls::external_flow([12, 12, 12], 2)
//!     .with_phases(MeshingPhases::CastellateSnap);
//!
//! let result = mesh_from_surface(&surface, &controls).unwrap();
//! println!("{}", result.quality.summary());
//! result.write_case("my_case").unwrap();   // → my_case/constant/polyMesh
//! ```
//!
//! The worked example `examples/sphere_in_box.rs` runs exactly this end to end
//! and is readable top to bottom.
//!
//! ## What belongs here
//!
//! Mesh **construction** and **format conversion** — producing / importing a
//! `polyMesh` (points, faces, owner/neighbour, boundary patches) that the
//! Layer-1–4 crate and the solver crates then operate on. The tools:
//!
//! - [`driver`] — **the high-level entry point**: surface + controls → a
//!   [`PolyMesh`](outram_foam_basic_lib::io::PolyMesh), with the phases chosen
//!   by enum.
//! - [`mesh_quality`] — `checkMesh`: non-orthogonality, skewness, aspect ratio
//!   and inverted-cell count for **any** `polyMesh`, including ones this crate
//!   did not generate.
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
//!
//! ## One mesh currency
//!
//! Each generator has its own working representation, but all of them convert
//! to `outram-foam-basic-lib`'s
//! [`PolyMesh`](outram_foam_basic_lib::io::PolyMesh) — the type that writes
//! `constant/polyMesh` to disk and that [`mesh_quality`] grades:
//!
//! | Produced by | Convert with |
//! |---|---|
//! | [`block_mesh`] | [`block_mesh::PolyMesh::to_foam_poly_mesh`] |
//! | [`snappy_hex_mesh`] | [`snappy_hex_mesh::PolyPatchMesh::to_foam_poly_mesh`] |
//! | [`poly_dual_mesh`] | [`poly_dual_mesh::DualMesh::to_foam_poly_mesh`] |
//! | [`driver`] | already a `PolyMesh` — [`GeneratedMesh::write_case`] |

pub mod block_mesh;
pub mod driver;
pub mod ideas_unv_to_foam;
pub mod mesh_quality;
pub mod poly_dual_mesh;
pub mod snappy_hex_mesh;

// The high-level entry points, re-exported at the crate root so they are the
// first thing a new user meets.
pub use driver::{
    mesh_from_surface, GeneratedMesh, KeepRegion, LayerOutcome, MeshingControls, MeshingPhases,
};
pub use mesh_quality::{assess_quality, MeshQualityReport, QualityVerdict};

/// Errors produced by the mesh utilities in this crate.
#[derive(Debug, thiserror::Error)]
pub enum MeshError {
    /// A dictionary / input file could not be parsed (bad syntax, missing key).
    #[error("mesh dictionary parse error: {0}")]
    DictParse(String),
    /// The input asks for a feature this crate does not support yet. Currently
    /// raised only by [`block_mesh`] for a non-`hex` block shape; deferred dict
    /// features that are safe to ignore (curved `edges`, `mergePatchPairs`) are
    /// skipped silently instead — see the [`block_mesh`] module docs.
    #[error("not yet implemented: {0}")]
    NotImplemented(String),
    /// A geometric / topological inconsistency was detected while building the mesh.
    #[error("mesh construction error: {0}")]
    Construction(String),
    /// A mesh could not be written to (or read from) disk — see
    /// [`GeneratedMesh::write_case`]. Carries the formatted underlying
    /// `outram_foam_basic_lib::io::IoError` plus the path involved.
    #[error("mesh I/O error: {0}")]
    Io(String),
}
