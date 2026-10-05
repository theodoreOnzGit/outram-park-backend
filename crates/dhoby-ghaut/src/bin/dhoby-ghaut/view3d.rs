//! The 3D viewport of Steps 1-3: a Blender-like view of the ASSEMBLED
//! geometry (maintainer, 2026-10-05), with the 2D slices one click away.
//!
//! Every picture is a ray trace through the solver's own geometry (OpenMC's
//! solid and wireframe ray-traced plots, ported in `outram-blender`), so what
//! is drawn is what the solver sees, as the crate's drawing rule asks. There
//! is no triangle mesh of the CSG anywhere in the workspace; ray tracing is
//! the faithful path, and it runs on the geometry engine thread.
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
//! While the camera moves a quarter-resolution preview is traced; the full
//! picture follows 0.35 s after it stops.

use egui::{Color32, Pos2, Rect, RichText, TextureHandle, TextureOptions, Vec2};

use crate::app::fs;
use crate::engine::{Ev, Req, Shading, View3dJob};

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
        self.dirty_full = true;
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

    fn job(&mut self, rect: Rect, full: bool) -> View3dJob {
        let k = if full {
            (900.0 / rect.width().max(rect.height())).min(1.0)
        } else {
            0.25
        };
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
        if let Some(t) = &self.texture {
            painter.image(
                t.id(),
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
        let status = if !have_geometry {
            "Assembling the geometry…".to_string()
        } else if self.pending.is_some_and(|p| p.1) {
            "ray tracing…".to_string()
        } else {
            format!(
                "ray-traced in {:.1} s · {} · drag: orbit · right/Shift+middle: pan · wheel: zoom",
                self.last_seconds,
                if self.ortho { "ortho" } else { "persp" }
            )
        };
        painter.text(
            rect.left_bottom() + Vec2::new(10.0, -10.0),
            egui::Align2::LEFT_BOTTOM,
            status,
            egui::FontId::proportional(fs(12.0)),
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
        if have_geometry && self.pending.is_none() {
            let still = now - self.changed_at > 0.35;
            if self.dirty_full && still {
                self.dirty_full = false;
                self.dirty_preview = false;
                let job = self.job(rect, true);
                send(Req::Render3d(job));
            } else if self.dirty_preview && !still {
                self.dirty_preview = false;
                let job = self.job(rect, false);
                send(Req::Render3d(job));
            }
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
        let h = fs(15.0) + 14.0;
        let mut x = rect.left() + 8.0;
        let y = rect.top() + 8.0;
        let mut button =
            |ui: &mut egui::Ui, label: &str, w: f32, selected: bool, hover: &str| -> bool {
                let b = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
                x += w + 6.0;
                ui.put(
                    b,
                    egui::Button::selectable(selected, RichText::new(label).size(fs(13.0))),
                )
                .on_hover_text(hover)
                .clicked()
            };
        for (label, yaw, pitch, hover) in views {
            if button(ui, label, fs(13.0) * 3.4, false, hover) {
                self.yaw = yaw;
                self.pitch = pitch;
            }
        }
        if button(
            ui,
            "Frame all",
            fs(13.0) * 5.0,
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
            fs(13.0) * 3.6,
            false,
            "Toggle perspective / orthographic (numpad 5)",
        ) {
            self.ortho = !self.ortho;
        }
        let solid = self.shading == Shading::Solid;
        if button(ui, "Solid", fs(13.0) * 3.4, solid, "Shaded opaque surfaces") {
            self.shading = Shading::Solid;
        }
        if button(
            ui,
            "X-ray",
            fs(13.0) * 3.4,
            !solid,
            "See-through, boundaries outlined",
        ) {
            self.shading = Shading::XRay;
        }
        // Zoom buttons, top right, for a mouse without a wheel.
        let w = fs(13.0) * 2.2;
        for (i, (label, f)) in [("+", 1.3), ("−", 1.0 / 1.3)].into_iter().enumerate() {
            let b = Rect::from_min_size(
                Pos2::new(rect.right() - 8.0 - (2 - i) as f32 * (w + 6.0), y),
                Vec2::new(w, h),
            );
            if ui
                .put(b, egui::Button::new(RichText::new(label).size(fs(15.0))))
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
            painter.circle_filled(tip, fs(8.0), col);
            painter.text(
                tip,
                egui::Align2::CENTER_CENTER,
                name,
                egui::FontId::proportional(fs(9.0)),
                Color32::BLACK,
            );
        }
    }
}
