//! Integrated PDF reader panel — GitHub issue #30's "don't want to
//! screenshot then digitise slowly, everything should be integrated into
//! the reader" (op-95x6), extended to view raster images directly,
//! Okular-style (op-wojr — "I want pdf reader to be able to view images
//! like okular as well").
//!
//! ## Two modes, split along a real capability boundary (op-9vo6.11)
//!
//! As of kopitiam-pdf 0.3.2 (kopitiam#96), the fast, well-behaved PDF
//! *reading* engine — continuous scroll, background rendering/caching,
//! PageUp/PageDown, arrows, vim keys (`j`/`k`/`gg`/`G`/Ctrl-d/u), `/`/`?`/`n`/
//! `N` search, thumbnails — is no longer kovan's own hand-rolled code. It is
//! [`kopitiam_pdf::gui_frontend::PdfReader`], embedded read-only
//! ([`PdfReaderConfig::read_only`]) via [`PdfReader::show`]. This is
//! **[`ViewMode::Read`]**, the default for a PDF, and it is what fixes the
//! dogfooding defects `op-9vo6.11` was filed over: lag on long theses, and
//! PageUp/PageDown/arrows/vim keys that used to not work at all.
//!
//! What that embedded reader does **not** yet give a host: any way to read
//! back its current zoom/scroll/page-layout transform, or a wired
//! `ReaderAction::RegionSelected` (the action exists in kopitiam-pdf's own
//! `action.rs`, documented as exactly this host contract, but nothing in
//! `reader.rs` constructs one yet — verified by grep against the published
//! 0.3.2 source, not assumed). Without either, an overlay drawn by this
//! panel cannot stay in sync with the embedded reader's own live,
//! continuously-scrolling page layout — there is nothing public to sync
//! against. So the box-draw/select-text/crop-to-digitiser interaction stays
//! on kovan's own rendering, same math as before this migration, just now
//! confined to **[`ViewMode::Annotate`]**: a single static page — rasterized
//! straight off the *same* loaded document via
//! [`PdfReader::document`]/[`PdfReader::current_page`], no second file load
//! — with kovan's own Prev/Next, zoom, and draw/select-text tools. A plain
//! raster image (PNG/JPEG, [`PlotRaster`]) has no "continuous scroll" to
//! speak of at all and always behaves like `Annotate` mode.
//!
//! This is a real, acknowledged scope gap against the bead's stated ideal
//! ("interactive tools IN continuous-scroll mode"), not a silent one — see
//! `docs/kopitiam-issues/kopitiam-pdf-no-viewport-getter-or-region-selected.md`
//! for the upstream ask that would close it (a viewport/zoom/scroll-offset
//! getter, or `RegionSelected` actually wired to a drag gesture). Once
//! either lands, `Annotate` mode's canvas can be dropped in favour of an
//! overlay on the live `Read`-mode pane.
//!
//! **Deleted in this migration, not ported**: kovan's own thumbnail strip,
//! continuous-scroll page layout, and hand-rolled hot-reload polling (now
//! [`kopitiam_pdf::gui_frontend::HotReload`], the same mechanism — see that
//! type's own doc comment, which credits kovan as its origin) — `Read` mode
//! gets all of these from the embedded reader instead. Also deleted:
//! `pdf_annots.rs`'s manual `/Annots` overlay renderer. `rasterize_page`
//! (used by both modes) already bakes existing PDF-native annotations into
//! the page raster itself (mupdf's `pdf_run_page_annots` pass, run after the
//! content stream) — verified by reading `kopitiam_pdf::mupdf`'s
//! `rasterize_page_ex` source, not assumed — so the separate overlay was
//! redundant for the one thing that matters here (visibility); the overlay's
//! extra hover-tooltip-from-`/Annots`-metadata feature is not reproduced,
//! matching kovan's stated philosophy that durable knowledge capture is the
//! fenced-TOML artifact model, not PDF-native annotations.
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

use eframe::egui::{
    self, Color32, ColorImage, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions,
};
use kopitiam_pdf::gui_frontend::{
    HotReload, PdfReader, PdfReaderConfig, ReaderAction, ReloadDecision, RELOAD_CHECK_INTERVAL,
};
use kopitiam_pdf::mupdf::{page_to_stext, rasterize_page, PdfDocument, StextBlock, StextOptions, StextPage};

use crate::artifact::{Artifact, ArtifactKind};
use crate::digitiser::dataset::utc_now_iso8601;
use crate::digitiser::raster::PlotRaster;
use crate::project;
use crate::session::PaperSession;

use super::csv_preview::draw_csv_preview;

/// Screen-resolution DPI for [`ViewMode::Annotate`]'s single-page raster and
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

/// Which of the two rendering paths (see the module doc) is showing a PDF.
/// Meaningless for a plain image, which always behaves like `Annotate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ViewMode {
    /// The embedded [`PdfReader`]'s own chrome — fast continuous scroll,
    /// search, thumbnails, vim keys. No box-draw/select-text tools (see the
    /// module doc for why).
    #[default]
    Read,
    /// Kovan's own static single-page canvas: draw-box → Annotate/Digitise
    /// graph/Read table, select-text, crop-to-digitiser.
    Annotate,
}

/// Which annotation interaction is active. Closed set, enum-dispatched.
/// Only meaningful while showing [`ViewMode::Annotate`]'s canvas (or a plain
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

/// What clicking an artifact in the page-context panel produced (op-j178,
/// GH issue #35 2026-09-01 05:37: "there should be a markdown editor in
/// there with the various blocks... these should be individually clickable
/// and editable. CSV... editable only through the digitiser UI"). A
/// digitised table/graph artifact has no `JumpToLine` counterpart —
/// `context_panel` never makes one clickable, since there is no data path
/// (yet) to reopen its already-extracted CSV as a live, re-editable
/// digitiser session; see that method's doc for why.
pub enum ContextPanelAction {
    /// Jump to this artifact's heading in the paper's markdown, in the kvim
    /// editor — [`Artifact::line`], 1-based.
    JumpToLine(usize),
}

/// State for one open document: its source (embedded [`PdfReader`] or a
/// plain image), which mode is showing, [`ViewMode::Annotate`]'s own page/
/// zoom/cached texture, and its annotations.
#[derive(Default)]
pub struct PdfReaderState {
    path: String,
    source: ReaderSource,
    mode: ViewMode,
    /// [`ViewMode::Annotate`]'s own page tracking — independent of the
    /// embedded [`PdfReader`]'s internal page (see the module doc's "Known
    /// gap": there is no public way to seek the embedded reader to a page,
    /// so switching modes only syncs `Read` -> `Annotate`, not back).
    /// Always `0` for a plain image.
    annotate_page: usize,
    texture: Option<TextureHandle>,
    /// The page index [`Self::texture`] was rendered for, so a page flip or
    /// zoom-only change knows whether to re-rasterize.
    texture_page: usize,
    /// [`ViewMode::Annotate`]'s zoom on the displayed texture.
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

/// What [`PdfReaderState::annotate_page`] should become this frame, if
/// anything, given the mode transition that just happened — op-smrs (GH
/// issue #35's 2026-09-01 checkpoint, §17): "read page 87 -> switch to
/// Annotate -> page resets to page 1" was a real bug, since nothing synced
/// the embedded [`PdfReader`]'s own page into `annotate_page` at the point
/// of the switch. `None` for every other transition (including the
/// reverse, Annotate -> Read, which has no public API to do the same yet —
/// see the module doc's "Known gap", kopitiam#105).
fn synced_annotate_page(before: ViewMode, after: ViewMode, reader_current_page: usize, reader_page_count: usize) -> Option<usize> {
    if before == ViewMode::Read && after == ViewMode::Annotate {
        Some(reader_current_page.min(reader_page_count.saturating_sub(1)))
    } else {
        None
    }
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

impl PdfReaderState {
    /// A fresh reader — `Read` mode and hot-reload both **on** by default
    /// for a PDF (GitHub issue #30's explicit "hot reload by default in
    /// case I compile live in tex or typst"; `Read` is [`ViewMode`]'s own
    /// derived default already). Prefer this over `PdfReaderState::default()`
    /// so hot-reload starts enabled, since [`HotReload`]'s own `Default`
    /// (unlike this panel's prior hand-rolled `bool`) starts disabled.
    pub fn new() -> Self {
        Self { hot_reload: HotReload::new(true), ..Self::default() }
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
        self.mode = ViewMode::Read;
        self.annotate_page = 0;
        self.texture = None;
        self.zoom = if self.zoom > 0.0 { self.zoom } else { 1.0 };
        self.annotations.clear();
        self.draw_start = None;
        self.pending_box = None;
        self.context_menu = None;
        self.annotate_editor = None;
        self.bibtex = None;
        self.stext_cache = None;
        self.select_start = None;
        self.text_selection = None;
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

    /// The page number in effect for [`ViewMode::Annotate`]'s canvas and the
    /// right-hand context panel: while reading, the embedded [`PdfReader`]'s
    /// own current page (so the context panel follows along as the operator
    /// scrolls); while annotating, [`Self::annotate_page`] (kovan's own
    /// independent tracking — see the module doc's "Known gap"). Always `0`
    /// for a plain image.
    fn active_page(&self) -> usize {
        match &self.source {
            ReaderSource::Pdf(reader) => {
                if self.mode == ViewMode::Read {
                    reader.current_page()
                } else {
                    self.annotate_page
                }
            }
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
            page_index: self.active_page(),
            min,
            max,
            created_at: utc_now_iso8601(),
            author: self.author_name(),
        }
    }

    /// Append every not-yet-saved annotation on the active page into the
    /// paper's own canonical Markdown (op-q1qj, GH issue #35 2026-09-01
    /// 05:37: "project root isn't decided") — via
    /// [`PaperSession::append_block`]/[`PaperSession::save_document`] when
    /// `active_paper` is `Some` (the normal case: this reader is showing an
    /// [`crate::app::DigitiseApp::activate_paper`]'d
    /// paper's PDF), falling back to the older
    /// [`crate::project::append_to_section`] path over the manual
    /// `project_root`/`project_markdown_rel` fields only for a PDF opened
    /// outside any paper. One `###` subsection per annotation, each stating
    /// author/page/pixel-bbox per the design doc's §4.1 shape. Saves the
    /// whole page's worth in one call rather than one call per annotation.
    fn save_annotations_into_project(&mut self, active_paper: Option<&mut PaperSession>) {
        let page = self.active_page();
        let Some(anns) = self.annotations.get(&page) else {
            self.message = "no annotations on this page to save".to_string();
            return;
        };
        if anns.is_empty() {
            self.message = "no annotations on this page to save".to_string();
            return;
        }
        let count = anns.len();
        let mut block = String::new();
        for ann in anns {
            block.push_str(&format!(
                "### annotation — {}\n- author: {}\n- page: {}\n- pixel bbox: [{:.1}, {:.1}, {:.1}, {:.1}]\n\n{}\n\n",
                ann.created_at,
                ann.author,
                page + 1,
                ann.min.x,
                ann.min.y,
                ann.max.x,
                ann.max.y,
                ann.text
            ));
        }
        let block = block.trim_end();

        if let Some(session) = active_paper {
            session.append_block(block);
            self.message = match session.save_document() {
                Ok(()) => format!("saved {count} annotation(s) into {}", session.citekey()),
                Err(e) => e.to_string(),
            };
            return;
        }

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

    /// Advance [`Self::annotate_page`] by one, clamped to the document's
    /// page count. Only meaningful in [`ViewMode::Annotate`] on a PDF.
    fn annotate_next_page(&mut self) {
        if self.annotate_page + 1 < self.source.page_count() {
            self.annotate_page += 1;
            self.pending_box = None;
            self.context_menu = None;
            self.text_selection = None;
        }
    }

    fn annotate_prev_page(&mut self) {
        let before = self.annotate_page;
        self.annotate_page = self.annotate_page.saturating_sub(1);
        if self.annotate_page != before {
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
            let doc = self.current_pdf_document()?;
            let stext = page_to_stext(doc, page, StextOptions::default()).ok()?;
            self.stext_cache = Some((page, stext));
        }
        self.stext_cache.as_ref().map(|(_, s)| s)
    }

    /// Rasterize/convert [`Self::active_page`] and upload it as a texture
    /// for [`ViewMode::Annotate`]'s canvas, if not already cached for that
    /// page.
    fn ensure_annotate_texture(&mut self, ctx: &egui::Context) {
        let page = self.active_page();
        if self.texture.is_some() && self.texture_page == page {
            return;
        }
        match &self.source {
            ReaderSource::None => {}
            ReaderSource::Pdf(reader) => match rasterize_page(reader.document(), page, RENDER_DPI) {
                Ok(pixmap) => {
                    let (w, h) = (pixmap.w as usize, pixmap.h as usize);
                    let image = if pixmap.alpha {
                        ColorImage::from_rgba_unmultiplied([w, h], &pixmap.samples)
                    } else {
                        ColorImage::from_rgb([w, h], &pixmap.samples)
                    };
                    self.texture = Some(ctx.load_texture(
                        format!("pdf-page-{page}"),
                        image,
                        TextureOptions::LINEAR,
                    ));
                    self.texture_page = page;
                }
                Err(e) => {
                    self.message = format!("page {} render failed: {e}", page + 1);
                    self.texture = None;
                }
            },
            ReaderSource::Image(raster) => {
                let image = raster_to_color_image(raster);
                self.texture =
                    Some(ctx.load_texture("reader-image", image, TextureOptions::LINEAR));
                self.texture_page = page;
            }
        }
    }

    /// The right "page context" panel (op-0y4k, op-j178). With an active
    /// paper open (`active_artifacts` is `Some`), this lists the artifacts
    /// — §14's fenced-TOML blocks — anchored to [`Self::active_page`],
    /// via the same [`crate::research_record::ResearchRecordIndex::
    /// anchored_to_page`] query §31 was built for ("when the PDF reader
    /// shows page 87, these are what to highlight"): a text/annotation
    /// artifact is a clickable link that jumps the kvim editor to its
    /// heading ([`Artifact::line`]); a digitised table/graph artifact shows
    /// its CSV read-only (never a clickable "edit this text" link — GH
    /// issue #35 2026-09-01: "CSV... editable only through the digitiser
    /// UI"). **Not a re-import path**: there is no way yet to reopen an
    /// already-extracted CSV as a live, re-calibrated digitiser session —
    /// the fenced-TOML artifact format's `[extraction]` table records only
    /// `method`/`engine` strings, no pixel/calibration data (see
    /// `crate::artifact::Extraction`) — so redoing one still means
    /// re-digitising the figure from the PDF. Falls back to the old
    /// disk-text `blocks_matching` preview over `project_root`/
    /// `project_markdown_rel` when no paper is active (a PDF opened outside
    /// any paper), unchanged from before this pass.
    fn context_panel(&mut self, ui: &mut egui::Ui, active_artifacts: Option<&[Artifact]>) -> Option<ContextPanelAction> {
        ui.heading("Page context");
        let Some(artifacts) = active_artifacts else {
            self.context_panel_fallback(ui);
            return None;
        };
        let page = (self.active_page() + 1) as u32;
        let anchored: Vec<&Artifact> =
            artifacts.iter().filter(|a| a.toml.source.as_ref().is_some_and(|s| s.covers_page(page))).collect();
        if anchored.is_empty() {
            ui.small("nothing saved for this page yet");
            return None;
        }
        let mut result = None;
        egui::ScrollArea::vertical().id_salt("pdf_context_panel_scroll").show(ui, |ui| {
            for artifact in anchored {
                match artifact.kind() {
                    ArtifactKind::DigitisedTable | ArtifactKind::DigitisedGraph => {
                        let icon = if artifact.kind() == ArtifactKind::DigitisedTable { "\u{1F4CA}" } else { "\u{1F4C8}" };
                        ui.label(format!("{icon} {}", artifact.heading));
                        if let Some(csv) = artifact.csv_block() {
                            draw_csv_preview(ui, csv);
                        }
                        ui.small("re-digitise from the PDF to change it");
                    }
                    _ => {
                        if ui.link(format!("\u{1F4DD} {}", artifact.heading)).clicked() {
                            result = Some(ContextPanelAction::JumpToLine(artifact.line));
                        }
                    }
                }
                ui.separator();
            }
        });
        result
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

    /// Draw the toolbar and the active view — [`ViewMode::Read`]'s embedded
    /// reader chrome, or [`ViewMode::Annotate`]'s box-draw/select-text
    /// canvas. `on_open_clicked` is called when the user asks to open a
    /// different document — the caller owns the file dialog (shared with the
    /// digitiser's "Load image" action) and reports the chosen path back via
    /// [`PdfReaderState::open`].
    ///
    /// Returns `Some` the frame the user completes a crop-then-right-click
    /// gesture (op-p17q / op-hnhp) — the caller (`DigitiseApp`) is expected
    /// to load it into the matching digitiser tab and switch views. Never
    /// `Some` while in `Read` mode — see the module doc's "Known gap".
    ///
    /// `active_paper` is the wider app's [`crate::app::
    /// DigitiseApp::activate_paper`]'d paper, if any (op-q1qj, GH issue #35
    /// 2026-09-01 05:37: "project root isn't decided") — when `Some`,
    /// annotations save straight into its canonical Markdown and the
    /// page-context panel reads live from the same file, instead of the
    /// manual `project_root`/`project_markdown_rel` fields (which remain
    /// the fallback for a PDF opened outside any paper).
    ///
    /// Returns `(crop_result, context_panel_action)` — the second element
    /// is `Some` the frame the page-context panel's own per-block click
    /// produces one (op-j178); see [`ContextPanelAction`].
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        mut on_open_clicked: impl FnMut(),
        active_paper: Option<&mut PaperSession>,
    ) -> (Option<CropResult>, Option<ContextPanelAction>) {
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
            return (None, None);
        }

        self.check_hot_reload(ui.ctx());

        let is_pdf = matches!(self.source, ReaderSource::Pdf(_));
        let show_annotate_ui = !is_pdf || self.mode == ViewMode::Annotate;
        let mode_before = self.mode;

        ui.horizontal(|ui| {
            if is_pdf {
                ui.selectable_value(&mut self.mode, ViewMode::Read, "Read");
                ui.selectable_value(&mut self.mode, ViewMode::Annotate, "Annotate & crop");
                ui.separator();
                let mut enabled = self.hot_reload.is_enabled();
                if ui.checkbox(&mut enabled, "Hot reload").changed() {
                    self.hot_reload.set_enabled(enabled);
                }
            }
            if show_annotate_ui {
                ui.separator();
                if is_pdf {
                    if ui.button("< Prev").clicked() {
                        self.annotate_prev_page();
                    }
                    ui.label(format!("page {} / {}", self.annotate_page + 1, self.source.page_count()));
                    if ui.button("Next >").clicked() {
                        self.annotate_next_page();
                    }
                    ui.separator();
                }
                ui.add(egui::Slider::new(&mut self.zoom, 0.25..=4.0).text("zoom"));
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

        // op-smrs (GH issue #35's 2026-09-01 checkpoint, S17): switching
        // Read -> Annotate used to always drop back to whatever
        // `annotate_page` last was (0, on the very first switch) instead of
        // the page the operator was just reading. Sync it at the point the
        // switch actually happens this frame — the one direction with a
        // public API to do it (`PdfReader::current_page()`); the reverse,
        // Annotate -> Read, has none yet (kopitiam#105, see the module doc).
        if let ReaderSource::Pdf(reader) = &self.source {
            if let Some(page) = synced_annotate_page(mode_before, self.mode, reader.current_page(), reader.page_count()) {
                self.annotate_page = page;
            }
        }

        // op-t55c (GH issue #35's 2026-09-01 checkpoint, S18): page-turn
        // keys only worked in Read mode, where the embedded PdfReader
        // supplies them for free — Annotate/Crop mode's own static canvas
        // had Prev/Next buttons but no keyboard bindings at all. Unlike the
        // page-sync direction above, this needs no missing upstream API:
        // wire the same keys the embedded reader already answers to, so
        // switching modes doesn't also mean switching input habits.
        // Deliberately deferred to any focused text-editing widget first
        // (the checkpoint's own stated exception) — the drag/click gestures
        // already used by DrawBox/SelectText are pointer-only and never
        // take keyboard focus, so this only ever yields to `author`/project-
        // field/text-editor typing.
        if is_pdf && self.mode == ViewMode::Annotate && ui.ctx().memory(|m| m.focused().is_none()) {
            let (page_down, page_up) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::PageDown) || i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::J),
                    i.key_pressed(egui::Key::PageUp) || i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::K),
                )
            });
            if page_down {
                self.annotate_next_page();
            } else if page_up {
                self.annotate_prev_page();
            }
        }

        if show_annotate_ui {
            ui.small(
                "Draw box → right-click it → Annotate / Digitise graph / Read table. \
                 Right-click an existing box → Edit / Delete.",
            );
        }
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
            if ui.button("Save page annotations").clicked() {
                save_clicked = true;
            }
        });
        if save_clicked {
            self.save_annotations_into_project(active_paper);
        }
        if show_annotate_ui {
            self.text_selection_panel(ui);
        }
        let mut crop_result = if show_annotate_ui { self.annotate_editor_panel(ui) } else { None };
        if is_pdf {
            self.bibtex_panel(ui);
        }
        ui.separator();

        let mut context_action = None;
        egui::Panel::right("pdf_reader_context")
            .resizable(true)
            .default_size(280.0)
            .show(ui, |ui| context_action = self.context_panel(ui, active_artifacts.as_deref()));

        if is_pdf && self.mode == ViewMode::Read {
            let ReaderSource::Pdf(reader) = &mut self.source else {
                unreachable!("is_pdf just matched ReaderSource::Pdf above")
            };
            let out = reader.show(ui);
            for action in out.actions {
                match action {
                    // This panel opens every PDF read-only (see `open_pdf`),
                    // so there is nothing to save and nowhere to save it —
                    // saying so beats silently ignoring the request.
                    ReaderAction::SaveRequested | ReaderAction::SaveAsRequested => {
                        reader.set_status("this viewer is read-only");
                    }
                    _ => {}
                }
            }
            if !self.message.is_empty() {
                ui.label(&self.message);
            }
            return (crop_result, context_action);
        }

        // --- ViewMode::Annotate / a plain image: kovan's own static
        // single-page canvas, unchanged from before this migration. ---
        self.ensure_annotate_texture(ui.ctx());
        let Some(texture) = self.texture.clone() else {
            if !self.message.is_empty() {
                ui.label(&self.message);
            }
            return (None, context_action);
        };
        let size = texture.size_vec2() * self.zoom;
        let zoom = self.zoom;
        let page = self.active_page();
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

            // --- overlays: saved annotations (hover-highlighted), then the
            // pending box ---
            let hovered = response
                .hover_pos()
                .map(to_image)
                .and_then(|p| {
                    self.annotations
                        .get(&page)
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
                    .get(&page)
                    .and_then(|anns| anns.get(i))
                    .map(|a| a.created_at.clone())
            });
            if let Some(anns) = self.annotations.get(&page) {
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
        });

        if let Some(result) = self.context_menu_ui(ui.ctx()) {
            crop_result = Some(result);
        }

        (crop_result, context_action)
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
                    self.annotations.entry(self.active_page()).or_default().push(Annotation {
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
            let anns = self.annotations.entry(self.active_page()).or_default();
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

    #[test]
    fn new_reader_starts_in_read_mode_with_hot_reload_on() {
        let r = PdfReaderState::new();
        assert_eq!(r.mode, ViewMode::Read);
        assert!(r.hot_reload.is_enabled());
        // Everything else should still be the plain derived-Default zero
        // state -- `new()` only overrides hot-reload.
        assert!(r.path.is_empty());
        assert!(r.annotations.is_empty());
    }

    #[test]
    fn synced_annotate_page_follows_the_reader_on_read_to_annotate() {
        assert_eq!(synced_annotate_page(ViewMode::Read, ViewMode::Annotate, 86, 200), Some(86));
    }

    #[test]
    fn synced_annotate_page_clamps_to_the_last_page() {
        // A defensive case only -- current_page() should never exceed
        // page_count() - 1 in practice, but the sync must not panic/produce
        // an out-of-range page if it somehow did.
        assert_eq!(synced_annotate_page(ViewMode::Read, ViewMode::Annotate, 999, 5), Some(4));
    }

    #[test]
    fn synced_annotate_page_does_nothing_on_other_transitions() {
        assert_eq!(synced_annotate_page(ViewMode::Annotate, ViewMode::Read, 86, 200), None);
        assert_eq!(synced_annotate_page(ViewMode::Read, ViewMode::Read, 86, 200), None);
        assert_eq!(synced_annotate_page(ViewMode::Annotate, ViewMode::Annotate, 86, 200), None);
    }

    /// op-q1qj (GH issue #35 2026-09-01 05:37, "project root isn't
    /// decided"): with an active paper's `PaperSession` supplied,
    /// `save_annotations_into_project` must write straight into its
    /// canonical Markdown via `append_block`/`save_document` — no
    /// `project_root`/`project_markdown_rel` needed at all.
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
                min: Pos2::new(1.0, 2.0),
                max: Pos2::new(3.0, 4.0),
                text: "a note about figure 3".to_string(),
                created_at: "2026-09-01T00:00:00Z".to_string(),
                author: "tester".to_string(),
            }],
        );

        let mut session = PaperSession::open(&root, &citekey).unwrap();
        state.save_annotations_into_project(Some(&mut session));

        assert!(state.message.contains(&citekey), "status should name the paper it saved into: {}", state.message);
        let on_disk = std::fs::read_to_string(root.paper_markdown(&citekey)).unwrap();
        assert!(on_disk.contains("a note about figure 3"), "annotation text should be in the saved markdown:\n{on_disk}");
        assert!(on_disk.contains("### annotation"));
    }
}
