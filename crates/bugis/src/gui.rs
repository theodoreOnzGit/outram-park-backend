//! The egui half (feature `egui`): visuals, selector, `Color32` palette.


/// The Gruvbox palette as `egui::Color32` ([`crate::gruvbox`]'s values).
pub mod palette {
    use crate::gruvbox as g;
    use egui::Color32;

    const fn c(x: crate::Rgb) -> Color32 {
        Color32::from_rgb(x.r, x.g, x.b)
    }

    pub const DARK0_HARD: Color32 = c(g::DARK0_HARD);
    pub const DARK0: Color32 = c(g::DARK0);
    pub const DARK1: Color32 = c(g::DARK1);
    pub const DARK2: Color32 = c(g::DARK2);
    pub const DARK3: Color32 = c(g::DARK3);
    pub const DARK4: Color32 = c(g::DARK4);
    pub const GRAY: Color32 = c(g::GRAY);
    pub const LIGHT0_HARD: Color32 = c(g::LIGHT0_HARD);
    pub const LIGHT0: Color32 = c(g::LIGHT0);
    pub const LIGHT1: Color32 = c(g::LIGHT1);
    pub const LIGHT2: Color32 = c(g::LIGHT2);
    pub const LIGHT3: Color32 = c(g::LIGHT3);
    pub const LIGHT4: Color32 = c(g::LIGHT4);
    pub const BRIGHT_RED: Color32 = c(g::BRIGHT_RED);
    pub const BRIGHT_GREEN: Color32 = c(g::BRIGHT_GREEN);
    pub const BRIGHT_YELLOW: Color32 = c(g::BRIGHT_YELLOW);
    pub const BRIGHT_BLUE: Color32 = c(g::BRIGHT_BLUE);
    pub const BRIGHT_PURPLE: Color32 = c(g::BRIGHT_PURPLE);
    pub const BRIGHT_AQUA: Color32 = c(g::BRIGHT_AQUA);
    pub const BRIGHT_ORANGE: Color32 = c(g::BRIGHT_ORANGE);
    pub const NEUTRAL_RED: Color32 = c(g::NEUTRAL_RED);
    pub const NEUTRAL_GREEN: Color32 = c(g::NEUTRAL_GREEN);
    pub const NEUTRAL_YELLOW: Color32 = c(g::NEUTRAL_YELLOW);
    pub const NEUTRAL_BLUE: Color32 = c(g::NEUTRAL_BLUE);
    pub const NEUTRAL_PURPLE: Color32 = c(g::NEUTRAL_PURPLE);
    pub const NEUTRAL_AQUA: Color32 = c(g::NEUTRAL_AQUA);
    pub const NEUTRAL_ORANGE: Color32 = c(g::NEUTRAL_ORANGE);
    pub const FADED_RED: Color32 = c(g::FADED_RED);
    pub const FADED_GREEN: Color32 = c(g::FADED_GREEN);
    pub const FADED_YELLOW: Color32 = c(g::FADED_YELLOW);
    pub const FADED_BLUE: Color32 = c(g::FADED_BLUE);
    pub const FADED_PURPLE: Color32 = c(g::FADED_PURPLE);
    pub const FADED_AQUA: Color32 = c(g::FADED_AQUA);
    pub const FADED_ORANGE: Color32 = c(g::FADED_ORANGE);

    /// The foreground kovan's light theme used (Gruvbox `dark1`).
    pub const LIGHT_FG: Color32 = DARK1;
}

impl From<crate::Rgb> for egui::Color32 {
    fn from(x: crate::Rgb) -> Self {
        egui::Color32::from_rgb(x.r, x.g, x.b)
    }
}

use egui::Color32;
use palette::*;

/// Which Gruvbox variant the GUI uses. An enum (the set is closed), per the
/// workspace's Rust design rules.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    /// Gruvbox Dark (`#1d2021` background), the default.
    #[default]
    GruvboxDark,
    /// Gruvbox Light (`#f9f5d7` background).
    GruvboxLight,
}

impl Theme {
    /// Every theme, in the order the selector shows them.
    pub const ALL: [Theme; 2] = [Theme::GruvboxDark, Theme::GruvboxLight];

    /// Selector label.
    pub fn label(self) -> &'static str {
        match self {
            Self::GruvboxDark => "Gruvbox Dark",
            Self::GruvboxLight => "Gruvbox Light",
        }
    }

    /// Whether this is the dark variant.
    pub fn is_dark(self) -> bool {
        matches!(self, Self::GruvboxDark)
    }

    /// The full [`egui::Visuals`] for this theme.
    pub fn visuals(self) -> egui::Visuals {
        gruvbox_visuals(self.is_dark())
    }

    /// Apply this theme to the context, now. Both of egui's style slots get
    /// the Gruvbox visuals and egui's theme is pinned to this one, so the
    /// system's dark/light preference can never swap egui's defaults back in.
    pub fn apply(self, ctx: &egui::Context) {
        ctx.set_visuals_of(egui::Theme::Dark, gruvbox_visuals(true));
        ctx.set_visuals_of(egui::Theme::Light, gruvbox_visuals(false));
        ctx.set_theme(if self.is_dark() { egui::Theme::Dark } else { egui::Theme::Light });
    }

    /// Which theme `visuals` is, read back from its `dark_mode` (exact for
    /// anything applied by [`Theme::apply`], which only tells dark from
    /// light).
    pub fn current(visuals: &egui::Visuals) -> Self {
        if visuals.dark_mode {
            Self::GruvboxDark
        } else {
            Self::GruvboxLight
        }
    }

    /// A Gruvbox accent that reads well on this theme's background:
    /// the bright variant on dark, the faded one on light. Index 0..7 cycles
    /// blue, orange, aqua, purple, yellow, green, red: for plot series and
    /// drawn geometry that should follow the theme.
    pub fn accent(self, i: usize) -> Color32 {
        const DARK: [Color32; 7] = [BRIGHT_BLUE, BRIGHT_ORANGE, BRIGHT_AQUA, BRIGHT_PURPLE, BRIGHT_YELLOW, BRIGHT_GREEN, BRIGHT_RED];
        const LIGHT: [Color32; 7] = [FADED_BLUE, FADED_ORANGE, FADED_AQUA, FADED_PURPLE, FADED_YELLOW, FADED_GREEN, FADED_RED];
        if self.is_dark() {
            DARK[i % 7]
        } else {
            LIGHT[i % 7]
        }
    }
}

/// A Dark/Light selector; applies the theme to the context when it changes.
/// Returns whether it changed.
pub fn selector(ui: &mut egui::Ui, theme: &mut Theme) -> bool {
    let before = *theme;
    ui.horizontal(|ui| {
        ui.label("Theme");
        for t in Theme::ALL {
            ui.selectable_value(theme, t, t.label());
        }
    });
    let changed = *theme != before;
    if changed {
        theme.apply(ui.ctx());
    }
    changed
}

/// The Gruvbox [`egui::Visuals`] (moved from `kovan`'s `app::theme`).
///
/// Starts from egui's own `Visuals::dark()`/`light()` (so every field not set
/// here keeps egui's defaults) and overrides the colours: `dark0_hard` /
/// `light0_hard` for the deepest background, `dark1..3` / `light1..3` for
/// panels and widget states, one accent (blue) for selection and links, and
/// the reds for errors.
pub fn gruvbox_visuals(dark: bool) -> egui::Visuals {
    let mut visuals = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };

    let (bg0, bg1, bg2, bg3, fg, accent, warn) = if dark {
        (DARK0_HARD, DARK1, DARK2, DARK3, LIGHT1, BRIGHT_BLUE, BRIGHT_YELLOW)
    } else {
        (LIGHT0_HARD, LIGHT1, LIGHT2, LIGHT3, LIGHT_FG, NEUTRAL_BLUE, BRIGHT_YELLOW)
    };

    visuals.panel_fill = bg0;
    visuals.window_fill = bg0;
    visuals.extreme_bg_color = bg0;
    visuals.faint_bg_color = bg1;
    visuals.hyperlink_color = accent;
    visuals.warn_fg_color = warn;
    visuals.error_fg_color = if dark { BRIGHT_RED } else { FADED_RED };
    visuals.selection.bg_fill = accent;
    visuals.selection.stroke.color = if dark { bg0 } else { LIGHT0_HARD };

    visuals.widgets.noninteractive.bg_fill = bg0;
    visuals.widgets.noninteractive.weak_bg_fill = bg1;
    visuals.widgets.noninteractive.fg_stroke.color = fg;

    visuals.widgets.inactive.bg_fill = bg1;
    visuals.widgets.inactive.weak_bg_fill = bg1;
    visuals.widgets.inactive.fg_stroke.color = fg;

    visuals.widgets.hovered.bg_fill = bg2;
    visuals.widgets.hovered.weak_bg_fill = bg2;
    visuals.widgets.hovered.fg_stroke.color = fg;
    visuals.widgets.hovered.bg_stroke.color = accent;

    visuals.widgets.active.bg_fill = bg3;
    visuals.widgets.active.weak_bg_fill = bg3;
    visuals.widgets.active.fg_stroke.color = fg;
    visuals.widgets.active.bg_stroke.color = accent;

    visuals.widgets.open.bg_fill = bg2;
    visuals.widgets.open.weak_bg_fill = bg2;
    visuals.widgets.open.fg_stroke.color = fg;

    visuals
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default is Gruvbox Dark, and each theme's visuals carry its
    /// background and read back as itself.
    #[test]
    fn dark_is_the_default_and_each_theme_round_trips() {
        assert_eq!(Theme::default(), Theme::GruvboxDark);
        for t in Theme::ALL {
            let v = t.visuals();
            assert_eq!(Theme::current(&v), t);
            assert_eq!(v.panel_fill, if t.is_dark() { DARK0_HARD } else { LIGHT0_HARD });
        }
    }

    /// The selector applies the theme to the context when clicked, and the
    /// context's visuals are then the Gruvbox ones (headless egui frame).
    #[test]
    fn apply_sets_the_context_visuals() {
        let ctx = egui::Context::default();
        Theme::GruvboxLight.apply(&ctx);
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let mut t = Theme::GruvboxLight;
            assert!(!selector(ui, &mut t));
        });
        assert_eq!(ctx.global_style().visuals.panel_fill, LIGHT0_HARD);
        Theme::GruvboxDark.apply(&ctx);
        assert_eq!(ctx.global_style().visuals.panel_fill, DARK0_HARD);
        // The other slot is Gruvbox too: a system theme flip stays Gruvbox.
        assert_eq!(ctx.style_of(egui::Theme::Light).visuals.panel_fill, LIGHT0_HARD);
    }

    /// Accents differ between the themes and cycle with period 7.
    #[test]
    fn accents_follow_the_theme() {
        for i in 0..7 {
            assert_ne!(Theme::GruvboxDark.accent(i), Theme::GruvboxLight.accent(i));
            assert_eq!(Theme::GruvboxDark.accent(i), Theme::GruvboxDark.accent(i + 7));
        }
    }
}
