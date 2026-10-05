//! The main view's coordinates and its always-visible controls: pan, zoom
//! about a point, fit to the screen, the + / − / Reset buttons and a scale
//! bar (mobile-first rule, items 1, 2 and 5).

use egui::{Color32, Pos2, Rect, RichText, Stroke, Vec2};

/// World (cm, y up) to screen (pixels, y down) for a 2D picture centred on a
/// subject of known half-width.
#[derive(Clone, Copy, Debug)]
pub struct View {
    /// World point (cm) at the centre of the canvas.
    pub centre: [f64; 2],
    /// Pixels per cm.
    pub scale: f64,
    /// Half-width of the subject, cm: what Reset fits to the screen.
    pub half_extent: f64,
    /// Fitted to the canvas yet? (Fit happens on the first frame, when the
    /// canvas size is known.)
    pub fitted: bool,
    /// The main view's size at the last fit, and whether the reader has
    /// zoomed or panned since. An untouched view refits when its size changes
    /// (the panel folds, the phone turns, or the first frames were laid out
    /// before the canvas reached device-pixel size, gh:#556).
    pub fit_size: Vec2,
    pub touched: bool,
}

impl View {
    pub fn new(half_extent: f64) -> Self {
        Self { centre: [0.0, 0.0], scale: 1.0, half_extent, fitted: false, fit_size: Vec2::ZERO, touched: false }
    }
    /// Pixels per cm that make the subject fill 94 % of the smaller side.
    pub fn fit_scale(&self, rect: Rect) -> f64 {
        (rect.width().min(rect.height()) as f64) * 0.94 / (2.0 * self.half_extent)
    }
    /// Reset: centre the subject and fit it to the screen.
    pub fn fit(&mut self, rect: Rect) {
        self.centre = [0.0, 0.0];
        self.scale = self.fit_scale(rect);
        self.fitted = true;
        self.fit_size = rect.size();
        self.touched = false;
    }
    pub fn to_screen(&self, rect: Rect, x: f64, y: f64) -> Pos2 {
        let c = rect.center();
        Pos2::new(c.x + ((x - self.centre[0]) * self.scale) as f32, c.y - ((y - self.centre[1]) * self.scale) as f32)
    }
    pub fn to_world(&self, rect: Rect, p: Pos2) -> [f64; 2] {
        let c = rect.center();
        [self.centre[0] + (p.x - c.x) as f64 / self.scale, self.centre[1] - (p.y - c.y) as f64 / self.scale]
    }
    /// Multiply the zoom by `factor`, keeping the world point under `p`
    /// fixed. Clamped to between half the fit and 600 times it.
    pub fn zoom_about(&mut self, rect: Rect, p: Pos2, factor: f64) {
        let before = self.to_world(rect, p);
        let fit = self.fit_scale(rect);
        self.touched = true;
        self.scale = (self.scale * factor).clamp(fit * 0.5, fit * 600.0);
        let after = self.to_world(rect, p);
        self.centre[0] += before[0] - after[0];
        self.centre[1] += before[1] - after[1];
    }
    /// The gestures on the main view: fit on the first frame and on a double
    /// click, wheel or pinch to zoom about the pointer, drag to pan. They
    /// supplement [`zoom_buttons`], never replace them.
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
            self.centre[1] += d.y as f64 / self.scale;
            self.touched = true;
        }
    }
}

/// Which of the three buttons was pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zoom {
    In,
    Out,
    Reset,
}

/// + / − / Reset as real, finger-sized (36 px) buttons in the main view's top
/// right corner: a phone has no wheel, and pinch or double-tap are not
/// discoverable. Text, not a ⟲ glyph: egui's bundled fonts may not carry it.
pub fn zoom_buttons(ui: &mut egui::Ui, rect: Rect) -> Option<Zoom> {
    zoom_buttons_sized(ui, rect, 18.0)
}

/// [`zoom_buttons`] with label text `text_px` points high; the buttons grow
/// with it (the desktop workbench doubles every font, gh:#561).
pub fn zoom_buttons_sized(ui: &mut egui::Ui, rect: Rect, text_px: f32) -> Option<Zoom> {
    let k = text_px / 18.0;
    let buttons = [("+", "Zoom in", 40.0 * k, Zoom::In), ("−", "Zoom out", 40.0 * k, Zoom::Out), ("Reset", "Centre and fit to the screen", 64.0 * k, Zoom::Reset)];
    let gap = 6.0;
    let total: f32 = buttons.iter().map(|b| b.2).sum::<f32>() + gap * (buttons.len() - 1) as f32;
    let mut x = rect.right() - 8.0 - total;
    let mut hit = None;
    for (label, hover, w, z) in buttons {
        let b = Rect::from_min_size(Pos2::new(x, rect.top() + 8.0), Vec2::new(w, 36.0 * k));
        if ui.put(b, egui::Button::new(RichText::new(label).size(text_px))).on_hover_text(hover).clicked() {
            hit = Some(z);
        }
        x += w + gap;
    }
    hit
}

/// Apply [`zoom_buttons`]' answer to a [`View`]: zoom 1.5 times about the
/// centre, or fit.
pub fn apply_zoom(view: &mut View, rect: Rect, z: Zoom) {
    match z {
        Zoom::In => view.zoom_about(rect, rect.center(), 1.5),
        Zoom::Out => view.zoom_about(rect, rect.center(), 1.0 / 1.5),
        Zoom::Reset => view.fit(rect),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Zooming keeps the world point under the finger where it was; Reset
    /// centres the subject and fits it.
    #[test]
    fn zoom_keeps_the_point_under_the_finger_and_reset_fits() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(390.0, 844.0));
        let mut v = View::new(8.7);
        v.fit(rect);
        assert!((v.scale - 390.0 * 0.94 / 17.4).abs() < 1e-9);
        let finger = Pos2::new(300.0, 200.0);
        let before = v.to_world(rect, finger);
        v.zoom_about(rect, finger, 3.0);
        let after = v.to_world(rect, finger);
        assert!((before[0] - after[0]).abs() < 1e-9 && (before[1] - after[1]).abs() < 1e-9);
        apply_zoom(&mut v, rect, Zoom::Reset);
        assert_eq!(v.centre, [0.0, 0.0]);
        v.zoom_about(rect, rect.center(), 1e-6);
        assert!((v.scale - 0.5 * v.fit_scale(rect)).abs() < 1e-12, "zoom out is clamped at half the fit");
        assert!(v.touched, "zooming marks the view as the reader's");
        apply_zoom(&mut v, rect, Zoom::Reset);
        assert!(!v.touched && v.fit_size == rect.size(), "Reset hands it back");
    }
}

/// A scale bar in the bottom-left corner: the smallest round length at least
/// 80 px long.
pub fn scale_bar(painter: &egui::Painter, rect: Rect, view: &View) {
    scale_bar_sized(painter, rect, view, 12.0);
}

/// [`scale_bar`] with its label `text_px` points high.
pub fn scale_bar_sized(painter: &egui::Painter, rect: Rect, view: &View, text_px: f32) {
    let bar_cm = [0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0]
        .into_iter()
        .find(|&c| c * view.scale >= 80.0)
        .unwrap_or(100.0);
    let x0 = rect.left() + 16.0;
    let y0 = rect.bottom() - 12.0;
    let x1 = x0 + (bar_cm * view.scale) as f32;
    painter.line_segment([Pos2::new(x0, y0), Pos2::new(x1, y0)], Stroke::new(2.0, Color32::WHITE));
    let label = if bar_cm < 0.1 { format!("{:.0} µm", bar_cm * 1e4) } else { format!("{bar_cm} cm") };
    painter.text(Pos2::new(x0, y0 - 4.0), egui::Align2::LEFT_BOTTOM, label, egui::FontId::proportional(text_px), Color32::WHITE);
}
