// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// 3D cursor, pivot points and custom transform orientations. Follows the
// published behaviour of Blender (source/blender/editors/space_view3d/
// view3d_cursor_snap.cc, the pivot-point handling in transform_convert.cc, and
// transform_orientations.cc, github.com/blender/blender, GPL-2.0-or-later):
// place the 3D cursor, choose the point a rotation/scale pivots about, and
// derive a coordinate frame from a vertex/edge/face selection. Concepts only —
// no upstream source copied.
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

//! **3D cursor + pivot points + custom orientations** (`op-hzs.54.25`, GH issue
//! #37 §D).
//!
//! - [`Cursor3D`] and its placement helpers ([`Cursor3D::to_grid`],
//!   [`Cursor3D::to_selected`], [`Cursor3D::to_active`],
//!   [`Cursor3D::to_world_origin`], [`selection_to_cursor`]).
//! - [`PivotPoint`] and [`pivot_position`] — the point a rotation / scale
//!   turns about.
//! - [`rotate_about_pivot`] / [`scale_about_pivot`] — apply a transform to a
//!   vertex selection about a pivot, with [`PivotPoint::IndividualOrigins`]
//!   handled per connected component.
//! - [`orientation_from_selection`] — a [`TransformBasis`] from a
//!   vertex / edge / face selection (Blender's *Create Orientation*).

use std::collections::BTreeSet;

use crate::math::Vec3;
use crate::mesh::{EdgeId, FaceId, Mesh, VertexId};
use crate::selection::Axis;
use crate::transform_input::TransformBasis;

/// The 3D cursor: a position and a rotation frame.
#[derive(Debug, Clone, Copy)]
pub struct Cursor3D {
    pub position: Vec3,
    pub basis: TransformBasis,
}

impl Default for Cursor3D {
    fn default() -> Self {
        Cursor3D { position: Vec3::ZERO, basis: TransformBasis::global() }
    }
}

impl Cursor3D {
    /// Snap the cursor position to a multiple of `step` on each axis.
    pub fn to_grid(&mut self, step: f64) {
        if step > 0.0 {
            let s = |x: f64| (x / step).round() * step;
            self.position = Vec3::new(s(self.position.x), s(self.position.y), s(self.position.z));
        }
    }

    /// Move the cursor to the median of `verts` (empty = whole mesh).
    pub fn to_selected(&mut self, mesh: &Mesh, verts: &[VertexId]) {
        let pos = mesh.positions();
        let idx: Vec<usize> = if verts.is_empty() {
            (0..pos.len()).collect()
        } else {
            verts.iter().map(|v| v.0).filter(|&i| i < pos.len()).collect()
        };
        if !idx.is_empty() {
            self.position =
                idx.iter().fold(Vec3::ZERO, |acc, &i| acc.add(pos[i])).scale(1.0 / idx.len() as f64);
        }
    }

    /// Move the cursor to a single active vertex.
    pub fn to_active(&mut self, mesh: &Mesh, active: VertexId) {
        if let Some(v) = mesh.vertex(active) {
            self.position = v.position;
        }
    }

    /// Move the cursor to the world origin.
    pub fn to_world_origin(&mut self) {
        self.position = Vec3::ZERO;
    }
}

/// The delta that moves `verts` so their median lands on `cursor`.
pub fn selection_to_cursor(mesh: &Mesh, verts: &[VertexId], cursor: Vec3) -> Vec3 {
    let pos = mesh.positions();
    let idx: Vec<usize> = if verts.is_empty() {
        (0..pos.len()).collect()
    } else {
        verts.iter().map(|v| v.0).collect()
    };
    if idx.is_empty() {
        return Vec3::ZERO;
    }
    let median = idx.iter().fold(Vec3::ZERO, |acc, &i| acc.add(pos[i])).scale(1.0 / idx.len() as f64);
    cursor.sub(median)
}

/// Which point a rotation / scale pivots about.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PivotPoint {
    /// Centre of the selection's axis-aligned bounding box.
    BoundingBoxCenter,
    /// The 3D cursor.
    Cursor,
    /// Each connected component about its own median.
    IndividualOrigins,
    /// The mean of the selected vertices.
    MedianPoint,
    /// A caller-nominated active vertex.
    ActiveElement(VertexId),
}

/// The single pivot position for `pivot` (for [`PivotPoint::IndividualOrigins`]
/// this returns the overall median — use [`rotate_about_pivot`] for the real
/// per-component behaviour).
pub fn pivot_position(
    mesh: &Mesh,
    verts: &[VertexId],
    pivot: PivotPoint,
    cursor: Vec3,
) -> Vec3 {
    let pos = mesh.positions();
    let idx: Vec<usize> = if verts.is_empty() {
        (0..pos.len()).collect()
    } else {
        verts.iter().map(|v| v.0).filter(|&i| i < pos.len()).collect()
    };
    match pivot {
        PivotPoint::Cursor => cursor,
        PivotPoint::ActiveElement(v) => pos.get(v.0).copied().unwrap_or(cursor),
        PivotPoint::MedianPoint | PivotPoint::IndividualOrigins => {
            if idx.is_empty() {
                cursor
            } else {
                idx.iter().fold(Vec3::ZERO, |acc, &i| acc.add(pos[i])).scale(1.0 / idx.len() as f64)
            }
        }
        PivotPoint::BoundingBoxCenter => {
            if idx.is_empty() {
                return cursor;
            }
            let (mut lo, mut hi) = (
                Vec3::new(f64::MAX, f64::MAX, f64::MAX),
                Vec3::new(f64::MIN, f64::MIN, f64::MIN),
            );
            for &i in &idx {
                lo = Vec3::new(lo.x.min(pos[i].x), lo.y.min(pos[i].y), lo.z.min(pos[i].z));
                hi = Vec3::new(hi.x.max(pos[i].x), hi.y.max(pos[i].y), hi.z.max(pos[i].z));
            }
            lo.add(hi).scale(0.5)
        }
    }
}

/// Rotate `verts` by `angle` about `axis` through the `pivot` point.
/// [`PivotPoint::IndividualOrigins`] rotates each connected component about its
/// own median.
pub fn rotate_about_pivot(
    mesh: &Mesh,
    verts: &[VertexId],
    pivot: PivotPoint,
    axis: Axis,
    angle: f64,
    cursor: Vec3,
) -> Mesh {
    transform_about_pivot(mesh, verts, pivot, cursor, |p, c| {
        c.add(rotate_about_axis(p.sub(c), axis_unit(axis), angle))
    })
}

/// Scale `verts` by `factor` about the `pivot` point.
pub fn scale_about_pivot(
    mesh: &Mesh,
    verts: &[VertexId],
    pivot: PivotPoint,
    factor: f64,
    cursor: Vec3,
) -> Mesh {
    transform_about_pivot(mesh, verts, pivot, cursor, |p, c| c.add(p.sub(c).scale(factor)))
}

fn transform_about_pivot(
    mesh: &Mesh,
    verts: &[VertexId],
    pivot: PivotPoint,
    cursor: Vec3,
    xf: impl Fn(Vec3, Vec3) -> Vec3,
) -> Mesh {
    let mut positions = mesh.positions();
    let sel: Vec<usize> = if verts.is_empty() {
        (0..positions.len()).collect()
    } else {
        verts.iter().map(|v| v.0).filter(|&i| i < positions.len()).collect()
    };

    if pivot == PivotPoint::IndividualOrigins {
        for comp in components(mesh, &sel) {
            let c = comp.iter().fold(Vec3::ZERO, |acc, &i| acc.add(positions[i])).scale(1.0 / comp.len().max(1) as f64);
            for &i in &comp {
                positions[i] = xf(positions[i], c);
            }
        }
    } else {
        let c = pivot_position(mesh, verts, pivot, cursor);
        for &i in &sel {
            positions[i] = xf(positions[i], c);
        }
    }
    Mesh::from_polygons(
        &positions,
        &mesh.polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect::<Vec<_>>(),
    )
}

/// A [`TransformBasis`] derived from a selection (Blender's *Create
/// Orientation*):
///
/// - a **face** — `z` = face normal, `x` along its first edge;
/// - an **edge** — `z` along the edge, `x` an arbitrary completion;
/// - two or more **vertices** — `z` along the line of best fit (here the vector
///   between the two farthest-apart), `x` an arbitrary completion.
pub fn orientation_from_selection(
    mesh: &Mesh,
    verts: &[VertexId],
    edges: &[EdgeId],
    faces: &[FaceId],
) -> Option<TransformBasis> {
    if let Some(&f) = faces.first() {
        let n = mesh.face_normal(f);
        let vs = mesh.face_vertices(f);
        if vs.len() >= 2 {
            let edge = mesh.vertex(vs[1])?.position.sub(mesh.vertex(vs[0])?.position);
            let x = ortho(edge, n);
            return Some(TransformBasis { x, y: n.cross(x), z: n });
        }
    }
    if let Some(&e) = edges.first() {
        let ed = mesh.edge(e)?;
        let dir = mesh.vertex(ed.verts[1])?.position.sub(mesh.vertex(ed.verts[0])?.position);
        return Some(TransformBasis::from_normal(dir));
    }
    if verts.len() >= 2 {
        let pos = mesh.positions();
        let ps: Vec<Vec3> = verts.iter().filter_map(|v| pos.get(v.0).copied()).collect();
        let mut best = (0.0, Vec3::new(0.0, 0.0, 1.0));
        for i in 0..ps.len() {
            for j in i + 1..ps.len() {
                let d = ps[j].sub(ps[i]);
                if d.length() > best.0 {
                    best = (d.length(), d);
                }
            }
        }
        return Some(TransformBasis::from_normal(best.1));
    }
    None
}

// --- helpers ---

fn axis_unit(axis: Axis) -> Vec3 {
    match axis {
        Axis::X => Vec3::new(1.0, 0.0, 0.0),
        Axis::Y => Vec3::new(0.0, 1.0, 0.0),
        Axis::Z => Vec3::new(0.0, 0.0, 1.0),
    }
}

fn rotate_about_axis(v: Vec3, k: Vec3, theta: f64) -> Vec3 {
    let (s, c) = theta.sin_cos();
    v.scale(c).add(k.cross(v).scale(s)).add(k.scale(k.dot(v) * (1.0 - c)))
}

/// The component of `v` orthogonal to unit `n`, normalised (or an arbitrary
/// perpendicular if `v` is parallel to `n`).
fn ortho(v: Vec3, n: Vec3) -> Vec3 {
    let p = v.sub(n.scale(v.dot(n)));
    if p.length() > 1e-9 {
        p.normalize()
    } else {
        let up = if n.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
        up.cross(n).normalize()
    }
}

/// Connected components of the selected vertex set (by shared edge).
fn components(mesh: &Mesh, sel: &[usize]) -> Vec<Vec<usize>> {
    let want: BTreeSet<usize> = sel.iter().copied().collect();
    let mut adj: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for e in 0..mesh.edge_count() {
        let ed = mesh.edge(EdgeId(e)).unwrap();
        let (a, b) = (ed.verts[0].0, ed.verts[1].0);
        if want.contains(&a) && want.contains(&b) {
            adj.entry(a).or_default().push(b);
            adj.entry(b).or_default().push(a);
        }
    }
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    let mut out = Vec::new();
    for &s in &want {
        if seen.contains(&s) {
            continue;
        }
        let mut comp = Vec::new();
        let mut stack = vec![s];
        while let Some(v) = stack.pop() {
            if !seen.insert(v) {
                continue;
            }
            comp.push(v);
            for &n in adj.get(&v).map(|x| x.as_slice()).unwrap_or(&[]) {
                if !seen.contains(&n) {
                    stack.push(n);
                }
            }
        }
        out.push(comp);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn cursor_placement_helpers() {
        let m = primitives::cube(2.0);
        let mut c = Cursor3D::default();
        c.position = Vec3::new(1.3, -0.6, 2.1);
        c.to_grid(1.0);
        assert_eq!(c.position, Vec3::new(1.0, -1.0, 2.0));

        c.to_selected(&m, &[VertexId(0), VertexId(6)]); // opposite corners
        assert_eq!(c.position, Vec3::ZERO);

        c.to_active(&m, VertexId(6));
        assert_eq!(c.position, m.vertex(VertexId(6)).unwrap().position);

        c.to_world_origin();
        assert_eq!(c.position, Vec3::ZERO);
    }

    #[test]
    fn selection_to_cursor_delta() {
        let m = primitives::grid(1, 1, 2.0);
        let d = selection_to_cursor(&m, &[], Vec3::new(5.0, 5.0, 5.0));
        // Grid median is the origin, so the delta is the cursor itself.
        assert_eq!(d, Vec3::new(5.0, 5.0, 5.0));
    }

    #[test]
    fn pivot_positions() {
        let m = primitives::cube(2.0); // corners ±1
        assert_eq!(pivot_position(&m, &[], PivotPoint::BoundingBoxCenter, Vec3::ZERO), Vec3::ZERO);
        assert_eq!(pivot_position(&m, &[], PivotPoint::MedianPoint, Vec3::ZERO), Vec3::ZERO);
        assert_eq!(
            pivot_position(&m, &[], PivotPoint::Cursor, Vec3::new(7.0, 0.0, 0.0)),
            Vec3::new(7.0, 0.0, 0.0)
        );
    }

    #[test]
    fn rotate_about_cursor_turns_the_selection() {
        let m = primitives::grid(1, 1, 2.0);
        let r = rotate_about_pivot(&m, &[], PivotPoint::Cursor, Axis::Z, std::f64::consts::FRAC_PI_2, Vec3::ZERO);
        // A corner at (1, y, 0) rotates 90° about Z to (~-y, 1, 0)... check a
        // known one: the vertex farthest along +x moves onto +y.
        let far_x = (0..m.vertex_count())
            .max_by(|&a, &b| m.vertex(VertexId(a)).unwrap().position.x.partial_cmp(&m.vertex(VertexId(b)).unwrap().position.x).unwrap())
            .unwrap();
        let p = r.vertex(VertexId(far_x)).unwrap().position;
        assert!(p.x.abs() < 1.001 && p.y > 0.9);
    }

    #[test]
    fn scale_about_pivot_grows_the_selection() {
        let m = primitives::cube(2.0);
        let s = scale_about_pivot(&m, &[], PivotPoint::MedianPoint, 2.0, Vec3::ZERO);
        for i in 0..8 {
            assert!((s.vertex(VertexId(i)).unwrap().position.length() - 2.0 * m.vertex(VertexId(i)).unwrap().position.length()).abs() < 1e-9);
        }
    }

    #[test]
    fn individual_origins_scales_each_component_locally() {
        // Two separate cubes; scaling with IndividualOrigins keeps each cube's
        // centre fixed rather than pushing them apart.
        let a = primitives::cube(2.0);
        let mut positions = a.positions();
        let mut faces: Vec<Vec<usize>> = a.polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect();
        let off = positions.len();
        for p in a.positions() {
            positions.push(p.add(Vec3::new(10.0, 0.0, 0.0)));
        }
        for f in a.polygons() {
            faces.push(f.iter().map(|v| v.0 + off).collect());
        }
        let m = Mesh::from_polygons(&positions, &faces);
        let s = scale_about_pivot(&m, &[], PivotPoint::IndividualOrigins, 2.0, Vec3::ZERO);
        // Second cube's centre stays near (10, 0, 0).
        let c2 = (off..m.vertex_count()).fold(Vec3::ZERO, |acc, i| acc.add(s.vertex(VertexId(i)).unwrap().position)).scale(1.0 / 8.0);
        assert!(c2.sub(Vec3::new(10.0, 0.0, 0.0)).length() < 1e-6);
    }

    #[test]
    fn orientation_from_a_face_uses_its_normal() {
        let m = primitives::grid(1, 1, 2.0); // z = 0 plane, normal +z
        let b = orientation_from_selection(&m, &[], &[], &[FaceId(0)]).unwrap();
        assert!((b.z.z.abs() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn orientation_from_an_edge_runs_along_it() {
        let m = primitives::grid(1, 1, 2.0);
        let b = orientation_from_selection(&m, &[], &[EdgeId(0)], &[]).unwrap();
        let ed = m.edge(EdgeId(0)).unwrap();
        let dir = m.vertex(ed.verts[1]).unwrap().position.sub(m.vertex(ed.verts[0]).unwrap().position).normalize();
        assert!(b.z.sub(dir).length() < 1e-9 || b.z.add(dir).length() < 1e-9);
    }
}
