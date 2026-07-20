//! Heat- and mass-transfer interactions between a single control volume and a
//! boundary condition (constant temperature, constant heat addition, or an
//! advective inflow/outflow).
//!
//! Each interaction adds a power contribution (W) to the control volume's
//! enthalpy-rate vector and, for conduction against a boundary, may register a
//! mesh-stability timestep limit (s). Advective boundary interactions also
//! push a volumetric flowrate (m^3/s) used later for the Courant-number
//! timestep. Constant-temperature boundaries drive the control volume toward
//! the boundary temperature (K); constant-heat-addition boundaries inject a
//! fixed power (W).

/// for advection calculations with heat flux or heat addition BC,
/// the temperature of flows flowing in and out of the BC will be
/// determined by that of the control volume
///
/// it will be the same temperature as that of the control volume
/// at that current timestep
///
/// this will be quite similar to how OpenFOAM treats inflows and outflows
/// at zero gradient BCs
pub mod advection_to_bcs;



/// calculates a conductance interaction between the constant 
/// temperature bc and cv
///
/// for conductance, orientation of bc and cv does not usually matter
pub mod conductance_to_bcs;

/// calculates a conductance interaction between the constant 
/// temperature bc and cv
///
/// for conductance, orientation of bc and cv does not usually matter
pub mod constant_heat_addition_to_bcs;
