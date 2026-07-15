//! Visual heat exchanger.
//!
//! Wraps [`tampines::components::HeatExchanger`] with screen geometry.

use egui::{Color32, Pos2, Rect, Response, Sense, Ui, Vec2, Widget};
use tampines::components::HeatExchanger;

/// Visual representation of a [`HeatExchanger`].
pub struct HeatExchangerVisual {
    /// The underlying physics component.
    pub physics: HeatExchanger,
    /// On-screen centre position.
    pub screen_position: Pos2,
    /// On-screen size.
    pub screen_vector: Vec2,
}

impl HeatExchangerVisual {
    /// Wrap a [`HeatExchanger`] with the given screen geometry.
    pub fn new(physics: HeatExchanger, screen_position: Pos2, screen_vector: Vec2) -> Self {
        Self { physics, screen_position, screen_vector }
    }
}

impl Widget for HeatExchangerVisual {
    /// Minimal-static rendering: a filled rectangle in a fixed neutral
    /// colour -- [`HeatExchanger`] holds geometry/coefficient fields, not a
    /// fluid state to colour by, so no temperature-driven colour is
    /// fabricated here.
    fn ui(self, ui: &mut Ui) -> Response {
        let rect = Rect::from_center_size(self.screen_position, self.screen_vector);
        let response = ui.allocate_rect(rect, Sense::hover());
        ui.painter().rect_filled(rect, 2.0, Color32::LIGHT_BLUE);
        response
    }
}
