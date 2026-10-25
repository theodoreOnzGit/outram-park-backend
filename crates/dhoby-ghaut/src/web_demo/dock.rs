//! **A collapsible dock on the bottom or right edge of the main view**
//! (maintainer, 2026-10-25: "the same keff interface and Shannon entropy
//! under one collapsible bottom panel ... The top view and control rod views
//! with reference graphs should be a collapsible side panel").
//!
//! The controls stay in [`super::panel::Panel`] on the left. A dock holds
//! *results and companion views* that belong beside the main picture: the
//! k_eff and entropy plots of a run along the bottom, secondary slices and
//! reference graphs on the right. Each dock folds to a thumb-sized tab and
//! comes back from it, like the side panel (mobile-first rule items 3–5).
//!
//! - [`Edge::Bottom`]: open by default at every width (a run's result is
//!   what the reader came for). Folded, it keeps a [`HEADER_PX`] strip along
//!   the bottom whose toggle sits at its left, so the strip can also carry
//!   the run's own buttons.
//! - [`Edge::Right`]: open by default only at [`WIDE_PX`] or wider; on a
//!   narrower screen it opens over most of the main view, as the side panel
//!   does on a phone. Folded, it takes no width: a "« title" tab sits below
//!   the zoom buttons at the main view's top right.
//!
//! Lay out with [`Dock::split`] (right dock first, on the full rect, then
//! the bottom dock on what is left), draw the body, then call
//! [`Dock::toggle`] on the returned header.
//!
//! Presentation only: no physics (the crate rule).

use egui::{Pos2, Rect, Vec2};

/// Height of a bottom dock's header strip, and of every dock toggle, px
/// (above Apple's 44 pt minimum target).
pub const HEADER_PX: f32 = 44.0;
/// Screens at least this wide open a right dock unfolded, px.
pub const WIDE_PX: f32 = 1000.0;
/// Width of a folded right dock's tab, px.
pub const TAB_W: f32 = 132.0;
/// How far below the main view's top a folded right dock's tab sits, px:
/// clear of the 52 px row every demo keeps for "Controls »" and the zoom
/// buttons.
pub const TAB_TOP_PX: f32 = 56.0;

/// Which edge a dock hangs from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Bottom,
    Right,
}

/// Where a dock put things this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Split {
    /// What is left for the main view.
    pub main: Rect,
    /// The dock's header: its toggle (and, on a bottom dock, room beside it
    /// for the caller's buttons). A folded right dock's header is its tab,
    /// over the main view.
    pub header: Rect,
    /// The dock's body, `None` while folded.
    pub body: Option<Rect>,
}

/// One dock.
pub struct Dock {
    pub edge: Edge,
    pub open: bool,
    /// Decided on the first [`Dock::split`], from the width.
    decided: bool,
    title: &'static str,
}

impl Dock {
    /// A dock on `edge` whose toggle says `title`.
    pub fn new(edge: Edge, title: &'static str) -> Self {
        Self { edge, open: true, decided: false, title }
    }

    /// Whether a dock on `edge` starts open on a screen `width` px wide.
    pub fn opens_at(edge: Edge, width: f32) -> bool {
        match edge {
            Edge::Bottom => true,
            Edge::Right => width >= WIDE_PX,
        }
    }

    /// Split `full` between the main view and the dock; the first call
    /// decides the initial state from `full`'s width.
    pub fn split(&mut self, full: Rect) -> Split {
        if !self.decided {
            self.open = Self::opens_at(self.edge, full.width());
            self.decided = true;
        }
        layout(self.edge, self.open, full)
    }

    /// The fold / unfold button in `split.header` (call after drawing the
    /// body, so it is on top). Returns the rect left beside it in a bottom
    /// dock's header, for the caller's own buttons.
    pub fn toggle(&mut self, ui: &mut egui::Ui, split: &Split) -> Rect {
        let h = split.header;
        let (button, rest, label) = match (self.edge, self.open) {
            (Edge::Bottom, open) => {
                let w = (h.width() * 0.45).min(230.0);
                let b = Rect::from_min_size(h.min, Vec2::new(w, h.height()));
                let rest = Rect::from_min_max(Pos2::new(b.right() + 6.0, h.top()), h.max);
                (b, rest, format!("{} {}", if open { "▾" } else { "▴" }, self.title))
            }
            (Edge::Right, true) => (h, Rect::NOTHING, format!("Hide {} »", self.title)),
            (Edge::Right, false) => (h, Rect::NOTHING, format!("« {}", self.title)),
        };
        let text = egui::RichText::new(label).size(16.0).strong();
        let hover = if self.open { "Fold this panel away" } else { "Show this panel" };
        if ui.put(button, egui::Button::new(text)).on_hover_text(hover).clicked() {
            self.open = !self.open;
        }
        rest
    }
}

/// The layout of a dock on `edge`, `open` or folded, in `full`.
pub fn layout(edge: Edge, open: bool, full: Rect) -> Split {
    match (edge, open) {
        (Edge::Bottom, false) => {
            let top = (full.bottom() - HEADER_PX).max(full.top());
            Split { main: full.with_max_y(top), header: full.with_min_y(top), body: None }
        }
        (Edge::Bottom, true) => {
            // A third of the height, more on a narrow (portrait) screen where
            // the plots stack; never so much that the picture disappears.
            let frac = if full.width() < super::panel::NARROW_PX { 0.42 } else { 0.34 };
            let h = (full.height() * frac).clamp((2.0 * HEADER_PX).min(full.height()), full.height() * 0.62);
            let top = full.bottom() - h;
            let header = Rect::from_min_max(Pos2::new(full.left(), top), Pos2::new(full.right(), top + HEADER_PX));
            Split { main: full.with_max_y(top), header, body: Some(full.with_min_y(header.bottom())) }
        }
        (Edge::Right, false) => {
            let header = Rect::from_min_size(Pos2::new(full.right() - 8.0 - TAB_W, full.top() + TAB_TOP_PX), Vec2::new(TAB_W, HEADER_PX));
            Split { main: full, header, body: None }
        }
        (Edge::Right, true) => {
            // A phone gets most of the screen; a wide one a column.
            let w = if full.width() < super::panel::NARROW_PX { full.width() * 0.88 } else { (full.width() * 0.38).clamp(300.0, 560.0) };
            let left = full.right() - w;
            let header = Rect::from_min_max(Pos2::new(left, full.top()), Pos2::new(full.right(), full.top() + HEADER_PX));
            Split { main: full.with_max_x(left), header, body: Some(Rect::from_min_max(Pos2::new(left, header.bottom()), full.max)) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(w: f32, h: f32) -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(w, h))
    }

    /// A bottom dock opens everywhere, a right dock only on a wide screen,
    /// and the first split decides it.
    #[test]
    fn the_initial_state_follows_the_width() {
        let mut b = Dock::new(Edge::Bottom, "k");
        let mut r = Dock::new(Edge::Right, "views");
        assert!(b.split(screen(390.0, 844.0)).body.is_some());
        assert!(r.split(screen(390.0, 844.0)).body.is_none());
        let mut r = Dock::new(Edge::Right, "views");
        assert!(r.split(screen(1280.0, 900.0)).body.is_some());
        // Decided once: a later, different width does not reopen it.
        let mut r = Dock::new(Edge::Right, "views");
        r.split(screen(390.0, 844.0));
        assert!(r.split(screen(1280.0, 900.0)).body.is_none());
    }

    /// The pieces tile the screen without overlap, the header is a thumb
    /// target, and the main view keeps room for its picture.
    #[test]
    fn open_docks_tile_the_screen() {
        for (w, h) in [(390.0, 844.0), (1280.0, 900.0), (320.0, 480.0)] {
            let full = screen(w, h);
            let s = layout(Edge::Bottom, true, full);
            let body = s.body.expect("open");
            assert_eq!(s.main.bottom(), s.header.top());
            assert_eq!(s.header.bottom(), body.top());
            assert_eq!(body.bottom(), full.bottom());
            assert!(s.header.height() >= 44.0);
            assert!(s.main.height() >= 0.38 * h - 0.5, "{w}x{h}: main {}", s.main.height());
            let s = layout(Edge::Right, true, full);
            let body = s.body.expect("open");
            assert_eq!(s.main.right(), body.left());
            assert_eq!(body.right(), full.right());
            assert_eq!(s.header.bottom(), body.top());
        }
    }

    /// Folded: a bottom dock keeps only its header strip; a right dock takes
    /// no width and its tab clears the 52 px top row and fits a 320 px screen.
    #[test]
    fn folded_docks_leave_the_main_view() {
        let full = screen(320.0, 480.0);
        let b = layout(Edge::Bottom, false, full);
        assert_eq!(b.main.height() + b.header.height(), full.height());
        assert!(b.body.is_none());
        let r = layout(Edge::Right, false, full);
        assert_eq!(r.main, full);
        assert!(r.header.top() >= 52.0 && r.header.left() >= 0.0 && r.header.height() >= 44.0);
    }
}
