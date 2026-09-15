//! Heat-transfer correlations used by the TUAS Boussinesq solver.
//!
//! This module groups the empirical and analytical relations that turn
//! geometry, fluid state, and flow conditions into heat-transfer quantities:
//!
//! - [`nusselt_number_correlations`] — dimensionless Nusselt number `Nu`
//!   correlations for convection (pipe flow, packed beds), stating the
//!   laminar/turbulent regime and Reynolds/Prandtl validity of each.
//! - [`thermal_resistance`] — 1D conduction/convection thermal resistance
//!   (kelvin per watt) and conductance (watts per kelvin), returning heat
//!   flow (watts) for a given temperature difference.
//! - [`view_factors`] — dimensionless radiation view factors (concentric
//!   cylinders) for the clamshell radiative heater geometry.
//! - [`parallel_heat_exchangers`] — log-mean-temperature-difference (LMTD)
//!   based heat duty for parallel-piped heat exchangers.
//! - [`heat_transfer_interactions`] — dispatch layer that, given the shape of
//!   two control volumes (slab / cylinder / sphere) and an interaction type
//!   (conduction, convection, advection, radiation), computes the conductance
//!   or heat flow between them.
//!
//! Anything that is not a heat-transfer correlation (fluid mechanics,
//! thermophysical properties, mesh/geometry primitives) belongs in the
//! sibling modules, not here.

/// correlations for convective heat transfer
pub mod nusselt_number_correlations;

/// basic calculation functions for thermal resistance
pub mod thermal_resistance;

/// view factor functions for radiative heat transfer
pub mod view_factors;

/// calculations for parallel piped heat exchangers
pub mod parallel_heat_exchangers;

/// heat transfer interactions between different shapes
/// of control volumes are calculated here
pub mod heat_transfer_interactions;
