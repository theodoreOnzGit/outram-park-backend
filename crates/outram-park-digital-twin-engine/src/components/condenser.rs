//! Visual condenser.
//!
//! Wraps [`tampines::components::Condenser`] with screen geometry.

use egui::{Color32, Pos2, Rect, Response, Sense, Ui, Vec2, Widget};
use tampines::components::Condenser;

/// Visual representation of a [`Condenser`].
pub struct CondenserVisual {
    /// The underlying physics component.
    pub physics: Condenser,
    /// On-screen centre position.
    pub screen_position: Pos2,
    /// On-screen size.
    pub screen_vector: Vec2,
}

impl CondenserVisual {
    /// Wrap a [`Condenser`] with the given screen geometry.
    pub fn new(physics: Condenser, screen_position: Pos2, screen_vector: Vec2) -> Self {
        Self { physics, screen_position, screen_vector }
    }
}

impl Widget for CondenserVisual {
    /// Minimal-static rendering: a filled rectangle in a fixed neutral
    /// (cold-side) colour -- [`Condenser`] does not store a current fluid
    /// state (only an operating pressure and target outlet quality), so no
    /// temperature-driven colour is fabricated here.
    fn ui(self, ui: &mut Ui) -> Response {
        let rect = Rect::from_center_size(self.screen_position, self.screen_vector);
        let response = ui.allocate_rect(rect, Sense::hover());
        ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(0, 135, 255));
        response
    }
}
