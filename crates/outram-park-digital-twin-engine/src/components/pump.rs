//! Visual pump.
//!
//! Wraps [`tampines::components::Pump`] with screen geometry.

use egui::{Color32, Pos2, Rect, Response, Sense, Ui, Vec2, Widget};
use tampines::components::Pump;

/// Visual representation of a [`Pump`].
pub struct PumpVisual {
    /// The underlying physics component.
    pub physics: Pump,
    /// On-screen centre position.
    pub screen_position: Pos2,
    /// On-screen size.
    pub screen_vector: Vec2,
}

impl PumpVisual {
    /// Wrap a [`Pump`] with the given screen geometry.
    pub fn new(physics: Pump, screen_position: Pos2, screen_vector: Vec2) -> Self {
        Self {
            physics,
            screen_position,
            screen_vector,
        }
    }
}

impl Widget for PumpVisual {
    /// Minimal-static rendering: a filled circle at `screen_position`, sized
    /// by `screen_vector` -- [`Pump`] has no stored fluid state to colour by
    /// yet (its `evaluate` is not wired up), so this uses a fixed neutral
    /// colour rather than a fabricated one.
    fn ui(self, ui: &mut Ui) -> Response {
        let radius = self.screen_vector.length().max(1.0) / 2.0;
        let rect = Rect::from_center_size(self.screen_position, self.screen_vector);
        let response = ui.allocate_rect(rect, Sense::hover());
        ui.painter()
            .circle_filled(self.screen_position, radius, Color32::GRAY);
        response
    }
}
