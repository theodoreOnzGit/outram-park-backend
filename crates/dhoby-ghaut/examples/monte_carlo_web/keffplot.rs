//! **The k_eff and Shannon-entropy plots every run shows** (maintainer,
//! 2026-10-25: "the same keff interface and Shannon entropy under one
//! collapsible bottom panel ... Same as the Godiva keff simulations too!").
//!
//! One drawing for every power iteration in the demo: Run mode on every rung
//! (the serial `Iteration` and the pooled one), and the HTR-10 whole core's
//! worker-pool run. Each converts its generations to [`GenPoint`]s and its
//! references to [`RefLine`]s, then calls [`draw_pair`] in the bottom dock
//! (`dhoby_ghaut::web_demo::dock`).
//!
//! - **k**: each generation's `k` (grey while inactive), the running mean
//!   with its ±σ band over the active ones, and the reference lines (a
//!   recorded value, an experiment, another code), each with its band when it
//!   has a σ.
//! - **entropy**: the fission source's Shannon entropy per generation, from
//!   just below its lowest value to the mesh's ceiling `log2(bins)` when it is
//!   known, so the rise as the source spreads is visible.
//!
//! Both shade the inactive generations. Presentation only: the numbers are
//! the run's own.

use egui::{Color32, Pos2, Rect, Stroke, StrokeKind, Vec2};

/// A generation's `k`, as a dot.
pub const K_COLOUR: Color32 = Color32::from_rgb(120, 170, 255);
/// The running mean.
pub const MEAN_COLOUR: Color32 = Color32::from_rgb(255, 200, 80);
/// The entropy line.
pub const ENTROPY_COLOUR: Color32 = Color32::from_rgb(120, 220, 160);
/// A recorded value of this model (blue, as the layers plot draws ours).
pub const RECORD_COLOUR: Color32 = Color32::from_rgb(120, 170, 255);
/// A reference from outside: experiment, another code.
pub const REFERENCE_COLOUR: Color32 = Color32::from_rgb(230, 230, 230);

/// One generation of a run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenPoint {
    /// From 0.
    pub index: usize,
    pub k: f64,
    pub active: bool,
    /// Mean and standard error over the active generations so far.
    pub k_mean: Option<(f64, f64)>,
    /// Bits.
    pub entropy: Option<f64>,
}

impl From<&outram_mc_libs::physics::keff::GenerationReport> for GenPoint {
    fn from(g: &outram_mc_libs::physics::keff::GenerationReport) -> Self {
        Self { index: g.index, k: g.k, active: g.active, k_mean: g.k_mean, entropy: g.entropy }
    }
}

impl From<&outram_mc_libs::physics::transport_csg::distributed::DistributedGeneration> for GenPoint {
    fn from(g: &outram_mc_libs::physics::transport_csg::distributed::DistributedGeneration) -> Self {
        Self { index: g.index, k: g.k, active: g.active, k_mean: g.k_mean, entropy: g.entropy }
    }
}

/// A horizontal reference on the k plot.
#[derive(Clone, Debug, PartialEq)]
pub struct RefLine {
    pub label: String,
    pub k: f64,
    /// Drawn as a band when given.
    pub sigma: Option<f64>,
    pub colour: Color32,
}

/// The run's shape: generations in all and how many are inactive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunShape {
    pub n_inactive: usize,
    pub total: usize,
}

/// A plot frame with linear axes.
struct Axes {
    rect: Rect,
    x: (f64, f64),
    y: (f64, f64),
}

impl Axes {
    fn p(&self, x: f64, y: f64) -> Pos2 {
        let fx = ((x - self.x.0) / (self.x.1 - self.x.0)) as f32;
        let fy = ((y - self.y.0) / (self.y.1 - self.y.0)) as f32;
        Pos2::new(self.rect.left() + fx * self.rect.width(), self.rect.bottom() - fy * self.rect.height())
    }
    fn frame(&self, painter: &egui::Painter, title: &str, text: f32, fmt: impl Fn(f64) -> String) {
        painter.rect_filled(self.rect, 4.0, Color32::from_rgb(20, 23, 30));
        painter.rect_stroke(self.rect, 4.0, Stroke::new(1.0, Color32::from_rgb(60, 66, 80)), StrokeKind::Inside);
        let f = egui::FontId::proportional((text * 0.85).max(8.0));
        let c = Color32::from_rgb(170, 176, 190);
        painter.text(self.rect.left_top() + Vec2::new(6.0, 3.0), egui::Align2::LEFT_TOP, title, egui::FontId::proportional(text * 0.92), Color32::WHITE);
        painter.text(self.rect.right_top() + Vec2::new(-4.0, 3.0), egui::Align2::RIGHT_TOP, fmt(self.y.1), f.clone(), c);
        painter.text(self.rect.right_bottom() + Vec2::new(-4.0, -3.0), egui::Align2::RIGHT_BOTTOM, fmt(self.y.0), f, c);
    }
    /// Shade the inactive generations `[0.5, n_inactive + 0.5]`.
    fn shade_inactive(&self, painter: &egui::Painter, n_inactive: usize, text: f32) {
        if n_inactive == 0 {
            return;
        }
        let a = self.p(0.5, self.y.1);
        let b = self.p(n_inactive as f64 + 0.5, self.y.0);
        let r = Rect::from_two_pos(a, b).intersect(self.rect);
        painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(120, 120, 140, 40));
        painter.text(r.center_bottom() + Vec2::new(0.0, -3.0), egui::Align2::CENTER_BOTTOM, "inactive", egui::FontId::proportional((text * 0.77).max(8.0)), Color32::from_rgb(150, 150, 170));
    }
}

/// The k plot's vertical range: every generation from the third on (the
/// first come from the guessed source and would squash the rest; they are
/// drawn clamped to the frame), every reference with its band, and at least
/// ±0.02 about 1, padded by a tenth.
pub fn k_range(pts: &[GenPoint], refs: &[RefLine]) -> (f64, f64) {
    let mut lo = 0.98f64;
    let mut hi = 1.02f64;
    for g in pts.iter().filter(|g| g.index >= 2) {
        lo = lo.min(g.k);
        hi = hi.max(g.k);
    }
    for r in refs {
        let s = r.sigma.unwrap_or(0.0);
        lo = lo.min(r.k - s);
        hi = hi.max(r.k + s);
    }
    let pad = 0.1 * (hi - lo);
    (lo - pad, hi + pad)
}

/// `k` per generation (dots), the running mean ± σ (band) and the
/// references.
pub fn draw_k(painter: &egui::Painter, rect: Rect, pts: &[GenPoint], shape: RunShape, refs: &[RefLine], text: f32) {
    let n = shape.total.max(2) as f64;
    let ax = Axes { rect, x: (0.5, n + 0.5), y: k_range(pts, refs) };
    ax.frame(painter, "k per generation (dots) · running mean ± σ (band)", text, |v| format!("{v:.3}"));
    ax.shade_inactive(painter, shape.n_inactive, text);
    // References: the labels alternate ends so close lines stay readable.
    for (i, r) in refs.iter().enumerate() {
        if let Some(s) = r.sigma {
            let band = Rect::from_two_pos(ax.p(0.5, r.k + s), ax.p(n + 0.5, r.k - s)).intersect(rect);
            painter.rect_filled(band, 0.0, Color32::from_rgba_unmultiplied(r.colour.r(), r.colour.g(), r.colour.b(), 24));
        }
        painter.line_segment([ax.p(0.5, r.k), ax.p(n + 0.5, r.k)], Stroke::new(1.2, r.colour));
        let (at, align) = if i % 2 == 0 {
            (ax.p(0.5, r.k) + Vec2::new(4.0, -2.0), egui::Align2::LEFT_BOTTOM)
        } else {
            (ax.p(n + 0.5, r.k) + Vec2::new(-4.0, 2.0), egui::Align2::RIGHT_TOP)
        };
        painter.text(at, align, &r.label, egui::FontId::proportional((text * 0.8).max(8.0)), r.colour);
    }
    let means: Vec<(f64, f64, f64)> = pts.iter().filter_map(|g| g.k_mean.filter(|m| m.1 > 0.0).map(|(m, e)| (g.index as f64 + 1.0, m, e))).collect();
    if means.len() > 1 {
        let upper: Vec<Pos2> = means.iter().map(|&(x, m, e)| ax.p(x, m + e)).collect();
        let lower: Vec<Pos2> = means.iter().map(|&(x, m, e)| ax.p(x, m - e)).collect();
        for w in 0..means.len() - 1 {
            let quad = vec![upper[w], upper[w + 1], lower[w + 1], lower[w]];
            painter.add(egui::Shape::convex_polygon(quad, Color32::from_rgba_unmultiplied(255, 200, 80, 50), Stroke::NONE));
        }
        let line: Vec<Pos2> = means.iter().map(|&(x, m, _)| ax.p(x, m)).collect();
        painter.add(egui::Shape::line(line, Stroke::new(2.0, MEAN_COLOUR)));
    }
    for g in pts {
        let c = if g.active { K_COLOUR } else { Color32::from_rgb(110, 115, 130) };
        painter.circle_filled(ax.p(g.index as f64 + 1.0, g.k.clamp(ax.y.0, ax.y.1)), 2.2, c);
    }
    if pts.is_empty() {
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, "k appears here once the run starts", egui::FontId::proportional(text), Color32::from_rgb(140, 146, 160));
    }
}

/// The entropy plot's vertical range: from just below the lowest value (a
/// whole bit under, floored at 0) to `ceiling` (the mesh's `log2(bins)`)
/// when known, else just above the highest.
pub fn entropy_range(pts: &[GenPoint], ceiling: Option<f64>) -> (f64, f64) {
    let hs: Vec<f64> = pts.iter().filter_map(|g| g.entropy).collect();
    let top = ceiling.unwrap_or_else(|| hs.iter().copied().fold(1.0, f64::max) + 0.5);
    let lowest = hs.iter().copied().fold(top, f64::min);
    ((lowest - 0.5).floor().max(0.0), top)
}

/// Entropy per generation.
pub fn draw_entropy(painter: &egui::Painter, rect: Rect, pts: &[GenPoint], shape: RunShape, ceiling: Option<f64>, text: f32) {
    let n = shape.total.max(2) as f64;
    let ax = Axes { rect, x: (0.5, n + 0.5), y: entropy_range(pts, ceiling) };
    ax.frame(painter, "Shannon entropy of the fission source (bits)", text, |v| format!("{v:.1}"));
    ax.shade_inactive(painter, shape.n_inactive, text);
    let line: Vec<Pos2> = pts.iter().filter_map(|g| g.entropy.map(|h| ax.p(g.index as f64 + 1.0, h))).collect();
    if line.len() > 1 {
        painter.add(egui::Shape::line(line.clone(), Stroke::new(1.5, ENTROPY_COLOUR)));
    }
    for p in line {
        painter.circle_filled(p, 2.0, ENTROPY_COLOUR);
    }
}

/// Both plots in `rect`: side by side (k wider) on a wide box, k above the
/// entropy on a narrow one, so both are always on screen.
pub fn draw_pair(painter: &egui::Painter, rect: Rect, pts: &[GenPoint], shape: RunShape, refs: &[RefLine], ceiling: Option<f64>, text: f32) {
    let (k, h) = pair_rects(rect);
    draw_k(painter, k, pts, shape, refs, text);
    draw_entropy(painter, h, pts, shape, ceiling, text);
}

/// Where [`draw_pair`] puts the two plots.
pub fn pair_rects(rect: Rect) -> (Rect, Rect) {
    if rect.width() < 640.0 {
        let (a, b) = rect.split_top_bottom_at_fraction(0.55);
        (a.with_max_y(a.bottom() - 3.0), b.with_min_y(b.top() + 3.0))
    } else {
        let (a, b) = rect.split_left_right_at_fraction(0.62);
        (a.with_max_x(a.right() - 4.0), b.with_min_x(b.left() + 4.0))
    }
}

/// A status line in the text pane: what it says and its colour.
pub type StatusLine = (String, Color32);

/// Status text: white.
pub const STATUS: Color32 = Color32::from_rgb(220, 226, 236);
/// Live, good news: green.
pub const LIVE: Color32 = Color32::from_rgb(130, 230, 140);
/// Recorded or a caveat: amber.
pub const AMBER: Color32 = Color32::from_rgb(250, 200, 80);
/// Failure: red.
pub const RED: Color32 = Color32::from_rgb(255, 110, 110);

/// Where every run's bottom-dock body puts things (maintainer 2026-10-26:
/// "all text should be in the bottom panel ... consistent for all keff
/// simulation"): `(plots, text pane, one-line strip)`. The strip is the
/// dock's clear band along its bottom, left of the page's References
/// button; the text pane is beside the plots on a wide body, under them on
/// a narrow one.
pub fn body_layout(rect: Rect) -> (Rect, Rect, Rect) {
    use dhoby_ghaut::web_demo::dock::{REFS_CLEAR_PX, REFS_W_PX};
    let strip = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - REFS_CLEAR_PX + 6.0), Pos2::new((rect.right() - REFS_W_PX - 8.0).max(rect.left() + 80.0), rect.bottom()));
    let rest = rect.with_max_y(rect.bottom() - REFS_CLEAR_PX);
    let (plots, pane) = if rest.width() >= 900.0 {
        let (a, b) = rest.split_left_right_at_fraction(0.62);
        (a.with_max_x(a.right() - 4.0), b.with_min_x(b.left() + 4.0))
    } else {
        let (a, b) = rest.split_top_bottom_at_fraction(if rest.width() < 640.0 { 0.62 } else { 0.58 });
        (a.with_max_y(a.bottom() - 3.0), b.with_min_y(b.top() + 3.0))
    };
    (plots, pane, strip)
}

/// The text pane: the run's status lines (wrapped, coloured), then its
/// console rows (monospace, newest at the bottom, scrolling), in one
/// scroll area. Only the visible console rows are laid out: a run of up to
/// 5000 generations has a row each, and laying out every one each frame
/// would lag a phone. GUI drawing, not reached by a test (exempt); its
/// place is [`body_layout`]'s, which is.
pub fn draw_text_pane(ui: &mut egui::Ui, painter: &egui::Painter, rect: Rect, status: &[StatusLine], console: &[String], placeholder: &str, text: f32, id: &str) {
    painter.rect_filled(rect, 4.0, Color32::from_rgb(8, 9, 12));
    if status.is_empty() && console.is_empty() {
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, placeholder, egui::FontId::proportional(text), Color32::from_rgb(140, 146, 160));
        return;
    }
    let mono = egui::FontId::monospace(text);
    let row_h = painter.layout_no_wrap("0".into(), mono, Color32::WHITE).size().y;
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(6.0)), |ui| {
        egui::ScrollArea::vertical().id_salt(id).stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
            for (t, c) in status {
                ui.label(egui::RichText::new(t).size(text).color(*c));
            }
            if !console.is_empty() {
                ui.add_space(4.0);
                // Rows past the visible window are not laid out.
                let first = ui.clip_rect().top();
                let top = ui.cursor().top();
                let skip = (((first - top) / row_h).floor().max(0.0) as usize).min(console.len());
                let show = ((ui.clip_rect().height() / row_h).ceil() as usize + 2).min(console.len() - skip);
                ui.add_space(skip as f32 * row_h);
                for l in &console[skip..skip + show] {
                    ui.add(egui::Label::new(egui::RichText::new(l).monospace().size(text).color(Color32::from_rgb(210, 216, 226))).extend());
                }
                ui.add_space((console.len() - skip - show) as f32 * row_h);
            }
        });
    });
}

/// The one-line strip: wrapped to the strip's width.
pub fn draw_strip(painter: &egui::Painter, strip: Rect, line: String, colour: Color32, text: f32) {
    let g = painter.layout(line, egui::FontId::proportional((text * 0.85).max(9.0)), colour, strip.width().max(80.0));
    painter.galley(strip.min, g, colour);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(index: usize, k: f64, entropy: Option<f64>) -> GenPoint {
        GenPoint { index, k, active: index >= 2, k_mean: None, entropy }
    }

    /// The first two generations do not set the range; a reference with its
    /// band always fits.
    #[test]
    fn the_k_range_skips_the_guessed_source_and_holds_the_references() {
        let pts = [g(0, 0.5, None), g(1, 0.7, None), g(2, 1.001, None), g(3, 0.999, None)];
        let (lo, hi) = k_range(&pts, &[]);
        assert!(lo > 0.9 && hi < 1.1, "{lo} {hi}");
        let refs = [RefLine { label: "RMC".into(), k: 1.05, sigma: Some(0.01), colour: REFERENCE_COLOUR }];
        let (_, hi) = k_range(&pts, &refs);
        assert!(hi > 1.06, "{hi}");
    }

    /// The entropy axis reaches the mesh's ceiling and starts below the
    /// lowest value; with no ceiling it is set by the data.
    #[test]
    fn the_entropy_range_reaches_the_ceiling() {
        let pts = [g(0, 1.0, Some(3.2)), g(1, 1.0, Some(5.1))];
        let c = 6.0;
        assert_eq!(entropy_range(&pts, Some(c)), (2.0, 6.0));
        let (lo, hi) = entropy_range(&pts, None);
        assert!(lo <= 3.2 && hi > 5.1);
    }

    /// The body: plots, text pane and strip do not overlap, the strip
    /// clears the References button, and the pane is beside the plots only
    /// on a wide body.
    #[test]
    fn the_body_has_room_for_plots_text_and_the_references_button() {
        use dhoby_ghaut::web_demo::dock::{REFS_CLEAR_PX, REFS_W_PX};
        for (w, wide) in [(380.0, false), (1000.0, true)] {
            let body = Rect::from_min_size(Pos2::ZERO, Vec2::new(w, 320.0));
            let (plots, pane, strip) = body_layout(body);
            assert!(plots.intersect(pane).area() <= 0.0 && plots.bottom() <= strip.top() && pane.bottom() <= strip.top());
            assert!(strip.right() <= w - REFS_W_PX && strip.top() >= 320.0 - REFS_CLEAR_PX);
            assert_eq!(pane.left() > plots.left(), wide, "{w}");
        }
    }

    /// Narrow: stacked; wide: side by side. Never overlapping.
    #[test]
    fn the_pair_stacks_on_a_phone() {
        let phone = Rect::from_min_size(Pos2::ZERO, Vec2::new(380.0, 300.0));
        let (k, h) = pair_rects(phone);
        assert!(k.bottom() <= h.top() && k.width() == h.width());
        let wide = Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 260.0));
        let (k, h) = pair_rects(wide);
        assert!(k.right() <= h.left() && k.width() > h.width());
    }
}
