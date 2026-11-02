//! The collapsible side panel (mobile-first rule, items 3 and 4): every
//! control lives in it; it has a visible "« Hide" button and, when folded, a
//! "Controls »" button on the main view brings it back (a drag handle is a
//! few pixels wide and hard to hit with a finger). It opens folded on a
//! screen under 700 px wide, open on a wider one, and never takes more than
//! 85 % of a narrow screen. Its contents scroll.
//!
//! **Thumb-sized (gh:#876 item 1, maintainer 2026-10-16):** "Hide" is a
//! full-width row [`HIDE_PX`] tall on its own, with [`HIDE_GAP_PX`] of empty
//! space below it, because a default-size Hide beside the heading put a
//! demo picker under the thumb and a near miss cancelled a running k_eff.
//! "Controls »" is [`REOPEN_SIZE`]: 44 px is the tallest that fits the 52 px
//! button row every demo keeps clear at the top of its main view, and
//! 140 px wide still clears [`zoom_buttons`](super::view::zoom_buttons)
//! (156 px from the right edge) on a 320 px screen.

use egui::{Rect, Vec2};

/// Screens narrower than this open with the panel folded, px.
pub const NARROW_PX: f32 = 700.0;
/// Height of the Hide button, px: above Material's 48 dp touch target.
pub const HIDE_PX: f32 = 52.0;
/// Size of the "Controls »" button, px (Apple's 44 pt minimum target).
pub const REOPEN_SIZE: Vec2 = Vec2::new(140.0, 44.0);
/// Empty space between Hide and the first control below it, px, so a near
/// miss lands on nothing.
pub const HIDE_GAP_PX: f32 = 16.0;

pub struct Panel {
    pub open: bool,
    /// The initial state is decided on the first frame, from the width.
    decided: bool,
}

impl Default for Panel {
    fn default() -> Self {
        Self { open: true, decided: false }
    }
}

impl Panel {
    /// Draw the panel on the left with a heading and "« Hide", and `contents`
    /// below in a scroll area. Call before the main view's `CentralPanel`.
    ///
    /// Also keeps the page's canvas at device-pixel size
    /// ([`keep_canvas_at_device_pixels`](super::platform::keep_canvas_at_device_pixels),
    /// gh:#556): every demo calls this once a frame, so every demo is
    /// covered without a call of its own. The fold decision waits for a
    /// frame laid out at the corrected size.
    pub fn show(&mut self, ui: &mut egui::Ui, heading: &str, contents: impl FnOnce(&mut egui::Ui)) {
        let resized = super::platform::keep_canvas_at_device_pixels(ui.ctx());
        if !self.decided && !resized {
            self.open = ui.max_rect().width() >= NARROW_PX;
            self.decided = true;
        }
        // Folds two ways: the panel's own drag handle (`open`), and the hide
        // button inside it (`self.open`); whichever changed wins.
        let before = self.open;
        let mut open = before;
        let mut hide = false;
        let width = (ui.max_rect().width() * 0.85).min(340.0);
        egui::Panel::left("controls").default_size(width).resizable(true).show_collapsible(ui, &mut open, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let w = ui.available_width();
                let b = egui::Button::new(egui::RichText::new("« Hide controls").size(18.0).strong()).min_size(Vec2::new(w, HIDE_PX));
                if ui.add(b).on_hover_text("Fold the panel away to see the whole picture").clicked() {
                    hide = true;
                }
                ui.add_space(HIDE_GAP_PX);
                ui.heading(heading);
                contents(ui);
            });
        });
        if hide {
            self.open = false;
        } else if self.open == before {
            self.open = open;
        }
    }

    /// The "Controls »" button on the main view, shown while folded.
    pub fn reopen_button(&mut self, ui: &mut egui::Ui, rect: Rect) {
        if !self.open {
            let b = Rect::from_min_size(rect.left_top() + Vec2::new(8.0, 8.0), REOPEN_SIZE);
            if ui.put(b, egui::Button::new(egui::RichText::new("Controls »").size(18.0).strong())).clicked() {
                self.open = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// gh:#876 item 1: the buttons are thumb-sized, "Controls »" stays inside
    /// the 52 px row every demo keeps clear, and it does not reach the zoom
    /// buttons (+ 40, − 40, Reset 64, gaps 6, inset 8) on a 320 px screen.
    #[test]
    fn hide_and_reopen_are_thumb_sized_and_clear_the_zoom_buttons() {
        assert!(HIDE_PX >= 48.0 && HIDE_GAP_PX >= 12.0);
        assert!(REOPEN_SIZE.y >= 44.0 && 8.0 + REOPEN_SIZE.y <= 52.0);
        let zoom_left = 320.0 - 8.0 - (40.0 + 40.0 + 64.0 + 2.0 * 6.0);
        assert!(8.0 + REOPEN_SIZE.x < zoom_left, "{} vs {zoom_left}", 8.0 + REOPEN_SIZE.x);
    }
}
