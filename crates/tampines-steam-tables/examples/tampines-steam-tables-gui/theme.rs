//! GUI theme controls: Light, Dark, System, and Gruvbox (Dark/Light).
//!
//! Issue #26 asks for "Light | Dark | System | Gruvbox", "better if feasible:
//! ... Gruvbox Light | Gruvbox Dark". Both Gruvbox variants are implemented,
//! since the extra work over one is small — the palette below already carries
//! matched light and dark rows.
//!
//! # Gruvbox provenance and licence
//!
//! The Gruvbox colour palette below is based on
//! [morhetz/gruvbox](https://github.com/morhetz/gruvbox), licensed under the
//! MIT License. Only the published hex colour values are reproduced here
//! (`GRUVBOX_*` constants); no source code from that project is used. See
//! this crate's example `README.md` for the same attribution.
//!
//! # Live canvas vs. exported figure
//!
//! [`GuiTheme::apply`] changes the `egui` chrome and the live `egui_plot`
//! canvas only. The exported PNG/PDF/SVG figures are a separate concern
//! ([`crate::figure`]) with their own export-style selector, and default to a
//! fixed light "publication" palette regardless of which GUI theme is active,
//! per issue #26's own suggestion ("If only one export style is implemented
//! first, choose Light publication").

use eframe::egui;

/// Which visual theme the GUI chrome and live plot canvas use.
///
/// An enum, not a trait object or a raw string: the set of themes is closed,
/// so a `match` over it is exhaustive at compile time, per the workspace Rust
/// design rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiTheme {
    /// Forces `egui`'s built-in light visuals.
    Light,
    /// Forces `egui`'s built-in dark visuals.
    Dark,
    /// Follows the OS theme preference (via `egui::ThemePreference::System`).
    System,
    /// Gruvbox dark background, e.g. `#282828`.
    GruvboxDark,
    /// Gruvbox light background, e.g. `#fbf1c7`.
    GruvboxLight,
}

impl GuiTheme {
    /// Every theme, in the order the selector shows them.
    pub const ALL: [GuiTheme; 5] = [
        GuiTheme::Light,
        GuiTheme::Dark,
        GuiTheme::System,
        GuiTheme::GruvboxDark,
        GuiTheme::GruvboxLight,
    ];

    /// Selector label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::System => "System",
            Self::GruvboxDark => "Gruvbox Dark",
            Self::GruvboxLight => "Gruvbox Light",
        }
    }

    /// Applies this theme to the `egui` context immediately.
    ///
    /// `Light`/`Dark`/`System` go through `egui`'s own
    /// [`egui::ThemePreference`] (so `System` gets `egui`'s OS-theme-following
    /// behaviour for free); the two Gruvbox variants set custom [`egui::Visuals`]
    /// built from the palette below.
    pub fn apply(self, ctx: &egui::Context) {
        match self {
            Self::Light => ctx.set_theme(egui::ThemePreference::Light),
            Self::Dark => ctx.set_theme(egui::ThemePreference::Dark),
            Self::System => ctx.set_theme(egui::ThemePreference::System),
            Self::GruvboxDark => ctx.set_visuals(gruvbox_visuals(true)),
            Self::GruvboxLight => ctx.set_visuals(gruvbox_visuals(false)),
        }
    }

    /// The exported-figure palette this theme would use for the "Current
    /// theme" export style. `system_is_dark` resolves `GuiTheme::System`
    /// (read from `ui.visuals().dark_mode` at the point of export, the same
    /// source [`live_ink_colour`] uses for the live canvas).
    ///
    /// Reuses the same Gruvbox hex constants as [`gruvbox_visuals`] rather
    /// than duplicating them, so the exported figure and the live `egui`
    /// chrome can never drift onto two different "Gruvbox".
    pub fn figure_palette(self, system_is_dark: bool) -> crate::figure::FigurePalette {
        use crate::figure::FigurePalette;

        let dark = match self {
            Self::Light => false,
            Self::Dark => true,
            Self::System => system_is_dark,
            Self::GruvboxDark => true,
            Self::GruvboxLight => false,
        };

        if matches!(self, Self::GruvboxDark | Self::GruvboxLight) {
            let (background, ink, grid) = if dark {
                (GRUVBOX_DARK0_HARD, GRUVBOX_LIGHT1, GRUVBOX_DARK2)
            } else {
                (GRUVBOX_LIGHT0_HARD, GRUVBOX_LIGHT_FG, GRUVBOX_LIGHT2)
            };
            FigurePalette {
                background: color32_to_rgb(background),
                ink: color32_to_rgb(ink),
                grid: color32_to_rgb(grid),
            }
        } else if dark {
            FigurePalette::DARK
        } else {
            FigurePalette::LIGHT_PUBLICATION
        }
    }
}

/// This module's colours are `egui::Color32` (what `egui::Visuals` wants);
/// [`crate::figure::FigurePalette`] wants [`crate::figure::Rgb`]. Both are
/// plain 8-bit sRGB triples, so this is a lossless field-for-field copy, kept
/// as one function so the two colour types never drift apart silently.
fn color32_to_rgb(c: egui::Color32) -> crate::figure::Rgb {
    crate::figure::Rgb::new(c.r(), c.g(), c.b())
}

// ---------------------------------------------------------------------------
// Gruvbox palette (MIT, morhetz/gruvbox) -- see the module doc for provenance.
// ---------------------------------------------------------------------------

const GRUVBOX_DARK0_HARD: egui::Color32 = egui::Color32::from_rgb(0x1d, 0x20, 0x21);
const GRUVBOX_DARK1: egui::Color32 = egui::Color32::from_rgb(0x3c, 0x38, 0x36);
const GRUVBOX_DARK2: egui::Color32 = egui::Color32::from_rgb(0x50, 0x49, 0x45);
const GRUVBOX_DARK3: egui::Color32 = egui::Color32::from_rgb(0x66, 0x5c, 0x54);
const GRUVBOX_LIGHT0_HARD: egui::Color32 = egui::Color32::from_rgb(0xf9, 0xf5, 0xd7);
const GRUVBOX_LIGHT1: egui::Color32 = egui::Color32::from_rgb(0xeb, 0xdb, 0xb2);
const GRUVBOX_LIGHT2: egui::Color32 = egui::Color32::from_rgb(0xd5, 0xc4, 0xa1);
const GRUVBOX_LIGHT3: egui::Color32 = egui::Color32::from_rgb(0xbd, 0xae, 0x93);
const GRUVBOX_LIGHT_FG: egui::Color32 = egui::Color32::from_rgb(0x3c, 0x38, 0x36);
const GRUVBOX_BRIGHT_BLUE: egui::Color32 = egui::Color32::from_rgb(0x83, 0xa5, 0x98);
const GRUVBOX_NEUTRAL_BLUE: egui::Color32 = egui::Color32::from_rgb(0x45, 0x85, 0x88);
const GRUVBOX_BRIGHT_YELLOW: egui::Color32 = egui::Color32::from_rgb(0xfa, 0xbd, 0x2f);
const GRUVBOX_FADED_RED: egui::Color32 = egui::Color32::from_rgb(0x9d, 0x00, 0x06);
const GRUVBOX_BRIGHT_RED: egui::Color32 = egui::Color32::from_rgb(0xfb, 0x49, 0x34);

/// Builds a full [`egui::Visuals`] from the Gruvbox palette.
///
/// Starts from `egui`'s own `Visuals::dark()`/`Visuals::light()` defaults (so
/// every field this function does not touch — corner radii, shadows, cursor
/// behaviour — keeps `egui`'s sensible defaults) and overrides only the
/// colour fields, following the standard Gruvbox background/foreground
/// tiering: `dark0_hard`/`light0_hard` for the deepest background,
/// `dark1..3`/`light1..3` for panels and widget states, one bright accent
/// (`bright_blue`) for selection/hyperlinks, and the faded/bright reds for
/// warnings and errors.
fn gruvbox_visuals(dark: bool) -> egui::Visuals {
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    let (bg0, bg1, bg2, bg3, fg, accent, warn) = if dark {
        (
            GRUVBOX_DARK0_HARD,
            GRUVBOX_DARK1,
            GRUVBOX_DARK2,
            GRUVBOX_DARK3,
            GRUVBOX_LIGHT1,
            GRUVBOX_BRIGHT_BLUE,
            GRUVBOX_BRIGHT_YELLOW,
        )
    } else {
        (
            GRUVBOX_LIGHT0_HARD,
            GRUVBOX_LIGHT1,
            GRUVBOX_LIGHT2,
            GRUVBOX_LIGHT3,
            GRUVBOX_LIGHT_FG,
            GRUVBOX_NEUTRAL_BLUE,
            GRUVBOX_BRIGHT_YELLOW,
        )
    };

    visuals.panel_fill = bg0;
    visuals.window_fill = bg0;
    visuals.extreme_bg_color = bg0;
    visuals.faint_bg_color = bg1;
    visuals.hyperlink_color = accent;
    visuals.warn_fg_color = warn;
    visuals.error_fg_color = if dark {
        GRUVBOX_BRIGHT_RED
    } else {
        GRUVBOX_FADED_RED
    };
    visuals.selection.bg_fill = accent;
    visuals.selection.stroke.color = if dark { bg0 } else { GRUVBOX_LIGHT0_HARD };

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
/// [`GuiTheme`] directly, so it is correct for `GuiTheme::System` (which
/// resolves to a concrete dark/light state inside `egui` itself, not
/// something this module tracks) as well as for the two Gruvbox variants.
pub fn live_ink_colour(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgb(235, 235, 235)
    } else {
        egui::Color32::from_rgb(20, 20, 20)
    }
}
