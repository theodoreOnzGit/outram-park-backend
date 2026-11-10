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
//!
//! **Since 2026-11-01 (RELAP-7 parity epic gh:#1056) the transient,
//! verified components live beside these:** [`junction`] (the HEM network's
//! flow junctions, volume branches, pump, valve, relief valve, turbine,
//! separator-dryer and downcomer), [`heat_structure`] (PipeWithHeatStructure,
//! CoreChannel, wall heat exchanger), [`reactor_power`] (core power
//! distribution). The steady wrappers below are not their front end.

pub mod condenser;
pub mod cooling_tower;
pub mod drain_tank;
pub mod freeze_valve;
pub mod heat_exchanger;
pub mod heat_structure;
pub mod helical_coil_otsg;
pub mod helical_coil_sg_standalone;
pub mod helical_coil_steam_generator;
pub mod junction;
pub mod pipe;
pub mod pump;
pub mod pump_coastdown;
pub mod reactor_power;
pub mod steam_generator;
pub mod turbine;
pub mod valve;
pub mod wall_condensation;
pub mod wet_well;

pub use condenser::Condenser;
pub use cooling_tower::CoolingTower;
pub use heat_exchanger::HeatExchanger;
pub use heat_structure::{
    ConductionZone, HeatStructure, HeatStructureLink, SideCondition, StructureBoundary, StructureShape,
    StructureSide, ThermalCoupler,
};
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
// The steady 1-D helical-coil once-through steam generator march (gh:#1157):
// coil correlations, Chen boiling, Biasi dryout, IF97 water. A different
// model from the transient `helical_coil_steam_generator` above, whose
// conductances are calibrated and which carries no coil correlation.
pub use helical_coil_otsg::{
    CoilRegion, HelicalCoilGeometry, HelicalCoilOtsg, OperatingPoint, OtsgOptions, OtsgSolution, ProfilePoint,
    Shooting, WaterRegime,
};
pub use junction::{HemJunction, HemNetwork, PipeEndRef, VolumeBranch};
pub use pipe::{Pipe, PipeBackend};
pub use pump::Pump;
pub use pump_coastdown::InertialPumpCoastdown;
pub use reactor_power::{AxialPowerShape, CoreChannelShare, ReactorPowerSource, TotalPower};
pub use steam_generator::SteamGenerator;
pub use turbine::Turbine;
pub use valve::Valve;
// A heat-structure surface facing a gas: MELCOR's natural convection plus
// condensation through non-condensables (gh:#1096), moved from
// `sembawang::bwr_mark1::pool_surface::structure_surface`.
pub use wall_condensation::{CondensateEnthalpy, SurfaceModel, WallCondensation, WallExchange, WallGas};
// The suppression-pool wet well (gh:#1075, RELAP-7 §6.12), ported from
// `sembawang::bwr_mark1`'s MELCOR-based torus.
pub use wet_well::{
    ExternalVolume, ImmersedHeatExchanger, Injection, VacuumBreaker, VacuumBreakerDirection, WetWell,
    WetWellGeometry, WetWellInitialState, WetWellInputs,
};
