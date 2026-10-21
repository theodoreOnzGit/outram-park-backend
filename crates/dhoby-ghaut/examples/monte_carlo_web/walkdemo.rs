//! **A rung's own demo** (gh:#785, "all code walks must have their own web
//! demo"): a main view and a panel that a rung owns, for a code walk whose
//! idea is not neutron tracks, a power iteration or a recorded sweep.
//!
//! The hook is small and generic, so the app needs no change per demo:
//!
//! - a rung returns `Some(kind)` from `McRung::walk_demo`; the app then offers
//!   the Watch view `demo` (`?view=demo`) and holds a [`WalkDemo`];
//! - the page side ([`WalkDemo`]) draws, handles its panel, and asks the
//!   worker for work as [`Outgoing`] messages: a geometry raster (the shared
//!   [`crate::raster`] pipeline) or a flat `f64` message;
//! - the worker side is the rung's `LoadedRung::walk` (flat `f64`s in and
//!   out) and, for rasters, `LoadedRung::raster`. Each message is one short
//!   computation, so the page never waits (the no-lag rule).
//!
//! The demos: [`crate::dhshort`] (step 5, the DH shortcuts) and
//! [`crate::packing`] (step 6, RSA packing and cut particles).

use crate::raster::RasterReq;
use dhoby_ghaut::web_demo::view::View;
use egui::Rect;

/// Which demo a rung has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalkKind {
    /// Step 5: the same FHR unit cell under each `DhTreatment`.
    Shortcuts,
    /// Step 6: RSA placing particles one at a time, and the cut particles.
    Packing,
}

/// What a demo asks the worker for.
#[derive(Clone, Debug, PartialEq)]
pub enum Outgoing {
    Raster(RasterReq),
    Walk(Vec<f64>),
}

/// The page side of a rung's demo.
#[allow(clippy::large_enum_variant)] // held by value: the workspace forbids `Box<T>`
pub enum WalkDemo {
    Shortcuts(crate::dhshort::ui::ShortcutsView),
    Packing(crate::packing::ui::PackingView),
}

impl WalkDemo {
    /// The demo for `kind`, with the first messages it needs sent on the first
    /// frame ([`Self::pump`]).
    pub fn new(kind: WalkKind) -> Self {
        match kind {
            WalkKind::Shortcuts => WalkDemo::Shortcuts(crate::dhshort::ui::ShortcutsView::new()),
            WalkKind::Packing => WalkDemo::Packing(crate::packing::ui::PackingView::new()),
        }
    }

    /// Messages the demo wants sent now (not drawing-related: the first
    /// request, a follow-up after an answer).
    pub fn pump(&mut self) -> Vec<Outgoing> {
        match self {
            WalkDemo::Shortcuts(s) => s.pump(),
            WalkDemo::Packing(p) => p.pump(),
        }
    }

    /// The worker's answer to an [`Outgoing::Walk`].
    pub fn receive(&mut self, msg: &[f64]) -> Result<(), String> {
        match self {
            WalkDemo::Shortcuts(s) => s.receive(msg),
            WalkDemo::Packing(p) => p.receive(msg),
        }
    }

    /// A raster answered (only demos that slice geometry ask for one).
    pub fn receive_raster(&mut self, ctx: &egui::Context, req: RasterReq, map: Vec<u8>, secs: f64) {
        if let WalkDemo::Shortcuts(s) = self {
            s.slicer.receive(ctx, req, map, secs);
        }
    }

    /// The main view the + / − / Reset buttons and the scale bar act on.
    pub fn view_mut(&mut self) -> &mut View {
        match self {
            WalkDemo::Shortcuts(s) => &mut s.slicer.view,
            WalkDemo::Packing(p) => &mut p.view,
        }
    }

    /// One line for the page title.
    pub fn status(&self) -> String {
        match self {
            WalkDemo::Shortcuts(s) => s.status(),
            WalkDemo::Packing(p) => p.status(),
        }
    }

    /// The panel. GUI drawing: exempt from the reaching-test rule (no
    /// headless egui context in the tests); its state changes are the tested
    /// methods it calls.
    pub fn panel(&mut self, ui: &mut egui::Ui) -> Vec<Outgoing> {
        match self {
            WalkDemo::Shortcuts(s) => s.panel(ui),
            WalkDemo::Packing(p) => p.panel(ui),
        }
    }

    /// The main view. GUI drawing: exempt from the reaching-test rule (as
    /// [`Self::panel`]).
    pub fn canvas(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        painter: &egui::Painter,
        resp: &egui::Response,
    ) -> Vec<Outgoing> {
        match self {
            WalkDemo::Shortcuts(s) => s.canvas(ui, rect, painter, resp),
            WalkDemo::Packing(p) => p.canvas(ui, rect, painter, resp),
        }
    }
}

/// Split the main view: the picture and a side area for its plot, beside it
/// on a wide landscape screen, under it on a phone (as the `layers` view).
pub fn split(full: Rect) -> (Rect, Rect) {
    if full.width() >= 700.0 && full.width() > full.height() {
        let (a, b) = full.split_left_right_at_fraction(0.56);
        (a, b.shrink2(egui::Vec2::new(6.0, 8.0)))
    } else {
        let (a, b) = full.split_top_bottom_at_fraction(0.56);
        (a, b.shrink2(egui::Vec2::new(8.0, 4.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A phone splits top and bottom, a wide screen left and right, and the
    /// picture keeps the larger share either way.
    #[test]
    fn the_split_follows_the_screen() {
        let phone = Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(390.0, 760.0));
        let (a, b) = split(phone);
        assert!(a.bottom() <= b.top() + 1e-3 && a.height() > b.height());
        let wide = Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(1280.0, 760.0));
        let (a, b) = split(wide);
        assert!(a.right() <= b.left() + 1e-3 && a.width() > b.width());
    }

    /// Each demo starts by asking the worker for what it needs, and a new demo
    /// asks only once until answered.
    #[test]
    fn each_demo_asks_for_its_first_work() {
        let mut d = WalkDemo::new(WalkKind::Shortcuts);
        assert!(
            matches!(d.pump().as_slice(), [Outgoing::Walk(m)] if m[0] == crate::dhshort::MSG_DESCRIBE)
        );
        assert!(d.pump().is_empty());
        let mut p = WalkDemo::new(WalkKind::Packing);
        assert!(
            matches!(p.pump().as_slice(), [Outgoing::Walk(m)] if m[0] == crate::packing::MSG_PLAN)
        );
        assert!(p.pump().is_empty());
    }
}
