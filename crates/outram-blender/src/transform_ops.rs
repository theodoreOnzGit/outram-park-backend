// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Edit-mode transform toolset. Follows the published behaviour of Blender's
// transform operators (source/blender/editors/transform/transform_mode_*.cc and
// the mesh smooth/randomize tools, github.com/blender/blender,
// GPL-2.0-or-later): To Sphere, Shear, Bend, Warp, Push/Pull, Shrink/Fatten,
// Randomize, Smooth Vertices — all position-only over a vertex selection.
// Concepts only — no upstream source copied; position rewrites.
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

//! **Edit-mode transform toolset** (`op-hzs.54.19`, GH issue #37 §C) —
//! parameterised versions of Blender's interactive transform tools. Every one
//! takes a `verts` subset (empty = whole mesh) and rewrites positions only.
//!
//! - [`to_sphere`] — blend toward a sphere of `radius` about `center`.
//! - [`shear`] — offset along `shear_axis` proportional to the coordinate on
//!   `measure_axis`.
//! - [`bend`] — wrap the selection around an arc of `angle` about `center`.
//! - [`warp`] — Blender's Warp: bend around the 3D cursor in the view plane.
//! - [`push_pull`] — move each vertex toward / away from `center`.
//! - [`shrink_fatten`] — move each vertex along its averaged normal.
//! - [`randomize`] — deterministic per-vertex jitter.
//! - [`smooth_vertices`] — Laplacian smoothing with a per-axis mask.

use crate::math::Vec3;
use crate::mesh::{Mesh, VertexId};
use crate::topology::MeshTopology;

/// A coordinate axis for [`shear`] / [`bend`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    fn get(self, p: Vec3) -> f64 {
        match self {
            Axis::X => p.x,
            Axis::Y => p.y,
            Axis::Z => p.z,
        }
    }
    fn unit(self) -> Vec3 {
        match self {
            Axis::X => Vec3::new(1.0, 0.0, 0.0),
            Axis::Y => Vec3::new(0.0, 1.0, 0.0),
            Axis::Z => Vec3::new(0.0, 0.0, 1.0),
        }
    }
}

fn apply(mesh: &Mesh, verts: &[VertexId], mut f: impl FnMut(usize, Vec3) -> Vec3) -> Mesh {
    let mut positions = mesh.positions();
    let idx: Vec<usize> = if verts.is_empty() {
        (0..positions.len()).collect()
    } else {
        verts.iter().map(|v| v.0).filter(|&i| i < positions.len()).collect()
    };
    for &i in &idx {
        positions[i] = f(i, positions[i]);
    }
    Mesh::from_polygons(&positions, &to_soup(mesh))
}

fn to_soup(mesh: &Mesh) -> Vec<Vec<usize>> {
    mesh.polygons().iter().map(|p| p.iter().map(|v| v.0).collect()).collect()
}

/// Blend the selection toward a sphere of `radius` about `center` by `factor`
/// (`0` = unchanged, `1` = fully on the sphere).
pub fn to_sphere(mesh: &Mesh, verts: &[VertexId], center: Vec3, radius: f64, factor: f64) -> Mesh {
    apply(mesh, verts, |_, p| {
        let d = p.sub(center);
        let len = d.length();
        if len < 1e-12 {
            return p;
        }
        let on_sphere = center.add(d.scale(radius / len));
        p.add(on_sphere.sub(p).scale(factor))
    })
}

/// Shear: shift each vertex along `shear_axis` by `factor · coord(measure_axis)`.
pub fn shear(mesh: &Mesh, verts: &[VertexId], measure_axis: Axis, shear_axis: Axis, factor: f64) -> Mesh {
    let dir = shear_axis.unit();
    apply(mesh, verts, |_, p| p.add(dir.scale(factor * measure_axis.get(p))))
}

/// Bend the selection around an arc: a vertex at signed distance `t` along
/// `along` from `center` is rotated by `angle · t / span` about the axis
/// `axis`, where `span` is the selection's extent along `along`.
pub fn bend(mesh: &Mesh, verts: &[VertexId], center: Vec3, along: Axis, axis: Axis, angle: f64) -> Mesh {
    // Selection extent along `along`.
    let positions = mesh.positions();
    let idx: Vec<usize> = if verts.is_empty() {
        (0..positions.len()).collect()
    } else {
        verts.iter().map(|v| v.0).collect()
    };
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for &i in &idx {
        let a = along.get(positions[i].sub(center));
        lo = lo.min(a);
        hi = hi.max(a);
    }
    let span = (hi - lo).abs().max(1e-9);
    let ax = axis.unit();
    apply(mesh, verts, |_, p| {
        let rel = p.sub(center);
        let t = along.get(rel);
        let a = angle * t / span;
        center.add(rotate_about_axis(rel, ax, a))
    })
}

/// Warp: bend the selection around `center` in the plane orthogonal to `axis`,
/// mapping its `along` extent onto an arc of `angle`.
pub fn warp(mesh: &Mesh, verts: &[VertexId], center: Vec3, along: Axis, axis: Axis, angle: f64) -> Mesh {
    bend(mesh, verts, center, along, axis, angle)
}

/// Push (`distance < 0`) or pull (`distance > 0`) each vertex along the ray
/// from `center`.
pub fn push_pull(mesh: &Mesh, verts: &[VertexId], center: Vec3, distance: f64) -> Mesh {
    apply(mesh, verts, |_, p| {
        let d = p.sub(center);
        let len = d.length();
        if len < 1e-12 {
            return p;
        }
        p.add(d.scale(distance / len))
    })
}

/// Move each vertex `offset` along its averaged (incident-face) normal —
/// Blender's Shrink/Fatten (offset along normals).
pub fn shrink_fatten(mesh: &Mesh, verts: &[VertexId], offset: f64) -> Mesh {
    let topo = MeshTopology::new(mesh);
    let mut vnorm = vec![Vec3::ZERO; mesh.vertex_count()];
    for (v, slot) in vnorm.iter_mut().enumerate() {
        let mut n = Vec3::ZERO;
        for &f in topo.vertex_faces(VertexId(v)) {
            n = n.add(mesh.face_normal(f));
        }
        *slot = if n.length() > 1e-12 { n.normalize() } else { Vec3::ZERO };
    }
    apply(mesh, verts, |i, p| p.add(vnorm[i].scale(offset)))
}

/// Deterministic per-vertex jitter of magnitude up to `amount`. `uniform`
/// gives the same displacement magnitude to every vertex (only the direction
/// varies); otherwise the magnitude is random too.
pub fn randomize(mesh: &Mesh, verts: &[VertexId], amount: f64, seed: u64, uniform: bool) -> Mesh {
    let mut rng = XorShift::new(seed);
    apply(mesh, verts, |_, p| {
        let dir = Vec3::new(rng.unit() - 0.5, rng.unit() - 0.5, rng.unit() - 0.5);
        let dir = if dir.length() > 1e-12 { dir.normalize() } else { Vec3::new(1.0, 0.0, 0.0) };
        let m = if uniform { amount } else { amount * rng.unit() };
        p.add(dir.scale(m))
    })
}

/// Laplacian-smooth the selection: `iterations` passes, each moving every
/// selected vertex a fraction `factor` toward the mean of its edge-neighbours.
/// `mask` disables movement on an axis when `false`.
pub fn smooth_vertices(
    mesh: &Mesh,
    verts: &[VertexId],
    iterations: u32,
    factor: f64,
    mask: [bool; 3],
) -> Mesh {
    let topo = MeshTopology::new(mesh);
    let sel: std::collections::BTreeSet<usize> = if verts.is_empty() {
        (0..mesh.vertex_count()).collect()
    } else {
        verts.iter().map(|v| v.0).collect()
    };
    let mut positions = mesh.positions();
    let f = factor.clamp(0.0, 1.0);
    for _ in 0..iterations {
        let snap = positions.clone();
        for &v in &sel {
            let ns: Vec<usize> = topo
                .vertex_edges(VertexId(v))
                .iter()
                .filter_map(|&e| topo.other_end(mesh, e, VertexId(v)))
                .map(|w| w.0)
                .collect();
            if ns.is_empty() {
                continue;
            }
            let mean = ns.iter().fold(Vec3::ZERO, |acc, &n| acc.add(snap[n])).scale(1.0 / ns.len() as f64);
            let delta = mean.sub(snap[v]).scale(f);
            positions[v] = Vec3::new(
                snap[v].x + if mask[0] { delta.x } else { 0.0 },
                snap[v].y + if mask[1] { delta.y } else { 0.0 },
                snap[v].z + if mask[2] { delta.z } else { 0.0 },
            );
        }
    }
    Mesh::from_polygons(&positions, &to_soup(mesh))
}

/// Rotate `v` about the unit axis `k` by `theta` (Rodrigues).
fn rotate_about_axis(v: Vec3, k: Vec3, theta: f64) -> Vec3 {
    let (s, c) = theta.sin_cos();
    v.scale(c)
        .add(k.cross(v).scale(s))
        .add(k.scale(k.dot(v) * (1.0 - c)))
}

/// Deterministic xorshift64* — no external crate (offline reproducibility).
struct XorShift(u64);
impl XorShift {
    fn new(seed: u64) -> Self {
        XorShift(seed.max(1))
    }
    fn unit(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        ((x.wrapping_mul(0x2545F4914F6CDD1D)) >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn to_sphere_puts_a_grid_on_a_sphere() {
        let m = primitives::grid(4, 4, 4.0);
        let c = Vec3::new(0.0, 0.0, -10.0); // no grid vertex sits here
        let s = to_sphere(&m, &[], c, 3.0, 1.0);
        for i in 0..s.vertex_count() {
            let r = s.vertex(VertexId(i)).unwrap().position.sub(c).length();
            assert!((r - 3.0).abs() < 1e-9);
        }
    }

    #[test]
    fn shear_offsets_proportional_to_the_measured_axis() {
        let m = primitives::grid(2, 2, 4.0);
        let s = shear(&m, &[], Axis::Y, Axis::X, 0.5);
        for i in 0..m.vertex_count() {
            let before = m.vertex(VertexId(i)).unwrap().position;
            let after = s.vertex(VertexId(i)).unwrap().position;
            assert!((after.x - (before.x + 0.5 * before.y)).abs() < 1e-9);
        }
    }

    #[test]
    fn bend_wraps_a_bar_into_an_arc() {
        // A bar along X; bend 180° about Z should fold the ends toward -X.
        let mut m = Mesh::new();
        for i in 0..9 {
            let x = i as f64 - 4.0;
            m.add_vertex(Vec3::new(x, 0.0, 0.0));
            m.add_vertex(Vec3::new(x, 0.2, 0.0));
        }
        // dummy face to keep verts
        m.add_face(&[VertexId(0), VertexId(1), VertexId(3)]);
        let b = bend(&m, &[], Vec3::ZERO, Axis::X, Axis::Z, std::f64::consts::PI);
        // The far ends rotated toward each other → both x < original magnitude.
        let end = b.vertex(VertexId(16)).unwrap().position; // x was +4
        assert!(end.x < 3.5, "the +X end bent inward");
    }

    #[test]
    fn shrink_fatten_inflates_a_cube() {
        let m = primitives::cube(2.0);
        let f = shrink_fatten(&m, &[], 0.5);
        // Every corner moved outward along its (1,1,1)-ish normal.
        for i in 0..8 {
            let a = m.vertex(VertexId(i)).unwrap().position.length();
            let b = f.vertex(VertexId(i)).unwrap().position.length();
            assert!(b > a);
        }
    }

    #[test]
    fn randomize_is_deterministic() {
        let m = primitives::grid(3, 3, 3.0);
        let a = randomize(&m, &[], 0.3, 42, false);
        let b = randomize(&m, &[], 0.3, 42, false);
        for i in 0..a.vertex_count() {
            assert!(a.vertex(VertexId(i)).unwrap().position.sub(b.vertex(VertexId(i)).unwrap().position).length() < 1e-12);
        }
    }

    #[test]
    fn smooth_relaxes_a_spike() {
        let mut m = primitives::grid(4, 4, 4.0);
        let mut pos = m.positions();
        let spike = pos.len() / 2;
        pos[spike] = pos[spike].add(Vec3::new(0.0, 0.0, 3.0));
        m = Mesh::from_polygons(&pos, &to_soup(&m));
        let before = m.vertex(VertexId(spike)).unwrap().position.z;
        let s = smooth_vertices(&m, &[], 5, 0.5, [true, true, true]);
        let after = s.vertex(VertexId(spike)).unwrap().position.z;
        assert!(after < before * 0.6, "spike pulled down");
    }

    #[test]
    fn smooth_axis_mask_locks_z() {
        let mut m = primitives::grid(4, 4, 4.0);
        let mut pos = m.positions();
        let spike = pos.len() / 2;
        pos[spike] = pos[spike].add(Vec3::new(0.0, 0.0, 3.0));
        m = Mesh::from_polygons(&pos, &to_soup(&m));
        let s = smooth_vertices(&m, &[], 5, 0.5, [true, true, false]);
        assert!((s.vertex(VertexId(spike)).unwrap().position.z - 3.0).abs() < 1e-6, "z locked");
    }

    #[test]
    fn push_pull_moves_along_the_ray() {
        let m = primitives::cube(2.0);
        let p = push_pull(&m, &[], Vec3::ZERO, 1.0);
        for i in 0..8 {
            let a = m.vertex(VertexId(i)).unwrap().position.length();
            let b = p.vertex(VertexId(i)).unwrap().position.length();
            assert!((b - (a + 1.0)).abs() < 1e-9);
        }
    }
}
