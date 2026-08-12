//! Balance-of-plant grouping.
//!
//! Re-exports the BOP-relevant [`crate::components`] types under one module
//! for discoverability, plus system-level (multi-component) assembly types
//! like [`RankineCycle`]. This module adds no new leaf physics -- it is
//! composition only; each component's own (currently stubbed) method is
//! still where its physics lives.
//!
//! [`crate::components::Pipe`] and [`crate::components::CoolingTower`] are
//! not re-exported here: `Pipe` is general-purpose (not BOP-specific), and
//! cooling towers have their own dedicated grouping, [`crate::cooling_tower`].

pub use crate::components::{Condenser, HeatExchanger, Pump, SteamGenerator, Turbine, Valve};

/// A basic Rankine-cycle assembly: steam generator, turbine, condenser, and
/// feedwater pump in series.
///
/// Grouping/composition only -- no whole-cycle calculation logic yet. Each
/// component's own method (e.g. [`Turbine::expand_to`], [`Condenser::condense`])
/// is still the entry point for that component's physics; a cycle-level
/// `run`/`step` that threads state between them is future work.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RankineCycle {
    /// Boiler / steam generator.
    pub steam_generator: SteamGenerator,
    /// Steam turbine.
    pub turbine: Turbine,
    /// Condenser.
    pub condenser: Condenser,
    /// Feedwater (condensate return) pump.
    pub feedwater_pump: Pump,
}

impl RankineCycle {
    /// Assemble a Rankine cycle from its four main components.
    pub fn new(
        steam_generator: SteamGenerator,
        turbine: Turbine,
        condenser: Condenser,
        feedwater_pump: Pump,
    ) -> Self {
        Self {
            steam_generator,
            turbine,
            condenser,
            feedwater_pump,
        }
    }
}
