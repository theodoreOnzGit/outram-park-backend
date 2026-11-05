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

/// Theme-derived colours for what a GUI DRAWS itself (a demo's main view,
/// a hand-painted plot, status lines over a picture), so a painter never
/// hardcodes RGB and Dark/Light switch every drawn view the same way.
///
/// Read from the [`egui::Visuals`] the ui is using (background, text, warn,
/// error) and from the theme's [`crate::gruvbox::Roles`] (cards, borders,
/// muted text, success). Physics data encodings (material palettes, colour
/// maps) are NOT chrome and do not belong here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ink {
    /// Which theme the visuals are.
    pub theme: Theme,
    /// Main-view background (`extreme_bg_color`: `dark0_hard` / `light0_hard`).
    pub bg: Color32,
    /// A card or inset plot drawn on the main view (`dark0` / `light0`).
    pub card: Color32,
    /// Card outlines and axes (`dark2` / `light2`).
    pub border: Color32,
    /// Faint grid lines (`dark1` / `light1`).
    pub grid: Color32,
    /// Ordinary text (the visuals' text colour).
    pub text: Color32,
    /// Secondary text: tick labels, captions (`light4` / `dark4`).
    pub muted: Color32,
    /// Warnings (`warn_fg_color`).
    pub warn: Color32,
    /// Errors (`error_fg_color`).
    pub error: Color32,
    /// Success / "converged" (`Roles::success`).
    pub ok: Color32,
    /// Translucent backing behind a label drawn over a picture.
    pub backing: Color32,
}

impl Ink {
    /// The ink for these visuals.
    pub fn of(visuals: &egui::Visuals) -> Self {
        let theme = Theme::current(visuals);
        let r = crate::gruvbox::Roles::of(theme.is_dark());
        let bg = visuals.extreme_bg_color;
        Self {
            theme,
            bg,
            card: r.canvas.into(),
            border: r.border.into(),
            grid: r.border_subtle.into(),
            text: visuals.text_color(),
            muted: r.text_muted.into(),
            warn: visuals.warn_fg_color,
            error: visuals.error_fg_color,
            ok: r.success.into(),
            backing: Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), 215),
        }
    }

    /// The ink of the theme this ui draws with.
    pub fn of_ui(ui: &egui::Ui) -> Self {
        Self::of(ui.visuals())
    }

    /// The ink of the context's current theme (for a painter-only helper:
    /// `Ink::of_ctx(painter.ctx())`).
    pub fn of_ctx(ctx: &egui::Context) -> Self {
        Self::of(&ctx.global_style().visuals)
    }

    /// [`Theme::accent`] of this ink's theme: plot series, drawn geometry.
    pub fn accent(&self, i: usize) -> Color32 {
        self.theme.accent(i)
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
        // Faded yellow on light (2026-11-05): bright yellow on `light0_hard`
        // is about 1.6:1 and could not be read; faded is `Roles::warning`.
        (LIGHT0_HARD, LIGHT1, LIGHT2, LIGHT3, LIGHT_FG, NEUTRAL_BLUE, FADED_YELLOW)
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

    /// The ink follows the theme: background, text, warn and error are the
    /// visuals' own, and every chrome colour differs between Dark and Light.
    /// Light's warning is the faded yellow (bright yellow is unreadable on
    /// `light0_hard`).
    #[test]
    fn ink_follows_the_theme() {
        let d = Ink::of(&Theme::GruvboxDark.visuals());
        let l = Ink::of(&Theme::GruvboxLight.visuals());
        assert_eq!(d.theme, Theme::GruvboxDark);
        assert_eq!(l.theme, Theme::GruvboxLight);
        assert_eq!(d.bg, DARK0_HARD);
        assert_eq!(l.bg, LIGHT0_HARD);
        assert_eq!(d.text, LIGHT1);
        assert_eq!(l.text, LIGHT_FG);
        assert_eq!(l.warn, FADED_YELLOW);
        assert_eq!(d.warn, BRIGHT_YELLOW);
        assert_eq!(d.error, BRIGHT_RED);
        assert_eq!(l.error, FADED_RED);
        for (a, b) in [(d.card, l.card), (d.border, l.border), (d.grid, l.grid), (d.muted, l.muted), (d.ok, l.ok), (d.backing, l.backing)] {
            assert_ne!(a, b);
        }
        assert_eq!(d.accent(1), Theme::GruvboxDark.accent(1));
        let ctx = egui::Context::default();
        Theme::GruvboxLight.apply(&ctx);
        assert_eq!(Ink::of_ctx(&ctx), l);
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| assert_eq!(Ink::of_ui(ui), l));
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
