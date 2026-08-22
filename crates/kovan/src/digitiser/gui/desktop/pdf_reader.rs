//! Integrated PDF reader panel — GitHub issue #30's "don't want to
//! screenshot then digitise slowly, everything should be integrated into
//! the reader" (op-95x6), extended to view raster images directly,
//! Okular-style (op-wojr — "I want pdf reader to be able to view images
//! like okular as well").
//!
//! Renders PDF pages through `kopitiam_pdf::mupdf` — the PDF-page-rendering
//! engine decision this crate's `Cargo.toml` records (op-6ez3): open a PDF's
//! bytes with [`PdfDocument::open`], rasterize the current page with
//! [`rasterize_page`] at a fixed screen DPI, upload the samples as an egui
//! texture. One page is rasterized and cached at a time — a PDF is not
//! pre-rendered in full, so opening a large document is cheap and only the
//! visited pages cost render time. A plain image (PNG/JPEG) opens through the
//! digitiser's own [`PlotRaster`] loader instead — see [`ReaderSource`] — and
//! is treated as a one-page document, so the same page-nav/zoom/scroll
//! viewer below serves both without knowing which it has.
//!
//! Also carries a first-cut annotation layer — highlights and text notes,
//! per page — for GitHub issue #30's "I want to be able to scroll freely and
//! annotate the pdf just like okular" (op-gv19). **Scoped down deliberately**:
//! annotations live only in [`PdfReaderState`] for as long as the process
//! runs — there is no sidecar file and nothing is written back into the PDF
//! or picked up again on reopen. Okular persists annotations to disk; doing
//! that here needs a real file format decision (a sidecar next to the PDF,
//! or folding into the not-yet-built "kovan folder" project format from
//! op-63u0/op-b1y5) that is out of scope for this first cut — see op-gv19's
//! bead description. Ink/freehand strokes are not implemented either:
//! [`Annotation`] covers only axis-aligned highlight rectangles and
//! point-anchored text notes, which covers the issue's own wording
//! ("highlights/notes") without the added complexity of a stroke model.
//!
//! Does not belong here: PDF *text* extraction (that is
//! `kovan_literature::extract_metadata`'s job, already exposed via `kovan-cli
//! lit`), and the draw-box-then-digitise interaction that will attach to this
//! panel (op-p17q / op-hnhp — separate, not-yet-implemented beads that reuse
//! this panel as their display surface). Those tools will need their own
//! canvas-interaction mode alongside [`AnnotationTool`] when they land.

use std::collections::HashMap;

use eframe::egui::{
    self, Color32, ColorImage, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions, Vec2,
};
use kopitiam_pdf::mupdf::{rasterize_page, PdfDocument};

use crate::digitiser::raster::PlotRaster;

/// Screen-resolution DPI for page rasterization — sharp enough to read body
/// text at a typical window size without the per-page render becoming slow.
/// The panel's zoom slider scales the *displayed* size, not this DPI, so
/// zooming in past 100% will show visible raster blur; re-rasterizing per
/// zoom level is future work if that turns out to matter in practice.
const RENDER_DPI: f32 = 150.0;

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

/// One annotation, anchored in the current page's *texture-pixel* space
/// (i.e. the un-zoomed rasterization/image size — the same convention the
/// digitiser's own calibration pixels use) so it stays aligned across zoom
/// changes without needing to be re-scaled. Closed set, enum-dispatched.
#[derive(Debug, Clone)]
enum Annotation {
    /// A translucent highlight rectangle, corners in texture-pixel space.
    Highlight { min: Pos2, max: Pos2 },
    /// A text note anchored at one texture-pixel point.
    Note { pos: Pos2, text: String },
}

/// Which annotation tool a click/drag on the page currently invokes. Closed
/// set, enum-dispatched per the workspace's no-trait-objects rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum AnnotationTool {
    /// No annotation interaction — page nav/zoom only.
    #[default]
    None,
    /// Drag a rectangle to add a highlight.
    Highlight,
    /// Click to add a text note (edited afterwards in the notes list below
    /// the toolbar).
    Note,
}

/// State for one open document: its source (PDF or image), which page is
/// showing, its cached rasterization, the zoom applied to the displayed
/// texture, and its annotations (op-gv19).
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
    // annotations (op-gv19) — in-memory only, see the module doc comment.
    tool: AnnotationTool,
    annotations: HashMap<usize, Vec<Annotation>>,
    /// Texture-pixel-space start corner of a highlight drag in progress.
    highlight_drag_start: Option<Pos2>,
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

impl PdfReaderState {
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
                self.source = ReaderSource::Pdf(doc);
                self.page_index = 0;
                self.texture = None;
                self.zoom = if self.zoom > 0.0 { self.zoom } else { 1.0 };
                self.annotations.clear();
                self.highlight_drag_start = None;
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
                self.page_index = 0;
                self.texture = None;
                self.zoom = if self.zoom > 0.0 { self.zoom } else { 1.0 };
                self.annotations.clear();
                self.highlight_drag_start = None;
                self.message = format!("opened {path}");
            }
            Err(e) => self.message = format!("cannot open {path} as PDF or image: {e}"),
        }
    }

    fn next_page(&mut self) {
        if self.page_index + 1 < self.source.page_count() {
            self.page_index += 1;
        }
    }

    fn prev_page(&mut self) {
        self.page_index = self.page_index.saturating_sub(1);
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

    /// Draw the toolbar (page nav, zoom) and the page image. `on_open_clicked`
    /// is called when the user asks to open a different document — the
    /// caller owns the file dialog (shared with the digitiser's "Load image"
    /// action) and reports the chosen path back via [`PdfReaderState::open`].
    pub fn ui(&mut self, ui: &mut egui::Ui, mut on_open_clicked: impl FnMut()) {
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
            return;
        }

        let page_count = self.source.page_count();
        let is_pdf = matches!(self.source, ReaderSource::Pdf(_));
        ui.horizontal(|ui| {
            if is_pdf {
                if ui.button("< Prev").clicked() {
                    self.prev_page();
                }
                ui.label(format!("page {} / {}", self.page_index + 1, page_count));
                if ui.button("Next >").clicked() {
                    self.next_page();
                }
                ui.separator();
            }
            ui.add(egui::Slider::new(&mut self.zoom, 0.25..=4.0).text("zoom"));
            ui.separator();
            ui.label("annotate:");
            ui.selectable_value(&mut self.tool, AnnotationTool::None, "Select");
            ui.selectable_value(&mut self.tool, AnnotationTool::Highlight, "Highlight");
            ui.selectable_value(&mut self.tool, AnnotationTool::Note, "Note");
            if ui.button("Clear page annotations").clicked() {
                self.annotations.remove(&self.page_index);
            }
        });
        self.notes_panel(ui);
        ui.separator();

        self.ensure_texture(ui.ctx());
        let Some(texture) = self.texture.clone() else {
            if !self.message.is_empty() {
                ui.label(&self.message);
            }
            return;
        };
        let size = texture.size_vec2() * self.zoom;
        let zoom = self.zoom;
        let page_index = self.page_index;
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

            let to_image =
                move |pos: Pos2| -> Pos2 { ((pos - rect.min) / zoom).to_pos2() };
            let to_screen = move |p: Pos2| -> Pos2 { rect.min + p.to_vec2() * zoom };

            match self.tool {
                AnnotationTool::Highlight => {
                    if response.drag_started_by(egui::PointerButton::Primary) {
                        self.highlight_drag_start =
                            response.interact_pointer_pos().map(to_image);
                    }
                    if let (Some(start), Some(pos)) =
                        (self.highlight_drag_start, response.interact_pointer_pos())
                    {
                        let current = to_image(pos);
                        let (min, max) = (
                            Pos2::new(start.x.min(current.x), start.y.min(current.y)),
                            Pos2::new(start.x.max(current.x), start.y.max(current.y)),
                        );
                        painter.rect_filled(
                            Rect::from_min_max(to_screen(min), to_screen(max)),
                            0.0,
                            Color32::from_rgba_unmultiplied(255, 230, 60, 70),
                        );
                        if response.drag_stopped() {
                            self.annotations
                                .entry(page_index)
                                .or_default()
                                .push(Annotation::Highlight { min, max });
                            self.highlight_drag_start = None;
                        }
                    }
                }
                AnnotationTool::Note => {
                    if response.clicked() {
                        if let Some(pos) = response.interact_pointer_pos() {
                            self.annotations.entry(page_index).or_default().push(
                                Annotation::Note {
                                    pos: to_image(pos),
                                    text: String::new(),
                                },
                            );
                        }
                    }
                }
                AnnotationTool::None => {}
            }

            // --- overlay: this page's saved annotations ---
            if let Some(anns) = self.annotations.get(&page_index) {
                for ann in anns {
                    match ann {
                        Annotation::Highlight { min, max } => {
                            painter.rect_filled(
                                Rect::from_min_max(to_screen(*min), to_screen(*max)),
                                0.0,
                                Color32::from_rgba_unmultiplied(255, 230, 60, 70),
                            );
                        }
                        Annotation::Note { pos, text } => {
                            let p = to_screen(*pos);
                            painter.circle_filled(p, 6.0, Color32::from_rgb(230, 140, 20));
                            painter.circle_stroke(p, 6.0, Stroke::new(1.0_f32, Color32::BLACK));
                            if !text.is_empty() {
                                painter.text(
                                    p + Vec2::new(8.0, -8.0),
                                    egui::Align2::LEFT_BOTTOM,
                                    text,
                                    egui::FontId::proportional(13.0),
                                    Color32::from_rgb(230, 140, 20),
                                );
                            }
                        }
                    }
                }
            }
        });
    }

    /// Editable list of this page's text notes — shown under the toolbar so
    /// a note placed with the Note tool has somewhere to type its text (the
    /// canvas itself only places the marker; egui has no good in-place
    /// text-entry-on-a-painter primitive, so editing happens here instead).
    fn notes_panel(&mut self, ui: &mut egui::Ui) {
        let Some(anns) = self.annotations.get_mut(&self.page_index) else {
            return;
        };
        if anns.is_empty() {
            return;
        }
        let mut delete: Option<usize> = None;
        ui.horizontal_wrapped(|ui| {
            for (i, ann) in anns.iter_mut().enumerate() {
                if let Annotation::Note { text, .. } = ann {
                    ui.group(|ui| {
                        ui.label(format!("note {}", i + 1));
                        ui.add(
                            egui::TextEdit::singleline(text)
                                .hint_text("note text")
                                .desired_width(160.0),
                        );
                        if ui.small_button("✕").clicked() {
                            delete = Some(i);
                        }
                    });
                }
            }
        });
        if let Some(i) = delete {
            anns.remove(i);
        }
    }
}
