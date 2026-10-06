//! Pan and zoom of a canvas in world units (y down), its always-visible
//! + / − / Fit / 100 % buttons, and the canvas-size fix of eframe's web
//! runner.
//!
//! Ported, not shared, from `dhoby_ghaut::web_demo::view` (`View`,
//! `zoom_buttons`) and `dhoby_ghaut::web_demo::platform::
//! keep_canvas_at_device_pixels` (gh:#556): dhoby-ghaut depends on `kovan`,
//! and desktop kovan will embed this crate, so depending on dhoby-ghaut
//! would make a cycle. Differences: world y grows downward (the code map's
//! and the star's convention), the subject is a rectangle rather than a
//! half-width, and there is a separate 100 % button (Fit fits, 100 % shows
//! one world unit per point centred on the focus).

use egui::{Pos2, Rect, RichText, Vec2};

/// World rectangle `[min_x, min_y, max_x, max_y]`.
pub type WorldRect = [f64; 4];

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    /// World point at the centre of the canvas.
    pub centre: [f64; 2],
    /// Points per world unit.
    pub scale: f64,
    /// What Fit shows.
    pub subject: WorldRect,
    /// Where 100 % centres.
    pub home: [f64; 2],
    pub fitted: bool,
    fit_size: Vec2,
    /// The reader has zoomed or panned since the last fit.
    pub touched: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { centre: [0.0, 0.0], scale: 1.0, subject: [-1.0, -1.0, 1.0, 1.0], home: [0.0, 0.0], fitted: false, fit_size: Vec2::ZERO, touched: false }
    }
}

impl Camera {
    /// A camera that fits `subject` on its first frame.
    pub fn new(subject: WorldRect, home: [f64; 2]) -> Self {
        Camera { subject, home, ..Default::default() }
    }

    pub fn fit_scale(&self, rect: Rect) -> f64 {
        let w = (self.subject[2] - self.subject[0]).max(1.0);
        let h = (self.subject[3] - self.subject[1]).max(1.0);
        ((rect.width() as f64) / w).min((rect.height() as f64) / h) * 0.94
    }

    pub fn fit(&mut self, rect: Rect) {
        self.centre = [0.5 * (self.subject[0] + self.subject[2]), 0.5 * (self.subject[1] + self.subject[3])];
        self.scale = self.fit_scale(rect);
        self.fitted = true;
        self.fit_size = rect.size();
        self.touched = false;
    }

    /// One world unit per point, centred on `home`.
    pub fn actual_size(&mut self) {
        self.centre = self.home;
        self.scale = 1.0;
        self.touched = true;
    }

    pub fn to_screen(&self, rect: Rect, x: f64, y: f64) -> Pos2 {
        let c = rect.center();
        Pos2::new(c.x + ((x - self.centre[0]) * self.scale) as f32, c.y + ((y - self.centre[1]) * self.scale) as f32)
    }

    pub fn to_world(&self, rect: Rect, p: Pos2) -> [f64; 2] {
        let c = rect.center();
        [self.centre[0] + (p.x - c.x) as f64 / self.scale, self.centre[1] + (p.y - c.y) as f64 / self.scale]
    }

    /// A world rectangle on screen.
    pub fn rect(&self, rect: Rect, x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect::from_min_max(self.to_screen(rect, x, y), self.to_screen(rect, x + w, y + h))
    }

    /// Zoom by `factor` keeping the world point under `p` fixed; clamped
    /// between a fifth of the fit and 4 points per world unit (or 20 fits).
    pub fn zoom_about(&mut self, rect: Rect, p: Pos2, factor: f64) {
        let before = self.to_world(rect, p);
        let fit = self.fit_scale(rect);
        self.touched = true;
        self.scale = (self.scale * factor).clamp(fit * 0.2, (fit * 20.0).max(4.0));
        let after = self.to_world(rect, p);
        self.centre[0] += before[0] - after[0];
        self.centre[1] += before[1] - after[1];
    }

    /// Fit on the first frame, on a double click and when an untouched view
    /// changes size; wheel or pinch zooms about the pointer; drag pans.
    pub fn handle_input(&mut self, ui: &egui::Ui, resp: &egui::Response) {
        let rect = resp.rect;
        let resized = (rect.size() - self.fit_size).length() > 0.5;
        if !self.fitted || resp.double_clicked() || (resized && !self.touched) {
            self.fit(rect);
        }
        if resp.hovered() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = zoom as f64 * (scroll as f64 * 0.0025).exp();
            if (factor - 1.0).abs() > 1e-6 {
                if let Some(p) = resp.hover_pos() {
                    self.zoom_about(rect, p, factor);
                }
            }
        }
        if resp.dragged() {
            let d = resp.drag_delta();
            self.centre[0] -= d.x as f64 / self.scale;
            self.centre[1] -= d.y as f64 / self.scale;
            self.touched = true;
        }
    }

    /// The + / − / Fit / 100 % buttons, finger-sized (36 pt), in the
    /// canvas's top right corner (mobile-first rule, item 2).
    pub fn buttons(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let buttons = [("+", "Zoom in", 40.0), ("−", "Zoom out", 40.0), ("Fit", "Fit everything on screen", 52.0), ("100%", "Actual size, centred", 60.0)];
        let gap = 6.0;
        let total: f32 = buttons.iter().map(|b| b.2).sum::<f32>() + gap * 3.0;
        let mut x = rect.right() - 8.0 - total;
        for (i, (label, hover, w)) in buttons.into_iter().enumerate() {
            let b = Rect::from_min_size(Pos2::new(x, rect.top() + 8.0), Vec2::new(w, 36.0));
            if ui.put(b, egui::Button::new(RichText::new(label).size(17.0))).on_hover_text(hover).clicked() {
                match i {
                    0 => self.zoom_about(rect, rect.center(), 1.5),
                    1 => self.zoom_about(rect, rect.center(), 1.0 / 1.5),
                    2 => self.fit(rect),
                    _ => self.actual_size(),
                }
            }
            x += w + gap;
        }
    }
}

/// Keep every canvas's backing store at its CSS size times
/// `devicePixelRatio`; ported from `dhoby_ghaut::web_demo::platform::
/// keep_canvas_at_device_pixels` (gh:#556), whose doc comment has the
/// measurements. Returns whether this frame was laid out at the wrong size.
/// Natively it does nothing.
pub fn keep_canvas_at_device_pixels(ctx: &egui::Context) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast as _;
        let mut fixed = false;
        let Some(w) = web_sys::window() else { return false };
        let Some(d) = w.document() else { return false };
        let Ok(list) = d.query_selector_all("canvas") else { return false };
        let dpr = w.device_pixel_ratio();
        if !(dpr.is_finite() && dpr > 0.0) {
            return false;
        }
        for i in 0..list.length() {
            let Some(c) = list.item(i).and_then(|n| n.dyn_into::<web_sys::HtmlCanvasElement>().ok()) else { continue };
            let r = c.get_bounding_client_rect();
            let (want_w, want_h) = ((r.width() * dpr).round(), (r.height() * dpr).round());
            if want_w < 1.0 || want_h < 1.0 {
                continue;
            }
            if (c.width() as f64 - want_w).abs() > 2.0 || (c.height() as f64 - want_h).abs() > 2.0 {
                c.set_width(want_w as u32);
                c.set_height(want_h as u32);
                ctx.request_repaint();
                fixed = true;
            }
            let laid_out = ctx.content_rect().width() as f64 * ctx.pixels_per_point() as f64;
            if (laid_out - want_w).abs() > 2.0 {
                ctx.request_repaint();
                fixed = true;
            }
        }
        fixed
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = ctx;
        false
    }
}
