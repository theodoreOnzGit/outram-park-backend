//! Generic heat exchanger component.
//!
//! Composes [`outram_park_fork_dwsim_libs::heat_exchanger`]'s LMTD/epsilon-NTU
//! building blocks behind a TAMPINES-native struct. For a two-phase
//! (boiling/condensing) secondary side, see [`crate::components::steam_generator`]
//! and [`crate::components::condenser`], which wrap this crate's own
//! [`crate::hem::HemSteamCv`] instead.

use crate::TampinesError;
use outram_park_fork_dwsim_libs::heat_exchanger::lmtd::FlowArrangement;
use uom::si::f64::{Area, HeatTransfer, Power, ThermodynamicTemperature};

/// A heat exchanger: flow arrangement, heat-transfer area, and overall
/// heat-transfer coefficient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatExchanger {
    /// Co-current or counter-current flow arrangement.
    pub arrangement: FlowArrangement,
    /// Heat-transfer area.
    pub area: Area,
    /// Overall heat-transfer coefficient `U`.
    pub overall_coefficient: HeatTransfer,
}

impl HeatExchanger {
    /// Construct a new heat exchanger with the given arrangement, area, and
    /// overall coefficient.
    pub fn new(
        arrangement: FlowArrangement,
        area: Area,
        overall_coefficient: HeatTransfer,
    ) -> Self {
        Self {
            arrangement,
            area,
            overall_coefficient,
        }
    }

    /// Duty and outlet temperatures for the given inlet temperatures.
    ///
    /// Not yet implemented -- the underlying LMTD/epsilon-NTU algebra
    /// already exists and works
    /// ([`outram_park_fork_dwsim_libs::heat_exchanger::lmtd`]/
    /// [`outram_park_fork_dwsim_libs::heat_exchanger::ntu_effectiveness`]);
    /// this wrapper's job (threading through real fluid states and their
    /// heat-capacity rates) is not yet wired up.
    pub fn calculate(
        &self,
        _t_hot_in: ThermodynamicTemperature,
        _t_cold_in: ThermodynamicTemperature,
    ) -> Result<(Power, ThermodynamicTemperature, ThermodynamicTemperature), TampinesError> {
        Err(TampinesError::NotYetImplemented {
            component: "components::heat_exchanger::HeatExchanger::calculate",
        })
    }
}
