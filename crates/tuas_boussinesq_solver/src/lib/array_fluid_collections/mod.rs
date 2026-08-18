//! Array control volumes and fluid-component collections.
//!
//! This module groups the building blocks TUAS uses to model spatially
//! resolved (1D) thermal-hydraulic components and networks of them:
//!
//! - Standalone fluid/solid node arrays (`standalone_fluid_nodes`,
//!   `standalone_solid_nodes`) — bare matrix solves over `SingleCVNode`
//!   structs, with no owning array-CV abstraction.
//! - Thermal-conductance array timestep solvers (`conductance_array_functions`)
//!   that advance the internal temperature profile (in K) of a lumped
//!   1D conductor given inner-node and outer-boundary conditions.
//! - Fully abstracted 1D array control volumes for solids and fluids
//!   (`one_dimension_cartesian_conducting_medium`,
//!   `solid_array_lateral_coupling`,
//!   `fluid_array_lateral_coupling`) that hide the matrix
//!   bookkeeping and can be coupled laterally (radially) to form 2D/3D
//!   lattices.
//! - Fluid-component collections (`fluid_component_collection`) for solving
//!   pressure drop (Pa) and mass flow rate (kg/s) across pipes arranged in
//!   series or parallel.

/// contains matrix calculations specific to fluid nodes
/// arranged in a 1D array
///
/// These are standalone, and not abstracted under an arrayCV
/// struct, they only use SingleCVNode structs and representative
/// arrays to represent an array of control volumes
pub mod standalone_fluid_nodes;

/// contains matrix calculations specific to solid nodes
/// these are meant to represent the "shell" of the pipe
/// or any kind of solid material in the pipe
///
/// These are standalone, and not abstracted under an arrayCV
/// struct, they only use SingleCVNode structs and representative
/// arrays to represent an array of control volumes
pub mod standalone_solid_nodes;

/// Thermal-conductance array timestep solvers: advance the internal
/// temperature profile (in kelvin) of a lumped 1D conductor over one
/// timestep, solving an implicit conductance-matrix / power-vector system
/// given an inner temperature node and an outer cooling boundary condition.
pub mod conductance_array_functions;

/// contains a full struct which abstracts away calculation details
///
/// this is relevant for one dimension cartesian (x,y,z) coordinates
/// you can't really couple these arrays laterally though
pub mod one_dimension_cartesian_conducting_medium;

/// contains a full struct which abstracts away calculation details
/// 1 dimensional solid arrays
///
/// this is relevant for one dimension cartesian (x,y,z) coordinates
/// except that you can couple these arrays laterally to form a 2D or
/// 3D lattice
pub mod solid_array_lateral_coupling;

/// contains a full struct which abstracts away calculation details
/// 1 dimensional fluid arrays
///
/// this is relevant for one dimension cartesian (x,y,z) coordinates
/// except that you can couple these arrays laterally to form a 2D or
/// 3D lattice
pub mod fluid_array_lateral_coupling;

/// contains code for calculating pressure drop and mass flowrates over
/// pipes in series or parallel
pub mod fluid_component_collection;
