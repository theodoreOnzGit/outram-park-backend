//! Choked (critical) two-phase flow.
//!
//! Thin re-export of [`tampines_steam_tables`]'s validated choked-flow
//! solvers (Homogeneous Equilibrium Model critical flow, validated against
//! Moody, Zaloudek, and Marviken reference data -- see that crate's own
//! `CLAUDE.md` and README for the V&V methodology and results).
//! [`crate::hem`] provides the underlying two-phase state type these
//! solvers operate on.

use tampines_steam_tables::steam_turbine_equations::converging_diverging_nozzles::choked_flow;

/// Critical (choked) pressure and mass flux for a stagnation state `(p0,
/// h0)` anywhere relative to the vapour-liquid-equilibrium dome --
/// subcooled liquid, two-phase (in-dome), or superheated/supercritical
/// vapour. Unified dispatcher; routes internally by the stagnation state's
/// IAPWS-IF97 flash region.
///
/// Alias for [`tampines_steam_tables`]'s
/// `get_critical_pressure_and_mass_flux_multiphase_ph`.
pub use choked_flow::get_critical_pressure_and_mass_flux_multiphase_ph as critical_pressure_and_mass_flux;

/// Mass flow rate and downstream thermodynamic state ([`crate::hem::HemSteamCv`])
/// for choked flow through a converging-diverging nozzle throat, given the
/// stagnation state and throat area.
///
/// Alias for [`tampines_steam_tables`]'s
/// `get_choked_flow_massrate_and_state_from_stagnation_properties_and_area`.
pub use tampines_steam_tables::prelude::get_choked_flow_massrate_and_state_from_stagnation_properties_and_area as choked_massrate_and_state;
