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
//! **Edit**/**Delete**. [`AnnotationTool`] is `None` (pan/zoom, and
//! right-click an existing box), `DrawBox` (drag to propose a new box), or
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

use std::collections::HashMap;

use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, Sense, Stroke};
use kopitiam_pdf::gui_frontend::{HotReload, PdfReader, PdfReaderConfig, ReloadDecision, RELOAD_CHECK_INTERVAL};
use kopitiam_pdf::mupdf::{page_to_stext, rasterize_page, PdfDocument, StextBlock, StextOptions, StextPage};

use crate::artifact::{block_span, Artifact, ArtifactKind, Region, SourceAnchor};
use crate::classify;
use crate::digitiser::dataset::utc_now_iso8601;
use crate::digitiser::raster::PlotRaster;
use crate::entity::Classification;
use crate::project;
use crate::session::PaperSession;

use super::csv_preview::draw_csv_preview;
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
    /// No drawing — pan/zoom, and right-click on an existing box.
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
#[derive(Debug, Clone, Copy)]
enum ContextMenuTarget {
    /// The just-drawn, not-yet-confirmed box in `pending_box`.
    NewBox,
    /// An already-saved annotation, by index into that page's `Vec` in
    /// `annotations`.
    Existing(usize),
}

/// A floating right-click menu (op-x9qn), positioned at the click's screen
/// coordinates. `Copy` so it can be read out of `self` by value without a
/// borrow fight against the `&mut self` methods its buttons call.
#[derive(Debug, Clone, Copy)]
struct ContextMenu {
    screen_pos: Pos2,
    target: ContextMenuTarget,
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
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let r = Region {
        x0: (min.x.min(max.x) / w) as f64,
        y0: (min.y.min(max.y) / h) as f64,
        x1: (min.x.max(max.x) / w) as f64,
        y1: (min.y.max(max.y) / h) as f64,
    };
    r.is_valid().then_some(r)
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
        let StextBlock::Text(tb) = block else { continue };
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
            (Pos2::new(x0 * scale, y0 * scale), Pos2::new(x1 * scale, y1 * scale))
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
        Self { hot_reload: HotReload::new(true), show_thumbs: true, ..Self::default() }
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
        let ReaderSource::Pdf(reader) = &mut self.source else { return };
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
                    self.annotate_page = self.annotate_page.min(reader.page_count().saturating_sub(1));
                    // The document changed under us — every cached page
                    // raster, structured-text page and search hit is stale.
                    self.pages.clear();
                    self.stext_cache = None;
                    self.search.computed_for.clear();
                    self.message = format!("{} changed on disk — reloaded", self.path);
                }
                Err(e) => self.message = format!("{} changed on disk, but reload failed: {e}", self.path),
            },
            Err(e) => self.message = format!("{} changed on disk, but could not be re-read: {e}", self.path),
        }
    }

    /// Generate a BibTeX entry for the currently open PDF (op-x3wl: "I want
    /// the pdf I'm reading to generate a bibtex entry I can copy and
    /// paste"). Reuses `kovan_literature::extract_metadata` +
    /// `to_bibtex` — the exact same pipeline `kovan-cli lit bibtex` already
    /// runs — rather than a second implementation.
    fn generate_bibtex(&mut self) {
        let ReaderSource::Pdf(_) = &self.source else {
            self.bibtex = Some(Err("no PDF open (BibTeX needs a PDF, not a plain image)".into()));
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
    fn crop_region_of_page(&self, page: usize, region: Region) -> Result<(PlotRaster, Pos2, Pos2), String> {
        let ReaderSource::Pdf(reader) = &self.source else {
            return Err("no PDF open".to_string());
        };
        let pixmap =
            rasterize_page(reader.document(), page, RENDER_DPI).map_err(|e| format!("page render failed: {e}"))?;
        let (pw, ph, stride, n) = (pixmap.w, pixmap.h, pixmap.stride, pixmap.n as usize);
        let samples = pixmap.samples;
        let min = Pos2::new(region.x0 as f32 * pw as f32, region.y0 as f32 * ph as f32);
        let max = Pos2::new(region.x1 as f32 * pw as f32, region.y1 as f32 * ph as f32);
        let min_x = min.x.max(0.0) as u32;
        let min_y = min.y.max(0.0) as u32;
        let w = ((max.x - min.x).max(1.0) as u32).min(pw.saturating_sub(min_x)).max(1);
        let h = ((max.y - min.y).max(1.0) as u32).min(ph.saturating_sub(min_y)).max(1);
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
        if let Some(p) = Self::artifact_page(artifact) {
            self.annotate_page = p;
            self.scroll_request = Some(p);
            self.thumb_synced = None;
        }
        match artifact.kind() {
            ArtifactKind::DigitisedTable | ArtifactKind::DigitisedGraph => self.recrop_artifact(artifact),
            _ => {
                self.block_editor.load_text(&artifact.body);
                self.editing_block_id = Some(artifact.id().to_string());
                None
            }
        }
    }

    /// The 0-based page an artifact's `[source]` anchor starts on, if any.
    fn artifact_page(artifact: &Artifact) -> Option<usize> {
        artifact.toml.source.as_ref().and_then(|s| s.first_page()).map(|p| p.saturating_sub(1) as usize)
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
                ui.add(egui::TextEdit::singleline(&mut prompt.figure).hint_text("e.g. \"Figure 4\" or \"Fig. 4.2\""));
            });
            ui.horizontal(|ui| {
                let can_confirm = !prompt.figure.trim().is_empty();
                if ui.add_enabled(can_confirm, egui::Button::new("Continue")).clicked() {
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
                    let snippet: String =
                        ann.text.split_whitespace().take(8).collect::<Vec<_>>().join(" ");
                    let heading = if snippet.is_empty() {
                        format!("Annotation (p{})", pg + 1)
                    } else {
                        format!("Annotation (p{}) — {snippet}", pg + 1)
                    };
                    let anchor =
                        SourceAnchor { page: Some((pg + 1) as u32), pages: None, region: ann.region() };
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
                    self.message = format!("saved {count} annotation(s) into {}", session.citekey());
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
                    if let Some(a) =
                        first_id.and_then(|id| crate::artifact::parse_document(&md).get(&id).cloned())
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
        let Some(doc) = self.current_pdf_document() else { return };
        if needle.is_empty() {
            return;
        }
        let scale = RENDER_DPI / 72.0;
        let pages = self.source.page_count();
        let mut hits = Vec::new();
        for page in 0..pages {
            let Ok(stext) = page_to_stext(doc, page, StextOptions::default()) else { continue };
            for block in &stext.blocks {
                let StextBlock::Text(tb) = block else { continue };
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

        egui::ScrollArea::vertical().id_salt("pdf_thumb_strip").show_viewport(ui, |ui, viewport| {
            let last_page = pages.saturating_sub(1);
            let first = ((viewport.min.y / row_h).floor().max(0.0) as usize).min(last_page);
            let last = ((viewport.max.y / row_h).floor().max(0.0) as usize).min(last_page);
            if let ReaderSource::Pdf(reader) = &self.source {
                self.pages.ensure_thumbs(ui.ctx(), reader.document(), first..=last);
            }

            let (rect, _) = ui.allocate_exact_size(egui::vec2(width, pages as f32 * row_h), Sense::hover());
            let painter = ui.painter_at(rect);
            let row_of = |p: usize| Rect::from_min_size(Pos2::new(rect.min.x, rect.min.y + p as f32 * row_h), egui::vec2(width, row_h));

            if follow {
                ui.scroll_to_rect(row_of(current), Some(egui::Align::Center));
            }

            for p in first..=last {
                let row = row_of(p);
                let resp = ui.interact(row, ui.id().with(("kovan-thumb", p)), Sense::click());
                if p == current {
                    painter.rect_filled(row, 3.0, Color32::from_rgba_unmultiplied(120, 170, 255, 60));
                } else if resp.hovered() {
                    painter.rect_filled(row, 3.0, Color32::from_rgba_unmultiplied(150, 150, 150, 30));
                }
                let img = Rect::from_min_size(Pos2::new(row.min.x + 7.0, row.min.y + 3.0), egui::vec2(img_w, img_h));
                match self.pages.thumb(p) {
                    Some(tex) => {
                        painter.image(tex.id(), img, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                    }
                    None => {
                        painter.rect_filled(img, 0.0, Color32::from_gray(235));
                    }
                }
                painter.rect_stroke(
                    img,
                    0.0,
                    Stroke::new(if p == current { 2.0 } else { 1.0 }, Color32::from_gray(150)),
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
    /// **Double-clicking** a card, or a banded block in the preview, is the
    /// only edit path: a text/annotation/formula/source-reference block
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
        let anchored: Vec<&Artifact> =
            artifacts.iter().filter(|a| a.toml.source.as_ref().is_some_and(|s| s.covers_page(page))).collect();

        let editor_text = context_editor.text();
        // op-j178: band every anchored block in the preview, and (op-4x5s)
        // a stronger band on whichever block's box is hovered on the canvas.
        context_editor.set_anchor_bands(anchored.iter().map(|a| block_span(&editor_text, a)).collect());
        let hovered_block = self
            .hover_created_at
            .as_deref()
            .and_then(|h| anchored.iter().find(|a| a.toml.kovan.created == h || a.id() == h));
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

        egui::ScrollArea::vertical().id_salt("pdf_context_panel_scroll").show(ui, |ui| {
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
                let inner = egui::Frame::new().fill(fill).inner_margin(4.0).show(ui, |ui| {
                    match artifact.kind() {
                        ArtifactKind::DigitisedTable | ArtifactKind::DigitisedGraph => {
                            let icon = if artifact.kind() == ArtifactKind::DigitisedTable {
                                "\u{1F4CA}"
                            } else {
                                "\u{1F4C8}"
                            };
                            ui.label(format!("{icon} {}", artifact.heading));
                            if let Some(csv) = artifact.csv_block() {
                                draw_csv_preview(ui, csv);
                            }
                            ui.small("double-click → re-open in the digitiser");
                        }
                        _ => {
                            ui.label(format!("\u{1F4DD} {}", artifact.heading));
                            if !artifact.body.trim().is_empty() {
                                ui.monospace(body_preview(&artifact.body));
                            }
                            ui.small("double-click → edit");
                        }
                    }
                });
                if inner.response.hovered() {
                    panel_hover = Some(id.clone());
                }
                if inner.response.double_clicked() {
                    open_target = Some(id.clone());
                }
                ui.separator();
            }

            ui.add_space(8.0);
            ui.strong("Preview (read-only)");
            if let Some(line) = context_editor.ui_readonly(ui) {
                if let Some(a) = artifacts.iter().find(|a| block_span(&editor_text, a).contains(&line)) {
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
        let path = std::path::Path::new(self.project_root.trim()).join(self.project_markdown_rel.trim());
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                ui.colored_label(Color32::from_rgb(230, 90, 90), format!("{}: {e}", path.display()));
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
        egui::ScrollArea::vertical().id_salt("pdf_context_panel_scroll").show(ui, |ui| {
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
        let active_artifacts: Option<Vec<Artifact>> = active_paper
            .as_ref()
            .map(|s| crate::research_record::ResearchRecordIndex::from_session(s).artifacts().to_vec());

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
                ui.label(format!("page {} / {}", self.annotate_page + 1, self.source.page_count()));
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
            ui.selectable_value(&mut self.tool, AnnotationTool::None, "Select");
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
        if let Some(off) = self.forced_offset.take() {
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
                    self.pages.ensure(ui.ctx(), reader.document(), want.clone(), render_scale);
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
                    keys_free && (i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals)),
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
                    self.forced_offset =
                        Some(egui::vec2((new_x - within.x).max(0.0), (new_y - within.y).max(0.0)));
                }
            }

            if let Some(target) = self.scroll_request.take() {
                let y = origin.y + self.pages.page_top(target, zoom, GAP);
                ui.scroll_to_rect(
                    Rect::from_min_size(Pos2::new(origin.x, y), egui::vec2(1.0, 10.0)),
                    Some(egui::Align::TOP),
                );
            }

            // Paint each visible page (a grey placeholder while it renders).
            for p in want.clone() {
                let pr = self.pages.page_rect(p, origin, zoom, GAP);
                if let Some(tex) = self.pages.texture(p) {
                    painter.image(tex.id(), pr, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
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
                let centre_page = ((viewport.center().y / stride).floor().max(0.0) as usize).min(n - 1);
                self.annotate_page = centre_page;
            }
            // A *gesture* is the one thing that should re-point the page at
            // the pointer — you draw/right-click on the page under the mouse,
            // whichever that is.
            if !busy && (response.drag_started() || response.secondary_clicked()) {
                if let Some((p, _)) = response.interact_pointer_pos().and_then(|s| self.pages.hit(s, origin, n, zoom, GAP)) {
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
                if let (Some(start), Some(pos)) =
                    (self.draw_start, response.interact_pointer_pos())
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
                        if click.x >= min.x && click.x <= max.x && click.y >= min.y && click.y <= max.y {
                            self.context_menu =
                                Some(ContextMenu { screen_pos, target: ContextMenuTarget::NewBox });
                        }
                    } else if let Some(i) = self
                        .annotations
                        .get(&page)
                        .and_then(|anns| anns.iter().position(|a| a.contains(click)))
                    {
                        self.context_menu =
                            Some(ContextMenu { screen_pos, target: ContextMenuTarget::Existing(i) });
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
                self.annotations.get(&page).and_then(|anns| anns.iter().position(|a| a.contains(p)))
            });
            let mut hover_id: Option<String> = hovered.and_then(|i| {
                self.annotations.get(&page).and_then(|anns| anns.get(i)).map(|a| a.created_at.clone())
            });

            // In-memory (not-yet-saved) annotation boxes — amber — on every
            // visible page.
            for p in want.clone() {
                if let Some(anns) = self.annotations.get(&p) {
                    for (i, ann) in anns.iter().enumerate() {
                        let hot = p == page && hovered == Some(i);
                        let r = box_rect(p, ann.min, ann.max);
                        painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(255, 230, 60, if hot { 110 } else { 60 }));
                        painter.rect_stroke(
                            r,
                            0.0,
                            Stroke::new(if hot { 2.5 } else { 1.0 }, Color32::from_rgb(230, 170, 20)),
                            egui::StrokeKind::Middle,
                        );
                    }
                }
            }

            // Saved-artifact region boxes — amber (annotation) / blue
            // (digitised) — on every visible page. Hover highlights the
            // panel card; a single click opens the artifact for editing.
            if let Some(artifacts) = active_artifacts.as_deref() {
                for art in artifacts {
                    let Some(pg) = Self::artifact_page(art) else { continue };
                    if !want.contains(&pg) {
                        continue;
                    }
                    let Some(region) = art.toml.source.as_ref().and_then(|s| s.region) else { continue };
                    let min = Pos2::new(region.x0 as f32 * page_px.x, region.y0 as f32 * page_px.y);
                    let max = Pos2::new(region.x1 as f32 * page_px.x, region.y1 as f32 * page_px.y);
                    let r = box_rect(pg, min, max);
                    let hit = hover_screen.is_some_and(|s| r.contains(s));
                    if hit {
                        hover_id = Some(art.toml.kovan.created.clone());
                        if opened {
                            open_target = Some(art.id().to_string());
                        }
                    }
                    let linked = hit || self.panel_hover_id.as_deref() == Some(art.id());
                    let is_annot = matches!(art.kind(), ArtifactKind::Annotation);
                    let (fill, stroke) = if is_annot {
                        (Color32::from_rgba_unmultiplied(255, 230, 60, if linked { 90 } else { 40 }), Color32::from_rgb(230, 170, 20))
                    } else {
                        (Color32::from_rgba_unmultiplied(120, 170, 255, if linked { 90 } else { 40 }), Color32::from_rgb(90, 140, 235))
                    };
                    painter.rect_filled(r, 0.0, fill);
                    painter.rect_stroke(r, 0.0, Stroke::new(if linked { 2.5 } else { 1.0 }, stroke), egui::StrokeKind::Middle);
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
                    Color32::from_rgba_unmultiplied(255, 210, 40, if is_current { 150 } else { 70 }),
                );
                if is_current {
                    painter.rect_stroke(r, 2.0, Stroke::new(2.0, Color32::from_rgb(210, 120, 0)), egui::StrokeKind::Outside);
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
        self.scroll_anchor = egui::vec2(
            (scroll_out.state.offset.x + self.last_viewport.x * 0.5) / content.x.max(1.0),
            (scroll_out.state.offset.y + self.last_viewport.y * 0.5) / stride.max(1.0),
        );
        self.last_zoom = zoom;

        // A double-click on a saved region box (GH issue #35 2026-09-02):
        // straight into editing it.
        if let Some(id) = open_target {
            if let Some(art) = active_artifacts.as_deref().and_then(|a| a.iter().find(|x| x.id() == id).cloned()) {
                if let Some(r) = self.open_artifact(&art) {
                    crop_result = Some(r);
                }
            }
        }

        if let Some(result) = self.context_menu_ui(ui.ctx()) {
            crop_result = Some(result);
        }

        crop_result
    }

    /// Draw the floating right-click menu (op-x9qn), if one is open.
    /// Returns `Some` the frame a Digitise-graph/Read-table crop is
    /// confirmed.
    fn context_menu_ui(&mut self, ctx: &egui::Context) -> Option<CropResult> {
        let menu = self.context_menu?;
        let mut close = false;
        let mut result = None;
        egui::Area::new(egui::Id::new("pdf_reader_context_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(menu.screen_pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(140.0);
                    match menu.target {
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
                                    self.pending_figure_prompt =
                                        Some(PendingFigurePrompt { min, max, figure: String::new() });
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
                            if ui.button("Edit").clicked() {
                                if let Some(a) =
                                    self.annotations.get(&self.active_page()).and_then(|a| a.get(i))
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
        if close {
            self.context_menu = None;
        }
        result
    }

    /// The last text selection (op-z9u0), if any — a read-only preview with
    /// Copy-to-clipboard and "Save as annotation" (folds the selection into
    /// the same `annotations` markdown section a hand-typed note goes into,
    /// per the module doc).
    fn text_selection_panel(&mut self, ui: &mut egui::Ui) {
        let Some((min, max, text)) = self.text_selection.clone() else { return };
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
                    self.annotations.entry(self.active_page()).or_default().push(Annotation {
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
        let Some(editor) = &mut self.annotate_editor else { return None };
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
            quad: Quad { ul: p, ur: p, ll: p, lr: p },
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
        let page = stub_page(vec![stub_line("line", PdfRect::new(0.0, 10.0, 200.0, 20.0))]);
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
        let text =
            select_text_in_rect(&page, 1.0, Pos2::new(1000.0, 1000.0), Pos2::new(1100.0, 1100.0));
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
        assert_eq!(hits.len(), 4, "two lower, one upper, and rhorho as two non-overlapping");
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
            let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
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
            IngestChoice { citekey: citekey.clone(), access: Access::Open, topics: vec!["htgrs".into()], projects: vec![] },
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

        assert!(state.message.contains(&citekey), "status should name the paper it saved into: {}", state.message);
        let on_disk = std::fs::read_to_string(root.paper_markdown(&citekey)).unwrap();
        assert!(on_disk.contains("a note about figure 3"), "annotation text should be in the saved markdown:\n{on_disk}");
        assert!(on_disk.contains("kind = \"annotation\""), "should be a real fenced-TOML artifact:\n{on_disk}");

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
        let md = "# P\n\n## Graphite note\n\n```toml\n[kovan]\nid = \"graphite-note\"\nkind = \"annotation\"\ncreated = \"c\"\nmodified = \"m\"\n\n[source]\npage = 3\n```\n\nthe prose body\n";
        let doc = crate::artifact::parse_document(md);
        let art = doc.get("graphite-note").unwrap();

        let mut state = PdfReaderState::default();
        let crop = state.open_artifact(art);
        assert!(crop.is_none(), "a text block is edited in place, not sent to a digitiser");
        assert_eq!(state.editing_block_id.as_deref(), Some("graphite-note"));
        assert_eq!(state.block_editor.text(), "the prose body");
        assert_eq!(PdfReaderState::artifact_page(art), Some(2));
    }

    #[test]
    fn normalise_region_normalises_and_rejects_degenerate() {
        let r = normalise_region(Pos2::new(50.0, 100.0), Pos2::new(150.0, 300.0), 200.0, 400.0).unwrap();
        assert!((r.x0 - 0.25).abs() < 1e-9 && (r.y1 - 0.75).abs() < 1e-9);
        assert!(normalise_region(Pos2::new(10.0, 10.0), Pos2::new(10.0, 10.0), 200.0, 400.0).is_none());
        assert!(normalise_region(Pos2::new(10.0, 10.0), Pos2::new(20.0, 20.0), 0.0, 400.0).is_none());
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

        let no_size = CropProvenance { page_px: [0.0, 0.0], ..p };
        assert!(no_size.region().is_none());
    }
}
