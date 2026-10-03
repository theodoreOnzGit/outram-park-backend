//! Rust-side extensions on top of the vendored OpenFOAM primitives — helpers
//! that OpenFOAM's C++ tree does not provide but the TAMPINES/system-code use
//! cases need. Currently: [`one_dimensional_meshing::create_one_d_mesh`], which
//! builds a uniform 1-D `FvMesh` from a length, cross-sectional area and cell
//! count (used to drive pipe simulations such as the Marviken test and the
//! `OPCPFluidArray` solver) rather than reading an OpenFOAM `polyMesh`.

/// Generates 1-D finite-volume meshes for pipe / system-code simulations
/// (e.g. the TAMPINES steam-tables Marviken test) directly from a length,
/// area and cell count.
// DEDUPED 2026-10-03 (GitHub #492): this module's 1-D mesher were a copy of
// `outram-foam-basic-lib`'s, code-identical apart from doc comments (or, for
// `FvMesh`/`MeshError`, a strict subset of it: foam-basic-lib adds the
// cyclic/AMI fields and checks, which this crate never sets, so every mesh
// built here behaves identically). foam-basic-lib is the one copy; these are
// re-exports. Merging `FvMesh` alone was not possible: its fields are
// foam-basic-lib `Vector3`s, so the primitives had to come with it.
pub use outram_foam_basic_lib::interface::one_dimensional_meshing;
