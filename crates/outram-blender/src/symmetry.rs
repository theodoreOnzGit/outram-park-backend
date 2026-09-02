// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Mesh symmetry. Follows the published behaviour of Blender's symmetrize and
// snap-to-symmetry operators (source/blender/bmesh/operators/bmo_symmetrize.cc
// and editmesh_tools.cc `MESH_OT_symmetry_snap`, github.com/blender/blender,
// GPL-2.0-or-later): mirror one side of a mesh onto the other across an axis
// plane and weld the seam, or average each vertex with its mirror partner.
// Concepts only — no upstream source copied; polygon-soup rebuilds.
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

//! **Mesh symmetry** (`op-hzs.54.21`, GH issue #37 §C).
//!
//! - [`symmetrize`] — keep one half of the mesh (the `keep_positive` side of
//!   the [`Axis`] plane through the origin), mirror it onto the other half, and
//!   weld the seam. Blender's `Mesh ▸ Symmetrize`.
//! - [`snap_to_symmetry`] — move every vertex to the average of its own
//!   position and its mirror partner's, so the mesh becomes exactly symmetric
//!   without changing topology. Blender's `Mesh ▸ Snap to Symmetry`.
//! - [`mirror_selection`] — the mirror-image vertices of a selection (the
//!   position-matched partners), for live / topology mirror editing.

use std::collections::HashMap;

use crate::math::Vec3;
use crate::mesh::{Mesh, VertexId};
pub use crate::selection::Axis;

/// Symmetrize `mesh` across the [`Axis`] plane through the origin. Keeps the
/// side where `axis · p >= 0` when `keep_positive`, else the `<= 0` side;
/// mirrors it onto the other side and welds vertices within `merge_threshold`
/// of the plane (and of each other on the seam).
pub fn symmetrize(mesh: &Mesh, axis: Axis, keep_positive: bool, merge_threshold: f64) -> Mesh {
    let coord = |p: Vec3| match axis {
        Axis::X => p.x,
        Axis::Y => p.y,
        Axis::Z => p.z,
    };
    let sign = if keep_positive { 1.0 } else { -1.0 };
    let tol = merge_threshold.max(1e-9);

    let src = mesh.positions();
    let polys = mesh.polygons();

    // Keep vertices on the kept side or on the plane.
    let keep: Vec<bool> = src.iter().map(|&p| coord(p) * sign >= -tol).collect();

    // New vertex list: kept originals (snapped to the plane if within tol),
    // then their mirror images (skipping on-plane verts, which are shared).
    let mut positions: Vec<Vec3> = Vec::new();
    let mut kept_idx: HashMap<usize, usize> = HashMap::new();
    let mut mirror_idx: HashMap<usize, usize> = HashMap::new();
    for (i, &p) in src.iter().enumerate() {
        if !keep[i] {
            continue;
        }
        let on_plane = coord(p).abs() <= tol;
        let snapped = if on_plane { snap_axis(p, axis, 0.0) } else { p };
        positions.push(snapped);
        let ki = positions.len() - 1;
        kept_idx.insert(i, ki);
        if on_plane {
            mirror_idx.insert(i, ki);
        } else {
            positions.push(reflect(snapped, axis));
            mirror_idx.insert(i, positions.len() - 1);
        }
    }

    let mut faces: Vec<Vec<usize>> = Vec::new();
    for poly in polys {
        if !poly.iter().all(|v| keep[v.0]) {
            continue;
        }
        let kept: Vec<usize> = poly.iter().map(|v| kept_idx[&v.0]).collect();
        let mut mirrored: Vec<usize> = poly.iter().rev().map(|v| mirror_idx[&v.0]).collect();
        faces.push(kept);
        // Drop a mirrored face that collapsed onto the plane (all verts shared).
        mirrored.dedup();
        if mirrored.len() >= 3 {
            faces.push(mirrored);
        }
    }

    let built = Mesh::from_polygons(&positions, &faces);
    crate::weld::weld(&built, tol)
}

/// Make `mesh` exactly symmetric about the [`Axis`] plane without changing
/// topology: each vertex moves to the average of its position and the
/// position of the vertex nearest its mirror image (within `match_threshold`).
/// Unmatched vertices near the plane are snapped onto it.
pub fn snap_to_symmetry(mesh: &Mesh, axis: Axis, match_threshold: f64) -> Mesh {
    let src = mesh.positions();
    let lookup = PositionLookup::new(&src, match_threshold.max(1e-6));
    let coord = |p: Vec3| match axis {
        Axis::X => p.x,
        Axis::Y => p.y,
        Axis::Z => p.z,
    };
    let mut out = src.clone();
    for (i, &p) in src.iter().enumerate() {
        if coord(p).abs() <= match_threshold.max(1e-6) {
            out[i] = snap_axis(p, axis, 0.0);
            continue;
        }
        if let Some(j) = lookup.find(reflect(p, axis)) {
            // Average p with the reflection of its partner.
            let partner_reflected = reflect(src[j], axis);
            out[i] = p.add(partner_reflected).scale(0.5);
        }
    }
    Mesh::from_polygons(&out, &to_soup(mesh))
}

/// The mirror-image vertices of `verts` across the [`Axis`] plane, matched by
/// position within `tolerance`. Unmatched inputs are dropped.
pub fn mirror_selection(mesh: &Mesh, verts: &[VertexId], axis: Axis, tolerance: f64) -> Vec<VertexId> {
    let src = mesh.positions();
    let lookup = PositionLookup::new(&src, tolerance.max(1e-6));
    verts
        .iter()
        .filter_map(|&v| {
            let p = src.get(v.0)?;
            lookup.find(reflect(*p, axis)).map(VertexId)
        })
        .collect()
}

// --- helpers ---

fn reflect(p: Vec3, axis: Axis) -> Vec3 {
    match axis {
        Axis::X => Vec3::new(-p.x, p.y, p.z),
        Axis::Y => Vec3::new(p.x, -p.y, p.z),
        Axis::Z => Vec3::new(p.x, p.y, -p.z),
    }
}

fn snap_axis(p: Vec3, axis: Axis, value: f64) -> Vec3 {
    match axis {
        Axis::X => Vec3::new(value, p.y, p.z),
        Axis::Y => Vec3::new(p.x, value, p.z),
        Axis::Z => Vec3::new(p.x, p.y, value),
    }
}

fn to_soup(mesh: &Mesh) -> Vec<Vec<usize>> {
    mesh.polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect()
}

/// Grid-hash position → vertex-id lookup with a tolerance.
struct PositionLookup {
    cell: f64,
    grid: HashMap<[i64; 3], Vec<(Vec3, usize)>>,
}

impl PositionLookup {
    fn new(positions: &[Vec3], tol: f64) -> Self {
        let cell = tol.max(1e-9);
        let mut grid: HashMap<[i64; 3], Vec<(Vec3, usize)>> = HashMap::new();
        for (i, &p) in positions.iter().enumerate() {
            grid.entry(Self::key(p, cell)).or_default().push((p, i));
        }
        PositionLookup { cell, grid }
    }
    fn key(p: Vec3, cell: f64) -> [i64; 3] {
        [(p.x / cell).round() as i64, (p.y / cell).round() as i64, (p.z / cell).round() as i64]
    }
    fn find(&self, target: Vec3) -> Option<usize> {
        let k = Self::key(target, self.cell);
        let tol2 = self.cell * self.cell;
        let mut best: Option<(f64, usize)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    for &(p, id) in self
                        .grid
                        .get(&[k[0] + dx, k[1] + dy, k[2] + dz])
                        .map(|v| v.as_slice())
                        .unwrap_or(&[])
                    {
                        let d2 = p.sub(target).dot(p.sub(target));
                        if d2 <= tol2 && best.is_none_or(|(bd, _)| d2 < bd) {
                            best = Some((d2, id));
                        }
                    }
                }
            }
        }
        best.map(|(_, id)| id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn symmetrize_a_half_open_box_into_a_closed_one() {
        // Take a cube, delete the +X half's faces, then symmetrize +X onto -X.
        let cube = primitives::cube(2.0);
        let px_faces: Vec<crate::mesh::FaceId> = (0..cube.face_count())
            .map(crate::mesh::FaceId)
            .filter(|&f| cube.face_centroid(f).x > 0.1)
            .collect();
        let half = crate::dissolve::delete(&cube, crate::dissolve::DeleteMode::Faces, &[], &[], &px_faces);
        assert!(half.face_count() < 6);
        let sym = symmetrize(&half, Axis::X, false, 1e-4); // keep the -X side, mirror onto +X
        assert_eq!(sym.euler_characteristic(), 2, "symmetric result is closed again");
    }

    #[test]
    fn snap_to_symmetry_perfects_a_wobbly_pair() {
        let mut m = Mesh::new();
        let a = m.add_vertex(Vec3::new(-1.0, 0.0, 0.0));
        let b = m.add_vertex(Vec3::new(1.05, 0.1, 0.0)); // should be (1, 0, 0)
        let c = m.add_vertex(Vec3::new(0.0, 2.0, 0.0));
        m.add_face(&[a, b, c]);
        let s = snap_to_symmetry(&m, Axis::X, 0.3);
        let pa = s.vertex(a).unwrap().position;
        let pb = s.vertex(b).unwrap().position;
        assert!((pa.x + pb.x).abs() < 1e-9, "x-coords now mirror");
        assert!((pa.y - pb.y).abs() < 1e-9, "y-coords now equal");
    }

    #[test]
    fn mirror_selection_finds_partners_on_a_cube() {
        let m = primitives::cube(2.0);
        let neg_x: Vec<VertexId> = (0..m.vertex_count())
            .map(VertexId)
            .filter(|&v| m.vertex(v).unwrap().position.x < 0.0)
            .collect();
        let partners = mirror_selection(&m, &neg_x, Axis::X, 1e-6);
        assert_eq!(partners.len(), 4);
        for p in partners {
            assert!(m.vertex(p).unwrap().position.x > 0.0);
        }
    }
}
