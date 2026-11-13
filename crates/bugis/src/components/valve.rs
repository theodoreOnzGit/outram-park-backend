//! Visual valve: the art.
//!
//! Colours itself by opening fraction (fully closed = grey, fully open =
//! green). ~~Wraps [`tampines::components::Valve`] with screen geometry.~~
//! **CORRECTED 2026-11-05** (maintainer: "widget art can live in bugis, the
//! combination of logic and art can live inside the digital twin engine"):
//! this file holds the drawing only, given a plain opening fraction. The
//! physics wrapper, `ValveVisual` around `tampines::components::Valve`, lives
//! in `outram_park_digital_twin_engine::components::valve` and draws through
//! [`ValveArt`].

use egui::{Color32, Pos2, Rect, Response, Sense, Ui, Vec2, Widget};

/// The drawing of a valve, from a plain opening fraction.
pub struct ValveArt {
    /// Opening, dimensionless: `0.0` fully closed, `1.0` fully open. Values
    /// outside `[0, 1]` are clamped when drawn.
    pub opening: f32,
    /// On-screen centre position.
    pub screen_position: Pos2,
    /// On-screen size.
    pub screen_vector: Vec2,
}

impl ValveArt {
    /// A valve drawing at the given opening fraction and screen geometry.
    pub fn new(opening: f32, screen_position: Pos2, screen_vector: Vec2) -> Self {
        Self {
            opening,
            screen_position,
            screen_vector,
        }
    }

    /// The fill colour for this opening: grey when closed, green when open.
    pub fn colour(&self) -> Color32 {
        let opening = self.opening.clamp(0.0, 1.0);
        Color32::from_rgb(
            (128.0 * (1.0 - opening)) as u8,
            (128.0 + 127.0 * opening) as u8,
            (128.0 * (1.0 - opening)) as u8,
        )
    }
}

impl Widget for ValveArt {
    /// Minimal-static rendering: a filled rectangle, green fraction of which
    /// scales with opening (`0` -> all grey, `1` -> all green).
    fn ui(self, ui: &mut Ui) -> Response {
        let rect = Rect::from_center_size(self.screen_position, self.screen_vector);
        let response = ui.allocate_rect(rect, Sense::hover());
        ui.painter().rect_filled(rect, 2.0, self.colour());
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Closed is grey, open is green, and out-of-range openings clamp.
    #[test]
    fn opening_grades_grey_to_green_and_draws() {
        let at = |o: f32| ValveArt::new(o, Pos2::new(20.0, 20.0), Vec2::new(10.0, 10.0));
        assert_eq!(at(0.0).colour(), Color32::from_rgb(128, 128, 128));
        assert_eq!(at(1.0).colour(), Color32::from_rgb(0, 255, 0));
        assert_eq!(at(-1.0).colour(), at(0.0).colour());
        assert_eq!(at(2.0).colour(), at(1.0).colour());
        assert!(crate::components::shapes_drawn_by(at(0.5)) > 0);
    }
}
