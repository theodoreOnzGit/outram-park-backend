// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// The interactive "draw a primitive" gestures as parameterised, headless
// operators. Blender analogue (architecture only): the Add Object Tool
// interactive add operators (`object.primitive_add` interactive mode /
// `MESH_OT_primitive_*_add` with the "place" gizmo) plus the Snap/PDT numeric
// entry. No upstream source copied.
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

//! **Interactive primitive-draw tool** (`op-hzs.54.41`, GH issue #37 §H) — the
//! CAD "draw a box / circle / cone" gesture, expressed as a headless staged
//! operator instead of a mouse-driven modal.
//!
//! The gesture is: pick a [`WorkPlane`], drag out a footprint rectangle on it,
//! then drag a depth along the plane normal. Each 3-D input point can be run
//! through the [`crate::snap`] engine, and each scalar (a footprint side, the
//! depth) can be typed as an expression evaluated by
//! [`crate::transform_input::eval_expr`].
//!
//! - [`WorkPlane`] — an oriented base plane (`origin`, orthonormal `u`, `v`,
//!   `normal`); [`WorkPlane::xy`] / [`WorkPlane::xz`] / [`WorkPlane::yz`] /
//!   [`WorkPlane::from_origin_normal`].
//! - [`DrawGesture`] — the staged state machine (`PickBase → DragFootprint →
//!   DragDepth → Done`); [`DrawGesture::resolve`] builds the [`Mesh`].
//! - [`box_from_drag`] / [`circle_from_drag`] / [`cone_from_drag`] — the
//!   one-shot forms when you already have the points.
//! - [`snap_input`] — project one world point onto a [`crate::snap::SnapTarget`].
//!
//! ## Units
//!
//! Points and lengths are dimensionless model-space quantities (see
//! [`crate::math`]).

use crate::math::Vec3;
use crate::mesh::Mesh;
use crate::snap::{snap_point, SnapTarget};

/// An oriented drawing plane: a point on it plus an orthonormal basis, where
/// `u` and `v` span the plane and `normal = u x v` is the extrude direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorkPlane {
    /// A point on the plane (the gesture's local origin).
    pub origin: Vec3,
    /// In-plane "x" axis (unit).
    pub u: Vec3,
    /// In-plane "y" axis (unit, perpendicular to `u`).
    pub v: Vec3,
    /// Plane normal / extrude axis (unit, `= u x v`).
    pub normal: Vec3,
}

impl WorkPlane {
    /// The world `x-y` plane through the origin, extruding along `+z`.
    pub fn xy() -> Self {
        WorkPlane {
            origin: Vec3::ZERO,
            u: Vec3::new(1.0, 0.0, 0.0),
            v: Vec3::new(0.0, 1.0, 0.0),
            normal: Vec3::new(0.0, 0.0, 1.0),
        }
    }

    /// The world `x-z` plane, extruding along `+y`.
    pub fn xz() -> Self {
        WorkPlane {
            origin: Vec3::ZERO,
            u: Vec3::new(1.0, 0.0, 0.0),
            v: Vec3::new(0.0, 0.0, 1.0),
            normal: Vec3::new(0.0, -1.0, 0.0),
        }
    }

    /// The world `y-z` plane, extruding along `+x`.
    pub fn yz() -> Self {
        WorkPlane {
            origin: Vec3::ZERO,
            u: Vec3::new(0.0, 1.0, 0.0),
            v: Vec3::new(0.0, 0.0, 1.0),
            normal: Vec3::new(1.0, 0.0, 0.0),
        }
    }

    /// A plane through `origin` with the given `normal` (need not be unit); the
    /// in-plane basis is chosen deterministically (Duff et al. orthonormal
    /// basis) so the same normal always yields the same `u`, `v`.
    pub fn from_origin_normal(origin: Vec3, normal: Vec3) -> Self {
        let n = normal.normalize();
        let s = if n.z >= 0.0 { 1.0 } else { -1.0 };
        let a = -1.0 / (s + n.z);
        let b = n.x * n.y * a;
        let u = Vec3::new(1.0 + s * n.x * n.x * a, s * b, -s * n.x);
        let v = Vec3::new(b, s + n.y * n.y * a, -n.y);
        WorkPlane {
            origin,
            u: u.normalize(),
            v: v.normalize(),
            normal: n,
        }
    }

    /// World-space point for plane coordinates `(a, b)` and height `h` along
    /// the normal.
    pub fn point(&self, a: f64, b: f64, h: f64) -> Vec3 {
        self.origin
            .add(self.u.scale(a))
            .add(self.v.scale(b))
            .add(self.normal.scale(h))
    }

    /// Project a world point onto plane coordinates `(u_coord, v_coord)`
    /// (the normal component is dropped).
    pub fn project(&self, p: Vec3) -> (f64, f64) {
        let d = p.sub(self.origin);
        (d.dot(self.u), d.dot(self.v))
    }
}

/// Project one world point onto a snap target of `mesh`, returning the snapped
/// position (or `p` unchanged if nothing is within `max_dist`).
pub fn snap_input(mesh: &Mesh, p: Vec3, target: SnapTarget, max_dist: f64) -> Vec3 {
    snap_point(mesh, p, target, max_dist, &[])
        .map(|h| h.position)
        .unwrap_or(p)
}

/// Evaluate a scalar that may be a literal or an expression (`"2*0.5"`,
/// `"pi/4"`); `None` on a parse error.
pub fn eval_dimension(s: &str) -> Option<f64> {
    crate::transform_input::eval_expr(s)
}

/// A box from two opposite base corners (world points, assumed on/near the
/// plane) and a `depth` along the plane normal. The footprint is the
/// axis-aligned (in plane coords) rectangle spanned by the two corners.
pub fn box_from_drag(plane: &WorkPlane, corner_a: Vec3, corner_b: Vec3, depth: f64) -> Mesh {
    let (a0, a1) = plane.project(corner_a);
    let (b0, b1) = plane.project(corner_b);
    let (u0, u1) = (a0.min(b0), a0.max(b0));
    let (v0, v1) = (a1.min(b1), a1.max(b1));
    let base = [
        plane.point(u0, v0, 0.0),
        plane.point(u1, v0, 0.0),
        plane.point(u1, v1, 0.0),
        plane.point(u0, v1, 0.0),
    ];
    let top: Vec<Vec3> = base
        .iter()
        .map(|p| p.add(plane.normal.scale(depth)))
        .collect();
    let mut positions = base.to_vec();
    positions.extend(top);
    let faces = vec![
        vec![0usize, 3, 2, 1], // base (facing -normal)
        vec![4, 5, 6, 7],      // top (facing +normal)
        vec![0, 1, 5, 4],
        vec![1, 2, 6, 5],
        vec![2, 3, 7, 6],
        vec![3, 0, 4, 7],
    ];
    Mesh::from_polygons(&positions, &faces)
}

/// A cylinder from a base centre, a rim point (radius = their in-plane
/// distance) and a `depth` along the normal. `segments` clamped `>= 3`.
pub fn circle_from_drag(
    plane: &WorkPlane,
    center: Vec3,
    rim: Vec3,
    depth: f64,
    segments: usize,
) -> Mesh {
    let radius = in_plane_distance(plane, center, rim);
    disc_prism(plane, center, radius, radius, depth, segments)
}

/// A cone (apex up) from a base centre, a rim point and a `depth`.
/// `segments` clamped `>= 3`.
pub fn cone_from_drag(
    plane: &WorkPlane,
    center: Vec3,
    rim: Vec3,
    depth: f64,
    segments: usize,
) -> Mesh {
    let radius = in_plane_distance(plane, center, rim);
    disc_prism(plane, center, radius, 0.0, depth, segments)
}

fn in_plane_distance(plane: &WorkPlane, a: Vec3, b: Vec3) -> f64 {
    let (a0, a1) = plane.project(a);
    let (b0, b1) = plane.project(b);
    ((a0 - b0).powi(2) + (a1 - b1).powi(2)).sqrt()
}

/// Generalised disc → (disc | apex) prism on the plane, `r_top == 0` → cone.
fn disc_prism(
    plane: &WorkPlane,
    center: Vec3,
    r_base: f64,
    r_top: f64,
    depth: f64,
    segments: usize,
) -> Mesh {
    let n = segments.max(3);
    let (cu, cv) = plane.project(center);
    let mut positions: Vec<Vec3> = Vec::new();
    for i in 0..n {
        let a = std::f64::consts::TAU * i as f64 / n as f64;
        positions.push(plane.point(cu + r_base * a.cos(), cv + r_base * a.sin(), 0.0));
    }
    let apex = r_top.abs() < 1e-12;
    if apex {
        positions.push(plane.point(cu, cv, depth));
    } else {
        for i in 0..n {
            let a = std::f64::consts::TAU * i as f64 / n as f64;
            positions.push(plane.point(cu + r_top * a.cos(), cv + r_top * a.sin(), depth));
        }
    }
    let mut faces: Vec<Vec<usize>> = Vec::new();
    faces.push((0..n).rev().collect()); // base cap
    if apex {
        for i in 0..n {
            faces.push(vec![i, (i + 1) % n, n]);
        }
    } else {
        for i in 0..n {
            let j = (i + 1) % n;
            faces.push(vec![i, j, n + j, n + i]);
        }
        faces.push((n..2 * n).collect()); // top cap
    }
    Mesh::from_polygons(&positions, &faces)
}

/// Which primitive a [`DrawGesture`] builds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrawKind {
    /// A rectangular box.
    Box,
    /// A cylinder (footprint's shorter side is the diameter proxy — the
    /// gesture uses the drag distance as the radius directly).
    Cylinder {
        /// Sides around the axis.
        segments: usize,
    },
    /// A cone with the apex at the depth end.
    Cone {
        /// Sides around the base.
        segments: usize,
    },
}

/// The stage a [`DrawGesture`] is at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Stage {
    /// Waiting for the first base point.
    PickBase,
    /// Have the first point; waiting for the opposite footprint corner / rim.
    DragFootprint,
    /// Have the footprint; waiting for the depth.
    DragDepth,
    /// Complete — [`DrawGesture::resolve`] will succeed.
    Done,
}

/// The staged "draw a primitive" operator. Feed it points (optionally
/// snapped by the caller via [`snap_input`]); call [`DrawGesture::resolve`]
/// once [`DrawGesture::stage`] is [`Stage::Done`].
#[derive(Debug, Clone)]
pub struct DrawGesture {
    plane: WorkPlane,
    kind: DrawKind,
    p0: Option<Vec3>,
    p1: Option<Vec3>,
    depth: Option<f64>,
}

impl DrawGesture {
    /// Start a gesture on `plane` building `kind`.
    pub fn new(plane: WorkPlane, kind: DrawKind) -> Self {
        DrawGesture {
            plane,
            kind,
            p0: None,
            p1: None,
            depth: None,
        }
    }

    /// Current stage.
    pub fn stage(&self) -> Stage {
        match (self.p0, self.p1, self.depth) {
            (None, _, _) => Stage::PickBase,
            (Some(_), None, _) => Stage::DragFootprint,
            (Some(_), Some(_), None) => Stage::DragDepth,
            (Some(_), Some(_), Some(_)) => Stage::Done,
        }
    }

    /// Supply the next point in the gesture (base corner, then footprint
    /// corner / rim). Ignored once both are set.
    pub fn push_point(&mut self, world: Vec3) {
        if self.p0.is_none() {
            self.p0 = Some(world);
        } else if self.p1.is_none() {
            self.p1 = Some(world);
        }
    }

    /// Supply the depth by a world point: the signed distance from the base
    /// plane to `world` along the plane normal.
    pub fn push_depth_point(&mut self, world: Vec3) {
        let h = world.sub(self.plane.origin).dot(self.plane.normal);
        self.depth = Some(h);
    }

    /// Supply the depth directly (or from [`eval_dimension`]).
    pub fn set_depth(&mut self, depth: f64) {
        self.depth = Some(depth);
    }

    /// Build the mesh. `None` unless [`Self::stage`] is [`Stage::Done`].
    pub fn resolve(&self) -> Option<Mesh> {
        let (p0, p1, d) = (self.p0?, self.p1?, self.depth?);
        Some(match self.kind {
            DrawKind::Box => box_from_drag(&self.plane, p0, p1, d),
            DrawKind::Cylinder { segments } => circle_from_drag(&self.plane, p0, p1, d, segments),
            DrawKind::Cone { segments } => cone_from_drag(&self.plane, p0, p1, d, segments),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::bounding_box;
    use crate::primitives;

    #[test]
    fn box_drag_on_xy_has_the_dragged_extents() {
        let plane = WorkPlane::xy();
        let m = box_from_drag(
            &plane,
            Vec3::new(-1.0, -2.0, 0.0),
            Vec3::new(3.0, 1.0, 0.0),
            5.0,
        );
        assert_eq!(m.euler_characteristic(), 2);
        let (lo, hi) = bounding_box(&m);
        assert!((hi.x - lo.x - 4.0).abs() < 1e-9);
        assert!((hi.y - lo.y - 3.0).abs() < 1e-9);
        assert!((hi.z - lo.z - 5.0).abs() < 1e-9);
    }

    #[test]
    fn box_drag_on_yz_extrudes_along_x() {
        let plane = WorkPlane::yz();
        let m = box_from_drag(
            &plane,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 2.0, 2.0),
            3.0,
        );
        let (lo, hi) = bounding_box(&m);
        assert!(
            (hi.x - lo.x - 3.0).abs() < 1e-9,
            "depth is along +x on the yz plane"
        );
    }

    #[test]
    fn circle_drag_radius_is_the_in_plane_distance() {
        let plane = WorkPlane::xy();
        let m = circle_from_drag(&plane, Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0), 1.0, 24);
        assert_eq!(m.euler_characteristic(), 2);
        let (lo, hi) = bounding_box(&m);
        assert!((hi.x - lo.x - 4.0).abs() < 1e-6, "diameter = 2 * radius");
    }

    #[test]
    fn cone_drag_is_a_closed_solid_with_an_apex() {
        let plane = WorkPlane::xy();
        let m = cone_from_drag(&plane, Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), 2.0, 16);
        assert_eq!(m.vertex_count(), 17);
        assert_eq!(m.euler_characteristic(), 2);
    }

    #[test]
    fn gesture_state_machine_advances_and_resolves() {
        let mut g = DrawGesture::new(WorkPlane::xy(), DrawKind::Box);
        assert_eq!(g.stage(), Stage::PickBase);
        g.push_point(Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(g.stage(), Stage::DragFootprint);
        g.push_point(Vec3::new(2.0, 3.0, 0.0));
        assert_eq!(g.stage(), Stage::DragDepth);
        assert!(g.resolve().is_none());
        g.push_depth_point(Vec3::new(1.0, 1.0, 4.0));
        assert_eq!(g.stage(), Stage::Done);
        let m = g.resolve().unwrap();
        let (lo, hi) = bounding_box(&m);
        assert!((hi.z - lo.z - 4.0).abs() < 1e-9);
    }

    #[test]
    fn snap_input_locks_onto_a_vertex() {
        let cube = primitives::cube(2.0); // corners at +-1
        let raw = Vec3::new(0.9, 1.1, 0.95);
        let snapped = snap_input(&cube, raw, SnapTarget::Vertex, 0.5);
        assert!(snapped.sub(Vec3::new(1.0, 1.0, 1.0)).length() < 1e-9);
        // Out of range → unchanged.
        let far = snap_input(&cube, Vec3::new(5.0, 5.0, 5.0), SnapTarget::Vertex, 0.5);
        assert_eq!(far, Vec3::new(5.0, 5.0, 5.0));
    }

    #[test]
    fn snap_input_grid_increment() {
        let cube = primitives::cube(2.0);
        let snapped = snap_input(
            &cube,
            Vec3::new(0.34, 0.71, -0.4),
            SnapTarget::Increment(0.25),
            1.0,
        );
        assert!(snapped.sub(Vec3::new(0.25, 0.75, -0.5)).length() < 1e-9);
    }

    #[test]
    fn eval_dimension_parses_expressions() {
        assert!((eval_dimension("2*0.5").unwrap() - 1.0).abs() < 1e-12);
        assert!((eval_dimension("pi/2").unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!(eval_dimension("not a number").is_none());
    }

    #[test]
    fn work_plane_from_normal_is_orthonormal() {
        let p = WorkPlane::from_origin_normal(Vec3::new(1.0, 2.0, 3.0), Vec3::new(1.0, 1.0, 1.0));
        assert!(p.u.dot(p.v).abs() < 1e-9);
        assert!(p.u.dot(p.normal).abs() < 1e-9);
        assert!(p.v.dot(p.normal).abs() < 1e-9);
        assert!((p.u.length() - 1.0).abs() < 1e-9);
        assert!((p.normal.length() - 1.0).abs() < 1e-9);
    }
}
