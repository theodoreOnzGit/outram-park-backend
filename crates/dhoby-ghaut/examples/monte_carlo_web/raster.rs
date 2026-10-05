//! **A live slice of a rung's assembled geometry** (gh:#528): the zoom
//! ladder of the `htr10` rung (core → pebble → TRISO → kernel), drawn from
//! what the solver sees.
//!
//! The worker rasterises the window the reader is looking at with
//! outram-mc-libs' port of OpenMC's slice plotter (`SlicePlot::id_map`: one
//! `Geometry::locate` per pixel on the ASSEMBLED geometry, the same call the
//! geometry review images use) and sends one byte per pixel: the material
//! index, or [`VOID`] / [`OUTSIDE`] / [`OVERLAP`]. The page colours it with
//! the rung's palette. While the reader zooms or pans, the last image is
//! redrawn under the new view at once (the UI never waits) and a new raster is
//! asked for once the view has been still for [`SETTLE_S`].

// The wire format serves the browser build (and the tests).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use dhoby_ghaut::web_demo::platform::now_s;
use dhoby_ghaut::web_demo::view::View;
use egui::{Color32, ColorImage, Pos2, Rect, TextureHandle, TextureOptions, Vec2};

/// Pixel codes beyond the material indices.
pub const VOID: u8 = 253;
pub const OUTSIDE: u8 = 254;
pub const OVERLAP: u8 = 255;

/// The view must be still this long before a new raster is asked for, s.
pub const SETTLE_S: f64 = 0.25;
/// Largest raster side, px (a phone-sized view is ~400 × 450).
pub const MAX_PX: usize = 520;

/// Which plane: x-y at height `z`, or x-z at `y`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Basis {
    Xy,
    Xz,
}

/// What to rasterise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterReq {
    /// Asked-for counter, so a late answer to an old request is recognised.
    pub id: u32,
    pub basis: Basis,
    /// Centre of the window (horizontal, vertical) in the plane, cm, and the
    /// coordinate of the plane along the third axis.
    pub centre: [f64; 2],
    pub depth: f64,
    /// Full width and height of the window, cm.
    pub width: [f64; 2],
    pub px: [usize; 2],
    /// A rung parameter that changes the geometry (`htr10`: bed layers N).
    pub param: f64,
}

impl RasterReq {
    pub fn encode(&self) -> [f64; 10] {
        [
            self.id as f64,
            if self.basis == Basis::Xy { 0.0 } else { 1.0 },
            self.centre[0],
            self.centre[1],
            self.depth,
            self.width[0],
            self.width[1],
            self.px[0] as f64,
            self.px[1] as f64,
            self.param,
        ]
    }
    pub fn decode(v: &[f64]) -> Result<Self, String> {
        if v.len() != 10 {
            return Err(format!("raster request: {} values", v.len()));
        }
        Ok(Self {
            id: v[0] as u32,
            basis: if v[1] == 0.0 { Basis::Xy } else { Basis::Xz },
            centre: [v[2], v[3]],
            depth: v[4],
            width: [v[5], v[6]],
            px: [v[7] as usize, v[8] as usize],
            param: v[9],
        })
    }
}

/// A step of the zoom ladder: a named plane and window.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub label: &'static str,
    pub basis: Basis,
    pub centre: [f64; 2],
    pub depth: f64,
    /// Half-width of the window Reset fits, cm.
    pub half: f64,
}

/// What a rung with a live slice offers.
#[derive(Clone, Debug)]
pub struct RasterInfo {
    /// Colour and name per material index.
    pub palette: Vec<(Color32, &'static str)>,
    pub ladder: Vec<Preset>,
    /// What the slice is drawn from, one line.
    pub source: &'static str,
    /// The ladder step the view opens on, and the geometry parameter it uses.
    pub start: usize,
    pub param: f64,
}

/// Two requests for the same window, up to round-off (zooming in and back
/// out does not return bit for bit to the same scale).
fn same(a: &RasterReq, b: &RasterReq) -> bool {
    a.basis == b.basis
        && a.depth == b.depth
        && a.param == b.param
        && a.px == b.px
        && (a.centre[0] - b.centre[0]).abs() < 1e-9 * a.width[0].max(1e-9)
        && (a.centre[1] - b.centre[1]).abs() < 1e-9 * a.width[1].max(1e-9)
        && (a.width[0] / b.width[0] - 1.0).abs() < 1e-9
}

/// Rasterise `req` on `geom` (worker side): one byte per pixel, row 0 on top.
pub fn raster(geom: &outram_mc_libs::geometry::geometry::Geometry, req: &RasterReq) -> Vec<u8> {
    use outram_mc_libs::geometry::plot::{PlotBasis, SliceHit, SlicePlot};
    use outram_mc_libs::geometry::position::Position;
    let origin = match req.basis {
        Basis::Xy => Position::new(req.centre[0], req.centre[1], req.depth),
        Basis::Xz => Position::new(req.centre[0], req.depth, req.centre[1]),
    };
    let plot = SlicePlot {
        origin,
        basis: if req.basis == Basis::Xy { PlotBasis::Xy } else { PlotBasis::Xz },
        width: req.width,
        pixels: req.px,
        level: None,
        show_overlaps: false,
        meshlines: None,
    };
    plot.id_map(geom)
        .hits
        .iter()
        .map(|h| match *h {
            SliceHit::Found { material: Some(m), .. } => (m.min(252)) as u8,
            SliceHit::Found { material: None, .. } => VOID,
            SliceHit::NotFound => OUTSIDE,
            SliceHit::Overlap => OVERLAP,
        })
        .collect()
}

// ─── UI side ─────────────────────────────────────────────────────────────────

/// The live slice as the page follows it.
pub struct Slicer {
    pub info: RasterInfo,
    /// Current ladder step (plane and depth come from it).
    pub preset: usize,
    pub view: View,
    pub param: f64,
    next_id: u32,
    /// The request in flight, if any.
    pending: Option<RasterReq>,
    /// The newest image and the request it answers.
    shown: Option<(RasterReq, TextureHandle, Vec<u8>)>,
    /// What the view looked like last frame, and since when.
    last_window: Option<RasterReq>,
    still_since: f64,
    pub last_secs: f64,
}

impl Slicer {
    pub fn new(info: RasterInfo, preset: usize, param: f64) -> Self {
        let view = View::new(info.ladder[preset].half).with_home(info.ladder[preset].centre);
        Self { info, preset, view, param, next_id: 1, pending: None, shown: None, last_window: None, still_since: now_s(), last_secs: 0.0 }
    }

    /// Go to ladder step `i` (centred and fitted on the next frame).
    pub fn go(&mut self, i: usize) {
        self.preset = i;
        self.view = View::new(self.info.ladder[i].half).with_home(self.info.ladder[i].centre);
    }

    /// The window the view shows now, as a request.
    fn window(&self, rect: Rect) -> RasterReq {
        let p = &self.info.ladder[self.preset];
        let w = [rect.width() as f64 / self.view.scale, rect.height() as f64 / self.view.scale];
        let k = (MAX_PX as f64 / rect.width().max(rect.height()) as f64).min(1.0);
        RasterReq {
            id: 0,
            basis: p.basis,
            centre: self.view.centre,
            depth: p.depth,
            width: w,
            px: [((rect.width() as f64 * k) as usize).max(8), ((rect.height() as f64 * k) as usize).max(8)],
            param: self.param,
        }
    }

    /// Each frame: ask for a raster of the current window once the view has
    /// settled and nothing is in flight. Returns the request to send.
    pub fn pump(&mut self, rect: Rect) -> Option<RasterReq> {
        let w = self.window(rect);
        if self.last_window.as_ref().is_none_or(|l| !same(l, &w)) {
            self.last_window = Some(w);
            self.still_since = now_s();
        }
        let up_to_date = self.shown.as_ref().is_some_and(|(r, _, _)| same(r, &w));
        if self.pending.is_none() && !up_to_date && now_s() - self.still_since >= SETTLE_S {
            let req = RasterReq { id: self.next_id, ..w };
            self.next_id += 1;
            self.pending = Some(req);
            return Some(req);
        }
        None
    }

    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    /// Whether the newest image answers the current window (no repaint needed).
    pub fn settled(&self) -> bool {
        self.shown.as_ref().zip(self.last_window.as_ref()).is_some_and(|((r, _, _), w)| same(r, w))
    }

    /// A raster arrived: colour it into a texture.
    pub fn receive(&mut self, ctx: &egui::Context, req: RasterReq, map: Vec<u8>, secs: f64) {
        if self.pending.is_some_and(|p| p.id == req.id) {
            self.pending = None;
        }
        if map.len() != req.px[0] * req.px[1] {
            return;
        }
        self.last_secs = secs;
        let pixels: Vec<Color32> = map.iter().map(|&m| self.colour(m)).collect();
        let img = ColorImage { size: req.px, source_size: Vec2::new(req.px[0] as f32, req.px[1] as f32), pixels };
        let tex = ctx.load_texture("geometry-slice", img, TextureOptions::NEAREST);
        self.shown = Some((req, tex, map));
    }

    pub fn colour(&self, m: u8) -> Color32 {
        match m {
            VOID => Color32::from_rgb(232, 236, 244),
            OUTSIDE => Color32::from_rgb(14, 16, 20),
            OVERLAP => Color32::from_rgb(255, 0, 255),
            i => self.info.palette.get(i as usize).map_or(Color32::GRAY, |c| c.0),
        }
    }

    /// Draw the newest image where its window lies under the current view.
    pub fn draw(&self, painter: &egui::Painter, rect: Rect) {
        if let Some((r, tex, _)) = &self.shown {
            if r.basis == self.info.ladder[self.preset].basis && r.depth == self.info.ladder[self.preset].depth {
                let a = self.view.to_screen(rect, r.centre[0] - 0.5 * r.width[0], r.centre[1] + 0.5 * r.width[1]);
                let b = self.view.to_screen(rect, r.centre[0] + 0.5 * r.width[0], r.centre[1] - 0.5 * r.width[1]);
                painter.image(tex.id(), Rect::from_two_pos(a, b), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }
        }
    }

    /// Materials present in the newest image, for the legend.
    pub fn present(&self) -> Vec<(Color32, &'static str)> {
        let Some((_, _, map)) = &self.shown else { return Vec::new() };
        let mut seen = [false; 256];
        for &m in map {
            seen[m as usize] = true;
        }
        let mut out: Vec<(Color32, &'static str)> =
            (0..self.info.palette.len()).filter(|&i| seen[i]).map(|i| (self.info.palette[i].0, self.info.palette[i].1)).collect();
        if seen[VOID as usize] {
            out.push((self.colour(VOID), "void"));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_raster_request_crosses_the_worker_boundary() {
        let r = RasterReq { id: 7, basis: Basis::Xz, centre: [1.5, -2.0], depth: 0.25, width: [3.0, 4.5], px: [300, 450], param: 12.0 };
        assert_eq!(RasterReq::decode(&r.encode()).unwrap(), r);
    }
}
