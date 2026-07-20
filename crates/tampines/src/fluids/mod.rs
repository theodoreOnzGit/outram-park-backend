//! Fluid selection unified across TAMPINES's backend property libraries.
//!
//! This module belongs to the *fluid-selection* concern only -- picking which
//! backend evaluates a fluid's thermophysical properties. It does not itself
//! hold flow/loop state; see [`crate::single_phase`] and
//! [`crate::compressible`] for the component-level wrappers that use a
//! [`TampinesFluid`] to construct a loop segment.

use outram_park_fork_coolprop::Fluid as CoolPropFluid;

/// Which backend thermophysical-property source a TAMPINES fluid is
/// evaluated with.
///
/// This is a *property-backend* selector, not an independent substance list:
/// [`TampinesFluid::CoolProp`] carries an
/// [`outram_park_fork_coolprop::Fluid`] (137 fluids, including water, air,
/// nitrogen, helium, ...); [`TampinesFluid::Steam`] instead routes to the
/// dedicated IAPWS-IF97 `tampines-steam-tables` backend, since that is this
/// workspace's own validated water/steam implementation (steam-turbine and
/// choked-flow equations against Kretzschmar & Wagner 2019 and Moody/Zaloudek/
/// Marviken references) rather than CoolProp's own water equation of state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TampinesFluid {
    /// Evaluate via the CoolProp-derived backend (`outram-park-fork-coolprop`).
    CoolProp(CoolPropFluid),
    /// Evaluate water/steam via the IAPWS-IF97 `tampines-steam-tables` backend.
    Steam,
}
