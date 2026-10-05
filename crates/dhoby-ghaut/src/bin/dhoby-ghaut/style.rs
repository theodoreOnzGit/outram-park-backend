//! The workbench's style system (gh:#586): one type scale, sizes derived from
//! it, and one UI scale the reader sets.
//!
//! On 2026-10-05 every font was doubled and then reverted the same day, the
//! maintainer asking to "settle styles systematically". This module is that
//! settlement's mechanism, with the values left to the reader:
//!
//! - **One type scale.** Every text size in the workbench is a [`Text`] token
//!   (tiny, small, body, emphasis, subheading, heading, title), never a bare
//!   number. egui's own text styles (Small, Body, Button, Monospace, Heading)
//!   are set from the same tokens in [`apply`], so a widget that uses the
//!   default style and one that sets a size agree.
//! - **Sizes derived from the type.** Panel and card widths, button heights
//!   and on-canvas spacing are multiples of the body size ([`em`]), so they
//!   move with the text instead of being hand-set in pixels.
//! - **One UI scale, the reader's.** Everything above is in egui points;
//!   [`UiScale`] sets egui's zoom factor, which multiplies text, widths and
//!   spacing together, so nothing overlaps at any scale. It is changed from
//!   the top bar (A− / A+ / reset) or with egui's Ctrl + / Ctrl − / Ctrl 0,
//!   is remembered between sessions in `~/.config/dhoby-ghaut/ui.toml`, and
//!   can be forced with `--ui-scale S`.
//! - **A default from the screen.** With nothing saved, a monitor at least
//!   2400 points wide (e.g. the maintainer's 2560×1440 at 108 dpi, which the
//!   OS reports at 1 pixel per point) starts at 1.25; anything smaller at 1.0.
//!   This is a starting point, not a decision: the reader's choice wins.

use std::path::PathBuf;

/// A step on the type scale. Sizes in egui points at UI scale 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Text {
    /// Labels inside small glyphs (the axis gizmo).
    Tiny,
    /// Secondary text: hints, captions, overlays on the main view.
    Small,
    /// Running text and most controls; the unit [`em`] is based on.
    Body,
    /// Buttons on the main view, emphasised values.
    Emphasis,
    /// Card titles and sub-headings.
    Subheading,
    /// Panel and step headings.
    Heading,
    /// The start screen's title.
    Title,
}

impl Text {
    /// The size in points at UI scale 1. A modular scale of about 1.15 per
    /// step from a 13 pt body (13 pt is egui's own body size rounded up).
    #[must_use]
    pub fn size(self) -> f32 {
        match self {
            Self::Tiny => 9.0,
            Self::Small => 11.5,
            Self::Body => 13.0,
            Self::Emphasis => 15.0,
            Self::Subheading => 17.5,
            Self::Heading => 20.0,
            Self::Title => 26.0,
        }
    }

    /// A proportional font at this size.
    #[must_use]
    pub fn font(self) -> egui::FontId {
        egui::FontId::proportional(self.size())
    }
}

/// `n` body-text ems, in points: the unit for widths and spacing.
#[must_use]
pub fn em(n: f32) -> f32 {
    n * Text::Body.size()
}

/// Height of a button drawn on the main view (emphasis text plus padding).
#[must_use]
pub fn button_height() -> f32 {
    Text::Emphasis.size() + em(1.0)
}

/// Set egui's text styles from the type scale. Called once at start-up.
pub fn apply(ctx: &egui::Context) {
    use egui::{FontFamily, FontId, TextStyle};
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::new(Text::Small.size(), FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(Text::Body.size(), FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(Text::Body.size(), FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(Text::Body.size() - 0.5, FontFamily::Monospace)),
            (TextStyle::Heading, FontId::new(Text::Heading.size(), FontFamily::Proportional)),
        ]
        .into();
    });
}

/// The reader's UI scale: egui's zoom factor, remembered between sessions.
pub struct UiScale {
    /// The scale in force (1 = the type scale as written).
    pub scale: f32,
    /// What was last saved, so the file is written only on a change.
    saved: f32,
    /// A `--ui-scale` given on the command line wins for this session and is
    /// not saved.
    forced: bool,
}

/// The range the buttons and the saved file are held to.
pub const SCALE_RANGE: (f32, f32) = (0.6, 3.0);

impl UiScale {
    /// The saved scale, else `--ui-scale`, else a default from the monitor.
    pub fn new(ctx: &egui::Context, cli: Option<f32>) -> Self {
        let clamp = |s: f32| s.clamp(SCALE_RANGE.0, SCALE_RANGE.1);
        let (scale, forced) = match cli {
            Some(s) => (clamp(s), true),
            None => match load() {
                Some(s) => (clamp(s), false),
                None => (default_for(ctx), false),
            },
        };
        ctx.set_zoom_factor(scale);
        Self { scale, saved: scale, forced }
    }

    /// Follow a change made with egui's own Ctrl + / Ctrl − keys, and save.
    pub fn sync(&mut self, ctx: &egui::Context) {
        let z = ctx.zoom_factor();
        if (z - self.scale).abs() > 1e-4 {
            self.scale = z;
        }
        if !self.forced && (self.scale - self.saved).abs() > 1e-4 {
            save(self.scale);
            self.saved = self.scale;
        }
    }

    /// Set the scale (the top bar's A− / A+ / reset).
    pub fn set(&mut self, ctx: &egui::Context, s: f32) {
        self.scale = s.clamp(SCALE_RANGE.0, SCALE_RANGE.1);
        self.forced = false;
        ctx.set_zoom_factor(self.scale);
    }

    /// The A− / percentage / A+ group, in that order left to right in either
    /// layout direction.
    pub fn controls(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let mut order = [0, 1, 2];
        if ui.layout().prefer_right_to_left() {
            order.reverse();
        }
        for k in order {
            let (label, hover, to) = match k {
                0 => ("A−".to_string(), "Smaller text and panels (Ctrl −)", self.scale / 1.1),
                1 => (
                    format!("{:.0} %", 100.0 * self.scale),
                    "UI scale: text, panels and spacing together. Click to reset to this screen's default",
                    default_for(&ctx),
                ),
                _ => ("A+".to_string(), "Larger text and panels (Ctrl +)", self.scale * 1.1),
            };
            if ui.small_button(label).on_hover_text(hover).clicked() {
                self.set(&ctx, to);
            }
        }
    }
}

/// 1.25 on a monitor at least 2400 points wide, else 1.0 (see the module doc).
fn default_for(ctx: &egui::Context) -> f32 {
    let wide = ctx.input(|i| i.viewport().monitor_size).is_some_and(|m| m.x >= 2400.0);
    if wide { 1.25 } else { 1.0 }
}

fn config_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("dhoby-ghaut").join("ui.toml"))
}

fn load() -> Option<f32> {
    let text = std::fs::read_to_string(config_file()?).ok()?;
    parse_scale(&text)
}

/// The `ui_scale = S` line of `ui.toml`.
fn parse_scale(text: &str) -> Option<f32> {
    text.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == "ui_scale").then(|| v.trim().parse().ok()).flatten()
    })
}

fn save(scale: f32) {
    let Some(path) = config_file() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(
        path,
        format!("# dhoby-ghaut UI scale (gh:#586); set from the top bar or Ctrl +/-\nui_scale = {scale:.3}\n"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_type_scale_rises_monotonically_from_tiny_to_title() {
        let all = [
            Text::Tiny,
            Text::Small,
            Text::Body,
            Text::Emphasis,
            Text::Subheading,
            Text::Heading,
            Text::Title,
        ];
        assert!(all.windows(2).all(|w| w[0].size() < w[1].size()));
        assert!((em(2.0) - 2.0 * Text::Body.size()).abs() < 1e-6);
    }

    #[test]
    fn the_saved_scale_is_read_back() {
        assert_eq!(parse_scale("# c\nui_scale = 1.250\n"), Some(1.25));
        assert_eq!(parse_scale("other = 2\n"), None);
    }
}
