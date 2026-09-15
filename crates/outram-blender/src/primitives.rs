// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Closed-form generators for a cube, UV sphere, cylinder and grid, validated
// against Euler's polyhedron formula V - E + F = chi (L. Euler, 1758). No named
// published algorithm and no upstream source — written from first principles.
// Blender analogue (architecture only): the editmesh_add "Add Mesh" operators.
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

//! **Real** mesh primitive generators (Blender's "Add Mesh" primitives).
//!
//! This is the one module in the crate with fully implemented, unit-tested
//! algorithms — the analogue of Blender's `editors/mesh/editmesh_add.cc` add
//! operators (Add Cube / UV Sphere / Cylinder / Grid). Each function returns a
//! valid [`Mesh`] built entirely through the public [`Mesh::add_vertex`] /
//! [`Mesh::add_face`] API, so edge deduplication and loop wiring come for free.
//!
//! ## Correctness checks
//!
//! The generators are validated against **Euler's polyhedron formula**
//! `V - E + F = chi`:
//!
//! - closed genus-0 solids ([`cube`], [`uv_sphere`], [`cylinder`]) give
//!   `chi = 2`;
//! - a flat [`grid`] patch is a topological disc, `chi = 1`.
//!
//! These are exact topological identities, so the tests assert them exactly
//! (no tolerance). Vertex/edge/face counts are also checked against the closed
//! forms derived in each function's doc comment.
//!
//! ## Units
//!
//! All size/radius/height arguments are dimensionless model-space lengths (see
//! [`crate::math`]); a length unit is attached only when a mesh is handed to a
//! solver through an [`crate::export`] bridge.

use crate::math::Vec3;
use crate::mesh::{Mesh, VertexId};
use std::f64::consts::PI;

/// An axis-aligned cube of side `size`, centred on the origin.
///
/// Topology: **8** vertices, **12** edges, **6** quad faces, `chi = 2`. Faces
/// are wound counter-clockwise as seen from outside, so face normals point
/// outward. `size` is the full edge length (a `size` of `1.0` spans `-0.5..0.5`
/// on each axis).
pub fn cube(size: f64) -> Mesh {
    let h = size * 0.5;
    let mut m = Mesh::new();
    // 8 corners; index layout: bit0=+x, bit1=+y, bit2=+z is *not* used — the
    // explicit table below is easier to reason about for winding.
    let v = [
        m.add_vertex(Vec3::new(-h, -h, -h)), // 0
        m.add_vertex(Vec3::new(h, -h, -h)),  // 1
        m.add_vertex(Vec3::new(h, h, -h)),   // 2
        m.add_vertex(Vec3::new(-h, h, -h)),  // 3
        m.add_vertex(Vec3::new(-h, -h, h)),  // 4
        m.add_vertex(Vec3::new(h, -h, h)),   // 5
        m.add_vertex(Vec3::new(h, h, h)),    // 6
        m.add_vertex(Vec3::new(-h, h, h)),   // 7
    ];
    // Six quads, each wound CCW when viewed from outside (outward normal).
    m.add_face(&[v[0], v[3], v[2], v[1]]); // -Z bottom
    m.add_face(&[v[4], v[5], v[6], v[7]]); // +Z top
    m.add_face(&[v[0], v[1], v[5], v[4]]); // -Y front
    m.add_face(&[v[3], v[7], v[6], v[2]]); // +Y back
    m.add_face(&[v[0], v[4], v[7], v[3]]); // -X left
    m.add_face(&[v[1], v[2], v[6], v[5]]); // +X right
    m
}

/// A UV (latitude/longitude) sphere of the given `radius`.
///
/// `segments` is the number of subdivisions **around** the polar axis
/// (longitude), `rings` the number of subdivisions **from pole to pole**
/// (latitude). Both must be `>= 3` / `>= 2` respectively for a sensible sphere;
/// this matches Blender's Add-UV-Sphere defaults of 32 segments and 16 rings.
///
/// Topology (closed, genus-0, `chi = 2`): **2 poles + (rings - 1) * segments**
/// vertices; the two pole caps are triangle fans (`segments` triangles each)
/// and the `rings - 2` intermediate bands are quad strips
/// (`(rings - 2) * segments` quads). Faces are wound counter-clockwise as seen
/// from outside, so face normals point **outward** (positive enclosed volume) —
/// consistent with [`cube`] and [`cylinder`].
///
/// # Panics
///
/// Panics if `segments < 3` or `rings < 2` — too few to close the surface.
pub fn uv_sphere(segments: usize, rings: usize, radius: f64) -> Mesh {
    assert!(
        segments >= 3,
        "uv_sphere needs segments >= 3, got {segments}"
    );
    assert!(rings >= 2, "uv_sphere needs rings >= 2, got {rings}");
    let mut m = Mesh::new();

    let north = m.add_vertex(Vec3::new(0.0, 0.0, radius));
    let south = m.add_vertex(Vec3::new(0.0, 0.0, -radius));

    // Latitude circles j = 1..rings-1 (excluding the two poles at j=0, j=rings).
    // ring_verts[j-1][i] = vertex at latitude band j, longitude i.
    let mut ring_verts: Vec<Vec<VertexId>> = Vec::with_capacity(rings - 1);
    for j in 1..rings {
        let theta = PI * (j as f64) / (rings as f64); // polar angle from +Z
        let (st, ct) = (theta.sin(), theta.cos());
        let mut row = Vec::with_capacity(segments);
        for i in 0..segments {
            let phi = 2.0 * PI * (i as f64) / (segments as f64);
            row.push(m.add_vertex(Vec3::new(
                radius * st * phi.cos(),
                radius * st * phi.sin(),
                radius * ct,
            )));
        }
        ring_verts.push(row);
    }

    let top = &ring_verts[0];
    // North cap: fan of triangles wound so the outward normal points up (+Z).
    // Seen from above (down the +Z axis) the loop pole -> top[i] -> top[i+1]
    // runs counter-clockwise, giving a +Z (outward) normal.
    for i in 0..segments {
        let a = top[i];
        let b = top[(i + 1) % segments];
        m.add_face(&[north, a, b]);
    }
    // South cap: mirror winding so the outward normal points down (-Z).
    let bottom = &ring_verts[rings - 2];
    for i in 0..segments {
        let a = bottom[i];
        let b = bottom[(i + 1) % segments];
        m.add_face(&[south, b, a]);
    }
    // Intermediate quad bands between latitude j (upper) and j+1 (lower), wound
    // so the outward normal points radially outward.
    for j in 0..(rings - 2) {
        for i in 0..segments {
            let i1 = (i + 1) % segments;
            let a = ring_verts[j][i];
            let b = ring_verts[j][i1];
            let c = ring_verts[j + 1][i1];
            let d = ring_verts[j + 1][i];
            m.add_face(&[a, d, c, b]);
        }
    }
    m
}

/// A closed cylinder of the given `radius` and `height`, axis along Z.
///
/// `segments` is the number of sides around the axis (Blender's "Vertices"
/// field, default 32). The caps are single n-gon faces (Blender's "Ngon"
/// cap-fill), not triangle fans.
///
/// Topology (closed, genus-0, `chi = 2`): **2 * segments** vertices,
/// **3 * segments** edges, **segments + 2** faces (`segments` side quads plus
/// two `segments`-gon caps). The cylinder spans `-height/2 .. height/2` on Z.
///
/// # Panics
///
/// Panics if `segments < 3` — too few sides to enclose a volume.
pub fn cylinder(segments: usize, radius: f64, height: f64) -> Mesh {
    assert!(
        segments >= 3,
        "cylinder needs segments >= 3, got {segments}"
    );
    let mut m = Mesh::new();
    let hz = height * 0.5;

    let mut bottom = Vec::with_capacity(segments);
    let mut top = Vec::with_capacity(segments);
    for i in 0..segments {
        let phi = 2.0 * PI * (i as f64) / (segments as f64);
        let (x, y) = (radius * phi.cos(), radius * phi.sin());
        bottom.push(m.add_vertex(Vec3::new(x, y, -hz)));
        top.push(m.add_vertex(Vec3::new(x, y, hz)));
    }

    // Side quads, wound CCW when viewed from outside.
    for i in 0..segments {
        let i1 = (i + 1) % segments;
        m.add_face(&[bottom[i], bottom[i1], top[i1], top[i]]);
    }
    // Top cap: CCW seen from +Z (outward normal +Z).
    m.add_face(&top);
    // Bottom cap: reversed so its outward normal points -Z.
    let mut bottom_rev = bottom.clone();
    bottom_rev.reverse();
    m.add_face(&bottom_rev);
    m
}

/// A flat rectangular grid (subdivided plane) in the Z = 0 plane.
///
/// `nx` and `ny` are the number of quad subdivisions along X and Y (each `>= 1`);
/// `size` is the full side length (the patch spans `-size/2 .. size/2` on both
/// axes). This is Blender's Add-Grid primitive.
///
/// Topology (a topological **disc**, `chi = 1`): **(nx + 1) * (ny + 1)**
/// vertices and **nx * ny** quad faces. Being an open patch (it has a boundary),
/// it is *not* closed — that is the intended difference from [`cube`] /
/// [`cylinder`] / [`uv_sphere`], and its Euler characteristic is 1, not 2.
///
/// # Panics
///
/// Panics if `nx < 1` or `ny < 1`.
pub fn grid(nx: usize, ny: usize, size: f64) -> Mesh {
    assert!(
        nx >= 1 && ny >= 1,
        "grid needs nx >= 1 and ny >= 1, got {nx}x{ny}"
    );
    let mut m = Mesh::new();
    let half = size * 0.5;
    let dx = size / nx as f64;
    let dy = size / ny as f64;

    // (nx+1) x (ny+1) lattice of vertices, row-major in i (X), j (Y).
    let idx = |i: usize, j: usize| i * (ny + 1) + j;
    let mut verts = Vec::with_capacity((nx + 1) * (ny + 1));
    for i in 0..=nx {
        for j in 0..=ny {
            verts.push(m.add_vertex(Vec3::new(-half + dx * i as f64, -half + dy * j as f64, 0.0)));
        }
    }

    for i in 0..nx {
        for j in 0..ny {
            m.add_face(&[
                verts[idx(i, j)],
                verts[idx(i + 1, j)],
                verts[idx(i + 1, j + 1)],
                verts[idx(i, j + 1)],
            ]);
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_counts_and_euler() {
        let c = cube(2.0);
        assert_eq!(c.vertex_count(), 8);
        assert_eq!(c.edge_count(), 12);
        assert_eq!(c.face_count(), 6);
        assert_eq!(c.euler_characteristic(), 2);
    }

    #[test]
    fn uv_sphere_counts_and_euler() {
        let segments = 16;
        let rings = 8;
        let s = uv_sphere(segments, rings, 1.0);
        assert_eq!(s.vertex_count(), 2 + (rings - 1) * segments);
        // segments + segments caps + (rings-2)*segments quads.
        assert_eq!(s.face_count(), 2 * segments + (rings - 2) * segments);
        assert_eq!(
            s.euler_characteristic(),
            2,
            "closed sphere must have chi = 2"
        );
    }

    #[test]
    fn uv_sphere_default_resolution_is_closed() {
        // Blender defaults: 32 segments, 16 rings.
        let s = uv_sphere(32, 16, 3.0);
        assert_eq!(s.euler_characteristic(), 2);
    }

    #[test]
    fn cylinder_counts_and_euler() {
        let segments = 12;
        let cy = cylinder(segments, 1.0, 2.0);
        assert_eq!(cy.vertex_count(), 2 * segments);
        assert_eq!(cy.edge_count(), 3 * segments);
        assert_eq!(cy.face_count(), segments + 2);
        assert_eq!(
            cy.euler_characteristic(),
            2,
            "closed cylinder must have chi = 2"
        );
    }

    /// Signed volume via the divergence theorem: `(1/6) sum v0 . (v1 x v2)`
    /// over fan-triangulated faces. Positive for an outward-wound closed
    /// surface. Regression guard for the `uv_sphere` inward-winding bug (bead
    /// op-hzs.14): every closed generator must be outward-oriented (positive),
    /// and consistent with each other, so a downstream boolean / classifier
    /// never sees a mixed-orientation input.
    fn signed_volume(m: &Mesh) -> f64 {
        let ps = m.positions();
        let mut vol = 0.0;
        for face in m.polygons() {
            let v0 = ps[face[0].0];
            for k in 1..face.len() - 1 {
                vol += v0.dot(ps[face[k].0].cross(ps[face[k + 1].0]));
            }
        }
        vol / 6.0
    }

    /// Methodology: every closed primitive must be **outward**-wound, i.e. its
    /// signed volume must be positive and match the analytic enclosed volume.
    /// `cube(2.0)` = `2^3 = 8`; `cylinder(seg, 1, 2)` -> `pi r^2 h = 2*pi`
    /// approximated by a `seg`-gon prism (a bit under); `uv_sphere` -> a bit
    /// under `4/3 pi r^3`. Pass criterion: all strictly positive, cube exact,
    /// the faceted ones within their polygonal-approximation band.
    ///
    /// Results (asserted below): cube `= 8`; cylinder and sphere strictly
    /// positive and within the faceted band of their analytic volumes.
    #[test]
    fn closed_primitives_are_outward_wound() {
        assert!(
            (signed_volume(&cube(2.0)) - 8.0).abs() < 1e-9,
            "cube volume must be exactly 8"
        );

        let cyl = signed_volume(&cylinder(64, 1.0, 2.0));
        let cyl_analytic = 2.0 * PI; // pi * 1^2 * 2
        assert!(
            cyl > 0.0,
            "cylinder must be outward-wound (positive volume), got {cyl}"
        );
        assert!(
            cyl < cyl_analytic && cyl > 0.98 * cyl_analytic,
            "cylinder volume {cyl} out of band"
        );

        let sph = signed_volume(&uv_sphere(64, 32, 1.0));
        let sph_analytic = 4.0 / 3.0 * PI; // r = 1
        assert!(
            sph > 0.0,
            "uv_sphere must be outward-wound (positive volume), got {sph}"
        );
        assert!(
            sph < sph_analytic && sph > 0.98 * sph_analytic,
            "sphere volume {sph} out of band"
        );
    }

    #[test]
    fn grid_is_a_disc() {
        let (nx, ny) = (4, 3);
        let g = grid(nx, ny, 1.0);
        assert_eq!(g.vertex_count(), (nx + 1) * (ny + 1));
        assert_eq!(g.face_count(), nx * ny);
        assert_eq!(
            g.euler_characteristic(),
            1,
            "flat grid patch is a disc, chi = 1"
        );
    }
}
