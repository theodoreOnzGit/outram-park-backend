//! BUGIS — **B**randing, **U**ser-interface and **G**raphics for
//! **I**nteractive **S**imulators.
//!
//! > Research, education and V&V only. See the workspace `RESPONSIBLE_USE.md`.
//!
//! The one place the workspace's egui simulators get their look from: the
//! **Gruvbox** palette, as a dark and a light [`egui::Visuals`], and a small
//! selector widget to switch between them (maintainer, 2026-11-02: "give all
//! simulators a gruvbox theme"; Gruvbox Dark is the default, with a
//! Dark/Light toggle).
//!
//! ```no_run
//! # fn f(ctx: &egui::Context, ui: &mut egui::Ui) {
//! let mut theme = bugis::Theme::default(); // Gruvbox Dark
//! theme.apply(ctx);                        // once, at start-up
//! bugis::selector(ui, &mut theme);         // in the side panel: applies on change
//! # }
//! ```
//!
//! # Provenance
//!
//! The visuals are a **move**, not a rewrite, of `kovan`'s
//! `src/app/theme.rs` `gruvbox_visuals` (itself ported from
//! `tampines-steam-tables-gui`'s `theme.rs`, GitHub issue #30). Both now call
//! this crate, so the three copies cannot drift.
//!
//! The Gruvbox colour palette is based on
//! [morhetz/gruvbox](https://github.com/morhetz/gruvbox), licensed under the
//! MIT License. Only the published hex colour values are reproduced here
//! ([`palette`]); no source code from that project is used.

#![forbid(unsafe_code)]

/// The Gruvbox palette's published hex values (MIT, morhetz/gruvbox).
pub mod palette {
    use egui::Color32;

    pub const DARK0_HARD: Color32 = Color32::from_rgb(0x1d, 0x20, 0x21);
    pub const DARK0: Color32 = Color32::from_rgb(0x28, 0x28, 0x28);
    pub const DARK1: Color32 = Color32::from_rgb(0x3c, 0x38, 0x36);
    pub const DARK2: Color32 = Color32::from_rgb(0x50, 0x49, 0x45);
    pub const DARK3: Color32 = Color32::from_rgb(0x66, 0x5c, 0x54);
    pub const DARK4: Color32 = Color32::from_rgb(0x7c, 0x6f, 0x64);
    pub const GRAY: Color32 = Color32::from_rgb(0x92, 0x83, 0x74);
    pub const LIGHT0_HARD: Color32 = Color32::from_rgb(0xf9, 0xf5, 0xd7);
    pub const LIGHT0: Color32 = Color32::from_rgb(0xfb, 0xf1, 0xc7);
    pub const LIGHT1: Color32 = Color32::from_rgb(0xeb, 0xdb, 0xb2);
    pub const LIGHT2: Color32 = Color32::from_rgb(0xd5, 0xc4, 0xa1);
    pub const LIGHT3: Color32 = Color32::from_rgb(0xbd, 0xae, 0x93);
    pub const LIGHT4: Color32 = Color32::from_rgb(0xa8, 0x99, 0x84);

    pub const BRIGHT_RED: Color32 = Color32::from_rgb(0xfb, 0x49, 0x34);
    pub const BRIGHT_GREEN: Color32 = Color32::from_rgb(0xb8, 0xbb, 0x26);
    pub const BRIGHT_YELLOW: Color32 = Color32::from_rgb(0xfa, 0xbd, 0x2f);
    pub const BRIGHT_BLUE: Color32 = Color32::from_rgb(0x83, 0xa5, 0x98);
    pub const BRIGHT_PURPLE: Color32 = Color32::from_rgb(0xd3, 0x86, 0x9b);
    pub const BRIGHT_AQUA: Color32 = Color32::from_rgb(0x8e, 0xc0, 0x7c);
    pub const BRIGHT_ORANGE: Color32 = Color32::from_rgb(0xfe, 0x80, 0x19);

    pub const NEUTRAL_RED: Color32 = Color32::from_rgb(0xcc, 0x24, 0x1d);
    pub const NEUTRAL_GREEN: Color32 = Color32::from_rgb(0x98, 0x97, 0x1a);
    pub const NEUTRAL_YELLOW: Color32 = Color32::from_rgb(0xd7, 0x99, 0x21);
    pub const NEUTRAL_BLUE: Color32 = Color32::from_rgb(0x45, 0x85, 0x88);
    pub const NEUTRAL_PURPLE: Color32 = Color32::from_rgb(0xb1, 0x62, 0x86);
    pub const NEUTRAL_AQUA: Color32 = Color32::from_rgb(0x68, 0x9d, 0x6a);
    pub const NEUTRAL_ORANGE: Color32 = Color32::from_rgb(0xd6, 0x5d, 0x0e);

    pub const FADED_RED: Color32 = Color32::from_rgb(0x9d, 0x00, 0x06);
    pub const FADED_GREEN: Color32 = Color32::from_rgb(0x79, 0x74, 0x0e);
    pub const FADED_YELLOW: Color32 = Color32::from_rgb(0xb5, 0x76, 0x14);
    pub const FADED_BLUE: Color32 = Color32::from_rgb(0x07, 0x66, 0x78);
    pub const FADED_PURPLE: Color32 = Color32::from_rgb(0x8f, 0x3f, 0x71);
    pub const FADED_AQUA: Color32 = Color32::from_rgb(0x42, 0x7b, 0x58);
    pub const FADED_ORANGE: Color32 = Color32::from_rgb(0xaf, 0x3a, 0x03);

    /// The foreground kovan's light theme used (Gruvbox `dark1`).
    pub const LIGHT_FG: Color32 = DARK1;
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
