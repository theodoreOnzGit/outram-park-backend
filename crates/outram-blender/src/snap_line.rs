// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// The "Snap Utilities Line" CAD polyline tool. Blender analogue (architecture
// only): the bundled `mesh_snap_utilities_line` add-on — connected-vertex
// placement with live geometry snapping, incremental snap, numeric length &
// angle entry, and auto-cut of crossed faces. No upstream source copied;
// reuses this crate's snap engine, knife and work-plane types.
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

//! **Snap Utilities Line** (`op-hzs.54.43`, GH issue #37 §I) — place a
//! connected polyline with live snapping and numeric length/angle entry,
//! expressed as a headless staged operator.
//!
//! - [`LineTool`] — the growing polyline. Add points [`LineTool::add_raw`],
//!   [`LineTool::add_snapped`] (through the [`crate::snap`] engine), or
//!   [`LineTool::add_polar`] / [`LineTool::add_constrained`] (numeric length
//!   and/or angle relative to a [`crate::draw_tool::WorkPlane`]).
//! - [`LineTool::undo`] / [`LineTool::close`].
//! - [`LineTool::commit_wire`] — append the polyline to a mesh as an edge
//!   wire.
//! - [`LineTool::auto_cut_chords`] + [`crate::knife::knife`] — cut faces the
//!   polyline crosses, for the tractable edge-to-edge-on-one-face case (the
//!   general surface-walking projection is deferred, as it is upstream in
//!   [`crate::knife`]).
//!
//! ## Units
//!
//! Positions and lengths are dimensionless model-space quantities; angles are
//! radians.

use crate::draw_tool::WorkPlane;
use crate::knife::{Chord, KnifePoint};
use crate::math::Vec3;
use crate::mesh::{EdgeId, FaceId, Mesh};
use crate::snap::{snap_point, SnapElement, SnapTarget};

/// One placed point of the polyline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinePoint {
    /// World position.
    pub position: Vec3,
    /// The mesh element it snapped to, if any.
    pub on: Option<SnapElement>,
}

/// The connected-polyline drawing tool.
#[derive(Debug, Clone, Default)]
pub struct LineTool {
    points: Vec<LinePoint>,
    closed: bool,
}

impl LineTool {
    /// A fresh, empty tool.
    pub fn new() -> Self {
        Self::default()
    }

    /// The points placed so far, in order.
    pub fn points(&self) -> &[LinePoint] {
        &self.points
    }

    /// Whether [`LineTool::close`] has joined the ends.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Append a raw world point (no snap).
    pub fn add_raw(&mut self, position: Vec3) {
        self.points.push(LinePoint { position, on: None });
    }

    /// Snap `cursor` to the nearest `target` of `mesh` within `max_dist`, and
    /// append the result (falls back to `cursor` unchanged if nothing is in
    /// range and `target` is not a grid snap).
    pub fn add_snapped(&mut self, mesh: &Mesh, cursor: Vec3, target: SnapTarget, max_dist: f64) {
        match snap_point(mesh, cursor, target, max_dist, &[]) {
            Some(hit) => self.points.push(LinePoint { position: hit.position, on: hit.element }),
            None => self.add_raw(cursor),
        }
    }

    /// Append a point at `length` from the previous point, in the direction of
    /// `angle` measured in `plane` from `plane.u` (CCW about `plane.normal`).
    /// No-op if there is no previous point.
    pub fn add_polar(&mut self, plane: &WorkPlane, length: f64, angle: f64) {
        let Some(prev) = self.points.last().map(|p| p.position) else { return };
        let dir = plane.u.scale(angle.cos()).add(plane.v.scale(angle.sin()));
        self.points.push(LinePoint { position: prev.add(dir.scale(length)), on: None });
    }

    /// Snap `cursor` as [`LineTool::add_snapped`] would, then optionally
    /// override the segment `length` and/or its `angle` (in `plane`), keeping
    /// the previous point as the anchor. With both overrides `None` this is
    /// exactly [`LineTool::add_snapped`].
    #[allow(clippy::too_many_arguments)]
    pub fn add_constrained(
        &mut self,
        mesh: &Mesh,
        cursor: Vec3,
        target: SnapTarget,
        max_dist: f64,
        plane: &WorkPlane,
        length: Option<f64>,
        angle: Option<f64>,
    ) {
        let snapped = snap_point(mesh, cursor, target, max_dist, &[])
            .map(|h| LinePoint { position: h.position, on: h.element })
            .unwrap_or(LinePoint { position: cursor, on: None });

        let Some(prev) = self.points.last().map(|p| p.position) else {
            self.points.push(snapped);
            return;
        };
        if length.is_none() && angle.is_none() {
            self.points.push(snapped);
            return;
        }
        let raw = snapped.position.sub(prev);
        let cur_len = raw.length().max(1e-12);
        let cur_ang = raw.dot(plane.v).atan2(raw.dot(plane.u));
        let l = length.unwrap_or(cur_len);
        let a = angle.unwrap_or(cur_ang);
        let dir = plane.u.scale(a.cos()).add(plane.v.scale(a.sin()));
        self.points.push(LinePoint { position: prev.add(dir.scale(l)), on: None });
    }

    /// Remove the last placed point.
    pub fn undo(&mut self) {
        self.points.pop();
        self.closed = false;
    }

    /// Join the last point back to the first (cyclic polyline).
    pub fn close(&mut self) {
        if self.points.len() >= 3 {
            self.closed = true;
        }
    }

    /// Segment count (`points - 1`, or `points` when closed).
    pub fn segment_count(&self) -> usize {
        match self.points.len() {
            0 | 1 => 0,
            n if self.closed => n,
            n => n - 1,
        }
    }

    /// Append the polyline to `base` as an edge wire (degenerate sliver
    /// triangles, the crate's polygon-soup idiom for edge-only geometry).
    pub fn commit_wire(&self, base: &Mesh) -> Mesh {
        if self.points.len() < 2 {
            return base.clone();
        }
        let mut positions = base.positions();
        let mut faces: Vec<Vec<usize>> =
            base.polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect();
        let start = positions.len();
        for p in &self.points {
            positions.push(p.position);
        }
        let n = self.points.len();
        let last = if self.closed { n } else { n - 1 };
        for i in 0..last {
            let a = start + i;
            let b = start + (i + 1) % n;
            faces.push(vec![a, b, a]);
        }
        Mesh::from_polygons(&positions, &faces)
    }

    /// Knife chords for segments whose *both* endpoints snapped onto edges of
    /// a single common face — the tractable slice of "auto-cut crossed faces".
    /// Feed the result to [`crate::knife::knife`].
    ///
    /// Segments drawn interior-to-interior of a face, or crossing several
    /// faces, are not returned (the surface-walking projection needed for the
    /// general case is deferred — see [`crate::knife`]).
    pub fn auto_cut_chords(&self, mesh: &Mesh) -> Vec<Chord> {
        let mut chords = Vec::new();
        let n = self.points.len();
        if n < 2 {
            return chords;
        }
        let last = if self.closed { n } else { n - 1 };
        for i in 0..last {
            let (a, b) = (self.points[i], self.points[(i + 1) % n]);
            let (SnapElement::Edge(ea), SnapElement::Edge(eb)) = (
                match a.on {
                    Some(e) => e,
                    None => continue,
                },
                match b.on {
                    Some(e) => e,
                    None => continue,
                },
            ) else {
                continue;
            };
            if ea == eb {
                continue;
            }
            let Some(face) = common_face(mesh, ea, eb) else { continue };
            chords.push(Chord {
                face,
                from: KnifePoint::EdgeSplit { edge: ea, t: edge_param(mesh, ea, a.position) },
                to: KnifePoint::EdgeSplit { edge: eb, t: edge_param(mesh, eb, b.position) },
            });
        }
        chords
    }
}

/// A face incident to both `ea` and `eb`, if one exists.
fn common_face(mesh: &Mesh, ea: EdgeId, eb: EdgeId) -> Option<FaceId> {
    let faces_of = |e: EdgeId| -> Vec<FaceId> {
        let ed = mesh.edge(e).unwrap();
        let (u, w) = (ed.verts[0], ed.verts[1]);
        (0..mesh.face_count())
            .map(FaceId)
            .filter(|&f| {
                let vs = mesh.face_vertices(f);
                vs.contains(&u) && vs.contains(&w)
            })
            .collect()
    };
    let fa = faces_of(ea);
    faces_of(eb).into_iter().find(|f| fa.contains(f))
}

/// Parameter `t in (0,1)` of the point on edge `e` closest to `p`.
fn edge_param(mesh: &Mesh, e: EdgeId, p: Vec3) -> f64 {
    let ed = mesh.edge(e).unwrap();
    let a = mesh.vertex(ed.verts[0]).unwrap().position;
    let b = mesh.vertex(ed.verts[1]).unwrap().position;
    let ab = b.sub(a);
    let len2 = ab.dot(ab).max(1e-12);
    (p.sub(a).dot(ab) / len2).clamp(1e-6, 1.0 - 1e-6)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn polyline_grows_and_undoes() {
        let mut t = LineTool::new();
        assert_eq!(t.segment_count(), 0);
        t.add_raw(Vec3::ZERO);
        t.add_raw(Vec3::new(1.0, 0.0, 0.0));
        t.add_raw(Vec3::new(1.0, 1.0, 0.0));
        assert_eq!(t.segment_count(), 2);
        t.undo();
        assert_eq!(t.points().len(), 2);
        t.add_raw(Vec3::new(0.0, 1.0, 0.0));
        t.add_raw(Vec3::new(0.0, 0.5, 0.0));
        t.close();
        assert!(t.is_closed());
        assert_eq!(t.segment_count(), 4);
    }

    #[test]
    fn add_snapped_locks_onto_a_cube_vertex() {
        let cube = primitives::cube(2.0);
        let mut t = LineTool::new();
        t.add_snapped(&cube, Vec3::new(0.9, 0.9, 1.05), SnapTarget::Vertex, 0.5);
        let p = t.points()[0];
        assert!(p.position.sub(Vec3::new(1.0, 1.0, 1.0)).length() < 1e-9);
        assert!(matches!(p.on, Some(SnapElement::Vertex(_))));
    }

    #[test]
    fn add_polar_places_by_length_and_angle() {
        let plane = WorkPlane::xy();
        let mut t = LineTool::new();
        t.add_raw(Vec3::ZERO);
        t.add_polar(&plane, 2.0, std::f64::consts::FRAC_PI_2); // straight up +y
        let p = t.points()[1].position;
        assert!(p.sub(Vec3::new(0.0, 2.0, 0.0)).length() < 1e-9);
    }

    #[test]
    fn add_constrained_overrides_length_only() {
        let plane = WorkPlane::xy();
        let cube = primitives::cube(2.0);
        let mut t = LineTool::new();
        t.add_raw(Vec3::ZERO);
        // Cursor points roughly along +x at distance ~5, but we pin length 3.
        t.add_constrained(
            &cube,
            Vec3::new(5.0, 0.2, 0.0),
            SnapTarget::Increment(100.0), // effectively no useful snap
            0.0,
            &plane,
            Some(3.0),
            None,
        );
        let seg = t.points()[1].position.sub(Vec3::ZERO);
        assert!((seg.length() - 3.0).abs() < 1e-9);
        assert!(seg.x > 0.0 && seg.y.abs() < seg.x); // direction preserved
    }

    #[test]
    fn commit_wire_appends_edges_to_the_base_mesh() {
        let base = primitives::cube(2.0);
        let mut t = LineTool::new();
        t.add_raw(Vec3::new(2.0, 0.0, 0.0));
        t.add_raw(Vec3::new(3.0, 0.0, 0.0));
        t.add_raw(Vec3::new(3.0, 1.0, 0.0));
        let out = t.commit_wire(&base);
        assert_eq!(out.vertex_count(), base.vertex_count() + 3);
        assert!(out.edge_count() > base.edge_count());
    }

    #[test]
    fn auto_cut_chords_on_a_single_grid_face() {
        // A 2x2 grid: the centre vertex is shared; pick a face and draw a
        // segment between two of its edges.
        let m = primitives::grid(2, 2, 4.0);
        let f = FaceId(0);
        let verts = m.face_vertices(f);
        assert_eq!(verts.len(), 4);
        // Two opposite edges of face 0.
        let e_of = |a, b| {
            (0..m.edge_count()).map(EdgeId).find(|&e| {
                let ed = m.edge(e).unwrap();
                (ed.verts[0] == a && ed.verts[1] == b) || (ed.verts[0] == b && ed.verts[1] == a)
            })
        };
        let e0 = e_of(verts[0], verts[1]).unwrap();
        let e1 = e_of(verts[2], verts[3]).unwrap();
        let p0 = midpoint(&m, e0);
        let p1 = midpoint(&m, e1);

        let mut t = LineTool::new();
        t.points.push(LinePoint { position: p0, on: Some(SnapElement::Edge(e0)) });
        t.points.push(LinePoint { position: p1, on: Some(SnapElement::Edge(e1)) });

        let chords = t.auto_cut_chords(&m);
        assert_eq!(chords.len(), 1);
        assert_eq!(chords[0].face, f);

        let res = crate::knife::knife(&m, &chords);
        assert!(res.mesh.face_count() > m.face_count(), "face 0 was split");
    }

    fn midpoint(m: &Mesh, e: EdgeId) -> Vec3 {
        let ed = m.edge(e).unwrap();
        m.vertex(ed.verts[0]).unwrap().position.add(m.vertex(ed.verts[1]).unwrap().position).scale(0.5)
    }
}
