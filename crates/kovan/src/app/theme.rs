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
//! # Artifact-kind accent colours (op-30um.5, GitHub issue #35 "layer 2")
//!
//! [`artifact_accent`] is the **single, central** place that resolves a
//! [`crate::artifact::ArtifactKind`] to a colour. Every place in the GUI
//! that draws a source-anchored artifact — today the PDF canvas
//! (`app::pdf_reader`), and in future the mindmap — must call it rather than
//! writing a literal `Color32` at the call site, so the same kind always
//! reads as the same colour everywhere, in both themes.
//!
//! # Gruvbox provenance and licence
//!
//! The Gruvbox colour palette below is based on
//! [morhetz/gruvbox](https://github.com/morhetz/gruvbox), licensed under the
//! MIT License. Only the published hex colour values are reproduced here
//! (`GRUVBOX_*` constants); no source code from that project is used.

use eframe::egui;

use crate::artifact::ArtifactKind;

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

    /// Reads back which of the two themes is currently in effect, from
    /// `visuals`' own [`egui::Visuals::dark_mode`] — e.g. `ui.visuals()` —
    /// rather than requiring every caller to separately track or receive a
    /// copy of whichever `GuiTheme` the top-level app last applied.
    ///
    /// This is how a panel that does not own the app's `GuiTheme` field
    /// itself (e.g. `app::pdf_reader`, which draws artifact-accent boxes but
    /// is not handed the app's own theme selector) recovers "which theme is
    /// this?" from the `egui::Ui`/`Visuals` it is already given. Exact for
    /// anything this crate applies via [`GuiTheme::apply`], since
    /// [`GuiTheme`] only ever distinguishes dark from light.
    pub fn current(visuals: &egui::Visuals) -> Self {
        if visuals.dark_mode {
            Self::GruvboxDark
        } else {
            Self::GruvboxLight
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
const GRUVBOX_NEUTRAL_YELLOW: egui::Color32 = egui::Color32::from_rgb(0xd7, 0x99, 0x21);
const GRUVBOX_FADED_RED: egui::Color32 = egui::Color32::from_rgb(0x9d, 0x00, 0x06);
const GRUVBOX_BRIGHT_RED: egui::Color32 = egui::Color32::from_rgb(0xfb, 0x49, 0x34);
// Added for the artifact-kind accent mapping (op-30um.5) — the base palette
// above only ever needed one accent (blue) plus warn/error, so aqua, purple
// and orange were not yet defined.
const GRUVBOX_BRIGHT_AQUA: egui::Color32 = egui::Color32::from_rgb(0x8e, 0xc0, 0x7c);
const GRUVBOX_NEUTRAL_AQUA: egui::Color32 = egui::Color32::from_rgb(0x68, 0x9d, 0x6a);
const GRUVBOX_BRIGHT_PURPLE: egui::Color32 = egui::Color32::from_rgb(0xd3, 0x86, 0x9b);
const GRUVBOX_NEUTRAL_PURPLE: egui::Color32 = egui::Color32::from_rgb(0xb1, 0x62, 0x86);
const GRUVBOX_BRIGHT_ORANGE: egui::Color32 = egui::Color32::from_rgb(0xfe, 0x80, 0x19);
const GRUVBOX_NEUTRAL_ORANGE: egui::Color32 = egui::Color32::from_rgb(0xd6, 0x5d, 0x0e);

/// The accent colour that identifies one [`ArtifactKind`] wherever a saved,
/// source-anchored artifact is drawn — today the PDF canvas's region boxes
/// (`app::pdf_reader`), and, per the GitHub issue #35 "layer 2" prototype
/// this ports, intended to stay the *same* mapping when the mindmap grows
/// its own artifact nodes. Resolved centrally, here, so nothing downstream
/// invents its own literal `Color32` for an artifact kind — see the module
/// doc.
///
/// | [`ArtifactKind`] | accent |
/// |---|---|
/// | `Annotation` / `Note` | Gruvbox yellow |
/// | `DigitisedGraph` (+ CSV payload) | Gruvbox aqua |
/// | `DigitisedTable` (+ CSV payload) | Gruvbox blue |
/// | `Formula` | Gruvbox purple |
/// | `SourceReference` | Gruvbox orange |
///
/// Each kind has a `dark`-picked and a `light`-picked hex value (Gruvbox's
/// own bright/neutral split — the brighter, cooler variant reads clearly on
/// a dark background; the neutral, more saturated variant carries the same
/// contrast against Gruvbox's pale `light0_hard` background), so the same
/// kind is legible, and visually distinct from every other kind, in both
/// [`GuiTheme`] variants.
///
/// A future "corrupt/unavailable artifact" health layer is expected to add
/// a red accent alongside this table — see [`unavailable_accent`], a
/// documented seam this function deliberately does not cover: nothing in
/// this crate today classifies an artifact as corrupt/unavailable, so there
/// is no case to add here yet, and inventing one would pre-empt that
/// layer's own design.
pub fn artifact_accent(kind: ArtifactKind, theme: GuiTheme) -> egui::Color32 {
    let dark = matches!(theme, GuiTheme::GruvboxDark);
    match kind {
        // The paper header draws no region box; it shares the note accent
        // so a mindmap node for it is not left uncoloured.
        // Connectors and mindmaps have no page region to tint; they share
        // the note accent so a graph node for them is never uncoloured.
        ArtifactKind::Relation | ArtifactKind::Mindmap | ArtifactKind::Paper => {
            if dark {
                GRUVBOX_BRIGHT_YELLOW
            } else {
                GRUVBOX_NEUTRAL_YELLOW
            }
        }
        ArtifactKind::Annotation | ArtifactKind::Note => {
            if dark {
                GRUVBOX_BRIGHT_YELLOW
            } else {
                GRUVBOX_NEUTRAL_YELLOW
            }
        }
        ArtifactKind::DigitisedGraph => {
            if dark {
                GRUVBOX_BRIGHT_AQUA
            } else {
                GRUVBOX_NEUTRAL_AQUA
            }
        }
        ArtifactKind::DigitisedTable => {
            if dark {
                GRUVBOX_BRIGHT_BLUE
            } else {
                GRUVBOX_NEUTRAL_BLUE
            }
        }
        ArtifactKind::Formula => {
            if dark {
                GRUVBOX_BRIGHT_PURPLE
            } else {
                GRUVBOX_NEUTRAL_PURPLE
            }
        }
        ArtifactKind::SourceReference => {
            if dark {
                GRUVBOX_BRIGHT_ORANGE
            } else {
                GRUVBOX_NEUTRAL_ORANGE
            }
        }
    }
}

/// Reserved Gruvbox red for a corrupt/unavailable artifact — the health
/// layer op-30um.5's own spec explicitly defers (see the module doc and
/// [`artifact_accent`]'s doc). **Not called from anywhere in this crate
/// yet.** It is added now, alongside the rest of the artifact-kind accent
/// vocabulary, so that whichever future bead builds the health layer picks
/// this colour up rather than inventing a second red somewhere else in the
/// GUI.
///
/// op-30um.6 checked for a natural call site (the kind-aware right-click
/// menu) and found none: that menu only knows an artifact's *kind*, never
/// whether its region/CSV/anchor is actually intact, so it has nothing
/// health-related to colour yet. `#[allow(dead_code)]` rather than deleting
/// this — it stays the reserved colour for whenever an artifact-health
/// check exists to call it.
#[allow(dead_code)]
pub fn unavailable_accent(theme: GuiTheme) -> egui::Color32 {
    match theme {
        GuiTheme::GruvboxDark => GRUVBOX_BRIGHT_RED,
        GuiTheme::GruvboxLight => GRUVBOX_FADED_RED,
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_KINDS: [ArtifactKind; 6] = [
        ArtifactKind::Note,
        ArtifactKind::Annotation,
        ArtifactKind::SourceReference,
        ArtifactKind::Formula,
        ArtifactKind::DigitisedTable,
        ArtifactKind::DigitisedGraph,
    ];

    #[test]
    fn note_and_annotation_share_the_same_accent() {
        // §14's Note/Annotation split is about anchoring, not colour — the
        // layer-2 spec groups them under one "yellow" row.
        for theme in GuiTheme::ALL {
            assert_eq!(
                artifact_accent(ArtifactKind::Note, theme),
                artifact_accent(ArtifactKind::Annotation, theme)
            );
        }
    }

    #[test]
    fn every_other_kind_gets_a_visually_distinct_accent_in_each_theme() {
        for theme in GuiTheme::ALL {
            let distinct = [
                ArtifactKind::Annotation,
                ArtifactKind::DigitisedGraph,
                ArtifactKind::DigitisedTable,
                ArtifactKind::Formula,
                ArtifactKind::SourceReference,
            ];
            for (i, a) in distinct.iter().enumerate() {
                for b in &distinct[i + 1..] {
                    assert_ne!(
                        artifact_accent(*a, theme),
                        artifact_accent(*b, theme),
                        "{a:?} and {b:?} must not share an accent under {theme:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn accent_is_defined_for_every_kind_in_both_themes() {
        for theme in GuiTheme::ALL {
            for kind in ALL_KINDS {
                // Just asserts the match is exhaustive and returns something
                // opaque — a compile-time guarantee already, but pinning it
                // here means adding a 7th `ArtifactKind` variant with no
                // corresponding match arm fails a *test*, not just silently
                // falls through to a wildcard (there is deliberately no `_`
                // arm in `artifact_accent`).
                let c = artifact_accent(kind, theme);
                assert_eq!(c.a(), 255, "accent must be fully opaque: {kind:?}/{theme:?}");
            }
        }
    }

    #[test]
    fn dark_and_light_accents_differ_for_every_kind() {
        // Reusing the exact same hex in both themes was the failure mode
        // `gruvbox_visuals`'s own `warn` colour has today (bright yellow in
        // both) — deliberately not repeated here: every artifact accent has
        // its own dark/light pair so contrast against each background holds.
        for kind in ALL_KINDS {
            assert_ne!(
                artifact_accent(kind, GuiTheme::GruvboxDark),
                artifact_accent(kind, GuiTheme::GruvboxLight),
                "{kind:?} should pick a different hex per theme"
            );
        }
    }

    #[test]
    fn unavailable_accent_is_gruvbox_red_and_differs_per_theme() {
        assert_eq!(unavailable_accent(GuiTheme::GruvboxDark), GRUVBOX_BRIGHT_RED);
        assert_eq!(unavailable_accent(GuiTheme::GruvboxLight), GRUVBOX_FADED_RED);
    }

    #[test]
    fn current_reads_back_dark_mode_from_visuals() {
        assert_eq!(
            GuiTheme::current(&egui::Visuals::dark()),
            GuiTheme::GruvboxDark
        );
        assert_eq!(
            GuiTheme::current(&egui::Visuals::light()),
            GuiTheme::GruvboxLight
        );
        // And the visuals this crate actually builds round-trip too.
        assert_eq!(
            GuiTheme::current(&gruvbox_visuals(true)),
            GuiTheme::GruvboxDark
        );
        assert_eq!(
            GuiTheme::current(&gruvbox_visuals(false)),
            GuiTheme::GruvboxLight
        );
    }
}
