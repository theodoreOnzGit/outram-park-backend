//! Visual turbine.
//!
//! Wraps [`tampines::components::Turbine`] with screen geometry and a
//! temperature range for colour mapping. Colours itself by the inlet
//! temperature -- [`tampines::hem::HemSteamCv::get_temperature`] is a real,
//! working getter, so this is real data, not a placeholder.

use crate::color_maps::hot_to_cold_colour_mark_1;
use crate::components::hotness_from_temperature;
use egui::{Pos2, Rect, Response, Sense, Ui, Vec2, Widget};
use tampines::components::Turbine;
use uom::si::f64::ThermodynamicTemperature;

/// Visual representation of a [`Turbine`].
pub struct TurbineVisual {
    /// The underlying physics component.
    pub physics: Turbine,
    /// On-screen centre position.
    pub screen_position: Pos2,
    /// On-screen size.
    pub screen_vector: Vec2,
    /// Temperature mapped to `hotness = 0.0`.
    pub min_temp: ThermodynamicTemperature,
    /// Temperature mapped to `hotness = 1.0`.
    pub max_temp: ThermodynamicTemperature,
}

impl TurbineVisual {
    /// Wrap a [`Turbine`] with the given screen geometry and colour-mapping
    /// temperature range.
    pub fn new(
        physics: Turbine,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self { physics, screen_position, screen_vector, min_temp, max_temp }
    }
}

impl Widget for TurbineVisual {
    /// Minimal-static rendering: a filled rectangle coloured by inlet
    /// temperature.
    fn ui(self, ui: &mut Ui) -> Response {
        let rect = Rect::from_center_size(self.screen_position, self.screen_vector);
        let response = ui.allocate_rect(rect, Sense::hover());
        let hotness =
            hotness_from_temperature(self.physics.inlet.get_temperature(), self.min_temp, self.max_temp);
        ui.painter().rect_filled(rect, 2.0, hot_to_cold_colour_mark_1(hotness));
        response
    }
}
