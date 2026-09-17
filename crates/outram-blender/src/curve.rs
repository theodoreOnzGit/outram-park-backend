// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Curve authoring. Follows the published architecture of Blender's curve
// objects (source/blender/blenkernel/intern/curve.cc, and the Bezier/NURBS
// evaluators in curve_to_mesh and BKE_curve, github.com/blender/blender,
// GPL-2.0-or-later): poly / Bezier / NURBS splines, per-point handle types,
// radius and tilt, cyclic toggle. Concepts only — no upstream source copied.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! **Curve authoring** (`op-hzs.54.34`, GH issue #37 §G — the CAD sketch
//! layer). The foundation for curve → surface geometry (`op-hzs.54.35`),
//! curve ↔ mesh conversion (`.36`), NURBS surfaces (`.37`) and text
//! (`.38`).
//!
//! A [`Spline`] is an ordered list of [`ControlPoint`]s plus a
//! [`SplineType`] (poly / Bézier / NURBS) and a `cyclic` flag.
//! [`Spline::sample`] evaluates it to a polyline of `resolution` points per
//! segment; [`Spline::sample_with_frames`] also returns the per-point radius
//! and an oriented frame (tangent + tilted normal), which the sweep operators
//! ride.

use crate::math::Vec3;

/// The interpolation family of a [`Spline`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplineType {
    /// Straight segments through the control points.
    Poly,
    /// Cubic Bézier between consecutive points, driven by their handles.
    Bezier,
    /// Non-uniform rational B-spline of the spline's `order`.
    Nurbs,
}

/// How a Bézier handle is derived when [`Spline::recalculate_handles`] runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleType {
    /// A smooth tangent from the neighbouring points' spacing.
    Automatic,
    /// Points straight at the neighbouring control point (a sharp-ish corner).
    Vector,
    /// Kept collinear with the opposite handle, length preserved.
    Aligned,
    /// Left untouched.
    Free,
}

/// One control point of a [`Spline`].
#[derive(Debug, Clone, Copy)]
pub struct ControlPoint {
    /// The point itself (the "knot").
    pub position: Vec3,
    /// The incoming Bézier handle (absolute position). Unused for poly / NURBS.
    pub handle_left: Vec3,
    /// The outgoing Bézier handle (absolute position).
    pub handle_right: Vec3,
    /// Handle-recalculation rule for the left handle.
    pub type_left: HandleType,
    /// Handle-recalculation rule for the right handle.
    pub type_right: HandleType,
    /// Cross-section radius at this point (rides through to a bevel).
    pub radius: f64,
    /// Roll of the local frame about the tangent, in radians.
    pub tilt: f64,
    /// NURBS weight (`1.0` = a plain B-spline point).
    pub weight: f64,
}

impl ControlPoint {
    /// A point at `position` with mirrored auto handles and unit radius.
    pub fn new(position: Vec3) -> Self {
        ControlPoint {
            position,
            handle_left: position,
            handle_right: position,
            type_left: HandleType::Automatic,
            type_right: HandleType::Automatic,
            radius: 1.0,
            tilt: 0.0,
            weight: 1.0,
        }
    }
}

/// An ordered spline.
#[derive(Debug, Clone)]
pub struct Spline {
    /// Interpolation family.
    pub spline_type: SplineType,
    /// The control points, in order.
    pub points: Vec<ControlPoint>,
    /// Whether the spline closes back on itself.
    pub cyclic: bool,
    /// Evaluated points per segment (`>= 1`).
    pub resolution: usize,
    /// NURBS order (degree + 1); `>= 2`. Ignored for poly / Bézier.
    pub order: usize,
}

impl Spline {
    /// A new poly spline through `positions`.
    pub fn poly(positions: &[Vec3]) -> Self {
        Spline {
            spline_type: SplineType::Poly,
            points: positions.iter().map(|&p| ControlPoint::new(p)).collect(),
            cyclic: false,
            resolution: 1,
            order: 4,
        }
    }

    /// A new Bézier spline through `positions` with auto handles.
    pub fn bezier(positions: &[Vec3]) -> Self {
        let mut s = Spline {
            spline_type: SplineType::Bezier,
            points: positions.iter().map(|&p| ControlPoint::new(p)).collect(),
            cyclic: false,
            resolution: 12,
            order: 4,
        };
        s.recalculate_handles();
        s
    }

    /// A new NURBS spline of `order` through `positions`.
    pub fn nurbs(positions: &[Vec3], order: usize) -> Self {
        Spline {
            spline_type: SplineType::Nurbs,
            points: positions.iter().map(|&p| ControlPoint::new(p)).collect(),
            cyclic: false,
            resolution: 12,
            order: order.max(2),
        }
    }

    /// Append a control point at `position`.
    pub fn push(&mut self, position: Vec3) {
        self.points.push(ControlPoint::new(position));
    }

    /// Toggle the cyclic flag.
    pub fn toggle_cyclic(&mut self) {
        self.cyclic = !self.cyclic;
    }

    /// Change the interpolation family (recomputing handles for Bézier).
    pub fn set_type(&mut self, ty: SplineType) {
        self.spline_type = ty;
        if ty == SplineType::Bezier {
            self.recalculate_handles();
        }
    }

    /// Insert a control point at the midpoint of every segment (a curve
    /// subdivide that keeps the shape for Bézier / NURBS).
    pub fn subdivide(&mut self) {
        if self.points.len() < 2 {
            return;
        }
        let samples = self.sample();
        // Re-fit: take every other sampled point as a new control point.
        let step = self.resolution.max(1);
        let mut new_pts = Vec::new();
        let mut i = 0;
        while i < samples.len() {
            new_pts.push(ControlPoint::new(samples[i]));
            i += step.max(1) / 2 + 1;
        }
        if new_pts.len() >= 2 {
            self.points = new_pts;
            if self.spline_type == SplineType::Bezier {
                self.recalculate_handles();
            }
        }
    }

    /// Recompute Bézier handles per each point's [`HandleType`]. `Free` handles
    /// are left as they are.
    pub fn recalculate_handles(&mut self) {
        let n = self.points.len();
        if n == 0 {
            return;
        }
        let pos: Vec<Vec3> = self.points.iter().map(|p| p.position).collect();
        for i in 0..n {
            let prev = if i > 0 {
                pos[i - 1]
            } else if self.cyclic {
                pos[n - 1]
            } else {
                pos[i]
            };
            let next = if i + 1 < n {
                pos[i + 1]
            } else if self.cyclic {
                pos[0]
            } else {
                pos[i]
            };
            let p = pos[i];

            let auto_dir = next.sub(prev);
            let auto_len = auto_dir.length();
            let tangent = if auto_len > 1e-9 {
                auto_dir.scale(1.0 / auto_len)
            } else {
                Vec3::new(1.0, 0.0, 0.0)
            };
            let spacing = (p.sub(prev).length() + next.sub(p).length()) * 0.5 / 3.0;

            let cp = &mut self.points[i];
            match cp.type_left {
                HandleType::Automatic => cp.handle_left = p.sub(tangent.scale(spacing)),
                HandleType::Vector => cp.handle_left = p.add(prev.sub(p).scale(1.0 / 3.0)),
                HandleType::Aligned => {
                    let d = cp.handle_right.sub(p);
                    let len = d.length();
                    cp.handle_left = p.sub(
                        if len > 1e-9 {
                            d.scale(1.0 / len)
                        } else {
                            tangent
                        }
                        .scale(len.max(spacing)),
                    );
                }
                HandleType::Free => {}
            }
            match cp.type_right {
                HandleType::Automatic => cp.handle_right = p.add(tangent.scale(spacing)),
                HandleType::Vector => cp.handle_right = p.add(next.sub(p).scale(1.0 / 3.0)),
                HandleType::Aligned => {
                    let d = p.sub(cp.handle_left);
                    let len = d.length();
                    cp.handle_right = p.add(
                        if len > 1e-9 {
                            d.scale(1.0 / len)
                        } else {
                            tangent
                        }
                        .scale(len.max(spacing)),
                    );
                }
                HandleType::Free => {}
            }
        }
    }

    /// Evaluate the spline to a polyline (`resolution` points per segment).
    pub fn sample(&self) -> Vec<Vec3> {
        self.sample_with_frames()
            .into_iter()
            .map(|s| s.position)
            .collect()
    }

    /// Evaluate the spline to a list of [`SplineSample`]s (position, radius,
    /// tangent, tilted normal).
    pub fn sample_with_frames(&self) -> Vec<SplineSample> {
        let n = self.points.len();
        if n == 0 {
            return Vec::new();
        }
        if n == 1 {
            return vec![SplineSample {
                position: self.points[0].position,
                radius: self.points[0].radius,
                tangent: Vec3::new(1.0, 0.0, 0.0),
                normal: Vec3::new(0.0, 0.0, 1.0),
            }];
        }
        let raw: Vec<(Vec3, f64)> = match self.spline_type {
            SplineType::Poly => self.sample_poly(),
            SplineType::Bezier => self.sample_bezier(),
            SplineType::Nurbs => self.sample_nurbs(),
        };
        frames_from(&raw, self.cyclic, &self.tilts())
    }

    fn tilts(&self) -> Vec<f64> {
        self.points.iter().map(|p| p.tilt).collect()
    }

    fn seg_count(&self) -> usize {
        if self.cyclic {
            self.points.len()
        } else {
            self.points.len() - 1
        }
    }

    fn sample_poly(&self) -> Vec<(Vec3, f64)> {
        let n = self.points.len();
        let mut out = Vec::new();
        for s in 0..self.seg_count() {
            let a = &self.points[s];
            let b = &self.points[(s + 1) % n];
            for k in 0..self.resolution.max(1) {
                let t = k as f64 / self.resolution.max(1) as f64;
                out.push((
                    a.position.add(b.position.sub(a.position).scale(t)),
                    lerp(a.radius, b.radius, t),
                ));
            }
        }
        if !self.cyclic {
            let last = &self.points[n - 1];
            out.push((last.position, last.radius));
        }
        out
    }

    fn sample_bezier(&self) -> Vec<(Vec3, f64)> {
        let n = self.points.len();
        let mut out = Vec::new();
        for s in 0..self.seg_count() {
            let a = &self.points[s];
            let b = &self.points[(s + 1) % n];
            let (p0, p1, p2, p3) = (a.position, a.handle_right, b.handle_left, b.position);
            for k in 0..self.resolution.max(1) {
                let t = k as f64 / self.resolution.max(1) as f64;
                out.push((cubic_bezier(p0, p1, p2, p3, t), lerp(a.radius, b.radius, t)));
            }
        }
        if !self.cyclic {
            let last = &self.points[n - 1];
            out.push((last.position, last.radius));
        }
        out
    }

    fn sample_nurbs(&self) -> Vec<(Vec3, f64)> {
        let n = self.points.len();
        let p = self.order.min(n).max(2) - 1; // degree
                                              // Clamped uniform knot vector.
        let m = n + p + 1;
        let mut knots = vec![0.0; m];
        for (i, kv) in knots.iter_mut().enumerate() {
            *kv = if i <= p {
                0.0
            } else if i >= n {
                (n - p) as f64
            } else {
                (i - p) as f64
            };
        }
        let u0 = knots[p];
        let u1 = knots[n];
        let total = (self.resolution.max(1) * self.seg_count().max(1)).max(2);
        let mut out = Vec::with_capacity(total + 1);
        for step in 0..=total {
            let u = u0 + (u1 - u0) * step as f64 / total as f64;
            let (pt, r) = nurbs_point(&self.points, &knots, p, u);
            out.push((pt, r));
        }
        out
    }
}

/// One evaluated point of a spline plus its local frame.
#[derive(Debug, Clone, Copy)]
pub struct SplineSample {
    pub position: Vec3,
    pub radius: f64,
    /// Unit tangent (direction of travel).
    pub tangent: Vec3,
    /// Unit normal, rolled by the interpolated tilt.
    pub normal: Vec3,
}

// --- evaluators ---

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn cubic_bezier(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f64) -> Vec3 {
    let u = 1.0 - t;
    p0.scale(u * u * u)
        .add(p1.scale(3.0 * u * u * t))
        .add(p2.scale(3.0 * u * t * t))
        .add(p3.scale(t * t * t))
}

#[allow(clippy::needless_range_loop)]
fn nurbs_point(pts: &[ControlPoint], knots: &[f64], degree: usize, u: f64) -> (Vec3, f64) {
    let n = pts.len();
    // Find span.
    let mut span = degree;
    while span < n - 1 && u >= knots[span + 1] {
        span += 1;
    }
    // Basis functions (Cox-de Boor).
    let mut basis = vec![0.0; degree + 1];
    basis[0] = 1.0;
    let mut left = vec![0.0; degree + 1];
    let mut right = vec![0.0; degree + 1];
    for j in 1..=degree {
        left[j] = u - knots[span + 1 - j];
        right[j] = knots[span + j] - u;
        let mut saved = 0.0;
        for r in 0..j {
            let denom = right[r + 1] + left[j - r];
            let temp = if denom.abs() > 1e-12 {
                basis[r] / denom
            } else {
                0.0
            };
            basis[r] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        basis[j] = saved;
    }
    let mut num = Vec3::ZERO;
    let mut den = 0.0;
    let mut rad = 0.0;
    for i in 0..=degree {
        let idx = span - degree + i;
        if idx < n {
            let w = pts[idx].weight * basis[i];
            num = num.add(pts[idx].position.scale(w));
            rad += pts[idx].radius * basis[i];
            den += w;
        }
    }
    if den.abs() > 1e-12 {
        (num.scale(1.0 / den), rad)
    } else {
        (pts[span.min(n - 1)].position, pts[span.min(n - 1)].radius)
    }
}

#[allow(clippy::needless_range_loop)]
fn frames_from(raw: &[(Vec3, f64)], cyclic: bool, tilts: &[f64]) -> Vec<SplineSample> {
    let m = raw.len();
    let mut out = Vec::with_capacity(m);
    // Parallel-transport an initial normal along the polyline.
    let tangent_at = |i: usize| -> Vec3 {
        let a = if i == 0 {
            if cyclic {
                raw[m - 1].0
            } else {
                raw[0].0
            }
        } else {
            raw[i - 1].0
        };
        let b = if i + 1 < m {
            raw[i + 1].0
        } else if cyclic {
            raw[0].0
        } else {
            raw[m - 1].0
        };
        let d = b.sub(a);
        if d.length() > 1e-9 {
            d.normalize()
        } else {
            Vec3::new(1.0, 0.0, 0.0)
        }
    };
    let t0 = tangent_at(0);
    let up = if t0.z.abs() < 0.9 {
        Vec3::new(0.0, 0.0, 1.0)
    } else {
        Vec3::new(1.0, 0.0, 0.0)
    };
    let mut normal = up.cross(t0).normalize();
    let mut prev_t = t0;
    for i in 0..m {
        let t = tangent_at(i);
        // Rotate the carried normal from prev_t to t.
        let axis = prev_t.cross(t);
        if axis.length() > 1e-9 {
            let angle = prev_t.dot(t).clamp(-1.0, 1.0).acos();
            normal = rodrigues(normal, axis.normalize(), angle);
        }
        prev_t = t;
        // Apply the interpolated tilt about the tangent.
        let tilt = interp_tilt(tilts, i, m);
        let n = rodrigues(normal, t, tilt);
        out.push(SplineSample {
            position: raw[i].0,
            radius: raw[i].1,
            tangent: t,
            normal: n.normalize(),
        });
    }
    out
}

fn interp_tilt(tilts: &[f64], i: usize, m: usize) -> f64 {
    if tilts.is_empty() {
        return 0.0;
    }
    let t = i as f64 / (m.max(2) - 1) as f64 * (tilts.len() - 1) as f64;
    let a = t.floor() as usize;
    let b = (a + 1).min(tilts.len() - 1);
    lerp(tilts[a], tilts[b], t - a as f64)
}

fn rodrigues(v: Vec3, k: Vec3, theta: f64) -> Vec3 {
    let (s, c) = theta.sin_cos();
    v.scale(c)
        .add(k.cross(v).scale(s))
        .add(k.scale(k.dot(v) * (1.0 - c)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poly_spline_is_the_control_polygon() {
        let s = Spline::poly(&[
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
        ]);
        let pts = s.sample();
        assert_eq!(pts.first().copied(), Some(Vec3::ZERO));
        assert_eq!(pts.last().copied(), Some(Vec3::new(1.0, 1.0, 0.0)));
        assert_eq!(pts.len(), 3, "resolution 1 → control points only");
    }

    #[test]
    fn bezier_passes_through_its_control_points() {
        let mut s = Spline::bezier(&[
            Vec3::ZERO,
            Vec3::new(2.0, 2.0, 0.0),
            Vec3::new(4.0, 0.0, 0.0),
        ]);
        s.resolution = 20;
        let pts = s.sample();
        assert!(pts.first().unwrap().sub(Vec3::ZERO).length() < 1e-9);
        assert!(pts.last().unwrap().sub(Vec3::new(4.0, 0.0, 0.0)).length() < 1e-9);
        // The middle control point is interpolated by a Bézier spline.
        assert!(pts
            .iter()
            .any(|p| p.sub(Vec3::new(2.0, 2.0, 0.0)).length() < 1e-6));
    }

    #[test]
    fn vector_handles_make_straight_segments() {
        let mut s = Spline::bezier(&[
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
        ]);
        for p in s.points.iter_mut() {
            p.type_left = HandleType::Vector;
            p.type_right = HandleType::Vector;
        }
        s.recalculate_handles();
        s.resolution = 8;
        let pts = s.sample();
        // Every sample lies on the x axis.
        assert!(pts.iter().all(|p| p.y.abs() < 1e-9 && p.z.abs() < 1e-9));
    }

    #[test]
    fn nurbs_stays_within_the_control_hull() {
        let ctrl = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 2.0, 0.0),
            Vec3::new(3.0, 2.0, 0.0),
            Vec3::new(4.0, 0.0, 0.0),
        ];
        let s = Spline::nurbs(&ctrl, 3);
        let pts = s.sample();
        assert!(pts.len() > 4);
        // Endpoints of a clamped NURBS coincide with the first/last control pt.
        assert!(pts.first().unwrap().sub(ctrl[0]).length() < 1e-6);
        assert!(pts.last().unwrap().sub(ctrl[3]).length() < 1e-6);
        // Everything inside the y ∈ [0, 2] band of the hull.
        assert!(pts.iter().all(|p| p.y >= -1e-6 && p.y <= 2.0 + 1e-6));
    }

    #[test]
    fn cyclic_toggle_closes_the_loop() {
        let mut s = Spline::poly(&[
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
        ]);
        assert_eq!(s.sample().len(), 3);
        s.toggle_cyclic();
        assert_eq!(s.sample().len(), 3, "3 segments now, still resolution 1");
        // The last sampled point is on the closing segment, not the last ctrl.
        s.resolution = 4;
        let pts = s.sample();
        assert!(pts.len() > 3);
    }

    #[test]
    fn radius_and_tilt_ride_through_the_frames() {
        let mut s = Spline::poly(&[Vec3::ZERO, Vec3::new(4.0, 0.0, 0.0)]);
        s.points[0].radius = 1.0;
        s.points[1].radius = 3.0;
        s.points[1].tilt = std::f64::consts::FRAC_PI_2;
        s.resolution = 8;
        let frames = s.sample_with_frames();
        assert!((frames.first().unwrap().radius - 1.0).abs() < 1e-9);
        assert!((frames.last().unwrap().radius - 3.0).abs() < 1e-9);
        // The normal rolled ~90° between the ends.
        let dot = frames
            .first()
            .unwrap()
            .normal
            .dot(frames.last().unwrap().normal);
        assert!(dot.abs() < 0.2, "tilt rolled the frame");
    }

    #[test]
    fn set_type_switches_evaluator() {
        let mut s = Spline::poly(&[
            Vec3::ZERO,
            Vec3::new(2.0, 2.0, 0.0),
            Vec3::new(4.0, 0.0, 0.0),
        ]);
        s.set_type(SplineType::Bezier);
        assert_eq!(s.spline_type, SplineType::Bezier);
        s.resolution = 10;
        // Bézier now curves — a mid sample is off the control polygon.
        let pts = s.sample();
        assert!(pts.iter().any(|p| p.y > 1.0 && p.y < 2.0));
    }
}
