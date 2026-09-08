//! Integrated PDF reader panel — GitHub issue #30's "don't want to
//! screenshot then digitise slowly, everything should be integrated into
//! the reader" (op-95x6), extended to view raster images directly,
//! Okular-style (op-wojr — "I want pdf reader to be able to view images
//! like okular as well").
//!
//! ## One continuous canvas that kovan renders itself (GH issue #35 2026-09-02)
//!
//! kovan renders the PDF pages itself — [`super::page_canvas::PageView`], a
//! bounded page-texture cache over [`kopitiam_pdf::mupdf::rasterize_page`],
//! stacked in a `ScrollArea::show_viewport` continuous column. This is the
//! **only** view.
//!
//! It replaced a two-mode design (a `Read` mode that embedded
//! [`kopitiam_pdf::gui_frontend::PdfReader`], and an `Annotate` mode that was
//! kovan's own single static page). The embedded reader gave fast reading and
//! `/` search for free, but its `PdfReaderOutput` carries only a
//! `Vec<ReaderAction>` — no host-overlay hook and no per-page screen geometry
//! (verified against the published 0.3.2 source; filed as
//! [kopitiam#107](https://github.com/theodoreOnzGit/kopitiam/issues/107)) — so
//! saved region boxes could never be drawn over it or clicked. The maintainer
//! wants boxes visible + double-click-to-edit *everywhere*, so the embedded
//! reader is gone and its one irreplaceable feature, `/`-search, is
//! reimplemented here ([`SearchState`], `line_hits`) over the same
//! `page_to_stext` structured text the select-text tool already uses.
//!
//! [`PdfReader`] is still *held* (as the parsed-document container behind
//! [`ReaderSource::Pdf`] and for [`PdfReader::load_bytes`] hot-reload) but its
//! `show()` is never called, so its render/thumbnail workers never start.
//!
//! `rasterize_page` bakes existing PDF-native `/Annots` into the page raster
//! itself (mupdf's `pdf_run_page_annots` pass) so Okular highlights etc. are
//! visible without a separate overlay renderer.
//!
//! ## Unified box interaction model (op-x9qn, superseding op-gv19's first cut)
//!
//! GitHub issue #30's 2026-08-23 follow-up comment asked for one drawing
//! gesture rather than a toolbar of separate tools: draw a box anywhere on
//! the page, right-click it, and pick what it becomes —
//! **Annotate** (a free-text note), **Digitise graph**, or **Read table**.
//! Right-clicking an *already-saved* annotation box instead offers
//! **Edit**/**Delete**. [`AnnotationTool`] is `None` — the "Pan" tool:
//! drag the page to scroll, arrow keys nudge the view, plus zoom and
//! right-click on an existing box — `DrawBox` (drag to propose a new box), or
//! `SelectText` (drag to select real PDF text lines, op-z9u0). [`ContextMenu`]
//! is the floating menu that appears on a right-click hit; [`Annotation`] is
//! a saved free-text note (page + pixel rect + author + timestamp);
//! [`CropProvenance`] carries the same provenance alongside a Digitise/Read
//! crop so the digitiser tab it hands off to can later save the CSV it
//! produces back into the project's markdown (op-96am) with the same page
//! and pixel bbox recorded on the note.
//!
//! **In-memory only, still** — nothing here persists to disk on its own.
//! Saving into a project's markdown (op-96am) is a separate, explicit action.
//! Ink/freehand strokes are still not implemented: every box is
//! axis-aligned.
//!
//! ## Text selection (op-z9u0)
//!
//! `SelectText` drags out a rectangle and selects the **lines** of real PDF
//! text (not raw pixels) whose bounding box intersects it, via
//! `kopitiam_pdf::mupdf::page_to_stext` — MuPDF's structured-text model
//! (`StextPage`/`StextLine`/`StextChar`). Device space there is in PDF
//! *points* (unscaled, 72/inch); this panel's pixel space is points ×
//! `RENDER_DPI / 72.0` — the same scale [`rasterize_page`] itself applies —
//! so a line's bbox is converted once by that factor before hit-testing
//! against the drag rect. **Line granularity, not glyph/character
//! granularity** — a deliberate scope cut (see [`select_text_in_rect`]).
//!
//! ## Every source-anchored artifact draws, not only annotations (op-30um.5)
//!
//! The canvas's "saved artifact region boxes" pass draws **every** artifact
//! [`crate::research_record::ResearchRecordIndex`] returns that has a valid
//! page plus a normalised `[source] region` — `Annotation`/`Note`,
//! `DigitisedGraph`, `DigitisedTable`, `Formula`, and `SourceReference`
//! alike, per GitHub issue #35's "layer 2" prototype
//! (`collaboration/kovan-issue-35-prototypes/layer2-artifact-overlays/`).
//! The artifact is the node; a digitised graph's or table's CSV is that
//! artifact's *payload* (shown via [`draw_csv_preview`] in the page-context
//! panel), never a second overlay of its own. An artifact anchored only by
//! a `pages` range has no `region` (§15 forbids combining the two) and so
//! is never boxable — it still appears in the page-context panel, just not
//! as a canvas rectangle.
//!
//! Colour is **semantic and resolved centrally**: [`super::theme::
//! artifact_accent`] maps each [`ArtifactKind`] to a Gruvbox accent that
//! holds in both themes (yellow / aqua / blue / purple / orange), so no
//! call site here constructs a literal `Color32` for an artifact box.
//!
//! The region→screen-rect reconstruction itself
//! ([`region_to_screen_rect`], selected per visible page by
//! [`artifact_overlays_for_page`]) is a **pure function** independent of
//! `egui::Ui`/`PageView` — the same placement arithmetic
//! [`super::page_canvas::PageView::project`] uses, so the two agree pixel
//! for pixel, but callable and unit-testable with no window or GPU texture
//! state.
//!
//! ## Right-click on a saved artifact: one kind-aware menu (op-30um.3/.6)
//!
//! Right-clicking an already-saved artifact's region box — of *any*
//! [`ArtifactKind`], now that [`super::theme::artifact_accent`] draws them
//! all — offers one fixed five-entry menu: an edit entry, then
//! **Add connection…** / **Edit connections…** / **Delete connection…**,
//! then **Delete annotation…**, which asks "Sure anot? [No] [Yes]" before
//! calling [`classify::delete_artifact_cascade`]. Only the edit entry's
//! *label* varies by kind ("Edit annotation", "Edit source reference",
//! "Edit formula", "Edit table", "Edit digitisation") — its handling does
//! not, since [`PdfReaderState::open_artifact`] already dispatches
//! text-bodied kinds to the block editor and `DigitisedTable`/
//! `DigitisedGraph` to a digitiser re-crop. This is deliberately **one**
//! menu, not five (op-30um.6's stated anti-goal).
//!
//! The entry list itself — which buttons appear, in what order, enabled or
//! not, and what each does when clicked — is [`saved_artifact_menu_entries`],
//! a plain function from `(ArtifactKind, bool)` to `Vec<MenuEntry>` with no
//! `egui` in its signature. [`PdfReaderState::context_menu_ui`] does nothing
//! but render that list and match on each [`MenuAction`]; the connection
//! actions themselves are a further thin shell straight over
//! [`relation::add_connection`]/[`relation::connections`]/
//! [`relation::edit_connection`]/[`relation::delete_connection`], fed by
//! [`library_candidates`] for "Add connection…"'s fuzzy picker. No
//! graph-walking, relation deletion, or partial write ever happens in the
//! egui layer itself — a cancelled dialog leaves everything untouched.

use std::collections::HashMap;

use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, Sense, Stroke};
use kopitiam_pdf::gui_frontend::{
    HotReload, PdfReader, PdfReaderConfig, ReloadDecision, RELOAD_CHECK_INTERVAL,
};
use kopitiam_pdf::mupdf::{
    page_to_stext, rasterize_page, PdfDocument, StextBlock, StextOptions, StextPage,
};

use crate::artifact::{block_span, Artifact, ArtifactKind, Region, SourceAnchor};
use crate::autocomplete::{library_candidates, LibraryCandidate};
use crate::classify;
use crate::digitiser::dataset::utc_now_iso8601;
use crate::digitiser::raster::PlotRaster;
use crate::entity::Classification;
use crate::graph::artifact_node;
use crate::index::KnowledgeIndex;
use crate::project;
use crate::relation::{self, RelationKind};
use crate::root::KovanRoot;
use crate::session::PaperSession;

use super::csv_preview::{draw_csv_preview, CopyButton};
use super::kvim_editor::{CompletionSource, KvimEditorState};
use super::page_canvas::PageView;

/// Screen-resolution DPI for the continuous canvas's page raster and
/// for the crop-to-digitiser render — sharp enough to read body text at a
/// typical window size. `Read` mode's own DPI is the embedded reader's own
/// business, not this constant's.
const RENDER_DPI: f32 = 150.0;

/// What is currently open in the reader — a multi-page PDF (owned by an
/// embedded [`PdfReader`], not by this panel directly), or a single directly-
/// loaded raster image. Closed set, enum-dispatched per the workspace's
/// no-trait-objects rule.
#[derive(Default)]
enum ReaderSource {
    #[default]
    None,
    Pdf(PdfReader),
    Image(PlotRaster),
}

impl ReaderSource {
    /// Total pages — a directly-loaded image is always exactly one "page".
    fn page_count(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Pdf(reader) => reader.page_count(),
            Self::Image(_) => 1,
        }
    }
}

/// Which annotation interaction is active. Closed set, enum-dispatched.
/// Only meaningful on the continuous canvas (or a plain
/// image, which has no other mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum AnnotationTool {
    /// The "Pan" tool: no drawing — grab-and-drag the page to scroll, arrow
    /// keys nudge the view, plus zoom and right-click on an existing box.
    #[default]
    None,
    /// Drag to propose a new box; right-click it for the Annotate/Digitise
    /// graph/Read table menu.
    DrawBox,
    /// Drag to select the real PDF text lines under the rectangle (op-z9u0)
    /// — a genuine text selection, not a region annotation.
    SelectText,
}

/// A saved free-text annotation (the "Annotate" menu action) — a
/// texture-pixel-space rect plus provenance (op-96am: "annotations... go
/// straight into markdown with date and time and author... metadata of
/// which page and exact pixels").
#[derive(Debug, Clone)]
struct Annotation {
    min: Pos2,
    max: Pos2,
    text: String,
    created_at: String,
    author: String,
    /// The annotate-canvas page's pixel size at `RENDER_DPI` when this box
    /// was drawn — so [`PdfReaderState::save_annotations_into_project`] can
    /// normalise `min`/`max` into a `[source] region` even for an
    /// annotation whose page is no longer the one the texture holds (the
    /// operator scrolled away, or is in Read mode, before saving).
    /// `[0.0, 0.0]` when the texture was somehow absent → page-only anchor.
    page_px: [f32; 2],
}

impl Annotation {
    fn contains(&self, p: Pos2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    /// This annotation's rect as a normalised `[source] region`, if the
    /// page size was captured.
    fn region(&self) -> Option<Region> {
        normalise_region(self.min, self.max, self.page_px[0], self.page_px[1])
    }
}

/// What a floating [`ContextMenu`] was opened on.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ContextMenuTarget {
    /// The just-drawn, not-yet-confirmed box in `pending_box`.
    NewBox,
    /// An already-saved, not-yet-persisted **in-memory** annotation box, by
    /// index into that page's `Vec` in `annotations` — see the module doc's
    /// "in-memory only" note. Offers only Edit/Delete, since it has no
    /// citekey/artifact id yet to hang a relation off of.
    Existing(usize),
    /// A **saved** artifact already in the paper's Markdown (any kind, per
    /// op-30um.5's overlay — Annotation/Note, DigitisedGraph, DigitisedTable,
    /// Formula, SourceReference), by its stable
    /// [`crate::artifact::ArtifactMeta::id`]. Offers the full op-30um.3 menu:
    /// Edit / Add connection / Edit connections / Delete connection /
    /// Delete annotation.
    SavedArtifact(String),
}

/// A floating right-click menu (op-x9qn), positioned at the click's screen
/// coordinates. `Clone`, not `Copy` — [`ContextMenuTarget::SavedArtifact`]
/// owns a `String`, so a read-out-of-`self` call site now clones instead of
/// copying (same "read it out without fighting the `&mut self` methods its
/// buttons call" reasoning [`ContextMenuTarget`] used to rely on `Copy` for).
#[derive(Debug, Clone)]
struct ContextMenu {
    screen_pos: Pos2,
    target: ContextMenuTarget,
}

/// Which connection-related sub-popup one of op-30um.3's new menu items
/// opens, and which node it operates on. A thin shell over
/// [`crate::relation`]'s CRUD: this only renders results and forwards a
/// click to the matching function — no relation lookup or graph walk
/// happens anywhere else in this file.
#[derive(Debug, Clone)]
enum ConnectionPopup {
    /// "Add connection..." — a fuzzy target picker (backed by
    /// [`library_candidates`]) plus a cyclable [`RelationKind`].
    Add {
        source: String,
        query: String,
        kind: RelationKind,
    },
    /// "Edit connections..." / "Delete connection..." — both open the same
    /// view of every [`relation::UserRelation`] touching `node`
    /// ([`relation::connections`]), each row offering a kind-cycle button
    /// and a Delete button; the two menu entries are two doors into one
    /// management view rather than two separate dialogs.
    Manage { node: String },
    /// "Delete annotation..." confirm — the maintainer's own wording,
    /// verbatim: "Sure anot? [No] [Yes]". `Yes` calls
    /// [`classify::delete_artifact_cascade`] exactly once; `No` calls
    /// nothing at all (the dialog is closed, `self` otherwise untouched).
    ConfirmDelete { citekey: String, artifact_id: String },
}

/// What one [`MenuEntry`] does when clicked, for the op-30um.3/.6 saved-
/// artifact right-click menu. [`PdfReaderState::context_menu_ui`] is the
/// only place that matches on this and calls into behaviour (opening the
/// block editor, a [`ConnectionPopup`]) — [`saved_artifact_menu_entries`]
/// itself never touches `egui`, a [`KovanRoot`], or a [`KnowledgeIndex`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MenuAction {
    /// Edit or re-open this artifact's payload.
    /// [`PdfReaderState::open_artifact`] already dispatches by kind (loads
    /// the block editor for text-bodied kinds, re-crops into the digitiser
    /// for `DigitisedTable`/`DigitisedGraph`) — this action is the single
    /// varying entry op-30um.6 asks for; the label is what changes per
    /// kind, not the handling.
    EditArtifact,
    /// Take the canvas to this artifact's page and zoom so its `[source]`
    /// region fills the view. Present only for a source-anchored artifact
    /// (maintainer, GH issue #35, 2026-09-08).
    GoToPage,
    /// Re-crop this artifact's region and hand it to the matching
    /// digitiser. Digitised graphs and tables only — the heavier action,
    /// which is why it moved out of double-click and onto the menu.
    GoToDigitiser,
    /// Opens [`ConnectionPopup::Add`].
    AddConnection,
    /// Opens [`ConnectionPopup::Manage`] (shared with `DeleteConnection` —
    /// one management view, two doors in, per [`ConnectionPopup`]'s doc).
    EditConnections,
    /// Opens [`ConnectionPopup::Manage`].
    DeleteConnection,
    /// Opens [`ConnectionPopup::ConfirmDelete`] — the "Sure anot?" confirm.
    /// [`crate::classify::delete_artifact_cascade`] only runs if that
    /// confirm is accepted; picking this entry never deletes by itself.
    DeleteArtifact,
}

/// One row of the op-30um.3/.6 saved-artifact right-click menu, decoupled
/// from `egui` so the *composition* of the menu — which entries appear for
/// which [`ArtifactKind`], in what order, enabled or not — is a plain data
/// value a test can assert on directly, with no window and no GPU context.
/// Same "testable without a window" reasoning as the workspace's headless-
/// simulator hard rule, applied to a menu instead of a physics loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MenuEntry {
    /// The button text, exactly as shown.
    pub label: &'static str,
    /// What picking this entry does — matched in [`PdfReaderState::context_menu_ui`].
    pub action: MenuAction,
    /// Whether the button is clickable. The connection entries are
    /// disabled (never hidden) when no `(KovanRoot, KnowledgeIndex)` pair
    /// is available, matching how the citation/wiki completion popup
    /// already behaves without a library.
    pub enabled: bool,
    /// Whether a `ui.separator()` is drawn immediately after this entry.
    pub separator_after: bool,
}

/// Builds the op-30um.3/.6 right-click menu for a saved artifact of `kind`,
/// given whether a library (`KovanRoot` + `KnowledgeIndex`) is available to
/// back the connection actions.
///
/// Every kind gets the identical five-entry shape and the identical
/// connection/delete verbs (op-30um.6's explicit requirement — "keep the
/// connection and delete verbs identical across kinds"); only the first
/// entry's label and the fact that it dispatches through
/// [`PdfReaderState::open_artifact`]'s existing per-kind branch varies.
/// This is deliberately **one** function for all five [`ArtifactKind`]
/// variants, not five menus — the anti-goal op-30um.6 states explicitly.
///
/// Reference behaviour (wording only, not ported code):
/// `collaboration/kovan-issue-35-prototypes/layer2-artifact-overlays/prototype_artifact_context_menu.py`.
pub(super) fn saved_artifact_menu_entries(
    kind: ArtifactKind,
    have_library: bool,
    has_page: bool,
) -> Vec<MenuEntry> {
    let edit_label = match kind {
        // The paper header is an artifact too, but it is not source-anchored
        // and so never has a canvas rectangle to right-click.
        ArtifactKind::Paper => "Edit paper metadata",
        // Neither is source-anchored, so neither is ever right-clicked on
        // the page; the arms exist so a new kind is a compile error here.
        ArtifactKind::Relation => "Edit connection",
        ArtifactKind::Mindmap => "Edit mindmap",
        ArtifactKind::Note | ArtifactKind::Annotation => "Edit annotation",
        ArtifactKind::DigitisedGraph => "Edit digitisation",
        ArtifactKind::DigitisedTable => "Edit table",
        ArtifactKind::Formula => "Edit formula",
        ArtifactKind::SourceReference => "Edit source reference",
    };
    let is_csv = matches!(
        kind,
        ArtifactKind::DigitisedTable | ArtifactKind::DigitisedGraph
    );
    let entries = vec![
        MenuEntry {
            label: edit_label,
            action: MenuAction::EditArtifact,
            enabled: true,
            separator_after: true,
        },
        MenuEntry {
            label: "Add connection…",
            action: MenuAction::AddConnection,
            enabled: have_library,
            separator_after: false,
        },
        MenuEntry {
            label: "Edit connections…",
            action: MenuAction::EditConnections,
            enabled: have_library,
            separator_after: false,
        },
        MenuEntry {
            label: "Delete connection…",
            action: MenuAction::DeleteConnection,
            enabled: have_library,
            separator_after: true,
        },
        MenuEntry {
            label: "Delete annotation…",
            action: MenuAction::DeleteArtifact,
            enabled: true,
            separator_after: false,
        },
    ];

    // Navigation entries go at the top, above Edit: they are the cheap,
    // non-mutating actions, and "go to page" is the one every anchored
    // artifact has.
    let mut nav = Vec::new();
    if has_page {
        nav.push(MenuEntry {
            label: "Go to page",
            action: MenuAction::GoToPage,
            enabled: true,
            separator_after: !is_csv,
        });
    }
    if is_csv {
        nav.push(MenuEntry {
            label: "Go to digitiser",
            action: MenuAction::GoToDigitiser,
            // Re-cropping needs the region the crop came from.
            enabled: has_page,
            separator_after: true,
        });
    }
    nav.extend(entries);
    nav
}

/// In-progress "Annotate" text editor, opened from the context menu's
/// Annotate/Edit actions. Shown as a panel under the toolbar (not floated
/// over the exact box position) — simpler and immune to scroll-coordinate
/// edge cases than a canvas-anchored popup, at the cost of not visually
/// hovering right over the box while typing.
struct AnnotateEditor {
    min: Pos2,
    max: Pos2,
    text: String,
    /// `Some(i)` when editing annotation `i` on the current page in place;
    /// `None` for a brand-new annotation.
    editing_existing: Option<usize>,
}

/// Provenance for a Digitise-graph/Read-table crop (op-p17q/op-hnhp), routed
/// through to whichever digitiser tab the crop is handed to so a later
/// "save into project markdown" action there (op-96am) can record where the
/// CSV came from — same shape as [`Annotation`]'s provenance fields, kept
/// as a separate type since a crop is not itself a saved [`Annotation`].
#[derive(Debug, Clone)]
pub struct CropProvenance {
    pub page_index: usize,
    pub min: Pos2,
    pub max: Pos2,
    /// The page's pixel size at `RENDER_DPI` when the crop was taken — so a
    /// digitiser save can normalise `min`/`max` into a [`Region`] for the
    /// artifact's `[source]` (GH issue #35 2026-09-02: "back to the
    /// digitiser" needs the region recorded). `[0.0, 0.0]` when unknown
    /// (no texture at crop time) — then [`Self::region`] is `None`.
    pub page_px: [f32; 2],
    pub created_at: String,
    pub author: String,
    /// The figure's own identifier/caption in the source document (e.g.
    /// `"Figure 4"`), as entered in [`PdfReaderState`]'s figure prompt
    /// (`op-8ci2`) — empty for a "Read table" crop, which has no equivalent
    /// prompt in this pass.
    pub figure: String,
    /// Set only when this crop *re-crops* an already-saved digitised
    /// artifact's region (the page-context panel's double-click-to-reopen):
    /// the digitiser's "save into notes" then **replaces** that block
    /// instead of appending a duplicate.
    pub source_artifact_id: Option<String>,
}

impl CropProvenance {
    /// The crop rectangle as a normalised, validated [`Region`] — `None` if
    /// the page size was not captured or the rectangle is degenerate.
    pub fn region(&self) -> Option<Region> {
        let [w, h] = self.page_px;
        normalise_region(self.min, self.max, w, h)
    }
}

/// A pixel rectangle on a page of size `w` × `h`, as a normalised, validated
/// [`Region`] (§15: fractions of the page, origin top-left). `None` for a
/// degenerate page size or a zero-area / out-of-range rectangle.
pub(super) fn normalise_region(min: Pos2, max: Pos2, w: f32, h: f32) -> Option<Region> {
    Region::from_pixels((min.x, min.y), (max.x, max.y), w, h)
}


/// The screen-space rectangle [`Region`] (§15, normalised page fractions)
/// reconstructs to on the continuous multi-page canvas, given which 0-based
/// `page` it is anchored to, that page's logical pixel size at the render
/// DPI, and the canvas's own placement parameters — the canvas-space
/// `origin`, display `zoom`, and inter-page `gap`, the same three
/// [`super::page_canvas::PageView::project`] already threads through. The
/// arithmetic mirrors `project` exactly (`page_top = page * (page_px.y *
/// zoom + gap)`, then `origin + point * zoom`), so a call here and a call
/// through the live `PageView` land on the same pixel — but this function
/// needs no `PageView` (and so no `egui` texture/GPU state), which is what
/// makes it unit-testable headlessly (op-30um.5).
///
/// Returns `None` — "do not draw" — when `region` fails [`Region::is_valid`]
/// or `page_px`/`zoom` is degenerate. An artifact with a bad or unmeasured
/// region must not draw nonsense rather than fail loudly here; the parse
/// side (`crate::artifact::SourceAnchor::validate`) is where a bad region is
/// actually reported.
pub(super) fn region_to_screen_rect(
    region: Region,
    page: usize,
    page_px: egui::Vec2,
    origin: Pos2,
    zoom: f32,
    gap: f32,
) -> Option<Rect> {
    if !region.is_valid() || page_px.x <= 0.0 || page_px.y <= 0.0 || zoom <= 0.0 {
        return None;
    }
    let page_top = page as f32 * (page_px.y * zoom + gap);
    let to_screen = |x_frac: f64, y_frac: f64| -> Pos2 {
        origin
            + egui::vec2(
                x_frac as f32 * page_px.x * zoom,
                page_top + y_frac as f32 * page_px.y * zoom,
            )
    };
    Some(Rect::from_min_max(
        to_screen(region.x0, region.y0),
        to_screen(region.x1, region.y1),
    ))
}

/// Every source-anchored artifact that should draw a rectangle on 0-based
/// `page` of the continuous canvas, each paired with its reconstructed
/// screen rect ([`region_to_screen_rect`]) — the pure core of
/// [`PdfReaderState::ui`]'s "saved artifact region boxes" render pass
/// (op-30um.5).
///
/// The production rule this exists for: **every** artifact with a valid
/// page plus a normalised source region draws, not only `Annotation`s — see
/// the module doc. Skips (never returned for this `page`): an artifact
/// anchored to a different page, one with no `[source]` at all, one with a
/// `pages`-range-only anchor (no `region` — §15 forbids combining the two,
/// so a range artifact is correctly excluded here regardless of how many
/// pages it spans), and one whose region is degenerate or out of range.
pub(super) fn artifact_overlays_for_page<'a>(
    artifacts: &'a [Artifact],
    page: usize,
    page_px: egui::Vec2,
    origin: Pos2,
    zoom: f32,
    gap: f32,
) -> Vec<(&'a Artifact, Rect)> {
    artifacts
        .iter()
        .filter_map(|artifact| {
            if PdfReaderState::artifact_page(artifact) != Some(page) {
                return None;
            }
            let region = artifact.toml.source.as_ref()?.region?;
            let rect = region_to_screen_rect(region, page, page_px, origin, zoom, gap)?;
            Some((artifact, rect))
        })
        .collect()
}

/// The "other" endpoint of `rel` as seen from `node`, plus which arrow to
/// draw — pulled out of [`PdfReaderState::connection_popup_ui`]'s "Manage"
/// list rendering so the source/target direction logic is unit-testable
/// without an `egui::Ui` (op-30um.3).
pub(super) fn relation_other_end<'a>(rel: &'a relation::UserRelation, node: &str) -> (&'static str, &'a str) {
    if rel.source == node {
        ("→", &rel.target)
    } else {
        ("←", &rel.source)
    }
}

/// An in-progress "Digitise graph" crop (`op-8ci2`) waiting on the figure
/// identifier — asked for immediately after the right-click gesture,
/// while the user still has the figure in view, rather than leaving the
/// digitiser's own required `figure*` field blank for them to notice and
/// fill in later. Rendered as a panel the same way [`AnnotateEditor`] is,
/// not a canvas-anchored popup — see that type's doc for why.
struct PendingFigurePrompt {
    min: Pos2,
    max: Pos2,
    figure: String,
}

/// A completed crop-then-right-click gesture (op-p17q / op-hnhp), returned
/// from [`PdfReaderState::ui`] the frame it happens.
pub enum CropResult {
    Plot(PlotRaster, CropProvenance),
    Table(PlotRaster, CropProvenance),
}

/// State for one open document: its source (embedded [`PdfReader`] or a
/// plain image), its continuous-canvas page/
/// zoom/cached texture, and its annotations.
/// One in-document search hit — a tight box in `RENDER_DPI` texture-pixel
/// space on `page`.
#[derive(Debug, Clone, Copy)]
struct SearchHit {
    page: usize,
    min: Pos2,
    max: Pos2,
}

/// In-document `/`-search state (GH issue #35 2026-09-02) — the one feature
/// the dropped embedded reader is missed for, reimplemented over
/// [`kopitiam_pdf::mupdf::page_to_stext`].
#[derive(Default)]
struct SearchState {
    /// The live query text (bound to the toolbar field).
    query: String,
    /// Hits for [`Self::computed_for`], in document order.
    hits: Vec<SearchHit>,
    /// The query `hits` was computed for — so a rescan only runs when the
    /// text actually changes.
    computed_for: String,
    /// Index into `hits` of the "current" hit (`Next`/`Prev` cycle it).
    current: Option<usize>,
}

#[derive(Default)]
pub struct PdfReaderState {
    path: String,
    source: ReaderSource,
    /// The page currently in view on the continuous canvas — the top page
    /// the canvas is scrolled to (or the page under the pointer), updated
    /// each frame. Feeds [`Self::active_page`] and the context panel.
    /// Always `0` for a plain image.
    annotate_page: usize,
    /// The continuous multi-page raster view (GH issue #35 2026-09-02) —
    /// kovan renders the pages itself so it can draw region boxes over them
    /// and route clicks (the embedded reader cannot — kopitiam#107).
    pages: PageView,
    /// Whether the left page-thumbnail strip is shown (op-0y4k's Okular-style
    /// page picker). Re-implemented on kovan's own rasters after the embedded
    /// reader — which used to supply it — was dropped. On by default; see
    /// [`PdfReaderState::new`].
    show_thumbs: bool,
    /// The page the thumbnail strip was last auto-scrolled to follow, so it
    /// keeps the current page in view without fighting a manual strip scroll.
    thumb_synced: Option<usize>,
    /// A page the continuous canvas should scroll to on the next frame —
    /// set by the Prev/Next buttons, `j`/`k`, and a search-hit jump.
    /// Consumed by the canvas.
    scroll_request: Option<usize>,
    /// In-document `/`-search over the same structured text the select-text
    /// tool uses ([`SearchState`]) — reimplemented here because the embedded
    /// reader that used to provide it is gone (GH issue #35 2026-09-02).
    search: SearchState,
    /// The canvas zoom on the page rasters.
    zoom: f32,
    /// `zoom` as of the last canvas frame. `0.0` before the first frame.
    last_zoom: f32,
    /// Where the viewport was centred **last frame**, in zoom-independent
    /// document units: `y` = fractional page position (page index + fraction
    /// through it), `x` = fraction of the page width. Re-derived from the
    /// `ScrollArea`'s real offset every frame, and used to restore the exact
    /// same view point when `zoom` changes — the `ScrollArea` offset is
    /// absolute points, so without this a zoom silently lands you on a
    /// different page (maintainer's bug 2026-09-02).
    scroll_anchor: egui::Vec2,
    /// The visible viewport size last frame — needed to convert the centred
    /// anchor back into a top-left scroll offset.
    last_viewport: egui::Vec2,
    /// The `ScrollArea`'s actual offset last frame, so a vertical-only page
    /// jump can leave the horizontal scroll where the operator put it.
    last_offset: egui::Vec2,
    /// A scroll offset to force on the **next** frame — set by a
    /// pointer-anchored zoom (Ctrl+scroll, `+`/`-`) so the document point
    /// under the mouse stays under the mouse. `ScrollArea` applies it before
    /// layout and input, so it is exact and one-shot.
    forced_offset: Option<egui::Vec2>,
    message: String,
    // annotations — in-memory only, see the module doc comment.
    tool: AnnotationTool,
    annotations: HashMap<usize, Vec<Annotation>>,
    /// Texture-pixel-space start corner of a box drag in progress.
    draw_start: Option<Pos2>,
    /// The last completed, not-yet-confirmed box (texture-pixel space,
    /// min/max corners) — persists after the drag ends until the user
    /// right-clicks it (opens the context menu) or starts a new drag
    /// (replaces it).
    pending_box: Option<(Pos2, Pos2)>,
    context_menu: Option<ContextMenu>,
    /// The connection sub-popup opened from a saved artifact's right-click
    /// menu (op-30um.3), if any — see [`ConnectionPopup`].
    connection_popup: Option<ConnectionPopup>,
    /// Status text for the last connection CRUD action (op-30um.3), shown
    /// in [`Self::connection_popup_ui`] — e.g. an error from
    /// `add_connection`/`edit_connection`/`delete_connection` failing.
    connection_message: String,
    annotate_editor: Option<AnnotateEditor>,
    /// See [`PendingFigurePrompt`] (`op-8ci2`).
    pending_figure_prompt: Option<PendingFigurePrompt>,
    /// Author name recorded on new annotations/crops (op-96am's provenance
    /// "author" field) — analogous to the digitiser's own "operator" field.
    author: String,
    /// "kovan folder" project (op-63u0) to save annotations into, and the
    /// markdown file (relative to that root) they belong to — see
    /// [`Self::save_annotations_into_project`].
    project_root: String,
    project_markdown_rel: String,
    /// Cached structured-text page (op-z9u0), for [`Self::active_page`]
    /// only — re-extracted on page change.
    stext_cache: Option<(usize, StextPage)>,
    /// Texture-pixel-space start corner of a text-selection drag in
    /// progress (op-z9u0).
    select_start: Option<Pos2>,
    /// The last completed text selection: its bounding rect (union of every
    /// selected line's bbox, texture-pixel space) and the concatenated text
    /// of the lines it covers, one per line. `None` before any selection.
    text_selection: Option<(Pos2, Pos2, String)>,
    /// The `created_at` of the annotation the pointer is currently hovering
    /// on the canvas, if any (op-4x5s) — a poor-man's stable id
    /// [`Self::context_panel`] uses to highlight the matching markdown
    /// block. One frame behind the canvas hover (see where it's set).
    hover_created_at: Option<String>,
    /// Polls the open file's mtime and reloads the embedded [`PdfReader`]
    /// when it changes (op-eehc — "hot reload by default in case I compile
    /// live in tex or typst"). Default-on to match kovan's own prior
    /// default, set explicitly in [`Self::new`] (`HotReload`'s own
    /// `Default` is off).
    hot_reload: HotReload,
    /// The last "Generate BibTeX" result (op-x3wl) — `Ok` holds the
    /// rendered entry, `Err` a human-readable failure. `None` before the
    /// button has ever been pressed for the currently open PDF.
    bibtex: Option<Result<String, String>>,
    /// The page the embedded page-context editor was last auto-scrolled to
    /// this paper's blocks for (op-j178 / GH issue #35 2026-09-02) — so the
    /// one-shot `jump_to_line` fires on an actual page change, not every
    /// frame. `None` until the first sync.
    context_page_synced: Option<usize>,
    /// A single reusable kvim editor for **inline per-block editing** in the
    /// page-context panel (op-j178: "text/annotation blocks editable
    /// inline"). Only ever holds one block at a time — you edit one block,
    /// Save or Cancel, then the next. `editing_block_id` says which artifact
    /// (by stable id) it currently holds, or `None` when no block is open.
    block_editor: KvimEditorState,
    editing_block_id: Option<String>,
    /// The stable id of the anchored-artifact card the pointer is hovering
    /// in the page-context panel, if any — the canvas overlay reads it to
    /// highlight that artifact's `region` box (op-4x5s, panel → canvas
    /// direction). One frame behind, same as `hover_created_at` the other
    /// way.
    panel_hover_id: Option<String>,
}

/// Build an egui [`ColorImage`] from a [`PlotRaster`] by reading every pixel
/// through its `rgb()` accessor — the same approach the digitiser's own
/// image panel uses (`gui/desktop/mod.rs`'s `image_panel`) to go from a
/// `PlotRaster` to a displayable texture, reused here rather than
/// reimplemented so a bug in one path shows up in both.
pub(super) fn raster_to_color_image(raster: &PlotRaster) -> ColorImage {
    let (w, h) = (raster.width() as usize, raster.height() as usize);
    let mut rgb = Vec::with_capacity(w * h * 3);
    for y in 0..raster.height() {
        for x in 0..raster.width() {
            rgb.extend_from_slice(&raster.rgb(x, y));
        }
    }
    ColorImage::from_rgb([w, h], &rgb)
}

/// Whether `path` names a PDF by extension (case-insensitive) — everything
/// else is tried as a raster image. A content-sniffing check (magic bytes)
/// would be more robust, but the file dialog's own filters (op-689u/op-nje6)
/// already constrain what gets picked in practice, so the extension is
/// enough here.
fn looks_like_pdf(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// The concatenated text (op-z9u0) of every [`kopitiam_pdf::mupdf::StextLine`]
/// on `page` whose device-space bbox (converted to texture-pixel space by
/// `scale`) intersects `(min, max)`, one output line per selected PDF line
/// — the "line granularity, not glyph granularity" scope cut from the
/// module doc: a line is either wholly selected or not selected at all,
/// never a partial-line (character-range) selection.
fn select_text_in_rect(page: &StextPage, scale: f32, min: Pos2, max: Pos2) -> String {
    let mut out = String::new();
    for block in &page.blocks {
        let StextBlock::Text(tb) = block else {
            continue;
        };
        for line in &tb.lines {
            let b = line.bbox;
            let (lx0, ly0, lx1, ly1) = (b.x0 * scale, b.y0 * scale, b.x1 * scale, b.y1 * scale);
            let intersects = lx0 <= max.x && lx1 >= min.x && ly0 <= max.y && ly1 >= min.y;
            if intersects {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&line.text());
            }
        }
    }
    out
}

/// Every `(start_char, end_char)` half-open char range in `chars` that
/// case-insensitively matches `needle` (ASCII fold). Overlapping matches
/// are not returned — the scan advances past each hit.
fn substr_char_ranges(chars: &[char], needle: &str) -> Vec<(usize, usize)> {
    let needle: Vec<char> = needle.chars().flat_map(|c| c.to_lowercase()).collect();
    if needle.is_empty() || needle.len() > chars.len() {
        return Vec::new();
    }
    let lower: Vec<char> = chars.iter().map(|c| c.to_ascii_lowercase()).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + needle.len() <= lower.len() {
        if lower[i..i + needle.len()] == needle[..] {
            out.push((i, i + needle.len()));
            i += needle.len();
        } else {
            i += 1;
        }
    }
    out
}

/// Case-insensitive occurrences of `needle` on one structured-text line,
/// each as a tight bounding box in **texture-pixel space** (device-space
/// char quads × `scale`). Word/character granularity, unlike
/// [`select_text_in_rect`]'s line granularity — a search hit should
/// highlight the matched word, not its whole line.
fn line_hits(line: &kopitiam_pdf::mupdf::StextLine, needle: &str, scale: f32) -> Vec<(Pos2, Pos2)> {
    let chars: Vec<char> = line.chars.iter().map(|ch| ch.c).collect();
    substr_char_ranges(&chars, needle)
        .into_iter()
        .map(|(s, e)| {
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for ch in &line.chars[s..e] {
                for p in [ch.quad.ul, ch.quad.ur, ch.quad.ll, ch.quad.lr] {
                    x0 = x0.min(p.x);
                    y0 = y0.min(p.y);
                    x1 = x1.max(p.x);
                    y1 = y1.max(p.y);
                }
            }
            (
                Pos2::new(x0 * scale, y0 * scale),
                Pos2::new(x1 * scale, y1 * scale),
            )
        })
        .collect()
}

/// Split `text` on lines starting with `### ` (one block per subsection,
/// running to the next `### ` or EOF) and keep only the blocks containing
/// at least one of `needles` — [`PdfReaderState::context_panel`]'s plain
/// substring filter over a project's markdown, not a markdown parser.
fn blocks_matching(text: &str, needles: &[&str]) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if line.starts_with("### ") {
            if let Some(block) = current.take() {
                blocks.push(block);
            }
            current = Some(String::new());
        }
        if let Some(block) = &mut current {
            block.push_str(line);
            block.push('\n');
        }
    }
    if let Some(block) = current {
        blocks.push(block);
    }
    blocks.retain(|b| needles.iter().any(|n| b.contains(n)));
    blocks
}

/// A short read-only preview of an artifact body for the page-context
/// panel — the first few non-empty lines, capped, with an ellipsis when
/// there is more.
fn body_preview(body: &str) -> String {
    const MAX_LINES: usize = 4;
    const MAX_CHARS: usize = 280;
    let body = body.trim();
    let head = body.lines().take(MAX_LINES).collect::<Vec<_>>().join("\n");
    let truncated: String = head.chars().take(MAX_CHARS).collect();
    if truncated.len() < body.len() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

/// 1-based line of the first `#`-prefixed heading in `md` whose block (up
/// to the next heading) mentions `page` — the `page: N` / `page N,` markers
/// [`PdfReaderState::save_annotations_into_project`]'s legacy path emitted,
/// kept so pre-artifact `### annotation` notes still auto-scroll. `None`
/// when nothing on the page is written as plain text.
fn first_note_heading_line(md: &str, page: usize) -> Option<usize> {
    let marker_a = format!("page: {}", page + 1);
    let marker_b = format!("page {},", page + 1);
    let mut heading_line: Option<usize> = None;
    for (i, line) in md.lines().enumerate() {
        if line.starts_with('#') {
            heading_line = Some(i + 1);
        } else if (line.contains(&marker_a) || line.contains(&marker_b)) && heading_line.is_some() {
            return heading_line;
        }
    }
    None
}

impl PdfReaderState {
    /// A fresh reader — `Read` mode and hot-reload both **on** by default
    /// for a PDF (GitHub issue #30's explicit "hot reload by default in
    /// case I compile live in tex or typst"; (historical note)
    /// derived default already). Prefer this over `PdfReaderState::default()`
    /// so hot-reload starts enabled, since [`HotReload`]'s own `Default`
    /// (unlike this panel's prior hand-rolled `bool`) starts disabled.
    pub fn new() -> Self {
        Self {
            hot_reload: HotReload::new(true),
            show_thumbs: true,
            ..Self::default()
        }
    }

    /// Open `path` as the working document — a PDF or a raster image
    /// (op-wojr), dispatched by [`looks_like_pdf`] — replacing whatever was
    /// previously open.
    pub fn open(&mut self, path: &str) {
        if looks_like_pdf(path) {
            self.open_pdf(path);
        } else {
            self.open_image(path);
        }
    }

    fn reset_interaction_state(&mut self) {
        self.annotate_page = 0;
        self.pages.clear();
        self.search = SearchState::default();
        self.zoom = if self.zoom > 0.0 { self.zoom } else { 1.0 };
        self.last_zoom = 0.0;
        self.scroll_anchor = egui::Vec2::ZERO;
        self.last_viewport = egui::Vec2::ZERO;
        self.forced_offset = None;
        self.last_offset = egui::Vec2::ZERO;
        self.thumb_synced = None;
        self.annotations.clear();
        self.draw_start = None;
        self.pending_box = None;
        self.context_menu = None;
        self.annotate_editor = None;
        self.bibtex = None;
        self.stext_cache = None;
        self.select_start = None;
        self.text_selection = None;
        self.hover_created_at = None;
        self.panel_hover_id = None;
        self.editing_block_id = None;
        self.context_page_synced = None;
    }

    fn open_pdf(&mut self, path: &str) {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.message = format!("cannot read {path}: {e}");
                return;
            }
        };
        match PdfReader::open_bytes_with(bytes, PdfReaderConfig::read_only()) {
            Ok(mut reader) => {
                reader.set_label(path.to_string());
                let page_count = reader.page_count();
                self.path = path.to_string();
                self.hot_reload = HotReload::new(true);
                self.hot_reload.mark_current(std::path::Path::new(path));
                self.source = ReaderSource::Pdf(reader);
                self.reset_interaction_state();
                self.message = format!("opened {path} ({page_count} page(s))");
            }
            Err(e) => self.message = format!("cannot open {path}: {e}"),
        }
    }

    fn open_image(&mut self, path: &str) {
        match PlotRaster::from_path(std::path::Path::new(path)) {
            Ok(raster) => {
                self.path = path.to_string();
                self.source = ReaderSource::Image(raster);
                self.reset_interaction_state();
                self.message = format!("opened {path}");
            }
            Err(e) => self.message = format!("cannot open {path} as PDF or image: {e}"),
        }
    }

    /// Reload the open PDF if it changed on disk (op-eehc), via
    /// [`HotReload::poll`] + [`PdfReader::load_bytes`] — the same mechanism
    /// kovan hand-rolled before this migration, now shared with `kpdf`
    /// itself (see [`HotReload`]'s own doc, which credits this panel as its
    /// origin). No-op when hot-reload is off, nothing is open, the source
    /// isn't a PDF, or fewer than [`RELOAD_CHECK_INTERVAL`] has passed since
    /// the last check.
    fn check_hot_reload(&mut self, ctx: &egui::Context) {
        let ReaderSource::Pdf(reader) = &mut self.source else {
            return;
        };
        if !self.hot_reload.is_enabled() || self.path.is_empty() {
            return;
        }
        ctx.request_repaint_after(RELOAD_CHECK_INTERVAL);
        let path = std::path::Path::new(&self.path);
        if self.hot_reload.poll(path, std::time::Instant::now()) != ReloadDecision::Changed {
            return;
        }
        match std::fs::read(path) {
            Ok(bytes) => match reader.load_bytes(bytes) {
                Ok(()) => {
                    self.hot_reload.mark_current(path);
                    self.annotate_page = self
                        .annotate_page
                        .min(reader.page_count().saturating_sub(1));
                    // The document changed under us — every cached page
                    // raster, structured-text page and search hit is stale.
                    self.pages.clear();
                    self.stext_cache = None;
                    self.search.computed_for.clear();
                    self.message = format!("{} changed on disk — reloaded", self.path);
                }
                Err(e) => {
                    self.message = format!("{} changed on disk, but reload failed: {e}", self.path)
                }
            },
            Err(e) => {
                self.message = format!(
                    "{} changed on disk, but could not be re-read: {e}",
                    self.path
                )
            }
        }
    }

    /// Generate a BibTeX entry for the currently open PDF (op-x3wl: "I want
    /// the pdf I'm reading to generate a bibtex entry I can copy and
    /// paste"). Reuses `kovan_literature::extract_metadata` +
    /// `to_bibtex` — the exact same pipeline `kovan-cli lit bibtex` already
    /// runs — rather than a second implementation.
    fn generate_bibtex(&mut self) {
        let ReaderSource::Pdf(_) = &self.source else {
            self.bibtex = Some(Err(
                "no PDF open (BibTeX needs a PDF, not a plain image)".into()
            ));
            return;
        };
        self.bibtex = Some(
            kovan_literature::extract_metadata(std::path::Path::new(&self.path))
                .map(|doc| kovan_literature::to_bibtex(&doc))
                .map_err(|e| e.to_string()),
        );
    }

    /// The page the canvas is scrolled to / the pointer is over — what the
    /// right-hand context panel follows. Always `0` for a plain image.
    fn active_page(&self) -> usize {
        match &self.source {
            ReaderSource::Pdf(_) => self.annotate_page,
            ReaderSource::Image(_) | ReaderSource::None => 0,
        }
    }

    /// The open PDF document, for a crop/stext read against the *same*
    /// loaded bytes the embedded [`PdfReader`] already holds — no second
    /// file load. `None` when nothing is open or the source is a plain
    /// image.
    fn current_pdf_document(&self) -> Option<&PdfDocument> {
        match &self.source {
            ReaderSource::Pdf(reader) => Some(reader.document()),
            ReaderSource::Image(_) | ReaderSource::None => None,
        }
    }

    /// Crop the current page/image to `(min, max)` (texture-pixel space) and
    /// build a standalone [`PlotRaster`] from it — the hand-off to the plot
    /// digitiser (op-p17q) or table digitiser (op-hnhp). Re-rasterizes the
    /// current page rather than caching the last `Pixmap` alongside the
    /// texture: simpler, and rasterization is already cheap enough per-page
    /// that a second render for this one-time crop action isn't worth the
    /// extra cached-state bookkeeping.
    fn crop_current_page(&self, min: Pos2, max: Pos2) -> Result<PlotRaster, String> {
        let min_x = min.x.max(0.0) as u32;
        let min_y = min.y.max(0.0) as u32;
        let want_w = (max.x - min.x).max(1.0) as u32;
        let want_h = (max.y - min.y).max(1.0) as u32;
        match &self.source {
            ReaderSource::None => Err("nothing open".to_string()),
            ReaderSource::Pdf(reader) => {
                let pixmap = rasterize_page(reader.document(), self.annotate_page, RENDER_DPI)
                    .map_err(|e| format!("page render failed: {e}"))?;
                let (pw, ph, stride, n) = (pixmap.w, pixmap.h, pixmap.stride, pixmap.n as usize);
                let samples = pixmap.samples;
                let w = want_w.min(pw.saturating_sub(min_x)).max(1);
                let h = want_h.min(ph.saturating_sub(min_y)).max(1);
                Ok(PlotRaster::from_rgb_fn(w, h, move |x, y| {
                    let px = (min_x + x).min(pw.saturating_sub(1));
                    let py = (min_y + y).min(ph.saturating_sub(1));
                    let offset = py as usize * stride + px as usize * n;
                    [samples[offset], samples[offset + 1], samples[offset + 2]]
                }))
            }
            ReaderSource::Image(raster) => {
                let (rw, rh) = (raster.width(), raster.height());
                let w = want_w.min(rw.saturating_sub(min_x)).max(1);
                let h = want_h.min(rh.saturating_sub(min_y)).max(1);
                Ok(PlotRaster::from_rgb_fn(w, h, move |x, y| {
                    let px = (min_x + x).min(rw.saturating_sub(1));
                    let py = (min_y + y).min(rh.saturating_sub(1));
                    raster.rgb(px, py)
                }))
            }
        }
    }

    /// Crop the **normalised `region`** of PDF page `page` (0-based) to a
    /// standalone [`PlotRaster`], re-rasterising that page at `RENDER_DPI`.
    /// Returns the raster plus the pixel-space `(min, max)` corners the
    /// region resolved to, for the re-crop's [`CropProvenance`] (GH issue
    /// #35 2026-09-02: "back to the digitiser" for a saved digitised block).
    fn crop_region_of_page(
        &self,
        page: usize,
        region: Region,
    ) -> Result<(PlotRaster, Pos2, Pos2), String> {
        let ReaderSource::Pdf(reader) = &self.source else {
            return Err("no PDF open".to_string());
        };
        let pixmap = rasterize_page(reader.document(), page, RENDER_DPI)
            .map_err(|e| format!("page render failed: {e}"))?;
        let (pw, ph, stride, n) = (pixmap.w, pixmap.h, pixmap.stride, pixmap.n as usize);
        let samples = pixmap.samples;
        let min = Pos2::new(region.x0 as f32 * pw as f32, region.y0 as f32 * ph as f32);
        let max = Pos2::new(region.x1 as f32 * pw as f32, region.y1 as f32 * ph as f32);
        let min_x = min.x.max(0.0) as u32;
        let min_y = min.y.max(0.0) as u32;
        let w = ((max.x - min.x).max(1.0) as u32)
            .min(pw.saturating_sub(min_x))
            .max(1);
        let h = ((max.y - min.y).max(1.0) as u32)
            .min(ph.saturating_sub(min_y))
            .max(1);
        let raster = PlotRaster::from_rgb_fn(w, h, move |x, y| {
            let px = (min_x + x).min(pw.saturating_sub(1));
            let py = (min_y + y).min(ph.saturating_sub(1));
            let offset = py as usize * stride + px as usize * n;
            [samples[offset], samples[offset + 1], samples[offset + 2]]
        });
        Ok((raster, min, max))
    }

    /// Re-open a saved digitised table/graph artifact in the matching
    /// digitiser (GH issue #35 2026-09-02) — re-crop its `[source]` region
    /// from the open PDF and return it as a [`CropResult`] the app already
    /// knows how to route (`CropResult::Table` → table digitiser,
    /// `::Plot` → graph digitiser). `None` when the artifact has no usable
    /// region, or the crop fails. Marks the provenance with the artifact's
    /// id so re-saving replaces the block instead of appending a duplicate.
    fn recrop_artifact(&mut self, artifact: &Artifact) -> Option<CropResult> {
        let anchor = artifact.toml.source.as_ref()?;
        let region = anchor.region?;
        let page = anchor.page?.saturating_sub(1) as usize;
        match self.crop_region_of_page(page, region) {
            Ok((raster, min, max)) => {
                self.annotate_page = page;
                // The re-digitise save goes through `replace_artifact_body`,
                // which keeps the original `[source]` (region included), so
                // `page_px`/`region()` are irrelevant here — only the id
                // matters, to target the right block.
                let prov = CropProvenance {
                    page_index: page,
                    min,
                    max,
                    page_px: [0.0, 0.0],
                    created_at: utc_now_iso8601(),
                    author: self.author_name(),
                    figure: artifact.heading.clone(),
                    source_artifact_id: Some(artifact.id().to_string()),
                };
                Some(match artifact.kind() {
                    ArtifactKind::DigitisedTable => CropResult::Table(raster, prov),
                    _ => CropResult::Plot(raster, prov),
                })
            }
            Err(e) => {
                self.message = format!("re-crop failed: {e}");
                None
            }
        }
    }

    /// Open the floating context menu on `target`, or close it if it is
    /// already open on that same target.
    ///
    /// A second right-click on the same thing dismisses the menu rather
    /// than re-opening it in place (maintainer, GH issue #35, 2026-09-08:
    /// "box should disappear after a second right click"). Right-clicking a
    /// *different* target moves the menu there instead of closing it, which
    /// is what makes the gesture usable for comparing two artifacts.
    fn toggle_context_menu(&mut self, screen_pos: Pos2, target: ContextMenuTarget) {
        let same = self
            .context_menu
            .as_ref()
            .is_some_and(|m| m.target == target);
        self.context_menu = if same {
            None
        } else {
            Some(ContextMenu { screen_pos, target })
        };
    }

    /// Take the canvas to `artifact`'s page and zoom so its `[source]`
    /// region roughly fills the view.
    ///
    /// The zoom is derived from the region's own extent — a region covering
    /// a third of the page height is worth ~3x — clamped to the same
    /// `0.25..=4.0` range the zoom slider uses. An artifact with a page but
    /// no region just navigates, leaving the zoom alone: there is nothing
    /// to frame.
    fn go_to_artifact(&mut self, artifact: &Artifact) {
        let Some(page) = Self::artifact_page(artifact) else {
            return;
        };
        self.annotate_page = page;
        self.scroll_request = Some(page);
        self.thumb_synced = None;
        if let Some(region) = artifact.toml.source.as_ref().and_then(|s| s.region) {
            let w = (region.x1 - region.x0).max(1e-3) as f32;
            let h = (region.y1 - region.y0).max(1e-3) as f32;
            // Fit the larger dimension, so neither axis overflows.
            self.zoom = (1.0 / w.max(h)).clamp(0.25, 4.0);
        }
    }

    /// Open a saved artifact for editing — the single path a **double-click
    /// on its box** on the continuous canvas and a **double-click on its
    /// card / preview line** in the context panel both go through, so the
    /// two cannot drift (GH issue #35 2026-09-02).
    ///
    /// A text / annotation / formula / source-reference block loads into
    /// `block_editor` (the panel then shows the inline editor); a digitised
    /// table / graph block re-crops its `[source]` region and comes back as
    /// a [`CropResult`] for the app to route to the matching digitiser.
    fn open_artifact(&mut self, artifact: &Artifact) -> Option<CropResult> {
        // Take the canvas to the page the block is anchored to — opening a
        // block from the panel while looking at a different page should
        // show you what it is about (maintainer, 2026-09-02).
        self.go_to_artifact(artifact);
        match artifact.kind() {
            // A digitised graph/table now *zooms to its page* rather than
            // re-opening the digitiser: going back to the digitiser is the
            // heavier, rarer action and lives on the right-click menu as
            // "Go to digitiser" (maintainer, GH issue #35, 2026-09-08).
            ArtifactKind::DigitisedTable | ArtifactKind::DigitisedGraph => None,
            _ => {
                self.block_editor.load_text(&artifact.body);
                self.editing_block_id = Some(artifact.id().to_string());
                None
            }
        }
    }

    /// The 0-based page an artifact's `[source]` anchor starts on, if any.
    fn artifact_page(artifact: &Artifact) -> Option<usize> {
        artifact
            .toml
            .source
            .as_ref()
            .and_then(|s| s.first_page())
            .map(|p| p.saturating_sub(1) as usize)
    }

    /// The annotate-canvas page pixel size (`RENDER_DPI`) — stamped onto a
    /// new [`Annotation`] so its region survives a page change before the
    /// save. A box can only be drawn on a page that has rendered, so by the
    /// time this is read the size is the real measured one.
    fn current_page_px(&self) -> [f32; 2] {
        let sz = self.pages.page_size_px();
        [sz.x, sz.y]
    }

    fn author_name(&self) -> String {
        let t = self.author.trim();
        if t.is_empty() {
            "unnamed".to_string()
        } else {
            t.to_string()
        }
    }

    fn make_provenance(&self, min: Pos2, max: Pos2, figure: impl Into<String>) -> CropProvenance {
        let page_px = self.current_page_px();
        CropProvenance {
            page_index: self.active_page(),
            min,
            max,
            page_px,
            created_at: utc_now_iso8601(),
            author: self.author_name(),
            figure: figure.into(),
            source_artifact_id: None,
        }
    }

    /// Draw the "what figure is this?" prompt, if [`Self::pending_figure_prompt`]
    /// is `Some` — a panel under the toolbar, same placement/rendering
    /// pattern as [`Self::annotate_editor_panel`] and for the same reason
    /// (immune to scroll-coordinate edge cases a canvas-anchored popup
    /// would have to handle). Completes the crop and returns
    /// `Some(CropResult::Plot(..))` once the user confirms with a
    /// non-empty figure identifier; `Cancel` discards the pending crop
    /// entirely (`op-8ci2`).
    fn figure_prompt_panel(&mut self, ui: &mut egui::Ui) -> Option<CropResult> {
        let prompt = self.pending_figure_prompt.as_mut()?;
        let mut confirm = false;
        let mut cancel = false;
        ui.group(|ui| {
            ui.label("Digitise graph — what figure is this?");
            ui.horizontal(|ui| {
                ui.label("figure*");
                ui.add(
                    egui::TextEdit::singleline(&mut prompt.figure)
                        .hint_text("e.g. \"Figure 4\" or \"Fig. 4.2\""),
                );
            });
            ui.horizontal(|ui| {
                let can_confirm = !prompt.figure.trim().is_empty();
                if ui
                    .add_enabled(can_confirm, egui::Button::new("Continue"))
                    .clicked()
                {
                    confirm = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
        if confirm {
            let prompt = self.pending_figure_prompt.take().expect("checked above");
            return match self.crop_current_page(prompt.min, prompt.max) {
                Ok(raster) => {
                    let provenance = self.make_provenance(prompt.min, prompt.max, prompt.figure);
                    Some(CropResult::Plot(raster, provenance))
                }
                Err(e) => {
                    self.message = format!("crop failed: {e}");
                    None
                }
            };
        }
        if cancel {
            self.pending_figure_prompt = None;
        }
        None
    }

    /// Persist **every** not-yet-saved in-memory annotation, across all
    /// pages — not just [`Self::active_page`]'s (GH issue #35 2026-09-02:
    /// clicking "Save annotations" from Read mode, or after scrolling, used
    /// to look up the embedded reader's current page and find nothing).
    ///
    /// With an active paper: each becomes a real §13/§14 fenced-TOML
    /// artifact (`kind = "annotation"`, `[source]` page + normalised
    /// `region` from the annotation's own captured page size) via
    /// [`classify::insert_artifact`], so it shows in the page-context list
    /// and `anchored_to_page`. The shared `context_editor`'s unsaved edits
    /// are folded in first, then it is reloaded and scrolled to the first
    /// new block. Falls back to the plain-text
    /// [`crate::project::append_to_section`] path only for a PDF outside any
    /// paper.
    fn save_annotations_into_project(
        &mut self,
        active_paper: Option<&mut PaperSession>,
        context_editor: &mut KvimEditorState,
    ) {
        // A still-open Annotate editor with text in it hasn't hit its own
        // "Save" yet — fold it in so the click doesn't silently drop it.
        if let Some(ed) = self.annotate_editor.take() {
            if !ed.text.trim().is_empty() {
                let page_px = self.current_page_px();
                let author = self.author_name();
                let anns = self.annotations.entry(self.active_page()).or_default();
                match ed.editing_existing {
                    Some(i) if i < anns.len() => {
                        anns[i].text = ed.text;
                        anns[i].min = ed.min;
                        anns[i].max = ed.max;
                        anns[i].page_px = page_px;
                    }
                    _ => anns.push(Annotation {
                        min: ed.min,
                        max: ed.max,
                        text: ed.text,
                        created_at: utc_now_iso8601(),
                        author,
                        page_px,
                    }),
                }
            }
        }

        // Deterministic order: page ascending, then insertion order.
        let mut pending: Vec<(usize, Vec<Annotation>)> = self
            .annotations
            .iter()
            .filter(|(_, a)| !a.is_empty())
            .map(|(p, a)| (*p, a.clone()))
            .collect();
        pending.sort_by_key(|(p, _)| *p);
        if pending.is_empty() {
            self.message = "no annotations to save".to_string();
            return;
        }
        let count: usize = pending.iter().map(|(_, a)| a.len()).sum();

        if let Some(session) = active_paper {
            if context_editor.is_modified() {
                session.set_markdown(context_editor.text());
            }

            let mut first_id: Option<String> = None;
            for (pg, anns) in &pending {
                for ann in anns {
                    let snippet: String = ann
                        .text
                        .split_whitespace()
                        .take(8)
                        .collect::<Vec<_>>()
                        .join(" ");
                    let heading = if snippet.is_empty() {
                        format!("Annotation (p{})", pg + 1)
                    } else {
                        format!("Annotation (p{}) — {snippet}", pg + 1)
                    };
                    let anchor = SourceAnchor {
                        page: Some((pg + 1) as u32),
                        pages: None,
                        region: ann.region(),
                    };
                    let index = crate::research_record::ResearchRecordIndex::from_session(session);
                    match classify::insert_artifact(
                        session,
                        &index,
                        &heading,
                        ArtifactKind::Annotation,
                        Some(anchor),
                        Classification::default(),
                        None,
                        &ann.text,
                    ) {
                        Ok(a) => {
                            first_id.get_or_insert_with(|| a.id().to_string());
                        }
                        Err(e) => {
                            self.message = format!("annotation save failed: {e}");
                            return;
                        }
                    }
                }
            }

            match session.save_document() {
                Ok(()) => {
                    self.message =
                        format!("saved {count} annotation(s) into {}", session.citekey());
                    // Persisted — drop the in-memory overlays and leave
                    // annotate edit mode (the maintainer's ask: the button
                    // both saves and escapes editing). The artifacts' own
                    // regions draw the boxes now.
                    self.annotations.clear();
                    self.pending_box = None;
                    self.context_menu = None;
                    self.tool = AnnotationTool::None;
                    self.editing_block_id = None;
                    let md = session.markdown().to_string();
                    context_editor.load_text(&md);
                    if let Some(a) = first_id
                        .and_then(|id| crate::artifact::parse_document(&md).get(&id).cloned())
                    {
                        context_editor.jump_to_line(a.line);
                    }
                    self.context_page_synced = None;
                }
                Err(e) => self.message = e.to_string(),
            }
            return;
        }

        // --- no active paper: the legacy plain-text section path ---
        let mut block = String::new();
        for (pg, anns) in &pending {
            for ann in anns {
                block.push_str(&format!(
                    "### annotation — {}\n- author: {}\n- page: {}\n- pixel bbox: [{:.1}, {:.1}, {:.1}, {:.1}]\n\n{}\n\n",
                    ann.created_at, ann.author, pg + 1, ann.min.x, ann.min.y, ann.max.x, ann.max.y, ann.text
                ));
            }
        }
        let block = block.trim_end();
        if self.project_root.trim().is_empty() || self.project_markdown_rel.trim().is_empty() {
            self.message = "set the project root and markdown path first".to_string();
            return;
        }
        match project::append_to_section(
            std::path::Path::new(self.project_root.trim()),
            self.project_markdown_rel.trim(),
            "annotations",
            block,
        ) {
            Ok(_) => self.message = format!("saved {count} annotation(s) into project markdown"),
            Err(e) => self.message = e.to_string(),
        }
    }

    /// Ask the continuous canvas to scroll one page forward, clamped to the
    /// document. Only meaningful on a PDF.
    fn annotate_next_page(&mut self) {
        let target = (self.annotate_page + 1).min(self.source.page_count().saturating_sub(1));
        if target != self.annotate_page {
            self.scroll_request = Some(target);
            self.pending_box = None;
            self.context_menu = None;
            self.text_selection = None;
        }
    }

    fn annotate_prev_page(&mut self) {
        let target = self.annotate_page.saturating_sub(1);
        if target != self.annotate_page {
            self.scroll_request = Some(target);
            self.pending_box = None;
            self.context_menu = None;
            self.text_selection = None;
        }
    }

    /// Rescan the whole document for `self.search.query` (GH issue #35
    /// 2026-09-02) — the `/`-search the dropped embedded reader used to
    /// provide. Runs only when the query text changes (see
    /// `SearchState::computed_for`), synchronously: a paper is a few dozen
    /// pages of structured text, cheap enough not to warrant a worker.
    fn recompute_search(&mut self) {
        self.search.computed_for = self.search.query.clone();
        self.search.hits.clear();
        self.search.current = None;
        let needle = self.search.query.trim();
        let Some(doc) = self.current_pdf_document() else {
            return;
        };
        if needle.is_empty() {
            return;
        }
        let scale = RENDER_DPI / 72.0;
        let pages = self.source.page_count();
        let mut hits = Vec::new();
        for page in 0..pages {
            let Ok(stext) = page_to_stext(doc, page, StextOptions::default()) else {
                continue;
            };
            for block in &stext.blocks {
                let StextBlock::Text(tb) = block else {
                    continue;
                };
                for line in &tb.lines {
                    for (min, max) in line_hits(line, needle, scale) {
                        hits.push(SearchHit { page, min, max });
                    }
                }
            }
        }
        if !hits.is_empty() {
            self.search.current = Some(0);
            self.scroll_request = Some(hits[0].page);
        }
        self.search.hits = hits;
    }

    /// Move the current search hit by `dir` (`+1` / `-1`), wrapping, and
    /// scroll the canvas to its page.
    fn search_step(&mut self, dir: isize) {
        let n = self.search.hits.len();
        if n == 0 {
            return;
        }
        let cur = self.search.current.unwrap_or(0) as isize;
        let next = ((cur + dir).rem_euclid(n as isize)) as usize;
        self.search.current = Some(next);
        self.scroll_request = Some(self.search.hits[next].page);
    }

    /// The left page-picker strip (op-0y4k's "Okular-style page thumbnails",
    /// restored on kovan's own rasters after the embedded reader that used to
    /// provide it was dropped — GH issue #35 2026-09-02).
    ///
    /// Virtualised: only the thumbnails the strip's own viewport touches are
    /// rasterised ([`PageView::ensure_thumbs`]). The current page's row is
    /// highlighted and kept in view; clicking a row scrolls the main canvas
    /// to that page.
    fn thumbnail_strip(&mut self, ui: &mut egui::Ui, pages: usize) {
        let thumb = self.pages.thumb_size_px();
        let width = ui.available_width().max(48.0);
        let img_w = (width - 14.0).max(24.0);
        let img_h = img_w * (thumb.y / thumb.x.max(1.0));
        let row_h = img_h + 20.0;
        let current = self.active_page();
        let follow = self.thumb_synced != Some(current);
        let mut jump = None;

        egui::ScrollArea::vertical()
            .id_salt("pdf_thumb_strip")
            .show_viewport(ui, |ui, viewport| {
                let last_page = pages.saturating_sub(1);
                let first = ((viewport.min.y / row_h).floor().max(0.0) as usize).min(last_page);
                let last = ((viewport.max.y / row_h).floor().max(0.0) as usize).min(last_page);
                if let ReaderSource::Pdf(reader) = &self.source {
                    self.pages
                        .ensure_thumbs(ui.ctx(), reader.document(), first..=last);
                }

                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(width, pages as f32 * row_h), Sense::hover());
                let painter = ui.painter_at(rect);
                let row_of = |p: usize| {
                    Rect::from_min_size(
                        Pos2::new(rect.min.x, rect.min.y + p as f32 * row_h),
                        egui::vec2(width, row_h),
                    )
                };

                if follow {
                    ui.scroll_to_rect(row_of(current), Some(egui::Align::Center));
                }

                for p in first..=last {
                    let row = row_of(p);
                    let resp = ui.interact(row, ui.id().with(("kovan-thumb", p)), Sense::click());
                    if p == current {
                        painter.rect_filled(
                            row,
                            3.0,
                            Color32::from_rgba_unmultiplied(120, 170, 255, 60),
                        );
                    } else if resp.hovered() {
                        painter.rect_filled(
                            row,
                            3.0,
                            Color32::from_rgba_unmultiplied(150, 150, 150, 30),
                        );
                    }
                    let img = Rect::from_min_size(
                        Pos2::new(row.min.x + 7.0, row.min.y + 3.0),
                        egui::vec2(img_w, img_h),
                    );
                    match self.pages.thumb(p) {
                        Some(tex) => {
                            painter.image(
                                tex.id(),
                                img,
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }
                        None => {
                            painter.rect_filled(img, 0.0, Color32::from_gray(235));
                        }
                    }
                    painter.rect_stroke(
                        img,
                        0.0,
                        Stroke::new(
                            if p == current { 2.0 } else { 1.0 },
                            Color32::from_gray(150),
                        ),
                        egui::StrokeKind::Outside,
                    );
                    painter.text(
                        Pos2::new(row.center().x, img.max.y + 2.0),
                        egui::Align2::CENTER_TOP,
                        format!("{}", p + 1),
                        egui::FontId::proportional(11.0),
                        ui.visuals().weak_text_color(),
                    );
                    if resp.clicked() {
                        jump = Some(p);
                    }
                }
            });

        if follow {
            self.thumb_synced = Some(current);
        }
        if let Some(p) = jump {
            self.scroll_request = Some(p);
            self.annotate_page = p;
            self.thumb_synced = Some(p);
        }
    }

    /// Structured text for `page` (op-z9u0), cached only for the page it
    /// was last extracted for — see `stext_cache`'s doc. `None` for a
    /// directly-loaded image (there is no PDF text layer to extract) or an
    /// extraction failure.
    fn stext_for_page(&mut self, page: usize) -> Option<&StextPage> {
        if !matches!(self.stext_cache, Some((p, _)) if p == page) {
            let doc = self.current_pdf_document()?;
            let stext = page_to_stext(doc, page, StextOptions::default()).ok()?;
            self.stext_cache = Some((page, stext));
        }
        self.stext_cache.as_ref().map(|(_, s)| s)
    }

    /// The right "page context" panel (op-0y4k, op-j178, GH issue #35
    /// 2026-09-02). With an active paper open (`active_artifacts` is
    /// `Some`), top to bottom:
    ///
    /// 1. **Anchored-block cards** — one per artifact
    ///    [`crate::research_record::ResearchRecordIndex::anchored_to_page`]
    ///    returns for [`Self::active_page`]; a CSV card also shows its
    ///    `draw_csv_preview`. Hovering a card highlights the matching
    ///    `region` box on the canvas (`panel_hover_id`); a box hovered on
    ///    the canvas highlights the card + the preview lines (op-4x5s).
    /// 2. **A read-only markdown preview** ([`KvimEditorState::ui_readonly`])
    ///    with every anchored block banded. It cannot be typed into —
    ///    editing a schema-sensitive block as raw text is how the fenced
    ///    TOML gets broken.
    ///
    /// Clicking a text card or a banded block in the preview, and
    /// **double-clicking** a digitised table/graph card, is the only edit
    /// path (GH issue #35 2026-09-02: "a single click to bring me into
    /// insert mode, not double click" — the heavier digitiser re-open keeps
    /// its double-click): a text/annotation/formula/source-reference block
    /// opens in `block_editor` (kvim, with `@`/`[[` autocompletion) → Save
    /// goes through [`classify::replace_artifact_body`]; a digitised
    /// table/graph block re-crops its `[source]` region from the PDF and
    /// hands it back as a [`CropResult`] the app routes to the matching
    /// digitiser.
    ///
    /// Falls back to the old disk-text `blocks_matching` preview over
    /// `project_root`/`project_markdown_rel` when no paper is active.
    #[allow(clippy::needless_option_as_deref)] // `active_paper` reborrowed for two sinks
    fn context_panel(
        &mut self,
        ui: &mut egui::Ui,
        active_artifacts: Option<&[Artifact]>,
        mut active_paper: Option<&mut PaperSession>,
        context_editor: &mut KvimEditorState,
        completion: Option<CompletionSource<'_>>,
    ) -> Option<CropResult> {
        ui.heading("Page context");
        let Some(artifacts) = active_artifacts else {
            self.context_panel_fallback(ui);
            return None;
        };
        let page0 = self.active_page();
        let page = (page0 + 1) as u32;
        let anchored: Vec<&Artifact> = artifacts
            .iter()
            .filter(|a| a.toml.source.as_ref().is_some_and(|s| s.covers_page(page)))
            .collect();

        let editor_text = context_editor.text();
        // op-j178: band every anchored block in the preview, and (op-4x5s)
        // a stronger band on whichever block's box is hovered on the canvas.
        context_editor.set_anchor_bands(
            anchored
                .iter()
                .map(|a| block_span(&editor_text, a))
                .collect(),
        );
        let hovered_block = self.hover_created_at.as_deref().and_then(|h| {
            anchored
                .iter()
                .find(|a| a.toml.kovan.created == h || a.id() == h)
        });
        context_editor.set_hover_band(hovered_block.map(|a| block_span(&editor_text, a)));

        // op-j178: scroll the preview to this page's blocks on a page change.
        if self.context_page_synced != Some(page0) {
            let target = anchored
                .iter()
                .map(|a| a.line)
                .min()
                .or_else(|| first_note_heading_line(&editor_text, page0));
            if let Some(line) = target {
                context_editor.jump_to_line(line);
            }
            self.context_page_synced = Some(page0);
        }

        let editing_id = self.editing_block_id.clone();
        let mut panel_hover: Option<String> = None;
        let mut open_target: Option<String> = None; // artifact id to open (dbl-click)
        let mut block_save: Option<(String, String)> = None;
        let mut block_cancel = false;

        // Inline block editor — pinned above the (scrolling) card list and
        // preview so its Save/Cancel are always in view, and with the
        // buttons above the editor (maintainer's ask, GH issue #35
        // 2026-09-02).
        if let Some(id) = &editing_id {
            // Look the block up in the *whole* document, not just this
            // page's anchored set — otherwise moving the mouse off the
            // canvas (which re-points `active_page` at the top of the
            // viewport) makes the editor vanish (maintainer's bug 2026-09-02).
            if let Some(a) = artifacts.iter().find(|a| a.id() == id.as_str()) {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(format!("editing: {}", a.heading));
                        if ui.button("Save").clicked() {
                            block_save = Some((id.clone(), self.block_editor.text()));
                        }
                        if ui.button("Cancel").clicked() {
                            block_cancel = true;
                        }
                    });
                    ui.push_id(("block-editor", id), |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("pdf_block_editor_scroll")
                            .max_height(220.0)
                            .show(ui, |ui| self.block_editor.ui(ui, completion));
                    });
                });
                ui.separator();
            }
        }

        egui::ScrollArea::vertical()
            .id_salt("pdf_context_panel_scroll")
            .show(ui, |ui| {
                if anchored.is_empty() {
                    ui.small("nothing saved for this page yet");
                }
                for artifact in &anchored {
                    let id = artifact.id().to_string();
                    let linked = hovered_block.is_some_and(|h| h.id() == id);
                    let fill = if linked {
                        Color32::from_rgba_unmultiplied(255, 230, 60, 40)
                    } else {
                        Color32::TRANSPARENT
                    };
                    // A text block opens on a single click (GH issue #35
                    // 2026-09-02: "a single click to bring me into insert mode");
                    // a digitised table/graph still needs a double-click, since
                    // opening the digitiser is the heavier action and its card is
                    // also the canvas-highlight hover target.
                    let mut open_on_single_click = false;
                    let inner = egui::Frame::new()
                        .fill(fill)
                        .inner_margin(4.0)
                        .show(ui, |ui| match artifact.kind() {
                            ArtifactKind::DigitisedTable | ArtifactKind::DigitisedGraph => {
                                let icon = if artifact.kind() == ArtifactKind::DigitisedTable {
                                    "\u{1F4CA}"
                                } else {
                                    "\u{1F4C8}"
                                };
                                ui.label(format!("{icon} {}", artifact.heading));
                                if let Some(csv) = artifact.csv_block() {
                                    // No copy button here: the preview is
                                    // for recognising the artifact, not for
                                    // exporting it.
                                    draw_csv_preview(ui, csv, CopyButton::Hidden, &id);
                                }
                                ui.small("double-click → go to page · right-click → menu");
                            }
                            _ => {
                                open_on_single_click = true;
                                ui.label(format!("\u{1F4DD} {}", artifact.heading));
                                if !artifact.body.trim().is_empty() {
                                    ui.monospace(body_preview(&artifact.body));
                                }
                                ui.small("click → edit · right-click → menu");
                            }
                        });
                    if inner.response.hovered() {
                        panel_hover = Some(id.clone());
                    }
                    // Right-click anywhere on the card opens the same menu a
                    // right-click on its canvas box does — every artifact
                    // gets a dropdown, including the ones with no region to
                    // click on (maintainer, GH issue #35, 2026-09-08).
                    if inner.response.secondary_clicked() {
                        let screen_pos = inner
                            .response
                            .interact_pointer_pos()
                            .unwrap_or_else(|| inner.response.rect.center());
                        self.toggle_context_menu(screen_pos, ContextMenuTarget::SavedArtifact(id.clone()));
                    }
                    let opened = if open_on_single_click {
                        inner.response.clicked() || inner.response.double_clicked()
                    } else {
                        inner.response.double_clicked()
                    };
                    if opened {
                        open_target = Some(id.clone());
                    }
                    ui.separator();
                }

                ui.add_space(8.0);
                ui.strong("Preview (read-only)");
                if let Some(line) = context_editor.ui_readonly(ui) {
                    if let Some(a) = artifacts
                        .iter()
                        .find(|a| block_span(&editor_text, a).contains(&line))
                    {
                        open_target = Some(a.id().to_string());
                    }
                }
            });

        self.panel_hover_id = panel_hover;

        if block_cancel {
            self.editing_block_id = None;
        }

        // A double-click landed on a block — open the right editor for it.
        let mut crop_result = None;
        if let Some(id) = open_target {
            if let Some(a) = artifacts.iter().find(|a| a.id() == id) {
                crop_result = self.open_artifact(a);
            }
        }

        // The inline block editor's Save — replace just that block's body.
        if let Some((id, new_body)) = block_save {
            self.editing_block_id = None;
            if let Some(session) = active_paper.as_deref_mut() {
                match classify::replace_artifact_body(session, &id, &new_body) {
                    Ok(_) => match session.save_document() {
                        Ok(()) => {
                            context_editor.load_text(session.markdown());
                            self.message = format!("saved edit to {id}");
                        }
                        Err(e) => self.message = e.to_string(),
                    },
                    Err(e) => self.message = format!("edit failed: {e}"),
                }
            }
        }

        crop_result
    }

    /// The pre-op-j178 read-only text preview, kept as the fallback for a
    /// PDF opened outside any paper (see [`Self::context_panel`]'s doc):
    /// raw text preview of whatever `project_root`/`project_markdown_rel`
    /// records for [`Self::active_page`], read live off disk (not cached),
    /// matching GitHub issue #30's "live from markdown file" ask. Filters
    /// `### ...` subsections by a `page: N`/`page N,` marker, matching the
    /// exact provenance text `Self::save_annotations_into_project`'s
    /// fallback path emits.
    fn context_panel_fallback(&mut self, ui: &mut egui::Ui) {
        if self.project_root.trim().is_empty() || self.project_markdown_rel.trim().is_empty() {
            ui.small(
                "Set a project root + markdown path above to see this page's saved \
                 annotations/CSVs here, live from the markdown file.",
            );
            return;
        }
        let path =
            std::path::Path::new(self.project_root.trim()).join(self.project_markdown_rel.trim());
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                ui.colored_label(
                    Color32::from_rgb(230, 90, 90),
                    format!("{}: {e}", path.display()),
                );
                return;
            }
        };
        let page = self.active_page();
        let marker_a = format!("page: {}", page + 1);
        let marker_b = format!("page {},", page + 1);
        let blocks = blocks_matching(&text, &[&marker_a, &marker_b]);
        if blocks.is_empty() {
            ui.small("nothing saved for this page yet");
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("pdf_context_panel_scroll")
            .show(ui, |ui| {
                for block in blocks {
                    // op-4x5s: highlight the block matching whatever annotation
                    // box the pointer was hovering over the annotate canvas,
                    // one frame ago (see `hover_created_at`'s doc).
                    let is_linked = self
                        .hover_created_at
                        .as_deref()
                        .is_some_and(|id| block.contains(id));
                    if is_linked {
                        egui::Frame::new()
                            .fill(Color32::from_rgba_unmultiplied(255, 230, 60, 40))
                            .inner_margin(4.0)
                            .show(ui, |ui| ui.monospace(&block));
                    } else {
                        ui.monospace(&block);
                    }
                    ui.separator();
                }
            });
    }

    /// Draw the toolbar and the continuous page canvas. `on_open_clicked` is
    /// called when the user asks to open a different document — the caller
    /// owns the file dialog (shared with the digitiser's "Load image"
    /// action) and reports the chosen path back via [`PdfReaderState::open`].
    ///
    /// Returns `Some` the frame the user completes a crop-then-right-click
    /// gesture (op-p17q / op-hnhp) or double-clicks a saved digitised box —
    /// the caller (`DigitiseApp`) loads it into the matching digitiser tab
    /// and switches views.
    ///
    /// `active_paper` is the wider app's [`crate::app::
    /// DigitiseApp::activate_paper`]'d paper, if any (op-q1qj, GH issue #35
    /// 2026-09-01 05:37: "project root isn't decided") — when `Some`,
    /// annotations save straight into its canonical Markdown and the
    /// page-context panel reads live from the same file, instead of the
    /// manual `project_root`/`project_markdown_rel` fields (which remain
    /// the fallback for a PDF opened outside any paper).
    ///
    /// `context_editor` is the shared page-context / Kvim-editor buffer
    /// (op-j178, GH issue #35 2026-09-02): the page-context panel renders it
    /// in place, scrolls it to the current page's blocks, and every
    /// annotation/inline-block save reloads it live. `completion` enables
    /// the citation/wiki autocomplete popup inside it (§29/§30). The caller
    /// must reload it from the session too after a save it drives itself.
    ///
    /// Returns `Some` only for a completed crop-then-right-click gesture.
    #[allow(clippy::needless_option_as_deref)] // `active_paper` reborrowed for two sinks
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        mut on_open_clicked: impl FnMut(),
        active_paper: Option<&mut PaperSession>,
        context_editor: &mut KvimEditorState,
        completion: Option<CompletionSource<'_>>,
    ) -> Option<CropResult> {
        let mut active_paper = active_paper;
        let active_citekey = active_paper.as_ref().map(|s| s.citekey().to_string());
        let active_artifacts: Option<Vec<Artifact>> = active_paper.as_ref().map(|s| {
            crate::research_record::ResearchRecordIndex::from_session(s)
                .artifacts()
                .to_vec()
        });

        ui.horizontal(|ui| {
            if ui.button("Open…").clicked() {
                on_open_clicked();
            }
            ui.label(if self.path.is_empty() {
                "nothing open"
            } else {
                self.path.as_str()
            });
        });

        // Legacy digitiser sections (GH issue #35, 2026-09-08). A graph or
        // table digitised while no paper was active was written as a plain
        // `### … — page N, pixel bbox […]` heading plus a bare ```csv fence,
        // with no `[kovan]` block — so `parse_document` never saw it as an
        // artifact and the canvas below drew no region box for it, while
        // annotations showed up fine. Offer the one-click upgrade rather
        // than rewriting the operator's file behind their back.
        let legacy_count = active_paper
            .as_ref()
            .map(|s| classify::find_legacy_csv_sections(s.markdown()).len())
            .unwrap_or(0);
        // A paper scaffolded before the artifact schema opens with a bare
        // `# <citekey>` and no TOML, so it has no paper header artifact.
        let needs_header = active_paper.as_ref().is_some_and(|s| {
            !crate::artifact::parse_document(s.markdown())
                .artifacts
                .iter()
                .any(|a| a.kind() == crate::artifact::ArtifactKind::Paper)
        });
        if legacy_count > 0 || needs_header {
            let page_px = self.current_page_px();
            let mut outcome: Option<String> = None;
            ui.horizontal(|ui| {
                ui.label(if legacy_count > 0 && needs_header {
                    format!(
                        "{legacy_count} digitiser section(s) in the old format, and no paper header block."
                    )
                } else if legacy_count > 0 {
                    format!(
                        "{legacy_count} digitiser section(s) saved in the old format — no region box is drawn for them."
                    )
                } else {
                    "This paper has no header artifact yet.".to_string()
                });
                if ui.button("Upgrade to artifacts").clicked() {
                    if let Some(session) = active_paper.as_mut() {
                        outcome = Some(
                            match classify::ensure_paper_header(session, None).and_then(|added| {
                                classify::migrate_legacy_csv_sections(session, page_px)
                                    .map(|n| (added, n))
                            }) {
                                Ok((added, n)) => match session.save_document() {
                                    Ok(()) => format!(
                                        "upgraded {n} digitiser section(s){}",
                                        if added { ", added the paper header" } else { "" }
                                    ),
                                    Err(e) => format!("upgrade saved nothing: {e}"),
                                },
                                Err(e) => format!("upgrade failed: {e}"),
                            },
                        );
                    }
                }
            });
            if let Some(m) = outcome {
                self.message = m;
            }
        }

        if matches!(self.source, ReaderSource::None) {
            ui.centered_and_justified(|ui| {
                ui.label("nothing open — click \"Open…\" (PDF or image)");
            });
            if !self.message.is_empty() {
                ui.label(&self.message);
            }
            return None;
        }

        self.check_hot_reload(ui.ctx());

        let is_pdf = matches!(self.source, ReaderSource::Pdf(_));

        ui.horizontal(|ui| {
            if is_pdf {
                ui.toggle_value(&mut self.show_thumbs, "\u{25A6} Pages")
                    .on_hover_text("Show the page thumbnails");
                let mut enabled = self.hot_reload.is_enabled();
                if ui.checkbox(&mut enabled, "Hot reload").changed() {
                    self.hot_reload.set_enabled(enabled);
                }
                ui.separator();
                if ui.button("< Prev").clicked() {
                    self.annotate_prev_page();
                }
                ui.label(format!(
                    "page {} / {}",
                    self.annotate_page + 1,
                    self.source.page_count()
                ));
                if ui.button("Next >").clicked() {
                    self.annotate_next_page();
                }
                ui.separator();
            }
            // The slider stays (maintainer, 2026-09-02): dragging it means
            // the pointer is *outside* the viewer, which is exactly the
            // case that anchors the zoom on the centre of what is on
            // screen — well-defined and stable. Ctrl+scroll / `+` / `-`
            // over the page anchor on the pointer instead.
            ui.add(egui::Slider::new(&mut self.zoom, 0.25..=4.0).text("zoom"))
                .on_hover_text("Ctrl+scroll, or + / -, zooms about the pointer");
            ui.separator();
            ui.label("tool:");
            ui.selectable_value(&mut self.tool, AnnotationTool::None, "Pan")
                .on_hover_text("drag the page to scroll; arrow keys nudge, PageUp/Down turn pages");
            ui.selectable_value(&mut self.tool, AnnotationTool::DrawBox, "Draw box");
            if is_pdf {
                ui.selectable_value(&mut self.tool, AnnotationTool::SelectText, "Select text");
            }
            if ui.button("Clear page annotations").clicked() {
                self.annotations.remove(&self.active_page());
            }
            ui.separator();
            ui.label("author:");
            ui.add(egui::TextEdit::singleline(&mut self.author).desired_width(100.0));
            if is_pdf {
                ui.separator();
                if ui.button("Generate BibTeX").clicked() {
                    self.generate_bibtex();
                }
            }
        });

        // --- in-document search (GH issue #35 2026-09-02) ---
        if is_pdf {
            ui.horizontal(|ui| {
                ui.label("\u{1F50D}");
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.search.query)
                        .desired_width(180.0)
                        .hint_text("search this PDF"),
                );
                let go_next = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if self.search.query != self.search.computed_for {
                    self.recompute_search();
                }
                if self.search.hits.is_empty() {
                    if !self.search.query.is_empty() {
                        ui.weak("no matches");
                    }
                } else {
                    let cur = self.search.current.map_or(0, |i| i + 1);
                    ui.weak(format!("{cur} / {}", self.search.hits.len()));
                    if ui.button("\u{25C0}").clicked() {
                        self.search_step(-1);
                    }
                    if ui.button("\u{25B6}").clicked() || go_next {
                        self.search_step(1);
                    }
                }
            });
        }

        // Page-turn keys (only when nothing text-y has keyboard focus).
        let text_editing = self.editing_block_id.is_some() || self.annotate_editor.is_some();
        if is_pdf && !text_editing && ui.ctx().memory(|m| m.focused().is_none()) {
            let (page_down, page_up, next_hit, prev_hit) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::PageDown) || i.key_pressed(egui::Key::J),
                    i.key_pressed(egui::Key::PageUp) || i.key_pressed(egui::Key::K),
                    i.key_pressed(egui::Key::N) && !i.modifiers.shift,
                    i.key_pressed(egui::Key::N) && i.modifiers.shift,
                )
            });
            if next_hit && !self.search.hits.is_empty() {
                self.search_step(1);
            } else if prev_hit && !self.search.hits.is_empty() {
                self.search_step(-1);
            } else if page_down {
                self.annotate_next_page();
            } else if page_up {
                self.annotate_prev_page();
            }
        }

        ui.small(
            "Draw box → right-click it → Annotate / Digitise graph / Read table. \
             Right-click an existing box → Edit / Delete. Double-click a saved box → edit it.",
        );
        let mut save_clicked = false;
        ui.horizontal(|ui| {
            match &active_citekey {
                Some(citekey) => {
                    ui.label(format!("saving annotations into {citekey}'s notes"));
                }
                None => {
                    ui.label("project root");
                    ui.text_edit_singleline(&mut self.project_root);
                    ui.label("markdown path");
                    ui.text_edit_singleline(&mut self.project_markdown_rel);
                }
            }
            if ui.button("Save annotations").clicked() {
                save_clicked = true;
            }
        });
        if save_clicked {
            self.save_annotations_into_project(active_paper.as_deref_mut(), context_editor);
        }
        self.text_selection_panel(ui);
        let mut crop_result = self.annotate_editor_panel(ui);
        if let Some(result) = self.figure_prompt_panel(ui) {
            crop_result = Some(result);
        }
        if is_pdf {
            self.bibtex_panel(ui);
        }
        ui.separator();

        let panel_crop = egui::Panel::right("pdf_reader_context")
            .resizable(true)
            .default_size(460.0)
            .min_size(320.0)
            .show(ui, |ui| {
                self.context_panel(
                    ui,
                    active_artifacts.as_deref(),
                    active_paper.as_deref_mut(),
                    context_editor,
                    completion,
                )
            })
            .inner;
        if panel_crop.is_some() {
            crop_result = panel_crop;
        }

        // The page-picker strip (op-0y4k), restored on kovan's own rasters.
        let page_count = self.source.page_count();
        if is_pdf && self.show_thumbs && page_count > 1 {
            egui::Panel::left("pdf_reader_thumbs")
                .resizable(true)
                .default_size(132.0)
                .min_size(72.0)
                .show(ui, |ui| self.thumbnail_strip(ui, page_count));
        }

        // --- a plain image / a PDF: kovan's own **continuous**
        // multi-page canvas (GH issue #35 2026-09-02). Renders the pages
        // itself (via `PageView`) so it can draw saved region boxes over
        // them and double-click a box to edit it — which the embedded reader
        // cannot (kopitiam#107). ---
        let zoom = self.zoom;
        const GAP: f32 = 16.0;
        let n = self.source.page_count().max(1);
        let mut open_target: Option<String> = None;

        // A zoom change scales the content but not the `ScrollArea`'s
        // (absolute, in points) offset, so the same offset lands somewhere
        // else in the document. `ScrollArea` applies `scroll_offset` before
        // it lays out or reads input, so setting it outright is exact and
        // one-shot — unlike an *animated* `scroll_to_rect` afterwards, which
        // lags a frame and compounds.
        //
        // `forced_offset` is the pointer-anchored target the previous frame's
        // zoom gesture computed (keep the document point under the mouse
        // exactly where it was); the centre-anchor below is the fallback for
        // a zoom change from anywhere else.
        let stride = self.pages.page_stride(zoom, GAP);
        let content = self.pages.content_size(n, zoom, GAP);
        let zoom_changed = self.last_zoom > 0.0 && (self.last_zoom - zoom).abs() > f32::EPSILON;
        let mut area = egui::ScrollArea::both();
        if let Some(target) = self.scroll_request.take() {
            // An explicit page jump (Prev/Next, `j`/`k`, Ctrl+D/U, a search
            // hit, a thumbnail click, or opening a block from the panel).
            // Set outright rather than with an animated `scroll_to_rect`:
            // opening a digitised block switches the app to the digitiser
            // view in the same frame, so an animation would never get a
            // second frame to land and the page would still be wrong on the
            // way back (maintainer, 2026-09-02).
            self.forced_offset = None;
            area = area.scroll_offset(egui::vec2(
                self.last_offset.x,
                self.pages.page_top(target, zoom, GAP),
            ));
        } else if let Some(off) = self.forced_offset.take() {
            area = area.scroll_offset(off);
        } else if zoom_changed {
            let target_y = (self.scroll_anchor.y * stride - self.last_viewport.y * 0.5).max(0.0);
            let target_x = (self.scroll_anchor.x * content.x - self.last_viewport.x * 0.5).max(0.0);
            area = area.scroll_offset(egui::vec2(target_x, target_y));
        }

        let scroll_out = area.show_viewport(ui, |ui, viewport| {
            // Rasterise + upload the pages the viewport touches (± one page
            // of margin), evicting far ones.
            let vis = self.pages.visible_range(viewport, n, zoom, GAP);
            let want = vis.start().saturating_sub(1)..=(vis.end() + 1).min(n - 1);
            let render_scale = PageView::render_scale(zoom);
            match &self.source {
                ReaderSource::Pdf(reader) => {
                    self.pages
                        .ensure(ui.ctx(), reader.document(), want.clone(), render_scale);
                }
                ReaderSource::Image(raster) => {
                    let image = raster_to_color_image(raster);
                    self.pages.set_single_image(ui.ctx(), image);
                }
                ReaderSource::None => {}
            }

            let (rect, response) = ui.allocate_exact_size(content, Sense::click_and_drag());
            let origin = rect.min;
            let painter = ui.painter_at(rect);

            // --- zoom about the pointer: Ctrl+scroll (or pinch), and `+`/`-`
            // (GH issue #35 2026-09-02 — the maintainer removed the zoom
            // slider precisely because a slider and a mouse-anchored view
            // fight each other). The document point under the pointer is
            // pinned: read it at the current zoom, then compute the exact
            // scroll offset that puts it back under the pointer at the new
            // zoom, and force that offset on the next frame.
            let keys_free = !text_editing && ui.ctx().memory(|m| m.focused().is_none());

            // Ctrl+D / Ctrl+U step a page, but only while the pointer is over
            // the page pane (maintainer, 2026-09-02) — the same chords mean
            // something else to the kvim editor, which owns them when focused.
            if keys_free && response.hovered() {
                let (down, up) = ui.input(|i| {
                    (
                        i.modifiers.ctrl && i.key_pressed(egui::Key::D),
                        i.modifiers.ctrl && i.key_pressed(egui::Key::U),
                    )
                });
                if down || up {
                    let last = n - 1;
                    let target = if down {
                        (self.annotate_page + 1).min(last)
                    } else {
                        self.annotate_page.saturating_sub(1)
                    };
                    if target != self.annotate_page {
                        self.annotate_page = target;
                        self.scroll_request = Some(target);
                        self.pending_box = None;
                        self.context_menu = None;
                        self.text_selection = None;
                    }
                }
            }
            let (pinch, plus, minus) = ui.input(|i| {
                (
                    i.zoom_delta(),
                    keys_free
                        && (i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals)),
                    keys_free && i.key_pressed(egui::Key::Minus),
                )
            });
            let step = if plus {
                1.25
            } else if minus {
                1.0 / 1.25
            } else {
                1.0
            };
            let factor = pinch * step;
            if (factor - 1.0).abs() > 1e-4 {
                let new_zoom = (zoom * factor).clamp(0.25, 4.0);
                if (new_zoom - zoom).abs() > f32::EPSILON {
                    // Anchor on the pointer; if it is outside the viewer
                    // (a `+`/`-` press with the mouse parked elsewhere), on
                    // the centre of what is on screen instead.
                    let viewport_screen_min = origin + viewport.min.to_vec2();
                    let anchor_screen = response
                        .hover_pos()
                        .unwrap_or_else(|| viewport_screen_min + viewport.size() * 0.5);
                    let within = anchor_screen - viewport_screen_min;
                    let anchor_content = viewport.min + within;
                    // Convert to zoom-invariant document units, then back at
                    // the new zoom. Works anywhere, including an inter-page
                    // gap, because the gap is part of the stride.
                    let new_stride = self.pages.page_size_px().y * new_zoom + GAP;
                    let new_y = (anchor_content.y / stride) * new_stride;
                    let new_x = (anchor_content.x / zoom) * new_zoom;
                    self.zoom = new_zoom;
                    self.forced_offset = Some(egui::vec2(
                        (new_x - within.x).max(0.0),
                        (new_y - within.y).max(0.0),
                    ));
                }
            }

            // --- Pan mode: grab-and-drag the page to scroll (GH issue #35
            // 2026-09-02 — this tool is a *pan* tool, it does not select
            // text). The other tools own the primary drag for drawing a box
            // / selecting text, so this is scoped to `None`. ---
            if self.tool == AnnotationTool::None {
                response.clone().on_hover_cursor(if response.dragged() {
                    egui::CursorIcon::Grabbing
                } else {
                    egui::CursorIcon::Grab
                });
                if response.dragged_by(egui::PointerButton::Primary) {
                    // No animation: the view must track the pointer 1:1.
                    ui.scroll_with_delta_animation(
                        response.drag_delta(),
                        egui::style::ScrollAnimation::none(),
                    );
                }
            }

            // --- Arrow keys nudge the view, whenever no text field owns the
            // keyboard (GH issue #35 2026-09-02). Harmless in every tool
            // mode — nothing else binds a bare arrow key here. PageUp/Down
            // and j/k still turn whole pages. ---
            if keys_free {
                const ARROW_STEP: f32 = 90.0;
                let mut delta = egui::Vec2::ZERO;
                ui.input(|i| {
                    if i.key_pressed(egui::Key::ArrowDown) {
                        delta.y -= ARROW_STEP;
                    }
                    if i.key_pressed(egui::Key::ArrowUp) {
                        delta.y += ARROW_STEP;
                    }
                    if i.key_pressed(egui::Key::ArrowRight) {
                        delta.x -= ARROW_STEP;
                    }
                    if i.key_pressed(egui::Key::ArrowLeft) {
                        delta.x += ARROW_STEP;
                    }
                });
                if delta != egui::Vec2::ZERO {
                    ui.scroll_with_delta(delta);
                }
            }

            // Paint each visible page (a grey placeholder while it renders).
            for p in want.clone() {
                let pr = self.pages.page_rect(p, origin, zoom, GAP);
                if let Some(tex) = self.pages.texture(p) {
                    painter.image(
                        tex.id(),
                        pr,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                } else {
                    painter.rect_filled(pr, 0.0, Color32::from_gray(230));
                    painter.text(
                        pr.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("rendering page {}…", p + 1),
                        egui::FontId::proportional(14.0),
                        Color32::from_gray(120),
                    );
                }
            }

            // The current page is the one at the **centre of the viewport** —
            // what you are actually reading. It is deliberately *not* driven
            // by the pointer: with a stationary mouse over the canvas, zoom
            // (or any re-layout) slides a different page under the cursor and
            // the whole panel would jump pages for no reason (maintainer's
            // 2026-09-02 diagnosis: "the mouse is being used as a reference
            // point"). Held still during a gesture, while a proposed box is
            // waiting, and while a block is being edited in the panel.
            let busy = self.draw_start.is_some()
                || self.select_start.is_some()
                || self.pending_box.is_some()
                || self.annotate_editor.is_some()
                || self.editing_block_id.is_some();
            if !busy {
                let centre_page =
                    ((viewport.center().y / stride).floor().max(0.0) as usize).min(n - 1);
                self.annotate_page = centre_page;
            }
            // A *gesture* is the one thing that should re-point the page at
            // the pointer — you draw/right-click on the page under the mouse,
            // whichever that is.
            if !busy && (response.drag_started() || response.secondary_clicked()) {
                if let Some((p, _)) = response
                    .interact_pointer_pos()
                    .and_then(|s| self.pages.hit(s, origin, n, zoom, GAP))
                {
                    self.annotate_page = p;
                }
            }
            let page = self.annotate_page;
            let page_origin = origin + egui::vec2(0.0, self.pages.page_top(page, zoom, GAP));
            let to_image = move |pos: Pos2| -> Pos2 { ((pos - page_origin) / zoom).to_pos2() };
            let to_screen = move |p: Pos2| -> Pos2 { page_origin + p.to_vec2() * zoom };

            // --- drawing a new box ---
            if self.tool == AnnotationTool::DrawBox {
                if response.drag_started_by(egui::PointerButton::Primary) {
                    self.draw_start = response.interact_pointer_pos().map(to_image);
                }
                if let (Some(start), Some(pos)) = (self.draw_start, response.interact_pointer_pos())
                {
                    let current = to_image(pos);
                    let (min, max) = (
                        Pos2::new(start.x.min(current.x), start.y.min(current.y)),
                        Pos2::new(start.x.max(current.x), start.y.max(current.y)),
                    );
                    painter.rect_stroke(
                        Rect::from_min_max(to_screen(min), to_screen(max)),
                        0.0,
                        Stroke::new(2.0_f32, Color32::from_rgb(60, 200, 255)),
                        egui::StrokeKind::Middle,
                    );
                    if response.drag_stopped() {
                        self.pending_box = Some((min, max));
                        self.draw_start = None;
                    }
                }
            } else if self.tool == AnnotationTool::SelectText {
                if response.drag_started_by(egui::PointerButton::Primary) {
                    self.select_start = response.interact_pointer_pos().map(to_image);
                }
                if let (Some(start), Some(pos)) =
                    (self.select_start, response.interact_pointer_pos())
                {
                    let current = to_image(pos);
                    let (min, max) = (
                        Pos2::new(start.x.min(current.x), start.y.min(current.y)),
                        Pos2::new(start.x.max(current.x), start.y.max(current.y)),
                    );
                    painter.rect_stroke(
                        Rect::from_min_max(to_screen(min), to_screen(max)),
                        0.0,
                        Stroke::new(2.0_f32, Color32::from_rgb(120, 230, 120)),
                        egui::StrokeKind::Middle,
                    );
                    if response.drag_stopped() {
                        // op-z9u0: RENDER_DPI/72.0 converts a stext line's
                        // device-space (PDF points) bbox into this panel's
                        // texture-pixel space — the same scale
                        // `rasterize_page` itself applies for the DPI it
                        // was given.
                        let scale = RENDER_DPI / 72.0;
                        self.text_selection = self
                            .stext_for_page(page)
                            .map(|stext| select_text_in_rect(stext, scale, min, max))
                            .map(|text| (min, max, text));
                        self.select_start = None;
                    }
                }
            }

            // --- right-click: open the context menu on whatever box was hit ---
            if response.secondary_clicked() {
                if let Some(screen_pos) = response.interact_pointer_pos() {
                    let click = to_image(screen_pos);
                    if let Some((min, max)) = self.pending_box {
                        if click.x >= min.x
                            && click.x <= max.x
                            && click.y >= min.y
                            && click.y <= max.y
                        {
                            self.toggle_context_menu(screen_pos, ContextMenuTarget::NewBox);
                        }
                    } else if let Some(i) = self
                        .annotations
                        .get(&page)
                        .and_then(|anns| anns.iter().position(|a| a.contains(click)))
                    {
                        self.toggle_context_menu(screen_pos, ContextMenuTarget::Existing(i));
                    } else if let Some(id) = active_artifacts.as_deref().and_then(|arts| {
                        artifact_overlays_for_page(
                            arts,
                            page,
                            self.pages.page_size_px(),
                            origin,
                            zoom,
                            GAP,
                        )
                        .into_iter()
                        .find(|(_, r)| r.contains(screen_pos))
                        .map(|(art, _)| art.id().to_string())
                    }) {
                        // op-30um.3: a saved artifact's own region box —
                        // the full Edit/Add-connection/Edit-connections/
                        // Delete-connection/Delete-annotation menu.
                        self.toggle_context_menu(screen_pos, ContextMenuTarget::SavedArtifact(id));
                    }
                }
            }

            // --- overlays across every visible page ---
            let page_px = self.pages.page_size_px();
            let hover_screen = response.hover_pos();
            // Double-click, not single (maintainer's ask 2026-09-02) — a
            // stray single click on a box was too easy to trip.
            let opened = response.double_clicked();
            let pages = &self.pages;
            let box_rect = |p: usize, min: Pos2, max: Pos2| {
                Rect::from_min_max(
                    pages.project(p, min, origin, zoom, GAP),
                    pages.project(p, max, origin, zoom, GAP),
                )
            };

            // op-4x5s: which in-memory annotation on the active page is
            // hovered (also the right-click "Existing" target).
            let hovered = hover_screen.map(to_image).and_then(|p| {
                self.annotations
                    .get(&page)
                    .and_then(|anns| anns.iter().position(|a| a.contains(p)))
            });
            let mut hover_id: Option<String> = hovered.and_then(|i| {
                self.annotations
                    .get(&page)
                    .and_then(|anns| anns.get(i))
                    .map(|a| a.created_at.clone())
            });

            // In-memory (not-yet-saved) annotation boxes — amber — on every
            // visible page.
            for p in want.clone() {
                if let Some(anns) = self.annotations.get(&p) {
                    for (i, ann) in anns.iter().enumerate() {
                        let hot = p == page && hovered == Some(i);
                        let r = box_rect(p, ann.min, ann.max);
                        painter.rect_filled(
                            r,
                            0.0,
                            Color32::from_rgba_unmultiplied(
                                255,
                                230,
                                60,
                                if hot { 110 } else { 60 },
                            ),
                        );
                        painter.rect_stroke(
                            r,
                            0.0,
                            Stroke::new(
                                if hot { 2.5 } else { 1.0 },
                                Color32::from_rgb(230, 170, 20),
                            ),
                            egui::StrokeKind::Middle,
                        );
                    }
                }
            }

            // Saved-artifact region boxes — every source-anchored artifact
            // with a valid page + normalised region draws (op-30um.5:
            // generalised from annotation-only), coloured by
            // `theme::artifact_accent` per its `ArtifactKind`. Hover
            // highlights the panel card; a single click opens the artifact
            // for editing.
            if let Some(artifacts) = active_artifacts.as_deref() {
                let gui_theme = super::theme::GuiTheme::current(ui.visuals());
                for p in want.clone() {
                    for (art, r) in artifact_overlays_for_page(artifacts, p, page_px, origin, zoom, GAP)
                    {
                        let hit = hover_screen.is_some_and(|s| r.contains(s));
                        if hit {
                            hover_id = Some(art.toml.kovan.created.clone());
                            if opened {
                                open_target = Some(art.id().to_string());
                            }
                        }
                        let linked = hit || self.panel_hover_id.as_deref() == Some(art.id());
                        let accent = super::theme::artifact_accent(art.kind(), gui_theme);
                        let fill = Color32::from_rgba_unmultiplied(
                            accent.r(),
                            accent.g(),
                            accent.b(),
                            if linked { 90 } else { 40 },
                        );
                        painter.rect_filled(r, 0.0, fill);
                        painter.rect_stroke(
                            r,
                            0.0,
                            Stroke::new(if linked { 2.5 } else { 1.0 }, accent),
                            egui::StrokeKind::Middle,
                        );
                    }
                }
            }
            self.hover_created_at = hover_id;

            // Search hits — soft yellow on every visible page, the current
            // one a bright outline (GH issue #35 2026-09-02).
            for (i, hit) in self.search.hits.iter().enumerate() {
                if !want.contains(&hit.page) {
                    continue;
                }
                let r = box_rect(hit.page, hit.min, hit.max);
                let is_current = self.search.current == Some(i);
                painter.rect_filled(
                    r,
                    2.0,
                    Color32::from_rgba_unmultiplied(
                        255,
                        210,
                        40,
                        if is_current { 150 } else { 70 },
                    ),
                );
                if is_current {
                    painter.rect_stroke(
                        r,
                        2.0,
                        Stroke::new(2.0, Color32::from_rgb(210, 120, 0)),
                        egui::StrokeKind::Outside,
                    );
                }
            }

            if let Some((min, max)) = self.pending_box {
                painter.rect_stroke(
                    Rect::from_min_max(to_screen(min), to_screen(max)),
                    0.0,
                    Stroke::new(2.0_f32, Color32::from_rgb(60, 200, 255)),
                    egui::StrokeKind::Middle,
                );
            }
            if let Some((min, max, _)) = &self.text_selection {
                painter.rect_filled(
                    Rect::from_min_max(to_screen(*min), to_screen(*max)),
                    0.0,
                    Color32::from_rgba_unmultiplied(120, 230, 120, 50),
                );
            }
        });

        // Re-derive the zoom-independent view anchor from the `ScrollArea`'s
        // *actual* offset, so the next zoom change restores exactly this
        // view point. Doing it from the real offset (rather than tracking it
        // ourselves) means ordinary wheel/drag scrolling stays authoritative.
        let viewport_size = scroll_out.inner_rect.size();
        if viewport_size.x > 0.0 && viewport_size.y > 0.0 {
            self.last_viewport = viewport_size;
        }
        self.last_offset = scroll_out.state.offset;
        self.scroll_anchor = egui::vec2(
            (scroll_out.state.offset.x + self.last_viewport.x * 0.5) / content.x.max(1.0),
            (scroll_out.state.offset.y + self.last_viewport.y * 0.5) / stride.max(1.0),
        );
        self.last_zoom = zoom;

        // A double-click on a saved region box (GH issue #35 2026-09-02):
        // straight into editing it.
        if let Some(id) = open_target {
            if let Some(art) = active_artifacts
                .as_deref()
                .and_then(|a| a.iter().find(|x| x.id() == id).cloned())
            {
                if let Some(r) = self.open_artifact(&art) {
                    crop_result = Some(r);
                }
            }
        }

        let root_index = completion.map(|c| (c.root, c.index));
        if let Some(result) = self.context_menu_ui(
            ui.ctx(),
            active_citekey.as_deref(),
            active_artifacts.as_deref(),
            root_index,
        ) {
            crop_result = Some(result);
        }
        self.connection_popup_ui(ui.ctx(), root_index, active_paper.as_deref_mut());

        crop_result
    }

    /// Draw the floating right-click menu (op-x9qn), if one is open.
    /// Returns `Some` the frame a Digitise-graph/Read-table crop is
    /// confirmed.
    ///
    /// `citekey` is the active paper's citekey (needed to build/resolve an
    /// `artifact:<citekey>#<id>` node for a [`ContextMenuTarget::SavedArtifact`]);
    /// `root_index` is `Some` only once a Kovan root and its
    /// [`KnowledgeIndex`] are both available (op-30um.3's connection actions
    /// are simply unavailable — shown as disabled, never a panic — without
    /// them, exactly like the citation/wiki completion popup this same
    /// `(root, index)` pair already feeds).
    fn context_menu_ui(
        &mut self,
        ctx: &egui::Context,
        citekey: Option<&str>,
        active_artifacts: Option<&[Artifact]>,
        root_index: Option<(&KovanRoot, &KnowledgeIndex)>,
    ) -> Option<CropResult> {
        let menu = self.context_menu.clone()?;
        let mut close = false;
        let mut result = None;
        let area = egui::Area::new(egui::Id::new("pdf_reader_context_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(menu.screen_pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(140.0);
                    match &menu.target {
                        ContextMenuTarget::NewBox => {
                            if ui.button("Annotate").clicked() {
                                if let Some((min, max)) = self.pending_box {
                                    self.annotate_editor = Some(AnnotateEditor {
                                        min,
                                        max,
                                        text: String::new(),
                                        editing_existing: None,
                                    });
                                }
                                close = true;
                            }
                            if ui.button("Digitise graph").clicked() {
                                // op-8ci2: ask for the figure identifier
                                // right now, while the figure is still in
                                // view, instead of cropping immediately and
                                // leaving the digitiser's required `figure*`
                                // field blank for the user to notice later.
                                if let Some((min, max)) = self.pending_box.take() {
                                    self.pending_figure_prompt = Some(PendingFigurePrompt {
                                        min,
                                        max,
                                        figure: String::new(),
                                    });
                                }
                                close = true;
                            }
                            if ui.button("Read table").clicked() {
                                if let Some((min, max)) = self.pending_box.take() {
                                    match self.crop_current_page(min, max) {
                                        Ok(raster) => {
                                            result = Some(CropResult::Table(
                                                raster,
                                                self.make_provenance(min, max, ""),
                                            ));
                                        }
                                        Err(e) => self.message = format!("crop failed: {e}"),
                                    }
                                }
                                close = true;
                            }
                        }
                        ContextMenuTarget::Existing(i) => {
                            let i = *i;
                            if ui.button("Edit").clicked() {
                                if let Some(a) = self
                                    .annotations
                                    .get(&self.active_page())
                                    .and_then(|a| a.get(i))
                                {
                                    self.annotate_editor = Some(AnnotateEditor {
                                        min: a.min,
                                        max: a.max,
                                        text: a.text.clone(),
                                        editing_existing: Some(i),
                                    });
                                }
                                close = true;
                            }
                            if ui.button("Delete").clicked() {
                                if let Some(anns) = self.annotations.get_mut(&self.active_page()) {
                                    if i < anns.len() {
                                        anns.remove(i);
                                    }
                                }
                                close = true;
                            }
                        }
                        // op-30um.3/.6: the full menu on a saved artifact's
                        // own region box, for every `ArtifactKind` alike.
                        // The entry list itself comes from the pure
                        // [`saved_artifact_menu_entries`] — this match arm
                        // does nothing but render that list and dispatch
                        // each [`MenuAction`], either reusing an existing
                        // operation ([`Self::open_artifact`]) or opening a
                        // [`ConnectionPopup`]/confirm dialog that itself
                        // calls straight into `crate::relation`/
                        // `crate::classify::delete_artifact_cascade`; no
                        // relation lookup, graph walk, or menu-composition
                        // logic happens in this match arm.
                        ContextMenuTarget::SavedArtifact(id) => {
                            let id = id.clone();
                            let node = citekey.map(|ck| artifact_node(ck, &id));
                            let have_library = root_index.is_some() && node.is_some();
                            let artifact = active_artifacts
                                .and_then(|arts| arts.iter().find(|a| a.id() == id).cloned());
                            match &artifact {
                                Some(art) => {
                                    let has_page = Self::artifact_page(art).is_some();
                                    for entry in
                                        saved_artifact_menu_entries(art.kind(), have_library, has_page)
                                    {
                                        if ui
                                            .add_enabled(
                                                entry.enabled,
                                                egui::Button::new(entry.label),
                                            )
                                            .clicked()
                                        {
                                            match entry.action {
                                                MenuAction::EditArtifact => {
                                                    if let Some(r) = self.open_artifact(art) {
                                                        result = Some(r);
                                                    }
                                                }
                                                MenuAction::GoToPage => self.go_to_artifact(art),
                                                MenuAction::GoToDigitiser => {
                                                    if let Some(r) = self.recrop_artifact(art) {
                                                        result = Some(r);
                                                    }
                                                }
                                                MenuAction::AddConnection => {
                                                    if let Some(source) = node.clone() {
                                                        self.connection_popup =
                                                            Some(ConnectionPopup::Add {
                                                                source,
                                                                query: String::new(),
                                                                kind: RelationKind::RelatedTo,
                                                            });
                                                    }
                                                }
                                                MenuAction::EditConnections
                                                | MenuAction::DeleteConnection => {
                                                    if let Some(node) = node.clone() {
                                                        self.connection_popup =
                                                            Some(ConnectionPopup::Manage { node });
                                                    }
                                                }
                                                MenuAction::DeleteArtifact => {
                                                    if let Some(ck) = citekey {
                                                        self.connection_popup =
                                                            Some(ConnectionPopup::ConfirmDelete {
                                                                citekey: ck.to_string(),
                                                                artifact_id: id.clone(),
                                                            });
                                                    }
                                                }
                                            }
                                            close = true;
                                        }
                                        if entry.separator_after {
                                            ui.separator();
                                        }
                                    }
                                }
                                None => {
                                    // The overlay that opened this menu came
                                    // from `active_artifacts`, so this is
                                    // only reachable if the artifact was
                                    // removed from under an open menu (e.g.
                                    // a cascade delete from elsewhere) —
                                    // nothing to act on, so offer nothing
                                    // but Cancel.
                                    ui.label("(artifact no longer available)");
                                }
                            }
                        }
                    }
                    ui.separator();
                    if ui.button("Cancel").clicked() {
                        if matches!(menu.target, ContextMenuTarget::NewBox) {
                            self.pending_box = None;
                        }
                        close = true;
                    }
                });
            });
        // A left-click anywhere outside the menu dismisses it, the way a
        // dropdown is expected to behave (maintainer, GH issue #35,
        // 2026-09-08). Checked against the menu's own rect rather than a
        // global "was clicked" flag, so a click *on* an entry still runs
        // that entry's action and closes via `close` below.
        // PRIMARY only. The right-click that opens this menu is also a
        // click, and on the opening frame the pointer sits at the menu's
        // own corner — treating any button here would make the menu close
        // itself the instant it appeared. Secondary clicks are the toggle
        // gesture and are handled at the call sites.
        let clicked_outside = ctx
            .input(|i| i.pointer.button_clicked(egui::PointerButton::Primary))
            && !ctx.input(|i| {
                i.pointer
                    .interact_pos()
                    .is_some_and(|p| area.response.rect.contains(p))
            });
        if close || clicked_outside {
            self.context_menu = None;
        }
        result
    }

    /// Draw the op-30um.3 connection sub-popup ([`ConnectionPopup`]), if one
    /// is open — a separate floating window from [`Self::context_menu_ui`]
    /// so a fuzzy-candidate list or a connections list has room, rather than
    /// being squeezed into the small right-click menu itself.
    ///
    /// Every button here calls straight into one of
    /// [`relation::add_connection`]/[`relation::connections`]/
    /// [`relation::edit_connection`]/[`relation::delete_connection`]/
    /// [`classify::delete_artifact_cascade`] and renders whatever it
    /// returns — this function never itself decides what edges exist, per
    /// op-30um.3's "no graph-walking in the UI layer" requirement.
    /// `active_paper` is the reader's own open session, needed because
    /// [`classify::delete_artifact_cascade`] works on its own short-lived
    /// sessions read from disk (an *incoming* relation lives in the other
    /// paper's Markdown, so one session cannot cover the write). After it
    /// succeeds, this session's buffer is stale: it still contains the
    /// deleted artifact, so the canvas would keep drawing its box and the
    /// next save would write the deletion back out. Reloading closes that
    /// gap — see [`crate::session::PaperSession::reload`].
    fn connection_popup_ui(
        &mut self,
        ctx: &egui::Context,
        root_index: Option<(&KovanRoot, &KnowledgeIndex)>,
        mut active_paper: Option<&mut PaperSession>,
    ) {
        let Some(popup) = self.connection_popup.clone() else {
            return;
        };
        let mut close = false;

        match popup {
            ConnectionPopup::Add {
                source,
                mut query,
                mut kind,
            } => {
                egui::Window::new("Add connection…")
                    .collapsible(false)
                    .resizable(true)
                    .show(ctx, |ui| {
                        let Some((root, index)) = root_index else {
                            ui.label("no library open");
                            if ui.button("Close").clicked() {
                                close = true;
                            }
                            return;
                        };
                        ui.horizontal(|ui| {
                            ui.label("connect");
                            ui.monospace(&source);
                            ui.label("as:");
                            if ui.button(kind.label()).clicked() {
                                kind = kind.next();
                            }
                        });
                        ui.add(
                            egui::TextEdit::singleline(&mut query)
                                .hint_text("search papers, artifacts, topics, projects…"),
                        );
                        ui.separator();
                        let candidates: Vec<LibraryCandidate> =
                            library_candidates(root, index, &query, &[]);
                        egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                            if candidates.is_empty() {
                                ui.label("no matches");
                            }
                            for c in &candidates {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{} — {}", c.candidate.label, c.candidate.detail));
                                    if ui.button("Add").clicked() {
                                        self.connection_message =
                                            match relation::add_connection(root, &source, &c.node, kind) {
                                                Ok(_) => {
                                                    format!("connected: {source} {} {}", kind.label(), c.node)
                                                }
                                                Err(e) => format!("could not add connection: {e}"),
                                            };
                                        close = true;
                                    }
                                });
                            }
                        });
                        if !self.connection_message.is_empty() {
                            ui.separator();
                            ui.label(&self.connection_message);
                        }
                        ui.separator();
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
                if !close {
                    self.connection_popup = Some(ConnectionPopup::Add { source, query, kind });
                }
            }
            ConnectionPopup::Manage { node } => {
                egui::Window::new("Connections")
                    .collapsible(false)
                    .resizable(true)
                    .show(ctx, |ui| {
                        let Some((root, index)) = root_index else {
                            ui.label("no library open");
                            if ui.button("Close").clicked() {
                                close = true;
                            }
                            return;
                        };
                        let conns = relation::connections(root, index, &node);
                        if conns.is_empty() {
                            ui.label("no connections");
                        }
                        for rel in &conns {
                            ui.horizontal(|ui| {
                                let (arrow, other) = relation_other_end(rel, &node);
                                ui.label(format!("{arrow} {other}"));
                                if ui
                                    .button(rel.kind.label())
                                    .on_hover_text("click to cycle the relation kind")
                                    .clicked()
                                {
                                    if let Err(e) =
                                        relation::edit_connection(root, index, &rel.id, None, Some(rel.kind.next()))
                                    {
                                        self.connection_message = format!("could not edit connection: {e}");
                                    }
                                }
                                if ui.button("Delete").clicked() {
                                    if let Err(e) = relation::delete_connection(root, index, &rel.id) {
                                        self.connection_message = format!("could not delete connection: {e}");
                                    }
                                }
                            });
                        }
                        if !self.connection_message.is_empty() {
                            ui.separator();
                            ui.label(&self.connection_message);
                        }
                        ui.separator();
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
                if !close {
                    self.connection_popup = Some(ConnectionPopup::Manage { node });
                }
            }
            ConnectionPopup::ConfirmDelete { citekey, artifact_id } => {
                egui::Window::new("Delete annotation")
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label(format!("Delete {artifact_id:?}?"));
                        ui.label("Sure anot?");
                        ui.horizontal(|ui| {
                            if ui.button("No").clicked() {
                                close = true;
                            }
                            if ui.button("Yes").clicked() {
                                self.connection_message = match root_index {
                                    // The cascade reads each paper from
                                    // disk, so an unsaved buffer would be
                                    // invisible to it — flush first, then
                                    // delete, then reload so this session
                                    // does not write the artifact back.
                                    Some((root, index)) => {
                                        let flushed = match active_paper.as_deref_mut() {
                                            Some(s) if s.is_dirty() => s.save_document().err(),
                                            _ => None,
                                        };
                                        match flushed {
                                            Some(e) => format!("could not save before deleting: {e}"),
                                            None => match classify::delete_artifact_cascade(
                                                root,
                                                index,
                                                &citekey,
                                                &artifact_id,
                                            ) {
                                                Ok(n) => {
                                                    let reload_err = active_paper
                                                        .as_deref_mut()
                                                        .and_then(|s| s.reload().err());
                                                    match reload_err {
                                                        Some(e) => format!(
                                                            "deleted, but reopening the paper failed: {e}"
                                                        ),
                                                        None => format!(
                                                            "deleted the artifact and {n} connection(s)"
                                                        ),
                                                    }
                                                }
                                                Err(e) => format!("could not delete: {e}"),
                                            },
                                        }
                                    }
                                    // Previously a silent no-op: the button
                                    // closed the dialog and nothing happened,
                                    // which reads exactly like a broken
                                    // delete. Say why instead.
                                    None => "cannot delete: no Kovan root and index are loaded \
                                             (open a library first)"
                                        .to_string(),
                                };
                                close = true;
                            }
                        });
                    });
                if !close {
                    self.connection_popup = Some(ConnectionPopup::ConfirmDelete { citekey, artifact_id });
                }
            }
        }

        if close {
            self.connection_popup = None;
        }
    }

    /// The last text selection (op-z9u0), if any — a read-only preview with
    /// Copy-to-clipboard and "Save as annotation" (folds the selection into
    /// the same `annotations` markdown section a hand-typed note goes into,
    /// per the module doc).
    fn text_selection_panel(&mut self, ui: &mut egui::Ui) {
        let Some((min, max, text)) = self.text_selection.clone() else {
            return;
        };
        ui.group(|ui| {
            ui.label(format!(
                "Selected text — page {} — bbox [{:.0}, {:.0}, {:.0}, {:.0}]",
                self.active_page() + 1,
                min.x,
                min.y,
                max.x,
                max.y
            ));
            let mut scratch = text.clone();
            ui.add(
                egui::TextEdit::multiline(&mut scratch)
                    .font(egui::TextStyle::Monospace)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            );
            ui.horizontal(|ui| {
                if ui.button("\u{1F4CB} Copy").clicked() {
                    ui.ctx().copy_text(text.clone());
                }
                if ui.button("Save as annotation").clicked() {
                    let author = self.author_name();
                    let page_px = self.current_page_px();
                    self.annotations
                        .entry(self.active_page())
                        .or_default()
                        .push(Annotation {
                            min,
                            max,
                            text: text.clone(),
                            created_at: utc_now_iso8601(),
                            author,
                            page_px,
                        });
                    self.text_selection = None;
                }
                if ui.button("Dismiss").clicked() {
                    self.text_selection = None;
                }
            });
        });
    }

    /// The Annotate text editor, shown as a panel under the toolbar while
    /// `annotate_editor` is `Some` — see [`AnnotateEditor`]'s doc for why
    /// this is a panel rather than a canvas-anchored popup. Returns `None`
    /// always (kept as `-> Option<CropResult>` only so `ui` can chain it the
    /// same way as `context_menu_ui`, for a single "did anything produce a
    /// crop this frame" return path); an Annotate action never produces a
    /// [`CropResult`].
    fn annotate_editor_panel(&mut self, ui: &mut egui::Ui) -> Option<CropResult> {
        let page = self.active_page();
        let Some(editor) = &mut self.annotate_editor else {
            return None;
        };
        let mut save = false;
        let mut cancel = false;
        ui.group(|ui| {
            ui.label(format!(
                "Annotate — page {} — bbox [{:.0}, {:.0}, {:.0}, {:.0}]",
                page + 1,
                editor.min.x,
                editor.min.y,
                editor.max.x,
                editor.max.y
            ));
            ui.add(
                egui::TextEdit::multiline(&mut editor.text)
                    .hint_text("note text")
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            );
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    save = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
        if save {
            let editor = self.annotate_editor.take().expect("checked above");
            let author = self.author_name();
            let page_px = self.current_page_px();
            let anns = self.annotations.entry(self.active_page()).or_default();
            match editor.editing_existing {
                Some(i) if i < anns.len() => {
                    anns[i].text = editor.text;
                    anns[i].min = editor.min;
                    anns[i].max = editor.max;
                    anns[i].page_px = page_px;
                }
                _ => anns.push(Annotation {
                    min: editor.min,
                    max: editor.max,
                    text: editor.text,
                    created_at: utc_now_iso8601(),
                    author,
                    page_px,
                }),
            }
            self.pending_box = None;
        } else if cancel {
            self.annotate_editor = None;
        }
        None
    }

    /// Shows the last "Generate BibTeX" result (op-x3wl), if any — a
    /// copy-to-clipboard field on success (via `egui`'s own
    /// `ctx().copy_text`, the same mechanism the digitiser's CSV preview
    /// button already uses — see `csv_preview.rs`), or the failure message.
    fn bibtex_panel(&mut self, ui: &mut egui::Ui) {
        let Some(result) = &self.bibtex else { return };
        match result {
            Ok(entry) => {
                ui.horizontal(|ui| {
                    ui.label("BibTeX:");
                    if ui.button("\u{1F4CB} Copy").clicked() {
                        ui.ctx().copy_text(entry.clone());
                    }
                });
                let mut scratch = entry.clone();
                ui.add(
                    egui::TextEdit::multiline(&mut scratch)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .desired_rows(6),
                );
            }
            Err(e) => {
                ui.colored_label(Color32::from_rgb(230, 90, 90), format!("BibTeX: {e}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kopitiam_pdf::mupdf::{Point, Quad, Rect as PdfRect, StextChar, StextLine, StextTextBlock};

    fn stub_char(c: char) -> StextChar {
        let p = Point::new(0.0, 0.0);
        StextChar {
            c,
            origin: p,
            quad: Quad {
                ul: p,
                ur: p,
                ll: p,
                lr: p,
            },
            size: 10.0,
            font: 0,
            flags: 0,
            cid: 0,
            wmode: 0,
        }
    }

    fn stub_line(text: &str, bbox: PdfRect) -> StextLine {
        StextLine {
            wmode: 0,
            flags: 0,
            dir: Point::new(1.0, 0.0),
            bbox,
            chars: text.chars().map(stub_char).collect(),
        }
    }

    fn stub_page(lines: Vec<StextLine>) -> StextPage {
        StextPage {
            mediabox: PdfRect::new(0.0, 0.0, 612.0, 792.0),
            blocks: vec![StextBlock::Text(StextTextBlock {
                bbox: PdfRect::new(0.0, 0.0, 612.0, 792.0),
                lines,
            })],
            fonts: Vec::new(),
        }
    }

    #[test]
    fn select_text_in_rect_picks_up_intersecting_lines_only() {
        // Two lines in PDF points, at y=[10,20] and y=[100,110]. A drag
        // rect covering just the first line (in pixel space, scale=1.0)
        // should select only its text.
        let page = stub_page(vec![
            stub_line("first line", PdfRect::new(0.0, 10.0, 200.0, 20.0)),
            stub_line("second line", PdfRect::new(0.0, 100.0, 200.0, 110.0)),
        ]);
        let text = select_text_in_rect(&page, 1.0, Pos2::new(0.0, 0.0), Pos2::new(300.0, 30.0));
        assert_eq!(text, "first line");
    }

    #[test]
    fn select_text_in_rect_applies_the_dpi_scale() {
        // Same fixture, but scale=2.0 (as if RENDER_DPI were 144) — the
        // line's device-space bbox in pixel space is now y=[20,40], so a
        // drag rect that only covers y=[0,15] in PIXEL space should miss it
        // even though it would have hit at scale=1.0.
        let page = stub_page(vec![stub_line(
            "line",
            PdfRect::new(0.0, 10.0, 200.0, 20.0),
        )]);
        let missed = select_text_in_rect(&page, 2.0, Pos2::new(0.0, 0.0), Pos2::new(300.0, 15.0));
        assert_eq!(missed, "");
        let hit = select_text_in_rect(&page, 2.0, Pos2::new(0.0, 0.0), Pos2::new(300.0, 30.0));
        assert_eq!(hit, "line");
    }

    #[test]
    fn select_text_in_rect_multiple_lines_join_with_newline() {
        let page = stub_page(vec![
            stub_line("a", PdfRect::new(0.0, 0.0, 10.0, 10.0)),
            stub_line("b", PdfRect::new(0.0, 20.0, 10.0, 30.0)),
        ]);
        let text = select_text_in_rect(&page, 1.0, Pos2::new(0.0, 0.0), Pos2::new(50.0, 50.0));
        assert_eq!(text, "a\nb");
    }

    #[test]
    fn select_text_in_rect_no_intersection_is_empty() {
        let page = stub_page(vec![stub_line("x", PdfRect::new(0.0, 0.0, 10.0, 10.0))]);
        let text = select_text_in_rect(
            &page,
            1.0,
            Pos2::new(1000.0, 1000.0),
            Pos2::new(1100.0, 1100.0),
        );
        assert_eq!(text, "");
    }

    #[test]
    fn blocks_matching_keeps_only_blocks_containing_a_needle() {
        let text = "\
### Fig. 7 — page 3, pixel bbox [1, 2, 3, 4]

```csv
x,y
1,2
```

### annotation — 2026-08-24T00:00:00Z
- author: x
- page: 1
- pixel bbox: [0, 0, 1, 1]

a note
";
        let blocks = blocks_matching(text, &["page: 1"]);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].starts_with("### annotation"));
        assert!(blocks[0].contains("a note"));
    }

    #[test]
    fn blocks_matching_supports_multiple_needles() {
        let text = "### a — page 1,\nx\n### b\n- page: 2\ny\n### c\nz\n";
        let blocks = blocks_matching(text, &["page 1,", "page: 2"]);
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn blocks_matching_no_match_is_empty() {
        assert!(blocks_matching("### a\nx\n", &["page: 99"]).is_empty());
    }

    #[test]
    fn blocks_matching_text_with_no_headings_is_empty() {
        assert!(blocks_matching("just prose, no ### headings\n", &["anything"]).is_empty());
    }

    #[test]
    fn substr_char_ranges_is_case_insensitive_and_non_overlapping() {
        let chars: Vec<char> = "The rho of the RHO-region, rhorho".chars().collect();
        let hits = substr_char_ranges(&chars, "rho");
        assert_eq!(
            hits.len(),
            4,
            "two lower, one upper, and rhorho as two non-overlapping"
        );
        for (s, e) in hits {
            let m: String = chars[s..e].iter().map(|c| c.to_ascii_lowercase()).collect();
            assert_eq!(m, "rho");
        }
        assert!(substr_char_ranges(&chars, "xyz").is_empty());
        assert!(substr_char_ranges(&chars, "").is_empty());
    }

    #[test]
    fn new_reader_starts_with_hot_reload_on_and_otherwise_default() {
        let r = PdfReaderState::new();
        assert!(r.hot_reload.is_enabled());
        // `new()` only overrides hot-reload.
        assert!(r.path.is_empty());
        assert!(r.annotations.is_empty());
        assert!(r.search.query.is_empty());
    }

    /// op-q1qj (GH issue #35 2026-09-01 05:37, "project root isn't
    /// decided"): with an active paper's `PaperSession` supplied,
    /// `save_annotations_into_project` must write straight into its
    /// canonical Markdown via `append_block`/`save_document` — no
    /// `project_root`/`project_markdown_rel` needed at all. GH issue #35
    /// 2026-09-02: the block is now a real `annotation` artifact, the
    /// in-memory overlay is dropped, and the shared editor is reloaded.
    #[test]
    fn save_annotations_writes_into_the_active_papers_session_when_given_one() {
        use crate::entity::Access;
        use crate::ingest::{self, IngestChoice};
        use crate::root::{KovanRoot, RootConfig};
        use crate::session::PaperSession;

        // Same minimal, structurally valid one-page PDF fixture as
        // `ingest.rs`'s own tests (`write_test_pdf`) — `ingest::preview`
        // needs a real parseable PDF, not just any bytes.
        fn write_test_pdf(path: &std::path::Path, title: &str) {
            use lopdf::{dictionary, Document, Object};
            let mut doc = Document::with_version("1.5");
            let pages_id = doc.new_object_id();
            let page_id = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id });
            let pages = dictionary! { "Type" => "Pages", "Kids" => vec![Object::Reference(page_id)], "Count" => 1 };
            doc.objects.insert(pages_id, Object::Dictionary(pages));
            let catalog_id =
                doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
            doc.trailer.set("Root", catalog_id);
            let info_id = doc.add_object(dictionary! { "Title" => Object::string_literal(title) });
            doc.trailer.set("Info", info_id);
            doc.save(path).unwrap();
        }

        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        let pdf_path = dir.path().join("incoming.pdf");
        write_test_pdf(&pdf_path, "A Test Paper About Figures");
        let preview = ingest::preview(&root, &pdf_path).unwrap();
        let citekey = preview.suggested_citekey.clone();
        ingest::ingest(
            &root,
            &preview,
            IngestChoice {
                citekey: citekey.clone(),
                access: Access::Open,
                topics: vec!["htgrs".into()],
                projects: vec![],
            },
        )
        .unwrap();

        let mut state = PdfReaderState::default();
        state.annotations.insert(
            0,
            vec![Annotation {
                min: Pos2::new(10.0, 20.0),
                max: Pos2::new(30.0, 40.0),
                text: "a note about figure 3".to_string(),
                created_at: "2026-09-01T00:00:00Z".to_string(),
                author: "tester".to_string(),
                page_px: [100.0, 200.0],
            }],
        );

        let mut session = PaperSession::open(&root, &citekey).unwrap();
        let mut editor = KvimEditorState::default();
        editor.load_text(session.markdown());
        state.save_annotations_into_project(Some(&mut session), &mut editor);

        assert!(
            state.message.contains(&citekey),
            "status should name the paper it saved into: {}",
            state.message
        );
        let on_disk = std::fs::read_to_string(root.paper_markdown(&citekey)).unwrap();
        assert!(
            on_disk.contains("a note about figure 3"),
            "annotation text should be in the saved markdown:\n{on_disk}"
        );
        assert!(
            on_disk.contains("kind = \"annotation\""),
            "should be a real fenced-TOML artifact:\n{on_disk}"
        );

        // The saved block re-parses as an artifact anchored to page 1.
        let index = crate::research_record::ResearchRecordIndex::from_session(&session);
        assert_eq!(index.anchored_to_page(1).len(), 1);
        // The in-memory overlay is dropped once persisted.
        assert!(state.annotations.get(&0).is_none_or(|v| v.is_empty()));
        // The shared editor now shows the new block.
        assert!(editor.text().contains("a note about figure 3"));
    }

    #[test]
    fn first_note_heading_line_finds_the_page_marker() {
        let md = "# Paper\n\n### annotation — x\n- page: 3\n\nnote three\n\n### annotation — y\n- page: 7\n\nnote seven\n";
        assert_eq!(first_note_heading_line(md, 2), Some(3)); // page 3 == page0 2
        assert_eq!(first_note_heading_line(md, 6), Some(8)); // page 7 heading is line 8
        assert_eq!(first_note_heading_line(md, 4), None);
    }

    #[test]
    fn open_artifact_on_a_text_block_loads_the_inline_editor() {
        let md = "# Graphite note\n\n```toml\n[kovan]\nid = \"graphite-note\"\nkind = \"annotation\"\ncreated = \"c\"\nmodified = \"m\"\n\n[source]\npage = 3\n```\n\nthe prose body\n";
        let doc = crate::artifact::parse_document(md);
        let art = doc.get("graphite-note").unwrap();

        let mut state = PdfReaderState::default();
        let crop = state.open_artifact(art);
        assert!(
            crop.is_none(),
            "a text block is edited in place, not sent to a digitiser"
        );
        assert_eq!(state.editing_block_id.as_deref(), Some("graphite-note"));
        assert_eq!(state.block_editor.text(), "the prose body");
        assert_eq!(PdfReaderState::artifact_page(art), Some(2));
    }

    #[test]
    fn normalise_region_normalises_and_rejects_degenerate() {
        let r = normalise_region(
            Pos2::new(50.0, 100.0),
            Pos2::new(150.0, 300.0),
            200.0,
            400.0,
        )
        .unwrap();
        assert!((r.x0 - 0.25).abs() < 1e-9 && (r.y1 - 0.75).abs() < 1e-9);
        assert!(
            normalise_region(Pos2::new(10.0, 10.0), Pos2::new(10.0, 10.0), 200.0, 400.0).is_none()
        );
        assert!(
            normalise_region(Pos2::new(10.0, 10.0), Pos2::new(20.0, 20.0), 0.0, 400.0).is_none()
        );
    }

    #[test]
    fn crop_provenance_region_uses_the_recorded_page_size() {
        let p = CropProvenance {
            page_index: 0,
            min: Pos2::new(20.0, 40.0),
            max: Pos2::new(120.0, 240.0),
            page_px: [200.0, 400.0],
            created_at: String::new(),
            author: String::new(),
            figure: String::new(),
            source_artifact_id: None,
        };
        let r = p.region().unwrap();
        assert!((r.x0 - 0.1).abs() < 1e-6 && (r.y1 - 0.6).abs() < 1e-6);

        let no_size = CropProvenance {
            page_px: [0.0, 0.0],
            ..p
        };
        assert!(no_size.region().is_none());
    }

    // -------------------------------------------------------------------
    // Region -> screen-rect reconstruction (op-30um.5, GH issue #35
    // "layer 2" — every source-anchored artifact draws, not only
    // annotations). Pure-function tests: no `egui::Ui`, no `PageView`, no
    // window.
    // -------------------------------------------------------------------

    /// Builds an [`Artifact`] directly (bypassing [`crate::artifact::
    /// parse_document`]'s own anchor validation) so a test can exercise
    /// `artifact_overlays_for_page`'s own defensive checks against an
    /// anchor shape the parser would otherwise reject upfront.
    fn make_artifact(id: &str, kind: ArtifactKind, source: Option<SourceAnchor>) -> Artifact {
        Artifact {
            heading: id.to_string(),
            level: crate::artifact::ARTIFACT_LEVEL,
            line: 1,
            toml: crate::artifact::ArtifactToml {
                kovan: crate::artifact::ArtifactMeta {
                    id: id.to_string(),
                    kind,
                    created: "c".to_string(),
                    modified: "m".to_string(),
                    reviewed: None,
                },
                source,
                classification: Classification::default(),
                extraction: None,
                relation: None,
            connections: Vec::new(),
            },
            body: String::new(),
        }
    }

    #[test]
    fn region_to_screen_rect_round_trips_through_normalise_region() {
        // Pixel rect -> `normalise_region` -> `Region` -> back through
        // `region_to_screen_rect` at zoom 1.0 / origin (0,0) / page 0 must
        // reproduce the original pixel rect.
        let page_px = egui::vec2(200.0, 400.0);
        let min = Pos2::new(50.0, 100.0);
        let max = Pos2::new(150.0, 300.0);
        let region = normalise_region(min, max, page_px.x, page_px.y).unwrap();
        let rect = region_to_screen_rect(region, 0, page_px, Pos2::ZERO, 1.0, 16.0).unwrap();
        assert!((rect.min.x - min.x).abs() < 1e-3, "{rect:?}");
        assert!((rect.min.y - min.y).abs() < 1e-3, "{rect:?}");
        assert!((rect.max.x - max.x).abs() < 1e-3, "{rect:?}");
        assert!((rect.max.y - max.y).abs() < 1e-3, "{rect:?}");
    }

    #[test]
    fn region_to_screen_rect_stacks_a_later_page_below_the_first() {
        // The same full-page region on page 2 (0-based) must land
        // `page_top` further down the continuous canvas — the exact
        // stacking `PageView::project` does, reproduced with no `PageView`.
        let page_px = egui::vec2(200.0, 400.0);
        let region = Region {
            x0: 0.0,
            y0: 0.0,
            x1: 1.0,
            y1: 1.0,
        };
        let (zoom, gap) = (1.0_f32, 16.0_f32);
        let rect0 = region_to_screen_rect(region, 0, page_px, Pos2::ZERO, zoom, gap).unwrap();
        let rect2 = region_to_screen_rect(region, 2, page_px, Pos2::ZERO, zoom, gap).unwrap();
        assert_eq!(rect0.min.y, 0.0);
        let expected_top = 2.0 * (page_px.y * zoom + gap);
        assert!((rect2.min.y - expected_top).abs() < 1e-3, "{rect2:?}");
    }

    #[test]
    fn region_to_screen_rect_rejects_a_degenerate_region() {
        let zero_width = Region {
            x0: 0.5,
            y0: 0.2,
            x1: 0.5,
            y1: 0.9,
        };
        assert!(!zero_width.is_valid());
        assert!(region_to_screen_rect(
            zero_width,
            0,
            egui::vec2(200.0, 400.0),
            Pos2::ZERO,
            1.0,
            16.0
        )
        .is_none());
    }

    #[test]
    fn region_to_screen_rect_rejects_a_degenerate_page_size() {
        let region = Region {
            x0: 0.1,
            y0: 0.1,
            x1: 0.9,
            y1: 0.9,
        };
        assert!(region_to_screen_rect(
            region,
            0,
            egui::vec2(0.0, 400.0),
            Pos2::ZERO,
            1.0,
            16.0
        )
        .is_none());
    }

    #[test]
    fn artifact_overlays_for_page_reconstructs_an_annotation_and_a_digitised_graph() {
        // Mirrors the layer-2 prototype's own acceptance check
        // (`prototype_artifact_overlays.py`'s "PASS: overlay reconstruction
        // is artifact-generic, not annotation-only"): both an existing
        // Annotation and a DigitisedGraph+CSV artifact must reconstruct as
        // PDF rectangles, from real parsed Markdown+TOML data.
        let md = r#"
# Graphite temperature assumption

```toml
[kovan]
id = "graphite-temperature-assumption"
kind = "annotation"
created = "2026-08-31T15:04:32+08:00"
modified = "2026-08-31T15:04:32+08:00"

[source]
page = 3
region = [0.214, 0.341, 0.721, 0.508]
```

Graphite temperature here appears to represent nominal operating conditions.

# Fig. 12 — power vs time

```toml
[kovan]
id = "fig-12-power-vs-time"
kind = "digitised_graph"
created = "2026-08-31T15:10:00+08:00"
modified = "2026-08-31T15:10:00+08:00"

[source]
page = 3
region = [0.1, 0.1, 0.9, 0.6]

[extraction]
method = "manual_digitisation"
```

```csv
t_s,power_mw
0,10
1,12
```
"#;
        let doc = crate::artifact::parse_document(md);
        assert!(doc.problems.is_empty(), "{:?}", doc.problems);
        assert_eq!(doc.artifacts.len(), 2);

        let page_px = egui::vec2(600.0, 800.0);
        let (zoom, gap) = (1.0_f32, 16.0_f32);
        // page 3 in the artifact (1-based) is index 2 (0-based).
        let overlays = artifact_overlays_for_page(&doc.artifacts, 2, page_px, Pos2::ZERO, zoom, gap);
        assert_eq!(
            overlays.len(),
            2,
            "{:?}",
            overlays.iter().map(|(a, _)| a.id()).collect::<Vec<_>>()
        );

        let annot = overlays
            .iter()
            .find(|(a, _)| a.kind() == ArtifactKind::Annotation)
            .expect("annotation overlay");
        let graph = overlays
            .iter()
            .find(|(a, _)| a.kind() == ArtifactKind::DigitisedGraph)
            .expect("digitised-graph overlay");
        assert!(
            graph.0.csv_block().is_some(),
            "the graph artifact carries its CSV payload — the CSV is not a separate node"
        );

        let page_top = 2.0 * (page_px.y * zoom + gap);
        let expected_annot_y0 = page_top + 0.341_f32 * page_px.y;
        let expected_graph_y0 = page_top + 0.1_f32 * page_px.y;
        assert!(
            (annot.1.min.y - expected_annot_y0).abs() < 1.0,
            "{:?} vs {expected_annot_y0}",
            annot.1
        );
        assert!(
            (graph.1.min.y - expected_graph_y0).abs() < 1.0,
            "{:?} vs {expected_graph_y0}",
            graph.1
        );
    }

    #[test]
    fn artifact_overlays_for_page_excludes_artifacts_anchored_to_a_different_page() {
        let md = "# Note\n\n```toml\n[kovan]\nid = \"note-1\"\nkind = \"note\"\ncreated = \"c\"\nmodified = \"m\"\n\n[source]\npage = 5\nregion = [0.1, 0.1, 0.9, 0.9]\n```\n";
        let doc = crate::artifact::parse_document(md);
        assert_eq!(doc.artifacts.len(), 1, "{:?}", doc.problems);

        let page_px = egui::vec2(600.0, 800.0);
        // Anchored to page 5 (1-based) == index 4 (0-based). A different
        // page currently in view must not draw it.
        let elsewhere =
            artifact_overlays_for_page(&doc.artifacts, 0, page_px, Pos2::ZERO, 1.0, 16.0);
        assert!(elsewhere.is_empty());
        let here = artifact_overlays_for_page(&doc.artifacts, 4, page_px, Pos2::ZERO, 1.0, 16.0);
        assert_eq!(here.len(), 1);
    }

    #[test]
    fn artifact_overlays_for_page_skips_a_degenerate_region_without_panicking() {
        // `parse_document` itself rejects an invalid region as a
        // `BadAnchor` problem before it ever becomes an `Artifact` (see
        // `crate::artifact`'s own tests) — this exercises
        // `artifact_overlays_for_page`'s own defensive check directly.
        let bad = make_artifact(
            "bad-region",
            ArtifactKind::Note,
            Some(SourceAnchor {
                page: Some(3),
                pages: None,
                region: Some(Region {
                    x0: 0.5,
                    y0: 0.2,
                    x1: 0.5,
                    y1: 0.9,
                }),
            }),
        );
        let overlays = artifact_overlays_for_page(
            std::slice::from_ref(&bad),
            2,
            egui::vec2(600.0, 800.0),
            Pos2::ZERO,
            1.0,
            16.0,
        );
        assert!(overlays.is_empty());
    }

    #[test]
    fn artifact_overlays_for_page_never_boxes_a_pages_range_anchor() {
        // §15: a `pages = [start, end]` anchor cannot also carry a
        // `region` — so a range-anchored artifact is never boxable,
        // regardless of how many pages it spans or which of those pages
        // is currently in view.
        let ranged = make_artifact(
            "spans-42-to-48",
            ArtifactKind::SourceReference,
            Some(SourceAnchor {
                page: None,
                pages: Some([42, 48]),
                region: None,
            }),
        );
        let page_px = egui::vec2(600.0, 800.0);
        // 0-based index of its own first page (42 - 1 = 41) — even asking
        // for exactly that page yields nothing, since there is no region.
        let overlays = artifact_overlays_for_page(
            std::slice::from_ref(&ranged),
            41,
            page_px,
            Pos2::ZERO,
            1.0,
            16.0,
        );
        assert!(overlays.is_empty());
    }

    // -------------------------------------------------------------------
    // relation_other_end (op-30um.3 — the "Edit connections…"/"Delete
    // connection…" list's direction logic).
    // -------------------------------------------------------------------

    #[test]
    fn relation_other_end_points_forward_when_node_is_the_source() {
        let rel = crate::relation::UserRelation {
            id: "r1".to_string(),
            source: "artifact:src#a".to_string(),
            target: "artifact:dst#b".to_string(),
            kind: crate::relation::RelationKind::Supports,
        };
        let (arrow, other) = relation_other_end(&rel, "artifact:src#a");
        assert_eq!(arrow, "→");
        assert_eq!(other, "artifact:dst#b");
    }

    #[test]
    fn relation_other_end_points_backward_when_node_is_the_target() {
        let rel = crate::relation::UserRelation {
            id: "r1".to_string(),
            source: "artifact:src#a".to_string(),
            target: "artifact:dst#b".to_string(),
            kind: crate::relation::RelationKind::Supports,
        };
        let (arrow, other) = relation_other_end(&rel, "artifact:dst#b");
        assert_eq!(arrow, "←");
        assert_eq!(other, "artifact:src#a");
    }

    // -------------------------------------------------------------------
    // saved_artifact_menu_entries (op-30um.3/.6 — the pure, window-free
    // menu-composition function the right-click menu renders verbatim).
    // -------------------------------------------------------------------

    /// Every [`ArtifactKind`] must get the identical five-entry shape
    /// (op-30um.6: "keep the connection and delete verbs identical across
    /// kinds") with only the first entry's label varying, and the
    /// connection separator/verbs/order fixed regardless of kind.
    #[test]
    fn saved_artifact_menu_entries_has_the_same_shape_for_every_kind() {
        for kind in [
            ArtifactKind::Note,
            ArtifactKind::Annotation,
            ArtifactKind::SourceReference,
            ArtifactKind::Formula,
            ArtifactKind::DigitisedTable,
            ArtifactKind::DigitisedGraph,
        ] {
            let entries = saved_artifact_menu_entries(kind, true, true);
            let actions: Vec<MenuAction> = entries.iter().map(|e| e.action).collect();
            // The connection and delete verbs are byte-identical across
            // every kind (op-30um.6); only the leading navigation entries
            // vary, and only by whether the kind carries CSV.
            let is_csv = matches!(
                kind,
                ArtifactKind::DigitisedTable | ArtifactKind::DigitisedGraph
            );
            let mut expected = vec![MenuAction::GoToPage];
            if is_csv {
                expected.push(MenuAction::GoToDigitiser);
            }
            expected.extend([
                MenuAction::EditArtifact,
                MenuAction::AddConnection,
                MenuAction::EditConnections,
                MenuAction::DeleteConnection,
                MenuAction::DeleteArtifact,
            ]);
            assert_eq!(actions, expected, "{kind:?}");
            assert_eq!(entries.len(), if is_csv { 7 } else { 6 }, "{kind:?}");
            // Exactly one entry can ever invoke the delete cascade — a
            // menu structurally cannot dispatch it twice from one click.
            assert_eq!(
                actions.iter().filter(|a| **a == MenuAction::DeleteArtifact).count(),
                1,
                "{kind:?}"
            );
            assert_eq!(entries.last().unwrap().label, "Delete annotation…", "{kind:?}");
            // Grouped as [navigation] | [edit] | [connections] | [delete]:
            // a separator closing each group, none elsewhere.
            let separators: Vec<bool> = entries.iter().map(|e| e.separator_after).collect();
            let mut expected_seps = if is_csv {
                vec![false, true]
            } else {
                vec![true]
            };
            expected_seps.extend([true, false, false, true, false]);
            assert_eq!(separators, expected_seps, "{kind:?}");
        }
    }

    #[test]
    fn a_second_right_click_on_the_same_artifact_closes_the_menu() {
        // Maintainer, GH issue #35 (2026-09-08): "box should disappear
        // after a second right click".
        let mut state = PdfReaderState::default();
        let at = Pos2::new(10.0, 20.0);
        let a = ContextMenuTarget::SavedArtifact("fig-1".to_string());

        state.toggle_context_menu(at, a.clone());
        assert!(state.context_menu.is_some(), "first right-click opens it");

        state.toggle_context_menu(at, a.clone());
        assert!(state.context_menu.is_none(), "second right-click closes it");

        // ...and a third opens it again, so the gesture is a true toggle.
        state.toggle_context_menu(at, a.clone());
        assert!(state.context_menu.is_some());
    }

    #[test]
    fn right_clicking_a_different_artifact_moves_the_menu_rather_than_closing() {
        let mut state = PdfReaderState::default();
        state.toggle_context_menu(
            Pos2::new(1.0, 1.0),
            ContextMenuTarget::SavedArtifact("fig-1".to_string()),
        );
        state.toggle_context_menu(
            Pos2::new(9.0, 9.0),
            ContextMenuTarget::SavedArtifact("table-2".to_string()),
        );
        let menu = state.context_menu.as_ref().expect("menu moved, not closed");
        assert_eq!(
            menu.target,
            ContextMenuTarget::SavedArtifact("table-2".to_string())
        );
        assert_eq!(menu.screen_pos, Pos2::new(9.0, 9.0));
    }

    /// The one entry that does vary by kind: its label, matching the
    /// layer-2 prototype's wording
    /// (`collaboration/kovan-issue-35-prototypes/layer2-artifact-overlays/prototype_artifact_context_menu.py`).
    #[test]
    fn saved_artifact_menu_entries_edit_label_is_kind_specific() {
        // The edit entry sits after the navigation group, so find it by
        // action rather than by position.
        let label_for = |kind| {
            saved_artifact_menu_entries(kind, true, true)
                .into_iter()
                .find(|e| e.action == MenuAction::EditArtifact)
                .expect("an edit entry")
                .label
        };
        assert_eq!(label_for(ArtifactKind::Note), "Edit annotation");
        assert_eq!(label_for(ArtifactKind::Annotation), "Edit annotation");
        assert_eq!(label_for(ArtifactKind::SourceReference), "Edit source reference");
        assert_eq!(label_for(ArtifactKind::Formula), "Edit formula");
        assert_eq!(label_for(ArtifactKind::DigitisedTable), "Edit table");
        assert_eq!(label_for(ArtifactKind::DigitisedGraph), "Edit digitisation");
    }

    /// Without a `(KovanRoot, KnowledgeIndex)` pair, the three connection
    /// entries are disabled (never hidden — same convention the citation
    /// completion popup already uses); Edit and Delete stay enabled since
    /// neither touches the relation store.
    #[test]
    fn saved_artifact_menu_entries_disables_connection_actions_without_a_library() {
        let entries = saved_artifact_menu_entries(ArtifactKind::Annotation, false, true);
        let enabled: Vec<bool> = entries.iter().map(|e| e.enabled).collect();
        // Go to page, Edit, [three connection entries disabled], Delete.
        assert_eq!(enabled, vec![true, true, false, false, false, true]);
    }

    /// The composition function itself is pure data assembly — calling it
    /// (even for a `DeleteArtifact`-bearing menu) can never mutate a
    /// library, matching op-30um.3's "no graph-walking, no relation
    /// deletion, and no partial writes in the UI" constraint. This is the
    /// pdf_reader-side half of "a cancelled delete mutates nothing": the
    /// menu that *offers* delete has no way to perform it just by being
    /// built.
    #[test]
    fn building_the_menu_never_touches_the_library_a_cancelled_delete_mutates_nothing() {
        use crate::entity::Access;
        use crate::research_record::ResearchRecordIndex;
        use crate::root::RootConfig;

        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        crate::entity::EntityConfig::paper(crate::entity::CiteKey::parse("src").unwrap(), Access::Open)
            .save_paper(&root.paper_dir("src"))
            .unwrap();
        let mut session = PaperSession::open(&root, "src").unwrap();
        let index = ResearchRecordIndex::from_session(&session);
        classify::insert_artifact(
            &mut session,
            &index,
            "A Note",
            ArtifactKind::Note,
            None,
            Classification::default(),
            None,
            "body",
        )
        .unwrap();
        session.save_document().unwrap();
        let before = std::fs::read_to_string(root.paper_markdown("src")).unwrap();

        // Building the menu — including its `DeleteArtifact` entry — is
        // the entire UI-side effect of a right-click. Nothing about
        // constructing it touches the filesystem.
        let _entries = saved_artifact_menu_entries(ArtifactKind::Note, true, true);

        let after = std::fs::read_to_string(root.paper_markdown("src")).unwrap();
        assert_eq!(before, after, "composing the menu must not touch the paper's file");
    }

    /// A separate check, at the domain level `saved_artifact_menu_entries`
    /// itself has no access to: the "No" path really does call nothing.
    /// `ConnectionPopup::ConfirmDelete` is a plain enum value — holding one
    /// (as the popup does while its dialog is open) has no effect on its
    /// own; only picking "Yes" (a distinct code path in
    /// `connection_popup_ui`, calling
    /// [`classify::delete_artifact_cascade`]) does. Constructing and then
    /// dropping the popup value here stands in for the whole "No"/dismiss
    /// interaction.
    #[test]
    fn holding_a_confirm_delete_popup_without_choosing_yes_calls_nothing() {
        let popup = ConnectionPopup::ConfirmDelete {
            citekey: "src".to_string(),
            artifact_id: "note-a".to_string(),
        };
        // Dropped here, unchosen — same as the dialog's "No" button, which
        // maps to `self.connection_popup = None` and nothing else.
        drop(popup);
    }

    /// The `DeleteArtifact` entry's real effect, at the same fidelity as
    /// [`classify`]'s own cascade tests: exactly one call to
    /// `classify::delete_artifact_cascade` — the same call
    /// `ConnectionPopup::ConfirmDelete`'s "Yes" button makes — removes the
    /// artifact, and calling it a second time on the same id errors
    /// instead of silently no-op'ing, which is what "invoking the cascade
    /// exactly once" means operationally: a second click cannot find
    /// anything left to delete.
    #[test]
    fn delete_artifact_entry_invokes_the_cascade_exactly_once() {
        use crate::entity::Access;
        use crate::research_record::ResearchRecordIndex;
        use crate::root::RootConfig;

        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        crate::entity::EntityConfig::paper(crate::entity::CiteKey::parse("src").unwrap(), Access::Open)
            .save_paper(&root.paper_dir("src"))
            .unwrap();
        let mut session = PaperSession::open(&root, "src").unwrap();
        let index = ResearchRecordIndex::from_session(&session);
        let artifact = classify::insert_artifact(
            &mut session,
            &index,
            "A Note",
            ArtifactKind::Note,
            None,
            Classification::default(),
            None,
            "body",
        )
        .unwrap();
        session.save_document().unwrap();
        let index = KnowledgeIndex::rebuild(&root);
        let artifact_id = artifact.id().to_string();

        let entries = saved_artifact_menu_entries(ArtifactKind::Note, true, true);
        assert!(matches!(
            entries.last().unwrap().action,
            MenuAction::DeleteArtifact
        ));

        // First invocation — the "Yes" branch's exact call — succeeds.
        let removed =
            classify::delete_artifact_cascade(&root, &index, "src", &artifact_id).unwrap();
        assert_eq!(removed, 0);

        // A second invocation on the same id (what a stray double-dispatch
        // would look like) finds nothing left and errors rather than
        // silently repeating the deletion.
        let err =
            classify::delete_artifact_cascade(&root, &index, "src", &artifact_id).unwrap_err();
        assert!(matches!(err, classify::CascadeError::ArtifactNotFound { .. }));
    }
}
