//! Visual reactor vessel: the art.
//!
//! ~~Wraps [`nee_soon::NordheimFuchsExactTimestepper`] (the prompt-excursion
//! "Prompt Excursion Layer" model) with screen geometry and a temperature
//! range for colour mapping.~~ **CORRECTED 2026-11-05** (maintainer: "widget
//! art can live in bugis, the combination of logic and art can live inside
//! the digital twin engine"): this file draws the vessel from a plain lumped
//! fuel temperature. The wrapper around `nee_soon`'s
//! `NordheimFuchsExactTimestepper`, `ReactorVesselVisual`, lives in
//! `outram_park_digital_twin_engine::components::reactor_vessel` and draws
//! through [`ReactorVesselArt`].

use crate::components::temperature_colour;
use egui::{Color32, Pos2, Rect, Response, Sense, Ui, Vec2, Widget};
use uom::si::f64::ThermodynamicTemperature;

/// The drawing of a reactor vessel filled by its lumped fuel temperature, so
/// a prompt excursion is visible as the vessel heating up rather than as a
/// number changing elsewhere on screen.
pub struct ReactorVesselArt {
    /// Lumped fuel temperature, the fill colour.
    pub fuel_temperature: ThermodynamicTemperature,
    /// On-screen centre position.
    pub screen_position: Pos2,
    /// On-screen size.
    pub screen_vector: Vec2,
    /// Fuel temperature mapped to the coldest displayable colour
    /// (`hotness = 0.0`).
    pub min_temp: ThermodynamicTemperature,
    /// Fuel temperature mapped to `hotness = 1.0` (hottest displayable
    /// colour).
    pub max_temp: ThermodynamicTemperature,
}

impl ReactorVesselArt {
    /// A vessel drawing at the given fuel temperature, screen geometry and
    /// fuel-temperature colour-mapping range.
    ///
    /// `min_temp`/`max_temp` bound the displayed colour scale; pick them to
    /// span the operating range the simulator expects to reach, so a normal
    /// operating point does not sit pinned at either end of the scale.
    pub fn new(
        fuel_temperature: ThermodynamicTemperature,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            fuel_temperature,
            screen_position,
            screen_vector,
            min_temp,
            max_temp,
        }
    }
}

impl Widget for ReactorVesselArt {
    /// Renders the vessel as a rectangle filled by the lumped fuel
    /// temperature (via [`crate::components::temperature_colour`]), outlined in
    /// a neutral stroke so the vessel boundary stays readable at every fill
    /// colour.
    fn ui(self, ui: &mut Ui) -> Response {
        let rect = Rect::from_center_size(self.screen_position, self.screen_vector);
        let response = ui.allocate_rect(rect, Sense::hover());

        let colour = temperature_colour(self.fuel_temperature, self.min_temp, self.max_temp);
        let painter = ui.painter();
        painter.rect_filled(rect, 2.0, colour);
        painter.rect_stroke(
            rect,
            2.0,
            egui::Stroke::new(2.0_f32, Color32::DARK_GRAY),
            egui::StrokeKind::Middle,
        );
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::thermodynamic_temperature::kelvin;

    /// The art draws a fill and an outline for any fuel temperature.
    #[test]
    fn the_vessel_draws() {
        let k = |v: f64| ThermodynamicTemperature::new::<kelvin>(v);
        let art = ReactorVesselArt::new(
            k(900.0),
            Pos2::new(40.0, 60.0),
            Vec2::new(50.0, 80.0),
            k(600.0),
            k(1200.0),
        );
        assert!(crate::components::shapes_drawn_by(art) > 0);
    }
}
