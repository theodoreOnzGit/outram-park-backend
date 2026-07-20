//! Mesh-refinement (spatial/temporal convergence) study for the idealised
//! isolated DRACS natural-circulation loop.
//!
//! Re-runs the zero-parasitic isolated DRACS loop at 2x, 5x, 10x and 20x the
//! baseline SAM nodalisation, with correspondingly smaller timesteps, to check
//! that the steady-state loop mass flow rate (kg/s) is converged with respect
//! to mesh and timestep. This addresses a TUAS-paper reviewer comment that
//! discretisation was not considered as a source of error.

/// mesh refinement 2 times that of SAM
/// nodalisation
///
/// timestep 0.25s
mod mesh_refinement_two_times;

/// mesh refinement 5 times that of SAM
/// nodalisation
///
/// timestep 0.1s
mod mesh_refinement_five_times;


/// mesh refinement 10 times that of SAM
/// nodalisation
///
/// timestep 0.05s
mod mesh_refinement_10_times;


/// mesh refinement 20 times that of SAM
/// nodalisation
///
/// timestep 0.025s
mod mesh_refinement_20_times;
