//! Cooling tower component.
//!
//! DWSIM has no dedicated cooling-tower unit-op source (only a saved FOSSEE
//! flowsheet using existing generic blocks, not a unit-op class -- see the
//! workspace's `op-dt3.10` bead notes). This component instead wraps
//! [`crate::humid_air`], TAMPINES's own psychrometrics module; the
//! Merkel-method/NTU cooling-tower physics itself is new (no DWSIM
//! equivalent to port).

use crate::humid_air::HumidAirState;
use crate::TampinesError;
use uom::si::f64::{TemperatureInterval, ThermodynamicTemperature, VolumeRate};

/// A cooling tower: ambient air inlet state, circulating-water inlet
/// temperature and flow rate, and a target approach temperature (the
/// smallest achievable difference between the water outlet temperature and
/// the air's wet-bulb temperature).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoolingTower {
    /// Ambient air inlet state.
    pub air_inlet: HumidAirState,
    /// Circulating-water inlet temperature.
    pub water_inlet_temperature: ThermodynamicTemperature,
    /// Circulating-water volumetric flow rate.
    pub water_flow_rate: VolumeRate,
    /// Target approach temperature (`T_water,out - T_wet_bulb,air`).
    pub target_approach: TemperatureInterval,
}

impl CoolingTower {
    /// Construct a new cooling tower with the given air inlet, water inlet
    /// conditions, and target approach temperature.
    pub fn new(
        air_inlet: HumidAirState,
        water_inlet_temperature: ThermodynamicTemperature,
        water_flow_rate: VolumeRate,
        target_approach: TemperatureInterval,
    ) -> Self {
        Self {
            air_inlet,
            water_inlet_temperature,
            water_flow_rate,
            target_approach,
        }
    }

    /// Evaluate this cooling tower's water outlet temperature and air
    /// outlet state.
    ///
    /// Not yet implemented -- needs a Merkel-method/NTU cooling-tower model,
    /// which is new physics with no DWSIM equivalent to draw on (see this
    /// module's doc).
    pub fn evaluate(&self) -> Result<(ThermodynamicTemperature, HumidAirState), TampinesError> {
        Err(TampinesError::NotYetImplemented {
            component: "components::cooling_tower::CoolingTower::evaluate",
        })
    }
}
