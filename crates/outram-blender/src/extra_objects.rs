// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Parametric "extra objects" generators. Blender analogue (architecture only):
// the bundled `add_mesh_extra_objects` add-on (round cube, gears, pipe joints,
// star, honeycomb, Z-function surface, ...) and `add_curve_extra_objects`.
// No upstream source copied — each generator is written from first principles
// and, where the result is a closed solid, checked against Euler's formula.
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

//! **Extra-objects generators** (`op-hzs.54.40`, GH issue #37 §H) — the
//! parametric shapes Blender's *Add Mesh: Extra Objects* add-on provides.
//!
//! - [`rounded_cube`] — a box with filleted edges/corners (rounded-box SDF
//!   projection of a subdivided cube).
//! - [`capsule`] — a cylinder capped by two hemispheres.
//! - [`spur_gear`] — an extruded trapezoidal-tooth spur gear.
//! - [`pipe`] / [`elbow`] — a straight pipe segment and a swept bend, both
//!   hollow (inner + outer wall), via [`crate::revolve`].
//! - [`wedge`] — a right-triangular prism.
//! - [`star`] — an extruded star polygon.
//! - [`honeycomb`] — a hex-cell grid (flat).
//! - [`z_function_surface`] — a grid patch with `z = f(x, y)`.
//!
//! ## Units
//!
//! All radii / lengths are dimensionless model-space quantities; angles are
//! radians; tooth/segment counts are clamped to sane minimums.

use std::f64::consts::{PI, TAU};

use crate::math::Vec3;
use crate::mesh::Mesh;

/// Extrude a closed 2-D outline (CCW, in the `z = 0` plane) into a solid
/// between `z0` and `z1`: two ear-clipped caps plus a side wall.
fn extrude_outline(outline: &[[f64; 2]], z0: f64, z1: f64) -> Mesh {
    let n = outline.len();
    if n < 3 {
        return Mesh::new();
    }
    let mut positions: Vec<Vec3> = Vec::with_capacity(n * 2);
    for &[x, y] in outline {
        positions.push(Vec3::new(x, y, z0));
    }
    for &[x, y] in outline {
        positions.push(Vec3::new(x, y, z1));
    }
    let mut faces: Vec<Vec<usize>> = Vec::new();
    for t in crate::text::ear_clip(outline) {
        faces.push(vec![t[0], t[2], t[1]]); // bottom, facing -z
    }
    for t in crate::text::ear_clip(outline) {
        faces.push(vec![n + t[0], n + t[1], n + t[2]]); // top, facing +z
    }
    for i in 0..n {
        let j = (i + 1) % n;
        faces.push(vec![i, j, n + j, n + i]);
    }
    Mesh::from_polygons(&positions, &faces)
}

/// A box of full extent `size` on each axis with edges and corners rounded to
/// `radius`, built by projecting a `segments`-subdivided cube onto the
/// rounded-box surface.
///
/// `radius` is clamped to `< size/2`; `segments` (per face edge, clamped
/// `>= 2`) controls how finely the fillets are tessellated. Closed genus-0,
/// `chi = 2` (after the shared-corner welding that [`Mesh::from_polygons`]
/// does).
pub fn rounded_cube(size: f64, radius: f64, segments: usize) -> Mesh {
    let h = size * 0.5;
    let r = radius.clamp(0.0, h * 0.999);
    let inner = h - r;
    let s = segments.max(2);

    // Six faces of a subdivided cube, then project each vertex.
    let project = |p: Vec3| -> Vec3 {
        let q = Vec3::new(
            p.x.clamp(-inner, inner),
            p.y.clamp(-inner, inner),
            p.z.clamp(-inner, inner),
        );
        let d = p.sub(q);
        let l = d.length();
        if l < 1e-12 {
            p
        } else {
            q.add(d.scale(r / l))
        }
    };

    let mut positions: Vec<Vec3> = Vec::new();
    let mut faces: Vec<Vec<usize>> = Vec::new();
    // For each of ±x, ±y, ±z faces build an (s+1)x(s+1) grid.
    let axes: [(usize, f64); 6] = [
        (0, 1.0),
        (0, -1.0),
        (1, 1.0),
        (1, -1.0),
        (2, 1.0),
        (2, -1.0),
    ];
    for (ax, sign) in axes {
        let base = positions.len();
        for iu in 0..=s {
            for iv in 0..=s {
                let u = -h + size * iu as f64 / s as f64;
                let v = -h + size * iv as f64 / s as f64;
                let p = match ax {
                    0 => Vec3::new(sign * h, u, v),
                    1 => Vec3::new(u, sign * h, v),
                    _ => Vec3::new(u, v, sign * h),
                };
                positions.push(project(p));
            }
        }
        let at = |iu: usize, iv: usize| base + iu * (s + 1) + iv;
        for iu in 0..s {
            for iv in 0..s {
                // Wind so the normal points along `sign` on this axis.
                let quad = [
                    at(iu, iv),
                    at(iu + 1, iv),
                    at(iu + 1, iv + 1),
                    at(iu, iv + 1),
                ];
                if sign > 0.0 {
                    faces.push(quad.to_vec());
                } else {
                    faces.push(vec![quad[0], quad[3], quad[2], quad[1]]);
                }
            }
        }
    }
    // Face patches meet along the cube edges at coincident projected
    // positions; weld them into one closed shell.
    crate::weld::weld(&Mesh::from_polygons(&positions, &faces), 1e-6)
}

/// A capsule about the `z` axis: a cylinder of `radius` and cylindrical length
/// `length` (the straight part), capped top and bottom by hemispheres of the
/// same `radius`.
///
/// `segments` around the axis (clamped `>= 3`), `rings` per hemisphere
/// (clamped `>= 1`). Total height is `length + 2*radius`. Closed genus-0,
/// `chi = 2`.
pub fn capsule(radius: f64, length: f64, segments: usize, rings: usize) -> Mesh {
    let n = segments.max(3);
    let hr = rings.max(1);
    let hl = length * 0.5;

    // Profile in the x-z plane, bottom pole → top pole.
    let mut profile: Vec<Vec3> = Vec::new();
    for i in 0..=hr {
        let a = -PI / 2.0 + (PI / 2.0) * i as f64 / hr as f64; // -90°..0°
        profile.push(Vec3::new(radius * a.cos(), 0.0, -hl + radius * a.sin()));
    }
    for i in 1..=hr {
        let a = (PI / 2.0) * i as f64 / hr as f64; // 0°..90°
        profile.push(Vec3::new(radius * a.cos(), 0.0, hl + radius * a.sin()));
    }
    let m = crate::revolve::revolve(&profile, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), n, TAU);
    crate::weld::weld(&m, 1e-6)
}

/// An extruded spur gear about the `z` axis with `teeth` trapezoidal teeth.
///
/// `root_radius` is the radius at the tooth root, `tooth_height` the added
/// radial length of each tooth, `width` the extrusion depth along `z`
/// (centred on `z = 0`). `tooth_frac` in `(0, 1)` is the fraction of each
/// angular pitch the tooth tip occupies (`0.5` ≈ equal land/gap). `teeth` is
/// clamped `>= 3`. Closed genus-0 prism, `chi = 2`.
pub fn spur_gear(
    teeth: usize,
    root_radius: f64,
    tooth_height: f64,
    width: f64,
    tooth_frac: f64,
) -> Mesh {
    let z = teeth.max(3);
    let tip = root_radius + tooth_height;
    let frac = tooth_frac.clamp(0.05, 0.9);
    let pitch = TAU / z as f64;
    let tip_half = pitch * frac * 0.5;
    let root_half = pitch * 0.5;

    // Three points per tooth: root land start, then the two tip corners. The
    // next tooth's root-land start closes the gap, so no duplicate points.
    let mut outline: Vec<[f64; 2]> = Vec::with_capacity(z * 3);
    for k in 0..z {
        let c = k as f64 * pitch;
        let pts = [
            (c - root_half, root_radius),
            (c - tip_half, tip),
            (c + tip_half, tip),
        ];
        for (ang, rad) in pts {
            outline.push([rad * ang.cos(), rad * ang.sin()]);
        }
    }
    extrude_outline(&outline, -width * 0.5, width * 0.5)
}

/// A straight hollow pipe about the `z` axis: outer radius `outer`, wall
/// thickness `wall`, length `length` (centred on `z = 0`), `segments` around
/// (clamped `>= 3`). Both ends are open annular rims. Genus-1 (a tube),
/// `chi = 0`.
pub fn pipe(outer: f64, wall: f64, length: f64, segments: usize) -> Mesh {
    let n = segments.max(3);
    let inner = (outer - wall).max(1e-4);
    let hl = length * 0.5;
    // Profile: up the outer wall, across the top rim, down the inner wall,
    // across the bottom rim — a closed rectangle in (r, z), swept.
    let profile = [
        Vec3::new(outer, 0.0, -hl),
        Vec3::new(outer, 0.0, hl),
        Vec3::new(inner, 0.0, hl),
        Vec3::new(inner, 0.0, -hl),
        Vec3::new(outer, 0.0, -hl),
    ];
    crate::revolve::revolve(&profile, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), n, TAU)
}

/// A swept pipe bend ("elbow"): a hollow annular cross-section (outer radius
/// `outer`, wall `wall`) swept along a circular arc of `bend_radius` through
/// `angle` radians.
///
/// The arc lies in the `x-y` plane starting along `+x`; `arc_segments` steps
/// along the bend, `tube_segments` around the section (both clamped `>= 3` /
/// `>= 2`). Open annular ends. Genus-1, `chi = 0`.
pub fn elbow(
    outer: f64,
    wall: f64,
    bend_radius: f64,
    angle: f64,
    arc_segments: usize,
    tube_segments: usize,
) -> Mesh {
    let na = arc_segments.max(3);
    let nt = tube_segments.max(4);
    let inner = (outer - wall).max(1e-4);

    // Section ring in local (right, up) coords: outer ring then inner ring is
    // handled by building a closed 2-wall polygon per station and stitching
    // consecutive stations.
    let ring = |rad: f64| -> Vec<(f64, f64)> {
        (0..nt)
            .map(|i| {
                let a = TAU * i as f64 / nt as f64;
                (rad * a.cos(), rad * a.sin())
            })
            .collect()
    };
    let outer_sec = ring(outer);
    let inner_sec = ring(inner);

    let mut positions: Vec<Vec3> = Vec::new();
    let mut stations: Vec<(usize, usize)> = Vec::new(); // (outer_base, inner_base) per station
    for s in 0..=na {
        let t = angle * s as f64 / na as f64;
        let centre = Vec3::new(bend_radius * t.cos(), bend_radius * t.sin(), 0.0);
        // Frame: tangent along the arc, "up" is +z, "right" points outward radially.
        let right = Vec3::new(t.cos(), t.sin(), 0.0);
        let up = Vec3::new(0.0, 0.0, 1.0);
        let ob = positions.len();
        for &(r, u) in &outer_sec {
            positions.push(centre.add(right.scale(r)).add(up.scale(u)));
        }
        let ib = positions.len();
        for &(r, u) in &inner_sec {
            positions.push(centre.add(right.scale(r)).add(up.scale(u)));
        }
        stations.push((ob, ib));
    }

    let mut faces: Vec<Vec<usize>> = Vec::new();
    for s in 0..na {
        let (o0, i0) = stations[s];
        let (o1, i1) = stations[s + 1];
        for k in 0..nt {
            let k2 = (k + 1) % nt;
            faces.push(vec![o0 + k, o0 + k2, o1 + k2, o1 + k]); // outer wall
            faces.push(vec![i0 + k2, i0 + k, i1 + k, i1 + k2]); // inner wall (flipped)
        }
    }
    // End rims (annulus between outer and inner at first/last station).
    for &(ob, ib) in &[stations[0]] {
        for k in 0..nt {
            let k2 = (k + 1) % nt;
            faces.push(vec![ob + k2, ob + k, ib + k, ib + k2]);
        }
    }
    for &(ob, ib) in &[stations[na]] {
        for k in 0..nt {
            let k2 = (k + 1) % nt;
            faces.push(vec![ob + k, ob + k2, ib + k2, ib + k]);
        }
    }
    Mesh::from_polygons(&positions, &faces)
}

/// A right-triangular prism ("wedge"): the triangle has legs `size_x` (along
/// `+x`) and `size_z` (along `+z`) with the right angle at the origin;
/// extruded `size_y` along `+y`. Closed genus-0, `chi = 2`.
pub fn wedge(size_x: f64, size_y: f64, size_z: f64) -> Mesh {
    let p = vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(size_x, 0.0, 0.0),
        Vec3::new(0.0, 0.0, size_z),
        Vec3::new(0.0, size_y, 0.0),
        Vec3::new(size_x, size_y, 0.0),
        Vec3::new(0.0, size_y, size_z),
    ];
    let faces = vec![
        vec![0usize, 2, 1], // y = 0 triangle (facing -y)
        vec![3, 4, 5],      // y = size_y triangle (facing +y)
        vec![0, 1, 4, 3],   // bottom (z = 0)
        vec![0, 3, 5, 2],   // back (x = 0)
        vec![1, 2, 5, 4],   // hypotenuse face
    ];
    Mesh::from_polygons(&p, &faces)
}

/// An extruded star polygon in the `z = 0` plane: `points` spikes alternating
/// between `outer_radius` and `inner_radius`, extruded `depth` along `z`
/// (centred). `points` clamped `>= 2`. `depth = 0` gives the flat filled
/// outline. Closed genus-0 for `depth > 0`.
pub fn star(points: usize, outer_radius: f64, inner_radius: f64, depth: f64) -> Mesh {
    let p = points.max(2);
    let mut outline: Vec<[f64; 2]> = Vec::with_capacity(p * 2);
    for i in 0..(2 * p) {
        let a = PI * i as f64 / p as f64 - PI / 2.0;
        let r = if i % 2 == 0 {
            outer_radius
        } else {
            inner_radius
        };
        outline.push([r * a.cos(), r * a.sin()]);
    }
    if depth <= 0.0 {
        let positions: Vec<Vec3> = outline.iter().map(|&[x, y]| Vec3::new(x, y, 0.0)).collect();
        let faces: Vec<Vec<usize>> = crate::text::ear_clip(&outline)
            .into_iter()
            .map(|t| t.to_vec())
            .collect();
        return Mesh::from_polygons(&positions, &faces);
    }
    extrude_outline(&outline, -depth * 0.5, depth * 0.5)
}

/// A flat honeycomb: `rows` x `cols` pointy-top hexagonal cells of
/// circumradius `cell_radius` in the `z = 0` plane, each cell a single 6-gon
/// face, packed on the standard offset hex lattice.
///
/// Returns one face per cell (`rows * cols` faces); shared cell edges are
/// deduplicated by [`Mesh::from_polygons`].
pub fn honeycomb(rows: usize, cols: usize, cell_radius: f64) -> Mesh {
    let r = cell_radius;
    let w = 3.0_f64.sqrt() * r; // flat-to-flat width (pointy-top)
    let vspace = 1.5 * r;
    let mut positions: Vec<Vec3> = Vec::new();
    let mut faces: Vec<Vec<usize>> = Vec::new();
    for row in 0..rows.max(1) {
        for col in 0..cols.max(1) {
            let cx = col as f64 * w + if row % 2 == 1 { w * 0.5 } else { 0.0 };
            let cy = row as f64 * vspace;
            let base = positions.len();
            for k in 0..6 {
                let a = PI / 180.0 * (60.0 * k as f64 - 90.0);
                positions.push(Vec3::new(cx + r * a.cos(), cy + r * a.sin(), 0.0));
            }
            faces.push((base..base + 6).collect());
        }
    }
    Mesh::from_polygons(&positions, &faces)
}

/// A grid patch over `[-extent_x, extent_x] x [-extent_y, extent_y]` in the
/// `x-y` plane with height `z = f(x, y)`, tessellated `nx` by `ny` quads
/// (each clamped `>= 1`).
///
/// `f` is any `Fn(f64, f64) -> f64` (a plain generic — no trait object), so
/// callers pass a closure. A topological disc, `chi = 1`.
pub fn z_function_surface<F: Fn(f64, f64) -> f64>(
    nx: usize,
    ny: usize,
    extent_x: f64,
    extent_y: f64,
    f: F,
) -> Mesh {
    let (nx, ny) = (nx.max(1), ny.max(1));
    let mut positions: Vec<Vec3> = Vec::with_capacity((nx + 1) * (ny + 1));
    for iy in 0..=ny {
        for ix in 0..=nx {
            let x = -extent_x + 2.0 * extent_x * ix as f64 / nx as f64;
            let y = -extent_y + 2.0 * extent_y * iy as f64 / ny as f64;
            positions.push(Vec3::new(x, y, f(x, y)));
        }
    }
    let at = |ix: usize, iy: usize| iy * (nx + 1) + ix;
    let mut faces: Vec<Vec<usize>> = Vec::with_capacity(nx * ny);
    for iy in 0..ny {
        for ix in 0..nx {
            faces.push(vec![
                at(ix, iy),
                at(ix + 1, iy),
                at(ix + 1, iy + 1),
                at(ix, iy + 1),
            ]);
        }
    }
    Mesh::from_polygons(&positions, &faces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::bounding_box;

    #[test]
    fn rounded_cube_is_closed_and_within_the_box() {
        let m = rounded_cube(2.0, 0.4, 3);
        assert_eq!(m.euler_characteristic(), 2);
        let (lo, hi) = bounding_box(&m);
        assert!(hi.x <= 1.0 + 1e-9 && lo.x >= -1.0 - 1e-9);
        assert!((hi.x - 1.0).abs() < 1e-9, "still reaches the face centres");
    }

    #[test]
    fn rounded_cube_zero_radius_is_a_plain_box() {
        let m = rounded_cube(2.0, 0.0, 2);
        assert_eq!(m.euler_characteristic(), 2);
        let (lo, hi) = bounding_box(&m);
        assert!((hi.x - 1.0).abs() < 1e-9 && (lo.z + 1.0).abs() < 1e-9);
    }

    #[test]
    fn capsule_is_a_closed_solid_of_the_right_height() {
        let m = capsule(0.5, 2.0, 16, 4);
        assert_eq!(m.euler_characteristic(), 2);
        let (lo, hi) = bounding_box(&m);
        assert!((hi.z - lo.z - 3.0).abs() < 1e-6, "length + 2*radius");
        assert!((hi.x - 0.5).abs() < 1e-6);
    }

    #[test]
    fn spur_gear_is_a_closed_prism_with_tooth_relief() {
        let m = spur_gear(12, 1.0, 0.3, 0.5, 0.5);
        assert_eq!(m.euler_characteristic(), 2);
        let (lo, hi) = bounding_box(&m);
        assert!((hi.z - lo.z - 0.5).abs() < 1e-9);
        // Some vertices out at the tip radius, some in at the root.
        let maxr = (0..m.vertex_count())
            .map(|i| {
                let p = m.vertex(crate::mesh::VertexId(i)).unwrap().position;
                (p.x * p.x + p.y * p.y).sqrt()
            })
            .fold(0.0_f64, f64::max);
        assert!(
            (maxr - 1.3).abs() < 1e-9,
            "tip radius = root + tooth_height"
        );
    }

    #[test]
    fn pipe_is_a_hollow_tube() {
        let m = pipe(1.0, 0.2, 2.0, 24);
        assert_eq!(m.euler_characteristic(), 0, "genus-1 open tube");
        let (lo, hi) = bounding_box(&m);
        assert!((hi.z - lo.z - 2.0).abs() < 1e-9);
    }

    #[test]
    fn elbow_bends_through_the_arc() {
        let m = elbow(0.5, 0.1, 3.0, PI / 2.0, 12, 12);
        assert!(m.face_count() > 40);
        let (_lo, hi) = bounding_box(&m);
        // Quarter bend from +x start to +y end: spans roughly bend_radius in both.
        assert!(hi.x > 3.0 && hi.y > 3.0);
    }

    #[test]
    fn wedge_is_a_closed_triangular_prism() {
        let m = wedge(2.0, 1.0, 1.5);
        assert_eq!(m.vertex_count(), 6);
        assert_eq!(m.face_count(), 5);
        assert_eq!(m.euler_characteristic(), 2);
    }

    #[test]
    fn star_flat_then_extruded() {
        let flat = star(5, 1.0, 0.4, 0.0);
        assert!(flat.face_count() > 0);
        assert_eq!(flat.euler_characteristic(), 1, "flat disc");
        let solid = star(5, 1.0, 0.4, 0.3);
        assert_eq!(solid.euler_characteristic(), 2, "extruded solid");
    }

    #[test]
    fn honeycomb_has_one_face_per_cell() {
        let m = honeycomb(3, 4, 0.5);
        assert_eq!(m.face_count(), 12);
        for f in 0..m.face_count() {
            assert_eq!(m.face(crate::mesh::FaceId(f)).unwrap().len, 6);
        }
    }

    #[test]
    fn z_function_surface_takes_a_closure() {
        let m = z_function_surface(8, 8, 2.0, 2.0, |x, y| 0.5 * (x * x + y * y).sqrt().sin());
        assert_eq!(m.vertex_count(), 81);
        assert_eq!(m.face_count(), 64);
        assert_eq!(m.euler_characteristic(), 1);
        // Height matches the function at a sample corner.
        let corner = m.vertex(crate::mesh::VertexId(0)).unwrap().position;
        let expect = 0.5 * ((corner.x * corner.x + corner.y * corner.y).sqrt()).sin();
        assert!((corner.z - expect).abs() < 1e-9);
    }
}
