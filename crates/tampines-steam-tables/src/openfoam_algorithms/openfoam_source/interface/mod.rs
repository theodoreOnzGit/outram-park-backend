//! Mesh-construction helpers for TAMPINES pipe/nozzle simulations.
//!
//! Home to [`one_dimensional_meshing`], which builds the 1-D finite-volume
//! meshes needed straight off the bat by the Marviken test and other
//! pipe-flow simulations (discretising a pipe of a given length into a
//! uniform run of cells and faces).
pub mod one_dimensional_meshing;
