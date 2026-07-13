//! Visual reactor vessel.
//!
//! **No backing physics type yet.** `nee_soon` (the intended source for a
//! reactor-vessel physics type) is currently a scaffold with a single empty
//! `NeeSoon {}` struct -- no `ReactorVessel` type exists there to wrap. This
//! is a visual-only placeholder; once `nee_soon` exposes a real type, this
//! struct should gain a `physics` field the same way every other visual
//! component here does.

use egui::{Color32, Pos2, Rect, Response, Sense, Ui, Vec2, Widget};

/// Visual placeholder for a reactor vessel (no `nee_soon` physics type to
/// wrap yet -- see this module's doc).
pub struct ReactorVesselVisual {
    /// On-screen centre position.
    pub screen_position: Pos2,
    /// On-screen size.
    pub screen_vector: Vec2,
}

impl ReactorVesselVisual {
    /// Construct a placeholder reactor-vessel visual with the given screen
    /// geometry.
    pub fn new(screen_position: Pos2, screen_vector: Vec2) -> Self {
        Self { screen_position, screen_vector }
    }
}

impl Widget for ReactorVesselVisual {
    /// Minimal-static rendering: an outlined rectangle in a fixed neutral
    /// colour -- there is no physics state to colour by yet.
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
