//! GUI theme: Gruvbox Dark and Gruvbox Light.
//!
//! ~~Ported from `tampines-steam-tables-gui`'s `theme.rs`~~ **MOVED
//! 2026-11-02:** the theme enum, the `egui::Visuals` construction and the
//! Gruvbox palette now live in the shared `bugis` crate, so every simulator
//! in the workspace draws the same Gruvbox (maintainer, 2026-11-02: "give all
//! simulators a gruvbox theme"). [`GuiTheme`] is a re-export of
//! [`bugis::Theme`] (same variants, `GruvboxDark` the default), and this
//! module keeps only what is kovan's own: the artifact-kind accents and the
//! navigation text style. The original port came from
//! `tampines-steam-tables-gui`'s `theme.rs` (GitHub issue #30).
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
//! The Gruvbox colour values come from `bugis::palette`, based on
//! [morhetz/gruvbox](https://github.com/morhetz/gruvbox) (MIT License). Only
//! the published hex colour values are reproduced; no source code from that
//! project is used.

use bugis::palette::*;
use eframe::egui;

use crate::artifact::ArtifactKind;

/// Which visual theme the GUI chrome uses: the workspace-wide
/// [`bugis::Theme`] (Gruvbox Dark by default, or Gruvbox Light).
pub use bugis::Theme as GuiTheme;

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
                BRIGHT_YELLOW
            } else {
                NEUTRAL_YELLOW
            }
        }
        // The #743 prose kinds have no page region either; they read as
        // notes.
        ArtifactKind::Annotation
        | ArtifactKind::Note
        | ArtifactKind::LessonSection
        | ArtifactKind::WalkStep
        | ArtifactKind::CodeWalk
        | ArtifactKind::RecipeStep
        // The code-review kinds (#764) are text records too.
        | ArtifactKind::Review
        | ArtifactKind::NeedsFix
        | ArtifactKind::Upstream
        | ArtifactKind::DeletedFunctions
        | ArtifactKind::Architecture => {
            if dark {
                BRIGHT_YELLOW
            } else {
                NEUTRAL_YELLOW
            }
        }
        ArtifactKind::DigitisedGraph => {
            if dark {
                BRIGHT_AQUA
            } else {
                NEUTRAL_AQUA
            }
        }
        ArtifactKind::DigitisedTable => {
            if dark {
                BRIGHT_BLUE
            } else {
                NEUTRAL_BLUE
            }
        }
        ArtifactKind::Formula => {
            if dark {
                BRIGHT_PURPLE
            } else {
                NEUTRAL_PURPLE
            }
        }
        ArtifactKind::SourceReference => {
            if dark {
                BRIGHT_ORANGE
            } else {
                NEUTRAL_ORANGE
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
        GuiTheme::GruvboxDark => BRIGHT_RED,
        GuiTheme::GruvboxLight => FADED_RED,
    }
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
                assert_eq!(
                    c.a(),
                    255,
                    "accent must be fully opaque: {kind:?}/{theme:?}"
                );
            }
        }
    }

    #[test]
    fn dark_and_light_accents_differ_for_every_kind() {
        // Reusing the exact same hex in both themes was the failure mode
        // `bugis::gruvbox_visuals`'s own `warn` colour has today (bright yellow in
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
        assert_eq!(
            unavailable_accent(GuiTheme::GruvboxDark),
            BRIGHT_RED
        );
        assert_eq!(
            unavailable_accent(GuiTheme::GruvboxLight),
            FADED_RED
        );
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
            GuiTheme::current(&bugis::gruvbox_visuals(true)),
            GuiTheme::GruvboxDark
        );
        assert_eq!(
            GuiTheme::current(&bugis::gruvbox_visuals(false)),
            GuiTheme::GruvboxLight
        );
    }
}

/// Text size of navigation controls, points: the top bar's tabs and buttons,
/// and the Mindmap and Wiki navigation rows (breadcrumbs, zoom). Larger than
/// the theme's body text so they read as the app's navigation (maintainer
/// direction, 2026-09-22). A UI choice.
pub(crate) const NAVIGATION_FONT_SIZE: f32 = 18.0;

/// Enlarge the button and label text, and the button padding, of `ui` and
/// everything drawn inside it, for navigation controls. Call it at the start
/// of the row's own `ui` (e.g. inside `ui.horizontal`), so it applies to that
/// row and not to the page around it.
pub(crate) fn navigation_style(ui: &mut egui::Ui) {
    let style = ui.style_mut();
    for text_style in [egui::TextStyle::Button, egui::TextStyle::Body] {
        if let Some(font) = style.text_styles.get_mut(&text_style) {
            font.size = NAVIGATION_FONT_SIZE;
        }
    }
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.item_spacing.x = 8.0;
}
