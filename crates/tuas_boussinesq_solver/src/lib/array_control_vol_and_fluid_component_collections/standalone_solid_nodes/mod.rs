//! Standalone solid-node arrays with radial conduction only (no axial term).
//!
//! These are one-dimensional arrays of solid control volumes (e.g. a pipe or
//! shell wall) advanced one timestep by an implicit energy balance that couples
//! each node radially to adjacent boundaries through thermal-conductance arrays
//! (W/K per node); axial conduction along the array is neglected. All node
//! temperatures are in kelvin and optional volumetric heat generation q is in W.
//!
//! What belongs here: `shell_solid_node` (a solid node between an inner and an
//! outer boundary) and `core_solid_nodes` (a solid node with one boundary, the
//! other treated as adiabatic — implemented by delegating to the shell solver
//! with a zero-conductance side). The temperature solve reuses the
//! conductance-matrix solver from the sibling `standalone_fluid_nodes` module.

/// code for solving solid nodes connected to two
/// adjacent boundaries:
///
/// an inner boundary 
/// and an outer boundary
pub mod shell_solid_node;

/// code for solving solid nodes connected to one adjacent boundary 
pub mod core_solid_nodes;
