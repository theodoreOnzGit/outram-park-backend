//! Choked (critical) two-phase flow.
//!
//! Thin re-export of [`tampines_steam_tables`]'s choked-flow solvers
//! (Homogeneous Equilibrium Model critical flow). V&V status, re-read from
//! that crate's test source on 2026-08-11:
//!
//! - **Moody (1975) Fig. 1** -- verified: 13 active isobar tests
//!   (`moody_critical_mass_flux_homogeneous_eqm.rs`, p0/p_ref = 0.25-30.0)
//!   assert `|log10 G_test - log10 G_ref| <= 0.06` (0.08 for
//!   deeply-subcooled Region-1 points). The validator is region-filtered:
//!   points that are neither in-dome (Region 4) nor deeply subcooled are
//!   skipped as a documented HEM limitation, not asserted.
//! - **Zaloudek** -- verified: ~20+ active tests per file across the
//!   in-dome, subcooled, superheated, generic-dispatcher, and
//!   backward-throat test files (critical-pressure relative tolerance
//!   0.005-0.05 by curve; mass-flux log10 tolerance 0.05). The reference
//!   curves are graph-read HEM curves, not raw experimental data.
//! - **Marviken is NOT validated.** The digitised NUREG/CR-2671 test-23/24
//!   data sits in `marviken_tests.rs`, but the test is
//!   `#[ignore = "skip first, Marviken is more complex"]`, its only
//!   assertion is commented out, and the body ends in `todo!()`. Do not
//!   cite Marviken as a validation basis for these solvers.
//!
//! See that crate's own `CLAUDE.md` and README for the full V&V
//! methodology and results. (Separately, the Edwards-O'Brien blowdown
//! benchmark is implemented and tested in [`crate::multiphase_1d`] and in
//! `tampines-steam-tables`'s `tests/edwards_blowdown.rs` -- a different
//! module and benchmark from the nozzle critical-flow gates above.)
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
