//! Shared heat-transfer correlation access point.
//!
//! Thin re-export of the Nusselt-number correlation dispatch enum from
//! [`tuas_boussinesq_solver`] (used by [`crate::single_phase`]'s lumped
//! pipe/component model) and the thermal-conductivity/viscosity transport
//! functions from [`outram_park_fork_coolprop`] (used by
//! [`crate::compressible`]'s finite-volume model). ~~This module adds no new
//! correlations~~ **CORRECTED 2026-11-01 (GitHub #1063):** it now also holds
//! [`relap5_htc`], the RELAP5 geometry-101/110 single-phase wall
//! heat-transfer selection, which adds the Seban–Shimazaki, Inayatov and
//! Kazimi–Carelli forms (its Dittus–Boelter and Churchill–Chu legs come
//! from `tuas_boussinesq_solver`). See each backend crate's own
//! documentation for the re-exported correlations' physics and validated
//! ranges.

/// Dispatches which Nusselt-number correlation a single-phase pipe/component
/// segment uses (Gnielinski, Dittus-Boelter-derived, Wakao, CIET-specific,
/// fixed, ...).
///
/// Alias for [`tuas_boussinesq_solver`]'s `NusseltCorrelation`.
pub use tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation;

/// Error type returned by [`NusseltCorrelation`]'s methods. Re-exported from
/// `tuas_boussinesq_solver` for convenience.
pub use tuas_boussinesq_solver::tuas_lib_error::TuasLibError;

/// Dynamic viscosity `μ` \[Pa·s\] of a CoolProp-backed fluid at temperature
/// `T` \[K\] and mass density `ρ` \[kg/m³\], or `None` if that fluid has no
/// supported viscosity model. Re-exported from `outram-park-fork-coolprop`.
pub use outram_park_fork_coolprop::viscosity;

/// Thermal conductivity `λ` \[W/(m·K)\] of a CoolProp-backed fluid at
/// temperature `T` \[K\] and mass density `ρ` \[kg/m³\], or `None` if that
/// fluid has no supported conductivity model. Re-exported from
/// `outram-park-fork-coolprop`.
pub use outram_park_fork_coolprop::conductivity;

/// Which viscosity/conductivity correlation backs a given
/// [`outram_park_fork_coolprop::Fluid`] (48 of 137 fluids have one).
/// Re-exported from `outram-park-fork-coolprop`.
pub use outram_park_fork_coolprop::FluidTransport;

/// RELAP5 geometry-101/110 single-phase wall heat-transfer selection
/// (laminar / Dittus–Boelter / Churchill–Chu maximum; Seban–Shimazaki and
/// Kazimi–Carelli liquid-metal forms; Inayatov bundle multiplier). GitHub
/// #1063.
pub mod relap5_htc;

/// Dittus–Boelter inputs and the heating/cooling selector, re-exported
/// from `tuas_boussinesq_solver` (the `NusseltCorrelation::DittusBoelter`
/// variant's data).
pub use tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::{
    DittusBoelterData, FluidHeatingDirection,
};

/// Churchill–Chu vertical-plate free convection, with the Grashof and
/// Rayleigh numbers, re-exported from `tuas_boussinesq_solver`.
pub use tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::{
    churchill_chu_vertical_plate_nusselt, grashof_number, rayleigh_number, ChurchillChuData,
};
