//! Control valve component.
//!
//! Composes [`outram_park_fork_dwsim_libs::valve::iec_60534`]'s IEC 60534
//! sizing types behind a TAMPINES-native struct.

use crate::TampinesError;
use outram_park_fork_dwsim_libs::valve::iec_60534::{OpeningCharacteristic, ValveFlowCoefficient};
use uom::si::f64::{MassRate, Pressure, Ratio};

/// A control valve: a maximum flow coefficient, an opening-percentage-to-`Kv`
/// characteristic, and the current opening.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Valve {
    /// Fully-open flow coefficient.
    pub kv_max: ValveFlowCoefficient,
    /// Relationship between opening percentage and `Kv`.
    pub opening_characteristic: OpeningCharacteristic,
    /// Current valve opening, \[0, 100\] percent.
    pub opening_percent: Ratio,
}

impl Valve {
    /// Construct a new valve with the given maximum `Kv`, opening
    /// characteristic, and initial opening.
    pub fn new(
        kv_max: ValveFlowCoefficient,
        opening_characteristic: OpeningCharacteristic,
        opening_percent: Ratio,
    ) -> Self {
        Self { kv_max, opening_characteristic, opening_percent }
    }

    /// This valve's current (opening-adjusted) flow coefficient.
    pub fn current_kv(&self) -> ValveFlowCoefficient {
        self.opening_characteristic.kv_at_opening(self.kv_max, self.opening_percent)
    }

    /// Mass flow rate through this valve for the given upstream/downstream
    /// pressures and upstream fluid density.
    ///
    /// Not yet implemented as a TAMPINES component method -- the underlying
    /// liquid/gas/two-phase sizing algebra already exists and works
    /// ([`outram_park_fork_dwsim_libs::valve::iec_60534`]); this wrapper's
    /// job (picking the right service branch from a real fluid state) is
    /// not yet wired up.
    pub fn mass_flow(
        &self,
        _p1: Pressure,
        _p2: Pressure,
    ) -> Result<MassRate, TampinesError> {
        Err(TampinesError::NotYetImplemented {
            component: "components::valve::Valve::mass_flow",
        })
    }
}
