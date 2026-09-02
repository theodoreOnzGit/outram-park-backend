//! `PageView` — kovan's own continuous multi-page PDF raster view
//! (GH issue #35 2026-09-02).
//!
//! # Why this exists
//!
//! The embedded `kopitiam_pdf::PdfReader` is a sealed widget: its
//! `PdfReaderOutput` carries only a `Vec<ReaderAction>`, there is no
//! host-overlay hook and no per-page screen geometry, so a caller cannot
//! draw its own boxes over the reader or route clicks on them
//! ([kopitiam#107](https://github.com/theodoreOnzGit/kopitiam/issues/107)).
//! The maintainer wants saved annotation / digitised-figure region boxes
//! visible **while scrolling**, single-click-to-edit — so kovan renders the
//! pages itself in the Annotate/crop canvas and this module owns the
//! rasterisation, the texture cache and the screen ⇄ page-pixel coordinate
//! maths.
//!
//! # Assumptions
//!
//! - **Uniform page size.** The first page that rasterises sets the pixel
//!   size used to lay out *every* page. Almost every paper is uniform; a
//!   document with wildly varying page sizes will have slightly wrong
//!   scroll extents, not broken interaction.
//! - Coordinates are `RENDER_DPI` **texture-pixel space** for a page (the
//!   same space `Annotation`/`CropProvenance`/`rasterize_page` already use),
//!   mapped to and from screen space by `zoom`.

use std::collections::{HashMap, HashSet, VecDeque};

use eframe::egui::{self, ColorImage, Pos2, Rect, TextureHandle, TextureOptions, Vec2};
use kopitiam_pdf::mupdf::{rasterize_page, PdfDocument};

/// How many page textures to keep uploaded at once — a window comfortably
/// larger than what fits on screen, so a scroll of a page or two never
/// waits on a re-raster.
const CACHE_CAP: usize = 10;

/// The **logical** page-pixel DPI — the coordinate space every annotation,
/// region, crop and hit-test uses, independent of the display zoom.
pub const BASE_DPI: f32 = 150.0;

/// Fallback logical page size (US-Letter at [`BASE_DPI`]) used only before
/// the first page has rendered, so layout has *something* to work with.
const FALLBACK_PX: Vec2 = Vec2 { x: 1275.0, y: 1650.0 };

/// One cached page raster and the supersampling factor it was rendered at.
struct PageTex {
    handle: TextureHandle,
    /// `1` = rendered at [`BASE_DPI`], `2` = 2×, etc. — bumped as the
    /// operator zooms in so the page stays sharp rather than being a
    /// stretched [`BASE_DPI`] bitmap.
    scale: u8,
}

/// A cached, uploaded page raster plus continuous-view geometry helpers.
#[derive(Default)]
pub struct PageView {
    /// The assumed-uniform **logical** ([`BASE_DPI`]) page pixel size, from
    /// the first page that rendered.
    logical_px: Option<Vec2>,
    textures: HashMap<usize, PageTex>,
    /// Eviction order — least-recently-ensured at the front.
    order: VecDeque<usize>,
    /// Pages whose rasterisation failed, so `ensure` does not retry them
    /// every frame.
    failed: HashSet<usize>,
}

impl PageView {
    /// Drop every cached texture (a new document opened, or the panel
    /// reset).
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// The **logical** ([`BASE_DPI`]) page pixel size layout and every
    /// coordinate conversion uses — the measured one, or [`FALLBACK_PX`]
    /// before anything has rendered. Independent of the display zoom and of
    /// the DPI a page's texture happens to be rendered at.
    pub fn page_size_px(&self) -> Vec2 {
        self.logical_px.unwrap_or(FALLBACK_PX)
    }

    /// The integer supersampling factor for a display `zoom` — a page's
    /// raster is rendered at `BASE_DPI * factor` so a zoomed-in page stays
    /// crisp. Bucketed (1/2/3) so a slider drag re-renders at most twice.
    pub fn render_scale(zoom: f32) -> u8 {
        (zoom.ceil() as i32).clamp(1, 3) as u8
    }

    /// One page's vertical stride in **content** pixels at `zoom`: the page
    /// height plus the inter-page `gap`.
    pub fn page_stride(&self, zoom: f32, gap: f32) -> f32 {
        (self.page_size_px().y * zoom + gap).max(1.0)
    }

    /// Content-space Y of page `p`'s top edge.
    pub fn page_top(&self, p: usize, zoom: f32, gap: f32) -> f32 {
        p as f32 * self.page_stride(zoom, gap)
    }

    /// Total content size for an `n`-page document (no trailing gap).
    pub fn content_size(&self, n: usize, zoom: f32, gap: f32) -> Vec2 {
        let h = (n as f32 * self.page_stride(zoom, gap) - gap).max(1.0);
        Vec2::new(self.page_size_px().x * zoom, h)
    }

    /// The inclusive page range intersecting `viewport` (content
    /// coordinates), clamped to `0..n`.
    pub fn visible_range(&self, viewport: Rect, n: usize, zoom: f32, gap: f32) -> std::ops::RangeInclusive<usize> {
        if n == 0 {
            return 0..=0;
        }
        let stride = self.page_stride(zoom, gap);
        let first = (viewport.min.y / stride).floor().max(0.0) as usize;
        let last = (viewport.max.y / stride).floor().max(0.0) as usize;
        let first = first.min(n - 1);
        let last = last.min(n - 1).max(first);
        first..=last
    }

    /// Rasterise + upload every page in `want` that is not cached **at
    /// supersampling factor `scale`** (re-rendering one cached at a
    /// different scale), and evict cached pages far outside `want`. `want`
    /// is clamped by the caller to the real page count.
    pub fn ensure(
        &mut self,
        ctx: &egui::Context,
        doc: &PdfDocument,
        want: std::ops::RangeInclusive<usize>,
        scale: u8,
    ) {
        let scale = scale.max(1);
        for p in want.clone() {
            if self.textures.get(&p).is_some_and(|t| t.scale == scale) || self.failed.contains(&p) {
                self.order.retain(|&q| q != p);
                self.order.push_back(p);
                continue;
            }
            match rasterize_page(doc, p, BASE_DPI * scale as f32) {
                Ok(pixmap) => {
                    let (w, h) = (pixmap.w as usize, pixmap.h as usize);
                    let image = if pixmap.alpha {
                        ColorImage::from_rgba_unmultiplied([w, h], &pixmap.samples)
                    } else {
                        ColorImage::from_rgb([w, h], &pixmap.samples)
                    };
                    let handle = ctx.load_texture(format!("kovan-page-{p}@{scale}"), image, TextureOptions::LINEAR);
                    self.logical_px.get_or_insert(Vec2::new(w as f32 / scale as f32, h as f32 / scale as f32));
                    self.textures.insert(p, PageTex { handle, scale });
                    self.order.retain(|&q| q != p);
                    self.order.push_back(p);
                }
                Err(_) => {
                    self.failed.insert(p);
                }
            }
        }
        while self.textures.len() > CACHE_CAP {
            let Some(&victim) = self.order.iter().find(|q| !want.contains(q)) else {
                break;
            };
            self.order.retain(|&q| q != victim);
            self.textures.remove(&victim);
        }
    }

    /// The uploaded texture for `p`, if cached (at whatever scale).
    pub fn texture(&self, p: usize) -> Option<&TextureHandle> {
        self.textures.get(&p).map(|t| &t.handle)
    }

    /// Install a single already-built image as the only "page" (page 0) —
    /// for a directly-loaded raster image (`ReaderSource::Image`), which has
    /// no PDF to rasterise. Idempotent per `size`.
    pub fn set_single_image(&mut self, ctx: &egui::Context, image: ColorImage) {
        let size = Vec2::new(image.size[0] as f32, image.size[1] as f32);
        if self.logical_px == Some(size) && self.textures.contains_key(&0) {
            return;
        }
        self.clear();
        let handle = ctx.load_texture("kovan-image", image, TextureOptions::LINEAR);
        self.logical_px = Some(size);
        self.textures.insert(0, PageTex { handle, scale: 1 });
        self.order.push_back(0);
    }

    /// The on-screen rect of page `p`'s image, given the content's top-left
    /// `origin` in screen space.
    pub fn page_rect(&self, p: usize, origin: Pos2, zoom: f32, gap: f32) -> Rect {
        let top = self.page_top(p, zoom, gap);
        let sz = self.page_size_px() * zoom;
        Rect::from_min_size(origin + Vec2::new(0.0, top), sz)
    }

    /// `(page, page-pixel point)` → screen point (the inverse of
    /// [`Self::hit`] without the bounds rejection — for placing overlays).
    pub fn project(&self, page: usize, px: Pos2, origin: Pos2, zoom: f32, gap: f32) -> Pos2 {
        origin + Vec2::new(px.x * zoom, self.page_top(page, zoom, gap) + px.y * zoom)
    }

    /// Screen point → `(page, page-pixel point)`. `None` when the point is
    /// left of the pages, past the last page, or in an inter-page gap.
    pub fn hit(&self, screen: Pos2, origin: Pos2, n: usize, zoom: f32, gap: f32) -> Option<(usize, Pos2)> {
        if n == 0 {
            return None;
        }
        let rel = screen - origin;
        if rel.x < 0.0 || rel.y < 0.0 {
            return None;
        }
        let stride = self.page_stride(zoom, gap);
        let p = (rel.y / stride).floor() as usize;
        if p >= n {
            return None;
        }
        let sz = self.page_size_px();
        let within_y = rel.y - p as f32 * stride;
        if within_y > sz.y * zoom {
            return None; // in the gap below the page
        }
        let px = Pos2::new(rel.x / zoom, within_y / zoom);
        if px.x > sz.x || px.y > sz.y {
            return None;
        }
        Some((p, px))
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `PageView` with a known page size but no textures — enough to test
    /// the pure geometry.
    fn sized(w: f32, h: f32) -> PageView {
        PageView { logical_px: Some(Vec2::new(w, h)), ..PageView::default() }
    }

    #[test]
    fn layout_stacks_pages_with_the_gap() {
        let v = sized(600.0, 800.0);
        assert_eq!(v.page_top(0, 1.0, 10.0), 0.0);
        assert_eq!(v.page_top(1, 1.0, 10.0), 810.0);
        assert_eq!(v.page_top(3, 1.0, 10.0), 2430.0);
        // 3 pages, no trailing gap: 800*3 + 10*2 = 2420
        assert_eq!(v.content_size(3, 1.0, 10.0).y, 2420.0);
        assert_eq!(v.content_size(3, 1.0, 10.0).x, 600.0);
    }

    #[test]
    fn render_scale_buckets_the_display_zoom() {
        assert_eq!(PageView::render_scale(0.5), 1);
        assert_eq!(PageView::render_scale(1.0), 1);
        assert_eq!(PageView::render_scale(1.3), 2);
        assert_eq!(PageView::render_scale(2.0), 2);
        assert_eq!(PageView::render_scale(2.4), 3);
        assert_eq!(PageView::render_scale(9.0), 3); // clamped
    }

    #[test]
    fn zoom_scales_the_stride() {
        let v = sized(600.0, 800.0);
        assert_eq!(v.page_top(1, 2.0, 10.0), 1610.0); // 800*2 + 10
    }

    #[test]
    fn hit_then_project_round_trips_inside_a_page() {
        let v = sized(600.0, 800.0);
        let origin = Pos2::new(50.0, 20.0);
        // A point on page 2, 100px right / 200px down in page space, at 1.5x.
        let screen = v.project(2, Pos2::new(100.0, 200.0), origin, 1.5, 12.0);
        let (page, px) = v.hit(screen, origin, 5, 1.5, 12.0).unwrap();
        assert_eq!(page, 2);
        assert!((px.x - 100.0).abs() < 1e-3 && (px.y - 200.0).abs() < 1e-3);
    }

    #[test]
    fn hit_rejects_the_inter_page_gap_and_out_of_range() {
        let v = sized(600.0, 800.0);
        let origin = Pos2::ZERO;
        // y = 805 at zoom 1, gap 10 → 5px into the gap after page 0.
        assert!(v.hit(Pos2::new(10.0, 805.0), origin, 3, 1.0, 10.0).is_none());
        // past the last page
        assert!(v.hit(Pos2::new(10.0, 10_000.0), origin, 3, 1.0, 10.0).is_none());
        // left of the pages
        assert!(v.hit(Pos2::new(-5.0, 10.0), origin, 3, 1.0, 10.0).is_none());
    }

    #[test]
    fn visible_range_covers_the_viewport() {
        let v = sized(600.0, 800.0);
        // pages are 810 tall (800 + 10 gap). viewport y 900..2000 → pages 1..=2.
        let vp = Rect::from_min_max(Pos2::new(0.0, 900.0), Pos2::new(600.0, 2000.0));
        assert_eq!(v.visible_range(vp, 10, 1.0, 10.0), 1..=2);
    }
}
