//! Visual pipe.
//!
//! Wraps [`tampines::components::Pipe`] with screen geometry and a
//! temperature range for colour mapping.

use crate::color_maps::hot_to_cold_colour_mark_1;
use egui::{Pos2, Rect, Response, Sense, Ui, Vec2, Widget};
use tampines::components::Pipe;
use uom::si::f64::ThermodynamicTemperature;

/// Visual representation of a [`Pipe`].
///
/// `screen_vector` gives the pipe's on-screen direction and length (from
/// `screen_position` to `screen_position + screen_vector`).
pub struct PipeVisual {
    /// The underlying physics component.
    pub physics: Pipe,
    /// On-screen anchor position (one endpoint of the pipe).
    pub screen_position: Pos2,
    /// On-screen direction and length, from `screen_position`.
    pub screen_vector: Vec2,
    /// Temperature mapped to [`crate::color_maps::hot_to_cold_colour_mark_1`]'s
    /// `hotness = 0.0` (coldest displayable colour).
    pub min_temp: ThermodynamicTemperature,
    /// Temperature mapped to `hotness = 1.0` (hottest displayable colour).
    pub max_temp: ThermodynamicTemperature,
}

impl PipeVisual {
    /// Wrap a [`Pipe`] with the given screen geometry and colour-mapping
    /// temperature range.
    pub fn new(
        physics: Pipe,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self { physics, screen_position, screen_vector, min_temp, max_temp }
    }
}

impl Widget for PipeVisual {
    /// Minimal-static rendering: a straight line from `screen_position` along
    /// `screen_vector`, coloured via a placeholder mid-range hotness (`0.5`)
    /// -- [`Pipe`]'s backend flow arrays have a working temperature getter,
    /// but pulling a single representative scalar out of a per-cell array is
    /// left to a future pass, not this scaffold.
    fn ui(self, ui: &mut Ui) -> Response {
        let end = self.screen_position + self.screen_vector;
        let rect = Rect::from_two_pos(self.screen_position, end);
        let response = ui.allocate_rect(rect, Sense::hover());
        ui.painter().line_segment(
            [self.screen_position, end],
            (4.0, hot_to_cold_colour_mark_1(0.5)),
        );
        response
    }
}
