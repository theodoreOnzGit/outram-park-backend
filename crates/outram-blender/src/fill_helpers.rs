// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Smaller fill / mirror helper operators. Blender analogue (architecture only):
// the bundled `mesh_f2` (context-aware F fill), `object_auto_mirror` (bisect +
// mirror in one step) and `mesh_bsurfaces` (surface from stroke input)
// add-ons. No upstream source copied; each is reimplemented from its
// documented behaviour, reusing this crate's bisect / weld.
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

//! **Fill / mirror helpers** (`op-hzs.54.46`, GH issue #37 §I).
//!
//! - [`f2_fill`] — the F2 "smart F": from one boundary edge, close the corner
//!   with a quad when the two neighbouring boundary edges allow it, else a
//!   triangle.
//! - [`auto_mirror`] — bisect the mesh by a plane, keep one half, mirror it
//!   back and weld along the cut (Auto Mirror in one call).
//! - [`bsurfaces`] — a lofted quad surface through a set of ordered strokes
//!   (Bsurfaces from annotation strokes).
//!
//! ## Units
//!
//! Positions are dimensionless model-space quantities (see [`crate::math`]).

use crate::draw_tool::WorkPlane;
use crate::math::Vec3;
use crate::mesh::{EdgeId, Mesh, VertexId};
use crate::topology::MeshTopology;

/// Context-aware fill from a single boundary edge (F2's smart `F`).
///
/// `edge` must be a boundary edge (used by exactly one face). The two boundary
/// edges sharing its endpoints are followed to their far vertices `c` (past
/// the `verts[0]` end) and `d` (past the `verts[1]` end):
///
/// - `c == d` → a triangle `(a, b, c)` is added;
/// - otherwise → a quad `(c, a, b, d)` is added.
///
/// Returns the mesh unchanged if `edge` is not a boundary edge or the
/// neighbours cannot be found.
pub fn f2_fill(mesh: &Mesh, edge: EdgeId) -> Mesh {
    let topo = MeshTopology::new(mesh);
    if !topo.is_boundary_edge(edge) {
        return mesh.clone();
    }
    let ed = mesh.edge(edge).unwrap();
    let (a, b) = (ed.verts[0], ed.verts[1]);

    let far = |v: VertexId| -> Option<VertexId> {
        for &e in topo.vertex_edges(v) {
            if e == edge || !topo.is_boundary_edge(e) {
                continue;
            }
            return topo.other_end(mesh, e, v);
        }
        None
    };
    let (Some(c), Some(d)) = (far(a), far(b)) else {
        return mesh.clone();
    };

    let (positions, mut faces) = soup(mesh);
    if c == d {
        faces.push(vec![a.0, b.0, c.0]);
    } else {
        faces.push(vec![c.0, a.0, b.0, d.0]);
    }
    Mesh::from_polygons(&positions, &faces)
}

/// Bisect `mesh` by `plane` (keeping the half on the `−normal` side, matching
/// [`crate::bisect::bisect`]), mirror that half across the plane, and weld the
/// two halves along the cut with tolerance `weld_dist`.
///
/// The result is symmetric about `plane`. If the mesh lies entirely on one
/// side, the kept half is just mirrored and welded (a doubled shell).
pub fn auto_mirror(mesh: &Mesh, plane: &WorkPlane, weld_dist: f64) -> Mesh {
    let half = crate::bisect::bisect(mesh, plane.origin, plane.normal);
    let (base_pos, base_faces) = soup(&half);
    if base_pos.is_empty() {
        return half;
    }
    let n = base_pos.len();

    let mut positions = base_pos.clone();
    for p in &base_pos {
        let dist = p.sub(plane.origin).dot(plane.normal);
        positions.push(p.sub(plane.normal.scale(2.0 * dist)));
    }
    let mut faces = base_faces.clone();
    for f in &base_faces {
        // Reversed winding for the mirrored copy so normals stay outward.
        let mut mirrored: Vec<usize> = f.iter().rev().map(|&v| v + n).collect();
        mirrored.rotate_right(1);
        faces.push(mirrored);
    }
    crate::weld::weld(&Mesh::from_polygons(&positions, &faces), weld_dist.max(1e-9))
}

/// A lofted quad surface through `strokes` (each an ordered polyline). Every
/// stroke is resampled to `cols` points; consecutive strokes are bridged into
/// a `(strokes.len()-1) x (cols-1)` quad grid.
///
/// `cols` is clamped `>= 2`; strokes with fewer than 2 points are skipped.
/// Needs at least two usable strokes, else an empty mesh.
pub fn bsurfaces(strokes: &[Vec<Vec3>], cols: usize) -> Mesh {
    let c = cols.max(2);
    let rows: Vec<Vec<Vec3>> = strokes
        .iter()
        .filter(|s| s.len() >= 2)
        .map(|s| resample(s, c))
        .collect();
    if rows.len() < 2 {
        return Mesh::new();
    }
    let mut positions: Vec<Vec3> = Vec::with_capacity(rows.len() * c);
    for row in &rows {
        positions.extend_from_slice(row);
    }
    let at = |r: usize, k: usize| r * c + k;
    let mut faces: Vec<Vec<usize>> = Vec::new();
    for r in 0..rows.len() - 1 {
        for k in 0..c - 1 {
            faces.push(vec![at(r, k), at(r, k + 1), at(r + 1, k + 1), at(r + 1, k)]);
        }
    }
    Mesh::from_polygons(&positions, &faces)
}

// --- helpers ---

fn soup(mesh: &Mesh) -> (Vec<Vec3>, Vec<Vec<usize>>) {
    (
        mesh.positions(),
        mesh.polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect(),
    )
}

/// Resample a polyline to exactly `n` points at equal arc length.
fn resample(pts: &[Vec3], n: usize) -> Vec<Vec3> {
    let mut acc = vec![0.0_f64];
    for w in pts.windows(2) {
        acc.push(acc[acc.len() - 1] + w[1].sub(w[0]).length());
    }
    let total = *acc.last().unwrap();
    if total < 1e-12 {
        return vec![pts[0]; n];
    }
    (0..n)
        .map(|i| {
            let target = total * i as f64 / (n as f64 - 1.0);
            let mut seg = 0;
            while seg + 1 < acc.len() - 1 && acc[seg + 1] < target {
                seg += 1;
            }
            let seg_len = (acc[seg + 1] - acc[seg]).max(1e-12);
            let t = ((target - acc[seg]) / seg_len).clamp(0.0, 1.0);
            pts[seg].add(pts[seg + 1].sub(pts[seg]).scale(t))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn f2_fill_closes_a_corner_with_a_quad() {
        // An L of three edges: verts 0-1-2-3, boundary edges (0,1),(1,2),(2,3).
        let positions = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ];
        // One face to make (0,1),(1,2),(2,3),(3,0) exist; delete (3,0) by using
        // a triangle strip instead: faces (0,1,2) leaves (2,0) internal... so
        // build the open strip as two sliver faces to seed edges.
        let m = Mesh::from_polygons(
            &positions,
            &[vec![0, 1, 2], vec![0, 2, 3]], // a quad split — edge (0,2) internal
        );
        // (1,2) is a boundary edge of this quad; its neighbours are (0,1) and (2,3).
        let e12 = (0..m.edge_count())
            .map(EdgeId)
            .find(|&e| {
                let ed = m.edge(e).unwrap();
                let s: std::collections::BTreeSet<usize> = [ed.verts[0].0, ed.verts[1].0].into();
                s == [1usize, 2].into()
            })
            .unwrap();
        let out = f2_fill(&m, e12);
        assert_eq!(out.face_count(), m.face_count() + 1, "one fill face added");
    }

    #[test]
    fn f2_fill_ignores_a_non_boundary_edge() {
        let cube = primitives::cube(2.0);
        // Every cube edge is manifold (two faces).
        let out = f2_fill(&cube, EdgeId(0));
        assert_eq!(out.face_count(), cube.face_count());
    }

    #[test]
    fn auto_mirror_produces_a_symmetric_mesh() {
        // Half a cube: bisect a cube at x=0 keeping x<=0, then auto-mirror.
        let cube = primitives::cube(2.0);
        let plane = WorkPlane::yz(); // normal +x, keeps x <= 0
        let out = auto_mirror(&cube, &plane, 1e-6);
        let (lo, hi) = crate::measure::bounding_box(&out);
        assert!((hi.x + lo.x).abs() < 1e-6, "symmetric about x = 0");
        assert!((hi.x - 1.0).abs() < 1e-6 && (lo.x + 1.0).abs() < 1e-6, "full width restored");
    }

    #[test]
    fn bsurfaces_lofts_strokes_into_a_grid() {
        let strokes = vec![
            vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0)],
            vec![Vec3::new(0.0, 1.0, 0.3), Vec3::new(2.0, 1.0, 0.3)],
            vec![Vec3::new(0.0, 2.0, 0.0), Vec3::new(1.0, 2.0, -0.2), Vec3::new(2.0, 2.0, 0.0)],
        ];
        let m = bsurfaces(&strokes, 5);
        assert_eq!(m.vertex_count(), 3 * 5);
        assert_eq!(m.face_count(), 2 * 4, "(rows-1) x (cols-1) quads");
        assert_eq!(m.euler_characteristic(), 1, "open surface patch (disc)");
    }

    #[test]
    fn bsurfaces_needs_two_strokes() {
        let one = vec![vec![Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0)]];
        assert_eq!(bsurfaces(&one, 4).face_count(), 0);
    }
}
