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
//!
//! **MOVED 2026-11-02 (gh:#1112):** this widget library moved, byte for byte
//! apart from paths, from `outram-park-digital-twin-engine`'s
//! `src/components/` into `bugis` (feature `widgets`). The engine re-exports
//! it at its old path. One file stayed behind: `excursion` (the excursion
//! overlay) reads the engine's cited `htr10::design` fuel-temperature limits,
//! which are physics data and not GUI, so it stays in the engine and the
//! engine's `components` adds it beside this re-export.
//!
//! **Split 2026-11-05** (maintainer: "widget art can live in bugis, the
//! combination of logic and art can live inside the digital twin engine"):
//! this module is now the **art**. Every widget here draws from plain numbers
//! (temperatures, fractions, geometry). The widgets that hold a physics
//! component (`PipeVisual`, `ValveVisual`, `ReactorVesselVisual`,
//! `CondenserVisual`, `CoolingTowerVisual`, `HeatExchangerVisual`,
//! `PumpVisual`, `SteamGeneratorVisual`, `TurbineVisual`) and `PipeComponent`
//! moved to `outram_park_digital_twin_engine::components`, where each reads
//! its `tampines` / `tampines-steam-tables` / DWSIM / `nee_soon` state and
//! draws through the art type of the same stem here (`PipeArt`, `ValveArt`,
//! ...). bugis depends on no physics crate.

pub mod bend;
pub mod condenser;
pub mod control_rod_drive;
pub mod cooling_tower;
pub mod distillation_column;
pub mod fhr_reactor_vessel;
pub mod heat_exchanger;
pub(crate) mod htr10_conus_packing;
pub mod htr10_plant;
pub mod htr10_reactor_schematic;
pub mod htr10_reactor_vessel;
pub mod htr10_steam_generator;
pub mod instrumentation;
mod legend;
pub(crate) mod pebble_bed_texture;
pub mod pebble_packing;
pub mod pipe;
pub mod pipe_route;
pub mod pump;
pub mod reactor_archetype;
pub mod reactor_vessel;
pub mod steam_generator;
pub mod temperature_button;
pub mod turbine;
pub mod valve;

pub use bend::PipeBendVisual;
pub use condenser::{
    CondenserDisplayRange, CondenserKind, CondenserScalars, CondenserArt, CondenserArtState,
};
pub use control_rod_drive::slewed_control_rod_insertion;
pub use cooling_tower::{AirReadout, CoolingTowerArt, CoolingTowerArtScalars, CoolingTowerInlet};
pub use distillation_column::DistillationColumnVisual;
pub use fhr_reactor_vessel::FhrReactorVesselVisual;
pub use heat_exchanger::{
    ApproachVerdict, HeatExchangerConstruction, HeatExchangerDisplayRange, HeatExchangerKind,
    HeatExchangerScalars, HeatExchangerArt, HeatExchangerArtState,
};
pub use htr10_reactor_schematic::Htr10ReactorSchematic;
pub use htr10_reactor_vessel::{Htr10FlowAnchors, Htr10ReactorVesselVisual};
pub use htr10_steam_generator::Htr10SteamGeneratorVisual;
pub use instrumentation::InstrumentationVisual;
pub use legend::{LegendUnit, TemperatureLegend};
pub use pipe::{
    CoaxialDuctGeometry, CoaxialDuctVisual, PipePhaseShade, PipeScalars, PipeScale, PipeArt,
    PipeArtState, PipeCells,
};
pub use pump::PumpArt;
pub use reactor_archetype::{ReactorArchetype, ReactorArchetypeVisual};
pub use reactor_vessel::ReactorVesselArt;
pub use steam_generator::{SteamGeneratorArt, SteamGeneratorScalars};
pub use turbine::{StageAngles, TurbineFlowPath, TurbineArt};
pub use valve::ValveArt;

use crate::colour_maps::color32::crameri;
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
/// Uses Crameri's `vik` (see [`crate::colour_maps::color32::crameri`]): **blue at the
/// cold end, neutral white at the midpoint of the display range, red at the
/// hot end**. Being a diverging map, the midpoint carries meaning -- it is the
/// colour of "neither hot nor cold" -- so `min_temp`/`max_temp` should be set
/// symmetrically about whatever reference matters, not just clamped to the
/// extremes seen.
///
/// It is also perceptually uniform, so equal temperature steps look like equal
/// colour steps. The older [`crate::colour_maps::color32::hot_to_cold_colour_mark_1`] is
/// not, and put a muddy purple where the neutral point should be.
pub fn temperature_colour(
    t: ThermodynamicTemperature,
    min: ThermodynamicTemperature,
    max: ThermodynamicTemperature,
) -> Color32 {
    crameri::vik(hotness_from_temperature(t, min, max))
}

/// Normalise `t` to a \[0, 1\] "hotness" for colour mapping (see
/// [`crate::colour_maps::color32`]), clamped to the `[min, max]` range -- shared by
/// every visual component that colours itself by temperature.
pub fn hotness_from_temperature(
    t: ThermodynamicTemperature,
    min: ThermodynamicTemperature,
    max: ThermodynamicTemperature,
) -> f32 {
    let t = t.get::<kelvin>();
    let min = min.get::<kelvin>();
    let max = max.get::<kelvin>();
    (((t - min) / (max - min)) as f32).clamp(0.0, 1.0)
}

/// Draw `widget` once in a headless `egui` context and return how many shapes
/// it painted.
///
/// A check that a widget draws at all, for tests here and in the crates that
/// wrap this art (the digital-twin engine's physics-backed widgets, which
/// delegate their drawing to the art types in this module). Not a picture
/// test: it counts shapes, it does not compare pixels.
pub fn shapes_drawn_by(widget: impl egui::Widget) -> usize {
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(1280.0, 1200.0),
        )),
        ..Default::default()
    };
    // One warm-up frame so fonts exist before anything is laid out.
    let _ = ctx.run_ui(input.clone(), |_| {});
    let mut widget = Some(widget);
    let output = ctx.run_ui(input, |ui| {
        if let Some(w) = widget.take() {
            ui.add(w);
        }
    });
    output.shapes.len()
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
        assert!(cold.b() > cold.r(), "cold end must be blue, got {cold:?}");

        let mid: Color32 = temperature_colour(k(600.0), lo, hi);
        let spread =
            mid.r().max(mid.g()).max(mid.b()) as i16 - mid.r().min(mid.g()).min(mid.b()) as i16;
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
