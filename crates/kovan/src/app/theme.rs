//! GUI theme: Gruvbox Dark and Gruvbox Light.
//!
//! Ported from `tampines-steam-tables-gui`'s `theme.rs` (GitHub issue #30,
//! "For theming ... only gruvbox light and dark work" — the maintainer asked
//! for `kovan`'s GUI to match that crate's example GUI). This is a
//! straightforward reuse of the same `egui::Visuals` construction; the
//! `figure_palette`/`live_ink_colour` half of the original file is not
//! ported — those exist for `tampines-steam-tables-gui`'s separate exported
//! PNG/PDF/SVG figure styling, which `kovan`'s digitiser/reader has no
//! equivalent of.
//!
//! # Gruvbox provenance and licence
//!
//! The Gruvbox colour palette below is based on
//! [morhetz/gruvbox](https://github.com/morhetz/gruvbox), licensed under the
//! MIT License. Only the published hex colour values are reproduced here
//! (`GRUVBOX_*` constants); no source code from that project is used.

use eframe::egui;

/// Which visual theme the GUI chrome uses.
///
/// An enum, not a trait object or a raw string: the set of themes is closed,
/// so a `match` over it is exhaustive at compile time, per the workspace Rust
/// design rules.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GuiTheme {
    /// Gruvbox dark background, e.g. `#282828`.
    #[default]
    GruvboxDark,
    /// Gruvbox light background, e.g. `#fbf1c7`.
    GruvboxLight,
}

impl GuiTheme {
    /// Every theme, in the order the selector shows them.
    pub const ALL: [GuiTheme; 2] = [GuiTheme::GruvboxDark, GuiTheme::GruvboxLight];

    /// Selector label.
    pub fn label(self) -> &'static str {
        match self {
            Self::GruvboxDark => "Gruvbox Dark",
            Self::GruvboxLight => "Gruvbox Light",
        }
    }

    /// Applies this theme to the `egui` context immediately, via custom
    /// [`egui::Visuals`] built from the palette below.
    pub fn apply(self, ctx: &egui::Context) {
        match self {
            Self::GruvboxDark => ctx.set_visuals(gruvbox_visuals(true)),
            Self::GruvboxLight => ctx.set_visuals(gruvbox_visuals(false)),
        }
    }
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
