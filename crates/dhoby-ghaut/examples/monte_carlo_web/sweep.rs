//! **A recorded sweep, read with a slider** (gh:#528): the `htr10` rung's
//! k against loading height for N = 10–20 layers. Nothing is computed here:
//! the points are the record's (quoted from its CSV at build time) and the
//! curves are the literature's; the slider picks which row is read out and
//! which bed the geometry view beside it shows.
//!
//! Drawn in the workspace's plotting convention: literature thick and solid,
//! ours dotted, recorded points as markers with ±1σ bars.

use crate::keff::LineStyle;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};

/// One point: `x` on the plot, `k ± sigma`, the slider value it belongs to
/// (if any) and what to say about it when it is selected.
#[derive(Clone, Debug)]
pub struct SweepPoint {
    pub x: f64,
    pub k: f64,
    pub sigma: f64,
    pub tag: Option<f64>,
    pub detail: String,
}

#[derive(Clone, Debug)]
pub struct SweepCurve {
    pub label: String,
    pub style: LineStyle,
    pub colour: Color32,
    pub points: Vec<SweepPoint>,
}

#[derive(Clone, Debug)]
pub struct RecordedSweep {
    pub title: &'static str,
    /// Slider name, range and default (integer steps).
    pub param: &'static str,
    pub range: (f64, f64),
    pub default: f64,
    pub x_label: &'static str,
    pub curves: Vec<SweepCurve>,
    /// What the record is and is not (its deliberate liberties).
    pub notes: Vec<&'static str>,
}

impl RecordedSweep {
    /// The selected points' details, one line each (points with nothing to
    /// say, such as a reference curve's, give no line).
    pub fn details(&self, at: f64) -> Vec<String> {
        self.curves.iter().flat_map(|c| c.points.iter()).filter(|p| p.tag == Some(at) && !p.detail.is_empty()).map(|p| p.detail.clone()).collect()
    }
}

/// The plot, with the points for slider value `at` ringed.
pub fn draw(painter: &egui::Painter, rect: Rect, s: &RecordedSweep, at: f64, text: f32) {
    let f = egui::FontId::proportional(text);
    let small = egui::FontId::proportional((text * 0.85).max(8.0));
    let grey = Color32::from_rgb(170, 176, 190);
    painter.rect_filled(rect, 4.0, Color32::from_rgb(18, 21, 28));
    painter.text(rect.left_top() + Vec2::new(8.0, 5.0), egui::Align2::LEFT_TOP, s.title, f, Color32::WHITE);
    let pts = || s.curves.iter().flat_map(|c| c.points.iter());
    let (mut x0, mut x1, mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for p in pts() {
        x0 = x0.min(p.x);
        x1 = x1.max(p.x);
        y0 = y0.min(p.k - p.sigma);
        y1 = y1.max(p.k + p.sigma);
    }
    if !x0.is_finite() {
        return;
    }
    let (px, py) = (0.03 * (x1 - x0), 0.06 * (y1 - y0));
    let (x0, x1, y0, y1) = (x0 - px, x1 + px, y0 - py, y1 + py);
    let legend_h = (s.curves.len() as f32) * text * 1.3 + 8.0;
    let plot = Rect::from_min_max(rect.left_top() + Vec2::new(text * 3.4, text * 1.8 + 8.0), rect.right_bottom() - Vec2::new(10.0, text * 1.7 + legend_h));
    if plot.height() < 40.0 || plot.width() < 60.0 {
        return;
    }
    let p = |x: f64, y: f64| {
        Pos2::new(plot.left() + ((x - x0) / (x1 - x0)) as f32 * plot.width(), plot.bottom() - ((y - y0) / (y1 - y0)) as f32 * plot.height())
    };
    let gridc = Color32::from_rgb(40, 45, 56);
    let px_per = plot.width() as f64 / (x1 - x0);
    let xstep = [10.0, 20.0, 25.0, 50.0].into_iter().find(|st| st * px_per >= 48.0).unwrap_or(50.0);
    let mut x = (x0 / xstep).ceil() * xstep;
    while x <= x1 {
        let a = p(x, y0);
        painter.line_segment([a, Pos2::new(a.x, plot.top())], Stroke::new(1.0, gridc));
        painter.text(Pos2::new(a.x, plot.bottom() + 2.0), egui::Align2::CENTER_TOP, format!("{x:.0}"), small.clone(), grey);
        x += xstep;
    }
    let ystep = if y1 - y0 > 0.25 { 0.05 } else { 0.02 };
    let mut y = (y0 / ystep).ceil() * ystep;
    while y <= y1 {
        let a = p(x0, y);
        painter.line_segment([a, Pos2::new(plot.right(), a.y)], Stroke::new(1.0, gridc));
        painter.text(Pos2::new(plot.left() - 3.0, a.y), egui::Align2::RIGHT_CENTER, format!("{y:.2}"), small.clone(), grey);
        y += ystep;
    }
    painter.text(Pos2::new(plot.right(), plot.bottom() + text * 1.15 + 2.0), egui::Align2::RIGHT_TOP, s.x_label, small.clone(), grey);
    painter.text(Pos2::new(plot.left() + 4.0, plot.top() + 2.0), egui::Align2::LEFT_TOP, "k_eff", small.clone(), grey);
    let clip = painter.with_clip_rect(plot.expand(6.0));
    // k = 1.
    if y0 < 1.0 && y1 > 1.0 {
        clip.line_segment([p(x0, 1.0), p(x1, 1.0)], Stroke::new(1.0, Color32::from_rgb(90, 96, 110)));
    }
    for c in &s.curves {
        let line: Vec<Pos2> = c.points.iter().map(|q| p(q.x, q.k)).collect();
        match c.style {
            LineStyle::Published => {
                clip.add(egui::Shape::line(line, Stroke::new(3.0, c.colour)));
            }
            LineStyle::Ours => clip.extend(egui::Shape::dotted_line(&line, c.colour, 5.0, 1.4)),
            LineStyle::Reference => clip.extend(egui::Shape::dashed_line(&line, Stroke::new(1.4, c.colour), 7.0, 5.0)),
        }
        if c.style == LineStyle::Ours {
            for q in &c.points {
                clip.line_segment([p(q.x, q.k - q.sigma), p(q.x, q.k + q.sigma)], Stroke::new(1.2, c.colour));
                clip.circle_stroke(p(q.x, q.k), 3.0, Stroke::new(1.3, c.colour));
            }
        }
        for q in c.points.iter().filter(|q| q.tag == Some(at)) {
            clip.circle_stroke(p(q.x, q.k), 7.0, Stroke::new(2.0, Color32::WHITE));
        }
    }
    let mut ly = plot.bottom() + text * 1.7 + 8.0;
    for c in &s.curves {
        let a = Pos2::new(rect.left() + 12.0, ly + text * 0.55);
        let seg = [a, a + Vec2::new(24.0, 0.0)];
        match c.style {
            LineStyle::Published => {
                painter.line_segment(seg, Stroke::new(3.0, c.colour));
            }
            LineStyle::Ours => painter.extend(egui::Shape::dotted_line(&seg, c.colour, 5.0, 1.4)),
            LineStyle::Reference => painter.extend(egui::Shape::dashed_line(&seg, Stroke::new(1.4, c.colour), 7.0, 5.0)),
        }
        painter.text(a + Vec2::new(30.0, 0.0), egui::Align2::LEFT_CENTER, &c.label, small.clone(), Color32::from_rgb(210, 216, 226));
        ly += text * 1.3;
    }
}
