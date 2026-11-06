//! A residual (or any convergence) history and its plot, hand-painted with
//! [`egui::Painter`] (gh:#1155; planned in
//! `docs/lessons/cfd-and-thermal-hydraulics-track-scoping.md`, gh:#891).
//!
//! [`History`] keeps a bounded number of points per series: when it is full
//! it drops every other point, so a long run keeps its whole shape at half
//! the resolution rather than only its tail. [`plot_points`] maps a series to
//! screen points (log or linear y) and is what the tests check; [`paint`]
//! draws the axes, the series and the legend.
use petir::mathf::RealMath;
use egui::{Pos2, Rect, Stroke};

// Series colours: `Ink::accent(i)` of the painter's theme (blue, orange,
// aqua, purple, …; Gruvbox bright on Dark, faded on Light).

/// Named series sharing one x axis (step or time).
#[derive(Debug, Clone, PartialEq)]
pub struct History {
    pub names: Vec<String>,
    pub x: Vec<f64>,
    pub ys: Vec<Vec<f64>>,
    /// Most points kept per series (at least 16).
    pub cap: usize,
}

impl History {
    /// An empty history of the named series, keeping up to `cap` points.
    pub fn new(names: &[&str], cap: usize) -> Self {
        Self { names: names.iter().map(|s| s.to_string()).collect(), x: Vec::new(), ys: vec![Vec::new(); names.len()], cap: cap.max(16) }
    }

    /// Append one point; `ys` has one value per series (missing ones are NaN).
    pub fn push(&mut self, x: f64, ys: &[f64]) {
        self.x.push(x);
        for (i, s) in self.ys.iter_mut().enumerate() {
            s.push(ys.get(i).copied().unwrap_or(f64::NAN));
        }
        if self.x.len() > self.cap {
            let keep = |v: &mut Vec<f64>| {
                let last = *v.last().unwrap_or(&f64::NAN);
                let mut k: Vec<f64> = v.iter().step_by(2).copied().collect();
                if v.len() % 2 == 0 {
                    k.push(last);
                }
                *v = k;
            };
            keep(&mut self.x);
            for s in &mut self.ys {
                keep(s);
            }
        }
    }

    /// Forget every point.
    pub fn clear(&mut self) {
        self.x.clear();
        for s in &mut self.ys {
            s.clear();
        }
    }

    /// Number of points kept.
    pub fn len(&self) -> usize {
        self.x.len()
    }

    /// No points yet?
    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }

    /// The y range over every series' plottable values (positive ones for a
    /// log axis), or `None` if there are none. A log range is widened to
    /// whole decades.
    pub fn y_range(&self, log_y: bool) -> Option<(f64, f64)> {
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for v in self.ys.iter().flatten().filter(|v| v.is_finite() && (!log_y || **v > 0.0)) {
            let v = if log_y { v.r_log10() } else { *v };
            lo = lo.min(v);
            hi = hi.max(v);
        }
        if !lo.is_finite() {
            return None;
        }
        if log_y {
            lo = lo.floor();
            hi = hi.ceil().max(lo + 1.0);
        } else if !(lo < hi) {
            lo -= 0.5;
            hi += 0.5;
        }
        Some((lo, hi))
    }
}

/// Series `i` of `h` as screen points inside `plot`, y range `(lo, hi)` (in
/// decades for a log axis). Points that cannot be plotted are skipped.
pub fn plot_points(h: &History, i: usize, plot: Rect, log_y: bool, range: (f64, f64)) -> Vec<Pos2> {
    let (x0, x1) = (h.x.first().copied().unwrap_or(0.0), h.x.last().copied().unwrap_or(1.0));
    let xs = if x1 > x0 { x1 - x0 } else { 1.0 };
    h.x.iter()
        .zip(&h.ys[i])
        .filter(|(_, y)| y.is_finite() && (!log_y || **y > 0.0))
        .map(|(x, y)| {
            let yy = if log_y { y.r_log10() } else { *y };
            Pos2::new(
                plot.left() + ((x - x0) / xs) as f32 * plot.width(),
                plot.bottom() - ((yy - range.0) / (range.1 - range.0)) as f32 * plot.height(),
            )
        })
        .collect()
}

/// Draw `h` in `rect`: a framed plot, y gridlines (each decade on a log
/// axis), every series and a legend, with `x_label` under it. Drawing code
/// (the geometry comes from the tested [`plot_points`] and
/// [`History::y_range`]).
pub fn paint(painter: &egui::Painter, rect: Rect, h: &History, log_y: bool, x_label: &str, text_px: f32) {
    let font = egui::FontId::proportional(text_px);
    let plot = Rect::from_min_max(rect.min + egui::vec2(text_px * 3.6, 4.0), rect.max - egui::vec2(6.0, text_px * 2.6));
    let ink = crate::Ink::of_ctx(painter.ctx());
    painter.rect_filled(rect, 4.0, ink.backing);
    if plot.width() < 20.0 || plot.height() < 20.0 {
        return;
    }
    painter.rect_stroke(plot, 0.0, Stroke::new(1.0, ink.border), egui::StrokeKind::Inside);
    let Some(range) = h.y_range(log_y) else {
        painter.text(plot.center(), egui::Align2::CENTER_CENTER, "no data yet", font, ink.muted);
        return;
    };
    let ticks: Vec<f64> = if log_y {
        let step = ((range.1 - range.0) / 6.0).ceil().max(1.0);
        let mut t = Vec::new();
        let mut d = range.0;
        while d <= range.1 + 1e-9 {
            t.push(d);
            d += step;
        }
        t
    } else {
        (0..=4).map(|k| range.0 + (range.1 - range.0) * k as f64 / 4.0).collect()
    };
    for t in ticks {
        let y = plot.bottom() - ((t - range.0) / (range.1 - range.0)) as f32 * plot.height();
        painter.line_segment([Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)], Stroke::new(0.5, ink.grid));
        let label = if log_y { format!("1e{t:.0}") } else { super::field::fmt(t) };
        painter.text(Pos2::new(plot.left() - 3.0, y), egui::Align2::RIGHT_CENTER, label, font.clone(), ink.muted);
    }
    let mut lx = plot.left() + 4.0;
    for i in 0..h.ys.len() {
        let c = ink.accent(i);
        let pts = plot_points(h, i, plot, log_y, range);
        if pts.len() >= 2 {
            painter.add(egui::Shape::line(pts, Stroke::new(1.5, c)));
        }
        let g = painter.layout_no_wrap(h.names[i].clone(), font.clone(), c);
        let w = g.size().x;
        painter.galley(Pos2::new(lx, plot.top() + 2.0), g, c);
        lx += w + 12.0;
    }
    painter.text(Pos2::new(plot.center().x, rect.bottom() - 2.0), egui::Align2::CENTER_BOTTOM, x_label, font, ink.muted);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_decimates_and_keeps_the_last_point() {
        let mut h = History::new(&["a", "b"], 16);
        for k in 0..17 {
            h.push(k as f64, &[10f64.powi(-(k as i32)), 1.0]);
        }
        assert!(h.len() <= 16 && h.len() >= 8, "{}", h.len());
        assert_eq!(h.x[0], 0.0);
        assert_eq!(*h.x.last().unwrap(), 16.0);
        assert_eq!(h.ys[0].len(), h.len());
        h.push(17.0, &[]);
        assert!(h.ys[1].last().unwrap().is_nan());
        h.clear();
        assert!(h.is_empty() && h.y_range(true).is_none());
    }

    #[test]
    fn points_map_into_the_plot() {
        let mut h = History::new(&["r"], 100);
        h.push(0.0, &[1.0]);
        h.push(5.0, &[1e-2]);
        h.push(10.0, &[-1.0]);
        let r = h.y_range(true).unwrap();
        assert_eq!(r, (-2.0, 0.0));
        let plot = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 50.0));
        let p = plot_points(&h, 0, plot, true, r);
        assert_eq!(p.len(), 2, "the negative value is skipped on a log axis");
        assert_eq!(p[0], Pos2::new(0.0, 0.0));
        assert_eq!(p[1], Pos2::new(50.0, 50.0));
        let lin = h.y_range(false).unwrap();
        assert_eq!(lin, (-1.0, 1.0));
        assert_eq!(plot_points(&h, 0, plot, false, lin).len(), 3);
    }
}
