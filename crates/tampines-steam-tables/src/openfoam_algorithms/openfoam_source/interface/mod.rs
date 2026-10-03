//! Mesh-construction helpers for TAMPINES pipe/nozzle simulations.
//!
//! Home to [`one_dimensional_meshing`], which builds the 1-D finite-volume
//! meshes needed straight off the bat by the Marviken test and other
//! pipe-flow simulations (discretising a pipe of a given length into a
//! uniform run of cells and faces).
// DEDUPED 2026-10-03 (GitHub #492): this module's 1-D mesher were a copy of
// `outram-foam-basic-lib`'s, code-identical apart from doc comments (or, for
// `FvMesh`/`MeshError`, a strict subset of it: foam-basic-lib adds the
// cyclic/AMI fields and checks, which this crate never sets, so every mesh
// built here behaves identically). foam-basic-lib is the one copy; these are
// re-exports. Merging `FvMesh` alone was not possible: its fields are
// foam-basic-lib `Vector3`s, so the primitives had to come with it.
pub use outram_foam_basic_lib::interface::one_dimensional_meshing;
