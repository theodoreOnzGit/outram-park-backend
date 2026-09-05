// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Normals toolset. Follows the published behaviour of Blender's normals menu
// (source/blender/editors/mesh/editmesh_tools.cc `MESH_OT_normals_*` and the
// custom-split-normal layer in mesh_normals.cc, github.com/blender/blender,
// GPL-2.0-or-later): flip, recalculate inside / outside, per-vertex normals by
// weighting, point-to-target, angle-based split normals, harden normals.
// Concepts only — no upstream source copied. Extends `recalc_normals`.
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

//! **Normals toolset** (`op-hzs.54.27`, GH issue #37 §E).
//!
//! - [`flip_faces`] — reverse the winding of selected faces.
//! - [`recalculate`] — [`crate::recalc_normals`] plus an *inside* option.
//! - [`vertex_normals`] — per-vertex normals by [`NormalWeight`]
//!   (uniform / face-area / corner-angle).
//! - [`point_normals_to_target`] — per-vertex normals aimed toward / away from
//!   a point.
//! - [`SplitNormals`] — a per-face-corner normal layer;
//!   [`split_normals_by_angle`] auto-smooths below an angle,
//!   [`harden_normals`] makes the selected faces contribute flat.

use std::collections::BTreeSet;

use crate::math::Vec3;
use crate::mesh::{FaceId, Mesh, VertexId};
use crate::topology::MeshTopology;

/// How incident face normals are weighted into a vertex normal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalWeight {
    /// Every incident face counts equally.
    Uniform,
    /// Weight by face area (Blender's "Face Area").
    FaceArea,
    /// Weight by the face's interior angle at that vertex (Blender's "Corner
    /// Angle" — the default).
    CornerAngle,
}

/// Reverse the winding (hence normal) of the faces in `faces` (empty = all).
pub fn flip_faces(mesh: &Mesh, faces: &[FaceId]) -> Mesh {
    let sel: BTreeSet<usize> = if faces.is_empty() {
        (0..mesh.face_count()).collect()
    } else {
        faces.iter().map(|f| f.0).collect()
    };
    let out: Vec<Vec<usize>> = mesh
        .polygons()
        .iter()
        .enumerate()
        .map(|(fi, poly)| {
            let mut v: Vec<usize> = poly.iter().map(|x| x.0).collect();
            if sel.contains(&fi) {
                v.reverse();
            }
            v
        })
        .collect();
    Mesh::from_polygons(&mesh.positions(), &out)
}

/// Make the whole mesh's winding consistent and point it `outside` (or inside).
pub fn recalculate(mesh: &Mesh, outside: bool) -> Mesh {
    let out = crate::recalc_normals::recalculate_normals(mesh);
    if outside {
        out
    } else {
        flip_faces(&out, &[])
    }
}

/// Per-vertex normals for `mesh`, one entry per [`VertexId`], weighted per
/// `weight`. A vertex on no face gets [`Vec3::ZERO`].
pub fn vertex_normals(mesh: &Mesh, weight: NormalWeight) -> Vec<Vec3> {
    let topo = MeshTopology::new(mesh);
    let mut out = vec![Vec3::ZERO; mesh.vertex_count()];
    for (v, slot) in out.iter_mut().enumerate() {
        let mut acc = Vec3::ZERO;
        for &f in topo.vertex_faces(VertexId(v)) {
            let n = mesh.face_normal(f);
            let w = match weight {
                NormalWeight::Uniform => 1.0,
                NormalWeight::FaceArea => crate::measure::face_area(mesh, f),
                NormalWeight::CornerAngle => {
                    crate::measure::corner_angle(mesh, f, VertexId(v)).unwrap_or(0.0)
                }
            };
            acc = acc.add(n.scale(w));
        }
        *slot = if acc.length() > 1e-12 {
            acc.normalize()
        } else {
            Vec3::ZERO
        };
    }
    out
}

/// Per-vertex normals aimed at `target` (`invert` flips them to aim away).
/// Blender's *Point to Target*.
pub fn point_normals_to_target(mesh: &Mesh, target: Vec3, invert: bool) -> Vec<Vec3> {
    mesh.positions()
        .iter()
        .map(|&p| {
            let d = target.sub(p);
            let n = if d.length() > 1e-12 {
                d.normalize()
            } else {
                Vec3::new(0.0, 0.0, 1.0)
            };
            if invert {
                n.scale(-1.0)
            } else {
                n
            }
        })
        .collect()
}

/// A per-face-corner normal layer (Blender's custom split normals). `normals[f]`
/// has one entry per corner of face `f`, in its vertex order.
#[derive(Debug, Clone, Default)]
pub struct SplitNormals {
    pub normals: Vec<Vec<Vec3>>,
}

impl SplitNormals {
    /// Every corner normal equal to its face's flat normal.
    pub fn flat(mesh: &Mesh) -> Self {
        SplitNormals {
            normals: (0..mesh.face_count())
                .map(|f| {
                    let n = mesh.face_normal(FaceId(f));
                    vec![n; mesh.face_vertices(FaceId(f)).len()]
                })
                .collect(),
        }
    }
}

/// Auto-smooth split normals: a corner's normal is the average of the incident
/// face normals whose angle to this face's normal is `<= angle` radians; a
/// steeper neighbour is excluded (a hard edge). `angle = 0` gives flat shading,
/// `angle = π` gives fully smooth.
pub fn split_normals_by_angle(mesh: &Mesh, angle: f64) -> SplitNormals {
    let topo = MeshTopology::new(mesh);
    let cos_tol = angle.cos();
    let mut normals: Vec<Vec<Vec3>> = Vec::with_capacity(mesh.face_count());
    for f in 0..mesh.face_count() {
        let fn_ = mesh.face_normal(FaceId(f));
        let vs = mesh.face_vertices(FaceId(f));
        let mut corner = Vec::with_capacity(vs.len());
        for &v in &vs {
            let mut acc = fn_;
            for &g in topo.vertex_faces(v) {
                if g.0 == f {
                    continue;
                }
                let gn = mesh.face_normal(g);
                if fn_.dot(gn) >= cos_tol {
                    acc = acc.add(gn);
                }
            }
            corner.push(if acc.length() > 1e-12 {
                acc.normalize()
            } else {
                fn_
            });
        }
        normals.push(corner);
    }
    SplitNormals { normals }
}

/// Harden normals: start from [`split_normals_by_angle`], then force every
/// corner of a face in `faces` to that face's flat normal (so the selection
/// reads as a crisp, faceted region regardless of its neighbours). Blender's
/// *Harden Normals*.
pub fn harden_normals(mesh: &Mesh, faces: &[FaceId], angle: f64) -> SplitNormals {
    let mut sn = split_normals_by_angle(mesh, angle);
    let sel: BTreeSet<usize> = faces.iter().map(|f| f.0).collect();
    for &fi in &sel {
        if let Some(corner) = sn.normals.get_mut(fi) {
            let n = mesh.face_normal(FaceId(fi));
            for c in corner.iter_mut() {
                *c = n;
            }
        }
    }
    sn
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn flip_faces_reverses_winding() {
        let m = primitives::grid(1, 1, 2.0); // normal +z
        let before = m.face_normal(FaceId(0));
        let f = flip_faces(&m, &[FaceId(0)]);
        let after = f.face_normal(FaceId(0));
        assert!(before.add(after).length() < 1e-9, "normal reversed");
    }

    #[test]
    fn recalculate_inside_points_normals_in() {
        let m = primitives::cube(2.0);
        let inside = recalculate(&m, false);
        // A face's normal should now point toward the origin.
        let f = FaceId(0);
        let c = inside.face_centroid(f);
        let n = inside.face_normal(f);
        assert!(n.dot(c) < 0.0, "normal points inward");
    }

    #[test]
    fn vertex_normals_of_a_cube_point_out_of_the_corner() {
        let m = primitives::cube(2.0);
        let ns = vertex_normals(&m, NormalWeight::CornerAngle);
        for i in 0..8 {
            let p = m.vertex(VertexId(i)).unwrap().position.normalize();
            // Corner normal should roughly align with the corner direction.
            assert!(ns[i].dot(p) > 0.5, "vertex {i} normal points outward");
        }
    }

    #[test]
    fn weighting_modes_all_produce_unit_normals() {
        let m = primitives::uv_sphere(12, 8, 1.0);
        for w in [
            NormalWeight::Uniform,
            NormalWeight::FaceArea,
            NormalWeight::CornerAngle,
        ] {
            for n in vertex_normals(&m, w) {
                if n.length() > 1e-9 {
                    assert!((n.length() - 1.0).abs() < 1e-9);
                }
            }
        }
    }

    #[test]
    fn point_to_target_aims_at_the_point() {
        let m = primitives::grid(2, 2, 4.0);
        let ns = point_normals_to_target(&m, Vec3::new(0.0, 0.0, 10.0), false);
        for n in &ns {
            assert!(n.z > 0.0, "all normals aim up toward the target");
        }
        let away = point_normals_to_target(&m, Vec3::new(0.0, 0.0, 10.0), true);
        assert!(away.iter().all(|n| n.z < 0.0));
    }

    #[test]
    fn split_normals_flat_vs_smooth_on_a_cube() {
        let m = primitives::cube(2.0);
        // angle 0 → flat (corner normal == face normal).
        let flat = split_normals_by_angle(&m, 0.0);
        for f in 0..m.face_count() {
            let n = m.face_normal(FaceId(f));
            for &c in &flat.normals[f] {
                assert!(c.sub(n).length() < 1e-9);
            }
        }
        // angle π → smooth (corner normals blend the 3 faces at each corner).
        let smooth = split_normals_by_angle(&m, std::f64::consts::PI);
        assert!(smooth.normals[0][0].sub(m.face_normal(FaceId(0))).length() > 0.1);
    }

    #[test]
    fn harden_normals_flattens_the_selection() {
        let m = primitives::cube(2.0);
        let sn = harden_normals(&m, &[FaceId(0)], std::f64::consts::PI);
        let n = m.face_normal(FaceId(0));
        for &c in &sn.normals[0] {
            assert!(c.sub(n).length() < 1e-9, "hardened face is flat");
        }
        // Another face stays smooth.
        assert!(sn.normals[1][0].sub(m.face_normal(FaceId(1))).length() > 0.1);
    }
}
