//! Step 1's fresh DEM pour: a third engine thread and the pebble view.
//!
//! The maintainer asked to run a fresh DEM fill of `x` pebbles in the early
//! steps (gh:#561). The physics is `outram_park_fork_liggghts::htr10_fill`
//! (the LIGGGHTS port's `GranularSystem`, HTR-10 vessel, V&V § 4.9 contact
//! defaults); this file only runs it off the UI thread in chunks, streams the
//! pebble positions, and draws them. The pour gets its own thread so slices
//! and 3D renders stay live during the minutes it takes. Since 2026-10-05
//! (gh:#587) the pebbles are drawn on the GPU as sphere impostors
//! (`gpu_view::PebbleGpu`): the UI thread uploads the streamed centres and
//! submits one instanced draw (1.2-1.3 ms of CPU time for 27 000 pebbles),
//! instead of sorting and painting 27 000 egui discs; the discs remain the
//! fallback without a wgpu device.

use std::sync::{Arc, RwLock};

use dhoby_ghaut::web_demo::link::NativeEngine;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use outram_park_fork_liggghts::htr10_fill::{
    FillProgress, Htr10Fill, Htr10FillSettings, CONE_HEIGHT_M, CORE_RADIUS_M, PEBBLE_RADIUS_M, TUBE_RADIUS_M,
    VALVE_Z_M,
};

use crate::gpu_view::{Gpu, PebbleCamera, PebbleGpu};

/// Set to `true` to stop a pour at the next chunk boundary.
pub type StopFlag = Arc<RwLock<bool>>;

pub enum DemReq {
    Run { settings: Htr10FillSettings, chunk: usize, stop: StopFlag },
}

pub enum DemEv {
    Progress { progress: FillProgress, centres: Vec<[f32; 3]> },
    Done { progress: FillProgress, centres: Vec<[f64; 3]>, stopped: bool },
    Error(String),
}

#[derive(Default)]
pub struct DemEngine;

impl NativeEngine for DemEngine {
    type Req = DemReq;
    type Ev = DemEv;

    fn handle(&mut self, req: DemReq, post: &mut impl FnMut(DemEv)) {
        let DemReq::Run { settings, chunk, stop } = req;
        let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut fill = match Htr10Fill::new(settings) {
                Ok(f) => f,
                Err(e) => {
                    post(DemEv::Error(format!("DEM set-up refused: {e:?}")));
                    return;
                }
            };
            let first = fill.progress();
            post(DemEv::Progress { progress: first, centres: f32_centres(&fill) });
            loop {
                let p = fill.advance(chunk);
                let stopped = stop.read().map_or(false, |s| *s);
                if p.settled || p.gave_up || stopped {
                    let centres = fill.centres().iter().map(|c| [c.x, c.y, c.z]).collect();
                    post(DemEv::Done { progress: p, centres, stopped: stopped && !p.settled });
                    return;
                }
                post(DemEv::Progress { progress: p, centres: f32_centres(&fill) });
            }
        }));
        if let Err(e) = run {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_string()))
                .unwrap_or_else(|| "unknown panic".into());
            post(DemEv::Error(format!("DEM pour failed: {msg}")));
        }
    }
}

fn f32_centres(fill: &Htr10Fill) -> Vec<[f32; 3]> {
    fill.centres().iter().map(|c| [c.x as f32, c.y as f32, c.z as f32]).collect()
}

/// The pebble view: every pebble as a shaded sphere with the vessel outline;
/// orbit by dragging, zoom with the wheel or buttons. Drawn on the GPU as
/// sphere impostors (`gpu_view::PebbleGpu`, gh:#587) when eframe runs on
/// wgpu; otherwise every pebble is an egui disc, painted back to front.
pub struct DemView {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    /// Show only the half `y > 0` (a cutaway, so the inside of the bed shows).
    pub half: bool,
    /// The GPU impostor renderer, when there is a wgpu device.
    gpu: Option<PebbleGpu>,
}

impl DemView {
    pub fn new() -> Self {
        Self { yaw: -0.9, pitch: 0.25, zoom: 1.0, half: true, gpu: None }
    }

    /// Draw on `gpu`'s wgpu device from now on (the egui painter otherwise).
    pub fn use_gpu(&mut self, gpu: &Gpu) {
        match gpu.render_state().map(|rs| PebbleGpu::new(&rs)) {
            Some(Ok(p)) => self.gpu = Some(p),
            Some(Err(e)) => eprintln!("dhoby-ghaut: DEM view on the CPU painter: {e}"),
            None => {}
        }
    }

    fn project(&self, p: [f32; 3]) -> (f32, f32, f32) {
        let (cy, sy) = (self.yaw.cos(), self.yaw.sin());
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        // Screen right, screen up, and depth toward the viewer.
        let x = -sy * p[0] + cy * p[1];
        let depth = cp * (cy * p[0] + sy * p[1]) + sp * p[2];
        let y = -sp * (cy * p[0] + sy * p[1]) + cp * p[2];
        (x, y, depth)
    }

    /// The CPU fallback: every pebble an egui disc, sorted back to front.
    fn paint_discs(&self, painter: &egui::Painter, centres: &[[f32; 3]], r_px: f32, to_screen: impl Fn(f32, f32) -> Pos2) {
        let mut shown: Vec<(f32, f32, f32, f32)> = centres
            .iter()
            .filter(|c| !self.half || c[1] >= 0.0)
            .map(|c| {
                let (x, y, d) = self.project(*c);
                (d, x, y, c[2])
            })
            .collect();
        shown.sort_by(|a, b| a.0.total_cmp(&b.0));
        let r_px = r_px.max(1.0);
        let (dmin, dmax) = shown.iter().fold((f32::MAX, f32::MIN), |(lo, hi), s| (lo.min(s.0), hi.max(s.0)));
        for (d, x, y, z) in &shown {
            // Colour by height (conus and tube darker), shade by depth.
            let base = if *z < 0.0 { [120.0, 120.0, 128.0] } else { [175.0, 175.0, 182.0] };
            let k = if dmax > dmin { 0.55 + 0.45 * (d - dmin) / (dmax - dmin) } else { 1.0 };
            let c = Color32::from_rgb((base[0] * k) as u8, (base[1] * k) as u8, (base[2] * k) as u8);
            painter.circle(to_screen(*x, *y), r_px, c, Stroke::new(0.5, Color32::from_gray(30)));
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, centres: &[[f32; 3]], progress: Option<&FillProgress>) -> Rect {
        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        if resp.dragged() {
            let d = resp.drag_delta();
            self.yaw -= d.x * 0.008;
            self.pitch = (self.pitch + d.y * 0.008).clamp(-1.4, 1.4);
        }
        if resp.hovered() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            self.zoom = (self.zoom * zoom * (scroll * 0.002).exp()).clamp(0.3, 20.0);
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(40, 40, 46));
        // Model extent: from the valve to the top of the tallest pebble or 2 m.
        let top = centres.iter().map(|c| c[2]).fold(1.0f32, f32::max);
        let (zlo, zhi) = (VALVE_Z_M as f32, top + PEBBLE_RADIUS_M as f32);
        let extent = (zhi - zlo).max(2.0 * CORE_RADIUS_M as f32) * 0.55;
        let scale = rect.height().min(rect.width()) / (2.0 * extent) * self.zoom;
        let zmid = 0.5 * (zlo + zhi);
        let to_screen = |x: f32, y: f32| rect.center() + Vec2::new(x * scale, -(y - zmid) * scale);
        // Vessel outline: barrel, conus and tube silhouettes.
        let wall = Stroke::new(1.5, Color32::from_gray(150));
        let rims = [
            (zhi, CORE_RADIUS_M as f32),
            (0.0, CORE_RADIUS_M as f32),
            (-CONE_HEIGHT_M as f32, TUBE_RADIUS_M as f32),
            (VALVE_Z_M as f32, TUBE_RADIUS_M as f32),
        ];
        for w in rims.windows(2) {
            for side in [-1.0f32, 1.0] {
                let (z0, r0) = w[0];
                let (z1, r1) = w[1];
                let a = self.project([0.0, 0.0, z0]);
                let b = self.project([0.0, 0.0, z1]);
                painter.line_segment([to_screen(a.0 + side * r0, a.1), to_screen(b.0 + side * r1, b.1)], wall);
            }
        }
        let r_m = PEBBLE_RADIUS_M as f32;
        if let Some(pg) = self.gpu.as_mut() {
            // On the GPU: sphere impostors with a depth buffer, no sort.
            let ppp = ui.ctx().pixels_per_point();
            let size = [(rect.width() * ppp).round() as u32, (rect.height() * ppp).round() as u32];
            let reach = (CORE_RADIUS_M as f32).hypot(zlo.abs().max(zhi.abs())) + r_m + 0.1;
            let cam = PebbleCamera {
                yaw: self.yaw,
                pitch: self.pitch,
                ndc_per_m: [scale / (0.5 * rect.width()), scale / (0.5 * rect.height())],
                z_mid: zmid,
                radius: r_m,
                depth_extent: reach,
                half: self.half,
            };
            let tex = pg.draw(size, centres, &cam);
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            painter.image(tex, rect, uv, Color32::WHITE);
        } else {
            self.paint_discs(&painter, centres, r_m * scale, to_screen);
        }
        let mut lines = vec![format!(
            "{} pebbles{}  ({})",
            centres.len(),
            if self.half { " (half shown: y > 0)" } else { "" },
            match &self.gpu {
                Some(p) => format!("GPU impostors, {:.1} ms to submit", p.last_submit_ms),
                None => "CPU painter".into(),
            }
        )];
        if let Some(p) = progress {
            lines.push(format!(
                "step {}  t = {:.2} s  KE/E_drop {:.2e}  phi {:.4}  surface {:.1} cm",
                p.steps,
                p.time.get::<uom::si::time::second>(),
                p.ke_ratio_core,
                p.phi_whole_core,
                p.surface_height.get::<uom::si::length::centimeter>()
            ));
        }
        painter.text(
            rect.left_bottom() + Vec2::new(10.0, -10.0),
            egui::Align2::LEFT_BOTTOM,
            lines.join("\n"),
            crate::style::Text::Small.font(),
            Color32::from_gray(225),
        );
        // + / − / half toggle, top right.
        let h = crate::style::button_height();
        let w = crate::style::Text::Body.size() * 2.2;
        let mut x = rect.right() - 8.0 - 3.0 * (w + 6.0) - crate::style::Text::Body.size() * 3.0;
        let y = rect.top() + 8.0;
        for (label, f) in [("+", 1.3f32), ("−", 1.0 / 1.3)] {
            if ui.put(Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h)), egui::Button::new(egui::RichText::new(label).size(crate::style::Text::Emphasis.size()))).clicked() {
                self.zoom = (self.zoom * f).clamp(0.3, 20.0);
            }
            x += w + 6.0;
        }
        let label = if self.half { "Whole" } else { "Half" };
        if ui
            .put(Rect::from_min_size(Pos2::new(x, y), Vec2::new(crate::style::Text::Body.size() * 4.0, h)), egui::Button::new(egui::RichText::new(label).size(crate::style::Text::Body.size())))
            .on_hover_text("Show the whole bed, or only the half y > 0 to see inside")
            .clicked()
        {
            self.half = !self.half;
        }
        if centres.len() > 30_000 {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        }
        rect
    }
}
