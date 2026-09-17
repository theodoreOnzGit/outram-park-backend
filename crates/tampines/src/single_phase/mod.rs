//! Lumped single-phase liquid thermal-hydraulics.
//!
//! Re-exports [`tuas_boussinesq_solver`]'s finite-volume lumped-parameter
//! single-phase pipe/component model under a TAMPINES-local name. TUAS's
//! backend fluid list ([`LiquidMaterial`]) currently covers molten-salt and
//! thermal-oil loops (TherminolVP1, DowthermA, HITEC, YD325, FLiBe, ...) --
//! it does **not** yet back water, air, or helium. For those fluids, use
//! [`crate::compressible`] (CoolProp-backed) instead until TUAS grows
//! matching liquid coverage.
//!
//! This is a thin wiring module: it does not add behaviour beyond naming.
//! The full TAMPINES fluid-array interface (unifying this with
//! [`crate::compressible`] and the steam-tables backend behind one enum) is
//! tracked separately -- see the workspace's beads issue tracker, epic
//! `op-dt3`.

/// A single-phase liquid loop segment (1D lumped finite-volume pipe/component
/// with lateral thermal coupling).
///
/// Alias for [`tuas_boussinesq_solver`]'s `FluidArray` -- see that type's own
/// documentation for the underlying physics and constructors
/// (`new_cylinder`, `new_annular_cylinder`, `new_odd_shaped_pipe`,
/// `new_custom_component`).
pub use tuas_boussinesq_solver::array_fluid_collections::fluid_array_lateral_coupling::FluidArray as SinglePhaseFluidArray;

/// The liquid substances a [`SinglePhaseFluidArray`] can currently be backed
/// by. Re-exported from `tuas_boussinesq_solver` for convenience.
pub use tuas_boussinesq_solver::boussinesq_thermophysical_properties::LiquidMaterial;

/// Error type returned by [`SinglePhaseFluidArray`] methods. Re-exported from
/// `tuas_boussinesq_solver` for convenience.
pub use tuas_boussinesq_solver::tuas_lib_error::TuasLibError;
