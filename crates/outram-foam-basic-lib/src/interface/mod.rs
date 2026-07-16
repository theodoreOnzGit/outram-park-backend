//! User-facing helpers for building meshes and fields without hand-assembling
//! the low-level [`crate::mesh`] and [`crate::fields`] data structures.
//!
//! Currently this provides [`one_dimensional_meshing`], a generator for the
//! uniform 1-D pipe meshes used by pipe-flow and steam-table (e.g. Marviken)
//! simulations.

/// now, for the TAMPINES steam tables Marviken test,
/// and other pipe simulations, I will often need to make
/// one dimensional meshes straight off the bat,
///
///
pub mod one_dimensional_meshing;
