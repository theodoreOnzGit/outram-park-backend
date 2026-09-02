// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Measurement and inspection. Follows the published behaviour of Blender's
// mesh-statistics overlays, the Measure tool (source/blender/editors/gizmo_
// library/gizmo_types/, view3d_gizmo_ruler.cc) and the Mesh Analysis overlay
// (draw_mesh_analysis in the overlay engine), github.com/blender/blender,
// GPL-2.0-or-later: edge length / face area / angle readouts, ruler + protractor
// measurements, model dimensions, and manufacturability checks (overhang,
// distortion, sharpness, self-intersection). Concepts only — no upstream source
// copied.
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

//! **Measurement & inspection** (`op-hzs.54.26`, GH issue #37 §D). Depends on
//! [`crate::snap`] for the closest-point helpers.
//!
//! - Per-element readouts: [`edge_length`], [`face_area`], [`face_perimeter`],
//!   [`dihedral_angle`], [`corner_angle`].
//! - Whole-mesh: [`total_edge_length`], [`total_surface_area`],
//!   [`signed_volume`], [`bounding_box`], [`dimensions`].
//! - Tools: [`Ruler`] (distance), [`Protractor`] (angle).
//! - Mesh analysis: [`overhang`], [`distortion`], [`sharp_edges`],
//!   [`self_intersections`], [`thickness`].

use crate::math::Vec3;
use crate::mesh::{EdgeId, FaceId, Mesh, VertexId};
use crate::topology::MeshTopology;

/// Length of an edge (`0.0` if out of range).
pub fn edge_length(mesh: &Mesh, e: EdgeId) -> f64 {
    match mesh.edge(e) {
        Some(ed) => {
            let (a, b) = (mesh.vertex(ed.verts[0]), mesh.vertex(ed.verts[1]));
            match (a, b) {
                (Some(a), Some(b)) => b.position.sub(a.position).length(),
                _ => 0.0,
            }
        }
        None => 0.0,
    }
}

/// Area of a face by Newell's method (robust for non-planar / concave faces).
pub fn face_area(mesh: &Mesh, f: FaceId) -> f64 {
    let vs = mesh.face_vertices(f);
    if vs.len() < 3 {
        return 0.0;
    }
    let mut n = Vec3::ZERO;
    let p: Vec<Vec3> = vs.iter().map(|v| mesh.vertex(*v).map(|x| x.position).unwrap_or(Vec3::ZERO)).collect();
    for i in 0..p.len() {
        let a = p[i];
        let b = p[(i + 1) % p.len()];
        n = n.add(a.cross(b));
    }
    n.length() * 0.5
}

/// Perimeter of a face (sum of its edge lengths).
pub fn face_perimeter(mesh: &Mesh, f: FaceId) -> f64 {
    let vs = mesh.face_vertices(f);
    let n = vs.len();
    (0..n)
        .map(|i| {
            let a = mesh.vertex(vs[i]).map(|x| x.position).unwrap_or(Vec3::ZERO);
            let b = mesh.vertex(vs[(i + 1) % n]).map(|x| x.position).unwrap_or(Vec3::ZERO);
            b.sub(a).length()
        })
        .sum()
}

/// Angle between the two faces on an edge, in radians (`0` = flat, `π` =
/// folded flat back). `None` if the edge does not have exactly two faces.
pub fn dihedral_angle(mesh: &Mesh, e: EdgeId) -> Option<f64> {
    let topo = MeshTopology::new(mesh);
    let f = topo.edge_faces(e);
    if f.len() != 2 {
        return None;
    }
    let n0 = mesh.face_normal(f[0]);
    let n1 = mesh.face_normal(f[1]);
    Some(n0.dot(n1).clamp(-1.0, 1.0).acos())
}

/// Interior angle of face `f` at corner `v`, in radians.
pub fn corner_angle(mesh: &Mesh, f: FaceId, v: VertexId) -> Option<f64> {
    let vs = mesh.face_vertices(f);
    let i = vs.iter().position(|&x| x == v)?;
    let n = vs.len();
    let o = mesh.vertex(vs[i])?.position;
    let a = mesh.vertex(vs[(i + n - 1) % n])?.position.sub(o);
    let b = mesh.vertex(vs[(i + 1) % n])?.position.sub(o);
    Some((a.dot(b) / (a.length() * b.length() + 1e-12)).clamp(-1.0, 1.0).acos())
}

/// Sum of all edge lengths.
pub fn total_edge_length(mesh: &Mesh) -> f64 {
    (0..mesh.edge_count()).map(|e| edge_length(mesh, EdgeId(e))).sum()
}

/// Sum of all face areas.
pub fn total_surface_area(mesh: &Mesh) -> f64 {
    (0..mesh.face_count()).map(|f| face_area(mesh, FaceId(f))).sum()
}

/// Signed volume of the mesh via the divergence theorem (`Σ (a · (b × c)) / 6`
/// over a fan triangulation of each face). Meaningful for a **closed**,
/// consistently-wound surface; positive for outward-facing winding.
pub fn signed_volume(mesh: &Mesh) -> f64 {
    let mut v = 0.0;
    for f in 0..mesh.face_count() {
        let vs = mesh.face_vertices(FaceId(f));
        if vs.len() < 3 {
            continue;
        }
        let a = mesh.vertex(vs[0]).map(|x| x.position).unwrap_or(Vec3::ZERO);
        for i in 1..vs.len() - 1 {
            let b = mesh.vertex(vs[i]).map(|x| x.position).unwrap_or(Vec3::ZERO);
            let c = mesh.vertex(vs[i + 1]).map(|x| x.position).unwrap_or(Vec3::ZERO);
            v += a.dot(b.cross(c));
        }
    }
    v / 6.0
}

/// Axis-aligned bounding box `(min, max)`.
pub fn bounding_box(mesh: &Mesh) -> (Vec3, Vec3) {
    let mut lo = Vec3::new(f64::MAX, f64::MAX, f64::MAX);
    let mut hi = Vec3::new(f64::MIN, f64::MIN, f64::MIN);
    for p in mesh.positions() {
        lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    (lo, hi)
}

/// The model's dimensions (bounding-box extent) — Blender's N-panel *Dimensions*.
pub fn dimensions(mesh: &Mesh) -> Vec3 {
    let (lo, hi) = bounding_box(mesh);
    hi.sub(lo)
}

/// A two-point distance measurement (Blender's Ruler).
#[derive(Debug, Clone, Copy)]
pub struct Ruler {
    pub a: Vec3,
    pub b: Vec3,
}

impl Ruler {
    /// The measured distance.
    pub fn distance(&self) -> f64 {
        self.b.sub(self.a).length()
    }
}

/// A three-point angle measurement — the angle at `vertex` (Blender's
/// Protractor).
#[derive(Debug, Clone, Copy)]
pub struct Protractor {
    pub a: Vec3,
    pub vertex: Vec3,
    pub b: Vec3,
}

impl Protractor {
    /// The measured angle at `vertex`, in radians.
    pub fn angle(&self) -> f64 {
        let u = self.a.sub(self.vertex);
        let v = self.b.sub(self.vertex);
        (u.dot(v) / (u.length() * v.length() + 1e-12)).clamp(-1.0, 1.0).acos()
    }
}

/// Overhang: the angle of face `f`'s normal from `up`, in radians. `0` = the
/// face points straight up; `π` = straight down. Used to flag unsupported
/// overhangs for additive manufacturing.
pub fn overhang(mesh: &Mesh, f: FaceId, up: Vec3) -> f64 {
    let n = mesh.face_normal(f);
    let u = if up.length() > 1e-9 { up.normalize() } else { Vec3::new(0.0, 0.0, 1.0) };
    n.dot(u).clamp(-1.0, 1.0).acos()
}

/// Distortion: how far face `f` deviates from planar, as the maximum angle
/// (radians) between its per-triangle normals over a fan triangulation. `0` for
/// a triangle or a perfectly planar polygon.
pub fn distortion(mesh: &Mesh, f: FaceId) -> f64 {
    let vs = mesh.face_vertices(f);
    if vs.len() < 4 {
        return 0.0;
    }
    let p: Vec<Vec3> = vs.iter().map(|v| mesh.vertex(*v).map(|x| x.position).unwrap_or(Vec3::ZERO)).collect();
    let mut norms: Vec<Vec3> = Vec::new();
    for i in 1..p.len() - 1 {
        let n = p[i].sub(p[0]).cross(p[i + 1].sub(p[0]));
        if n.length() > 1e-12 {
            norms.push(n.normalize());
        }
    }
    let mut worst = 0.0;
    for i in 0..norms.len() {
        for j in i + 1..norms.len() {
            worst = f64::max(worst, norms[i].dot(norms[j]).clamp(-1.0, 1.0).acos());
        }
    }
    worst
}

/// Every edge whose dihedral angle exceeds `angle` radians — Blender's Mesh
/// Analysis "sharp" and the seed set for Mark Sharp.
pub fn sharp_edges(mesh: &Mesh, angle: f64) -> Vec<EdgeId> {
    (0..mesh.edge_count())
        .map(EdgeId)
        .filter(|&e| dihedral_angle(mesh, e).is_some_and(|d| d > angle))
        .collect()
}

/// Pairs of faces whose triangulations intersect. `O(F²)` broad phase on
/// bounding boxes then a triangle-triangle test — fine for interactive meshes,
/// not a spatial-hash implementation.
pub fn self_intersections(mesh: &Mesh) -> Vec<(FaceId, FaceId)> {
    let tris = triangulated(mesh);
    let face_verts: Vec<std::collections::BTreeSet<usize>> = (0..mesh.face_count())
        .map(|f| mesh.face_vertices(FaceId(f)).into_iter().map(|v| v.0).collect())
        .collect();
    let mut out = Vec::new();
    for i in 0..tris.len() {
        for j in i + 1..tris.len() {
            let (fi, fj) = (tris[i].2, tris[j].2);
            if fi == fj {
                continue; // same source face
            }
            // Adjacent faces (sharing a vertex) touch legitimately.
            if !face_verts[fi].is_disjoint(&face_verts[fj]) {
                continue;
            }
            if !aabb_overlap(&tris[i].1 .0, &tris[i].1 .1, &tris[j].1 .0, &tris[j].1 .1) {
                continue;
            }
            if tri_tri_intersect(&tris[i].0, &tris[j].0) {
                let pair = (tris[i].2.min(tris[j].2), tris[i].2.max(tris[j].2));
                if !out.contains(&(FaceId(pair.0), FaceId(pair.1))) {
                    out.push((FaceId(pair.0), FaceId(pair.1)));
                }
            }
        }
    }
    out
}

/// Local wall thickness at face `f`: cast a ray from its centroid along `-normal`
/// and return the distance to the first other face it hits, or `None`.
pub fn thickness(mesh: &Mesh, f: FaceId) -> Option<f64> {
    let origin = mesh.face_centroid(f);
    let dir = mesh.face_normal(f).scale(-1.0);
    if dir.length() < 1e-9 {
        return None;
    }
    let tris = triangulated(mesh);
    let mut best = f64::MAX;
    for (t, _, src) in &tris {
        if *src == f.0 {
            continue;
        }
        if let Some(dist) = ray_tri(origin.add(dir.scale(1e-6)), dir, t) {
            if dist < best {
                best = dist;
            }
        }
    }
    (best.is_finite()).then_some(best + 1e-6)
}

// --- helpers ---

type Tri = [Vec3; 3];

fn triangulated(mesh: &Mesh) -> Vec<(Tri, (Vec3, Vec3), usize)> {
    let mut out = Vec::new();
    for f in 0..mesh.face_count() {
        let vs = mesh.face_vertices(FaceId(f));
        let p: Vec<Vec3> = vs.iter().map(|v| mesh.vertex(*v).map(|x| x.position).unwrap_or(Vec3::ZERO)).collect();
        for i in 1..p.len().saturating_sub(1) {
            let t: Tri = [p[0], p[i], p[i + 1]];
            let lo = Vec3::new(
                t[0].x.min(t[1].x).min(t[2].x),
                t[0].y.min(t[1].y).min(t[2].y),
                t[0].z.min(t[1].z).min(t[2].z),
            );
            let hi = Vec3::new(
                t[0].x.max(t[1].x).max(t[2].x),
                t[0].y.max(t[1].y).max(t[2].y),
                t[0].z.max(t[1].z).max(t[2].z),
            );
            out.push((t, (lo, hi), f));
        }
    }
    out
}

fn aabb_overlap(lo0: &Vec3, hi0: &Vec3, lo1: &Vec3, hi1: &Vec3) -> bool {
    lo0.x <= hi1.x && hi0.x >= lo1.x && lo0.y <= hi1.y && hi0.y >= lo1.y && lo0.z <= hi1.z && hi0.z >= lo1.z
}

/// Möller triangle-triangle intersection (interval-overlap form), tolerant of
/// coplanar cases (which it reports as intersecting).
fn tri_tri_intersect(a: &Tri, b: &Tri) -> bool {
    let n1 = a[1].sub(a[0]).cross(a[2].sub(a[0]));
    let d1 = -n1.dot(a[0]);
    let db = [n1.dot(b[0]) + d1, n1.dot(b[1]) + d1, n1.dot(b[2]) + d1];
    if db[0].signum() == db[1].signum() && db[1].signum() == db[2].signum() && db[0].abs() > 1e-9 {
        return false;
    }
    let n2 = b[1].sub(b[0]).cross(b[2].sub(b[0]));
    let d2 = -n2.dot(b[0]);
    let da = [n2.dot(a[0]) + d2, n2.dot(a[1]) + d2, n2.dot(a[2]) + d2];
    if da[0].signum() == da[1].signum() && da[1].signum() == da[2].signum() && da[0].abs() > 1e-9 {
        return false;
    }
    // Coplanar or both straddle: fall back to an edge-vs-triangle test.
    for e in [(a[0], a[1]), (a[1], a[2]), (a[2], a[0])] {
        if ray_tri(e.0, e.1.sub(e.0), b).is_some_and(|t| (0.0..=1.0).contains(&t)) {
            return true;
        }
    }
    for e in [(b[0], b[1]), (b[1], b[2]), (b[2], b[0])] {
        if ray_tri(e.0, e.1.sub(e.0), a).is_some_and(|t| (0.0..=1.0).contains(&t)) {
            return true;
        }
    }
    false
}

/// Möller-Trumbore ray-triangle. Returns the ray parameter `t >= 0` at the hit.
fn ray_tri(origin: Vec3, dir: Vec3, tri: &Tri) -> Option<f64> {
    let e1 = tri[1].sub(tri[0]);
    let e2 = tri[2].sub(tri[0]);
    let pv = dir.cross(e2);
    let det = e1.dot(pv);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let tv = origin.sub(tri[0]);
    let u = tv.dot(pv) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let qv = tv.cross(e1);
    let v = dir.dot(qv) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(qv) * inv;
    (t >= 0.0).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn cube_measurements() {
        let m = primitives::cube(2.0); // side 2
        assert!((edge_length(&m, EdgeId(0)) - 2.0).abs() < 1e-9);
        assert!((total_surface_area(&m) - 24.0).abs() < 1e-9); // 6 * 4
        assert!((total_edge_length(&m) - 24.0).abs() < 1e-9); // 12 * 2
        assert!((signed_volume(&m).abs() - 8.0).abs() < 1e-6); // 2^3
        assert_eq!(dimensions(&m), Vec3::new(2.0, 2.0, 2.0));
    }

    #[test]
    fn dihedral_and_corner_angles_of_a_cube() {
        let m = primitives::cube(2.0);
        let d = dihedral_angle(&m, EdgeId(0)).unwrap();
        assert!((d - std::f64::consts::FRAC_PI_2).abs() < 1e-9, "cube edges are 90°");
        let f = FaceId(0);
        let v = m.face_vertices(f)[0];
        assert!((corner_angle(&m, f, v).unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn ruler_and_protractor() {
        let r = Ruler { a: Vec3::ZERO, b: Vec3::new(3.0, 4.0, 0.0) };
        assert!((r.distance() - 5.0).abs() < 1e-9);
        let p = Protractor {
            a: Vec3::new(1.0, 0.0, 0.0),
            vertex: Vec3::ZERO,
            b: Vec3::new(0.0, 1.0, 0.0),
        };
        assert!((p.angle() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn overhang_and_sharp_edges() {
        let m = primitives::cube(2.0);
        // The +Z face points up → overhang ≈ 0; the -Z face → ≈ π.
        let top = (0..m.face_count()).map(FaceId).find(|&f| m.face_centroid(f).z > 0.5).unwrap();
        let bot = (0..m.face_count()).map(FaceId).find(|&f| m.face_centroid(f).z < -0.5).unwrap();
        assert!(overhang(&m, top, Vec3::new(0.0, 0.0, 1.0)) < 0.01);
        assert!((overhang(&m, bot, Vec3::new(0.0, 0.0, 1.0)) - std::f64::consts::PI).abs() < 0.01);
        assert_eq!(sharp_edges(&m, std::f64::consts::FRAC_PI_4).len(), 12);
    }

    #[test]
    fn distortion_zero_for_a_planar_quad_positive_for_a_bent_one() {
        let flat = primitives::grid(1, 1, 2.0);
        assert!(distortion(&flat, FaceId(0)) < 1e-9);

        let mut m = Mesh::new();
        let a = m.add_vertex(Vec3::new(0.0, 0.0, 0.0));
        let b = m.add_vertex(Vec3::new(1.0, 0.0, 0.0));
        let c = m.add_vertex(Vec3::new(1.0, 1.0, 0.5)); // lifted
        let d = m.add_vertex(Vec3::new(0.0, 1.0, 0.0));
        m.add_face(&[a, b, c, d]);
        assert!(distortion(&m, FaceId(0)) > 0.1);
    }

    #[test]
    fn self_intersection_detects_two_crossing_quads() {
        let mut m = Mesh::new();
        // Quad in the XY plane.
        let a = m.add_vertex(Vec3::new(-1.0, -1.0, 0.0));
        let b = m.add_vertex(Vec3::new(1.0, -1.0, 0.0));
        let c = m.add_vertex(Vec3::new(1.0, 1.0, 0.0));
        let d = m.add_vertex(Vec3::new(-1.0, 1.0, 0.0));
        m.add_face(&[a, b, c, d]);
        // Quad in the XZ plane crossing through it.
        let e = m.add_vertex(Vec3::new(-1.0, 0.0, -1.0));
        let f = m.add_vertex(Vec3::new(1.0, 0.0, -1.0));
        let g = m.add_vertex(Vec3::new(1.0, 0.0, 1.0));
        let h = m.add_vertex(Vec3::new(-1.0, 0.0, 1.0));
        m.add_face(&[e, f, g, h]);
        assert_eq!(self_intersections(&m).len(), 1);

        // A clean cube has none.
        assert!(self_intersections(&primitives::cube(2.0)).is_empty());
    }

    #[test]
    fn thickness_of_a_thin_slab() {
        // Two parallel quads 0.3 apart, joined — a slab. Thickness ≈ 0.3.
        let mut m = Mesh::new();
        let top = [
            m.add_vertex(Vec3::new(-1.0, -1.0, 0.15)),
            m.add_vertex(Vec3::new(1.0, -1.0, 0.15)),
            m.add_vertex(Vec3::new(1.0, 1.0, 0.15)),
            m.add_vertex(Vec3::new(-1.0, 1.0, 0.15)),
        ];
        let bot = [
            m.add_vertex(Vec3::new(-1.0, -1.0, -0.15)),
            m.add_vertex(Vec3::new(1.0, -1.0, -0.15)),
            m.add_vertex(Vec3::new(1.0, 1.0, -0.15)),
            m.add_vertex(Vec3::new(-1.0, 1.0, -0.15)),
        ];
        m.add_face(&[top[0], top[1], top[2], top[3]]); // normal +z
        m.add_face(&[bot[3], bot[2], bot[1], bot[0]]); // normal -z
        let t = thickness(&m, FaceId(0)).unwrap();
        assert!((t - 0.3).abs() < 0.05, "got {t}");
    }
}
