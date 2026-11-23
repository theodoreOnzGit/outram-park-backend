//! Drawing a laid-out [`Diagram`] with egui (feature `egui`).
//!
//! Classic fault-tree line art: every occurrence is a **label box** (name,
//! and an optional detail line such as a probability) with its **symbol**
//! below it: the AND gate's flat-bottomed dome, the OR gate's shield, a
//! circle for a basic event, a house outline for a house event, and a
//! triangle for a transfer (a repeated gate drawn once in full elsewhere).
//! Gates other than AND and OR draw the OR shield with their text inside
//! (`2/n`, `XOR`, `NOT`, ...). Edges are orthogonal elbows from a gate's
//! symbol down to each input's label box.
//!
//! The caller owns the view: it passes a world-to-screen mapping (for
//! example `web_demo::view::View::to_screen`) and the scale, so this module
//! does not depend on `web_demo` and works in any egui app.
//!
//! Every function here is GUI drawing code, exempt from a reaching test
//! (egui paint cannot be driven from a unit test); the geometry it draws
//! from is [`super::layout`]'s, which is tested, and [`node_rect`]'s, which
//! is pure and tested below.

use alloc::vec::Vec;
use egui::epaint::{CubicBezierShape, QuadraticBezierShape};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};

use super::{Diagram, Layout, Mark, Symbol};

/// Width of a node, world units (a fraction of [`super::SLOT_W`], leaving a
/// gap between neighbours).
pub const NODE_W: f64 = 0.86;
/// Height of a node's label box, world units.
pub const LABEL_H: f64 = 0.30;
/// Height of a node's symbol under its label box, world units.
pub const SYMBOL_H: f64 = 0.26;
/// Gap between the label box and the symbol, world units.
pub const SYMBOL_GAP: f64 = 0.02;

/// Colours a drawing uses, taken from the app's theme.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// Lines and text.
    pub ink: Color32,
    /// Label-box fill.
    pub fill: Color32,
    /// Highlighted nodes' outline.
    pub highlight: Color32,
    /// The selected node's outline.
    pub selected: Color32,
    /// Dimmed nodes' lines and text.
    pub dim: Color32,
}

impl Palette {
    /// The palette for an egui theme: text colour for ink, the faint
    /// background for boxes, Gruvbox yellow for a highlight and aqua for the
    /// selection.
    pub fn from_visuals(v: &egui::Visuals) -> Self {
        let ink = v.text_color();
        Self {
            ink,
            fill: v.faint_bg_color,
            highlight: crate::palette::BRIGHT_YELLOW,
            selected: crate::palette::BRIGHT_AQUA,
            dim: ink.gamma_multiply(0.35),
        }
    }
}

/// The world-space rectangle a node occupies around its position `centre`
/// (label box on top, symbol below), as `[min_x, min_y, max_x, max_y]` with
/// y up. Pure; used for hit-testing and for the edges.
pub fn node_rect(centre: [f64; 2]) -> [f64; 4] {
    let top = centre[1] + 0.5 * LABEL_H;
    let bottom = top - LABEL_H - SYMBOL_GAP - SYMBOL_H;
    [centre[0] - 0.5 * NODE_W, bottom, centre[0] + 0.5 * NODE_W, top]
}

/// Draw `layout` of `diagram` with `painter`.
///
/// `to_screen` maps a world point (x, y up) to the screen; `px_per_unit` is
/// the current scale (pixels per world unit), which sets the text size; text
/// is left out below a legible size. `marks` is indexed like
/// [`Layout::placed`] (see [`super::highlight`]); a shorter slice draws the
/// rest plainly.
///
/// Returns each occurrence's screen rectangle, for the caller's
/// hit-testing (index into [`Layout::placed`]).
pub fn draw(
    painter: &Painter,
    diagram: &Diagram,
    layout: &Layout,
    to_screen: impl Fn([f64; 2]) -> Pos2,
    px_per_unit: f32,
    marks: &[Mark],
    palette: Palette,
) -> Vec<Rect> {
    let screen_rect = |w: [f64; 4]| Rect::from_two_pos(to_screen([w[0], w[1]]), to_screen([w[2], w[3]]));
    let line_w = (0.012 * px_per_unit).clamp(1.0, 3.0);
    let colour_of = |m: Mark| match m {
        Mark::Normal => palette.ink,
        Mark::Highlight => palette.highlight,
        Mark::Selected => palette.selected,
        Mark::Dim => palette.dim,
    };
    let mark_at = |i: usize| marks.get(i).copied().unwrap_or(Mark::Normal);

    // Edges first, under the nodes.
    for (i, p) in layout.placed.iter().enumerate() {
        let Some(parent) = p.parent else { continue };
        let parent_box = node_rect(layout.world(parent));
        let child_box = node_rect(layout.world(i));
        let from = [layout.placed[parent].x, parent_box[1]];
        let to = [p.x, child_box[3]];
        let mid_y = 0.5 * (from[1] + to[1]);
        let colour = if mark_at(i) == Mark::Dim { palette.dim } else { colour_of(mark_at(i).min_highlight()) };
        let stroke = Stroke::new(line_w, colour);
        let a = to_screen(from);
        let b = to_screen([from[0], mid_y]);
        let c = to_screen([to[0], mid_y]);
        let d = to_screen(to);
        painter.line_segment([a, b], stroke);
        painter.line_segment([b, c], stroke);
        painter.line_segment([c, d], stroke);
    }

    let font_px = (0.13 * px_per_unit).min(18.0);
    let mut hits = Vec::with_capacity(layout.placed.len());
    for (i, p) in layout.placed.iter().enumerate() {
        let node = &diagram.nodes[p.node];
        let centre = layout.world(i);
        let whole = node_rect(centre);
        hits.push(screen_rect(whole));
        let mark = mark_at(i);
        let colour = colour_of(mark);
        let stroke = Stroke::new(if matches!(mark, Mark::Highlight | Mark::Selected) { 2.0 * line_w } else { line_w }, colour);

        // Label box.
        let label = [whole[0], whole[3] - LABEL_H, whole[2], whole[3]];
        let label_rect = screen_rect(label);
        let fill = if mark == Mark::Dim { palette.fill.gamma_multiply(0.5) } else { palette.fill };
        painter.rect_filled(label_rect, 3.0, fill);
        painter.rect_stroke(label_rect, 3.0, stroke, StrokeKind::Inside);
        // Text shrinks to fit the box's width, so a long name is smaller
        // rather than clipped, and appears once the reader zooms in enough.
        let name_px = fit_text_px(font_px, label_rect.width(), node.label.chars().count());
        if name_px >= 5.5 {
            let text_colour = if mark == Mark::Dim { palette.dim } else { palette.ink };
            let name_at = if node.detail.is_empty() { label_rect.center() } else { label_rect.center() - Vec2::new(0.0, 0.28 * label_rect.height()) };
            let clip = painter.with_clip_rect(label_rect.shrink(1.0));
            clip.text(name_at, Align2::CENTER_CENTER, &node.label, FontId::proportional(name_px), text_colour);
            if !node.detail.is_empty() {
                let detail_at = label_rect.center() + Vec2::new(0.0, 0.25 * label_rect.height());
                let detail_px = fit_text_px(0.85 * font_px, label_rect.width(), node.detail.chars().count());
                clip.text(detail_at, Align2::CENTER_CENTER, &node.detail, FontId::proportional(detail_px), text_colour);
            }
        }

        // Symbol, below the label box.
        let sym = [centre[0] - 0.22 * NODE_W, whole[1], centre[0] + 0.22 * NODE_W, whole[1] + SYMBOL_H];
        let s = screen_rect(sym);
        // Its connector up to the label box.
        painter.line_segment([to_screen([centre[0], sym[3]]), to_screen([centre[0], label[1]])], stroke);
        if p.transfer {
            draw_triangle(painter, s, stroke);
        } else {
            match node.symbol {
                Symbol::And => draw_and(painter, s, stroke),
                Symbol::BasicEvent => {
                    let r = 0.5 * s.height().min(s.width());
                    painter.circle_filled(s.center(), r, fill);
                    painter.circle_stroke(s.center(), r, stroke);
                }
                Symbol::HouseEvent(_) => draw_house(painter, s, stroke),
                Symbol::Or | Symbol::AtLeast(_) | Symbol::Xor | Symbol::Not | Symbol::Nand | Symbol::Nor | Symbol::Null => {
                    draw_or(painter, s, stroke)
                }
            }
            let text = node.symbol.short_text();
            let inside = !matches!(node.symbol, Symbol::And | Symbol::Or | Symbol::BasicEvent);
            if inside && font_px >= 6.0 {
                painter.text(s.center(), Align2::CENTER_CENTER, text, FontId::proportional(0.8 * font_px), colour);
            }
        }
    }
    hits
}

/// The text size, px, that fits `chars` characters of proportional text
/// into `width_px` (an average glyph is taken as 0.58 em wide), capped at
/// `wanted_px`. Pure; tested.
pub fn fit_text_px(wanted_px: f32, width_px: f32, chars: usize) -> f32 {
    if chars == 0 {
        return wanted_px;
    }
    wanted_px.min(0.92 * width_px / (0.58 * chars as f32))
}

impl Mark {
    /// The edge colour class into a highlighted or selected node:
    /// highlighted (a selected node's incoming edge is part of the path).
    fn min_highlight(self) -> Mark {
        match self {
            Mark::Selected => Mark::Highlight,
            other => other,
        }
    }
}

/// AND gate: a flat bottom and a dome over it.
fn draw_and(painter: &Painter, s: Rect, stroke: Stroke) {
    let (l, r, t, b) = (s.left(), s.right(), s.top(), s.bottom());
    painter.add(Shape::CubicBezier(CubicBezierShape::from_points_stroke(
        [Pos2::new(l, b), Pos2::new(l, t - 0.1 * s.height()), Pos2::new(r, t - 0.1 * s.height()), Pos2::new(r, b)],
        false,
        Color32::TRANSPARENT,
        stroke,
    )));
    painter.line_segment([Pos2::new(l, b), Pos2::new(r, b)], stroke);
}

/// OR gate: two curved sides meeting at the top point, and a concave base.
fn draw_or(painter: &Painter, s: Rect, stroke: Stroke) {
    let (l, r, t, b) = (s.left(), s.right(), s.top(), s.bottom());
    let apex = Pos2::new(s.center().x, t);
    let bend = 0.25 * s.height();
    for (base, ctrl) in [(Pos2::new(l, b), Pos2::new(l, t + bend)), (Pos2::new(r, b), Pos2::new(r, t + bend))] {
        painter.add(Shape::QuadraticBezier(QuadraticBezierShape::from_points_stroke(
            [base, ctrl, apex],
            false,
            Color32::TRANSPARENT,
            stroke,
        )));
    }
    painter.add(Shape::QuadraticBezier(QuadraticBezierShape::from_points_stroke(
        [Pos2::new(l, b), Pos2::new(s.center().x, b - bend), Pos2::new(r, b)],
        false,
        Color32::TRANSPARENT,
        stroke,
    )));
}

/// House event: a square with a pitched roof.
fn draw_house(painter: &Painter, s: Rect, stroke: Stroke) {
    let roof = s.top() + 0.4 * s.height();
    let pts = [
        Pos2::new(s.left(), s.bottom()),
        Pos2::new(s.left(), roof),
        Pos2::new(s.center().x, s.top()),
        Pos2::new(s.right(), roof),
        Pos2::new(s.right(), s.bottom()),
    ];
    painter.add(Shape::closed_line(pts.to_vec(), stroke));
}

/// Transfer symbol: a triangle, apex up.
fn draw_triangle(painter: &Painter, s: Rect, stroke: Stroke) {
    let pts = [Pos2::new(s.center().x, s.top()), Pos2::new(s.right(), s.bottom()), Pos2::new(s.left(), s.bottom())];
    painter.add(Shape::closed_line(pts.to_vec(), stroke));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A node's box is centred on its x, NODE_W wide, label on top of the
    /// symbol, with its top LABEL_H/2 above the node's level.
    #[test]
    fn node_rect_stacks_label_over_symbol() {
        let r = node_rect([2.0, -1.0]);
        assert!((r[0] - (2.0 - 0.5 * NODE_W)).abs() < 1e-12);
        assert!((r[2] - (2.0 + 0.5 * NODE_W)).abs() < 1e-12);
        assert!((r[3] - (-1.0 + 0.5 * LABEL_H)).abs() < 1e-12);
        assert!((r[3] - r[1] - (LABEL_H + SYMBOL_GAP + SYMBOL_H)).abs() < 1e-12);
        // A node fits inside one level, so levels do not overlap.
        assert!(r[3] - r[1] < super::super::LEVEL_H);
        assert_eq!(Mark::Selected.min_highlight(), Mark::Highlight);
        assert_eq!(Mark::Dim.min_highlight(), Mark::Dim);
        // Short text keeps its size; long text shrinks to the width.
        assert_eq!(fit_text_px(12.0, 100.0, 4), 12.0);
        let px = fit_text_px(12.0, 100.0, 40);
        assert!(px < 12.0 && (px * 0.58 * 40.0 - 92.0).abs() < 1e-3);
        assert_eq!(fit_text_px(12.0, 100.0, 0), 12.0);
    }
}
