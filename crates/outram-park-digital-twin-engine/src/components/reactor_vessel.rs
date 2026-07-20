//! Visual reactor vessel.
//!
//! Wraps [`nee_soon::NordheimFuchsExactTimestepper`] (the prompt-excursion
//! "Prompt Excursion Layer" model) with screen geometry, the same pattern
//! [`crate::components::pipe::PipeVisual`] uses for [`tampines::components::Pipe`].

use egui::{Color32, Pos2, Rect, Response, Sense, Ui, Vec2, Widget};
use nee_soon::NordheimFuchsExactTimestepper;

/// Visual representation of a reactor vessel driven by a
/// [`NordheimFuchsExactTimestepper`].
pub struct ReactorVesselVisual {
    /// The underlying physics component (prompt power + fuel temperature).
    pub physics: NordheimFuchsExactTimestepper,
    /// On-screen centre position.
    pub screen_position: Pos2,
    /// On-screen size.
    pub screen_vector: Vec2,
}

impl ReactorVesselVisual {
    /// Wrap a [`NordheimFuchsExactTimestepper`] with the given screen
    /// geometry.
    pub fn new(
        physics: NordheimFuchsExactTimestepper,
        screen_position: Pos2,
        screen_vector: Vec2,
    ) -> Self {
        Self { physics, screen_position, screen_vector }
    }
}

impl Widget for ReactorVesselVisual {
    /// Minimal-static rendering: an outlined rectangle in a fixed neutral
    /// colour -- deriving colour/fill from `physics.power` is left to a
    /// future pass, same as [`crate::components::pipe::PipeVisual`]'s
    /// placeholder hotness.
    fn ui(self, ui: &mut Ui) -> Response {
        let rect = Rect::from_center_size(self.screen_position, self.screen_vector);
        let response = ui.allocate_rect(rect, Sense::hover());
        ui.painter().rect_stroke(
            rect,
            2.0,
            egui::Stroke::new(2.0, Color32::DARK_GRAY),
            egui::StrokeKind::Middle,
        );
        response
    }
}
