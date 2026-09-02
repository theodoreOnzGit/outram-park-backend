// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Additional closed-form primitive generators (plane, circle, cone, torus,
// icosphere) and the "Add Mesh" redo-panel common settings. Blender analogue
// (architecture only): source/blender/editors/mesh/editmesh_add.cc and the
// add_mesh_* operators' redo panels. No upstream source copied; each generator
// is written from first principles and checked against Euler's polyhedron
// formula V - E + F = chi.
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

//! **Extra mesh primitives + Add-Mesh common settings** (`op-hzs.54.39`, GH
//! issue #37 §H).
//!
//! Completes the [`crate::primitives`] set with the primitives Blender's *Add
//! Mesh* menu offers that were not yet covered:
//!
//! - [`plane`] — a single quad (Add Plane).
//! - [`circle`] — an `n`-gon, optionally filled (Add Circle, fill = Nothing /
//!   N-Gon).
//! - [`cone`] — a cone or truncated cone (Add Cone, with `radius2`).
//! - [`torus`] — a ring torus (Add Torus, major/minor radius + segments).
//! - [`icosphere`] — a geodesic sphere by `subdivisions` of an icosahedron
//!   (Add Ico Sphere).
//!
//! [`AddMeshOptions`] is the "redo panel" every add-operator shares: where to
//! put the new geometry (`location`) and how to orient it (`rotation_euler`,
//! XYZ radians). [`AddMeshOptions::place`] applies it to a freshly generated
//! mesh.
//!
//! ## Units
//!
//! All radius / size arguments are dimensionless model-space lengths, as in
//! [`crate::primitives`]; angles are radians.

use std::f64::consts::PI;

use crate::math::Vec3;
use crate::mesh::Mesh;

/// The common "redo panel" settings shared by every *Add Mesh* operator:
/// where the new geometry is placed and how it is oriented.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AddMeshOptions {
    /// World-space location of the primitive's origin.
    pub location: Vec3,
    /// Orientation as intrinsic XYZ Euler angles, in radians, applied about
    /// the primitive's own origin before translation.
    pub rotation_euler: Vec3,
    /// Uniform scale applied about the origin before rotation (`1.0` = none).
    pub scale: f64,
}

impl Default for AddMeshOptions {
    fn default() -> Self {
        AddMeshOptions { location: Vec3::ZERO, rotation_euler: Vec3::ZERO, scale: 1.0 }
    }
}

impl AddMeshOptions {
    /// Placement at `location` with no rotation and unit scale.
    pub fn at(location: Vec3) -> Self {
        AddMeshOptions { location, ..Default::default() }
    }

    /// Apply this placement (scale → rotate → translate) to `mesh`, returning
    /// the transformed mesh.
    pub fn place(&self, mesh: &Mesh) -> Mesh {
        let (sx, cx) = self.rotation_euler.x.sin_cos();
        let (sy, cy) = self.rotation_euler.y.sin_cos();
        let (sz, cz) = self.rotation_euler.z.sin_cos();
        // R = Rz * Ry * Rx (intrinsic XYZ).
        let r = [
            [cy * cz, sx * sy * cz - cx * sz, cx * sy * cz + sx * sz],
            [cy * sz, sx * sy * sz + cx * cz, cx * sy * sz - sx * cz],
            [-sy, sx * cy, cx * cy],
        ];
        let s = self.scale;
        let positions: Vec<Vec3> = mesh
            .positions()
            .iter()
            .map(|p| {
                let (x, y, z) = (p.x * s, p.y * s, p.z * s);
                Vec3::new(
                    r[0][0] * x + r[0][1] * y + r[0][2] * z + self.location.x,
                    r[1][0] * x + r[1][1] * y + r[1][2] * z + self.location.y,
                    r[2][0] * x + r[2][1] * y + r[2][2] * z + self.location.z,
                )
            })
            .collect();
        let faces = polygons_as_usize(mesh);
        Mesh::from_polygons(&positions, &faces)
    }
}

fn polygons_as_usize(mesh: &Mesh) -> Vec<Vec<usize>> {
    mesh.polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect()
}

/// A single `size x size` quad in the `z = 0` plane, centred on the origin,
/// wound CCW as seen from `+z`.
///
/// Topology: **4** vertices, **4** edges, **1** face, `chi = 1` (a disc).
pub fn plane(size: f64) -> Mesh {
    let h = size * 0.5;
    let positions =
        vec![Vec3::new(-h, -h, 0.0), Vec3::new(h, -h, 0.0), Vec3::new(h, h, 0.0), Vec3::new(-h, h, 0.0)];
    Mesh::from_polygons(&positions, &[vec![0, 1, 2, 3]])
}

/// A regular `segments`-gon of `radius` in the `z = 0` plane, centred on the
/// origin.
///
/// With `fill = true` a single `segments`-sided N-gon face is added (`chi = 1`);
/// with `fill = false` only the boundary ring of edges exists (a wire loop,
/// `chi = 0`). `segments` is clamped to `>= 3`.
pub fn circle(segments: usize, radius: f64, fill: bool) -> Mesh {
    let n = segments.max(3);
    let positions: Vec<Vec3> = (0..n)
        .map(|i| {
            let a = 2.0 * PI * i as f64 / n as f64;
            Vec3::new(radius * a.cos(), radius * a.sin(), 0.0)
        })
        .collect();
    if fill {
        Mesh::from_polygons(&positions, &[(0..n).collect()])
    } else {
        // Edge-only loop, recorded as degenerate sliver triangles so the
        // polygon-soup model keeps every boundary edge — the crate idiom, see
        // [`crate::curve_surface`]'s internal wire mesh.
        let faces: Vec<Vec<usize>> = (0..n).map(|i| vec![i, (i + 1) % n, i]).collect();
        Mesh::from_polygons(&positions, &faces)
    }
}

/// A cone (or truncated cone) about the `z` axis, base at `z = -height/2`.
///
/// `radius1` is the base radius, `radius2` the top radius (`0.0` → a true
/// apex). `segments` (clamped `>= 3`) sides. Base and top are filled with
/// N-gon caps (the top cap is omitted when `radius2 == 0`). Closed genus-0,
/// `chi = 2`.
pub fn cone(segments: usize, radius1: f64, radius2: f64, height: f64) -> Mesh {
    let n = segments.max(3);
    let hz = height * 0.5;
    let mut positions: Vec<Vec3> = Vec::new();
    for i in 0..n {
        let a = 2.0 * PI * i as f64 / n as f64;
        positions.push(Vec3::new(radius1 * a.cos(), radius1 * a.sin(), -hz));
    }
    let apex = radius2.abs() < 1e-12;
    if apex {
        positions.push(Vec3::new(0.0, 0.0, hz));
    } else {
        for i in 0..n {
            let a = 2.0 * PI * i as f64 / n as f64;
            positions.push(Vec3::new(radius2 * a.cos(), radius2 * a.sin(), hz));
        }
    }
    let mut faces: Vec<Vec<usize>> = Vec::new();
    // Base cap (facing -z → wound CW seen from +z).
    faces.push((0..n).rev().collect());
    if apex {
        let a = n;
        for i in 0..n {
            faces.push(vec![i, (i + 1) % n, a]);
        }
    } else {
        for i in 0..n {
            let j = (i + 1) % n;
            faces.push(vec![i, j, n + j, n + i]);
        }
        faces.push((n..2 * n).collect()); // top cap, facing +z
    }
    Mesh::from_polygons(&positions, &faces)
}

/// A ring torus about the `z` axis: a tube of `minor_radius` whose centreline
/// is a circle of `major_radius` in the `z = 0` plane.
///
/// `major_segments` around the main ring, `minor_segments` around the tube
/// (each clamped `>= 3`). All quads, closed, genus-1 → `chi = 0`.
pub fn torus(major_segments: usize, minor_segments: usize, major_radius: f64, minor_radius: f64) -> Mesh {
    let (nm, nt) = (major_segments.max(3), minor_segments.max(3));
    let mut positions: Vec<Vec3> = Vec::with_capacity(nm * nt);
    for i in 0..nm {
        let u = 2.0 * PI * i as f64 / nm as f64;
        let (cu, su) = (u.cos(), u.sin());
        for j in 0..nt {
            let v = 2.0 * PI * j as f64 / nt as f64;
            let r = major_radius + minor_radius * v.cos();
            positions.push(Vec3::new(r * cu, r * su, minor_radius * v.sin()));
        }
    }
    let idx = |i: usize, j: usize| (i % nm) * nt + (j % nt);
    let mut faces: Vec<Vec<usize>> = Vec::with_capacity(nm * nt);
    for i in 0..nm {
        for j in 0..nt {
            faces.push(vec![idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    Mesh::from_polygons(&positions, &faces)
}

/// A geodesic sphere of `radius`: an icosahedron subdivided `subdivisions`
/// times, each new vertex pushed back onto the sphere.
///
/// `subdivisions` is clamped to `0..=5` (a level-5 icosphere is 20480 faces).
/// All triangles, closed genus-0 → `chi = 2`.
pub fn icosphere(subdivisions: usize, radius: f64) -> Mesh {
    let t = (1.0 + 5.0_f64.sqrt()) * 0.5;
    let mut verts: Vec<Vec3> = vec![
        Vec3::new(-1.0, t, 0.0),
        Vec3::new(1.0, t, 0.0),
        Vec3::new(-1.0, -t, 0.0),
        Vec3::new(1.0, -t, 0.0),
        Vec3::new(0.0, -1.0, t),
        Vec3::new(0.0, 1.0, t),
        Vec3::new(0.0, -1.0, -t),
        Vec3::new(0.0, 1.0, -t),
        Vec3::new(t, 0.0, -1.0),
        Vec3::new(t, 0.0, 1.0),
        Vec3::new(-t, 0.0, -1.0),
        Vec3::new(-t, 0.0, 1.0),
    ];
    let mut tris: Vec<[usize; 3]> = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
        [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
        [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];

    let mut midpoint_cache: std::collections::HashMap<(usize, usize), usize> =
        std::collections::HashMap::new();
    for _ in 0..subdivisions.min(5) {
        let mut next: Vec<[usize; 3]> = Vec::with_capacity(tris.len() * 4);
        let mut mid = |a: usize, b: usize, verts: &mut Vec<Vec3>| -> usize {
            let key = if a < b { (a, b) } else { (b, a) };
            if let Some(&m) = midpoint_cache.get(&key) {
                return m;
            }
            let m = verts[a].add(verts[b]).scale(0.5);
            verts.push(m);
            let id = verts.len() - 1;
            midpoint_cache.insert(key, id);
            id
        };
        for tri in &tris {
            let a = mid(tri[0], tri[1], &mut verts);
            let b = mid(tri[1], tri[2], &mut verts);
            let c = mid(tri[2], tri[0], &mut verts);
            next.push([tri[0], a, c]);
            next.push([tri[1], b, a]);
            next.push([tri[2], c, b]);
            next.push([a, b, c]);
        }
        tris = next;
    }

    let positions: Vec<Vec3> = verts.iter().map(|v| v.normalize().scale(radius)).collect();
    let faces: Vec<Vec<usize>> = tris.iter().map(|t| vec![t[0], t[1], t[2]]).collect();
    Mesh::from_polygons(&positions, &faces)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_is_one_quad_disc() {
        let m = plane(2.0);
        assert_eq!(m.vertex_count(), 4);
        assert_eq!(m.face_count(), 1);
        assert_eq!(m.euler_characteristic(), 1);
    }

    #[test]
    fn filled_circle_is_an_ngon() {
        let m = circle(8, 1.0, true);
        assert_eq!(m.vertex_count(), 8);
        assert_eq!(m.face_count(), 1);
        assert_eq!(m.euler_characteristic(), 1);
        for i in 0..m.vertex_count() {
            let p = m.vertex(crate::mesh::VertexId(i)).unwrap().position;
            assert!((p.x * p.x + p.y * p.y - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn wire_circle_is_an_edge_loop_not_an_ngon() {
        let m = circle(6, 1.0, false);
        assert_eq!(m.vertex_count(), 6);
        // Recorded as 6 sliver triangles, not one filled n-gon.
        assert_eq!(m.polygons().len(), 6);
        assert_ne!(m.face_count(), 1);
    }

    #[test]
    fn apex_cone_is_a_closed_solid() {
        let m = cone(12, 1.0, 0.0, 2.0);
        assert_eq!(m.vertex_count(), 13);
        assert_eq!(m.euler_characteristic(), 2);
    }

    #[test]
    fn truncated_cone_is_a_closed_solid() {
        let m = cone(10, 1.0, 0.5, 2.0);
        assert_eq!(m.vertex_count(), 20);
        assert_eq!(m.euler_characteristic(), 2);
        let (lo, hi) = crate::measure::bounding_box(&m);
        assert!((hi.z - lo.z - 2.0).abs() < 1e-9);
    }

    #[test]
    fn torus_is_genus_one() {
        let m = torus(16, 8, 2.0, 0.5);
        assert_eq!(m.vertex_count(), 16 * 8);
        assert_eq!(m.face_count(), 16 * 8);
        assert_eq!(m.euler_characteristic(), 0, "genus-1 torus");
    }

    #[test]
    fn icosphere_level0_is_the_icosahedron() {
        let m = icosphere(0, 1.0);
        assert_eq!(m.vertex_count(), 12);
        assert_eq!(m.face_count(), 20);
        assert_eq!(m.euler_characteristic(), 2);
    }

    #[test]
    fn icosphere_level2_stays_on_the_sphere() {
        let m = icosphere(2, 3.0);
        assert_eq!(m.euler_characteristic(), 2);
        for i in 0..m.vertex_count() {
            let p = m.vertex(crate::mesh::VertexId(i)).unwrap().position;
            assert!((p.length() - 3.0).abs() < 1e-9, "radius preserved");
        }
    }

    #[test]
    fn add_mesh_options_place_translates_and_rotates() {
        let m = plane(2.0);
        let opts = AddMeshOptions {
            location: Vec3::new(5.0, 0.0, 0.0),
            rotation_euler: Vec3::new(PI / 2.0, 0.0, 0.0), // +90° about x: z→y... y→-z... wait check
            scale: 1.0,
        };
        let placed = opts.place(&m);
        let (lo, hi) = crate::measure::bounding_box(&placed);
        // Rotating the flat XY quad about x by 90° makes it span in z, not y.
        assert!((hi.z - lo.z) > 1.0, "quad now stands up in z");
        assert!((hi.y - lo.y) < 1e-9, "quad is now flat in y");
        // Centre moved to x = 5.
        assert!(((lo.x + hi.x) * 0.5 - 5.0).abs() < 1e-9);
    }

    #[test]
    fn add_mesh_options_scale() {
        let m = plane(2.0);
        let placed = AddMeshOptions { scale: 3.0, ..Default::default() }.place(&m);
        let (lo, hi) = crate::measure::bounding_box(&placed);
        assert!((hi.x - lo.x - 6.0).abs() < 1e-9);
    }
}
