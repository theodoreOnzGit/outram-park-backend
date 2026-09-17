//! Visual reactor vessel.
//!
//! Wraps [`nee_soon::NordheimFuchsExactTimestepper`] (the prompt-excursion
//! "Prompt Excursion Layer" model) with screen geometry and a temperature
//! range for colour mapping, the same pattern
//! [`crate::components::pipe::PipeVisual`] uses for [`tampines::components::Pipe`].

use crate::components::temperature_colour;
use egui::{Color32, Pos2, Rect, Response, Sense, Ui, Vec2, Widget};
use nee_soon::NordheimFuchsExactTimestepper;
use uom::si::f64::ThermodynamicTemperature;

/// Visual representation of a reactor vessel driven by a
/// [`NordheimFuchsExactTimestepper`].
///
/// The vessel fills with a colour derived from the model's lumped fuel
/// temperature, so a prompt excursion is visible as the vessel heating up
/// rather than as a number changing elsewhere on screen.
pub struct ReactorVesselVisual {
    /// The underlying physics component (prompt power + fuel temperature).
    pub physics: NordheimFuchsExactTimestepper,
    /// On-screen centre position.
    pub screen_position: Pos2,
    /// On-screen size.
    pub screen_vector: Vec2,
    /// Fuel temperature mapped to
    /// the coldest displayable colour (`hotness = 0.0`)
    /// (coldest displayable colour).
    pub min_temp: ThermodynamicTemperature,
    /// Fuel temperature mapped to `hotness = 1.0` (hottest displayable
    /// colour).
    pub max_temp: ThermodynamicTemperature,
}

impl ReactorVesselVisual {
    /// Wrap a [`NordheimFuchsExactTimestepper`] with the given screen
    /// geometry and fuel-temperature colour-mapping range.
    ///
    /// `min_temp`/`max_temp` bound the displayed colour scale; pick them to
    /// span the operating range the simulator expects to reach, so a normal
    /// operating point does not sit pinned at either end of the scale.
    pub fn new(
        physics: NordheimFuchsExactTimestepper,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            physics,
            screen_position,
            screen_vector,
            min_temp,
            max_temp,
        }
    }
}

impl Widget for ReactorVesselVisual {
    /// Renders the vessel as a rectangle filled by the lumped fuel
    /// temperature (via [`crate::components::temperature_colour`]), outlined in
    /// a neutral stroke so the vessel boundary stays readable at every fill
    /// colour.
    fn ui(self, ui: &mut Ui) -> Response {
        let rect = Rect::from_center_size(self.screen_position, self.screen_vector);
        let response = ui.allocate_rect(rect, Sense::hover());

        let colour =
            temperature_colour(self.physics.fuel_temperature, self.min_temp, self.max_temp);
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
    use crate::components::hotness_from_temperature;
    use uom::si::thermodynamic_temperature::kelvin;

    #[test]
    fn fuel_temperature_drives_the_fill_hotness() {
        let mut physics = NordheimFuchsExactTimestepper::default();
        physics.fuel_temperature = ThermodynamicTemperature::new::<kelvin>(900.0);

        let visual = ReactorVesselVisual::new(
            physics,
            Pos2::ZERO,
            Vec2::new(50.0, 80.0),
            ThermodynamicTemperature::new::<kelvin>(600.0),
            ThermodynamicTemperature::new::<kelvin>(1200.0),
        );

        // 900 K is exactly midway through the [600, 1200] K display range.
        assert_eq!(
            hotness_from_temperature(
                visual.physics.fuel_temperature,
                visual.min_temp,
                visual.max_temp
            ),
            0.5
        );
    }
}
