//! The 3D viewport of Steps 1-3: a Blender-like view of the ASSEMBLED
//! geometry (maintainer, 2026-10-05), with the 2D slices one click away.
//!
//! Every picture is a ray trace through the solver's own geometry (OpenMC's
//! solid and wireframe ray-traced plots, ported in `outram-blender`), so what
//! is drawn is what the solver sees, as the crate's drawing rule asks. There
//! is no triangle mesh of the CSG anywhere in the workspace; ray tracing is
//! the faithful path. ~~It runs on the geometry engine thread.~~ **CHANGED
//! 2026-10-05 (gh:#587):** it runs **on the GPU** (`crate::gpu_view`,
//! `outram_blender::csg::gpu`), every frame the camera moves, at full
//! resolution; the CPU tracer on the geometry engine thread is the fallback
//! (no wgpu device, a geometry the GPU tracer refuses, or the moment before
//! the flattened geometry reaches the GPU) and the reference the GPU tracer
//! is tested against (`tests/gpu_csg_parity.rs`).
//!
//! Controls, Blender's where a mouse has the buttons:
//! - **orbit**: middle-drag, or left-drag;
//! - **pan**: Shift + middle-drag, or right-drag;
//! - **zoom**: wheel (and the + / − buttons);
//! - **views**: Front, Right, Top, Iso (Blender's numpad 1 / 3 / 7), Persp/Ortho,
//!   Frame all (Home);
//! - **section**: cut the view with a plane normal to x, y or z (Blender's
//!   clipping), flip which side is kept, slide it; the cut face is painted in
//!   the material's colour (solid shading);
//! - **outliner**: show or hide each material in the settings panel.
//!
//! On the CPU fallback, while the camera moves a quarter-resolution preview
//! is traced; the full picture follows 0.35 s after it stops.

use egui::{Color32, Pos2, Rect, RichText, TextureHandle, TextureOptions, Vec2};

use outram_blender::csg::gpu::render::GpuPlot;
use outram_mc_libs::geometry::plot::{
    Camera, ClipPlane, ColourScheme, PlotColourBy, Projection, Rgb, SolidRayTracePlot,
    WireframeRayTracePlot, DEFAULT_PLOTTER_SEED,
};
use outram_mc_libs::geometry::position::{Direction, Position};

use crate::engine::{palette, Ev, Req, Shading, View3dJob};
use crate::gpu_view::{Gpu, GpuSlot, GpuStatus};

/// The plot a 3D job asks for, and its colours: OpenMC's solid ray trace
/// (hidden materials left out of the opaque set, the section plane) or its
/// wireframe ray trace (hidden materials given no attenuation and no
/// outline). One description for both tracers: the CPU on the engine thread
/// (`engine::render_3d`) and the GPU (`gpu_view`). `n` is the material table
/// length, `material_count(geometry)`; the palette's is used if longer.
pub fn plot_for(job: &View3dJob, n: usize) -> (GpuPlot, ColourScheme) {
    let pal = palette();
    let n = n.max(pal.len());
    let mut seed = DEFAULT_PLOTTER_SEED;
    let mut scheme = ColourScheme::new(PlotColourBy::Material, n, &mut seed)
        .with_background(Rgb::new(48, 48, 52));
    for (i, (c, _)) in pal.iter().enumerate() {
        scheme = scheme.with_colour(i, *c);
    }
    let camera = Camera {
        position: Position::new(job.eye[0], job.eye[1], job.eye[2]),
        look_at: Position::new(job.look_at[0], job.look_at[1], job.look_at[2]),
        up: Direction::new(0.0, 0.0, 1.0),
        pixels: job.pixels,
        projection: match job.ortho_width {
            Some(width) => Projection::Orthographic { width },
            None => Projection::Perspective {
                horizontal_fov_deg: job.fov_deg,
            },
        },
    };
    let shown = |i: usize| job.visible.get(i).copied().unwrap_or(true);
    let plot = match job.shading {
        Shading::Solid => {
            let mut plot = SolidRayTracePlot::new(camera, n);
            for i in (0..n).filter(|&i| shown(i)) {
                plot = plot.with_opaque(i);
            }
            plot.diffuse_fraction = 0.35;
            if let Some((normal, offset)) = job.clip {
                plot = plot.with_clip(ClipPlane { normal, offset });
            }
            GpuPlot::Solid(plot)
        }
        Shading::XRay => {
            let mut plot = WireframeRayTracePlot::new(camera, n);
            for i in 0..n {
                plot = plot.with_xs(i, if shown(i) { 0.02 } else { 0.0 });
            }
            // Outline only what is shown: hidden pebbles' boundaries would
            // otherwise cover the picture.
            plot.wireframe_ids = (0..n).filter(|&i| shown(i)).collect();
            GpuPlot::Wireframe(plot)
        }
    };
    (plot, scheme)
}

pub struct View3d {
    /// Point the camera orbits \[cm\].
    pub target: [f64; 3],
    /// Azimuth from +x and elevation \[rad\].
    pub yaw: f64,
    pub pitch: f64,
    /// Eye distance from the target \[cm\].
    pub distance: f64,
    pub ortho: bool,
    pub shading: Shading,
    /// Per material: drawn or hidden.
    pub visible: Vec<bool>,
    /// Size of the model, for Frame all \[cm\].
    pub extent: f64,
    /// Section cut: axis (0 x, 1 y, 2 z), offset \[cm\] and which side is kept
    /// (`true`: the + side). `None` is no cut. Solid shading only.
    pub cut: Option<(usize, f64, bool)>,
    texture: Option<TextureHandle>,
    next_id: u64,
    pending: Option<(u64, bool)>,
    changed_at: f64,
    dirty_full: bool,
    dirty_preview: bool,
    last_key: Vec<i64>,
    pub last_seconds: f64,
    /// The GPU tracer (gh:#587), shared with the slice view; `None` draws on
    /// the CPU.
    pub gpu: Option<Gpu>,
    slot: GpuSlot,
    /// The picture on screen came from the GPU.
    on_gpu: bool,
    /// The last GPU job was at full resolution; and whether full-resolution
    /// frames are fast enough (< 30 ms) to trace while the camera moves.
    last_job_full: bool,
    full_res_fast: bool,
}

impl View3d {
    pub fn new() -> Self {
        Self {
            target: [0.0, 0.0, 0.0],
            yaw: -1.0,
            pitch: 0.45,
            distance: 900.0,
            ortho: false,
            shading: Shading::Solid,
            visible: Vec::new(),
            extent: 320.0,
            cut: None,
            texture: None,
            next_id: 1,
            pending: None,
            changed_at: 0.0,
            dirty_full: true,
            dirty_preview: false,
            last_key: Vec::new(),
            last_seconds: 0.0,
            gpu: None,
            slot: GpuSlot::new(),
            on_gpu: false,
            last_job_full: false,
            full_res_fast: false,
        }
    }

    /// Blender's "frame all": look at `centre` from far enough to see `extent`.
    pub fn frame(&mut self, centre: [f64; 3], extent: f64) {
        self.target = centre;
        self.extent = extent;
        self.distance = 2.6 * extent;
        self.dirty_full = true;
    }

    pub fn invalidate(&mut self) {
        self.texture = None;
        self.slot.clear();
        self.dirty_full = true;
        // A render already in flight belongs to the old view: ignore it.
        self.pending = None;
    }

    fn eye(&self) -> [f64; 3] {
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        let (cy, sy) = (self.yaw.cos(), self.yaw.sin());
        [
            self.target[0] + self.distance * cp * cy,
            self.target[1] + self.distance * cp * sy,
            self.target[2] + self.distance * sp,
        ]
    }

    /// Screen-right and screen-up unit vectors in world space.
    fn basis(&self) -> ([f64; 3], [f64; 3]) {
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        let (cy, sy) = (self.yaw.cos(), self.yaw.sin());
        let right = [-sy, cy, 0.0];
        let up = [-sp * cy, -sp * sy, cp];
        (right, up)
    }

    pub fn on_event(&mut self, ctx: &egui::Context, ev: &Ev) -> bool {
        if let Ev::View3d { id, image, seconds } = ev {
            let Some((want, full)) = self.pending else {
                return false;
            };
            if *id != want {
                return false;
            }
            self.pending = None;
            if full {
                self.last_seconds = *seconds;
            }
            self.on_gpu = false;
            let mut bytes = Vec::with_capacity(image.pixels.len() * 3);
            for p in &image.pixels {
                bytes.extend_from_slice(&[p.r, p.g, p.b]);
            }
            let ci = egui::ColorImage::from_rgb([image.width, image.height], &bytes);
            self.texture = Some(ctx.load_texture("view3d", ci, TextureOptions::LINEAR));
            return true;
        }
        false
    }

    /// The render job for the view in `rect` at `k` pixels per point (the
    /// CPU's full picture caps the longer side at 900 pixels and its preview
    /// is a quarter of that; the GPU draws every screen pixel).
    fn job(&mut self, rect: Rect, k: f32, full: bool) -> View3dJob {
        let pixels = [
            ((rect.width() * k) as usize).max(32),
            ((rect.height() * k) as usize).max(24),
        ];
        let id = self.next_id;
        self.next_id += 1;
        self.pending = Some((id, full));
        View3dJob {
            id,
            eye: self.eye(),
            look_at: self.target,
            pixels,
            ortho_width: self.ortho.then_some(self.distance * 0.9),
            fov_deg: 50.0,
            shading: self.shading,
            visible: self.visible.clone(),
            clip: self.cut.map(|(axis, offset, plus)| {
                let mut n = [0.0; 3];
                n[axis] = if plus { 1.0 } else { -1.0 };
                (n, if plus { offset } else { -offset })
            }),
        }
    }

    /// Draw the viewport and its toolbar; send renders when it is out of date.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        now: f64,
        have_geometry: bool,
        send: &mut impl FnMut(Req),
    ) -> Rect {
        let (rect, resp) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(48, 48, 52));
        let gpu_tex = self.gpu.as_ref().and_then(|g| self.slot.texture(g));
        let shown = if self.on_gpu { gpu_tex } else { None };
        if let Some(t) = shown.or(self.texture.as_ref().map(TextureHandle::id)) {
            painter.image(
                t,
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        // Navigation.
        let (shift, scroll, zoom) =
            ui.input(|i| (i.modifiers.shift, i.smooth_scroll_delta.y, i.zoom_delta()));
        let middle = resp.dragged_by(egui::PointerButton::Middle);
        let pan = resp.dragged_by(egui::PointerButton::Secondary) || (middle && shift);
        let orbit = !pan && (middle || resp.dragged_by(egui::PointerButton::Primary));
        let d = resp.drag_delta();
        if orbit {
            self.yaw -= f64::from(d.x) * 0.008;
            self.pitch = (self.pitch + f64::from(d.y) * 0.008).clamp(-1.45, 1.45);
        }
        if pan {
            let (r, u) = self.basis();
            let s = self.distance / f64::from(rect.height().max(1.0)) * 0.9;
            for a in 0..3 {
                self.target[a] -= (r[a] * f64::from(d.x) - u[a] * f64::from(d.y)) * s;
            }
        }
        if resp.hovered() {
            let f = f64::from(zoom) * (f64::from(scroll) * 0.002).exp();
            if (f - 1.0).abs() > 1e-6 {
                self.distance = (self.distance / f).clamp(0.5, 20.0 * self.extent);
            }
        }
        if resp.double_clicked() || ui.input(|i| i.key_pressed(egui::Key::Home)) {
            let (t, e) = (self.target, self.extent);
            self.frame(t, e);
        }
        self.toolbar(ui, rect);
        self.gizmo(&painter, rect);
        let gpu_ready = have_geometry && self.gpu.as_ref().is_some_and(Gpu::poll);
        let gpu_busy = self.slot.busy();
        // The GPU became ready after a CPU picture (or has a new geometry):
        // trace it there, without waiting for the camera to move.
        if gpu_ready && gpu_tex.is_none() && !gpu_busy {
            self.dirty_full = true;
        }
        if !gpu_busy && self.last_job_full && self.slot.last_seconds > 0.0 {
            self.full_res_fast = self.slot.last_seconds < 0.03;
        }
        let fallback = match self.gpu.as_ref().map(Gpu::status) {
            None => " (CPU: no wgpu device)".to_string(),
            Some(GpuStatus::Preparing) => " (CPU while the GPU loads the geometry)".to_string(),
            Some(GpuStatus::Refused(why) | GpuStatus::Off(why)) => format!(" (CPU: {why})"),
            Some(GpuStatus::Ready) => String::new(),
        };
        let mode = if self.ortho { "ortho" } else { "persp" };
        let status = if !have_geometry {
            "Assembling the geometry…".to_string()
        } else if self.on_gpu && shown.is_some() {
            format!(
                "GPU ray-traced in {:.0} ms · {mode} · drag: orbit · right/Shift+middle: pan · wheel: zoom",
                1000.0 * self.slot.last_seconds,
            )
        } else if self.pending.is_some_and(|p| p.1) {
            format!("ray tracing on the CPU…{fallback}")
        } else {
            format!(
                "CPU ray-traced in {:.1} s{fallback} · {mode} · drag: orbit · right/Shift+middle: pan · wheel: zoom",
                self.last_seconds,
            )
        };
        painter.text(
            rect.left_bottom() + Vec2::new(10.0, -10.0),
            egui::Align2::LEFT_BOTTOM,
            status,
            crate::style::Text::Small.font(),
            Color32::from_gray(220),
        );

        // Re-render: a preview while moving, the full picture once still.
        let q = |x: f64| (x * 1000.0).round() as i64;
        let mut key = vec![
            q(self.yaw),
            q(self.pitch),
            q(self.distance),
            q(self.target[0]),
            q(self.target[1]),
            q(self.target[2]),
        ];
        key.extend([
            i64::from(self.ortho),
            i64::from(self.shading == Shading::Solid),
            rect.width() as i64,
            rect.height() as i64,
        ]);
        key.extend(self.visible.iter().map(|v| i64::from(*v)));
        if let Some((a, o, p)) = self.cut {
            key.extend([a as i64, q(o), i64::from(p)]);
        }
        if key != self.last_key {
            self.last_key = key;
            self.changed_at = now;
            self.dirty_full = true;
            self.dirty_preview = true;
        }
        if gpu_ready {
            // GPU: every change, at screen resolution, as soon as the last
            // frame has run (frames are never queued behind a slow one).
            // While the camera moves, full resolution if the last full frame
            // took under 30 ms, else half resolution (a quarter of the
            // rays); the full picture once it has been still 0.15 s. ~~The
            // HTR-10 half-section takes ~0.4 s at full resolution on an RTX
            // A5000.~~ Since the grid index (2026-10-05, gh:#587): 17 ms.
            let still = now - self.changed_at > 0.15;
            if (self.dirty_full || self.dirty_preview) && !gpu_busy && (still || self.dirty_preview)
            {
                let ppp = ui.ctx().pixels_per_point();
                let full = still || self.full_res_fast;
                self.last_job_full = full;
                let job = self.job(rect, if full { ppp } else { 0.5 * ppp }, still);
                // A CPU picture still in flight belongs to an older view.
                self.pending = None;
                if let Some(g) = &self.gpu {
                    let (plot, scheme) = plot_for(&job, g.n_materials());
                    if g.draw(
                        &mut self.slot,
                        &plot,
                        &scheme,
                        eframe::egui_wgpu::wgpu::FilterMode::Linear,
                        false,
                    ) {
                        self.dirty_full &= !still;
                        self.dirty_preview = false;
                        self.on_gpu = true;
                    }
                }
            }
        } else if have_geometry && self.pending.is_none() {
            let still = now - self.changed_at > 0.35;
            if self.dirty_full && still {
                self.dirty_full = false;
                self.dirty_preview = false;
                let k = (900.0 / rect.width().max(rect.height())).min(1.0);
                let job = self.job(rect, k, true);
                send(Req::Render3d(job));
            } else if self.dirty_preview && !still {
                self.dirty_preview = false;
                let job = self.job(rect, 0.25, false);
                send(Req::Render3d(job));
            }
        }
        if gpu_busy || (gpu_ready && (self.dirty_full || self.dirty_preview)) {
            ui.ctx().request_repaint();
        }
        if self.dirty_full || self.pending.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(60));
        }
        rect
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let views: [(&str, f64, f64, &str); 4] = [
            (
                "Front",
                -std::f64::consts::FRAC_PI_2,
                0.0,
                "Looking along +y (Blender numpad 1)",
            ),
            ("Right", 0.0, 0.0, "Looking along −x (numpad 3)"),
            (
                "Top",
                -std::f64::consts::FRAC_PI_2,
                1.45,
                "Looking down −z (numpad 7)",
            ),
            ("Iso", -0.8, 0.6, "Three-quarter view"),
        ];
        let h = crate::style::button_height();
        let mut x = rect.left() + 8.0;
        let mut y = rect.top() + 8.0;
        // The + / − pair sits top right; view buttons wrap to a new row
        // before reaching it, so nothing overlaps at any UI scale (gh:#586).
        let row_end = rect.right() - 8.0 - 2.0 * (crate::style::em(2.2) + 6.0);
        let mut button =
            |ui: &mut egui::Ui, label: &str, w: f32, selected: bool, hover: &str| -> bool {
                if x + w > row_end && x > rect.left() + 8.0 {
                    x = rect.left() + 8.0;
                    y += h + 6.0;
                }
                let b = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
                x += w + 6.0;
                ui.put(
                    b,
                    egui::Button::selectable(selected, RichText::new(label).size(crate::style::Text::Body.size())),
                )
                .on_hover_text(hover)
                .clicked()
            };
        for (label, yaw, pitch, hover) in views {
            if button(ui, label, crate::style::Text::Body.size() * 3.4, false, hover) {
                self.yaw = yaw;
                self.pitch = pitch;
            }
        }
        if button(
            ui,
            "Frame all",
            crate::style::Text::Body.size() * 5.0,
            false,
            "Fit the model (Home, or double-click)",
        ) {
            let (t, e) = (self.target, self.extent);
            self.frame(t, e);
        }
        let ortho = self.ortho;
        if button(
            ui,
            if ortho { "Ortho" } else { "Persp" },
            crate::style::Text::Body.size() * 3.6,
            false,
            "Toggle perspective / orthographic (numpad 5)",
        ) {
            self.ortho = !self.ortho;
        }
        let solid = self.shading == Shading::Solid;
        if button(ui, "Solid", crate::style::Text::Body.size() * 3.4, solid, "Shaded opaque surfaces") {
            self.shading = Shading::Solid;
        }
        if button(
            ui,
            "X-ray",
            crate::style::Text::Body.size() * 3.4,
            !solid,
            "See-through, boundaries outlined",
        ) {
            self.shading = Shading::XRay;
        }
        // Zoom buttons, top right, for a mouse without a wheel.
        let w = crate::style::Text::Body.size() * 2.2;
        for (i, (label, f)) in [("+", 1.3), ("−", 1.0 / 1.3)].into_iter().enumerate() {
            let b = Rect::from_min_size(
                Pos2::new(rect.right() - 8.0 - (2 - i) as f32 * (w + 6.0), rect.top() + 8.0),
                Vec2::new(w, h),
            );
            if ui
                .put(b, egui::Button::new(RichText::new(label).size(crate::style::Text::Emphasis.size())))
                .clicked()
            {
                self.distance = (self.distance / f).clamp(0.5, 20.0 * self.extent);
            }
        }
    }

    /// Blender's navigation gizmo: the world axes as seen by the camera.
    fn gizmo(&self, painter: &egui::Painter, rect: Rect) {
        let c = Pos2::new(rect.right() - 70.0, rect.bottom() - 80.0);
        let (r, u) = self.basis();
        let axes = [
            ("X", [1.0, 0.0, 0.0], Color32::from_rgb(230, 70, 70)),
            ("Y", [0.0, 1.0, 0.0], Color32::from_rgb(110, 200, 70)),
            ("Z", [0.0, 0.0, 1.0], Color32::from_rgb(70, 130, 240)),
        ];
        painter.circle_filled(c, 52.0, Color32::from_black_alpha(90));
        for (name, a, col) in axes {
            let sx = (r[0] * a[0] + r[1] * a[1] + r[2] * a[2]) as f32;
            let sy = (u[0] * a[0] + u[1] * a[1] + u[2] * a[2]) as f32;
            let tip = c + Vec2::new(sx, -sy) * 42.0;
            painter.line_segment([c, tip], egui::Stroke::new(3.0, col));
            painter.circle_filled(tip, crate::style::em(0.6), col);
            painter.text(
                tip,
                egui::Align2::CENTER_CENTER,
                name,
                crate::style::Text::Tiny.font(),
                Color32::BLACK,
            );
        }
    }
}
