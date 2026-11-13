//! The 3-D viewport's camera and the mapping from mesh sections to the
//! screen. A Blender-style orbit camera: drag to orbit about the target,
//! secondary-drag (or the Pan toggle, or two fingers) to pan, wheel or pinch
//! to zoom, with the + / − / Reset buttons always on the view
//! (mobile-first rule). Pure geometry, tested; the painting is in `app.rs`.
use crate::engine::Section;
use egui::{Pos2, Rect, Vec2};

/// World up for the camera: +y, so a pipe along z lies across the screen.
const UP: [f64; 3] = [0.0, 1.0, 0.0];

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(a: [f64; 3]) -> [f64; 3] {
    let m = dot(a, a).sqrt().max(1e-300);
    [a[0] / m, a[1] / m, a[2] / m]
}

/// An orbit camera with perspective projection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orbit {
    /// Point orbited about \[m\].
    pub target: [f64; 3],
    /// Azimuth about +y and elevation, radians.
    pub yaw: f64,
    pub pitch: f64,
    /// Eye-to-target distance \[m\].
    pub dist: f64,
    /// Radius of the subject \[m\]: what Reset fits.
    pub radius: f64,
    /// Where Reset returns to.
    pub home: [f64; 3],
    pub fitted: bool,
}

/// Vertical field of view, radians (about 40°).
pub const FOV: f64 = 0.7;

impl Orbit {
    /// A camera on a subject of `radius` centred at `home`, three-quarter view.
    pub fn new(home: [f64; 3], radius: f64) -> Self {
        let mut o = Self { target: home, yaw: 0.9, pitch: 0.45, dist: 1.0, radius: radius.max(1e-6), home, fitted: false };
        o.reset();
        o
    }

    /// Reset: three-quarter view, the subject filling the view.
    pub fn reset(&mut self) {
        self.target = self.home;
        self.yaw = 0.9;
        self.pitch = 0.45;
        self.dist = 1.25 * self.radius / (0.5 * FOV).tan();
        self.fitted = true;
    }

    /// The eye position \[m\].
    pub fn eye(&self) -> [f64; 3] {
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        let (cy, sy) = (self.yaw.cos(), self.yaw.sin());
        [self.target[0] + self.dist * cp * sy, self.target[1] + self.dist * sp, self.target[2] + self.dist * cp * cy]
    }

    /// The camera basis: right, up, forward (unit vectors).
    pub fn basis(&self) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let f = norm(sub(self.target, self.eye()));
        let r = norm(cross(f, UP));
        let u = cross(r, f);
        (r, u, f)
    }

    /// Pixels per unit of tan(angle): the focal length on `rect`.
    pub fn focal(rect: Rect) -> f64 {
        0.5 * rect.height().min(rect.width()) as f64 / (0.5 * FOV).tan()
    }

    /// Project `p` to the screen; `None` behind the eye.
    pub fn project(&self, rect: Rect, p: [f64; 3]) -> Option<Pos2> {
        let (r, u, f) = self.basis();
        let v = sub(p, self.eye());
        let z = dot(v, f);
        if z <= 1e-3 * self.dist {
            return None;
        }
        let k = Self::focal(rect) / z;
        let c = rect.center();
        Some(Pos2::new(c.x + (dot(v, r) * k) as f32, c.y - (dot(v, u) * k) as f32))
    }

    /// Orbit by a drag of `d` pixels.
    pub fn orbit(&mut self, d: Vec2) {
        self.yaw -= f64::from(d.x) * 0.008;
        self.pitch = (self.pitch + f64::from(d.y) * 0.008).clamp(-1.5, 1.5);
    }

    /// Pan by a drag of `d` pixels: the target moves so the picture follows
    /// the finger.
    pub fn pan(&mut self, rect: Rect, d: Vec2) {
        let (r, u, _) = self.basis();
        let s = self.dist / Self::focal(rect);
        for i in 0..3 {
            self.target[i] += (-f64::from(d.x) * r[i] + f64::from(d.y) * u[i]) * s;
        }
    }

    /// Zoom by `factor` (> 1 is closer), clamped to 1/50 .. 20 of the fit.
    pub fn zoom(&mut self, factor: f64) {
        let fit = 1.25 * self.radius / (0.5 * FOV).tan();
        self.dist = (self.dist / factor.max(1e-6)).clamp(fit / 50.0, fit * 20.0);
    }

    /// The gestures: drag orbits (pans with `pan_mode`, the secondary
    /// button, or shift), two fingers pan and pinch, the wheel zooms, a
    /// double click resets. They supplement the buttons, never replace them.
    /// GUI input glue (its effects are the tested `orbit`, `pan`, `zoom`).
    pub fn handle_input(&mut self, ui: &egui::Ui, resp: &egui::Response, pan_mode: bool) {
        if !self.fitted || resp.double_clicked() {
            self.reset();
        }
        let rect = resp.rect;
        let (scroll, zoom, touch, shift) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta(), i.multi_touch(), i.modifiers.shift));
        if resp.hovered() || touch.is_some() {
            let f = f64::from(zoom) * (f64::from(scroll) * 0.0025).exp();
            if (f - 1.0).abs() > 1e-6 {
                self.zoom(f);
            }
        }
        if let Some(t) = touch {
            self.pan(rect, t.translation_delta);
        } else if resp.dragged() {
            let d = resp.drag_delta();
            if pan_mode || shift || resp.dragged_by(egui::PointerButton::Secondary) || resp.dragged_by(egui::PointerButton::Middle) && shift {
                self.pan(rect, d);
            } else {
                self.orbit(d);
            }
        }
    }
}

/// A point of a section's plane (axis 0/1/2, offset \[m\]) in 3-D, from its
/// in-plane coordinates (the convention of `outram_foam_mesh::section::Axis`).
pub fn plane_to_3d(axis: u8, offset: f64, a: f64, b: f64) -> [f64; 3] {
    match axis {
        0 => [offset, a, b],
        1 => [a, offset, b],
        _ => [a, b, offset],
    }
}

/// A point of a section in 3-D from its in-plane coordinates: through the
/// section's frame when it has one (the coil's cross-section), else
/// [`plane_to_3d`].
pub fn section_point(sec: &Section, a: f64, b: f64) -> [f64; 3] {
    match &sec.frame {
        Some(f) => [0, 1, 2].map(|i| f[i] + a * f[3 + i] + b * f[6 + i]),
        None => plane_to_3d(sec.axis, sec.offset, a, b),
    }
}

/// Every polygon of a section projected to the screen; a polygon with a
/// vertex behind the eye is dropped (an empty entry keeps the cell order).
pub fn project_section(cam: &Orbit, rect: Rect, sec: &Section) -> Vec<Vec<Pos2>> {
    sec.polygons
        .iter()
        .map(|p| {
            p.iter()
                .map(|q| cam.project(rect, section_point(sec, f64::from(q[0]), f64::from(q[1]))))
                .collect::<Option<Vec<Pos2>>>()
                .unwrap_or_default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(390.0, 600.0))
    }

    #[test]
    fn the_target_projects_to_the_centre_and_reset_fits() {
        let mut c = Orbit::new([0.0, 0.0, 0.1], 0.1);
        assert_eq!(c.project(rect(), c.target).unwrap(), rect().center());
        // The subject's bounding sphere fits: a point one radius to the right
        // of the target lands inside the view.
        let (r, _, _) = c.basis();
        let p = c.project(rect(), [r[0] * 0.1, r[1] * 0.1, 0.1 + r[2] * 0.1]).unwrap();
        assert!(rect().contains(p), "{p:?}");
        let d0 = c.dist;
        c.orbit(Vec2::new(100.0, 30.0));
        let e = c.eye();
        let dd = sub(e, c.target);
        assert!((dot(dd, dd).sqrt() - d0).abs() < 1e-12, "orbit keeps the distance");
        c.zoom(2.0);
        assert!((c.dist - 0.5 * d0).abs() < 1e-12);
        c.zoom(1e9);
        assert!(c.dist > 0.0);
        c.reset();
        assert_eq!(c.dist, d0);
        // Behind the eye: none.
        let e = c.eye();
        let back = [2.0 * e[0] - c.target[0], 2.0 * e[1] - c.target[1], 2.0 * e[2] - c.target[2]];
        assert!(c.project(rect(), back).is_none());
    }

    #[test]
    fn pan_moves_the_picture_with_the_finger() {
        let mut c = Orbit::new([0.0, 0.0, 0.0], 0.05);
        let p = [0.0, 0.0, 0.0];
        let before = c.project(rect(), p).unwrap();
        c.pan(rect(), Vec2::new(30.0, -10.0));
        let after = c.project(rect(), p).unwrap();
        assert!((after.x - before.x - 30.0).abs() < 0.5 && (after.y - before.y + 10.0).abs() < 0.5, "{before:?} -> {after:?}");
    }

    #[test]
    fn sections_map_back_to_their_plane() {
        assert_eq!(plane_to_3d(0, 1.0, 2.0, 3.0), [1.0, 2.0, 3.0]);
        assert_eq!(plane_to_3d(1, 1.0, 2.0, 3.0), [2.0, 1.0, 3.0]);
        assert_eq!(plane_to_3d(2, 1.0, 2.0, 3.0), [2.0, 3.0, 1.0]);
        let c = Orbit::new([0.0; 3], 0.05);
        let polys = vec![vec![[0.0, 0.0], [0.01, 0.0], [0.0, 0.01]]];
        let mut sec = Section { axis: 2, offset: 0.0, polygons: polys, frame: None };
        let s = project_section(&c, rect(), &sec);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].len(), 3);
        assert_eq!(s[0][0], rect().center());
        // A framed section: origin + a e1 + b e2.
        sec.frame = Some([1.0, 2.0, 3.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0]);
        assert_eq!(section_point(&sec, 0.5, 0.25), [1.25, 2.0, 3.5]);
    }
}
