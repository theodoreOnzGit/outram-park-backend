//! Homogeneous Equilibrium Model (HEM) two-phase steam/water state.
//!
//! Thin re-export of [`tampines_steam_tables`]'s validated HEM control-volume
//! type -- the pressure/enthalpy/quality steam-water state used by
//! [`crate::critical_flow`]'s choked-flow solvers and, in future, TAMPINES's
//! wider multiphase thermal-hydraulics work (HEM -> drift-flux -> CHF, see
//! the workspace's `op-21g.6` roadmap bead; deferred stubs for that land
//! separately, not in this module).

/// The HEM two-phase steam/water control volume: pressure, specific
/// enthalpy, specific entropy, quality, and derived properties (viscosity,
/// speed of sound, critical mass flux, ...).
///
/// Alias for [`tampines_steam_tables`]'s `TampinesSteamTableCV` -- see that
/// type's own documentation for its full getter/setter surface and
/// construction paths ((p,h), (p,s), (h,s), (T,p,x), saturation-line
/// constructors).
pub use tampines_steam_tables::prelude::TampinesSteamTableCV as HemSteamCv;
