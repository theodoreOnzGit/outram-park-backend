// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Precision Drawing Tools (PDT). Blender analogue (architecture only): the
// bundled `precision_drawing_tools` add-on — absolute/delta/polar/percent
// placement, 3-point arc & circle, line-line intersection, fillet, offset,
// taper, angle dimension, mirror across a working plane. No upstream source
// copied; the geometry is standard analytic construction.
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

//! **Precision Drawing Tools** (`op-hzs.54.44`, GH issue #37 §I) — the analytic
//! CAD constructions from Blender's PDT add-on, as pure functions on points
//! plus a few [`Mesh`] wrappers.
//!
//! Placement (compute one point):
//! - [`Placement::Absolute`] / [`Placement::Delta`] / [`Placement::Polar`] /
//!   [`Placement::Percent`] → [`Placement::resolve`].
//!
//! Constructions:
//! - [`three_point_circle`] — circumcircle (centre, radius, normal).
//! - [`three_point_arc`] — polyline along the arc `p0 → p1 → p2`.
//! - [`line_line_intersection`] — closest-approach point of two 3-D lines.
//! - [`fillet`] — tangent arc rounding a polyline corner.
//! - [`offset_polyline`] — parallel offset in a plane.
//! - [`taper`] — linear cross-section scaling along an axis.
//! - [`angle_between`] — the angle `∠(a, vertex, b)`.
//! - [`mirror_point`] / [`mirror_vertices`] — reflection across a
//!   [`crate::draw_tool::WorkPlane`].
//!
//! ## Units
//!
//! Positions/lengths are dimensionless model-space quantities; angles radians;
//! `Percent` is a literal percentage (`50.0` = halfway).

use crate::draw_tool::WorkPlane;
use crate::math::Vec3;
use crate::mesh::{Mesh, VertexId};

/// How to compute a single target point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    /// The point is exactly `coord`.
    Absolute { coord: Vec3 },
    /// `from + delta`.
    Delta { from: Vec3, delta: Vec3 },
    /// `from`, stepped `distance` along `angle` (radians) measured in `plane`
    /// from `plane.u`, CCW about `plane.normal`.
    Polar { from: Vec3, plane: WorkPlane, distance: f64, angle: f64 },
    /// `a + (b - a) * percent/100`.
    Percent { a: Vec3, b: Vec3, percent: f64 },
}

impl Placement {
    /// Resolve to the world point.
    pub fn resolve(&self) -> Vec3 {
        match *self {
            Placement::Absolute { coord } => coord,
            Placement::Delta { from, delta } => from.add(delta),
            Placement::Polar { from, plane, distance, angle } => {
                let dir = plane.u.scale(angle.cos()).add(plane.v.scale(angle.sin()));
                from.add(dir.scale(distance))
            }
            Placement::Percent { a, b, percent } => a.add(b.sub(a).scale(percent / 100.0)),
        }
    }
}

/// Add a vertex at `placement`'s point to `mesh`, returning the new mesh and
/// the id.
pub fn place_vertex(mesh: &Mesh, placement: &Placement) -> (Mesh, VertexId) {
    let mut m = mesh.clone();
    let id = m.add_vertex(placement.resolve());
    (m, id)
}

/// The circumcircle of three points: `(centre, radius, unit normal)`, or
/// `None` if the points are collinear.
pub fn three_point_circle(p0: Vec3, p1: Vec3, p2: Vec3) -> Option<(Vec3, f64, Vec3)> {
    let a = p1.sub(p0);
    let b = p2.sub(p0);
    let n = a.cross(b);
    let n2 = n.dot(n);
    if n2 < 1e-18 {
        return None;
    }
    // Circumcentre relative to p0 (Wikipedia "Circumscribed circle",
    // Cartesian coordinates in 3-D):
    //   m = ((|a|² b − |b|² a) × (a × b)) / (2 |a × b|²)
    let centre_rel = b
        .scale(a.dot(a))
        .sub(a.scale(b.dot(b)))
        .cross(n)
        .scale(1.0 / (2.0 * n2));
    let centre = p0.add(centre_rel);
    Some((centre, centre_rel.length(), n.normalize()))
}

/// A polyline of `segments + 1` points along the circular arc that starts at
/// `p0`, passes through `p1`, and ends at `p2`. Falls back to the straight
/// chords `p0, p1, p2` if the points are collinear. `segments` clamped `>= 2`.
pub fn three_point_arc(p0: Vec3, p1: Vec3, p2: Vec3, segments: usize) -> Vec<Vec3> {
    let seg = segments.max(2);
    let Some((c, r, n)) = three_point_circle(p0, p1, p2) else {
        return vec![p0, p1, p2];
    };
    // In-plane basis with `u` toward p0, so p0 sits at angle 0.
    let u = p0.sub(c).normalize();
    let v = n.cross(u);
    let ang = |p: Vec3| {
        let d = p.sub(c);
        d.dot(v).atan2(d.dot(u))
    };
    // Sweep from 0 in whichever direction keeps p1 on the arc to p2.
    let a1 = ang(p1).rem_euclid(std::f64::consts::TAU);
    let a2 = ang(p2).rem_euclid(std::f64::consts::TAU);
    let end = if a1 < a2 { a2 } else { a2 - std::f64::consts::TAU };
    (0..=seg)
        .map(|i| {
            let t = end * i as f64 / seg as f64;
            c.add(u.scale(r * t.cos())).add(v.scale(r * t.sin()))
        })
        .collect()
}

/// The point of closest approach of line `A` (through `a0`, `a1`) and line
/// `B` (through `b0`, `b1`): the midpoint of the shortest connecting segment.
/// `None` if the lines are parallel.
pub fn line_line_intersection(a0: Vec3, a1: Vec3, b0: Vec3, b1: Vec3) -> Option<Vec3> {
    let d1 = a1.sub(a0);
    let d2 = b1.sub(b0);
    let r = a0.sub(b0);
    let a = d1.dot(d1);
    let e = d2.dot(d2);
    let f = d2.dot(r);
    let c = d1.dot(r);
    let b = d1.dot(d2);
    let denom = a * e - b * b;
    if denom.abs() < 1e-12 {
        return None;
    }
    let s = (b * f - c * e) / denom;
    let t = (a * f - b * c) / denom;
    let pa = a0.add(d1.scale(s));
    let pb = b0.add(d2.scale(t));
    Some(pa.add(pb).scale(0.5))
}

/// A tangent fillet arc rounding the corner at `corner` between legs to
/// `prev` and `next`, with the given `radius`. Returns `segments + 1` points
/// from the tangent point on the `prev` leg to the one on the `next` leg
/// (empty if the legs are degenerate or the radius does not fit).
pub fn fillet(prev: Vec3, corner: Vec3, next: Vec3, radius: f64, segments: usize) -> Vec<Vec3> {
    let seg = segments.max(1);
    let d0 = prev.sub(corner);
    let d1 = next.sub(corner);
    let (l0, l1) = (d0.length(), d1.length());
    if l0 < 1e-9 || l1 < 1e-9 || radius <= 0.0 {
        return Vec::new();
    }
    let u0 = d0.scale(1.0 / l0);
    let u1 = d1.scale(1.0 / l1);
    let cos_full = u0.dot(u1).clamp(-1.0, 1.0);
    let half = (cos_full.acos()) * 0.5;
    if half <= 1e-6 || half >= std::f64::consts::FRAC_PI_2 - 1e-6 {
        return Vec::new();
    }
    // Distance from corner to each tangent point.
    let tan_dist = radius / half.tan();
    if tan_dist > l0 || tan_dist > l1 {
        return Vec::new();
    }
    let t0 = corner.add(u0.scale(tan_dist));
    let t1 = corner.add(u1.scale(tan_dist));
    // Arc centre: along the internal bisector, at radius / sin(half).
    let bis = u0.add(u1).normalize();
    let centre = corner.add(bis.scale(radius / half.sin()));
    let n = u0.cross(u1);
    if n.length() < 1e-12 {
        return vec![t0, t1];
    }
    let n = n.normalize();
    let r0 = t0.sub(centre);
    let r1 = t1.sub(centre);
    let sweep = {
        let x = r0.dot(r1).clamp(-r0.length() * r1.length(), r0.length() * r1.length());
        (x / (r0.length() * r1.length())).clamp(-1.0, 1.0).acos()
    };
    let axis = if r0.cross(r1).dot(n) >= 0.0 { n } else { n.scale(-1.0) };
    (0..=seg)
        .map(|i| {
            let a = sweep * i as f64 / seg as f64;
            rotate_about(r0, axis, a).add(centre)
        })
        .collect()
}

fn rotate_about(v: Vec3, axis: Vec3, angle: f64) -> Vec3 {
    let (s, c) = angle.sin_cos();
    v.scale(c)
        .add(axis.cross(v).scale(s))
        .add(axis.scale(axis.dot(v) * (1.0 - c)))
}

/// Parallel-offset a polyline by `distance` along the in-plane normal
/// (`plane_normal x segment_direction`), averaging the two adjacent segment
/// normals at each interior vertex. `cyclic` wraps the ends.
pub fn offset_polyline(pts: &[Vec3], plane_normal: Vec3, distance: f64, cyclic: bool) -> Vec<Vec3> {
    let n = pts.len();
    if n < 2 {
        return pts.to_vec();
    }
    let pn = plane_normal.normalize();
    let seg_normal = |i: usize| {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        pn.cross(b.sub(a)).normalize()
    };
    (0..n)
        .map(|i| {
            let nrm = if cyclic {
                seg_normal((i + n - 1) % n).add(seg_normal(i)).normalize()
            } else if i == 0 {
                seg_normal(0)
            } else if i == n - 1 {
                seg_normal(n - 2)
            } else {
                seg_normal(i - 1).add(seg_normal(i)).normalize()
            };
            pts[i].add(nrm.scale(distance))
        })
        .collect()
}

/// Taper `points` along `axis` (unit): each point's component perpendicular to
/// `axis`, measured from `pivot`, is scaled by `1 + rate * along`, where
/// `along` is its signed distance from `pivot` along `axis`.
pub fn taper(points: &[Vec3], axis: Vec3, rate: f64, pivot: Vec3) -> Vec<Vec3> {
    let k = axis.normalize();
    points
        .iter()
        .map(|&p| {
            let rel = p.sub(pivot);
            let along = rel.dot(k);
            let perp = rel.sub(k.scale(along));
            pivot.add(k.scale(along)).add(perp.scale(1.0 + rate * along))
        })
        .collect()
}

/// The angle `∠(a, vertex, b)` in radians, in `[0, π]`.
pub fn angle_between(a: Vec3, vertex: Vec3, b: Vec3) -> f64 {
    let u = a.sub(vertex);
    let v = b.sub(vertex);
    let d = u.length() * v.length();
    if d < 1e-12 {
        return 0.0;
    }
    (u.dot(v) / d).clamp(-1.0, 1.0).acos()
}

/// Reflect a point across a [`WorkPlane`].
pub fn mirror_point(p: Vec3, plane: &WorkPlane) -> Vec3 {
    let dist = p.sub(plane.origin).dot(plane.normal);
    p.sub(plane.normal.scale(2.0 * dist))
}

/// Reflect the given `verts` of `mesh` across `plane`, returning a new mesh
/// (topology unchanged; other vertices untouched).
pub fn mirror_vertices(mesh: &Mesh, verts: &[VertexId], plane: &WorkPlane) -> Mesh {
    let mut positions = mesh.positions();
    for v in verts {
        positions[v.0] = mirror_point(positions[v.0], plane);
    }
    let faces: Vec<Vec<usize>> =
        mesh.polygons().iter().map(|f| f.iter().map(|x| x.0).collect()).collect();
    Mesh::from_polygons(&positions, &faces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, PI, TAU};

    #[test]
    fn placement_modes() {
        assert_eq!(Placement::Absolute { coord: Vec3::new(1.0, 2.0, 3.0) }.resolve(), Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(
            Placement::Delta { from: Vec3::new(1.0, 0.0, 0.0), delta: Vec3::new(0.0, 2.0, 0.0) }.resolve(),
            Vec3::new(1.0, 2.0, 0.0)
        );
        let p = Placement::Polar {
            from: Vec3::ZERO,
            plane: WorkPlane::xy(),
            distance: 2.0,
            angle: FRAC_PI_2,
        }
        .resolve();
        assert!(p.sub(Vec3::new(0.0, 2.0, 0.0)).length() < 1e-9);
        let q = Placement::Percent { a: Vec3::ZERO, b: Vec3::new(4.0, 0.0, 0.0), percent: 25.0 }.resolve();
        assert!(q.sub(Vec3::new(1.0, 0.0, 0.0)).length() < 1e-9);
    }

    #[test]
    fn three_point_circle_recovers_a_known_circle() {
        // Unit circle in the xy-plane.
        let (c, r, n) = three_point_circle(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(-1.0, 0.0, 0.0),
        )
        .unwrap();
        assert!(c.length() < 1e-9);
        assert!((r - 1.0).abs() < 1e-9);
        assert!((n.z.abs() - 1.0).abs() < 1e-9);
        assert!(three_point_circle(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0)).is_none());
    }

    #[test]
    fn three_point_arc_stays_on_the_circle_and_hits_the_ends() {
        let p0 = Vec3::new(1.0, 0.0, 0.0);
        let p1 = Vec3::new(0.0, 1.0, 0.0);
        let p2 = Vec3::new(-1.0, 0.0, 0.0);
        let arc = three_point_arc(p0, p1, p2, 16);
        assert!(arc.first().unwrap().sub(p0).length() < 1e-9);
        assert!(arc.last().unwrap().sub(p2).length() < 1e-9);
        for q in &arc {
            assert!((q.length() - 1.0).abs() < 1e-9, "on the unit circle");
            assert!(q.z.abs() < 1e-9);
        }
        // Passes near p1 (the through-point).
        assert!(arc.iter().any(|q| q.sub(p1).length() < 0.2));
    }

    #[test]
    fn line_line_intersection_of_crossing_lines() {
        let x = line_line_intersection(
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        )
        .unwrap();
        assert!(x.length() < 1e-9);
        assert!(line_line_intersection(
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0)
        )
        .is_none());
    }

    #[test]
    fn fillet_arc_is_tangent_and_on_a_radius_circle() {
        // Right-angle corner at origin, legs along +x and +y.
        let arc = fillet(
            Vec3::new(3.0, 0.0, 0.0),
            Vec3::ZERO,
            Vec3::new(0.0, 3.0, 0.0),
            1.0,
            12,
        );
        assert!(arc.len() == 13);
        // For a 90° corner, tangent points are at distance r from the corner
        // along each leg: (1,0,0) and (0,1,0); arc centre at (1,1,0).
        assert!(arc.first().unwrap().sub(Vec3::new(1.0, 0.0, 0.0)).length() < 1e-9);
        assert!(arc.last().unwrap().sub(Vec3::new(0.0, 1.0, 0.0)).length() < 1e-9);
        let centre = Vec3::new(1.0, 1.0, 0.0);
        for q in &arc {
            assert!((q.sub(centre).length() - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn offset_polyline_shifts_a_straight_run_by_distance() {
        let pts = [Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0)];
        let off = offset_polyline(&pts, Vec3::new(0.0, 0.0, 1.0), 0.5, false);
        for (o, p) in off.iter().zip(pts.iter()) {
            let d = o.sub(*p);
            assert!((d.length() - 0.5).abs() < 1e-9);
            assert!(d.z.abs() < 1e-9 && d.x.abs() < 1e-9);
        }
    }

    #[test]
    fn taper_scales_cross_section_with_distance() {
        // A square ring at along = 2 should grow by (1 + 0.5*2) = 2x.
        let pts = [
            Vec3::new(1.0, 0.0, 2.0),
            Vec3::new(0.0, 1.0, 2.0),
            Vec3::new(-1.0, 0.0, 2.0),
        ];
        let t = taper(&pts, Vec3::new(0.0, 0.0, 1.0), 0.5, Vec3::ZERO);
        for (a, b) in pts.iter().zip(t.iter()) {
            let ra = (a.x * a.x + a.y * a.y).sqrt();
            let rb = (b.x * b.x + b.y * b.y).sqrt();
            assert!((rb - 2.0 * ra).abs() < 1e-9);
            assert!((b.z - 2.0).abs() < 1e-9, "position along axis unchanged");
        }
    }

    #[test]
    fn angle_between_right_angle() {
        let a = angle_between(Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));
        assert!((a - FRAC_PI_2).abs() < 1e-9);
        let straight = angle_between(Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO, Vec3::new(-1.0, 0.0, 0.0));
        assert!((straight - PI).abs() < 1e-9);
    }

    #[test]
    fn mirror_point_and_vertices_across_xy() {
        let plane = WorkPlane::xy();
        assert!(mirror_point(Vec3::new(1.0, 2.0, 3.0), &plane).sub(Vec3::new(1.0, 2.0, -3.0)).length() < 1e-9);

        let m = crate::primitives::cube(2.0);
        let all: Vec<VertexId> = (0..m.vertex_count()).map(VertexId).collect();
        let mirrored = mirror_vertices(&m, &all, &plane);
        // A cube mirrored across its own centre plane is unchanged as a set.
        assert_eq!(mirrored.euler_characteristic(), 2);
    }

    #[test]
    fn place_vertex_adds_one() {
        let m = crate::primitives::cube(2.0);
        let (m2, id) = place_vertex(&m, &Placement::Absolute { coord: Vec3::new(9.0, 9.0, 9.0) });
        assert_eq!(m2.vertex_count(), m.vertex_count() + 1);
        assert!(m2.vertex(id).unwrap().position.sub(Vec3::new(9.0, 9.0, 9.0)).length() < 1e-9);
    }

    #[test]
    fn arc_semicircle_length_is_pi_r() {
        let arc = three_point_arc(
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(-2.0, 0.0, 0.0),
            64,
        );
        let len: f64 = arc.windows(2).map(|w| w[1].sub(w[0]).length()).sum();
        // Semicircle of radius 2: arc length = π · r = 2π.
        assert!((len - PI * 2.0).abs() < 0.01);
        let _ = TAU;
    }
}
