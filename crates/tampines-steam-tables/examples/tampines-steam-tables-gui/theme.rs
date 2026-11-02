//! GUI theme controls: Gruvbox Dark and Gruvbox Light.
//!
//! Issue #26 originally asked for "Light | Dark | System | Gruvbox", "better
//! if feasible: ... Gruvbox Light | Gruvbox Dark", and all five were
//! implemented. Per the maintainer's 2026-08-21 follow-up ("For theming
//! buttons, only gruvbox light and dark work. The light and dark are buggy.
//! Just use the gruvbox light and dark buttons, everything else is
//! redundant"), the plain `egui::ThemePreference`-backed `Light`/`Dark`/
//! `System` variants were dropped and only the two Gruvbox variants remain.
//!
//! **MOVED 2026-11-02:** the theme enum, its `egui::Visuals` and the Gruvbox
//! palette now come from the workspace's shared colour-theming crate `bugis`
//! (this file was the original those were ported from, via `kovan`).
//! [`GuiTheme`] is a re-export of [`bugis::Theme`]; this module keeps only
//! what is this GUI's own: the exported-figure palette ([`figure_palette`])
//! and the live-canvas ink ([`live_ink_colour`]).
//!
//! # Gruvbox provenance and licence
//!
//! The Gruvbox colour values (`bugis::palette`) are based on
//! [morhetz/gruvbox](https://github.com/morhetz/gruvbox), licensed under the
//! MIT License. Only the published hex colour values are reproduced; no
//! source code from that project is used. See this crate's example
//! `README.md` for the same attribution.
//!
//! # Live canvas vs. exported figure
//!
//! [`GuiTheme::apply`] changes the `egui` chrome and the live `egui_plot`
//! canvas only. The exported PNG/PDF/SVG figures are a separate concern
//! ([`crate::figure`]) with their own export-style selector, and default to a
//! fixed light "publication" palette regardless of which GUI theme is active,
//! per issue #26's own suggestion ("If only one export style is implemented
//! first, choose Light publication").

use bugis::palette::{DARK0_HARD, DARK2, LIGHT0_HARD, LIGHT1, LIGHT2, LIGHT_FG};
use eframe::egui;

/// Which visual theme the GUI chrome and live plot canvas use: the shared
/// [`bugis::Theme`] (Gruvbox Dark or Gruvbox Light).
pub use bugis::Theme as GuiTheme;

/// The exported-figure palette `theme` would use for the "Current theme"
/// export style.
///
/// Reuses the same Gruvbox constants (`bugis::palette`) as the live `egui`
/// chrome rather than duplicating them, so the exported figure and the
/// chrome can never drift onto two different "Gruvbox".
pub fn figure_palette(theme: GuiTheme) -> crate::figure::FigurePalette {
    use crate::figure::FigurePalette;

    let (background, ink, grid) = if theme.is_dark() {
        (DARK0_HARD, LIGHT1, DARK2)
    } else {
        (LIGHT0_HARD, LIGHT_FG, LIGHT2)
    };
    FigurePalette {
        background: color32_to_rgb(background),
        ink: color32_to_rgb(ink),
        grid: color32_to_rgb(grid),
    }
}

/// This module's colours are `egui::Color32`; [`crate::figure::FigurePalette`]
/// wants [`crate::figure::Rgb`]. Both are plain 8-bit sRGB triples, so this
/// is a lossless field-for-field copy, kept as one function so the two colour
/// types never drift apart silently.
fn color32_to_rgb(c: egui::Color32) -> crate::figure::Rgb {
    crate::figure::Rgb::new(c.r(), c.g(), c.b())
}

/// The "ink" colour (axes, dome, critical/triple points, region boundaries)
/// for the live `egui_plot` canvas, tuned for readability against the current
/// `egui` visuals. Grid lines are not covered here: `egui_plot` draws its own
/// grid from the ambient `egui::Style` (`faint_bg_color`/`weak_text_color`),
/// which the theme in this module already sets appropriately, so no override
/// is needed for grid lines to stay visibly weaker than the plotted curves.
///
/// The exported figure has its own fixed light-publication palette
/// ([`crate::figure::INK`]) and does not use this — this function exists
/// because that fixed black is otherwise close to invisible on a dark
/// canvas background (issue #26: "In dark mode, the saturation dome should
/// not be faint").
///
/// Reads `dark_mode` off the live `egui::Visuals` rather than off
/// [`GuiTheme`] directly, so it stays correct regardless of which of the two
/// Gruvbox variants is active.
pub fn live_ink_colour(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgb(235, 235, 235)
    } else {
        egui::Color32::from_rgb(20, 20, 20)
    }
}
