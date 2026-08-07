// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Shell construction by offsetting a surface along its vertex normals and bridging
// the boundary with a rim. No named published algorithm — written from first
// principles; no upstream source was copied. Blender analogue (architecture only):
// the Solidify modifier, Simple mode.
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

//! Solidify — give a surface thickness by extruding it into a closed shell.
//!
//! This is the pure-Rust analogue of Blender's **Solidify** modifier
//! (`MOD_solidify`, simple mode): every vertex is duplicated and offset inward
//! along its area-weighted vertex normal by `thickness`, the offset copy forms
//! an **inner shell** with reversed winding (so its normals face the cavity),
//! and any open boundary edge is bridged to the inner shell by a **rim quad**.
//!
//! The result depends on whether the input is open or closed:
//!
//! - An **open** surface (a grid, a patch, a surface with holes) becomes a
//!   **closed solid slab** of the given thickness — the use case for turning an
//!   authored boundary surface into something with volume (e.g. a CFD wall).
//! - A **closed** surface (a cube, a sphere) becomes a **hollow shell** — the
//!   original outer surface plus a nested, inward-offset inner surface, with a
//!   cavity between them.
//!
//! # Winding
//!
//! The outer shell keeps the input winding (outward normals). The inner shell
//! reverses it, so its normals point into the cavity (away from the solid
//! material). Each rim quad traverses the shared boundary edge opposite to the
//! outer face that owns it, keeping the whole result consistently wound — the
//! same adjacent-faces-oppose rule used in [`crate::fill_holes`].
//!
//! # Degenerate normals
//!
//! A vertex whose incident face normals cancel gets a zero normal (via
//! [`crate::math::Vec3::normalize`], which returns zero rather than `NaN`), so
//! its inner copy coincides with the outer vertex instead of flying off to a
//! non-finite coordinate. No `faer`, no external dependency; Android-safe.

use crate::math::Vec3;
use crate::mesh::Mesh;
use std::collections::HashSet;

/// Extrude `mesh` into a closed shell of the given `thickness`, returning the
/// solidified mesh.
///
/// `thickness` is a length in mesh units; the inner shell is offset inward
/// (against the outward vertex normals) by this amount. A `thickness` of `0`
/// places the inner shell exactly on the outer one (a degenerate but valid
/// mesh). Open boundaries are closed with rim quads; a closed input yields a
/// hollow double shell.
///
/// This is infallible: the result is always a valid mesh.
///
/// # Examples
///
/// ```
/// use outram_blender::{primitives, solidify::solidify};
///
/// // A flat grid (open disk, chi = 1) solidifies into a closed slab (chi = 2).
/// let grid = primitives::grid(2, 2, 2.0);
/// assert_eq!(grid.euler_characteristic(), 1);
/// let slab = solidify(&grid, 0.2);
/// assert_eq!(slab.euler_characteristic(), 2);
/// ```
pub fn solidify(mesh: &Mesh, thickness: f64) -> Mesh {
    let positions = mesh.positions();
    let polygons = mesh.polygons();
    let n = positions.len();
    if n == 0 {
        return Mesh::new();
    }

    // Area-weighted vertex normals: accumulate each face's (unit normal × area)
    // onto its vertices, then normalize. Area weighting biases toward larger
    // faces, matching the standard smooth-normal convention.
    let mut normals = vec![Vec3::ZERO; n];
    for (f, poly) in polygons.iter().enumerate() {
        let unit = mesh.face_normal(crate::mesh::FaceId(f));
        let area = polygon_area(&positions, poly);
        let weighted = unit.scale(area);
        for v in poly {
            normals[v.0] = normals[v.0].add(weighted);
        }
    }
    for nrm in &mut normals {
        *nrm = nrm.normalize();
    }

    // Outer shell = original vertices; inner shell = offset copies at index +n.
    let mut new_positions = Vec::with_capacity(2 * n);
    new_positions.extend_from_slice(&positions);
    for i in 0..n {
        new_positions.push(positions[i].sub(normals[i].scale(thickness)));
    }

    let mut new_faces: Vec<Vec<usize>> = Vec::new();
    // Outer shell keeps its winding.
    for poly in &polygons {
        new_faces.push(poly.iter().map(|v| v.0).collect());
    }
    // Inner shell is the same faces, remapped to +n and reversed (cavity normals).
    for poly in &polygons {
        let face: Vec<usize> = poly.iter().rev().map(|v| v.0 + n).collect();
        new_faces.push(face);
    }

    // Rim: bridge every open boundary edge (reverse absent) to the inner shell.
    let mut directed: HashSet<(usize, usize)> = HashSet::new();
    for poly in &polygons {
        let k = poly.len();
        for i in 0..k {
            directed.insert((poly[i].0, poly[(i + 1) % k].0));
        }
    }
    for &(a, b) in &directed {
        if !directed.contains(&(b, a)) {
            // Boundary edge a->b owned by an outer face traversing a->b, so the
            // rim must traverse b->a on that shared edge: quad [b, a, a+n, b+n].
            new_faces.push(vec![b, a, a + n, b + n]);
        }
    }

    Mesh::from_polygons(&new_positions, &new_faces)
}

/// Area of a (planar-ish) polygon via a triangle fan from its first vertex.
///
/// Uses the magnitude of the summed cross products, which is exact for a planar
/// polygon and a reasonable estimate for a slightly non-planar one — adequate
/// as a vertex-normal weight.
fn polygon_area(positions: &[Vec3], poly: &[crate::mesh::VertexId]) -> f64 {
    if poly.len() < 3 {
        return 0.0;
    }
    let p0 = positions[poly[0].0];
    let mut acc = Vec3::ZERO;
    for i in 1..poly.len() - 1 {
        let e1 = positions[poly[i].0].sub(p0);
        let e2 = positions[poly[i + 1].0].sub(p0);
        acc = acc.add(e1.cross(e2));
    }
    0.5 * acc.length()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    /// True when every directed half-edge is unique and its reverse is present
    /// (a watertight, consistently-wound closed surface).
    fn is_watertight_consistent(mesh: &Mesh) -> bool {
        let mut directed: HashSet<(usize, usize)> = HashSet::new();
        for poly in mesh.polygons() {
            let k = poly.len();
            for i in 0..k {
                let a = poly[i].0;
                let b = poly[(i + 1) % k].0;
                if !directed.insert((a, b)) {
                    return false;
                }
            }
        }
        directed.iter().all(|&(a, b)| directed.contains(&(b, a)))
    }

    /// V&V — headline (open surface). Methodology: solidify a flat
    /// `grid(2,2)` (open disk, V=9, χ=1) by 0.2. Pass criterion: a closed slab.
    /// Result: V 9→18 (outer + inner shells), top 4 + bottom 4 + 8 rim quads =
    /// 16 faces, χ = 2, watertight & consistently wound. The two shells are
    /// separated by the thickness along −Z.
    #[test]
    fn open_grid_solidifies_to_closed_slab() {
        let grid = primitives::grid(2, 2, 2.0);
        assert_eq!(grid.euler_characteristic(), 1, "grid is an open disk");
        let slab = solidify(&grid, 0.2);
        assert_eq!(slab.vertex_count(), 18, "outer 9 + inner 9");
        assert_eq!(slab.face_count(), 16, "4 top + 4 bottom + 8 rim");
        assert_eq!(slab.euler_characteristic(), 2, "closed genus-0 slab");
        assert!(is_watertight_consistent(&slab), "slab is watertight");
        // The inner shell sits 0.2 below the outer along the +Z normal.
        let zs: Vec<f64> = slab.positions().iter().map(|p| p.z).collect();
        let zmin = zs.iter().cloned().fold(f64::MAX, f64::min);
        let zmax = zs.iter().cloned().fold(f64::MIN, f64::max);
        assert!((zmax - zmin - 0.2).abs() < 1e-9, "slab thickness is 0.2");
    }

    /// V&V — closed input becomes a hollow double shell. A `cube(2.0)`
    /// solidified has no boundary, so no rim: outer + inner shells only.
    /// Result: V 8→16, F 6→12, χ = 4 (two nested closed genus-0 surfaces), each
    /// shell watertight.
    #[test]
    fn closed_cube_solidifies_to_hollow_shell() {
        let cube = primitives::cube(2.0);
        let shell = solidify(&cube, 0.3);
        assert_eq!(shell.vertex_count(), 16, "outer 8 + inner 8");
        assert_eq!(shell.face_count(), 12, "6 outer + 6 inner, no rim");
        assert_eq!(shell.euler_characteristic(), 4, "two closed shells: chi = 2+2");
        assert!(is_watertight_consistent(&shell), "each shell is watertight");
        // Inner corner is offset inward: its distance from the origin is smaller.
        let outer_r = cube.positions()[0].length();
        let inner_r = shell.positions()[8].length();
        assert!(inner_r < outer_r, "inner shell is nested inside the outer");
    }

    /// V&V — thickness 0 collapses the inner shell onto the outer (valid, if
    /// degenerate). Topology counts still match the double-shell construction.
    #[test]
    fn zero_thickness_places_shells_coincident() {
        let grid = primitives::grid(2, 2, 2.0);
        let slab = solidify(&grid, 0.0);
        assert_eq!(slab.vertex_count(), 18);
        assert_eq!(slab.euler_characteristic(), 2);
        // Every inner vertex coincides with its outer source.
        let ps = slab.positions();
        for i in 0..9 {
            assert!(ps[i].sub(ps[i + 9]).length() < 1e-12);
        }
    }

    /// V&V — the `MeshOp::Solidify` dispatch matches the free function.
    #[test]
    fn meshop_solidify_matches_free_function() {
        use crate::ops::MeshOp;
        let grid = primitives::grid(3, 2, 1.0);
        let direct = solidify(&grid, 0.15);
        let via_op = MeshOp::Solidify { thickness: 0.15 }.apply(grid).expect("solidify infallible");
        assert_eq!(via_op.vertex_count(), direct.vertex_count());
        assert_eq!(via_op.face_count(), direct.face_count());
        assert_eq!(via_op.euler_characteristic(), direct.euler_characteristic());
    }
}
