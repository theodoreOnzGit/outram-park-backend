//! The collapsible side panel (mobile-first rule, items 3 and 4): every
//! control lives in it; it has a visible "« Hide" button and, when folded, a
//! "Controls »" button on the main view brings it back (a drag handle is a
//! few pixels wide and hard to hit with a finger). It opens folded on a
//! screen under 700 px wide, open on a wider one, and never takes more than
//! 85 % of a narrow screen. Its contents scroll.

use egui::{Rect, Vec2};

/// Screens narrower than this open with the panel folded, px.
pub const NARROW_PX: f32 = 700.0;

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
    pub fn show(&mut self, ui: &mut egui::Ui, heading: &str, contents: impl FnOnce(&mut egui::Ui)) {
        if !self.decided {
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
                ui.horizontal(|ui| {
                    ui.heading(heading);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("« Hide").on_hover_text("Fold the panel away to see the whole picture").clicked() {
                            hide = true;
                        }
                    });
                });
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
            let b = Rect::from_min_size(rect.left_top() + Vec2::new(8.0, 8.0), Vec2::new(104.0, 36.0));
            if ui.put(b, egui::Button::new("Controls »")).clicked() {
                self.open = true;
            }
        }
    }
}
