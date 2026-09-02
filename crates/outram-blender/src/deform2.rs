// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Deform modifiers (part 2). Follows the published behaviour of Blender's
// deform modifiers (source/blender/modifiers/intern/MOD_curve.cc,
// MOD_lattice.cc, MOD_hook.cc, MOD_shrinkwrap.cc, MOD_surfacedeform.cc,
// MOD_meshdeform.cc, MOD_laplaciandeform.cc, github.com/blender/blender,
// GPL-2.0-or-later): deform a mesh along a curve, through a lattice, by a hook
// point, onto a target surface, or bound to a target surface. Concepts only —
// no upstream source copied. Laplacian Deform forwards to `arap`.
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

//! **Deform modifiers pt.2** (`op-hzs.54.32`, GH issue #37 §F).
//!
//! - [`curve_deform`] — bend the mesh so its `axis` coordinate follows the
//!   arc-length of a polyline curve, riding the curve's local frame.
//! - [`Lattice`] + [`lattice_deform`] — trilinear free-form deformation from a
//!   3-D control-point grid (Blender's Lattice modifier).
//! - [`hook`] — a hook point drags a vertex set, with a smooth falloff.
//! - [`shrinkwrap`] — project vertices onto a target mesh
//!   ([`ShrinkMode::NearestSurfacePoint`] / [`ShrinkMode::ProjectAlongNormal`] /
//!   [`ShrinkMode::NearestVertex`]).
//! - [`SurfaceBind`] + [`surface_deform`] — bind vertices to a target mesh's
//!   triangles (barycentric) once, then follow the deformed target.
//! - [`laplacian_deform`] — anchored deformation; forwards to
//!   [`crate::arap::arap_deform`].

use crate::math::Vec3;
use crate::mesh::{FaceId, Mesh, VertexId};
use crate::selection::Axis;

fn axis_coord(p: Vec3, a: Axis) -> f64 {
    match a {
        Axis::X => p.x,
        Axis::Y => p.y,
        Axis::Z => p.z,
    }
}
fn axis_unit(a: Axis) -> Vec3 {
    match a {
        Axis::X => Vec3::new(1.0, 0.0, 0.0),
        Axis::Y => Vec3::new(0.0, 1.0, 0.0),
        Axis::Z => Vec3::new(0.0, 0.0, 1.0),
    }
}
fn to_soup(m: &Mesh) -> Vec<Vec<usize>> {
    m.polygons().iter().map(|p| p.iter().map(|v| v.0).collect()).collect()
}

/// Deform `mesh` so its `axis` coordinate rides `curve` (a polyline, `>= 2`
/// points). A vertex at axis-coordinate `c` is placed at arc-length
/// `c - axis_min` along the curve, offset by its perpendicular components in
/// the curve's local frame (tangent + a stable up).
pub fn curve_deform(mesh: &Mesh, curve: &[Vec3], axis: Axis) -> Mesh {
    if curve.len() < 2 {
        return mesh.clone();
    }
    // Cumulative arc length.
    let mut seg_len = vec![0.0];
    for w in curve.windows(2) {
        seg_len.push(seg_len.last().unwrap() + w[1].sub(w[0]).length());
    }
    let total = *seg_len.last().unwrap();
    if total < 1e-9 {
        return mesh.clone();
    }
    let k = axis_unit(axis);
    let lo = mesh.positions().iter().map(|&p| axis_coord(p, axis)).fold(f64::MAX, f64::min);

    let sample = |s: f64| -> (Vec3, Vec3) {
        let s = s.clamp(0.0, total);
        let i = seg_len.iter().rposition(|&l| l <= s).unwrap_or(0).min(curve.len() - 2);
        let seg = seg_len[i + 1] - seg_len[i];
        let t = if seg > 1e-9 { (s - seg_len[i]) / seg } else { 0.0 };
        let pos = curve[i].add(curve[i + 1].sub(curve[i]).scale(t));
        let tan = curve[i + 1].sub(curve[i]).normalize();
        (pos, tan)
    };

    let positions: Vec<Vec3> = mesh
        .positions()
        .iter()
        .map(|&p| {
            let c = axis_coord(p, axis);
            let along = k.scale(c);
            let perp = p.sub(along);
            let (base, tan) = sample(c - lo);
            let up = if tan.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
            let bx = up.cross(tan).normalize();
            let by = tan.cross(bx);
            base.add(bx.scale(perp.dot(perp_axis_x(axis)))).add(by.scale(perp.dot(perp_axis_y(axis))))
        })
        .collect();
    Mesh::from_polygons(&positions, &to_soup(mesh))
}

fn perp_axis_x(a: Axis) -> Vec3 {
    match a {
        Axis::X => Vec3::new(0.0, 1.0, 0.0),
        Axis::Y => Vec3::new(1.0, 0.0, 0.0),
        Axis::Z => Vec3::new(1.0, 0.0, 0.0),
    }
}
fn perp_axis_y(a: Axis) -> Vec3 {
    match a {
        Axis::X => Vec3::new(0.0, 0.0, 1.0),
        Axis::Y => Vec3::new(0.0, 0.0, 1.0),
        Axis::Z => Vec3::new(0.0, 1.0, 0.0),
    }
}

/// A 3-D grid of control points for [`lattice_deform`].
#[derive(Debug, Clone)]
pub struct Lattice {
    /// Grid resolution `[nx, ny, nz]` (each `>= 2`).
    pub dims: [usize; 3],
    /// The undeformed grid's corner and its opposite corner.
    pub rest_min: Vec3,
    pub rest_max: Vec3,
    /// Control-point positions, `points[x + nx*(y + ny*z)]`.
    pub points: Vec<Vec3>,
}

impl Lattice {
    /// A lattice matching a mesh's bounding box, control points at their rest
    /// (undeformed) positions — deform it by moving [`Lattice::points`].
    pub fn from_bounds(dims: [usize; 3], min: Vec3, max: Vec3) -> Self {
        let d = [dims[0].max(2), dims[1].max(2), dims[2].max(2)];
        let mut points = Vec::with_capacity(d[0] * d[1] * d[2]);
        for z in 0..d[2] {
            for y in 0..d[1] {
                for x in 0..d[0] {
                    points.push(Vec3::new(
                        lerp(min.x, max.x, x as f64 / (d[0] - 1) as f64),
                        lerp(min.y, max.y, y as f64 / (d[1] - 1) as f64),
                        lerp(min.z, max.z, z as f64 / (d[2] - 1) as f64),
                    ));
                }
            }
        }
        Lattice { dims: d, rest_min: min, rest_max: max, points }
    }

    fn at(&self, x: usize, y: usize, z: usize) -> Vec3 {
        self.points[x + self.dims[0] * (y + self.dims[1] * z)]
    }
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Trilinear free-form deformation: each mesh vertex's normalised position in
/// the lattice's rest box picks a trilinear blend of the (possibly moved)
/// control points.
pub fn lattice_deform(mesh: &Mesh, lat: &Lattice) -> Mesh {
    let ext = lat.rest_max.sub(lat.rest_min);
    let positions: Vec<Vec3> = mesh
        .positions()
        .iter()
        .map(|&p| {
            let u = ((p.x - lat.rest_min.x) / ext.x.max(1e-9)).clamp(0.0, 1.0) * (lat.dims[0] - 1) as f64;
            let v = ((p.y - lat.rest_min.y) / ext.y.max(1e-9)).clamp(0.0, 1.0) * (lat.dims[1] - 1) as f64;
            let w = ((p.z - lat.rest_min.z) / ext.z.max(1e-9)).clamp(0.0, 1.0) * (lat.dims[2] - 1) as f64;
            let (x0, y0, z0) = (u.floor() as usize, v.floor() as usize, w.floor() as usize);
            let (x1, y1, z1) = (
                (x0 + 1).min(lat.dims[0] - 1),
                (y0 + 1).min(lat.dims[1] - 1),
                (z0 + 1).min(lat.dims[2] - 1),
            );
            let (fx, fy, fz) = (u - x0 as f64, v - y0 as f64, w - z0 as f64);
            let c000 = lat.at(x0, y0, z0);
            let c100 = lat.at(x1, y0, z0);
            let c010 = lat.at(x0, y1, z0);
            let c110 = lat.at(x1, y1, z0);
            let c001 = lat.at(x0, y0, z1);
            let c101 = lat.at(x1, y0, z1);
            let c011 = lat.at(x0, y1, z1);
            let c111 = lat.at(x1, y1, z1);
            let lx = |a: Vec3, b: Vec3| a.add(b.sub(a).scale(fx));
            let e00 = lx(c000, c100);
            let e10 = lx(c010, c110);
            let e01 = lx(c001, c101);
            let e11 = lx(c011, c111);
            let f0 = e00.add(e10.sub(e00).scale(fy));
            let f1 = e01.add(e11.sub(e01).scale(fy));
            f0.add(f1.sub(f0).scale(fz))
        })
        .collect();
    Mesh::from_polygons(&positions, &to_soup(mesh))
}

/// A hook: drag `verts` by `to - from`, weighted by a smooth falloff from
/// `from` out to `falloff_radius` (`0` = rigid within the whole selection).
pub fn hook(mesh: &Mesh, verts: &[VertexId], from: Vec3, to: Vec3, falloff_radius: f64) -> Mesh {
    let delta = to.sub(from);
    let sel: std::collections::BTreeSet<usize> = verts.iter().map(|v| v.0).collect();
    let positions: Vec<Vec3> = mesh
        .positions()
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            if !sel.contains(&i) {
                return p;
            }
            let w = if falloff_radius <= 0.0 {
                1.0
            } else {
                let d = p.sub(from).length();
                (1.0 - d / falloff_radius).clamp(0.0, 1.0)
            };
            p.add(delta.scale(smoothstep(w)))
        })
        .collect();
    Mesh::from_polygons(&positions, &to_soup(mesh))
}

/// How [`shrinkwrap`] snaps a vertex onto the target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShrinkMode {
    /// The closest point anywhere on the target surface.
    NearestSurfacePoint,
    /// The first target hit along `+normal` then `-normal` from the vertex.
    ProjectAlongNormal,
    /// The closest target vertex.
    NearestVertex,
}

/// Move each vertex of `mesh` onto `target` per `mode`, blended by `factor`.
pub fn shrinkwrap(mesh: &Mesh, target: &Mesh, mode: ShrinkMode, factor: f64) -> Mesh {
    let tpos = target.positions();
    let tris: Vec<[Vec3; 3]> = {
        let mut v = Vec::new();
        for f in 0..target.face_count() {
            let vs = target.face_vertices(FaceId(f));
            let p: Vec<Vec3> = vs.iter().map(|x| tpos[x.0]).collect();
            for i in 1..p.len().saturating_sub(1) {
                v.push([p[0], p[i], p[i + 1]]);
            }
        }
        v
    };
    let vnorm = crate::normals::vertex_normals(mesh, crate::normals::NormalWeight::CornerAngle);

    let positions: Vec<Vec3> = mesh
        .positions()
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            let onto = match mode {
                ShrinkMode::NearestVertex => {
                    tpos.iter().copied().min_by(|&a, &b| {
                        a.sub(p).length().partial_cmp(&b.sub(p).length()).unwrap()
                    }).unwrap_or(p)
                }
                ShrinkMode::NearestSurfacePoint => nearest_on_tris(p, &tris).unwrap_or(p),
                ShrinkMode::ProjectAlongNormal => {
                    let n = vnorm.get(i).copied().unwrap_or(Vec3::new(0.0, 0.0, 1.0));
                    project_both_ways(p, n, &tris).unwrap_or_else(|| nearest_on_tris(p, &tris).unwrap_or(p))
                }
            };
            p.add(onto.sub(p).scale(factor.clamp(0.0, 1.0)))
        })
        .collect();
    Mesh::from_polygons(&positions, &to_soup(mesh))
}

/// A binding of a mesh's vertices to a target surface's triangles.
#[derive(Debug, Clone)]
pub struct SurfaceBind {
    /// Per source vertex: `(tri index, barycentric (u, v, w), signed offset
    /// along the tri normal)`.
    binds: Vec<(usize, [f64; 3], f64)>,
}

/// Bind `mesh`'s vertices to `target`'s triangles at the current pose.
pub fn bind_to_surface(mesh: &Mesh, target: &Mesh) -> SurfaceBind {
    let tpos = target.positions();
    let mut tris: Vec<[Vec3; 3]> = Vec::new();
    for f in 0..target.face_count() {
        let vs = target.face_vertices(FaceId(f));
        let p: Vec<Vec3> = vs.iter().map(|x| tpos[x.0]).collect();
        for i in 1..p.len().saturating_sub(1) {
            tris.push([p[0], p[i], p[i + 1]]);
        }
    }
    let binds = mesh
        .positions()
        .iter()
        .map(|&p| {
            let mut best = (0usize, [1.0, 0.0, 0.0], 0.0, f64::MAX);
            for (ti, t) in tris.iter().enumerate() {
                let (bary, foot, n) = closest_bary(p, t);
                let d = foot.sub(p).length();
                if d < best.3 {
                    best = (ti, bary, n.dot(p.sub(foot)), d);
                }
            }
            (best.0, best.1, best.2)
        })
        .collect();
    SurfaceBind { binds }
}

/// Re-evaluate a [`SurfaceBind`] against a deformed `target` (same topology),
/// producing the corresponding deformed source mesh.
pub fn surface_deform(mesh: &Mesh, bind: &SurfaceBind, deformed_target: &Mesh) -> Mesh {
    let tpos = deformed_target.positions();
    let mut tris: Vec<[Vec3; 3]> = Vec::new();
    for f in 0..deformed_target.face_count() {
        let vs = deformed_target.face_vertices(FaceId(f));
        let p: Vec<Vec3> = vs.iter().map(|x| tpos[x.0]).collect();
        for i in 1..p.len().saturating_sub(1) {
            tris.push([p[0], p[i], p[i + 1]]);
        }
    }
    let positions: Vec<Vec3> = bind
        .binds
        .iter()
        .map(|&(ti, b, off)| {
            let Some(t) = tris.get(ti) else { return Vec3::ZERO };
            let foot = t[0].scale(b[0]).add(t[1].scale(b[1])).add(t[2].scale(b[2]));
            let n = t[1].sub(t[0]).cross(t[2].sub(t[0]));
            let n = if n.length() > 1e-12 { n.normalize() } else { Vec3::new(0.0, 0.0, 1.0) };
            foot.add(n.scale(off))
        })
        .collect();
    Mesh::from_polygons(&positions, &to_soup(mesh))
}

/// Anchored (Laplacian) deformation — forwards to [`crate::arap::arap_deform`].
pub fn laplacian_deform(
    mesh: &Mesh,
    handles: &[(VertexId, Vec3)],
    iterations: u32,
) -> Result<Mesh, crate::arap::ArapError> {
    crate::arap::arap_deform(mesh, handles, iterations)
}

// --- helpers ---

fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn nearest_on_tris(p: Vec3, tris: &[[Vec3; 3]]) -> Option<Vec3> {
    let mut best: Option<(f64, Vec3)> = None;
    for t in tris {
        let (_, foot, _) = closest_bary(p, t);
        let d = foot.sub(p).length();
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, foot));
        }
    }
    best.map(|(_, f)| f)
}

fn project_both_ways(p: Vec3, n: Vec3, tris: &[[Vec3; 3]]) -> Option<Vec3> {
    let mut best: Option<(f64, Vec3)> = None;
    for &dir in &[n, n.scale(-1.0)] {
        for t in tris {
            if let Some(dist) = ray_tri(p, dir, t) {
                let hit = p.add(dir.scale(dist));
                if best.is_none_or(|(bd, _)| dist < bd) {
                    best = Some((dist, hit));
                }
            }
        }
    }
    best.map(|(_, h)| h)
}

/// Closest point on a triangle to `p`: barycentric weights, the foot point, and
/// the triangle's unit normal.
fn closest_bary(p: Vec3, t: &[Vec3; 3]) -> ([f64; 3], Vec3, Vec3) {
    let n = t[1].sub(t[0]).cross(t[2].sub(t[0]));
    let nl = n.length();
    if nl < 1e-12 {
        return ([1.0, 0.0, 0.0], t[0], Vec3::new(0.0, 0.0, 1.0));
    }
    let nn = n.scale(1.0 / nl);
    let proj = p.sub(nn.scale(p.sub(t[0]).dot(nn)));
    // Barycentric of `proj`.
    let v0 = t[1].sub(t[0]);
    let v1 = t[2].sub(t[0]);
    let v2 = proj.sub(t[0]);
    let d00 = v0.dot(v0);
    let d01 = v0.dot(v1);
    let d11 = v1.dot(v1);
    let d20 = v2.dot(v0);
    let d21 = v2.dot(v1);
    let den = d00 * d11 - d01 * d01;
    let (mut v, mut w) = if den.abs() > 1e-18 {
        ((d11 * d20 - d01 * d21) / den, (d00 * d21 - d01 * d20) / den)
    } else {
        (0.0, 0.0)
    };
    // Clamp into the triangle.
    v = v.max(0.0);
    w = w.max(0.0);
    if v + w > 1.0 {
        let s = v + w;
        v /= s;
        w /= s;
    }
    let u = 1.0 - v - w;
    let foot = t[0].scale(u).add(t[1].scale(v)).add(t[2].scale(w));
    ([u, v, w], foot, nn)
}

fn ray_tri(origin: Vec3, dir: Vec3, tri: &[Vec3; 3]) -> Option<f64> {
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
    fn curve_deform_bends_a_bar_along_an_l() {
        // A bar along Z, z ∈ [0, 4]. Curve goes up then turns +X.
        let mut m = Mesh::new();
        for i in 0..9 {
            m.add_vertex(Vec3::new(0.1, 0.0, i as f64 * 0.5));
            m.add_vertex(Vec3::new(-0.1, 0.0, i as f64 * 0.5));
        }
        m.add_face(&[VertexId(0), VertexId(1), VertexId(3)]);
        let curve = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0),
            Vec3::new(2.0, 0.0, 2.0),
        ];
        let d = curve_deform(&m, &curve, Axis::Z);
        // The far end (z was 4) should have moved well into +X.
        let end = d.vertex(VertexId(16)).unwrap().position;
        assert!(end.x > 1.0, "bar followed the curve turn");
    }

    #[test]
    fn lattice_deform_bulges_a_grid() {
        let m = primitives::grid(6, 6, 6.0);
        let (lo, hi) = crate::measure::bounding_box(&m);
        let mut lat = Lattice::from_bounds([2, 2, 2], lo.sub(Vec3::new(0.5, 0.5, 0.5)), hi.add(Vec3::new(0.5, 0.5, 0.5)));
        // Push all top control points up.
        for p in lat.points.iter_mut() {
            if p.z > 0.0 {
                p.z += 2.0;
            }
        }
        let d = lattice_deform(&m, &lat);
        assert!((0..d.vertex_count()).any(|i| d.vertex(VertexId(i)).unwrap().position.z > 0.5));
    }

    #[test]
    fn hook_drags_with_falloff() {
        let m = primitives::grid(8, 8, 8.0);
        let all: Vec<VertexId> = (0..m.vertex_count()).map(VertexId).collect();
        let d = hook(&m, &all, Vec3::ZERO, Vec3::new(0.0, 0.0, 3.0), 4.0);
        let centre = (0..m.vertex_count()).find(|&i| m.vertex(VertexId(i)).unwrap().position.length() < 1e-9).unwrap();
        assert!((d.vertex(VertexId(centre)).unwrap().position.z - 3.0).abs() < 1e-9, "hook centre fully dragged");
        // A far corner is outside the falloff radius.
        let corner = (0..m.vertex_count()).max_by(|&a, &b| {
            m.vertex(VertexId(a)).unwrap().position.length().partial_cmp(&m.vertex(VertexId(b)).unwrap().position.length()).unwrap()
        }).unwrap();
        assert!(d.vertex(VertexId(corner)).unwrap().position.z.abs() < 1e-9);
    }

    #[test]
    fn shrinkwrap_pulls_a_grid_onto_a_sphere() {
        let grid = primitives::grid(6, 6, 4.0); // z = 0
        let sphere = primitives::uv_sphere(20, 12, 3.0);
        let w = shrinkwrap(&grid, &sphere, ShrinkMode::NearestSurfacePoint, 1.0);
        for i in 0..w.vertex_count() {
            let r = w.vertex(VertexId(i)).unwrap().position.length();
            assert!((r - 3.0).abs() < 0.3, "on the sphere, r ≈ 3 (got {r})");
        }
    }

    #[test]
    fn surface_deform_binds_then_follows() {
        let cloth = primitives::grid(5, 5, 3.0); // z = 0
        let table = primitives::grid(5, 5, 4.0); // z = 0, bigger
        let bind = bind_to_surface(&cloth, &table);

        // Lift the table +2 in z.
        let mut p = table.positions();
        for q in &mut p {
            q.z += 2.0;
        }
        let lifted = Mesh::from_polygons(&p, &to_soup(&table));

        let out = surface_deform(&cloth, &bind, &lifted);
        for i in 0..out.vertex_count() {
            assert!((out.vertex(VertexId(i)).unwrap().position.z - 2.0).abs() < 0.1, "cloth rose with the table");
        }
    }

    #[test]
    fn laplacian_deform_forwards_to_arap() {
        let m = primitives::grid(5, 5, 4.0);
        let a = laplacian_deform(&m, &[(VertexId(0), m.vertex(VertexId(0)).unwrap().position.add(Vec3::new(0.0, 0.0, 1.0)))], 3);
        assert!(a.is_ok());
    }
}
