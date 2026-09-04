// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Snapping engine. Follows the published behaviour of Blender's snap system
// (source/blender/editors/transform/transform_snap.cc and
// transform_snap_object.cc, github.com/blender/blender, GPL-2.0-or-later):
// snap a transform to the nearest grid increment, vertex, edge point or face
// point of the static geometry, from a chosen base point of the moving
// selection. Concepts only — no upstream source copied; deterministic
// nearest-feature queries.
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

//! **Snapping engine** (`op-hzs.54.24`, GH issue #37 §D — the precision/CAD
//! core). Depends on [`crate::transform_input`].
//!
//! - [`SnapTarget`] — what to snap to: grid increment, vertex, edge midpoint,
//!   nearest point on an edge, nearest point on a face.
//! - [`SnapBase`] — which point of the moving selection is snapped: its
//!   closest vertex to the target, its bounding-box centre, its median, or a
//!   caller-nominated active vertex.
//! - [`snap_point`] — the nearest snap target to a query point.
//! - [`snap_translation`] — the delta that lands the base exactly on a target,
//!   or the raw delta if nothing is within `max_dist`.
//! - [`align_rotation_target`] — the surface normal at a face snap, so a caller
//!   can also orient the moved geometry (Blender's *Align Rotation to Target*).

use std::collections::BTreeSet;

use crate::math::Vec3;
use crate::mesh::{EdgeId, FaceId, Mesh, VertexId};

/// What a snap locks onto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnapTarget {
    /// Round each coordinate to a multiple of the given step (absolute grid).
    Increment(f64),
    /// The nearest static vertex.
    Vertex,
    /// The nearest static edge's midpoint.
    EdgeMidpoint,
    /// The nearest point lying on any static edge segment.
    EdgeNearest,
    /// The nearest point lying on any static face.
    FaceNearest,
}

/// Which point of the moving selection is aligned to the snap target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnapBase {
    /// The moving vertex currently closest to the target.
    Closest,
    /// The centre of the moving selection's bounding box.
    Center,
    /// The mean of the moving vertices.
    Median,
    /// A caller-nominated vertex.
    Active(VertexId),
}

/// A found snap location.
#[derive(Debug, Clone, Copy)]
pub struct SnapHit {
    /// Where to snap to.
    pub position: Vec3,
    /// The element the snap landed on (`None` for a grid snap).
    pub element: Option<SnapElement>,
}

/// Which mesh element a [`SnapHit`] is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapElement {
    Vertex(VertexId),
    Edge(EdgeId),
    Face(FaceId),
}

/// The nearest snap target to `query`. `exclude` vertices (and any element
/// using only excluded vertices) are ignored — pass the moving selection to
/// disable snap-onto-self. Returns `None` if nothing is within `max_dist`
/// (grid snaps always succeed).
pub fn snap_point(
    mesh: &Mesh,
    query: Vec3,
    target: SnapTarget,
    max_dist: f64,
    exclude: &[VertexId],
) -> Option<SnapHit> {
    let excl: BTreeSet<usize> = exclude.iter().map(|v| v.0).collect();
    let pos = mesh.positions();

    match target {
        SnapTarget::Increment(step) => {
            if step <= 0.0 {
                return None;
            }
            let s = |x: f64| (x / step).round() * step;
            Some(SnapHit {
                position: Vec3::new(s(query.x), s(query.y), s(query.z)),
                element: None,
            })
        }
        SnapTarget::Vertex => {
            let mut best: Option<(f64, usize)> = None;
            for (i, p) in pos.iter().enumerate() {
                if excl.contains(&i) {
                    continue;
                }
                let d = p.sub(query).length();
                if d <= max_dist && best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, i));
                }
            }
            best.map(|(_, i)| SnapHit {
                position: pos[i],
                element: Some(SnapElement::Vertex(VertexId(i))),
            })
        }
        SnapTarget::EdgeMidpoint | SnapTarget::EdgeNearest => {
            let mut best: Option<(f64, Vec3, EdgeId)> = None;
            for e in 0..mesh.edge_count() {
                let ed = mesh.edge(EdgeId(e)).unwrap();
                if excl.contains(&ed.verts[0].0) && excl.contains(&ed.verts[1].0) {
                    continue;
                }
                let (a, b) = (pos[ed.verts[0].0], pos[ed.verts[1].0]);
                let cand = if target == SnapTarget::EdgeMidpoint {
                    a.add(b).scale(0.5)
                } else {
                    closest_on_segment(query, a, b)
                };
                let d = cand.sub(query).length();
                if d <= max_dist && best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, cand, EdgeId(e)));
                }
            }
            best.map(|(_, p, e)| SnapHit {
                position: p,
                element: Some(SnapElement::Edge(e)),
            })
        }
        SnapTarget::FaceNearest => {
            let mut best: Option<(f64, Vec3, FaceId)> = None;
            for f in 0..mesh.face_count() {
                let vs = mesh.face_vertices(FaceId(f));
                if vs.iter().all(|v| excl.contains(&v.0)) {
                    continue;
                }
                let cand = closest_on_face(query, &vs.iter().map(|v| pos[v.0]).collect::<Vec<_>>());
                let d = cand.sub(query).length();
                if d <= max_dist && best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, cand, FaceId(f)));
                }
            }
            best.map(|(_, p, f)| SnapHit {
                position: p,
                element: Some(SnapElement::Face(f)),
            })
        }
    }
}

/// The translation delta that snaps the [`SnapBase`] of `moving` (currently at
/// its position `+ raw_delta`) onto the nearest [`SnapTarget`] of the static
/// geometry. Falls back to `raw_delta` when nothing is within `max_dist`.
pub fn snap_translation(
    mesh: &Mesh,
    moving: &[VertexId],
    raw_delta: Vec3,
    base: SnapBase,
    target: SnapTarget,
    max_dist: f64,
) -> Vec3 {
    let pos = mesh.positions();
    let mv: Vec<usize> = moving
        .iter()
        .map(|v| v.0)
        .filter(|&i| i < pos.len())
        .collect();
    if mv.is_empty() {
        return raw_delta;
    }

    let base_pos0 = match base {
        SnapBase::Active(v) => pos.get(v.0).copied().unwrap_or(Vec3::ZERO),
        SnapBase::Median => mv
            .iter()
            .fold(Vec3::ZERO, |acc, &i| acc.add(pos[i]))
            .scale(1.0 / mv.len() as f64),
        SnapBase::Center => {
            let (mut lo, mut hi) = (
                Vec3::new(f64::MAX, f64::MAX, f64::MAX),
                Vec3::new(f64::MIN, f64::MIN, f64::MIN),
            );
            for &i in &mv {
                lo = Vec3::new(lo.x.min(pos[i].x), lo.y.min(pos[i].y), lo.z.min(pos[i].z));
                hi = Vec3::new(hi.x.max(pos[i].x), hi.y.max(pos[i].y), hi.z.max(pos[i].z));
            }
            lo.add(hi).scale(0.5)
        }
        SnapBase::Closest => {
            // Provisional: use the median, then re-evaluate below.
            mv.iter()
                .fold(Vec3::ZERO, |acc, &i| acc.add(pos[i]))
                .scale(1.0 / mv.len() as f64)
        }
    };

    let query = base_pos0.add(raw_delta);
    let Some(hit) = snap_point(mesh, query, target, max_dist.max(1e-6), moving) else {
        return raw_delta;
    };

    match base {
        SnapBase::Closest => {
            // Snap whichever moving vertex is nearest the hit, then derive the
            // delta from that vertex.
            let nearest = mv
                .iter()
                .min_by(|&&a, &&b| {
                    pos[a]
                        .add(raw_delta)
                        .sub(hit.position)
                        .length()
                        .partial_cmp(&pos[b].add(raw_delta).sub(hit.position).length())
                        .unwrap()
                })
                .copied()
                .unwrap();
            hit.position.sub(pos[nearest])
        }
        _ => hit.position.sub(base_pos0),
    }
}

/// The unit normal at a face snap (Blender's *Align Rotation to Target*), or
/// `None` if the hit is not on a face.
pub fn align_rotation_target(mesh: &Mesh, hit: &SnapHit) -> Option<Vec3> {
    match hit.element? {
        SnapElement::Face(f) => Some(mesh.face_normal(f)),
        _ => None,
    }
}

// --- geometry helpers ---

fn closest_on_segment(p: Vec3, a: Vec3, b: Vec3) -> Vec3 {
    let ab = b.sub(a);
    let len2 = ab.dot(ab);
    if len2 < 1e-18 {
        return a;
    }
    let t = (p.sub(a).dot(ab) / len2).clamp(0.0, 1.0);
    a.add(ab.scale(t))
}

fn closest_on_face(p: Vec3, poly: &[Vec3]) -> Vec3 {
    if poly.len() < 3 {
        return poly.first().copied().unwrap_or(Vec3::ZERO);
    }
    // Project onto the face plane.
    let c = poly
        .iter()
        .fold(Vec3::ZERO, |acc, &q| acc.add(q))
        .scale(1.0 / poly.len() as f64);
    let mut n = Vec3::ZERO;
    for i in 0..poly.len() {
        let u = poly[i].sub(c);
        let v = poly[(i + 1) % poly.len()].sub(c);
        n = n.add(u.cross(v));
    }
    if n.length() < 1e-12 {
        return c;
    }
    let n = n.normalize();
    let proj = p.sub(n.scale(p.sub(c).dot(n)));

    // If the projection is inside the polygon (2-D point-in-poly via fan
    // sign test), use it; else clamp to the nearest boundary edge.
    if point_in_poly_3d(proj, poly, n) {
        proj
    } else {
        let mut best = poly[0];
        let mut bd = f64::MAX;
        for i in 0..poly.len() {
            let q = closest_on_segment(p, poly[i], poly[(i + 1) % poly.len()]);
            let d = q.sub(p).length();
            if d < bd {
                bd = d;
                best = q;
            }
        }
        best
    }
}

fn point_in_poly_3d(p: Vec3, poly: &[Vec3], n: Vec3) -> bool {
    // Sum of signed sub-triangle areas' consistency.
    let mut sign = 0.0;
    for i in 0..poly.len() {
        let a = poly[i].sub(p);
        let b = poly[(i + 1) % poly.len()].sub(p);
        let s = a.cross(b).dot(n);
        if s.abs() < 1e-12 {
            continue;
        }
        if sign == 0.0 {
            sign = s.signum();
        } else if s.signum() != sign {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn increment_snap_rounds_to_grid() {
        let m = primitives::cube(2.0);
        let h = snap_point(
            &m,
            Vec3::new(1.2, -0.4, 2.7),
            SnapTarget::Increment(1.0),
            0.0,
            &[],
        )
        .unwrap();
        assert_eq!(h.position, Vec3::new(1.0, 0.0, 3.0));
    }

    #[test]
    fn vertex_snap_finds_the_nearest_corner() {
        let m = primitives::cube(2.0); // corners at ±1
        let h = snap_point(&m, Vec3::new(0.9, 1.1, 0.8), SnapTarget::Vertex, 1.0, &[]).unwrap();
        assert_eq!(h.position, Vec3::new(1.0, 1.0, 1.0));
    }

    #[test]
    fn vertex_snap_respects_the_distance_limit() {
        let m = primitives::cube(2.0);
        assert!(snap_point(&m, Vec3::new(5.0, 5.0, 5.0), SnapTarget::Vertex, 1.0, &[]).is_none());
    }

    #[test]
    fn edge_midpoint_snap() {
        let m = primitives::cube(2.0);
        // Midpoint of the edge from (-1,-1,-1) to (1,-1,-1) is (0,-1,-1).
        let h = snap_point(
            &m,
            Vec3::new(0.1, -0.9, -1.1),
            SnapTarget::EdgeMidpoint,
            1.0,
            &[],
        )
        .unwrap();
        assert_eq!(h.position, Vec3::new(0.0, -1.0, -1.0));
    }

    #[test]
    fn face_nearest_projects_onto_the_face() {
        let m = primitives::grid(2, 2, 4.0); // z = 0 plane
        let h = snap_point(
            &m,
            Vec3::new(0.5, 0.5, 3.0),
            SnapTarget::FaceNearest,
            5.0,
            &[],
        )
        .unwrap();
        assert!((h.position.z).abs() < 1e-9);
        assert!((h.position.x - 0.5).abs() < 1e-9 && (h.position.y - 0.5).abs() < 1e-9);
        assert_eq!(align_rotation_target(&m, &h).unwrap().z.abs(), 1.0);
    }

    #[test]
    fn snap_translation_lands_the_base_on_a_vertex() {
        // Two separate cubes; drag one so its corner meets the other's corner.
        let a = primitives::cube(2.0); // corners ±1
        let mut positions = a.positions();
        let mut faces: Vec<Vec<usize>> = a
            .polygons()
            .iter()
            .map(|f| f.iter().map(|v| v.0).collect())
            .collect();
        let off = positions.len();
        for p in a.positions() {
            positions.push(p.add(Vec3::new(5.0, 0.0, 0.0))); // second cube at x ∈ [4,6]
        }
        for f in a.polygons() {
            faces.push(f.iter().map(|v| v.0 + off).collect());
        }
        let m = Mesh::from_polygons(&positions, &faces);

        let moving: Vec<VertexId> = (off..m.vertex_count()).map(VertexId).collect();
        // Roughly drag the second cube left by ~3; snapping should pull a
        // moving corner exactly onto a static corner.
        let delta = snap_translation(
            &m,
            &moving,
            Vec3::new(-3.2, 0.1, 0.0),
            SnapBase::Closest,
            SnapTarget::Vertex,
            2.0,
        );
        // After applying, some moving corner coincides with a static corner.
        let mut coincident = false;
        for &mv in &moving {
            let np = m.vertex(mv).unwrap().position.add(delta);
            for s in 0..off {
                if np.sub(m.vertex(VertexId(s)).unwrap().position).length() < 1e-9 {
                    coincident = true;
                }
            }
        }
        assert!(coincident, "a moving corner snapped onto a static corner");
    }

    #[test]
    fn snap_translation_falls_back_when_out_of_range() {
        let m = primitives::cube(2.0);
        let d = snap_translation(
            &m,
            &[VertexId(0)],
            Vec3::new(100.0, 0.0, 0.0),
            SnapBase::Median,
            SnapTarget::Vertex,
            0.5,
        );
        assert_eq!(d, Vec3::new(100.0, 0.0, 0.0));
    }
}
