//! Compressible pipe flow / gas-cooled thermal-hydraulics.
//!
//! Re-exports [`outram_park_fork_coolprop`]'s finite-volume compressible
//! pipe-flow model (a `rhoPimpleFoam`-derived solver) under a
//! TAMPINES-local name. This is the natural backend for HTGR primary loops,
//! gas-cooled transients, and any fluid CoolProp backs but
//! [`crate::single_phase`]'s TUAS backend does not (water, air, helium, ...).
//!
//! This is a thin wiring module: it does not add behaviour beyond naming.
//! The full TAMPINES fluid-array interface (unifying this with
//! [`crate::single_phase`] and the steam-tables backend behind one enum) is
//! tracked separately -- see the workspace's beads issue tracker, epic
//! `op-dt3`.

/// A compressible pipe-flow loop segment (finite-volume, CoolProp-backed
/// thermophysical properties, lateral thermal coupling + heat source).
///
/// Alias for [`outram_park_fork_coolprop`]'s `OPCPFluidArray` -- see that
/// type's own documentation for the underlying physics and its `new`
/// constructor.
///
/// Note: after calling `step()`, `t`/`rho` lag `he` (specific enthalpy) by
/// one outer-corrector iteration -- call `correct_thermo()` afterwards if a
/// caller needs `t`/`rho` consistent with the just-solved `he`.
pub use outram_park_fork_coolprop::OPCPFluidArray as CompressibleFluidArray;

/// Error type returned by [`CompressibleFluidArray`] methods. Re-exported
/// from `outram-park-fork-coolprop` for convenience.
pub use outram_park_fork_coolprop::OPCPFluidArrayError as CompressibleFluidArrayError;

/// The fluid substances a [`CompressibleFluidArray`] can be backed by.
/// Re-exported from `outram-park-fork-coolprop` for convenience.
pub use outram_park_fork_coolprop::Fluid as CoolPropFluid;
