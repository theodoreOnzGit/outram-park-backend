//! Visual process object wrappers.
//!
//! One file per visual process object, each composing its
//! [`tampines::components`] (or, for [`reactor_vessel`]/[`instrumentation`],
//! future `nee_soon`) physics counterpart with visual-only fields
//! (screen position/size, min/max temperature for colour mapping) and a
//! minimal `egui::Widget` implementation. Deliberately composes rather than
//! duplicates state -- avoid separating physics and rendering unnecessarily.

pub mod condenser;
pub mod cooling_tower;
pub mod heat_exchanger;
pub mod instrumentation;
pub mod pipe;
pub mod pump;
pub mod reactor_vessel;
pub mod steam_generator;
pub mod turbine;
pub mod valve;

pub use condenser::CondenserVisual;
pub use cooling_tower::CoolingTowerVisual;
pub use heat_exchanger::HeatExchangerVisual;
pub use instrumentation::InstrumentationVisual;
pub use pipe::PipeVisual;
pub use pump::PumpVisual;
pub use reactor_vessel::ReactorVesselVisual;
pub use steam_generator::SteamGeneratorVisual;
pub use turbine::TurbineVisual;
pub use valve::ValveVisual;

use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

/// Normalise `t` to a \[0, 1\] "hotness" for colour mapping (see
/// [`crate::color_maps`]), clamped to the `[min, max]` range -- shared by
/// every visual component that colours itself by temperature.
pub(crate) fn hotness_from_temperature(
    t: ThermodynamicTemperature,
    min: ThermodynamicTemperature,
    max: ThermodynamicTemperature,
) -> f32 {
    let t = t.get::<kelvin>();
    let min = min.get::<kelvin>();
    let max = max.get::<kelvin>();
    (((t - min) / (max - min)) as f32).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotness_clamps_and_normalizes() {
        let min = ThermodynamicTemperature::new::<kelvin>(300.0);
        let max = ThermodynamicTemperature::new::<kelvin>(400.0);
        assert_eq!(hotness_from_temperature(ThermodynamicTemperature::new::<kelvin>(350.0), min, max), 0.5);
        assert_eq!(hotness_from_temperature(ThermodynamicTemperature::new::<kelvin>(200.0), min, max), 0.0);
        assert_eq!(hotness_from_temperature(ThermodynamicTemperature::new::<kelvin>(500.0), min, max), 1.0);
    }
}
