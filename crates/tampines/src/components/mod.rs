//! Balance-of-plant component models.
//!
//! Eight component structs, each composing existing backend types
//! ([`crate::single_phase`], [`crate::compressible`], [`crate::hem`],
//! [`crate::humid_air`], and [`outram_park_fork_dwsim_libs`]'s equipment-
//! model correlations) rather than reimplementing physics. Method bodies
//! that need a real property-package/flash to actually run currently return
//! [`crate::TampinesError::NotYetImplemented`] -- the struct shapes and
//! field composition are the deliverable of this pass; wiring the method
//! bodies to real backend calls is tracked separately (see the workspace's
//! `op-dt3` epic).

pub mod condenser;
pub mod cooling_tower;
pub mod heat_exchanger;
pub mod helical_coil_sg_standalone;
pub mod helical_coil_steam_generator;
pub mod pipe;
pub mod pump;
pub mod steam_generator;
pub mod turbine;
pub mod valve;

pub use condenser::Condenser;
pub use cooling_tower::CoolingTower;
pub use heat_exchanger::HeatExchanger;
// The spatially resolved once-through exchanger, moved here from
// `htgr_sim_v1` on 2026-09-27 (maintainer direction). NOT the same thing as
// `steam_generator::SteamGenerator` below, which is a 46-line stub around the
// lumped DWSIM-derived `HeatExchanger` whose `step` still returns
// `NotYetImplemented`. The two are deliberately left side by side rather than
// merged: this one resolves the exchanger axially and is the one with physics
// in it, and collapsing them would hide which of the two a caller got.
pub use helical_coil_steam_generator::{
    NodalisedCounterFlowSteamGenerator, SteamGeneratorConfig, SteamGeneratorError,
    SteamGeneratorGeometry, SteamGeneratorState,
};
pub use pipe::{Pipe, PipeBackend};
pub use pump::Pump;
pub use steam_generator::SteamGenerator;
pub use turbine::Turbine;
pub use valve::Valve;
