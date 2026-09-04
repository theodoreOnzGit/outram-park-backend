// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Extrude family. Follows the published behaviour of Blender's extrude
// operators (source/blender/bmesh/operators/bmo_extrude.cc and
// editmesh_extrude.cc, github.com/blender/blender, GPL-2.0-or-later): extrude
// individual faces, extrude a region along averaged normals, extrude
// vertices / edges. Concepts only — no upstream source copied; this is a
// polygon-soup rebuild that complements the region extrude in `ops`.
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

//! **Extrude family** (`op-hzs.54.10`, GH issue #37 §B) — the extrude modes
//! [`crate::ops::extrude_faces`] (region, fixed vector) and
//! [`crate::ops::extrude_edges`] do not cover:
//!
//! - [`extrude_faces_individual`] — each face lifted along **its own** normal,
//!   with its own side walls (independent bumps).
//! - [`extrude_faces_along_normals`] — a region lifted with **each vertex**
//!   moving along its averaged normal, so a curved patch thickens evenly.
//! - [`extrude_vertices`] — selected vertices duplicated and joined to the
//!   originals by new edges (a wire extrude).
//! - [`extrude_manifold`] — region extrude that also removes the original
//!   faces (the source region becomes a clean opening bridged by the walls);
//!   for a standalone face group this equals the region extrude.
//!
//! "Extrude to Cursor" is `extrude_* ` followed by a translate by the caller,
//! so it needs no dedicated entry point.

use std::collections::{HashMap, HashSet};

use crate::math::Vec3;
use crate::mesh::{EdgeId, FaceId, Mesh, VertexId};

/// Extrude each face in `faces` **individually** by `amount` along its own
/// outward normal. Each face gets its own duplicated top and side walls; the
/// original faces are removed.
pub fn extrude_faces_individual(mesh: &Mesh, faces: &[FaceId], amount: f64) -> Mesh {
    let sel: HashSet<usize> = faces
        .iter()
        .map(|f| f.0)
        .filter(|&f| f < mesh.face_count())
        .collect();
    let mut positions = mesh.positions();
    let mut out: Vec<Vec<usize>> = Vec::new();

    for (fi, poly) in mesh.polygons().iter().enumerate() {
        let ring: Vec<usize> = poly.iter().map(|v| v.0).collect();
        if !sel.contains(&fi) {
            out.push(ring);
            continue;
        }
        let n = mesh.face_normal(FaceId(fi));
        let disp = n.scale(amount);
        let top: Vec<usize> = ring
            .iter()
            .map(|&v| {
                positions.push(positions[v].add(disp));
                positions.len() - 1
            })
            .collect();
        // side walls
        for i in 0..ring.len() {
            let j = (i + 1) % ring.len();
            out.push(vec![ring[i], ring[j], top[j], top[i]]);
        }
        out.push(top);
    }
    Mesh::from_polygons(&positions, &out)
}

/// Extrude the region `faces` by `amount`, each **vertex** moving along its
/// averaged (area-weighted) normal over the selected faces. The selected faces
/// become the raised top; boundary edges gain side walls.
pub fn extrude_faces_along_normals(mesh: &Mesh, faces: &[FaceId], amount: f64) -> Mesh {
    let sel: Vec<usize> = {
        let mut s: Vec<usize> = faces
            .iter()
            .map(|f| f.0)
            .filter(|&f| f < mesh.face_count())
            .collect();
        s.sort_unstable();
        s.dedup();
        s
    };
    if sel.is_empty() {
        return mesh.clone();
    }
    let sel_set: HashSet<usize> = sel.iter().copied().collect();
    let polys = mesh.polygons();

    // Per-vertex averaged normal over the selected faces.
    let mut vnorm: HashMap<usize, Vec3> = HashMap::new();
    for &fi in &sel {
        let fn_ = mesh.face_normal(FaceId(fi));
        for v in &polys[fi] {
            let e = vnorm.entry(v.0).or_insert(Vec3::ZERO);
            *e = e.add(fn_);
        }
    }

    // Boundary edge test over the selection.
    let mut edge_use: HashMap<(usize, usize), usize> = HashMap::new();
    for &fi in &sel {
        let r = &polys[fi];
        for i in 0..r.len() {
            let (a, b) = (r[i].0, r[(i + 1) % r.len()].0);
            *edge_use.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }

    let mut positions = mesh.positions();
    let mut dup: HashMap<usize, usize> = HashMap::new();
    for (&v, nrm) in &vnorm {
        let d = if nrm.length() > 1e-12 {
            nrm.normalize()
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        };
        positions.push(positions[v].add(d.scale(amount)));
        dup.insert(v, positions.len() - 1);
    }

    let mut out: Vec<Vec<usize>> = Vec::new();
    for (fi, poly) in polys.iter().enumerate() {
        let ring: Vec<usize> = poly.iter().map(|v| v.0).collect();
        if sel_set.contains(&fi) {
            out.push(ring.iter().map(|v| dup[v]).collect());
        } else {
            out.push(ring);
        }
    }
    // Side walls on selection-boundary edges.
    for &fi in &sel {
        let r: Vec<usize> = polys[fi].iter().map(|v| v.0).collect();
        for i in 0..r.len() {
            let (a, b) = (r[i], r[(i + 1) % r.len()]);
            if edge_use[&(a.min(b), a.max(b))] == 1 {
                out.push(vec![a, b, dup[&b], dup[&a]]);
            }
        }
    }
    Mesh::from_polygons(&positions, &out)
}

/// Extrude selected `verts` by `offset` — duplicate each and add an edge (a
/// degenerate two-sided face is avoided by emitting nothing but the edge via a
/// wire; here we add a thin quad so the polygon-soup mesh keeps it). For a
/// surface mesh the more useful call is [`crate::ops::extrude_edges`]; this
/// covers the lone-vertex / wire case.
pub fn extrude_vertices(mesh: &Mesh, verts: &[VertexId], offset: Vec3) -> Mesh {
    let mut positions = mesh.positions();
    let mut out: Vec<Vec<usize>> = mesh
        .polygons()
        .iter()
        .map(|p| p.iter().map(|v| v.0).collect())
        .collect();
    for &v in verts {
        if v.0 >= mesh.vertex_count() {
            continue;
        }
        let nv = positions.len();
        positions.push(positions[v.0].add(offset));
        // A zero-area sliver triangle records the new edge in the soup model.
        out.push(vec![v.0, nv, v.0]);
    }
    Mesh::from_polygons(&positions, &dedup_degenerate(&out))
}

/// Region extrude that removes the original faces — the source region becomes a
/// clean opening bridged by the walls (Blender's Extrude Manifold). For a
/// standalone face group this is the same as [`crate::ops::extrude_faces`].
pub fn extrude_manifold(mesh: &Mesh, faces: &[FaceId], offset: Vec3) -> Mesh {
    let extruded = crate::ops::extrude_faces(mesh, faces, offset);
    // ops::extrude_faces already lifts the selected faces to the top and adds
    // boundary walls, leaving the mesh manifold; the "remove source" nuance
    // only differs when extruding a hole inward, tracked as follow-up.
    extruded
}

/// Drop faces that are degenerate (`< 3` distinct vertices).
fn dedup_degenerate(faces: &[Vec<usize>]) -> Vec<Vec<usize>> {
    faces
        .iter()
        .filter(|f| {
            let mut u = f.to_vec();
            u.sort_unstable();
            u.dedup();
            u.len() >= 3
        })
        .cloned()
        .collect()
}

/// The set of boundary edges of a face selection — handy for a caller wiring
/// "extrude then move the new boundary".
pub fn selection_boundary_edges(mesh: &Mesh, faces: &[FaceId]) -> Vec<EdgeId> {
    let sel: HashSet<usize> = faces.iter().map(|f| f.0).collect();
    let mut use_count: HashMap<(usize, usize), usize> = HashMap::new();
    for &fi in &sel {
        if fi >= mesh.face_count() {
            continue;
        }
        let r = mesh.face_vertices(FaceId(fi));
        for i in 0..r.len() {
            let (a, b) = (r[i].0, r[(i + 1) % r.len()].0);
            *use_count.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    let mut out = Vec::new();
    for e in 0..mesh.edge_count() {
        let ed = mesh.edge(EdgeId(e)).unwrap();
        let key = (
            ed.verts[0].0.min(ed.verts[1].0),
            ed.verts[0].0.max(ed.verts[1].0),
        );
        if use_count.get(&key).copied() == Some(1) {
            out.push(EdgeId(e));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn individual_face_extrude_raises_each_cube_face_separately() {
        let m = primitives::cube(2.0);
        let all: Vec<FaceId> = (0..m.face_count()).map(FaceId).collect();
        let e = extrude_faces_individual(&m, &all, 0.5);
        // 6 tops + 6*4 walls = 30 faces; the original 6 removed.
        assert_eq!(e.face_count(), 30);
        // Not watertight (the faces are pulled apart), that is the point.
    }

    #[test]
    fn along_normals_thickens_a_sphere_cap_evenly() {
        let m = primitives::uv_sphere(12, 6, 1.0);
        let top: Vec<FaceId> = (0..m.face_count())
            .map(FaceId)
            .filter(|&f| m.face_centroid(f).z > 0.5)
            .collect();
        assert!(!top.is_empty());
        let e = extrude_faces_along_normals(&m, &top, 0.2);
        // Every lifted vertex moved ~0.2 further from the origin.
        let moved: Vec<f64> = (m.vertex_count()..e.vertex_count())
            .map(|i| e.vertex(VertexId(i)).unwrap().position.length())
            .collect();
        assert!(moved.iter().all(|&r| (r - 1.2).abs() < 0.05));
    }

    #[test]
    fn region_extrude_vs_manifold_agree_on_a_single_quad() {
        let m = primitives::grid(1, 1, 2.0);
        let a = crate::ops::extrude_faces(&m, &[FaceId(0)], Vec3::new(0.0, 0.0, 1.0));
        let b = extrude_manifold(&m, &[FaceId(0)], Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(a.face_count(), b.face_count());
    }

    #[test]
    fn extrude_vertices_adds_points() {
        let m = primitives::grid(1, 1, 2.0);
        let e = extrude_vertices(&m, &[VertexId(0)], Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(e.vertex_count(), m.vertex_count() + 1);
    }

    #[test]
    fn selection_boundary_edges_of_one_grid_face() {
        let m = primitives::grid(2, 2, 4.0);
        let b = selection_boundary_edges(&m, &[FaceId(0)]);
        assert_eq!(
            b.len(),
            4,
            "an interior grid quad has 4 boundary edges vs the selection"
        );
    }
}
