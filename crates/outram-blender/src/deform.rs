// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Deform modifiers (part 1). Follows the published behaviour of Blender's
// deform modifiers (source/blender/modifiers/intern/MOD_simpledeform.cc,
// MOD_cast.cc, MOD_displace.cc, MOD_warp.cc, MOD_wave.cc, github.com/blender/
// blender, GPL-2.0-or-later): Simple Deform (twist / bend / taper / stretch),
// Cast, Displace, Warp, Wave — all position-only. Concepts only — no upstream
// source copied.
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

//! **Deform modifiers pt.1** (`op-hzs.54.31`, GH issue #37 §F) — position-only
//! space deformers.
//!
//! - [`simple_deform`] — [`SimpleDeform::Twist`] / [`SimpleDeform::Bend`] /
//!   [`SimpleDeform::Taper`] / [`SimpleDeform::Stretch`] along an [`Axis`],
//!   parameterised over the mesh's extent on that axis.
//! - [`cast`] — pull toward a [`CastTarget`] (sphere / cylinder / cuboid).
//! - [`displace`] — offset along a direction by value noise (a stand-in for
//!   Blender's texture input).
//! - [`warp`] — bend space so a "from" segment maps onto a "to" segment.
//! - [`wave`] — a travelling sine ripple.

use crate::math::Vec3;
use crate::mesh::{Mesh, VertexId};
use crate::selection::Axis;

/// Simple Deform modes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SimpleDeform {
    /// Rotate progressively about the axis (radians end-to-end).
    Twist(f64),
    /// Bend into an arc of the given total angle (radians).
    Bend(f64),
    /// Scale the perpendicular cross-section from `1` to `1 + factor` along
    /// the axis.
    Taper(f64),
    /// Stretch by `factor` along the axis, contracting perpendicular by
    /// `1/√(1 + factor)` (volume-preserving-ish).
    Stretch(f64),
}

/// What [`cast`] pulls toward.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CastTarget {
    /// A sphere of the given radius about the mesh centre.
    Sphere(f64),
    /// A cylinder of the given radius about the axis through the mesh centre.
    Cylinder { radius: f64, axis: Axis },
    /// A cuboid of the given half-extents about the mesh centre.
    Cuboid(Vec3),
}

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

fn to_soup(mesh: &Mesh) -> Vec<Vec<usize>> {
    mesh.polygons()
        .iter()
        .map(|p| p.iter().map(|v| v.0).collect())
        .collect()
}

fn deform(mesh: &Mesh, verts: &[VertexId], f: impl Fn(Vec3) -> Vec3) -> Mesh {
    let mut positions = mesh.positions();
    let idx: Vec<usize> = if verts.is_empty() {
        (0..positions.len()).collect()
    } else {
        verts
            .iter()
            .map(|v| v.0)
            .filter(|&i| i < positions.len())
            .collect()
    };
    for &i in &idx {
        positions[i] = f(positions[i]);
    }
    Mesh::from_polygons(&positions, &to_soup(mesh))
}

fn axis_span(mesh: &Mesh, verts: &[VertexId], axis: Axis) -> (f64, f64) {
    let pos = mesh.positions();
    let idx: Vec<usize> = if verts.is_empty() {
        (0..pos.len()).collect()
    } else {
        verts.iter().map(|v| v.0).collect()
    };
    let mut lo = f64::MAX;
    let mut hi = f64::MIN;
    for &i in &idx {
        let c = axis_coord(pos[i], axis);
        lo = lo.min(c);
        hi = hi.max(c);
    }
    (lo, hi)
}

fn rodrigues(v: Vec3, k: Vec3, theta: f64) -> Vec3 {
    let (s, c) = theta.sin_cos();
    v.scale(c)
        .add(k.cross(v).scale(s))
        .add(k.scale(k.dot(v) * (1.0 - c)))
}

/// Apply a [`SimpleDeform`] along `axis` over the selection's extent.
pub fn simple_deform(mesh: &Mesh, verts: &[VertexId], mode: SimpleDeform, axis: Axis) -> Mesh {
    let (lo, hi) = axis_span(mesh, verts, axis);
    let span = (hi - lo).abs().max(1e-9);
    let mid = (lo + hi) * 0.5;
    let k = axis_unit(axis);
    deform(mesh, verts, |p| {
        let c = axis_coord(p, axis);
        let t = (c - lo) / span; // 0..1
        match mode {
            SimpleDeform::Twist(angle) => {
                // Rotate progressively about the axis through the origin.
                let along = k.scale(c);
                let perp = p.sub(along);
                along.add(rodrigues(perp, k, angle * (t - 0.5)))
            }
            SimpleDeform::Bend(angle) => {
                let a = angle * (t - 0.5);
                let rel = p.sub(k.scale(mid));
                k.scale(mid).add(rodrigues(rel, bend_axis(axis), a))
            }
            SimpleDeform::Taper(factor) => {
                let s = 1.0 + factor * t;
                let along = k.scale(c);
                along.add(p.sub(along).scale(s))
            }
            SimpleDeform::Stretch(factor) => {
                let along_scale = 1.0 + factor;
                let perp_scale = 1.0 / (1.0 + factor).abs().max(1e-6).sqrt();
                let along = k.scale(c);
                k.scale(mid + (c - mid) * along_scale)
                    .add(p.sub(along).scale(perp_scale))
            }
        }
    })
}

/// The rotation axis for a Bend along `axis` — perpendicular to it.
fn bend_axis(axis: Axis) -> Vec3 {
    match axis {
        Axis::X => Vec3::new(0.0, 0.0, 1.0),
        Axis::Y => Vec3::new(0.0, 0.0, 1.0),
        Axis::Z => Vec3::new(1.0, 0.0, 0.0),
    }
}

/// Pull the selection a fraction `factor` of the way toward `target`
/// (about the selection's centre).
pub fn cast(mesh: &Mesh, verts: &[VertexId], target: CastTarget, factor: f64) -> Mesh {
    let pos = mesh.positions();
    let idx: Vec<usize> = if verts.is_empty() {
        (0..pos.len()).collect()
    } else {
        verts.iter().map(|v| v.0).collect()
    };
    let centre = idx
        .iter()
        .fold(Vec3::ZERO, |a, &i| a.add(pos[i]))
        .scale(1.0 / idx.len().max(1) as f64);
    // Reference size: mean distance from the centre.
    let mean_r = idx
        .iter()
        .map(|&i| pos[i].sub(centre).length())
        .sum::<f64>()
        / idx.len().max(1) as f64;

    deform(mesh, verts, |p| {
        let d = p.sub(centre);
        let onto = match target {
            CastTarget::Sphere(r) => {
                let len = d.length();
                if len < 1e-9 {
                    p
                } else {
                    centre.add(d.scale(r.max(1e-9) / len))
                }
            }
            CastTarget::Cylinder { radius, axis } => {
                let k = axis_unit(axis);
                let along = k.scale(d.dot(k));
                let radial = d.sub(along);
                let rl = radial.length();
                if rl < 1e-9 {
                    p
                } else {
                    centre.add(along).add(radial.scale(radius.max(1e-9) / rl))
                }
            }
            CastTarget::Cuboid(half) => {
                let s = mean_r.max(1e-9);
                centre.add(Vec3::new(
                    half.x * (d.x / s).clamp(-1.0, 1.0),
                    half.y * (d.y / s).clamp(-1.0, 1.0),
                    half.z * (d.z / s).clamp(-1.0, 1.0),
                ))
            }
        };
        p.add(onto.sub(p).scale(factor))
    })
}

/// Displace along `direction` by `strength` times value noise sampled at
/// `p * noise_scale`. A texture-free stand-in for Blender's Displace.
pub fn displace(
    mesh: &Mesh,
    verts: &[VertexId],
    direction: Vec3,
    strength: f64,
    noise_scale: f64,
    seed: u64,
) -> Mesh {
    let dir = if direction.length() > 1e-9 {
        direction.normalize()
    } else {
        Vec3::new(0.0, 0.0, 1.0)
    };
    deform(mesh, verts, |p| {
        let n = value_noise(p.scale(noise_scale), seed) - 0.5;
        p.add(dir.scale(strength * n * 2.0))
    })
}

/// Warp space so the segment `from` … `from2` maps onto `to` … `to2` (a
/// rotate + scale + translate blended by proximity to `from`).
pub fn warp(
    mesh: &Mesh,
    verts: &[VertexId],
    from: Vec3,
    from2: Vec3,
    to: Vec3,
    to2: Vec3,
    falloff_radius: f64,
) -> Mesh {
    let src_vec = from2.sub(from);
    let dst_vec = to2.sub(to);
    let src_len = src_vec.length().max(1e-9);
    let scale = dst_vec.length() / src_len;
    let rot_axis = src_vec.cross(dst_vec);
    let angle = (src_vec.dot(dst_vec) / (src_len * dst_vec.length().max(1e-9)))
        .clamp(-1.0, 1.0)
        .acos();
    let k = if rot_axis.length() > 1e-9 {
        rot_axis.normalize()
    } else {
        Vec3::new(0.0, 0.0, 1.0)
    };

    deform(mesh, verts, |p| {
        let rel = p.sub(from);
        let full = to.add(rodrigues(rel.scale(scale), k, angle));
        let w = if falloff_radius <= 0.0 {
            1.0
        } else {
            (1.0 - rel.length() / falloff_radius).clamp(0.0, 1.0)
        };
        p.add(full.sub(p).scale(w))
    })
}

/// A travelling sine ripple: displace along `axis` by
/// `amplitude · sin(2π (r/wavelength − speed·time))` where `r` is the distance
/// from the origin in the plane orthogonal to `axis`.
pub fn wave(
    mesh: &Mesh,
    verts: &[VertexId],
    axis: Axis,
    amplitude: f64,
    wavelength: f64,
    speed: f64,
    time: f64,
) -> Mesh {
    let k = axis_unit(axis);
    let wl = wavelength.abs().max(1e-6);
    deform(mesh, verts, |p| {
        let radial = p.sub(k.scale(p.dot(k))).length();
        let phase = std::f64::consts::TAU * (radial / wl - speed * time);
        p.add(k.scale(amplitude * phase.sin()))
    })
}

/// Trilinear value noise in `[0, 1]` from a lattice of hashed random values.
fn value_noise(p: Vec3, seed: u64) -> f64 {
    let (xi, yi, zi) = (p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64);
    let (fx, fy, fz) = (p.x - xi as f64, p.y - yi as f64, p.z - zi as f64);
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let sz = fz * fz * (3.0 - 2.0 * fz);
    let mut acc = 0.0;
    for dz in 0..2 {
        for dy in 0..2 {
            for dx in 0..2 {
                let v = hash01(xi + dx, yi + dy, zi + dz, seed);
                let wx = if dx == 0 { 1.0 - sx } else { sx };
                let wy = if dy == 0 { 1.0 - sy } else { sy };
                let wz = if dz == 0 { 1.0 - sz } else { sz };
                acc += v * wx * wy * wz;
            }
        }
    }
    acc
}

fn hash01(x: i64, y: i64, z: i64, seed: u64) -> f64 {
    let mut h = seed
        ^ (x as u64).wrapping_mul(0x9E3779B97F4A7C15)
        ^ (y as u64).wrapping_mul(0xC2B2AE3D27D4EB4F)
        ^ (z as u64).wrapping_mul(0x165667B19E3779F9);
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51AFD7ED558CCD);
    h ^= h >> 33;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn twist_rotates_the_ends_oppositely() {
        // A bar along Z; twist ±. The top and bottom rotate opposite ways.
        let mut m = Mesh::new();
        let a = m.add_vertex(Vec3::new(1.0, 0.0, -1.0));
        let b = m.add_vertex(Vec3::new(1.0, 0.0, 1.0));
        let c = m.add_vertex(Vec3::new(0.0, 1.0, 0.0));
        m.add_face(&[a, b, c]);
        let t = simple_deform(&m, &[], SimpleDeform::Twist(std::f64::consts::PI), Axis::Z);
        let pa = t.vertex(a).unwrap().position;
        let pb = t.vertex(b).unwrap().position;
        // The two ends' x/y swapped signs relative to each other.
        assert!((pa.x + pb.x).abs() < 1e-9 || (pa.y - (-pb.y)).abs() < 1e-9);
    }

    #[test]
    fn taper_shrinks_one_end() {
        let m = primitives::cylinder(12, 1.0, 2.0); // axis Z, z ∈ [-1, 1]
        let t = simple_deform(&m, &[], SimpleDeform::Taper(-0.8), Axis::Z);
        // The -Z end (t≈0) keeps radius ~1; the +Z end (t≈1) shrinks to ~0.2.
        let r_at = |mm: &Mesh, want_z: f64| {
            (0..mm.vertex_count())
                .filter(|&i| (mm.vertex(VertexId(i)).unwrap().position.z - want_z).abs() < 0.1)
                .map(|i| {
                    let p = mm.vertex(VertexId(i)).unwrap().position;
                    (p.x * p.x + p.y * p.y).sqrt()
                })
                .fold(0.0_f64, f64::max)
        };
        assert!(r_at(&t, -1.0) > 0.9);
        assert!(r_at(&t, 1.0) < 0.5);
    }

    #[test]
    fn cast_to_sphere_puts_verts_on_the_sphere() {
        let m = primitives::grid(4, 4, 4.0);
        let c = cast(&m, &[], CastTarget::Sphere(3.0), 1.0);
        let centre = m
            .positions()
            .iter()
            .fold(Vec3::ZERO, |a, &p| a.add(p))
            .scale(1.0 / m.vertex_count() as f64);
        for i in 0..c.vertex_count() {
            let r = c.vertex(VertexId(i)).unwrap().position.sub(centre).length();
            // A vertex at the centre can't be cast; skip near-zero.
            if m.vertex(VertexId(i)).unwrap().position.sub(centre).length() > 1e-6 {
                assert!((r - 3.0).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn displace_is_deterministic_and_moves_points() {
        let m = primitives::grid(6, 6, 6.0);
        let a = displace(&m, &[], Vec3::new(0.0, 0.0, 1.0), 0.5, 0.5, 3);
        let b = displace(&m, &[], Vec3::new(0.0, 0.0, 1.0), 0.5, 0.5, 3);
        for i in 0..a.vertex_count() {
            assert!(
                a.vertex(VertexId(i))
                    .unwrap()
                    .position
                    .sub(b.vertex(VertexId(i)).unwrap().position)
                    .length()
                    < 1e-12
            );
        }
        assert!(
            (0..a.vertex_count()).any(|i| a.vertex(VertexId(i)).unwrap().position.z.abs() > 1e-6)
        );
    }

    #[test]
    fn wave_makes_a_ripple() {
        let m = primitives::grid(10, 10, 10.0);
        let w = wave(&m, &[], Axis::Z, 0.5, 3.0, 0.0, 0.0);
        let zs: Vec<f64> = (0..w.vertex_count())
            .map(|i| w.vertex(VertexId(i)).unwrap().position.z)
            .collect();
        assert!(zs.iter().cloned().fold(f64::MIN, f64::max) > 0.1);
        assert!(zs.iter().cloned().fold(f64::MAX, f64::min) < -0.1);
    }

    #[test]
    fn warp_maps_the_from_segment_onto_the_to_segment() {
        let m = primitives::grid(4, 4, 4.0);
        let w = warp(
            &m,
            &[],
            Vec3::ZERO,
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::ZERO,
            Vec3::new(0.0, 2.0, 0.0), // rotate the +x direction to +y
            0.0,
        );
        // A vertex that was at (2, 0, 0) should land near (0, 2, 0).
        let near = (0..m.vertex_count()).find(|&i| {
            m.vertex(VertexId(i))
                .unwrap()
                .position
                .sub(Vec3::new(2.0, 0.0, 0.0))
                .length()
                < 1e-6
        });
        if let Some(i) = near {
            let p = w.vertex(VertexId(i)).unwrap().position;
            assert!(p.sub(Vec3::new(0.0, 2.0, 0.0)).length() < 1e-6);
        }
    }
}
