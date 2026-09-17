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
pub mod pipe;
pub mod pump;
pub mod steam_generator;
pub mod turbine;
pub mod valve;

pub use condenser::Condenser;
pub use cooling_tower::CoolingTower;
pub use heat_exchanger::HeatExchanger;
pub use pipe::{Pipe, PipeBackend};
pub use pump::Pump;
pub use steam_generator::SteamGenerator;
pub use turbine::Turbine;
pub use valve::Valve;
