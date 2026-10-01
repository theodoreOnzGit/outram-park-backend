//! Draggable handles on a box: the one corner-drag interaction kovan uses
//! wherever a rectangle is corrected by hand.
//!
//! Maintainer, 2026-10-01: *"when i edit annotations, i should be able to
//! drag the corners of the boxes to correct them."*
//!
//! ## One interaction, two users
//!
//! The digitiser already dragged corners — the axis-aligned reference box's
//! four corners and the parallelogram calibration's four free corners
//! (op-zfnh / op-vyb9, `DigitiseApp::image_panel`). Its hit test (nearest of
//! the corners within a tolerance) and its handle (a 4 px circle, 6 px and
//! yellow while dragged, white rim) were inline in that function. They live
//! here now as [`nearest_handle`] and [`paint_handle`], and **both** the
//! digitiser and the PDF reader's annotation-box edit call them, so the two
//! cannot drift into different gestures.
//!
//! ## The annotation-box edit
//!
//! While an annotation is open in the page-context panel's block editor, the
//! PDF reader draws handles on its `[source] region` box
//! (`PdfReaderState::ui`). Dragging:
//!
//! - a **corner** resizes the box with the opposite corner fixed;
//! - an **edge** moves only that side;
//! - the **interior** moves the whole box, size kept.
//!
//! The result is computed here by [`drag_region`], a pure function of the
//! region at drag start, the grip, and the pointer's start and current
//! positions in normalised page fractions — recomputed from the start every
//! frame, so nothing accumulates. It clamps to the page, keeps every side at
//! least [`MIN_REGION_SIDE`], and builds the result through
//! [`Region::from_corners`], which sorts corners dragged past each other and
//! validates — the same constructor the box-drawing path uses through
//! [`Region::from_pixels`].
//!
//! The screen ↔ page transform is [`normalised_to_screen`] and its exact
//! inverse [`screen_to_normalised`]; `pdf_reader::region_to_screen_rect`, the
//! function every saved box is drawn with, is built on the former, so the
//! handles are hit-tested against exactly the rectangle on screen.
//!
//! Hit testing is in **screen** points ([`HANDLE_GRAB_PX`]), so a handle is
//! the same size to the pointer at every zoom, as the digitiser's `8 / zoom`
//! image-pixel tolerance already was.

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

use crate::artifact::Region;

/// How close, in screen points, the pointer must be to a corner or an edge
/// to grab it. The digitiser's corner tolerance (`8 / zoom` image pixels,
/// i.e. 8 screen points), reused.
pub(super) const HANDLE_GRAB_PX: f32 = 8.0;

/// The smallest side a dragged box may shrink to, as a fraction of the page.
/// Half a percent: about 3 pt on an A4 page, small enough for a footnote
/// marker, large enough that the box can still be grabbed and seen.
/// `Region::is_valid` only demands a non-zero size; this keeps a drag from
/// collapsing a box to a sliver nothing can grab again.
pub(super) const MIN_REGION_SIDE: f64 = 0.005;

/// The nearest of `handles` within `tol` of `(px, py)`, by index. `None`
/// entries (an unplaced corner) are skipped.
///
/// Was `hit_para_corner` inside the digitiser's `image_panel` (op-vyb9); the
/// digitiser and the PDF reader's box edit both call it now.
pub(super) fn nearest_handle(
    handles: &[Option<(f64, f64)>],
    tol: f64,
    px: f64,
    py: f64,
) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, c) in handles.iter().enumerate() {
        if let Some((cx, cy)) = c {
            let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
            if d < tol && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
    }
    best.map(|(i, _)| i)
}

/// Paint one drag handle at screen position `pos`: a blue circle with a
/// white rim, larger and yellow while `active` (being dragged). Constant on
/// screen whatever the zoom. The digitiser's corner handle, shared.
pub(super) fn paint_handle(painter: &egui::Painter, pos: Pos2, active: bool) {
    let radius = if active { 6.0 } else { 4.0 };
    let fill = if active {
        Color32::from_rgb(255, 210, 60)
    } else {
        Color32::from_rgb(60, 120, 255)
    };
    painter.circle_filled(pos, radius, fill);
    painter.circle_stroke(pos, radius, Stroke::new(1.0_f32, Color32::WHITE));
}

/// A corner of a box, in the digitiser's corner order (`top_left`,
/// `top_right`, `bottom_right`, `bottom_left`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Corner {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

impl Corner {
    /// All four, in index order.
    pub(super) const ALL: [Corner; 4] = [
        Corner::TopLeft,
        Corner::TopRight,
        Corner::BottomRight,
        Corner::BottomLeft,
    ];

    /// This corner of a screen rectangle.
    pub(super) fn of_rect(self, r: Rect) -> Pos2 {
        match self {
            Corner::TopLeft => r.left_top(),
            Corner::TopRight => r.right_top(),
            Corner::BottomRight => r.right_bottom(),
            Corner::BottomLeft => r.left_bottom(),
        }
    }
}

/// A side of a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Edge {
    Left,
    Top,
    Right,
    Bottom,
}

/// What part of a box a drag took hold of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Grip {
    /// Resize; the opposite corner stays fixed.
    Corner(Corner),
    /// Move one side only.
    Edge(Edge),
    /// Move the whole box, size kept.
    Body,
}

impl Grip {
    /// The pointer shape that says what this grip will do.
    pub(super) fn cursor(self) -> egui::CursorIcon {
        match self {
            Grip::Corner(Corner::TopLeft | Corner::BottomRight) => egui::CursorIcon::ResizeNwSe,
            Grip::Corner(Corner::TopRight | Corner::BottomLeft) => egui::CursorIcon::ResizeNeSw,
            Grip::Edge(Edge::Left | Edge::Right) => egui::CursorIcon::ResizeHorizontal,
            Grip::Edge(Edge::Top | Edge::Bottom) => egui::CursorIcon::ResizeVertical,
            Grip::Body => egui::CursorIcon::Move,
        }
    }
}

/// Which part of the screen rectangle `r` the pointer at `p` would grab:
/// a corner within `tol` first (via [`nearest_handle`]), then an edge
/// within `tol` along its length, then the interior. `None` outside.
pub(super) fn hit_grip(r: Rect, p: Pos2, tol: f32) -> Option<Grip> {
    let corners = Corner::ALL.map(|c| {
        let q = c.of_rect(r);
        Some((q.x as f64, q.y as f64))
    });
    if let Some(i) = nearest_handle(&corners, tol as f64, p.x as f64, p.y as f64) {
        return Some(Grip::Corner(Corner::ALL[i]));
    }
    let within_x = (r.min.x - tol..=r.max.x + tol).contains(&p.x);
    let within_y = (r.min.y - tol..=r.max.y + tol).contains(&p.y);
    let near = |a: f32, b: f32| (a - b).abs() < tol;
    if within_y && near(p.x, r.min.x) {
        return Some(Grip::Edge(Edge::Left));
    }
    if within_y && near(p.x, r.max.x) {
        return Some(Grip::Edge(Edge::Right));
    }
    if within_x && near(p.y, r.min.y) {
        return Some(Grip::Edge(Edge::Top));
    }
    if within_x && near(p.y, r.max.y) {
        return Some(Grip::Edge(Edge::Bottom));
    }
    r.contains(p).then_some(Grip::Body)
}

/// Push `moving` at least [`MIN_REGION_SIDE`] away from `fixed`, staying in
/// `0..=1`: on the side it is already on, or — when it sits exactly on
/// `fixed` — the side it started on (`start_side`, `+1` or `-1`); if that
/// side has no room (`fixed` is against the page edge), the other one.
fn keep_apart(moving: f64, fixed: f64, start_side: f64) -> f64 {
    if (moving - fixed).abs() >= MIN_REGION_SIDE {
        return moving;
    }
    let side = if moving > fixed {
        1.0
    } else if moving < fixed {
        -1.0
    } else {
        start_side
    };
    let preferred = fixed + side * MIN_REGION_SIDE;
    if (0.0..=1.0).contains(&preferred) {
        preferred
    } else {
        fixed - side * MIN_REGION_SIDE
    }
}

/// Which way `a` lies from `b`, as `+1`/`-1` (`+1` on a tie).
fn side_of(a: f64, b: f64) -> f64 {
    if a < b {
        -1.0
    } else {
        1.0
    }
}

/// The region a drag produces: `start` is the box when the drag began,
/// `grip` what was grabbed, `from` and `to` the pointer's start and current
/// positions in normalised page fractions (unclamped — the pointer may leave
/// the page).
///
/// The pointer's *displacement* is applied, not its position, so grabbing a
/// handle a few points off its exact corner does not make the box jump.
///
/// - **Corner**: that corner moves; the opposite corner is untouched. Dragged
///   past the opposite corner the box flips, and comes back with
///   `x0 < x1`, `y0 < y1` ([`Region::from_corners`] sorts).
/// - **Edge**: only that side moves; same flipping.
/// - **Body**: the whole box translates, size kept, stopped at the page edge.
///
/// Every coordinate is clamped to `0..=1` and every side kept at least
/// [`MIN_REGION_SIDE`]. `None` only when `start` itself is not a valid
/// region.
pub(super) fn drag_region(
    start: Region,
    grip: Grip,
    from: (f64, f64),
    to: (f64, f64),
) -> Option<Region> {
    if !start.is_valid() {
        return None;
    }
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let clamp = |v: f64| v.clamp(0.0, 1.0);
    let Region { x0, y0, x1, y1 } = start;
    // (moving x, fixed x) and (moving y, fixed y); `None` for an axis the
    // grip does not move.
    type Axis = Option<(f64, f64)>;
    let (mx, my): (Axis, Axis) = match grip {
        Grip::Body => {
            let (w, h) = (x1 - x0, y1 - y0);
            let nx0 = (x0 + dx).clamp(0.0, 1.0 - w);
            let ny0 = (y0 + dy).clamp(0.0, 1.0 - h);
            return Region::from_corners((nx0, ny0), (clamp(nx0 + w), clamp(ny0 + h)));
        }
        Grip::Corner(Corner::TopLeft) => (Some((x0, x1)), Some((y0, y1))),
        Grip::Corner(Corner::TopRight) => (Some((x1, x0)), Some((y0, y1))),
        Grip::Corner(Corner::BottomRight) => (Some((x1, x0)), Some((y1, y0))),
        Grip::Corner(Corner::BottomLeft) => (Some((x0, x1)), Some((y1, y0))),
        Grip::Edge(Edge::Left) => (Some((x0, x1)), None),
        Grip::Edge(Edge::Right) => (Some((x1, x0)), None),
        Grip::Edge(Edge::Top) => (None, Some((y0, y1))),
        Grip::Edge(Edge::Bottom) => (None, Some((y1, y0))),
    };
    let moved = |axis: Axis, d: f64| -> Axis {
        axis.map(|(m, f)| (keep_apart(clamp(m + d), f, side_of(m, f)), f))
    };
    // An axis the grip does not move keeps both of the start's edges.
    let (ax, bx) = moved(mx, dx).unwrap_or((x0, x1));
    let (ay, by) = moved(my, dy).unwrap_or((y0, y1));
    Region::from_corners((ax, ay), (bx, by))
}

/// A point in normalised page fractions on 0-based `page`, to screen —
/// the continuous canvas's placement (`page_top = page * (page_px.y * zoom
/// + gap)`, then `origin + point * zoom`), the same arithmetic as
/// `PageView::project`. `pdf_reader::region_to_screen_rect` is built on it.
pub(super) fn normalised_to_screen(
    x: f64,
    y: f64,
    page: usize,
    page_px: Vec2,
    origin: Pos2,
    zoom: f32,
    gap: f32,
) -> Pos2 {
    let page_top = page as f32 * (page_px.y * zoom + gap);
    origin
        + egui::vec2(
            x as f32 * page_px.x * zoom,
            page_top + y as f32 * page_px.y * zoom,
        )
}

/// The exact inverse of [`normalised_to_screen`]: a screen point, read as a
/// position on 0-based `page`, in normalised page fractions. **Not**
/// clamped — a pointer past the page edge gives a value outside `0..=1`,
/// which [`drag_region`] clamps. `None` for a degenerate page size or zoom.
pub(super) fn screen_to_normalised(
    screen: Pos2,
    page: usize,
    page_px: Vec2,
    origin: Pos2,
    zoom: f32,
    gap: f32,
) -> Option<(f64, f64)> {
    if page_px.x <= 0.0 || page_px.y <= 0.0 || zoom <= 0.0 {
        return None;
    }
    let page_top = page as f32 * (page_px.y * zoom + gap);
    let rel = screen - origin;
    Some((
        (rel.x / (page_px.x * zoom)) as f64,
        ((rel.y - page_top) / (page_px.y * zoom)) as f64,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: [f64; 4] = [0.2, 0.3, 0.6, 0.7];

    fn start() -> Region {
        Region::from(START)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn assert_region(r: Region, want: [f64; 4]) {
        let got: [f64; 4] = r.into();
        assert!(
            got.iter().zip(want).all(|(g, w)| close(*g, w)),
            "got {got:?}, want {want:?}"
        );
    }

    /// Dragging each corner by (+0.05, -0.1) moves that corner and no other:
    /// the opposite corner's two coordinates are exactly the start's.
    #[test]
    fn dragging_a_corner_moves_only_that_corner() {
        let d = |c| drag_region(start(), Grip::Corner(c), (0.5, 0.5), (0.55, 0.4)).unwrap();
        assert_region(d(Corner::TopLeft), [0.25, 0.2, 0.6, 0.7]);
        assert_region(d(Corner::TopRight), [0.2, 0.2, 0.65, 0.7]);
        assert_region(d(Corner::BottomRight), [0.2, 0.3, 0.65, 0.6]);
        assert_region(d(Corner::BottomLeft), [0.25, 0.3, 0.6, 0.6]);
    }

    #[test]
    fn dragging_an_edge_moves_only_that_side() {
        let d = |e| drag_region(start(), Grip::Edge(e), (0.5, 0.5), (0.55, 0.4)).unwrap();
        assert_region(d(Edge::Left), [0.25, 0.3, 0.6, 0.7]);
        assert_region(d(Edge::Right), [0.2, 0.3, 0.65, 0.7]);
        assert_region(d(Edge::Top), [0.2, 0.2, 0.6, 0.7]);
        assert_region(d(Edge::Bottom), [0.2, 0.3, 0.6, 0.6]);
    }

    /// The top-left corner dragged past the bottom-right one flips the box:
    /// the result is sorted, and the fixed corner (0.6, 0.7) is still a
    /// corner of it.
    #[test]
    fn crossing_corners_normalises() {
        let r = drag_region(
            start(),
            Grip::Corner(Corner::TopLeft),
            (0.2, 0.3),
            (0.9, 0.95),
        )
        .unwrap();
        assert_region(r, [0.6, 0.7, 0.9, 0.95]);
        assert!(r.x0 < r.x1 && r.y0 < r.y1 && r.is_valid());
    }

    #[test]
    fn a_drag_off_the_page_is_clamped_to_it() {
        let r = drag_region(
            start(),
            Grip::Corner(Corner::BottomRight),
            (0.6, 0.7),
            (1.8, 2.5),
        )
        .unwrap();
        assert_region(r, [0.2, 0.3, 1.0, 1.0]);
        let r = drag_region(
            start(),
            Grip::Corner(Corner::TopLeft),
            (0.2, 0.3),
            (-3.0, -0.4),
        )
        .unwrap();
        assert_region(r, [0.0, 0.0, 0.6, 0.7]);
        // Moving the whole box stops at the page edge with its size kept.
        let r = drag_region(start(), Grip::Body, (0.5, 0.5), (1.5, -1.0)).unwrap();
        assert_region(r, [0.6, 0.0, 1.0, 0.4]);
    }

    #[test]
    fn moving_the_body_keeps_the_size() {
        let r = drag_region(start(), Grip::Body, (0.4, 0.5), (0.3, 0.55)).unwrap();
        assert_region(r, [0.1, 0.35, 0.5, 0.75]);
    }

    /// Dropping a corner exactly on the opposite one, or just short of it,
    /// leaves a box of [`MIN_REGION_SIDE`] on the side it came from — never a
    /// degenerate one — and the fixed corner does not move.
    #[test]
    fn the_minimum_size_holds() {
        for to in [(0.6, 0.7), (0.599, 0.701), (0.6001, 0.6999)] {
            let r = drag_region(start(), Grip::Corner(Corner::TopLeft), (0.2, 0.3), to).unwrap();
            assert!(r.x1 - r.x0 >= MIN_REGION_SIDE - 1e-12, "{r:?}");
            assert!(r.y1 - r.y0 >= MIN_REGION_SIDE - 1e-12, "{r:?}");
            let fixed_x = close(r.x0, 0.6) || close(r.x1, 0.6);
            let fixed_y = close(r.y0, 0.7) || close(r.y1, 0.7);
            assert!(fixed_x && fixed_y, "fixed corner kept: {r:?}");
            assert!(r.is_valid());
        }
        // Collapsed onto the fixed side, the box keeps its minimum size on
        // the side it came from.
        let edge = Region::from([0.0, 0.0, 0.5, 0.5]);
        let r = drag_region(edge, Grip::Edge(Edge::Right), (0.5, 0.2), (-1.0, 0.2)).unwrap();
        assert_region(r, [0.0, 0.0, MIN_REGION_SIDE, 0.5]);
        // Pushed just past a fixed side that is against the page edge, there
        // is no room beyond it, so the minimum size goes on the other side.
        let edge = Region::from([0.5, 0.0, 0.998, 0.5]);
        let r = drag_region(edge, Grip::Edge(Edge::Left), (0.5, 0.2), (0.999, 0.2)).unwrap();
        assert_region(r, [0.998 - MIN_REGION_SIDE, 0.0, 0.998, 0.5]);
    }

    #[test]
    fn an_invalid_start_region_gives_nothing() {
        let bad = Region::from([0.6, 0.3, 0.2, 0.7]);
        assert!(drag_region(bad, Grip::Body, (0.0, 0.0), (0.1, 0.1)).is_none());
    }

    /// Screen → page → screen at zoom 1.75, a scrolled origin and page 2 of
    /// the stack: the inverse is exact, and a page corner lands on 0 / 1.
    #[test]
    fn screen_and_normalised_round_trip_at_a_non_unit_zoom_and_offset() {
        let page_px = Vec2::new(1240.0, 1754.0);
        let origin = Pos2::new(-312.5, -4021.0); // scrolled: content top-left off screen
        let (zoom, gap, page) = (1.75, 16.0, 2);
        for (x, y) in [(0.0, 0.0), (1.0, 1.0), (0.2, 0.3), (0.875, 0.0625)] {
            let s = normalised_to_screen(x, y, page, page_px, origin, zoom, gap);
            let (bx, by) = screen_to_normalised(s, page, page_px, origin, zoom, gap).unwrap();
            assert!(
                (bx - x).abs() < 1e-5 && (by - y).abs() < 1e-5,
                "({x},{y}) -> ({bx},{by})"
            );
        }
        let s = Pos2::new(400.0, 300.0);
        let (x, y) = screen_to_normalised(s, page, page_px, origin, zoom, gap).unwrap();
        let back = normalised_to_screen(x, y, page, page_px, origin, zoom, gap);
        assert!((back - s).length() < 1e-2, "{back:?}");
        assert!(screen_to_normalised(s, 0, page_px, origin, 0.0, gap).is_none());
    }

    /// Handles are hit in screen points: the same 8 pt reach whatever the
    /// zoom, corners before edges before the interior.
    #[test]
    fn hit_grip_prefers_corners_then_edges_then_the_body() {
        let r = Rect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(300.0, 200.0));
        let t = HANDLE_GRAB_PX;
        assert_eq!(
            hit_grip(r, Pos2::new(103.0, 98.0), t),
            Some(Grip::Corner(Corner::TopLeft))
        );
        assert_eq!(
            hit_grip(r, Pos2::new(297.0, 205.0), t),
            Some(Grip::Corner(Corner::BottomRight))
        );
        assert_eq!(
            hit_grip(r, Pos2::new(200.0, 102.0), t),
            Some(Grip::Edge(Edge::Top))
        );
        assert_eq!(
            hit_grip(r, Pos2::new(305.0, 150.0), t),
            Some(Grip::Edge(Edge::Right))
        );
        assert_eq!(hit_grip(r, Pos2::new(200.0, 150.0), t), Some(Grip::Body));
        assert_eq!(hit_grip(r, Pos2::new(50.0, 150.0), t), None);
    }

    #[test]
    fn nearest_handle_picks_the_closest_within_reach() {
        let h = [Some((0.0, 0.0)), None, Some((10.0, 0.0)), Some((6.0, 0.0))];
        assert_eq!(nearest_handle(&h, 8.0, 7.0, 0.0), Some(3));
        assert_eq!(nearest_handle(&h, 8.0, 1.0, 0.0), Some(0));
        assert_eq!(nearest_handle(&h, 0.5, 3.0, 0.0), None);
    }
}
