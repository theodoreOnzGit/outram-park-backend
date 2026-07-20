//! Rust-side extensions on top of the vendored OpenFOAM primitives — helpers
//! that OpenFOAM's C++ tree does not provide but the TAMPINES/system-code use
//! cases need. Currently: [`one_dimensional_meshing::create_one_d_mesh`], which
//! builds a uniform 1-D `FvMesh` from a length, cross-sectional area and cell
//! count (used to drive pipe simulations such as the Marviken test and the
//! `OPCPFluidArray` solver) rather than reading an OpenFOAM `polyMesh`.

/// Generates 1-D finite-volume meshes for pipe / system-code simulations
/// (e.g. the TAMPINES steam-tables Marviken test) directly from a length,
/// area and cell count.
pub mod one_dimensional_meshing;
