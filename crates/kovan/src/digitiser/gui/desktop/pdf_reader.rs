//! Integrated PDF reader panel — GitHub issue #30's "don't want to
//! screenshot then digitise slowly, everything should be integrated into
//! the reader" (op-95x6).
//!
//! Renders pages through `kopitiam_pdf::mupdf` — the PDF-page-rendering
//! engine decision this crate's `Cargo.toml` records (op-6ez3): open a PDF's
//! bytes with [`PdfDocument::open`], rasterize the current page with
//! [`rasterize_page`] at a fixed screen DPI, upload the samples as an egui
//! texture. One page is rasterized and cached at a time — a PDF is not
//! pre-rendered in full, so opening a large document is cheap and only the
//! visited pages cost render time.
//!
//! Does not belong here: PDF *text* extraction (that is
//! `kovan_literature::extract_metadata`'s job, already exposed via `kovan-cli
//! lit`), and the draw-box-then-digitise interaction that will attach to this
//! panel (op-p17q / op-hnhp — separate, not-yet-implemented beads that reuse
//! this panel as their display surface).

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use kopitiam_pdf::mupdf::{rasterize_page, PdfDocument};

/// Screen-resolution DPI for page rasterization — sharp enough to read body
/// text at a typical window size without the per-page render becoming slow.
/// The panel's zoom slider scales the *displayed* size, not this DPI, so
/// zooming in past 100% will show visible raster blur; re-rasterizing per
/// zoom level is future work if that turns out to matter in practice.
const RENDER_DPI: f32 = 150.0;

/// State for one open PDF: the parsed document, which page is showing, its
/// cached rasterization, and the zoom applied to the displayed texture.
#[derive(Default)]
pub struct PdfReaderState {
    path: String,
    doc: Option<PdfDocument>,
    page_count: usize,
    page_index: usize,
    texture: Option<TextureHandle>,
    /// The page index the cached `texture` was rendered for, so a page flip
    /// or zoom-only change knows whether to re-rasterize.
    texture_page: usize,
    zoom: f32,
    message: String,
}

impl PdfReaderState {
    /// Open `path` as the working PDF, replacing any previously open one.
    pub fn open(&mut self, path: &str) {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.message = format!("cannot read {path}: {e}");
                return;
            }
        };
        match PdfDocument::open(bytes) {
            Ok(doc) => {
                self.page_count = doc.page_count();
                self.path = path.to_string();
                self.doc = Some(doc);
                self.page_index = 0;
                self.texture = None;
                self.zoom = if self.zoom > 0.0 { self.zoom } else { 1.0 };
                self.message = format!("opened {path} ({} page(s))", self.page_count);
            }
            Err(e) => self.message = format!("cannot open {path}: {e}"),
        }
    }

    fn next_page(&mut self) {
        if self.page_index + 1 < self.page_count {
            self.page_index += 1;
        }
    }

    fn prev_page(&mut self) {
        self.page_index = self.page_index.saturating_sub(1);
    }

    /// Rasterize the current page and upload it as a texture, if not already
    /// cached for this page.
    fn ensure_texture(&mut self, ctx: &egui::Context) {
        let Some(doc) = &self.doc else { return };
        if self.texture.is_some() && self.texture_page == self.page_index {
            return;
        }
        match rasterize_page(doc, self.page_index, RENDER_DPI) {
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
        }
    }

    /// Draw the toolbar (page nav, zoom) and the page image. `on_open_clicked`
    /// is called when the user asks to open a different PDF — the caller owns
    /// the file dialog (shared with the digitiser's "Load image" action) and
    /// reports the chosen path back via [`PdfReaderState::open`].
    pub fn ui(&mut self, ui: &mut egui::Ui, mut on_open_clicked: impl FnMut()) {
        ui.horizontal(|ui| {
            if ui.button("Open PDF…").clicked() {
                on_open_clicked();
            }
            ui.label(if self.path.is_empty() {
                "no PDF open"
            } else {
                self.path.as_str()
            });
        });

        if self.doc.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label("no PDF open — click \"Open PDF…\"");
            });
            if !self.message.is_empty() {
                ui.label(&self.message);
            }
            return;
        }

        ui.horizontal(|ui| {
            if ui.button("< Prev").clicked() {
                self.prev_page();
            }
            ui.label(format!(
                "page {} / {}",
                self.page_index + 1,
                self.page_count
            ));
            if ui.button("Next >").clicked() {
                self.next_page();
            }
            ui.separator();
            ui.add(egui::Slider::new(&mut self.zoom, 0.25..=4.0).text("zoom"));
        });
        ui.separator();

        self.ensure_texture(ui.ctx());
        if let Some(texture) = &self.texture {
            let size = texture.size_vec2() * self.zoom;
            egui::ScrollArea::both().show(ui, |ui| {
                ui.image((texture.id(), size));
            });
        } else if !self.message.is_empty() {
            ui.label(&self.message);
        }
    }
}
