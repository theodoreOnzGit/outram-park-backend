//! Visual process object wrappers.
//!
//! One file per visual process object, each composing its
//! [`tampines::components`] (or, for [`reactor_vessel`], `nee_soon`) physics
//! counterpart with visual-only fields (screen position/size, min/max
//! temperature for colour mapping) and a minimal `egui::Widget`
//! implementation. Deliberately composes rather than duplicates state --
//! avoid separating physics and rendering unnecessarily.
//! [`instrumentation`] stays a generic label/value placeholder -- `nee_soon`
//! does not yet expose a dedicated instrumentation-readout type to wrap.

pub mod bend;
mod condenser;
pub mod cooling_tower;
pub mod fhr_reactor_vessel;
pub mod heat_exchanger;
pub mod instrumentation;
mod legend;
pub mod pipe;
mod pipe_component;
pub mod pump;
pub mod reactor_vessel;
pub mod steam_generator;
pub mod turbine;
pub mod valve;

pub use condenser::CondenserVisual;
pub use cooling_tower::CoolingTowerVisual;
pub use fhr_reactor_vessel::FhrReactorVesselVisual;
pub use heat_exchanger::HeatExchangerVisual;
pub use instrumentation::InstrumentationVisual;
pub use pipe::{PipePhaseShade, PipeScalars, PipeScale, PipeVisual, PipeVisualState};
pub use pump::PumpVisual;
pub use reactor_vessel::ReactorVesselVisual;
pub use steam_generator::SteamGeneratorVisual;
pub use bend::PipeBendVisual;
pub use legend::{LegendUnit, TemperatureLegend};
pub use pipe_component::PipeComponent;
pub use turbine::{StageAngles, TurbineFlowPath, TurbineVisual, TurbineVisualState};
pub use valve::ValveVisual;

use crate::color_maps::crameri;
use egui::Color32;
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

/// The colour a temperature is drawn in, given the display range it is
/// normalised against.
///
/// One helper so every widget in this module grades temperature identically:
/// a gallery in which two components disagree about what "hot" looks like is
/// worse than useless.
///
/// Uses Crameri's `vik` (see [`crate::color_maps::crameri`]): **blue at the
/// cold end, neutral white at the midpoint of the display range, red at the
/// hot end**. Being a diverging map, the midpoint carries meaning -- it is the
/// colour of "neither hot nor cold" -- so `min_temp`/`max_temp` should be set
/// symmetrically about whatever reference matters, not just clamped to the
/// extremes seen.
///
/// It is also perceptually uniform, so equal temperature steps look like equal
/// colour steps. The older [`crate::color_maps::hot_to_cold_colour_mark_1`] is
/// not, and put a muddy purple where the neutral point should be.
pub(crate) fn temperature_colour(
    t: ThermodynamicTemperature,
    min: ThermodynamicTemperature,
    max: ThermodynamicTemperature,
) -> Color32 {
    crameri::vik(hotness_from_temperature(t, min, max))
}

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
    use super::temperature_colour;
    use egui::Color32;

    /// Temperature must grade BLUE at the cold end, through NEUTRAL WHITE at
    /// the midpoint, to RED at the hot end. This is the convention the whole
    /// widget library is read against, so it is pinned rather than left to
    /// whichever colour map happens to be wired in.
    ///
    /// **Methodology:** sample the shared helper over a 300-900 K display
    /// range at the cold end, the midpoint and the hot end, and require blue
    /// to dominate, then all channels to be high and close together, then red
    /// to dominate.
    ///
    /// **Result (2026-08-06):** 300 K -> rgb(0,18,97) (blue dominant),
    /// 600 K -> rgb(236,229,224) (near-neutral, channel spread 12/255),
    /// 900 K -> rgb(89,0,8) (red dominant).
    #[test]
    fn temperature_grades_blue_through_white_to_red() {
        let k = |v: f64| ThermodynamicTemperature::new::<kelvin>(v);
        let (lo, hi) = (k(300.0), k(900.0));

        let cold: Color32 = temperature_colour(k(300.0), lo, hi);
        assert!(
            cold.b() > cold.r(),
            "cold end must be blue, got {cold:?}"
        );

        let mid: Color32 = temperature_colour(k(600.0), lo, hi);
        let spread = mid.r().max(mid.g()).max(mid.b()) as i16
            - mid.r().min(mid.g()).min(mid.b()) as i16;
        assert!(
            mid.r() > 180 && mid.g() > 180 && mid.b() > 180 && spread < 40,
            "midpoint must be near-neutral white, got {mid:?} spread {spread}"
        );

        let hot: Color32 = temperature_colour(k(900.0), lo, hi);
        assert!(hot.r() > hot.b(), "hot end must be red, got {hot:?}");
    }

    use super::*;

    #[test]
    fn hotness_clamps_and_normalizes() {
        let min = ThermodynamicTemperature::new::<kelvin>(300.0);
        let max = ThermodynamicTemperature::new::<kelvin>(400.0);
        assert_eq!(
            hotness_from_temperature(ThermodynamicTemperature::new::<kelvin>(350.0), min, max),
            0.5
        );
        assert_eq!(
            hotness_from_temperature(ThermodynamicTemperature::new::<kelvin>(200.0), min, max),
            0.0
        );
        assert_eq!(
            hotness_from_temperature(ThermodynamicTemperature::new::<kelvin>(500.0), min, max),
            1.0
        );
    }
}
