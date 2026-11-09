//! Visual reactor vessel: the physics wrapper.
//!
//! Wraps [`nee_soon::NordheimFuchsExactTimestepper`] (the prompt-excursion
//! "Prompt Excursion Layer" model) with screen geometry and a temperature
//! range for colour mapping, the same pattern
//! [`crate::components::pipe::PipeVisual`] uses for [`tampines::components::Pipe`].
//!
//! **Split 2026-11-05** (maintainer: "widget art can live in bugis, the
//! combination of logic and art can live inside the digital twin engine"):
//! the drawing is [`bugis::components::reactor_vessel::ReactorVesselArt`],
//! re-exported here; this file reads the fuel temperature off the model.

pub use bugis::components::reactor_vessel::*;
use egui::{Pos2, Response, Ui, Vec2, Widget};
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

    /// The drawing of this vessel: the model's lumped fuel temperature.
    pub fn art(&self) -> ReactorVesselArt {
        ReactorVesselArt::new(
            self.physics.fuel_temperature,
            self.screen_position,
            self.screen_vector,
            self.min_temp,
            self.max_temp,
        )
    }
}

impl Widget for ReactorVesselVisual {
    /// Draws [`ReactorVesselVisual::art`]: a rectangle filled by the lumped
    /// fuel temperature (via [`crate::components::temperature_colour`]),
    /// outlined in a neutral stroke.
    fn ui(self, ui: &mut Ui) -> Response {
        self.art().ui(ui)
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
        // The art carries the physics' fuel temperature, and the widget draws.
        assert_eq!(visual.art().fuel_temperature, visual.physics.fuel_temperature);
        assert!(bugis::components::shapes_drawn_by(visual) > 0);
    }
}
