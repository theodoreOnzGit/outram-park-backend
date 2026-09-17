// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Provenance: original OUTRAM PARK code. Not derived from any upstream project
// — elementary parametric triangulations (axis-aligned box, UV sphere, capped
// Z-cylinder) plus the divergence-theorem enclosed-volume formula, all standard
// textbook constructions written from scratch so the crate can generate its own
// test and reactor geometry with no dependency on a surface-authoring crate.
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

//! Closed **triangle-soup surface generators** for test and reactor geometry.
//!
//! The carver ([`crate::carve`]) takes a triangle soup (`points` + `tris`); this
//! module produces watertight, **outward-wound** soups for the common shapes a
//! reactor model is built from — a domain [`box_surface`], a spherical
//! [`sphere_surface`] (a pebble / particle), and a [`cylinder_surface`] (a fuel
//! pin / channel) — with no dependency on the surface-authoring crate. Combined
//! with [`crate::carve::carve_region`] they build "coolant around a pebble" and
//! similar geometries directly.
//!
//! [`surface_volume`] returns the enclosed volume of any outward-wound soup (the
//! divergence formula), used to sanity-check both these generators and any
//! imported surface. Pure Rust, no dependencies.

use crate::math::Vec3;
use std::f64::consts::PI;

/// A triangle-soup surface: point positions and triangle vertex-index triples.
pub type TriSoup = (Vec<Vec3>, Vec<[usize; 3]>);

/// Flip any triangle whose normal points toward the soup's centroid, so a
/// **convex** soup ends up consistently outward-wound. Idempotent on an
/// already-outward soup.
fn orient_outward(points: &[Vec3], tris: &mut [[usize; 3]]) {
    let mut c = Vec3::ZERO;
    for p in points {
        c = c.add(*p);
    }
    let c = c.scale(1.0 / points.len() as f64);
    for t in tris.iter_mut() {
        let (a, b, cc) = (points[t[0]], points[t[1]], points[t[2]]);
        let n = b.sub(a).cross(cc.sub(a));
        let tc = a.add(b).add(cc).scale(1.0 / 3.0);
        if n.dot(tc.sub(c)) < 0.0 {
            t.swap(1, 2);
        }
    }
}

/// Enclosed volume of an outward-wound closed triangle soup,
/// `V = (1/6) Σ v0 · (v1 × v2)` (the divergence theorem). Positive for outward
/// winding; its sign also reveals an inward-wound surface.
pub fn surface_volume(points: &[Vec3], tris: &[[usize; 3]]) -> f64 {
    let mut v6 = 0.0;
    for t in tris {
        v6 += points[t[0]].dot(points[t[1]].cross(points[t[2]]));
    }
    v6 / 6.0
}

/// Axis-aligned box `[min, max]` as 8 corners + 12 outward triangles.
pub fn box_surface(min: Vec3, max: Vec3) -> TriSoup {
    let v = vec![
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
    ];
    // Each face as two triangles, wound CCW seen from outside.
    let quad = |a: usize, b: usize, c: usize, d: usize| vec![[a, b, c], [a, c, d]];
    let mut t = Vec::new();
    for f in [
        quad(0, 3, 2, 1), // −Z
        quad(4, 5, 6, 7), // +Z
        quad(0, 1, 5, 4), // −Y
        quad(2, 3, 7, 6), // +Y
        quad(1, 2, 6, 5), // +X
        quad(0, 4, 7, 3), // −X
    ] {
        t.extend(f);
    }
    orient_outward(&v, &mut t);
    (v, t)
}

/// UV sphere of `radius` about `centre` — `n_lat` latitude bands (pole to pole),
/// `n_lon` longitude segments. Outward-wound; a triangulated pebble/particle.
///
/// # Panics
///
/// If `n_lat < 2` or `n_lon < 3`.
pub fn sphere_surface(centre: Vec3, radius: f64, n_lat: usize, n_lon: usize) -> TriSoup {
    assert!(n_lat >= 2 && n_lon >= 3, "need n_lat >= 2 and n_lon >= 3");
    let mut pts = Vec::new();
    // North pole, interior rings, south pole.
    pts.push(centre.add(Vec3::new(0.0, 0.0, radius)));
    for i in 1..n_lat {
        let theta = PI * i as f64 / n_lat as f64; // 0..π
        let (st, ct) = (theta.sin(), theta.cos());
        for j in 0..n_lon {
            let phi = 2.0 * PI * j as f64 / n_lon as f64;
            pts.push(centre.add(Vec3::new(
                radius * st * phi.cos(),
                radius * st * phi.sin(),
                radius * ct,
            )));
        }
    }
    pts.push(centre.add(Vec3::new(0.0, 0.0, -radius)));
    let south = pts.len() - 1;
    let ring = |i: usize, j: usize| 1 + (i - 1) * n_lon + (j % n_lon); // i in 1..n_lat

    let mut tris = Vec::new();
    // North cap fan.
    for j in 0..n_lon {
        tris.push([0, ring(1, j), ring(1, j + 1)]);
    }
    // Middle bands.
    for i in 1..n_lat - 1 {
        for j in 0..n_lon {
            let a = ring(i, j);
            let b = ring(i, j + 1);
            let c = ring(i + 1, j + 1);
            let d = ring(i + 1, j);
            tris.push([a, b, c]);
            tris.push([a, c, d]);
        }
    }
    // South cap fan.
    for j in 0..n_lon {
        tris.push([south, ring(n_lat - 1, j + 1), ring(n_lat - 1, j)]);
    }
    orient_outward(&pts, &mut tris);
    (pts, tris)
}

/// Z-axis cylinder of `radius` and `height` with its base centred at `base` —
/// `n_seg` circumferential segments, capped both ends. Outward-wound; a
/// triangulated fuel pin / channel.
///
/// # Panics
///
/// If `n_seg < 3`.
pub fn cylinder_surface(base: Vec3, radius: f64, height: f64, n_seg: usize) -> TriSoup {
    assert!(n_seg >= 3, "need n_seg >= 3");
    let mut pts = Vec::new();
    let bottom_centre = 0; // index
    pts.push(base);
    let top_centre = 1;
    pts.push(base.add(Vec3::new(0.0, 0.0, height)));
    // Rim rings: bottom then top.
    for j in 0..n_seg {
        let phi = 2.0 * PI * j as f64 / n_seg as f64;
        pts.push(base.add(Vec3::new(radius * phi.cos(), radius * phi.sin(), 0.0)));
    }
    for j in 0..n_seg {
        let phi = 2.0 * PI * j as f64 / n_seg as f64;
        pts.push(base.add(Vec3::new(radius * phi.cos(), radius * phi.sin(), height)));
    }
    let b = |j: usize| 2 + (j % n_seg);
    let t = |j: usize| 2 + n_seg + (j % n_seg);

    let mut tris = Vec::new();
    for j in 0..n_seg {
        // Bottom cap (outward = −Z): wind so the normal points down.
        tris.push([bottom_centre, b(j + 1), b(j)]);
        // Top cap (outward = +Z).
        tris.push([top_centre, t(j), t(j + 1)]);
        // Side quad (outward radial).
        tris.push([b(j), b(j + 1), t(j + 1)]);
        tris.push([b(j), t(j + 1), t(j)]);
    }
    orient_outward(&pts, &mut tris);
    (pts, tris)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carve::carve_region;

    /// V&V — the box soup has exactly the box volume and outward winding.
    #[test]
    fn box_volume_exact() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
        assert!(
            (surface_volume(&p, &t) - 8.0).abs() < 1e-12,
            "outward box volume 8"
        );
    }

    /// V&V — the UV sphere encloses ~(4/3)πr³. Methodology: a radius-1 sphere,
    /// 24 lat × 48 lon. Result: enclosed volume within 1 % of 4.18879
    /// (UV discretisation slightly under-fills), and positive (outward-wound).
    #[test]
    fn sphere_volume_approximates_analytic() {
        let (p, t) = sphere_surface(Vec3::ZERO, 1.0, 24, 48);
        let exact = 4.0 / 3.0 * PI;
        let rel = (surface_volume(&p, &t) - exact).abs() / exact;
        assert!(rel < 0.01, "sphere volume within 1% of {exact} (rel {rel})");
    }

    /// V&V — the cylinder encloses ~πr²h. Methodology: r = 1, h = 2, 64
    /// segments. Result: within 1 % of 2π ≈ 6.2832, outward-wound.
    #[test]
    fn cylinder_volume_approximates_analytic() {
        let (p, t) = cylinder_surface(Vec3::ZERO, 1.0, 2.0, 64);
        let exact = PI * 1.0 * 1.0 * 2.0;
        let rel = (surface_volume(&p, &t) - exact).abs() / exact;
        assert!(
            rel < 0.01,
            "cylinder volume within 1% of {exact} (rel {rel})"
        );
    }

    /// V&V — reactor pattern: mesh the coolant region around a pebble. Domain
    /// box [−2,2]³ (volume 64) minus a radius-1 sphere (volume ≈ 4.19) carved at
    /// cell size 0.1. Result: a valid closed mesh whose volume ≈ 64 − 4.19 ≈
    /// 59.8 (within a few % — staircase on the sphere), demonstrating
    /// coolant-around-fuel meshing with no blender dependency.
    #[test]
    fn coolant_around_a_pebble() {
        let (op, ot) = box_surface(Vec3::new(-2.0, -2.0, -2.0), Vec3::new(2.0, 2.0, 2.0));
        let (sp, st) = sphere_surface(Vec3::ZERO, 1.0, 16, 32);
        let m = carve_region(&op, &ot, &[(&sp[..], &st[..])], 0.1);
        m.validate().expect("coolant region cells are closed");
        let expect = 64.0 - 4.0 / 3.0 * PI;
        let rel = (m.total_volume() - expect).abs() / expect;
        assert!(
            rel < 0.03,
            "coolant volume {} within 3% of {expect} (rel {rel})",
            m.total_volume()
        );
        assert!(m.cell_count() > 10_000, "a resolved domain has many cells");
    }
}
