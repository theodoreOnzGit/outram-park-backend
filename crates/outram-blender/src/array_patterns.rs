// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Array patterns beyond the linear Array modifier. Blender analogue
// (architecture only): the Array modifier's "Object Offset" mode and the
// Curve modifier used together for radial / along-curve arrays. No upstream
// source copied. Built on `transform::Affine3` and `curve::Spline`.
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

//! **Array patterns** (`op-hzs.54.48`, GH issue #37 §J).
//!
//! - [`radial_array`] / [`circular_array`] — copies rotated about an axis
//!   (pin lattices, MSR loops, sphere rings).
//! - [`object_offset_array`] — copies under a compounding [`Affine3`] offset
//!   (the Array modifier's *Object Offset*), spiralling / scaling stacks.
//! - [`array_along_curve`] — copies distributed along a [`crate::curve::Spline`],
//!   each oriented to the curve frame.
//! - [`ArrayCaps`] — optional start / end cap meshes on any of the above.
//!
//! Every function returns one merged [`Mesh`]; copies are **not** welded (they
//! are separate shells), matching the Array modifier.
//!
//! ## Units
//!
//! Positions/lengths are dimensionless model-space quantities; angles radians.

use std::sync::Arc;

use crate::curve::Spline;
use crate::math::Vec3;
use crate::mesh::Mesh;
use crate::selection::Axis;
use crate::transform::Affine3;

/// Optional cap meshes placed at the ends of an array (Array modifier's
/// *Start Cap* / *End Cap*). Each cap is placed with the same offset the next
/// (or previous) copy would have had.
#[derive(Debug, Clone, Default)]
pub struct ArrayCaps {
    /// Placed one offset step *before* the first copy.
    pub start: Option<Arc<Mesh>>,
    /// Placed one offset step *after* the last copy.
    pub end: Option<Arc<Mesh>>,
}

/// Compose two affine maps: `compose(a, b)` applies `b` then `a`.
fn compose(a: Affine3, b: Affine3) -> Affine3 {
    let mut lin = [[0.0; 3]; 3];
    for (i, row) in lin.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = a.linear[i][0] * b.linear[0][j]
                + a.linear[i][1] * b.linear[1][j]
                + a.linear[i][2] * b.linear[2][j];
        }
    }
    Affine3::from_rows(lin, a.transform_point(b.translation))
}

/// A rotation of `angle` radians about the unit-ish `axis` through the origin
/// (Rodrigues), as an [`Affine3`] with no translation.
fn rotation(axis: Vec3, angle: f64) -> Affine3 {
    let k = axis.normalize();
    let (s, c) = angle.sin_cos();
    let (x, y, z) = (k.x, k.y, k.z);
    let t = 1.0 - c;
    Affine3::from_rows(
        [
            [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
            [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
            [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
        ],
        Vec3::ZERO,
    )
}

/// A rotation about `axis` through `center` (translate to origin, rotate,
/// translate back).
fn rotation_about(axis: Vec3, angle: f64, center: Vec3) -> Affine3 {
    let r = rotation(axis, angle);
    compose(
        Affine3::translation(center),
        compose(r, Affine3::translation(center.scale(-1.0))),
    )
}

fn append(pos: &mut Vec<Vec3>, faces: &mut Vec<Vec<usize>>, mesh: &Mesh, xf: Affine3) {
    let base = pos.len();
    for p in mesh.positions() {
        pos.push(xf.transform_point(p));
    }
    for f in mesh.polygons() {
        faces.push(f.iter().map(|v| v.0 + base).collect());
    }
}

/// `count` copies of `mesh`, copy `i` rotated by `i * step_angle` about `axis`
/// through `center`. `count` clamped `>= 1`.
pub fn radial_array(mesh: &Mesh, count: usize, step_angle: f64, axis: Vec3, center: Vec3) -> Mesh {
    radial_array_capped(mesh, count, step_angle, axis, center, &ArrayCaps::default())
}

/// [`radial_array`] with [`ArrayCaps`].
pub fn radial_array_capped(
    mesh: &Mesh,
    count: usize,
    step_angle: f64,
    axis: Vec3,
    center: Vec3,
    caps: &ArrayCaps,
) -> Mesh {
    let n = count.max(1);
    let mut pos = Vec::new();
    let mut faces = Vec::new();
    if let Some(c) = &caps.start {
        append(
            &mut pos,
            &mut faces,
            c,
            rotation_about(axis, -step_angle, center),
        );
    }
    for i in 0..n {
        append(
            &mut pos,
            &mut faces,
            mesh,
            rotation_about(axis, step_angle * i as f64, center),
        );
    }
    if let Some(c) = &caps.end {
        append(
            &mut pos,
            &mut faces,
            c,
            rotation_about(axis, step_angle * n as f64, center),
        );
    }
    Mesh::from_polygons(&pos, &faces)
}

/// `count` copies of `mesh` spread evenly around a full turn about `axis`
/// through `center` (step `= 2π / count`).
pub fn circular_array(mesh: &Mesh, count: usize, axis: Vec3, center: Vec3) -> Mesh {
    let n = count.max(1);
    radial_array(mesh, n, std::f64::consts::TAU / n as f64, axis, center)
}

/// `count` copies of `mesh`, copy `i` placed under `offset` applied `i` times
/// (the Array modifier's *Object Offset*). `count` clamped `>= 1`.
pub fn object_offset_array(mesh: &Mesh, count: usize, offset: Affine3) -> Mesh {
    object_offset_array_capped(mesh, count, offset, &ArrayCaps::default())
}

/// [`object_offset_array`] with [`ArrayCaps`].
pub fn object_offset_array_capped(
    mesh: &Mesh,
    count: usize,
    offset: Affine3,
    caps: &ArrayCaps,
) -> Mesh {
    let n = count.max(1);
    let mut pos = Vec::new();
    let mut faces = Vec::new();

    // offset^-1 for the start cap is not generally available; place the start
    // cap at the identity-minus-one-step position only when `offset` is a pure
    // translation (the common case). Otherwise place it at the identity.
    let inv_step = pure_translation_inverse(offset).unwrap_or(Affine3::IDENTITY);
    if let Some(c) = &caps.start {
        append(&mut pos, &mut faces, c, inv_step);
    }

    let mut acc = Affine3::IDENTITY;
    for _ in 0..n {
        append(&mut pos, &mut faces, mesh, acc);
        acc = compose(offset, acc);
    }
    if let Some(c) = &caps.end {
        append(&mut pos, &mut faces, c, acc);
    }
    Mesh::from_polygons(&pos, &faces)
}

fn pure_translation_inverse(a: Affine3) -> Option<Affine3> {
    let is_ident = (0..3)
        .all(|i| (0..3).all(|j| (a.linear[i][j] - if i == j { 1.0 } else { 0.0 }).abs() < 1e-12));
    is_ident.then(|| Affine3::translation(a.translation.scale(-1.0)))
}

/// `count` copies of `mesh` distributed at even parameter spacing along
/// `spline`, each translated to the sample point and rotated so its local
/// `align_axis` points along the curve tangent (and local +Y toward the curve
/// normal). `count` clamped `>= 1`.
pub fn array_along_curve(mesh: &Mesh, spline: &Spline, count: usize, align_axis: Axis) -> Mesh {
    array_along_curve_capped(mesh, spline, count, align_axis, &ArrayCaps::default())
}

/// [`array_along_curve`] with [`ArrayCaps`] (caps sit at the curve ends,
/// oriented to the end frames).
pub fn array_along_curve_capped(
    mesh: &Mesh,
    spline: &Spline,
    count: usize,
    align_axis: Axis,
    caps: &ArrayCaps,
) -> Mesh {
    let n = count.max(1);
    let frames = spline.sample_with_frames();
    if frames.is_empty() {
        return mesh.clone();
    }
    let sample_at = |u: f64| -> (Vec3, Vec3, Vec3) {
        let x = u.clamp(0.0, 1.0) * (frames.len() - 1) as f64;
        let i = (x.floor() as usize).min(frames.len() - 1);
        let f = &frames[i];
        (f.position, f.tangent, f.normal)
    };
    let place = |pos: &mut Vec<Vec3>, faces: &mut Vec<Vec<usize>>, u: f64, m: &Mesh| {
        let (p, t, nrm) = sample_at(u);
        append(pos, faces, m, frame_transform(align_axis, p, t, nrm));
    };

    let mut pos = Vec::new();
    let mut faces = Vec::new();
    if let Some(c) = &caps.start {
        place(&mut pos, &mut faces, 0.0, c);
    }
    for i in 0..n {
        let u = if n == 1 {
            0.0
        } else {
            i as f64 / (n as f64 - 1.0)
        };
        place(&mut pos, &mut faces, u, mesh);
    }
    if let Some(c) = &caps.end {
        place(&mut pos, &mut faces, 1.0, c);
    }
    Mesh::from_polygons(&pos, &faces)
}

/// A rigid transform putting local `align_axis` onto `tangent`, local +Y onto
/// `normal` (Gram-Schmidt-clean), translated to `position`.
fn frame_transform(align_axis: Axis, position: Vec3, tangent: Vec3, normal: Vec3) -> Affine3 {
    let t = tangent.normalize();
    let n = normal.sub(t.scale(normal.dot(t))).normalize();
    let bi = t.cross(n);
    // Columns of the rotation = images of local ex, ey, ez.
    let (cx, cy, cz) = match align_axis {
        Axis::X => (t, n, bi),
        Axis::Y => (bi, t, n),
        Axis::Z => (n, bi, t),
    };
    Affine3::from_rows(
        [[cx.x, cy.x, cz.x], [cx.y, cy.y, cz.y], [cx.z, cy.z, cz.z]],
        position,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::bounding_box;
    use crate::primitives;

    fn cube_at(size: f64, t: Vec3) -> Mesh {
        let m = primitives::cube(size);
        let positions: Vec<Vec3> = m.positions().iter().map(|p| p.add(t)).collect();
        let faces: Vec<Vec<usize>> = m
            .polygons()
            .iter()
            .map(|f| f.iter().map(|v| v.0).collect())
            .collect();
        Mesh::from_polygons(&positions, &faces)
    }

    #[test]
    fn circular_array_makes_count_copies_on_a_ring() {
        // A unit cube 5 units out on +x, arrayed 6× about z.
        let unit = cube_at(1.0, Vec3::new(5.0, 0.0, 0.0));
        let ring = circular_array(&unit, 6, Vec3::new(0.0, 0.0, 1.0), Vec3::ZERO);
        assert_eq!(ring.vertex_count(), 6 * 8);
        assert_eq!(ring.face_count(), 6 * 6);
        // Copies sit at 0°/60°/…; the 0° and 180° copies put the x-extent at
        // exactly ±5.5, and the ring is symmetric in y (no copy at ±90°, so
        // |y| < 5.5).
        let (lo, hi) = bounding_box(&ring);
        assert!((hi.x - 5.5).abs() < 1e-6 && (lo.x + 5.5).abs() < 1e-6);
        assert!((hi.y + lo.y).abs() < 1e-6 && hi.y < 5.5);
    }

    #[test]
    fn radial_array_partial_sweep() {
        let unit = cube_at(0.5, Vec3::new(3.0, 0.0, 0.0));
        let fan = radial_array(
            &unit,
            4,
            std::f64::consts::FRAC_PI_2 / 3.0,
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::ZERO,
        );
        assert_eq!(fan.vertex_count(), 4 * 8);
        // Last copy rotated 90° total → centred near (0, 3, 0).
        let (_lo, hi) = bounding_box(&fan);
        assert!(hi.y > 3.0 && hi.x > 3.0);
    }

    #[test]
    fn object_offset_translation_matches_linear_array() {
        let unit = primitives::cube(1.0);
        let out = object_offset_array(&unit, 4, Affine3::translation(Vec3::new(2.0, 0.0, 0.0)));
        assert_eq!(out.vertex_count(), 4 * 8);
        let (lo, hi) = bounding_box(&out);
        assert!((lo.x + 0.5).abs() < 1e-9);
        assert!((hi.x - 6.5).abs() < 1e-9, "4 copies stepped by 2");
    }

    #[test]
    fn object_offset_with_rotation_spirals() {
        let unit = cube_at(0.4, Vec3::new(2.0, 0.0, 0.0));
        let step = compose(
            Affine3::translation(Vec3::new(0.0, 0.0, 1.0)),
            rotation(Vec3::new(0.0, 0.0, 1.0), std::f64::consts::FRAC_PI_2),
        );
        let helix = object_offset_array(&unit, 4, step);
        let (lo2, hi2) = bounding_box(&helix);
        assert!((hi2.z - lo2.z) > 3.0, "rose by ~3 in z over 4 copies");
    }

    #[test]
    fn object_offset_caps_on_a_translation() {
        let unit = primitives::cube(1.0);
        let caps = ArrayCaps {
            start: Some(Arc::new(primitives::cube(1.0))),
            end: Some(Arc::new(primitives::cube(1.0))),
        };
        let out = object_offset_array_capped(
            &unit,
            3,
            Affine3::translation(Vec3::new(2.0, 0.0, 0.0)),
            &caps,
        );
        assert_eq!(out.vertex_count(), 5 * 8, "3 copies + 2 caps");
        let (lo, hi) = bounding_box(&out);
        assert!((lo.x + 2.5).abs() < 1e-9, "start cap one step before");
        assert!((hi.x - 6.5).abs() < 1e-9, "end cap one step after");
    }

    #[test]
    fn array_along_curve_follows_a_straight_spline() {
        let unit = primitives::cube(0.5);
        let s = Spline::poly(&[Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)]);
        let out = array_along_curve(&unit, &s, 5, Axis::X);
        assert_eq!(out.vertex_count(), 5 * 8);
        let (lo, hi) = bounding_box(&out);
        assert!(lo.x < 0.3 && hi.x > 9.7, "copies span the curve");
    }

    #[test]
    fn array_along_curve_orients_to_the_tangent() {
        // An anisotropic box: long in local x. Along a curve running in +y,
        // aligning local X to the tangent should make it long in y.
        let bar = {
            let m = primitives::cube(1.0);
            let p: Vec<Vec3> = m
                .positions()
                .iter()
                .map(|q| Vec3::new(q.x * 4.0, q.y, q.z))
                .collect();
            let f: Vec<Vec<usize>> = m
                .polygons()
                .iter()
                .map(|x| x.iter().map(|v| v.0).collect())
                .collect();
            Mesh::from_polygons(&p, &f)
        };
        let s = Spline::poly(&[Vec3::ZERO, Vec3::new(0.0, 12.0, 0.0)]);
        let out = array_along_curve(&bar, &s, 3, Axis::X);
        let (lo, hi) = bounding_box(&out);
        assert!((hi.y - lo.y) > (hi.x - lo.x), "bars now run along y");
    }
}
