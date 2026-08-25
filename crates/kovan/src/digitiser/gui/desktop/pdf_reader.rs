//! Integrated PDF reader panel — GitHub issue #30's "don't want to
//! screenshot then digitise slowly, everything should be integrated into
//! the reader" (op-95x6), extended to view raster images directly,
//! Okular-style (op-wojr — "I want pdf reader to be able to view images
//! like okular as well").
//!
//! Renders PDF pages through `kopitiam_pdf::mupdf` — the PDF-page-rendering
//! engine decision this crate's `Cargo.toml` records (op-6ez3): open a PDF's
//! bytes with [`PdfDocument::open`], rasterize a page with [`rasterize_page`]
//! at a fixed screen DPI, upload the samples as an egui texture. A plain
//! image (PNG/JPEG) opens through the digitiser's own [`PlotRaster`] loader
//! instead — see [`ReaderSource`] — and is treated as a one-page document, so
//! the same viewer below serves both without knowing which it has.
//!
//! ## Two page-flow modes (op-veti, op-eehc)
//!
//! - **Single-page** (the original mode): one page rasterized and cached at
//!   a time, with Prev/Next navigation. Full tool interactivity (draw-box,
//!   select-text, annotations).
//! - **Continuous scroll** (op-veti, default on — GitHub issue #30: "scroll
//!   thru the pdfs in continuous mode"): every page stacked vertically in
//!   one scrollable flow, Okular-style. View-only — see
//!   [`PdfReaderState::continuous_pages_ui`]'s doc for why the interactive
//!   tools stay single-page-only. Either way, only the pages actually
//!   rasterized (single-page: the current one; continuous: whatever has
//!   scrolled into view) cost render time — a PDF is never pre-rendered in
//!   full.
//!
//! **Hot reload** (op-eehc, default on — GitHub issue #30: "hot reload by
//! default in case I compile live in tex or typst"): the open file's mtime
//! is polled (throttled, not filesystem-watched) and a change triggers an
//! automatic re-open, restoring the page the operator was looking at. See
//! [`PdfReaderState::check_hot_reload`].
//!
//! ## Unified box interaction model (op-x9qn, superseding op-gv19's first cut)
//!
//! GitHub issue #30's 2026-08-23 follow-up comment asked for one drawing
//! gesture rather than a toolbar of separate tools: draw a box anywhere on
//! the page, right-click it, and pick what it becomes —
//! **Annotate** (a free-text note), **Digitise graph**, or **Read table**.
//! Right-clicking an *already-saved* annotation box instead offers
//! **Edit**/**Delete**. This replaces op-gv19's original separate
//! Highlight/Note toolbar tools (a plain visual-only highlight with no text
//! is no longer a distinct case — every new box goes through the same
//! menu) while keeping op-p17q/op-hnhp's draw-box-then-crop mechanics for
//! the Digitise-graph/Read-table choices unchanged underneath.
//!
//! [`AnnotationTool`] is now just `None` (pan/zoom, and right-click an
//! existing box) or `DrawBox` (drag to propose a new box). [`ContextMenu`]
//! is the floating menu that appears on a right-click hit; [`Annotation`]
//! is a saved free-text note (page + pixel rect + author + timestamp);
//! [`CropProvenance`] carries the same provenance alongside a Digitise/Read
//! crop so the digitiser tab it hands off to can later save the CSV it
//! produces back into the project's markdown (op-96am) with the same page
//! and pixel bbox recorded on the note.
//!
//! **In-memory only, still** — same scoping note as before: nothing here
//! persists to disk on its own. Saving into a project's markdown (op-96am)
//! is a separate, explicit action taken from the tab a crop was handed to,
//! or (for a plain Annotate note) not yet wired to a "save into project"
//! button in this first pass — see op-96am's own bead for what remains.
//! Ink/freehand strokes are still not implemented: every box is
//! axis-aligned.
//!
//! ## Text selection (op-z9u0)
//!
//! A separate `Select text` tool drags out a rectangle and selects the
//! **lines** of real PDF text (not raw pixels) whose bounding box
//! intersects it, via `kopitiam_pdf::mupdf::page_to_stext` — MuPDF's
//! structured-text model (`StextPage`/`StextLine`/`StextChar`), ported with
//! real per-line device-space bounding boxes. Device space there is in PDF
//! *points* (unscaled, 72/inch); this panel's pixel space is points ×
//! `RENDER_DPI / 72.0` — the same scale [`rasterize_page`] itself applies —
//! so a line's bbox is converted once by that factor before hit-testing
//! against the drag rect. **Line granularity, not glyph/character
//! granularity** — a deliberate scope cut (see [`select_text_in_rect`]):
//! selecting *part* of a line is out of scope for this pass. The selected
//! text can be copied to the clipboard or saved as an [`Annotation`] over
//! the selection's bounding box, so a text selection and a hand-typed note
//! end up in the same place (op-96am's `annotations` markdown section).
//!
//! Does not belong here: PDF *text* extraction as a batch/whole-document
//! operation (that is `kovan_literature::extract_metadata`'s job, already
//! exposed via `kovan-cli lit`) — this panel's structured-text use is
//! strictly interactive, one page at a time, cached per page only for as
//! long as that page stays open.

use std::collections::HashMap;

use eframe::egui::{
    self, Color32, ColorImage, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions, Vec2,
};
use kopitiam_pdf::mupdf::{page_to_stext, rasterize_page, PdfDocument, StextBlock, StextOptions, StextPage};

use super::pdf_annots::{read_page_annotations, AnnotationKind, PdfAnnotation};
use crate::digitiser::dataset::utc_now_iso8601;
use crate::digitiser::raster::PlotRaster;
use crate::project;

/// Screen-resolution DPI for page rasterization — sharp enough to read body
/// text at a typical window size without the per-page render becoming slow.
/// The panel's zoom slider scales the *displayed* size, not this DPI, so
/// zooming in past 100% will show visible raster blur; re-rasterizing per
/// zoom level is future work if that turns out to matter in practice.
const RENDER_DPI: f32 = 150.0;

/// Low-resolution DPI used only for the page-thumbnail strip (op-0y4k) —
/// far cheaper per page than [`RENDER_DPI`] since a thumbnail is shown at a
/// few dozen pixels tall.
const THUMBNAIL_DPI: f32 = 36.0;

/// How often [`PdfReaderState::check_hot_reload`] is allowed to `stat` the
/// open file — frequent enough that a live TeX/Typst recompile is picked up
/// promptly, infrequent enough not to hammer the filesystem every frame.
const RELOAD_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

/// What is currently open in the reader — a multi-page PDF, or a single
/// directly-loaded raster image. Closed set, enum-dispatched per the
/// workspace's no-trait-objects rule, rather than an `Option<PdfDocument>`
/// plus a separate `Option<PlotRaster>` (which would let both or neither be
/// `Some` at once, a state this panel never actually wants).
#[derive(Default)]
enum ReaderSource {
    #[default]
    None,
    Pdf(PdfDocument),
    Image(PlotRaster),
}

impl ReaderSource {
    /// Total pages — a directly-loaded image is always exactly one "page".
    fn page_count(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Pdf(doc) => doc.page_count(),
            Self::Image(_) => 1,
        }
    }
}

/// Which annotation interaction is active. Closed set, enum-dispatched.
/// See the module doc's "Unified box interaction model" — there is no
/// longer a separate tool per box *kind*; what a box becomes is chosen from
/// the right-click menu after it's drawn, not from the toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum AnnotationTool {
    /// No drawing — page nav/zoom, and right-click on an existing box.
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
}

impl Annotation {
    fn contains(&self, p: Pos2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
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
    pub created_at: String,
    pub author: String,
}

/// A completed crop-then-right-click gesture (op-p17q / op-hnhp), returned
/// from [`PdfReaderState::ui`] the frame it happens.
pub enum CropResult {
    Plot(PlotRaster, CropProvenance),
    Table(PlotRaster, CropProvenance),
}

/// State for one open document: its source (PDF or image), which page is
/// showing, its cached rasterization, the zoom applied to the displayed
/// texture, and its annotations.
#[derive(Default)]
pub struct PdfReaderState {
    path: String,
    source: ReaderSource,
    page_index: usize,
    texture: Option<TextureHandle>,
    /// The page index the cached `texture` was rendered for, so a page flip
    /// or zoom-only change knows whether to re-rasterize.
    texture_page: usize,
    zoom: f32,
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
    /// Author name recorded on new annotations/crops (op-96am's provenance
    /// "author" field) — analogous to the digitiser's own "operator" field.
    author: String,
    /// "kovan folder" project (op-63u0) to save annotations into, and the
    /// markdown file (relative to that root) they belong to — see
    /// [`Self::save_annotations_into_project`]. Operator-supplied, same
    /// reasoning as the digitiser tabs' own `project_root`/
    /// `project_markdown_rel` fields.
    project_root: String,
    project_markdown_rel: String,
    /// Cached structured-text page (op-z9u0), for the currently displayed
    /// page only — re-extracted on page change, not kept for every page
    /// (unlike `thumbnails`, since a full-resolution `StextPage` is a lot
    /// more data per page than a thumbnail texture).
    stext_cache: Option<(usize, StextPage)>,
    /// Texture-pixel-space start corner of a text-selection drag in
    /// progress (op-z9u0).
    select_start: Option<Pos2>,
    /// The last completed text selection: its bounding rect (union of every
    /// selected line's bbox, texture-pixel space) and the concatenated text
    /// of the lines it covers, one per line. `None` before any selection.
    text_selection: Option<(Pos2, Pos2, String)>,
    /// Cached low-res page thumbnails (op-0y4k) — keyed by page index,
    /// cleared only when a new document is opened (kept across page/zoom
    /// changes within the same document, unlike the single full-resolution
    /// `texture`).
    thumbnails: HashMap<usize, TextureHandle>,
    /// The `created_at` of the annotation the pointer is currently hovering
    /// on the canvas, if any (op-4x5s) — a poor-man's stable id
    /// [`Self::context_panel`] uses to highlight the matching markdown
    /// block. One frame behind the canvas hover (see where it's set).
    hover_created_at: Option<String>,
    /// Whether the left page-thumbnail strip is collapsed — collapsible per
    /// GitHub issue #30's "a collapsible panel to select pages... like
    /// Okular" (op-0y4k). Only meaningful for a multi-page PDF. Named so
    /// the derived `Default` (`false`) means "shown", matching what a user
    /// opening a fresh multi-page PDF expects to see.
    hide_thumbnails: bool,
    /// Whether pages render as one continuous scrollable flow (op-veti,
    /// "scroll thru the pdfs in continuous mode") instead of one page at a
    /// time. View-only in this mode — see [`Self::continuous_pages_ui`]'s
    /// doc for the scope cut on draw-box/select-text/annotation tools.
    continuous_scroll: bool,
    /// Full-resolution page textures for continuous-scroll mode (op-veti),
    /// keyed by page index — separate from the single-page `texture`/
    /// `texture_page` cache, since continuous mode may have several pages'
    /// textures live at once. Only populated for pages that have actually
    /// scrolled into view (see [`Self::full_res_texture`]).
    page_textures: HashMap<usize, TextureHandle>,
    /// Whether the open file is watched for changes and auto-reloaded
    /// (op-eehc — "hot reload by default in case I compile live in tex or
    /// typst"). Polled, throttled to [`RELOAD_CHECK_INTERVAL`] — not
    /// filesystem-watched, to avoid a new dependency for a desktop-only,
    /// not-latency-critical feature.
    hot_reload: bool,
    /// A page the thumbnail strip asked the continuous-scroll view to jump
    /// to (op-veti) — consumed (and cleared) by
    /// [`Self::continuous_pages_ui`] via `egui`'s `scroll_to_rect`. Ignored
    /// in single-page mode, which just uses `page_index` directly.
    scroll_to_page: Option<usize>,
    /// The open file's mtime as of the last successful open/reload — a
    /// change from this is what [`Self::check_hot_reload`] reloads on.
    file_mtime: Option<std::time::SystemTime>,
    /// Wall-clock time of the last hot-reload mtime check, so it happens at
    /// most once per [`RELOAD_CHECK_INTERVAL`] rather than every frame.
    last_reload_check: Option<std::time::Instant>,
    /// The last "Generate BibTeX" result (op-x3wl) — `Ok` holds the
    /// rendered entry, `Err` a human-readable failure. `None` before the
    /// button has ever been pressed for the currently open PDF.
    bibtex: Option<Result<String, String>>,
    /// Cached PDF-native annotations (`/Annots` — highlights, notes, etc.
    /// written by another viewer such as Okular; see [`super::pdf_annots`]),
    /// keyed by page index — lightweight enough to keep every page computed
    /// so far, unlike [`Self::stext_cache`]'s single-page cache.
    native_annots: HashMap<usize, Vec<PdfAnnotation>>,
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

/// A point on `quad`'s bottom-ish edge at fraction `t` down from its top
/// edge (`0.0` = top, `1.0` = bottom), interpolated between the top and
/// bottom edge midpoints — used to place the `Underline`/`StrikeOut`/
/// `Squiggly` stroke line within its markup quad without assuming the quad
/// is axis-aligned (a rotated line of text has a rotated quad).
fn quad_edge_point(quad: &[Pos2; 4], t: f32, frac: f32) -> Pos2 {
    // `quad` corners are [top-left, top-right, bottom-right, bottom-left]
    // (see `read_quad_points`'s reordering). Interpolate the top and bottom
    // edges at `frac` along their length, then between those two at `t`.
    let top = quad[0] + (quad[1] - quad[0]) * frac;
    let bottom = quad[3] + (quad[2] - quad[3]) * frac;
    top + (bottom - top) * t
}

/// Paint every native PDF annotation (`/Annots` — see
/// [`super::pdf_annots`]) already mapped into texture-pixel space, through
/// `to_screen`. See that module's doc for why this is a geometry+colour
/// approximation, not the annotation's real `/AP` appearance stream.
/// Returns the hovered annotation's tooltip text (`Contents`/`author`), if
/// `hover_pos` (texture-pixel space) lands on one with something to show.
fn draw_native_annotations(
    painter: &egui::Painter,
    annots: &[PdfAnnotation],
    to_screen: impl Fn(Pos2) -> Pos2,
    hover_pos: Option<Pos2>,
) -> Option<(String, Pos2)> {
    let mut tooltip = None;
    for ann in annots {
        let color = ann.color.unwrap_or(Color32::from_rgb(230, 170, 20));
        let alpha = (ann.opacity * 255.0).round().clamp(0.0, 255.0) as u8;
        match &ann.kind {
            AnnotationKind::Highlight => {
                for quad in &ann.quads {
                    let pts: Vec<Pos2> = quad.iter().map(|p| to_screen(*p)).collect();
                    painter.add(egui::Shape::convex_polygon(
                        pts,
                        color.gamma_multiply_u8(alpha / 2),
                        Stroke::NONE,
                    ));
                }
            }
            AnnotationKind::Underline | AnnotationKind::StrikeOut | AnnotationKind::Squiggly => {
                let t = match ann.kind {
                    AnnotationKind::Underline => 0.95,
                    AnnotationKind::StrikeOut => 0.5,
                    _ => 0.7,
                };
                for quad in &ann.quads {
                    if matches!(ann.kind, AnnotationKind::Squiggly) {
                        let segs = 8;
                        let pts: Vec<Pos2> = (0..=segs)
                            .map(|i| {
                                let frac = i as f32 / segs as f32;
                                let jitter = if i % 2 == 0 { -0.06 } else { 0.06 };
                                to_screen(quad_edge_point(quad, t + jitter, frac))
                            })
                            .collect();
                        painter.add(egui::Shape::line(pts, Stroke::new(1.5_f32, color)));
                    } else {
                        let a = to_screen(quad_edge_point(quad, t, 0.0));
                        let b = to_screen(quad_edge_point(quad, t, 1.0));
                        painter.line_segment([a, b], Stroke::new(1.5_f32, color));
                    }
                }
            }
            AnnotationKind::Square => {
                let rect = Rect::from_min_max(to_screen(ann.rect.0), to_screen(ann.rect.1));
                if let Some(ic) = ann.interior_color {
                    painter.rect_filled(rect, 0.0, ic.gamma_multiply_u8(alpha));
                }
                painter.rect_stroke(rect, 0.0, Stroke::new(1.5_f32, color), egui::StrokeKind::Middle);
            }
            AnnotationKind::Circle => {
                let (min, max) = (to_screen(ann.rect.0), to_screen(ann.rect.1));
                let center = min.lerp(max, 0.5);
                let radius = Vec2::new((max.x - min.x) / 2.0, (max.y - min.y) / 2.0);
                let n = 32;
                let pts: Vec<Pos2> = (0..n)
                    .map(|i| {
                        let a = i as f32 / n as f32 * std::f32::consts::TAU;
                        Pos2::new(center.x + radius.x * a.cos(), center.y + radius.y * a.sin())
                    })
                    .collect();
                let fill = ann
                    .interior_color
                    .map(|ic| ic.gamma_multiply_u8(alpha))
                    .unwrap_or(Color32::TRANSPARENT);
                painter.add(egui::Shape::convex_polygon(pts, fill, Stroke::new(1.5_f32, color)));
            }
            AnnotationKind::Line | AnnotationKind::Ink => {
                for stroke in &ann.strokes {
                    let pts: Vec<Pos2> = stroke.iter().map(|p| to_screen(*p)).collect();
                    painter.add(egui::Shape::line(pts, Stroke::new(1.5_f32, color)));
                }
            }
            AnnotationKind::Note | AnnotationKind::FreeText => {
                let rect = Rect::from_min_max(to_screen(ann.rect.0), to_screen(ann.rect.1));
                if matches!(ann.kind, AnnotationKind::FreeText) {
                    painter.rect_stroke(rect, 0.0, Stroke::new(1.5_f32, color), egui::StrokeKind::Middle);
                    painter.text(
                        rect.min + Vec2::new(2.0, 2.0),
                        egui::Align2::LEFT_TOP,
                        &ann.contents,
                        egui::FontId::proportional(11.0),
                        Color32::BLACK,
                    );
                } else {
                    painter.circle_filled(rect.min, 6.0, color);
                    painter.circle_stroke(rect.min, 6.0, Stroke::new(1.0_f32, Color32::BLACK));
                }
            }
            AnnotationKind::Other(_) => {}
        }
        if let Some(hp) = hover_pos {
            let (min, max) = ann.rect;
            if hp.x >= min.x && hp.x <= max.x && hp.y >= min.y && hp.y <= max.y && !ann.contents.is_empty() {
                let text = if ann.author.is_empty() {
                    ann.contents.clone()
                } else {
                    format!("{}: {}", ann.author, ann.contents)
                };
                tooltip = Some((text, to_screen(ann.rect.0)));
            }
        }
    }
    tooltip
}

impl PdfReaderState {
    /// A fresh reader, with continuous-scroll and hot-reload both **on** by
    /// default — GitHub issue #30's explicit asks ("scroll thru the pdfs in
    /// continuous mode", "hot reload by default"). Everything else stays
    /// the ordinary derived-`Default` zero state; prefer this over
    /// `PdfReaderState::default()` for exactly those two fields' sake.
    pub fn new() -> Self {
        Self { continuous_scroll: true, hot_reload: true, ..Self::default() }
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
        self.page_index = 0;
        self.texture = None;
        self.zoom = if self.zoom > 0.0 { self.zoom } else { 1.0 };
        self.annotations.clear();
        self.draw_start = None;
        self.pending_box = None;
        self.context_menu = None;
        self.annotate_editor = None;
        self.bibtex = None;
        self.thumbnails.clear();
        self.page_textures.clear();
        self.stext_cache = None;
        self.select_start = None;
        self.text_selection = None;
        self.scroll_to_page = None;
        self.native_annots.clear();
    }

    /// The `path`'s current on-disk mtime, if it can be read — used to seed/
    /// refresh [`Self::file_mtime`] on every successful open (including a
    /// hot-reload's own re-open), so [`Self::check_hot_reload`] always
    /// compares against the version actually loaded.
    fn read_mtime(path: &str) -> Option<std::time::SystemTime> {
        std::fs::metadata(path).ok()?.modified().ok()
    }

    fn open_pdf(&mut self, path: &str) {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.message = format!("cannot read {path}: {e}");
                return;
            }
        };
        match PdfDocument::open(bytes) {
            Ok(doc) => {
                let page_count = doc.page_count();
                self.path = path.to_string();
                self.file_mtime = Self::read_mtime(path);
                self.source = ReaderSource::Pdf(doc);
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
                self.file_mtime = Self::read_mtime(path);
                self.source = ReaderSource::Image(raster);
                self.reset_interaction_state();
                self.message = format!("opened {path}");
            }
            Err(e) => self.message = format!("cannot open {path} as PDF or image: {e}"),
        }
    }

    /// Reload the open file if it changed on disk (op-eehc) — a live TeX/
    /// Typst compile rewrites the PDF in place, and the reader should pick
    /// that up without the operator having to re-open it by hand. No-op
    /// when [`Self::hot_reload`] is off, nothing is open, or fewer than
    /// [`RELOAD_CHECK_INTERVAL`] has passed since the last check.
    ///
    /// Requests a repaint after the check interval so the reload actually
    /// happens promptly even while the operator isn't touching the mouse/
    /// keyboard — egui otherwise only repaints on input, and a live compile
    /// producing a new file is not an egui input event.
    fn check_hot_reload(&mut self, ctx: &egui::Context) {
        if !self.hot_reload || self.path.is_empty() {
            return;
        }
        ctx.request_repaint_after(RELOAD_CHECK_INTERVAL);
        let now = std::time::Instant::now();
        if let Some(last) = self.last_reload_check {
            if now.duration_since(last) < RELOAD_CHECK_INTERVAL {
                return;
            }
        }
        self.last_reload_check = Some(now);
        let Some(mtime) = Self::read_mtime(&self.path) else { return };
        if self.file_mtime == Some(mtime) {
            return;
        }
        let path = self.path.clone();
        let keep_page = self.page_index;
        self.open(&path);
        // `open` resets to page 0 via `reset_interaction_state` — restore
        // the page the operator was looking at (clamped, in case the
        // recompile changed the page count), so a live-editing loop doesn't
        // yank the view back to the start on every save.
        self.page_index = keep_page.min(self.source.page_count().saturating_sub(1));
        self.message = format!("{} changed on disk — reloaded", self.path);
    }

    /// Generate a BibTeX entry for the currently open PDF (op-x3wl: "I want
    /// the pdf I'm reading to generate a bibtex entry I can copy and
    /// paste"). Reuses `kovan_literature::extract_metadata` +
    /// `to_bibtex` — the exact same pipeline `kovan-cli lit bibtex` already
    /// runs — rather than a second implementation. Runs synchronously on
    /// the UI thread: this matches every other action in this GUI (the
    /// digitiser's own Auto-trace is synchronous too), and `extract_metadata`
    /// measured well under a second for real reports (see the TUI Ingest
    /// tab's own docs) — a worker thread would be the right call if a very
    /// large scanned PDF ever makes this noticeably block, but that has not
    /// been observed and is not worth the added complexity pre-emptively.
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

    /// Crop the current page/image to `(min, max)` (texture-pixel space) and
    /// build a standalone [`PlotRaster`] from it — the hand-off to the plot
    /// digitiser (op-p17q) or table digitiser (op-hnhp). Re-rasterizes the
    /// current PDF page rather than caching the last `Pixmap` alongside the
    /// texture: simpler, and rasterization is already cheap enough per-page
    /// (see the module doc) that a second render for this one-time crop
    /// action isn't worth the extra cached-state bookkeeping.
    fn crop_current_page(&self, min: Pos2, max: Pos2) -> Result<PlotRaster, String> {
        let min_x = min.x.max(0.0) as u32;
        let min_y = min.y.max(0.0) as u32;
        let want_w = (max.x - min.x).max(1.0) as u32;
        let want_h = (max.y - min.y).max(1.0) as u32;
        match &self.source {
            ReaderSource::None => Err("nothing open".to_string()),
            ReaderSource::Pdf(doc) => {
                let pixmap = rasterize_page(doc, self.page_index, RENDER_DPI)
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

    fn author_name(&self) -> String {
        let t = self.author.trim();
        if t.is_empty() {
            "unnamed".to_string()
        } else {
            t.to_string()
        }
    }

    fn make_provenance(&self, min: Pos2, max: Pos2) -> CropProvenance {
        CropProvenance {
            page_index: self.page_index,
            min,
            max,
            created_at: utc_now_iso8601(),
            author: self.author_name(),
        }
    }

    /// Append every not-yet-saved annotation on the current page into the
    /// project's `annotations` section (op-96am), via
    /// [`crate::project::append_to_section`] — one `###` subsection per
    /// annotation, each stating author/page/pixel-bbox per the design doc's
    /// §4.1 shape. Saves the whole page's worth in one call rather than one
    /// call per annotation, so appending N annotations after opening a page
    /// full of them doesn't need N separate stale-range-checked writes.
    fn save_annotations_into_project(&mut self) {
        let Some(anns) = self.annotations.get(&self.page_index) else {
            self.message = "no annotations on this page to save".to_string();
            return;
        };
        if anns.is_empty() {
            self.message = "no annotations on this page to save".to_string();
            return;
        }
        if self.project_root.trim().is_empty() || self.project_markdown_rel.trim().is_empty() {
            self.message = "set the project root and markdown path first".to_string();
            return;
        }
        let mut block = String::new();
        for ann in anns {
            block.push_str(&format!(
                "### annotation — {}\n- author: {}\n- page: {}\n- pixel bbox: [{:.1}, {:.1}, {:.1}, {:.1}]\n\n{}\n\n",
                ann.created_at,
                ann.author,
                self.page_index + 1,
                ann.min.x,
                ann.min.y,
                ann.max.x,
                ann.max.y,
                ann.text
            ));
        }
        match project::append_to_section(
            std::path::Path::new(self.project_root.trim()),
            self.project_markdown_rel.trim(),
            "annotations",
            block.trim_end(),
        ) {
            Ok(_) => self.message = format!("saved {} annotation(s) into project markdown", anns.len()),
            Err(e) => self.message = e.to_string(),
        }
    }

    fn next_page(&mut self) {
        if self.page_index + 1 < self.source.page_count() {
            self.page_index += 1;
            self.pending_box = None;
            self.context_menu = None;
            self.text_selection = None;
        }
    }

    fn prev_page(&mut self) {
        let before = self.page_index;
        self.page_index = self.page_index.saturating_sub(1);
        if self.page_index != before {
            self.pending_box = None;
            self.context_menu = None;
            self.text_selection = None;
        }
    }

    /// Structured text for `page` (op-z9u0), cached only for the page it
    /// was last extracted for — see `stext_cache`'s doc. `None` for a
    /// directly-loaded image (there is no PDF text layer to extract) or an
    /// extraction failure.
    fn stext_for_page(&mut self, page: usize) -> Option<&StextPage> {
        if !matches!(self.stext_cache, Some((p, _)) if p == page) {
            let ReaderSource::Pdf(doc) = &self.source else { return None };
            let stext = page_to_stext(doc, page, StextOptions::default()).ok()?;
            self.stext_cache = Some((page, stext));
        }
        self.stext_cache.as_ref().map(|(_, s)| s)
    }

    /// This page's PDF-native annotations (`/Annots` — see
    /// [`super::pdf_annots`]), reading and caching them on first access. An
    /// image source (no `/Annots` to have) always reports none.
    fn native_annotations_for_page(&mut self, page: usize) -> &[PdfAnnotation] {
        if !self.native_annots.contains_key(&page) {
            let annots = match &self.source {
                ReaderSource::Pdf(doc) => read_page_annotations(doc, page, RENDER_DPI),
                ReaderSource::None | ReaderSource::Image(_) => Vec::new(),
            };
            self.native_annots.insert(page, annots);
        }
        self.native_annots.get(&page).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Rasterize/convert the current page and upload it as a texture, if not
    /// already cached for this page.
    fn ensure_texture(&mut self, ctx: &egui::Context) {
        if self.texture.is_some() && self.texture_page == self.page_index {
            return;
        }
        match &self.source {
            ReaderSource::None => {}
            ReaderSource::Pdf(doc) => match rasterize_page(doc, self.page_index, RENDER_DPI) {
                Ok(pixmap) => {
                    let (w, h) = (pixmap.w as usize, pixmap.h as usize);
                    let image = if pixmap.alpha {
                        ColorImage::from_rgba_unmultiplied([w, h], &pixmap.samples)
                    } else {
                        ColorImage::from_rgb([w, h], &pixmap.samples)
                    };
                    self.texture = Some(ctx.load_texture(
                        format!("pdf-page-{}", self.page_index),
                        image,
                        TextureOptions::LINEAR,
                    ));
                    self.texture_page = self.page_index;
                }
                Err(e) => {
                    self.message = format!("page {} render failed: {e}", self.page_index + 1);
                    self.texture = None;
                }
            },
            ReaderSource::Image(raster) => {
                let image = raster_to_color_image(raster);
                self.texture =
                    Some(ctx.load_texture("reader-image", image, TextureOptions::LINEAR));
                self.texture_page = self.page_index;
            }
        }
    }

    /// Rasterize page `page` at [`THUMBNAIL_DPI`] and cache the texture
    /// (op-0y4k), returning it. `None` for a directly-loaded image (which
    /// has no "other pages" to thumbnail) or a render failure — either way
    /// the thumbnail strip simply shows nothing for that slot rather than
    /// erroring the whole panel.
    fn thumbnail_texture(&mut self, ctx: &egui::Context, page: usize) -> Option<TextureHandle> {
        if let Some(t) = self.thumbnails.get(&page) {
            return Some(t.clone());
        }
        let ReaderSource::Pdf(doc) = &self.source else { return None };
        let pixmap = rasterize_page(doc, page, THUMBNAIL_DPI).ok()?;
        let (w, h) = (pixmap.w as usize, pixmap.h as usize);
        let image = if pixmap.alpha {
            ColorImage::from_rgba_unmultiplied([w, h], &pixmap.samples)
        } else {
            ColorImage::from_rgb([w, h], &pixmap.samples)
        };
        let tex = ctx.load_texture(format!("pdf-thumb-{page}"), image, TextureOptions::LINEAR);
        self.thumbnails.insert(page, tex.clone());
        Some(tex)
    }

    /// The left page-thumbnail strip (op-0y4k) — an Okular-style page
    /// picker for a multi-page PDF. Uses `show_rows` so only the thumbnails
    /// actually scrolled into view are rasterized/uploaded, rather than
    /// every page in the document up front.
    fn thumbnail_strip(&mut self, ui: &mut egui::Ui) {
        let page_count = self.source.page_count();
        let row_height = 96.0;
        egui::ScrollArea::vertical().id_salt("pdf_thumbnails").show_rows(
            ui,
            row_height,
            page_count,
            |ui, range| {
                for page in range {
                    let selected = page == self.page_index;
                    let frame = egui::Frame::new().inner_margin(4.0).fill(if selected {
                        Color32::from_rgb(60, 90, 140)
                    } else {
                        Color32::TRANSPARENT
                    });
                    let resp = frame.show(ui, |ui| {
                        ui.set_height(row_height - 8.0);
                        ui.vertical_centered(|ui| {
                            if let Some(tex) = self.thumbnail_texture(ui.ctx(), page) {
                                let aspect = tex.size_vec2().y / tex.size_vec2().x.max(1.0);
                                let w = 72.0_f32;
                                ui.add(
                                    egui::Image::new(&tex)
                                        .fit_to_exact_size(Vec2::new(w, w * aspect)),
                                );
                            }
                            ui.label(format!("{}", page + 1));
                        });
                    });
                    if ui.interact(resp.response.rect, ui.id().with(("thumb", page)), Sense::click())
                        .clicked()
                    {
                        self.page_index = page;
                        self.scroll_to_page = Some(page);
                        self.pending_box = None;
                        self.context_menu = None;
                    }
                }
            },
        );
    }

    /// Rasterize page `page` at [`RENDER_DPI`] for continuous-scroll mode
    /// (op-veti) and cache the texture in [`Self::page_textures`] — the
    /// full-resolution counterpart to [`Self::thumbnail_texture`], keyed
    /// the same way but never evicted (a document opened for continuous
    /// reading is expected to have every visited page's texture live for
    /// as long as it stays open, same growth-without-eviction precedent
    /// `thumbnails` already sets in this file).
    fn full_res_texture(&mut self, ctx: &egui::Context, page: usize) -> Option<TextureHandle> {
        if let Some(t) = self.page_textures.get(&page) {
            return Some(t.clone());
        }
        let ReaderSource::Pdf(doc) = &self.source else { return None };
        let pixmap = rasterize_page(doc, page, RENDER_DPI).ok()?;
        let (w, h) = (pixmap.w as usize, pixmap.h as usize);
        let image = if pixmap.alpha {
            ColorImage::from_rgba_unmultiplied([w, h], &pixmap.samples)
        } else {
            ColorImage::from_rgb([w, h], &pixmap.samples)
        };
        let tex = ctx.load_texture(format!("pdf-page-full-{page}"), image, TextureOptions::LINEAR);
        self.page_textures.insert(page, tex.clone());
        Some(tex)
    }

    /// Continuous-scroll page flow (op-veti, "scroll thru the pdfs in
    /// continuous mode"): every page stacked vertically in one scrollable
    /// canvas, Okular-style, instead of navigating one page at a time.
    ///
    /// **View-only for this pass** — draw-box/select-text/annotation tools
    /// stay single-page-mode-only; switching back to single-page mode
    /// (opens at whichever page was last most-visible here, see below)
    /// re-enables full interactivity. Existing saved annotations are still
    /// shown, read-only, as a visual reference. This is a deliberate scope
    /// cut, not an oversight: re-deriving "which page, and where on it" a
    /// click/drag landed across a whole scrolling document is materially
    /// more work than the single-page coordinate math the tools already
    /// use, and the issue's own ask here is about *reading*, not digitising.
    ///
    /// Each page's **layout size** comes from its already-cached thumbnail
    /// texture ([`Self::thumbnail_texture`], cheap, [`THUMBNAIL_DPI`])
    /// scaled up to [`RENDER_DPI`] — this avoids a full rasterization just
    /// to learn a page's dimensions. The actual full-resolution texture
    /// ([`Self::full_res_texture`]) is only rasterized for pages that
    /// intersect the viewport; an off-screen page gets a plain placeholder
    /// rectangle instead, so opening a long document stays cheap and only
    /// the pages actually scrolled past cost render time (same principle
    /// the module doc states for single-page mode).
    ///
    /// Updates `self.page_index` to whichever page has the most vertical
    /// overlap with the viewport each frame, so the toolbar's "page N / M"
    /// readout (and single-page mode, if the operator switches back) track
    /// where the operator has actually scrolled to.
    fn continuous_pages_ui(&mut self, ui: &mut egui::Ui) {
        let page_count = self.source.page_count();
        if page_count == 0 {
            return;
        }
        let zoom = self.zoom;
        let scale = RENDER_DPI / THUMBNAIL_DPI;
        let spacing = 12.0_f32;

        let mut sizes = Vec::with_capacity(page_count);
        let mut max_w = 1.0_f32;
        let mut total_h = 0.0_f32;
        for p in 0..page_count {
            let size = self
                .thumbnail_texture(ui.ctx(), p)
                .map(|t| t.size_vec2() * scale * zoom)
                .unwrap_or(Vec2::new(612.0, 792.0) * (RENDER_DPI / 72.0) * zoom);
            max_w = max_w.max(size.x);
            total_h += size.y;
            sizes.push(size);
        }
        total_h += spacing * page_count.saturating_sub(1) as f32;

        egui::ScrollArea::both().id_salt("pdf_continuous_scroll").show(ui, |ui| {
            let (rect, _response) =
                ui.allocate_exact_size(Vec2::new(max_w, total_h), Sense::hover());
            let painter = ui.painter_at(rect);
            let viewport = ui.clip_rect();
            let mut y = rect.min.y;
            let mut most_visible: Option<(usize, f32)> = None;
            for (p, size) in sizes.iter().enumerate() {
                let page_rect = Rect::from_min_size(Pos2::new(rect.min.x, y), *size);
                if self.scroll_to_page == Some(p) {
                    ui.scroll_to_rect(page_rect, Some(egui::Align::TOP));
                    self.scroll_to_page = None;
                }
                if page_rect.intersects(viewport) {
                    if let Some(tex) = self.full_res_texture(ui.ctx(), p) {
                        painter.image(
                            tex.id(),
                            page_rect,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    } else {
                        painter.rect_filled(page_rect, 0.0, Color32::from_gray(235));
                    }
                    // PDF-native annotations (`/Annots`) draw underneath
                    // kovan's own, view-only here like everything else in
                    // continuous mode — no hover tooltip.
                    let page_annots = self.native_annotations_for_page(p).to_vec();
                    draw_native_annotations(
                        &painter,
                        &page_annots,
                        |pt: Pos2| page_rect.min + pt.to_vec2() * zoom,
                        None,
                    );
                    if let Some(anns) = self.annotations.get(&p) {
                        for ann in anns {
                            let min = page_rect.min + Vec2::new(ann.min.x, ann.min.y) * zoom;
                            let max = page_rect.min + Vec2::new(ann.max.x, ann.max.y) * zoom;
                            painter.rect_stroke(
                                Rect::from_min_max(min, max),
                                0.0,
                                Stroke::new(1.0_f32, Color32::from_rgb(230, 170, 20)),
                                egui::StrokeKind::Middle,
                            );
                        }
                    }
                    let overlap = page_rect.intersect(viewport).height().max(0.0);
                    if most_visible.is_none_or(|(_, best)| overlap > best) {
                        most_visible = Some((p, overlap));
                    }
                } else {
                    painter.rect_filled(page_rect, 0.0, Color32::from_gray(235));
                }
                y += size.y + spacing;
            }
            if let Some((p, _)) = most_visible {
                self.page_index = p;
            }
        });
    }

    /// The right "page context" panel (op-0y4k): raw text preview of
    /// whatever the currently open project's markdown records for the
    /// *currently displayed page* — annotations and digitised CSVs, read
    /// live off disk (not cached), matching GitHub issue #30's "live from
    /// markdown file" ask. Filters `### ...` subsections by a `page: N`/
    /// `page N,` marker, matching the exact provenance text this panel's
    /// own save actions emit (`Self::save_annotations_into_project`,
    /// `DigitiseApp::save_into_project`, `TableDigitiserState::
    /// save_into_project`) — a plain substring filter, not a markdown
    /// parser, since the marker text is under this crate's own control.
    fn context_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Page context");
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
                ui.colored_label(Color32::from_rgb(230, 90, 90), format!("{}: {e}", path.display()));
                return;
            }
        };
        let marker_a = format!("page: {}", self.page_index + 1);
        let marker_b = format!("page {},", self.page_index + 1);
        let blocks = blocks_matching(&text, &[&marker_a, &marker_b]);
        if blocks.is_empty() {
            ui.small("nothing saved for this page yet");
            return;
        }
        egui::ScrollArea::vertical().id_salt("pdf_context_panel_scroll").show(ui, |ui| {
            for block in blocks {
                // op-4x5s: highlight the block matching whatever annotation
                // box the pointer was hovering over the PDF canvas, one
                // frame ago (see `hover_created_at`'s doc).
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

    /// Draw the toolbar (page nav, zoom) and the page image. `on_open_clicked`
    /// is called when the user asks to open a different document — the
    /// caller owns the file dialog (shared with the digitiser's "Load image"
    /// action) and reports the chosen path back via [`PdfReaderState::open`].
    ///
    /// Returns `Some` the frame the user completes a crop-then-right-click
    /// gesture (op-p17q / op-hnhp) — the caller (`DigitiseApp`) is expected
    /// to load it into the matching digitiser tab and switch views.
    pub fn ui(&mut self, ui: &mut egui::Ui, mut on_open_clicked: impl FnMut()) -> Option<CropResult> {
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

        let page_count = self.source.page_count();
        let is_pdf = matches!(self.source, ReaderSource::Pdf(_));
        ui.horizontal(|ui| {
            if is_pdf {
                if self.continuous_scroll {
                    // Prev/Next are single-page-mode navigation; in
                    // continuous mode the operator scrolls instead, and
                    // `page_index` already tracks scroll position (see
                    // `continuous_pages_ui`), so it's shown read-only here.
                    ui.label(format!("page {} / {} (scrolling)", self.page_index + 1, page_count));
                } else {
                    if ui.button("< Prev").clicked() {
                        self.prev_page();
                    }
                    ui.label(format!("page {} / {}", self.page_index + 1, page_count));
                    if ui.button("Next >").clicked() {
                        self.next_page();
                    }
                }
                ui.separator();
            }
            ui.add(egui::Slider::new(&mut self.zoom, 0.25..=4.0).text("zoom"));
            if is_pdf {
                ui.separator();
                ui.checkbox(&mut self.continuous_scroll, "Continuous scroll");
                ui.checkbox(&mut self.hot_reload, "Hot reload");
            }
            if is_pdf && page_count > 1 {
                ui.separator();
                let label = if self.hide_thumbnails { "Show pages" } else { "Hide pages" };
                if ui.button(label).clicked() {
                    self.hide_thumbnails = !self.hide_thumbnails;
                }
            }
            ui.separator();
            ui.label("tool:");
            ui.selectable_value(&mut self.tool, AnnotationTool::None, "Select");
            ui.selectable_value(&mut self.tool, AnnotationTool::DrawBox, "Draw box");
            if is_pdf {
                ui.selectable_value(&mut self.tool, AnnotationTool::SelectText, "Select text");
            }
            if ui.button("Clear page annotations").clicked() {
                self.annotations.remove(&self.page_index);
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
        ui.small(
            "Draw box → right-click it → Annotate / Digitise graph / Read table. \
             Right-click an existing box → Edit / Delete.",
        );
        ui.horizontal(|ui| {
            ui.label("project root");
            ui.text_edit_singleline(&mut self.project_root);
            ui.label("markdown path");
            ui.text_edit_singleline(&mut self.project_markdown_rel);
            if ui.button("Save page annotations into project markdown").clicked() {
                self.save_annotations_into_project();
            }
        });
        self.text_selection_panel(ui);
        let mut crop_result = self.annotate_editor_panel(ui);
        self.bibtex_panel(ui);
        ui.separator();

        // 3-pane layout (op-0y4k): left page-thumbnail strip, right live
        // page-context panel, centre the PDF/image viewer below (whatever
        // of `ui`'s rect the two side panels didn't claim) — GitHub issue
        // #30's "On the left, a collapsible panel to select pages... In the
        // centre, pdf (65%)... In the right panel, raw text preview csv
        // tables and annotations corresponding to the page of pdf
        // displayed, live from markdown file."
        if is_pdf && page_count > 1 && !self.hide_thumbnails {
            egui::Panel::left("pdf_reader_thumbnails")
                .resizable(true)
                .default_size(110.0)
                .show_inside(ui, |ui| self.thumbnail_strip(ui));
        }
        egui::Panel::right("pdf_reader_context")
            .resizable(true)
            .default_size(280.0)
            .show_inside(ui, |ui| self.context_panel(ui));

        if is_pdf && self.continuous_scroll {
            self.continuous_pages_ui(ui);
            return crop_result; // continuous mode is view-only — see its own doc
        }

        self.ensure_texture(ui.ctx());
        let Some(texture) = self.texture.clone() else {
            if !self.message.is_empty() {
                ui.label(&self.message);
            }
            return None;
        };
        let size = texture.size_vec2() * self.zoom;
        let zoom = self.zoom;
        let page_index = self.page_index;
        let native_annots = self.native_annotations_for_page(page_index).to_vec();
        // Free scrolling in both directions (op-gv19's Okular-style ask) —
        // the whole page/image pans under a fixed viewport.
        egui::ScrollArea::both().show(ui, |ui| {
            let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
            let painter = ui.painter_at(rect);
            painter.image(
                texture.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );

            let to_image = move |pos: Pos2| -> Pos2 { ((pos - rect.min) / zoom).to_pos2() };
            let to_screen = move |p: Pos2| -> Pos2 { rect.min + p.to_vec2() * zoom };

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
                            .stext_for_page(page_index)
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
                        .get(&page_index)
                        .and_then(|anns| anns.iter().position(|a| a.contains(click)))
                    {
                        self.context_menu =
                            Some(ContextMenu { screen_pos, target: ContextMenuTarget::Existing(i) });
                    }
                }
            }

            // --- overlays: saved annotations (hover-highlighted), then the
            // pending box ---
            let hovered = response
                .hover_pos()
                .map(to_image)
                .and_then(|p| {
                    self.annotations
                        .get(&page_index)
                        .and_then(|anns| anns.iter().position(|a| a.contains(p)))
                });
            // op-4x5s: remember which annotation (by its `created_at`,
            // acting as a stable id) is hovered this frame, so the right
            // panel — drawn earlier in the same `ui()` call, on the
            // *previous* frame's value, one frame of lag being
            // imperceptible at interactive frame rates — can highlight the
            // matching markdown block too.
            self.hover_created_at = hovered.and_then(|i| {
                self.annotations
                    .get(&page_index)
                    .and_then(|anns| anns.get(i))
                    .map(|a| a.created_at.clone())
            });
            // PDF-native annotations (`/Annots` — Okular highlights/notes
            // and the like) draw underneath kovan's own annotation boxes.
            let native_tooltip = draw_native_annotations(
                &painter,
                &native_annots,
                to_screen,
                response.hover_pos().map(to_image),
            );
            if let Some(anns) = self.annotations.get(&page_index) {
                for (i, ann) in anns.iter().enumerate() {
                    let is_hovered = hovered == Some(i);
                    painter.rect_filled(
                        Rect::from_min_max(to_screen(ann.min), to_screen(ann.max)),
                        0.0,
                        Color32::from_rgba_unmultiplied(255, 230, 60, if is_hovered { 110 } else { 60 }),
                    );
                    painter.rect_stroke(
                        Rect::from_min_max(to_screen(ann.min), to_screen(ann.max)),
                        0.0,
                        Stroke::new(
                            if is_hovered { 2.5_f32 } else { 1.0_f32 },
                            Color32::from_rgb(230, 170, 20),
                        ),
                        egui::StrokeKind::Middle,
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
            if let Some((text, anchor)) = native_tooltip {
                let font = egui::FontId::proportional(12.0);
                let galley = painter.layout_no_wrap(text, font, Color32::BLACK);
                let box_rect =
                    Rect::from_min_size(anchor - Vec2::new(0.0, galley.size().y + 6.0), galley.size() + Vec2::new(8.0, 6.0));
                painter.rect_filled(box_rect, 3.0, Color32::from_rgba_unmultiplied(255, 250, 205, 235));
                painter.rect_stroke(box_rect, 3.0, Stroke::new(1.0_f32, Color32::from_gray(120)), egui::StrokeKind::Middle);
                painter.galley(box_rect.min + Vec2::new(4.0, 3.0), galley, Color32::BLACK);
            }
        });

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
                                if let Some((min, max)) = self.pending_box.take() {
                                    match self.crop_current_page(min, max) {
                                        Ok(raster) => {
                                            result = Some(CropResult::Plot(
                                                raster,
                                                self.make_provenance(min, max),
                                            ));
                                        }
                                        Err(e) => self.message = format!("crop failed: {e}"),
                                    }
                                }
                                close = true;
                            }
                            if ui.button("Read table").clicked() {
                                if let Some((min, max)) = self.pending_box.take() {
                                    match self.crop_current_page(min, max) {
                                        Ok(raster) => {
                                            result = Some(CropResult::Table(
                                                raster,
                                                self.make_provenance(min, max),
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
                                    self.annotations.get(&self.page_index).and_then(|a| a.get(i))
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
                                if let Some(anns) = self.annotations.get_mut(&self.page_index) {
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
                self.page_index + 1,
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
                    self.annotations.entry(self.page_index).or_default().push(Annotation {
                        min,
                        max,
                        text: text.clone(),
                        created_at: utc_now_iso8601(),
                        author,
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
        let Some(editor) = &mut self.annotate_editor else { return None };
        let mut save = false;
        let mut cancel = false;
        ui.group(|ui| {
            ui.label(format!(
                "Annotate — page {} — bbox [{:.0}, {:.0}, {:.0}, {:.0}]",
                self.page_index + 1,
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
            let anns = self.annotations.entry(self.page_index).or_default();
            match editor.editing_existing {
                Some(i) if i < anns.len() => {
                    anns[i].text = editor.text;
                    anns[i].min = editor.min;
                    anns[i].max = editor.max;
                }
                _ => anns.push(Annotation {
                    min: editor.min,
                    max: editor.max,
                    text: editor.text,
                    created_at: utc_now_iso8601(),
                    author,
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

    // --- hot reload (op-eehc) ---

    #[test]
    fn read_mtime_returns_none_for_a_missing_file() {
        assert!(PdfReaderState::read_mtime("/nonexistent/path/does/not/exist.pdf").is_none());
    }

    #[test]
    fn read_mtime_returns_some_for_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.txt");
        std::fs::write(&path, b"hello").unwrap();
        assert!(PdfReaderState::read_mtime(path.to_str().unwrap()).is_some());
    }

    #[test]
    fn new_reader_has_continuous_scroll_and_hot_reload_on_by_default() {
        let r = PdfReaderState::new();
        assert!(r.continuous_scroll);
        assert!(r.hot_reload);
        // Everything else should still be the plain derived-Default zero
        // state -- `new()` only overrides those two fields.
        assert!(r.path.is_empty());
        assert!(r.thumbnails.is_empty());
    }
}
