//! A log–log plot on the shared [`View`]: world coordinates are decades,
//! `(log10 x, log10 y)` minus an origin, so the framework's zoom about a
//! point, pan and fit work unchanged, and one zoom step is the same factor on
//! both axes.
//!
//! Drawing a million-point curve every frame is too slow for a phone, so each
//! curve is reduced per pixel column (first, lowest, highest and last value of
//! the points that land in it) before it is stroked. Nothing is drawn that is
//! not in the data: the reduction only drops points that would land on the
//! same pixel.

use dhoby_ghaut::web_demo::view::View;
use egui::{Color32, Pos2, Rect, Shape, Stroke};

/// Where the plotted data sit: the decade origin subtracted before the View,
/// and `ys`, how many world units one decade of y is, so the y range fills a
/// tall phone screen as well as the x range fills its width (zoom stays
/// uniform: one button press scales both axes by the same factor).
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub origin: [f64; 2],
    pub ys: f64,
}

impl Default for Frame {
    fn default() -> Self {
        Frame { origin: [0.0, 0.0], ys: 1.0 }
    }
}

impl Frame {
    /// A frame centred on the bounds `[x0, x1] × [y0, y1]` (data units), and
    /// the half-extent (decades) the View should fit.
    /// `(w, h)` is the canvas size in pixels: x fills the width, y the
    /// height.
    pub fn around(x0: f64, x1: f64, y0: f64, y1: f64, w: f64, h: f64) -> (Self, f64) {
        let (lx0, lx1, ly0, ly1) = (x0.log10(), x1.log10(), y0.log10(), y1.log10());
        let (xspan, yspan) = ((lx1 - lx0).max(0.5) * 1.06, (ly1 - ly0).max(0.5) * 1.06);
        let ys = (h * xspan / (w * yspan)).clamp(0.05, 20.0);
        let f = Frame { origin: [0.5 * (lx0 + lx1), 0.5 * (ly0 + ly1) * ys], ys };
        // View::fit_scale is min(w, h)*0.94 / (2 half): pick half so x fills w.
        let half = xspan * w.min(h) / (2.0 * w);
        (f, half.max(0.05))
    }
    pub fn to_screen(&self, view: &View, rect: Rect, x: f64, y: f64) -> Option<Pos2> {
        if x > 0.0 && y > 0.0 && x.is_finite() && y.is_finite() {
            Some(view.to_screen(rect, x.log10() - self.origin[0], y.log10() * self.ys - self.origin[1]))
        } else {
            None
        }
    }
    /// The data-unit x and y at a screen point.
    pub fn to_data(&self, view: &View, rect: Rect, p: Pos2) -> [f64; 2] {
        let w = view.to_world(rect, p);
        [10f64.powf(w[0] + self.origin[0]), 10f64.powf((w[1] + self.origin[1]) / self.ys)]
    }
}

/// Positive, finite bounds of a set of curves' points: `(x0, x1, y0, y1)`.
pub fn bounds(sets: &[&[[f64; 2]]]) -> Option<(f64, f64, f64, f64)> {
    let mut b: Option<(f64, f64, f64, f64)> = None;
    for s in sets {
        for &[x, y] in s.iter() {
            if x > 0.0 && y > 0.0 && x.is_finite() && y.is_finite() {
                b = Some(match b {
                    None => (x, x, y, y),
                    Some((a, c, d, e)) => (a.min(x), c.max(x), d.min(y), e.max(y)),
                });
            }
        }
    }
    b
}

/// Decade grid lines and labels, with the axis names.
pub fn axes(painter: &egui::Painter, rect: Rect, view: &View, frame: &Frame, xname: &str, yname: &str) {
    let lo = frame.to_data(view, rect, rect.left_bottom());
    let hi = frame.to_data(view, rect, rect.right_top());
    let grid = Color32::from_rgb(38, 44, 56);
    let label = Color32::from_rgb(150, 158, 175);
    let font = egui::FontId::proportional(11.0);
    let (kx0, kx1) = (lo[0].log10().floor() as i32, hi[0].log10().ceil() as i32);
    let step_x = ((kx1 - kx0) as f32 / (rect.width() / 46.0)).ceil().max(1.0) as i32;
    for k in (kx0..=kx1).filter(|k| k.rem_euclid(step_x) == 0) {
        if let Some(p) = frame.to_screen(view, rect, 10f64.powi(k), lo[1].max(1e-300)) {
            painter.line_segment([Pos2::new(p.x, rect.top()), Pos2::new(p.x, rect.bottom())], Stroke::new(1.0, grid));
            painter.text(Pos2::new(p.x + 2.0, rect.bottom() - 28.0), egui::Align2::LEFT_BOTTOM, format!("1e{k}"), font.clone(), label);
        }
    }
    let (ky0, ky1) = (lo[1].log10().floor() as i32, hi[1].log10().ceil() as i32);
    let step_y = ((ky1 - ky0) as f32 / (rect.height() / 40.0)).ceil().max(1.0) as i32;
    for k in (ky0..=ky1).filter(|k| k.rem_euclid(step_y) == 0) {
        if let Some(p) = frame.to_screen(view, rect, lo[0].max(1e-300), 10f64.powi(k)) {
            painter.line_segment([Pos2::new(rect.left(), p.y), Pos2::new(rect.right(), p.y)], Stroke::new(1.0, grid));
            painter.text(Pos2::new(rect.left() + 4.0, p.y - 1.0), egui::Align2::LEFT_BOTTOM, format!("1e{k}"), font.clone(), label);
        }
    }
    painter.text(Pos2::new(rect.right() - 8.0, rect.bottom() - 28.0), egui::Align2::RIGHT_BOTTOM, xname, egui::FontId::proportional(12.0), Color32::WHITE);
    painter.text(Pos2::new(rect.left() + 4.0, rect.top() + 52.0), egui::Align2::LEFT_TOP, yname, egui::FontId::proportional(12.0), Color32::WHITE);
}

/// Stroke a curve, reduced per pixel column. Non-positive values break it.
pub fn line(painter: &egui::Painter, rect: Rect, view: &View, frame: &Frame, pts: &[[f64; 2]], stroke: Stroke) {
    let mut run: Vec<Pos2> = Vec::new();
    let mut col: Option<(f32, f32, f32, f32, f32)> = None; // x, first, min, max, last
    let flush = |run: &mut Vec<Pos2>, col: &mut Option<(f32, f32, f32, f32, f32)>| {
        if let Some((x, first, mn, mx, last)) = col.take() {
            run.push(Pos2::new(x, first));
            run.push(Pos2::new(x, mn));
            run.push(Pos2::new(x, mx));
            run.push(Pos2::new(x, last));
        }
    };
    let finish = |run: &mut Vec<Pos2>| {
        if run.len() >= 2 {
            painter.add(Shape::line(std::mem::take(run), stroke));
        }
        run.clear();
    };
    let (xl, xr) = (rect.left() - 2.0, rect.right() + 2.0);
    for &[x, y] in pts {
        let Some(p) = frame.to_screen(view, rect, x, y) else {
            flush(&mut run, &mut col);
            finish(&mut run);
            continue;
        };
        if p.x < xl || p.x > xr {
            // Keep one point beyond each edge so the line reaches it.
            flush(&mut run, &mut col);
            if p.x > xr {
                run.push(p);
                break;
            }
            run.clear();
            run.push(p);
            continue;
        }
        let px = p.x.round();
        match &mut col {
            Some((cx, _, mn, mx, last)) if *cx == px => {
                *mn = mn.min(p.y);
                *mx = mx.max(p.y);
                *last = p.y;
            }
            _ => {
                flush(&mut run, &mut col);
                col = Some((px, p.y, p.y, p.y, p.y));
            }
        }
    }
    flush(&mut run, &mut col);
    finish(&mut run);
}

/// One dot per point, radius from the weight (a probability) when given.
pub fn dots(painter: &egui::Painter, rect: Rect, view: &View, frame: &Frame, pts: &[[f64; 2]], w: &[f64], colour: Color32) {
    for (i, &[x, y]) in pts.iter().enumerate() {
        if let Some(p) = frame.to_screen(view, rect, x, y) {
            if rect.expand(4.0).contains(p) {
                let r = w.get(i).map_or(2.5, |p| 1.2 + 9.0 * p.sqrt()) as f32;
                painter.circle_filled(p, r, colour);
            }
        }
    }
}

/// A legend in the top-left corner, under where the "Controls »" button goes.
pub fn legend(painter: &egui::Painter, rect: Rect, items: &[(Color32, &str)]) {
    let mut y = rect.top() + 52.0 + 18.0;
    for (c, name) in items {
        let x = rect.left() + 12.0;
        painter.line_segment([Pos2::new(x, y), Pos2::new(x + 18.0, y)], Stroke::new(3.0, *c));
        painter.text(Pos2::new(x + 24.0, y), egui::Align2::LEFT_CENTER, *name, egui::FontId::proportional(12.0), Color32::WHITE);
        y += 18.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Vec2;

    /// The frame is centred on the data's decades and fits them, x across
    /// the width and y down the height; a point at the centre of the bounds
    /// lands at the centre of the screen, and screen-to-data inverts it.
    #[test]
    fn the_frame_centres_and_fits_the_data() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(390.0, 844.0));
        let (frame, half) = Frame::around(1.0e-5, 1.0e7, 1.0e-1, 1.0e5, 390.0, 844.0);
        assert!((half - 0.5 * 12.0 * 1.06).abs() < 1e-12);
        let mut view = View::new(half);
        view.fit(rect);
        let p = frame.to_screen(&view, rect, 10f64.powf(1.0), 10f64.powf(2.0)).unwrap();
        assert!((p.x - rect.center().x).abs() < 1e-3 && (p.y - rect.center().y).abs() < 1e-3);
        assert!(frame.to_screen(&view, rect, 0.0, 1.0).is_none(), "zero has no log");
        let back = frame.to_data(&view, rect, p);
        assert!((back[0] - 10.0).abs() < 1e-9 && (back[1] - 100.0).abs() < 1e-9);
    }
}
