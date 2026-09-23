//! Geometry of the Mindmap page's star view and its pan/zoom viewport
//! (GitHub issue #243, epic #241). GUI-free, so it is tested headlessly and
//! builds everywhere this crate does.
//!
//! What belongs here: where each card of the drill-in star goes
//! ([`star_positions`]), the star's extent ([`star_bounds`]), and the
//! arithmetic of the scrollable canvas it is drawn on ([`CanvasLayout`]):
//! how big the canvas is at a zoom, where a world point lands on it, and the
//! **pan limit**: 25 % of the map's own size past each edge (maintainer
//! direction, 2026-09-22), plus half a viewport width sideways (added the same
//! day, for more horizontal room; see [`HORIZONTAL_PAN_VIEWPORT_FRACTION`]).
//!
//! What does not belong here: drawing, input, or which nodes exist. The egui
//! page is [`crate::mindmap`]; the node set comes from the knowledge index.
//!
//! # Why not `crate::mindmap_layout::Camera`
//!
//! That camera maps world space straight onto the viewport, with no scroll
//! bars and nothing that limits panning. The page instead draws on an
//! `egui::ScrollArea` canvas, which gives both, with the same approach as the
//! `htgr_sim_v1` v1.1 plant view in `outram-park-digital-twin-engine`
//! (`examples/htgr_sim_v1/app/plant_v1_1.rs`, `content_layout`). The logic is
//! ported, not shared: that crate is not a dependency of this one. The
//! difference is the margin: there it is 40 % of the viewport, here 25 % of
//! the content plus, horizontally, half the viewport.
//!
//! # Units
//!
//! World units are points at zoom 1. A card is [`CARD_SIZE`] world units; at
//! zoom `z` it is drawn `z` times that size, text included.

use crate::mindmap_layout::{Bounds, Point};
use std::f64::consts::PI;

/// A card's size in world units (points at zoom 1). A drawing choice.
pub const CARD_SIZE: (f64, f64) = (170.0, 46.0);

/// Least clear space between two cards, world units. A drawing choice.
pub const CARD_GAP: f64 = 24.0;

/// How far past each edge of the map the view can pan, as a fraction of the
/// map's own width and height (maintainer direction, 2026-09-22: "don't let
/// me scroll past 25% of where the mindmap content is").
pub const PAN_MARGIN_FRACTION: f64 = 0.25;

/// Extra horizontal pan room, as a fraction of the viewport width, added on
/// each side on top of [`PAN_MARGIN_FRACTION`] (maintainer direction,
/// 2026-09-22: "give me more horizontal space to pan around"). Half a
/// viewport lets either side edge of the map be brought to the middle of the
/// screen, even at Fit, where the map is usually narrower than the window and
/// the content margin alone left nothing to scroll sideways. Vertical pan
/// room is unchanged. A viewing margin, nothing physical.
pub const HORIZONTAL_PAN_VIEWPORT_FRACTION: f64 = 0.5;

/// The diagonal of a card: the least centre-to-centre distance at which two
/// cards cannot overlap, whatever direction one lies from the other.
fn card_diagonal() -> f64 {
    CARD_SIZE.0.hypot(CARD_SIZE.1)
}

/// Where the `n` cards around the centre of the star go, in world units, with
/// the centre card (the concept you are on) at the origin.
///
/// The cards sit evenly on one ring, the first at the top and the rest
/// clockwise. The ring is as small as it can be without two cards touching
/// (checked in `no_two_cards_overlap`):
///
/// - neighbours on the ring are a chord `2 r sin(pi / n)` apart, which must be
///   at least a card diagonal plus [`CARD_GAP`];
/// - every ring card must also clear the centre card, so `r` is at least one
///   diagonal plus the gap.
///
/// Measuring by the diagonal is conservative (cards are wider than tall), but
/// it holds in every direction, so there is no angle at which two cards meet.
pub fn star_positions(n: usize) -> Vec<Point> {
    star_positions_offset(n, 0.0)
}

/// [`star_positions`], with every card turned `offset` radians clockwise
/// around the centre. The radius is unchanged, so the cards stay exactly as
/// far apart as they were — only where the *gaps* between them fall moves.
fn star_positions_offset(n: usize, offset: f64) -> Vec<Point> {
    if n == 0 {
        return Vec::new();
    }
    let spacing = card_diagonal() + CARD_GAP;
    let for_neighbours = if n == 1 {
        0.0
    } else {
        spacing / (2.0 * (PI / n as f64).sin())
    };
    let r = for_neighbours.max(spacing);
    (0..n)
        .map(|i| {
            // Angle from straight up, clockwise (screen y grows downward).
            let a = offset + 2.0 * PI * i as f64 / n as f64;
            Point::new(r * a.sin(), -r * a.cos())
        })
        .collect()
}

/// Where a whole star goes when some ring cards are expanded in place
/// (GitHub issue #246): `ring[i]` is ring card `i`, and `fans[i]` holds the
/// positions of its sub-concepts (empty unless card `i` is expanded).
#[derive(Debug, Clone, PartialEq)]
pub struct StarLayout {
    pub ring: Vec<Point>,
    pub fans: Vec<Vec<Point>>,
}

impl StarLayout {
    /// Every card centre in the layout: ring, then each fan in order.
    pub fn all_points(&self) -> impl Iterator<Item = Point> + '_ {
        self.ring
            .iter()
            .copied()
            .chain(self.fans.iter().flatten().copied())
    }
}

/// Sub-concepts of the ring card at `parent`, fanned outward from the centre
/// of the star, `n` of them.
///
/// They sit on a half-circle around the parent card facing away from the
/// centre (one card goes straight out), at the smallest radius at which
/// neighbours on the arc and the parent card all keep a card diagonal plus
/// [`CARD_GAP`] apart.
fn fan_positions(parent: Point, n: usize) -> Vec<Point> {
    if n == 0 {
        return Vec::new();
    }
    let spacing = card_diagonal() + CARD_GAP;
    // Outward direction: away from the star's centre (the origin).
    let len = parent.x.hypot(parent.y);
    let (ux, uy) = if len > 0.0 {
        (parent.x / len, parent.y / len)
    } else {
        (0.0, -1.0)
    };
    let out = uy.atan2(ux);
    if n == 1 {
        return vec![Point::new(parent.x + spacing * ux, parent.y + spacing * uy)];
    }
    let span = PI; // a half-circle facing outward
    let step = span / (n - 1) as f64;
    let r = (spacing / (2.0 * (0.5 * step).sin())).max(spacing);
    (0..n)
        .map(|k| {
            let a = out - 0.5 * span + step * k as f64;
            Point::new(parent.x + r * a.cos(), parent.y + r * a.sin())
        })
        .collect()
}

/// Whether two cards centred at `a` and `b` overlap (touching counts), with
/// [`CARD_GAP`] of clearance required between them.
fn cards_collide(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < CARD_SIZE.0 + CARD_GAP && (a.y - b.y).abs() < CARD_SIZE.1 + CARD_GAP
}

/// Lay out a star whose ring card `i` has `fan_sizes[i]` sub-concepts shown
/// (zero for a collapsed card), with no two cards overlapping.
///
/// The ring starts at the radius [`star_positions`] would use. Expanded fans
/// can reach into their neighbours, so while any two cards (centre, ring or
/// fan) are closer than a card plus [`CARD_GAP`], the ring radius grows by
/// 10 % and everything is placed again. This always ends: neighbouring ring
/// cards separate in proportion to the radius while each fan keeps its size.
/// The loop is capped at [`MAX_RING_GROWTH_STEPS`] as a guard, never reached
/// in the tested range.
pub fn star_layout(fan_sizes: &[usize]) -> StarLayout {
    star_layout_offset(fan_sizes, 0.0)
}

/// [`star_layout`], rotated half a step when the "up one level" card is drawn
/// above the star (GitHub issue #283).
///
/// [`star_positions`] puts ring card 0 exactly straight up, which is where
/// the dotted connector to the parent card and the Up button on it go — the
/// card would cover both. Turning the ring by half a step (`pi / n`) puts the
/// *gap* between two ring cards at the top instead, for every `n`.
///
/// ~~Radii are untouched, so no card moves relative to any other.~~
/// **CORRECTED 2026-09-23** — a rotated star can come out with a *larger*
/// ring: [`cards_collide`] compares axis-aligned boxes, which is not
/// rotation-invariant (cards are 170 x 46, so two of them side by side need
/// far more room than two stacked), so the growth loop can fire for the
/// turned ring where it did not for the straight one. Seen at `n = 10`. The
/// invariant that holds is the one that matters: no two cards overlap, and no
/// card sits in the corridor straight up.
///
/// `has_parent = false` is exactly [`star_layout`].
pub fn star_layout_with_parent(fan_sizes: &[usize], has_parent: bool) -> StarLayout {
    let n = fan_sizes.len();
    let offset = if has_parent && n > 0 {
        PI / n as f64
    } else {
        0.0
    };
    star_layout_offset(fan_sizes, offset)
}

fn star_layout_offset(fan_sizes: &[usize], offset: f64) -> StarLayout {
    let n = fan_sizes.len();
    let base = star_positions_offset(n, offset);
    let base_r = base.first().map(|p| p.x.hypot(p.y)).unwrap_or(0.0);
    let mut scale = 1.0;
    let mut layout = StarLayout {
        ring: Vec::new(),
        fans: Vec::new(),
    };
    for _ in 0..MAX_RING_GROWTH_STEPS {
        let ring: Vec<Point> = base
            .iter()
            .map(|p| Point::new(p.x * scale, p.y * scale))
            .collect();
        let fans: Vec<Vec<Point>> = ring
            .iter()
            .zip(fan_sizes)
            .map(|(p, &m)| fan_positions(*p, m))
            .collect();
        layout = StarLayout { ring, fans };
        let mut cards: Vec<Point> = vec![Point::new(0.0, 0.0)];
        cards.extend(layout.all_points());
        let clash = cards
            .iter()
            .enumerate()
            .any(|(i, a)| cards[i + 1..].iter().any(|b| cards_collide(*a, *b)));
        if !clash || base_r == 0.0 {
            break;
        }
        scale *= 1.1;
    }
    layout
}

/// The Up button's box, in world units — the box drawn **on** the dotted
/// connector between the centre card and the parent card (#283). A drawing
/// choice, sized to hold one arrow glyph.
pub const UP_BUTTON_SIZE: (f64, f64) = (30.0, 26.0);

/// Where the "up one level" card goes: straight above the star, one card
/// spacing clear of the furthest card in `layout` (#283).
///
/// Measuring from the **furthest** card, not the ring radius, is what keeps
/// it clear of an expanded fan that reaches further out than the ring does.
/// One spacing (a card diagonal plus [`CARD_GAP`]) is enough on its own: any
/// card near enough on the x axis to matter lies at most `furthest` from the
/// centre, so it is at least a spacing below this card on the y axis, and
/// [`cards_collide`] needs both axes to be close.
pub fn parent_position(layout: &StarLayout) -> Point {
    let furthest = layout
        .all_points()
        .map(|p| p.x.hypot(p.y))
        .fold(0.0_f64, f64::max);
    Point::new(0.0, -(furthest + card_diagonal() + CARD_GAP))
}

/// Where the Up button's box sits on the dotted run between the centre card's
/// top edge and the parent card's bottom edge (#283).
///
/// The middle of that run, **but never nearer the star than the furthest card
/// reaches**. Half-way is the obvious place and is where it ends up when
/// there is no ring at all, but an expanded fan can swing a card up to within
/// a few points of the corridor — one was found sitting exactly on the button
/// at `n = 3` — and a button drawn under a card cannot be pressed. Pushing it
/// past the furthest card's radius clears *every* card whatever its angle:
/// each one has `|y| <= furthest`, so the gap on the y axis alone is then
/// more than half the two boxes' heights, and [`cards_collide`]-style overlap
/// needs both axes to be close.
pub fn up_button_centre(layout: &StarLayout) -> Point {
    let parent = parent_position(layout);
    let top_of_centre = -0.5 * CARD_SIZE.1;
    let bottom_of_parent = parent.y + 0.5 * CARD_SIZE.1;
    let middle = 0.5 * (top_of_centre + bottom_of_parent);
    let furthest = layout
        .all_points()
        .map(|p| p.x.hypot(p.y))
        .fold(0.0_f64, f64::max);
    let clear = -(furthest + 0.5 * (CARD_SIZE.1 + UP_BUTTON_SIZE.1) + CARD_GAP);
    // Both are negative (up the screen); the further one is the smaller.
    Point::new(0.0, middle.min(clear))
}

/// Guard on [`star_layout`]'s ring growth: 1.1^200 is about 2e8, far past any
/// radius a real star needs.
pub const MAX_RING_GROWTH_STEPS: usize = 200;

/// A connector between two cards: a cubic Bézier curve `[start, control 1,
/// control 2, end]` in world units, from an edge of one card to an edge of
/// the other (maintainer direction, 2026-09-22: "a smooth curved line from
/// edge to edge, not to the centre of the box").
///
/// **Which edges** depends on where the cards sit relative to each other:
/// the connector runs along the axis with the larger clear gap between them.
/// Side by side, it leaves the facing left/right edges at their midpoints;
/// one above the other, the facing top/bottom edges. It leaves and arrives at
/// right angles to those edges, the control points pulled out along the same
/// axis by half the distance between the ends (at least [`CARD_GAP`]), which
/// is what makes it read as one smooth S or arc.
///
/// `None` when the cards overlap on both axes (possible with pinned cards):
/// there is no clear edge to join, and a curve through the cards would be
/// noise. A drawing rule, nothing physical.
pub fn connector(a: Point, b: Point) -> Option<[Point; 4]> {
    let (w, h) = CARD_SIZE;
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let gap_x = dx.abs() - w;
    let gap_y = dy.abs() - h;
    if gap_x < 0.0 && gap_y < 0.0 {
        return None;
    }
    if gap_x >= gap_y {
        let s = dx.signum();
        let start = Point::new(a.x + s * 0.5 * w, a.y);
        let end = Point::new(b.x - s * 0.5 * w, b.y);
        let pull = (0.5 * (end.x - start.x).abs()).max(CARD_GAP);
        Some([
            start,
            Point::new(start.x + s * pull, start.y),
            Point::new(end.x - s * pull, end.y),
            end,
        ])
    } else {
        let s = dy.signum();
        let start = Point::new(a.x, a.y + s * 0.5 * h);
        let end = Point::new(b.x, b.y - s * 0.5 * h);
        let pull = (0.5 * (end.y - start.y).abs()).max(CARD_GAP);
        Some([
            start,
            Point::new(start.x, start.y + s * pull),
            Point::new(end.x, end.y - s * pull),
            end,
        ])
    }
}

/// The world-space box that holds every card: the centre card at the origin
/// (when `has_centre`) and a card at each of `ring`.
pub fn star_bounds(has_centre: bool, ring: &[Point]) -> Bounds {
    let (hw, hh) = (0.5 * CARD_SIZE.0, 0.5 * CARD_SIZE.1);
    let centres = ring
        .iter()
        .copied()
        .chain(has_centre.then(|| Point::new(0.0, 0.0)));
    let mut b: Option<Bounds> = None;
    for p in centres {
        let card = Bounds {
            min_x: p.x - hw,
            min_y: p.y - hh,
            max_x: p.x + hw,
            max_y: p.y + hh,
        };
        b = Some(match b {
            None => card,
            Some(b) => Bounds {
                min_x: b.min_x.min(card.min_x),
                min_y: b.min_y.min(card.min_y),
                max_x: b.max_x.max(card.max_x),
                max_y: b.max_y.max(card.max_y),
            },
        });
    }
    b.unwrap_or(Bounds {
        min_x: -0.5,
        min_y: -0.5,
        max_x: 0.5,
        max_y: 0.5,
    })
}

/// The scrollable canvas the star is drawn on, at one zoom and viewport size.
///
/// The canvas is the map at `zoom`, plus a margin of [`PAN_MARGIN_FRACTION`]
/// of the drawn map's width and height on every side, so the scroll area lets
/// the view travel exactly that far past each edge and no further.
/// Horizontally the margin also gains [`HORIZONTAL_PAN_VIEWPORT_FRACTION`] of
/// the viewport width. When the canvas would still be smaller than the
/// viewport on an axis, it is widened to the viewport and the map is centred
/// on that axis (nothing to scroll there).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasLayout {
    /// Where the world origin lands on the canvas, points from its top left.
    pub origin: (f64, f64),
    /// Canvas size, points.
    pub size: (f64, f64),
    /// World-to-canvas scale.
    pub zoom: f64,
}

impl CanvasLayout {
    /// Lay out `bounds` at `zoom` in a viewport of `viewport` points.
    pub fn new(bounds: Bounds, zoom: f64, viewport: (f64, f64)) -> Self {
        let axis = |min: f64, extent: f64, view: f64, view_fraction: f64| {
            let drawn = extent * zoom;
            let margin = PAN_MARGIN_FRACTION * drawn + view_fraction * view;
            let canvas = drawn + 2.0 * margin;
            let extra = (0.5 * (view - canvas)).max(0.0);
            // Canvas position of the world origin on this axis.
            let origin = margin + extra - min * zoom;
            (origin, canvas + 2.0 * extra)
        };
        let (ox, sx) = axis(
            bounds.min_x,
            bounds.width(),
            viewport.0,
            HORIZONTAL_PAN_VIEWPORT_FRACTION,
        );
        let (oy, sy) = axis(bounds.min_y, bounds.height(), viewport.1, 0.0);
        Self {
            origin: (ox, oy),
            size: (sx, sy),
            zoom,
        }
    }

    /// Canvas position, points, of world point `p`.
    pub fn to_canvas(&self, p: Point) -> (f64, f64) {
        (
            self.origin.0 + p.x * self.zoom,
            self.origin.1 + p.y * self.zoom,
        )
    }

    /// World point at canvas position `c`: the inverse of [`Self::to_canvas`].
    pub fn to_world(&self, c: (f64, f64)) -> Point {
        Point::new(
            (c.0 - self.origin.0) / self.zoom,
            (c.1 - self.origin.1) / self.zoom,
        )
    }

    /// The scroll offset that puts world point `p` at the middle of a
    /// `viewport`, limited to what the canvas allows.
    pub fn offset_centring(&self, p: Point, viewport: (f64, f64)) -> (f64, f64) {
        let (cx, cy) = self.to_canvas(p);
        (
            (cx - 0.5 * viewport.0).clamp(0.0, (self.size.0 - viewport.0).max(0.0)),
            (cy - 0.5 * viewport.1).clamp(0.0, (self.size.1 - viewport.1).max(0.0)),
        )
    }
}

/// The zoom at which `bounds` fits inside `viewport`, margins excluded.
pub fn fit_zoom(bounds: Bounds, viewport: (f64, f64)) -> f64 {
    let z = (viewport.0 / bounds.width()).min(viewport.1 / bounds.height());
    if z.is_finite() && z > 0.0 {
        z
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlap(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() < CARD_SIZE.0 && (a.y - b.y).abs() < CARD_SIZE.1
    }

    /// Whether a box of `size` centred at `a` overlaps a card centred at `b`.
    fn box_hits_card(a: Point, size: (f64, f64), b: Point) -> bool {
        (a.x - b.x).abs() < 0.5 * (size.0 + CARD_SIZE.0)
            && (a.y - b.y).abs() < 0.5 * (size.1 + CARD_SIZE.1)
    }

    /// Stars with a parent card, across ring sizes and with a fan or two
    /// expanded — the shapes the layout actually has to survive.
    fn stars_with_parents() -> Vec<(usize, StarLayout)> {
        let mut out = Vec::new();
        for n in 0..=16 {
            for expanded in [0usize, 1, 5] {
                let mut fan_sizes = vec![0; n];
                for (i, f) in fan_sizes.iter_mut().enumerate() {
                    // Expand the card nearest the top, where a fan is most
                    // likely to reach up towards the parent card.
                    if i == 0 {
                        *f = expanded;
                    }
                }
                out.push((n, star_layout_with_parent(&fan_sizes, true)));
            }
        }
        out
    }

    /// #283: the parent card must not touch anything in the star, including
    /// an expanded fan that reaches further out than the ring does.
    #[test]
    fn the_parent_card_clears_every_card_in_the_star() {
        for (n, layout) in stars_with_parents() {
            let parent = parent_position(&layout);
            assert!(parent.y < 0.0, "n={n}: the parent card must be above");
            assert!(
                !overlap(parent, Point::new(0.0, 0.0)),
                "n={n}: the parent card hits the centre card"
            );
            for (i, p) in layout.all_points().enumerate() {
                assert!(
                    !overlap(parent, p),
                    "n={n}: the parent card hits card {i} at {p:?}"
                );
            }
        }
    }

    /// #283: the Up button sits on the dotted line between the centre and the
    /// parent, so the corridor it occupies has to be clear — which is the
    /// whole reason the ring is rotated half a step when a parent is shown.
    #[test]
    fn the_up_button_box_clears_every_card_in_the_star() {
        for (n, layout) in stars_with_parents() {
            let button = up_button_centre(&layout);
            assert!(
                !box_hits_card(button, UP_BUTTON_SIZE, Point::new(0.0, 0.0)),
                "n={n}: the Up button covers the centre card"
            );
            for (i, p) in layout.all_points().enumerate() {
                assert!(
                    !box_hits_card(button, UP_BUTTON_SIZE, p),
                    "n={n}: the Up button covers card {i} at {p:?}"
                );
            }
        }
    }

    /// What the rotation is actually for: no ring card left in the corridor
    /// straight up, where the dotted connector and the Up button are drawn.
    ///
    /// Deliberately *not* "every radius is unchanged" — that was the first
    /// version of this test and it failed at `n = 10`, because
    /// [`cards_collide`] compares axis-aligned boxes and so is not
    /// rotation-invariant: a turned ring can trip the growth loop where the
    /// straight one did not. The layout is still correct there; the
    /// assumption was wrong, and `star_layout_with_parent`'s doc now says so.
    #[test]
    fn the_parent_rotation_leaves_the_corridor_above_the_centre_empty() {
        for n in 1..=16 {
            let fan_sizes = vec![0; n];
            assert_eq!(
                star_layout_with_parent(&fan_sizes, false).ring,
                star_layout(&fan_sizes).ring,
                "n={n}: has_parent = false must be exactly star_layout"
            );
            let turned = star_layout_with_parent(&fan_sizes, true);
            for (i, p) in turned.ring.iter().enumerate() {
                assert!(
                    !(p.x.abs() < 1.0 && p.y < 0.0),
                    "n={n}: ring card {i} sits in the corridor at {p:?}"
                );
            }
            let mut all = vec![Point::new(0.0, 0.0)];
            all.extend(turned.all_points());
            for (i, a) in all.iter().enumerate() {
                for (j, b) in all.iter().enumerate().skip(i + 1) {
                    assert!(!overlap(*a, *b), "n={n}: cards {i} and {j} overlap");
                }
            }
        }
    }

    /// No two cards of a star touch, for any number of children, and none
    /// touches the centre card.
    #[test]
    fn no_two_cards_overlap() {
        for n in 1..=150 {
            let ring = star_positions(n);
            assert_eq!(ring.len(), n);
            for (i, a) in ring.iter().enumerate() {
                assert!(
                    !overlap(*a, Point::new(0.0, 0.0)),
                    "n={n}: card {i} hits the centre"
                );
                for (j, b) in ring.iter().enumerate().skip(i + 1) {
                    assert!(!overlap(*a, *b), "n={n}: cards {i} and {j} overlap");
                }
            }
        }
    }

    /// The first card is straight above the centre, and the rest go round
    /// clockwise (screen y grows downward).
    #[test]
    fn the_ring_starts_at_the_top_and_runs_clockwise() {
        let ring = star_positions(4);
        assert!(ring[0].x.abs() < 1e-9 && ring[0].y < 0.0, "top");
        assert!(ring[1].x > 0.0 && ring[1].y.abs() < 1e-9, "right");
        assert!(ring[2].y > 0.0, "bottom");
        assert!(ring[3].x < 0.0, "left");
    }

    /// The view can pan exactly 25 % of the drawn map past the top and bottom
    /// edges, and that plus half a viewport past the side edges, at any zoom.
    #[test]
    fn the_pan_margin_is_a_quarter_of_the_drawn_map() {
        let bounds = Bounds {
            min_x: -100.0,
            min_y: -50.0,
            max_x: 300.0,
            max_y: 150.0,
        };
        let view = (100.0, 100.0);
        let extra_x = HORIZONTAL_PAN_VIEWPORT_FRACTION * view.0;
        for zoom in [1.0, 2.0, 3.5] {
            let c = CanvasLayout::new(bounds, zoom, view);
            let (w, h) = (400.0 * zoom, 200.0 * zoom);
            // Vertically: exactly a quarter of the drawn map on each side.
            assert!((c.size.1 - 1.5 * h).abs() < 1e-9, "height at zoom {zoom}");
            // Horizontally: a quarter of the map plus half a viewport.
            assert!(
                (c.size.0 - (1.5 * w + 2.0 * extra_x)).abs() < 1e-9,
                "width at zoom {zoom}"
            );
            let (x, y) = c.to_canvas(Point::new(bounds.min_x, bounds.min_y));
            assert!((x - (0.25 * w + extra_x)).abs() < 1e-9 && (y - 0.25 * h).abs() < 1e-9);
        }
    }

    /// At the Fit zoom the map fills the viewport's height and is narrower
    /// than its width, which used to leave nothing to scroll sideways. Now
    /// either side edge of the map can be panned to the middle of the screen
    /// (maintainer direction, 2026-09-22).
    #[test]
    fn at_fit_either_side_edge_can_reach_the_middle_of_the_screen() {
        let bounds = star_bounds(true, &star_positions(9));
        let view = (1600.0, 700.0);
        let c = CanvasLayout::new(bounds, fit_zoom(bounds, view), view);
        assert!(c.size.0 > view.0, "there is room to scroll sideways");
        let max_offset = c.size.0 - view.0;
        let (left, _) = c.to_canvas(Point::new(bounds.min_x, 0.0));
        let (right, _) = c.to_canvas(Point::new(bounds.max_x, 0.0));
        // Scrolled fully left, the map's left edge is at or right of the
        // middle; scrolled fully right, its right edge at or left of it.
        assert!(left >= 0.5 * view.0 - 1e-9, "left edge reaches {left}");
        assert!(
            right - max_offset <= 0.5 * view.0 + 1e-9,
            "right edge reaches {}",
            right - max_offset
        );
    }

    /// A map much smaller than the viewport is centred; it pans sideways
    /// (the horizontal room) but not vertically.
    #[test]
    fn a_small_map_is_centred_and_pans_only_sideways() {
        let bounds = Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 40.0,
        };
        let view = (1000.0, 600.0);
        let c = CanvasLayout::new(bounds, 1.0, view);
        assert_eq!(c.size.1, 600.0, "nothing to scroll vertically");
        assert!(c.size.0 > 1000.0, "room to pan sideways");
        let (ox, oy) = c.offset_centring(bounds.centre(), view);
        let (x, y) = c.to_canvas(bounds.centre());
        assert!(
            (x - ox - 500.0).abs() < 1e-9 && (y - oy - 300.0).abs() < 1e-9,
            "centred"
        );
        assert_eq!(oy, 0.0);
    }

    /// Canvas and world coordinates are inverses, and centring a point puts
    /// it at the middle of the viewport when the canvas allows it.
    #[test]
    fn centring_puts_a_point_in_the_middle_of_the_view() {
        let bounds = star_bounds(true, &star_positions(12));
        let c = CanvasLayout::new(bounds, 2.0, (400.0, 300.0));
        let p = Point::new(40.0, -25.0);
        let back = c.to_world(c.to_canvas(p));
        assert!((back.x - p.x).abs() < 1e-9 && (back.y - p.y).abs() < 1e-9);
        let (ox, oy) = c.offset_centring(Point::new(0.0, 0.0), (400.0, 300.0));
        let (cx, cy) = c.to_canvas(Point::new(0.0, 0.0));
        assert!((cx - ox - 200.0).abs() < 1e-9 && (cy - oy - 150.0).abs() < 1e-9);
    }

    /// Expanding cards in place never makes two cards overlap: every
    /// combination of fan sizes tried, including every card expanded.
    #[test]
    fn expanded_fans_never_overlap_anything() {
        let cases: Vec<Vec<usize>> = vec![
            vec![3],
            vec![0, 5],
            vec![4, 4],
            vec![6, 0, 6, 0],
            vec![2; 8],
            vec![12, 1, 0, 7, 3],
            vec![5; 12],
            vec![1; 20],
        ];
        for fans in cases {
            let l = star_layout(&fans);
            assert_eq!(l.ring.len(), fans.len());
            for (f, &m) in l.fans.iter().zip(&fans) {
                assert_eq!(f.len(), m);
            }
            let mut cards = vec![Point::new(0.0, 0.0)];
            cards.extend(l.all_points());
            for (i, a) in cards.iter().enumerate() {
                for (j, b) in cards.iter().enumerate().skip(i + 1) {
                    assert!(!overlap(*a, *b), "{fans:?}: cards {i} and {j} overlap");
                }
            }
        }
    }

    /// With nothing expanded, the layout is exactly the plain ring.
    #[test]
    fn a_collapsed_star_is_the_plain_ring() {
        let l = star_layout(&[0; 7]);
        assert_eq!(l.ring, star_positions(7));
        assert!(l.fans.iter().all(|f| f.is_empty()));
    }

    /// A fan points away from the centre of the star.
    #[test]
    fn a_fan_opens_outward() {
        let l = star_layout(&[3, 0, 0, 0]);
        let parent = l.ring[0]; // straight up
        for p in &l.fans[0] {
            assert!(p.y <= parent.y + 1e-9, "fan card below its parent: {p:?}");
        }
    }

    /// Is `p` on the border of the card centred at `c`?
    fn on_edge(p: Point, c: Point) -> bool {
        let (hw, hh) = (0.5 * CARD_SIZE.0, 0.5 * CARD_SIZE.1);
        let (x, y) = ((p.x - c.x).abs(), (p.y - c.y).abs());
        ((x - hw).abs() < 1e-9 && y <= hh + 1e-9) || ((y - hh).abs() < 1e-9 && x <= hw + 1e-9)
    }

    /// Cards side by side join facing left/right edges, horizontally; cards
    /// one above the other join facing top/bottom edges, vertically. Either
    /// way the curve starts and ends on a card edge, never at a centre.
    #[test]
    fn a_connector_joins_the_facing_edges() {
        let a = Point::new(0.0, 0.0);
        // To the right and a little down: a horizontal connector.
        let [s, c1, c2, e] = connector(a, Point::new(400.0, 60.0)).unwrap();
        assert!(on_edge(s, a) && on_edge(e, Point::new(400.0, 60.0)));
        assert_eq!(s.x, 0.5 * CARD_SIZE.0, "leaves the right edge");
        assert_eq!(e.x, 400.0 - 0.5 * CARD_SIZE.0, "arrives at the left edge");
        assert!(
            c1.y == s.y && c2.y == e.y,
            "leaves and arrives horizontally"
        );
        // Straight below: a vertical connector.
        let b = Point::new(30.0, 300.0);
        let [s, c1, c2, e] = connector(a, b).unwrap();
        assert_eq!(s.y, 0.5 * CARD_SIZE.1, "leaves the bottom edge");
        assert_eq!(e.y, 300.0 - 0.5 * CARD_SIZE.1, "arrives at the top edge");
        assert!(c1.x == s.x && c2.x == e.x, "leaves and arrives vertically");
        assert!(on_edge(s, a) && on_edge(e, b));
    }

    /// Swapping the two cards gives the same curve, reversed; overlapping
    /// cards get none; every ring spoke in a star has one.
    #[test]
    fn connectors_are_symmetric_and_exist_for_every_spoke() {
        let (a, b) = (Point::new(10.0, -20.0), Point::new(-350.0, 90.0));
        let mut there = connector(a, b).unwrap();
        there.reverse();
        assert_eq!(Some(there), connector(b, a));
        assert_eq!(connector(a, Point::new(20.0, -10.0)), None);
        for n in [1, 4, 9, 30] {
            for p in star_positions(n) {
                assert!(connector(Point::new(0.0, 0.0), p).is_some(), "n={n}: {p:?}");
            }
        }
    }

    #[test]
    fn fit_zoom_fits_the_tighter_axis() {
        let bounds = Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 200.0,
            max_y: 100.0,
        };
        assert!((fit_zoom(bounds, (400.0, 400.0)) - 2.0).abs() < 1e-12);
        assert!((fit_zoom(bounds, (400.0, 100.0)) - 1.0).abs() < 1e-12);
    }
}
