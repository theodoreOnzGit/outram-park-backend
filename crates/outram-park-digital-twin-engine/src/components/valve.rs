//! Visual valve.
//!
//! Wraps [`tampines::components::Valve`] with screen geometry. Colours
//! itself by opening percentage (fully closed = grey, fully open = green) --
//! [`Valve::current_kv`] is real/working, so this is real data, not a
//! placeholder.

use egui::{Color32, Pos2, Rect, Response, Sense, Ui, Vec2, Widget};
use tampines::components::Valve;
use uom::si::ratio::ratio;

/// Visual representation of a [`Valve`].
pub struct ValveVisual {
    /// The underlying physics component.
    pub physics: Valve,
    /// On-screen centre position.
    pub screen_position: Pos2,
    /// On-screen size.
    pub screen_vector: Vec2,
}

impl ValveVisual {
    /// Wrap a [`Valve`] with the given screen geometry.
    pub fn new(physics: Valve, screen_position: Pos2, screen_vector: Vec2) -> Self {
        Self {
            physics,
            screen_position,
            screen_vector,
        }
    }
}

impl Widget for ValveVisual {
    /// Minimal-static rendering: a filled rectangle, green fraction of which
    /// scales with opening percentage (`0%` -> all grey, `100%` -> all
    /// green).
    fn ui(self, ui: &mut Ui) -> Response {
        let rect = Rect::from_center_size(self.screen_position, self.screen_vector);
        let response = ui.allocate_rect(rect, Sense::hover());
        let opening = (self.physics.opening_percent.get::<ratio>() / 100.0).clamp(0.0, 1.0) as f32;
        let colour = Color32::from_rgb(
            (128.0 * (1.0 - opening)) as u8,
            (128.0 + 127.0 * opening) as u8,
            (128.0 * (1.0 - opening)) as u8,
        );
        ui.painter().rect_filled(rect, 2.0, colour);
        response
    }
}
