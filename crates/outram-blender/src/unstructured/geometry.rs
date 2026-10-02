// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Face and cell geometry ported from OpenFOAM (www.openfoam.com, GPL-3.0):
//   src/OpenFOAM/meshes/primitiveMesh/primitiveMeshFaceCentresAndAreas.C
//   src/OpenFOAM/meshes/primitiveMesh/primitiveMeshCellCentresAndVols.C
//   Copyright (C) 2011-2023 OpenFOAM Foundation, (C) 2016-2023 OpenCFD Ltd.
// by way of this workspace's own port of the same two routines,
// `outram_foam_basic_lib::io::poly_mesh` (`face_centre_and_area`,
// `PolyMesh::cell_geometry`), which this file mirrors line for line so the
// two cannot drift (the neutral mesh cannot call it: outram-mc-libs depends on
// this crate without the `foam-export` feature).
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

//! Face and cell geometry of the neutral mesh: face centres and area vectors,
//! cell volumes and centroids, and the two triangulations a cell is used
//! through.
//!
//! All quantities are in the mesh's own [`LengthUnit`](super::LengthUnit).
//!
//! # What "inside a cell" means: the bounding triangles
//!
//! A polyhedral cell with non-planar faces has no unique interior. This crate
//! fixes one: the cell is the region enclosed by the **fan triangles**
//! `(face centre, v_i, v_i+1)` of its faces
//! ([`UnstructuredMesh::for_each_bounding_triangle`](super::UnstructuredMesh::for_each_bounding_triangle)).
//! Two cells sharing a face share its fan triangles, so the cells tile the
//! mesh with no gap and no overlap, and the definition holds for **any**
//! closed cell, convex or not. Point location (outram-mc-libs) tests it by
//! the generalized winding number of those triangles, the same inside test
//! [`crate::boolean_classify`] uses for closed surface meshes.
//!
//! # The centroid decomposition: sampling only
//!
//! The tetrahedra `(face centre, v_i, v_i+1, cell centre)`
//! ([`UnstructuredMesh::for_each_cell_simplex`](super::UnstructuredMesh::for_each_cell_simplex))
//! are OpenFOAM's `tetDecomposition` (the `CELL_TETS` mode of
//! `polyMesh::findCell`). They tile the cell exactly when it is star-shaped
//! about its centroid, which every convex cell is; some cfMesh dual cells are
//! not (found 2026-10-03 on the first cfMesh cylinder put through this
//! module: cell 4 of a 0.1 m tet-dual mesh had an inverted decomposition
//! tetrahedron). Whether a cell's decomposition is valid is recorded
//! ([`UnstructuredMesh::decomposition_is_valid`](super::UnstructuredMesh::decomposition_is_valid));
//! it is used only to sample uniformly in a cell, with rejection sampling as
//! the fallback.
//!
//! The volume formula below is OpenFOAM's pyramid decomposition about an
//! estimated centre; for a closed cell with planar faces it is exact by the
//! divergence theorem whatever the cell's shape (a warped face adds the usual
//! OpenFOAM approximation).

/// `a - b`.
#[inline]
pub fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `a + b`.
#[inline]
pub fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// `s a`.
#[inline]
pub fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

/// `a . b`.
#[inline]
pub fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `a x b`.
#[inline]
pub fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// `|a|`.
#[inline]
pub fn mag(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// Signed volume of the tetrahedron `(a, b, c, d)`:
/// `((b - a) x (c - a)) . (d - a) / 6`, positive when `d` lies on the side
/// the right-hand normal of `(a, b, c)` points to.
#[inline]
pub fn tet_signed_volume(a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]) -> f64 {
    dot(cross(sub(b, a), sub(c, a)), sub(d, a)) / 6.0
}

/// Centre and area vector of one face.
///
/// - **3-D** (`dim == 3`): port of `primitiveMesh::makeFaceCentresAndAreas`
///   (mirrored from `outram_foam_basic_lib::io::poly_mesh::face_centre_and_area`):
///   a triangle is exact; a polygon is fanned about its vertex average, the
///   centre is the area-weighted mean of the fan-triangle centroids and the
///   area vector the sum of the fan-triangle area vectors. The area vector's
///   direction is the right-hand normal of the vertex loop.
/// - **2-D** (`dim == 2`): the face is an edge `a -> b` in the `z = 0` plane;
///   the centre is its midpoint and the area vector `(dy, -dx, 0)`, the
///   outward normal of a counter-clockwise cell scaled by the edge length
///   (unit depth).
pub fn face_centre_and_area(points: &[[f64; 3]], verts: &[usize], dim: usize) -> ([f64; 3], [f64; 3]) {
    if dim == 2 {
        let a = points[verts[0]];
        let b = points[verts[1]];
        return (scale(add(a, b), 0.5), [b[1] - a[1], -(b[0] - a[0]), 0.0]);
    }
    let n = verts.len();
    if n == 3 {
        let a = points[verts[0]];
        let b = points[verts[1]];
        let c = points[verts[2]];
        let centre = scale(add(add(a, b), c), 1.0 / 3.0);
        return (centre, scale(cross(sub(b, a), sub(c, a)), 0.5));
    }
    let mut c_est = [0.0; 3];
    for &v in verts {
        c_est = add(c_est, points[v]);
    }
    c_est = scale(c_est, 1.0 / n as f64);

    let mut sum_a = 0.0;
    let mut sum_n = [0.0; 3];
    let mut sum_ac = [0.0; 3];
    for i in 0..n {
        let p1 = points[verts[i]];
        let p2 = points[verts[(i + 1) % n]];
        let mid = scale(add(add(p1, p2), c_est), 1.0 / 3.0);
        let n_tri = cross(sub(p2, p1), sub(c_est, p1));
        let a_tri = mag(n_tri);
        sum_a += a_tri;
        sum_n = add(sum_n, n_tri);
        sum_ac = add(sum_ac, scale(mid, a_tri));
    }
    let centre = if sum_a > f64::EPSILON {
        scale(sum_ac, 1.0 / sum_a)
    } else {
        c_est
    };
    (centre, scale(sum_n, 0.5))
}

/// Volumes and centroids of every cell, by the OpenFOAM pyramid
/// decomposition (`primitiveMesh::makeCellCentresAndVols`).
///
/// First the cell centre is estimated as the mean of its face centres; then
/// each face contributes a pyramid with apex at that estimate. In 3-D the
/// pyramid volume is `Sf . (Cf - c_est) / 3` with centroid
/// `3/4 Cf + 1/4 c_est`; in 2-D the triangle area is `Sf . (Cf - c_est) / 2`
/// with centroid `2/3 Cf + 1/3 c_est`.
///
/// `face_owner_side` is, for each face, `(owner, neighbour)`; the area vector
/// is negated for the neighbour.
pub fn cell_volumes_and_centres(
    n_cells: usize,
    face_centres: &[[f64; 3]],
    face_areas: &[[f64; 3]],
    owner: &[usize],
    neighbour: &[Option<usize>],
    dim: usize,
) -> (Vec<f64>, Vec<[f64; 3]>) {
    let mut c_est = vec![[0.0; 3]; n_cells];
    let mut n_faces = vec![0.0f64; n_cells];
    for f in 0..face_centres.len() {
        c_est[owner[f]] = add(c_est[owner[f]], face_centres[f]);
        n_faces[owner[f]] += 1.0;
        if let Some(nb) = neighbour[f] {
            c_est[nb] = add(c_est[nb], face_centres[f]);
            n_faces[nb] += 1.0;
        }
    }
    for c in 0..n_cells {
        if n_faces[c] > 0.0 {
            c_est[c] = scale(c_est[c], 1.0 / n_faces[c]);
        }
    }
    let (w_face, w_apex, divisor) = if dim == 2 {
        (2.0 / 3.0, 1.0 / 3.0, 2.0)
    } else {
        (0.75, 0.25, 3.0)
    };
    let mut vol = vec![0.0f64; n_cells];
    let mut ctr = vec![[0.0; 3]; n_cells];
    let mut add_pyramid = |cell: usize, sf: [f64; 3], cf: [f64; 3], c_est: &[[f64; 3]]| {
        let pyr = dot(sf, sub(cf, c_est[cell]));
        let pc = add(scale(cf, w_face), scale(c_est[cell], w_apex));
        vol[cell] += pyr;
        ctr[cell] = add(ctr[cell], scale(pc, pyr));
    };
    for f in 0..face_centres.len() {
        add_pyramid(owner[f], face_areas[f], face_centres[f], &c_est);
        if let Some(nb) = neighbour[f] {
            add_pyramid(nb, scale(face_areas[f], -1.0), face_centres[f], &c_est);
        }
    }
    for c in 0..n_cells {
        if vol[c].abs() > f64::EPSILON {
            ctr[c] = scale(ctr[c], 1.0 / vol[c]);
        } else {
            ctr[c] = c_est[c];
        }
        vol[c] /= divisor;
    }
    (vol, ctr)
}

/// Visit the fan triangles `(face centre, v_i, v_i+1)` of one 3-D face, wound
/// like the face (right-hand normal along the face's area vector).
#[inline]
pub fn for_each_fan_triangle<F: FnMut([f64; 3], [f64; 3], [f64; 3])>(
    points: &[[f64; 3]],
    verts: &[usize],
    centre: [f64; 3],
    mut f: F,
) {
    let n = verts.len();
    for i in 0..n {
        f(centre, points[verts[i]], points[verts[(i + 1) % n]]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit square face in the plane z = 0, wound counter-clockwise seen
    /// from +z: centre (0.5, 0.5, 0), area vector (0, 0, 1).
    #[test]
    fn square_face_centre_and_area() {
        let pts = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]];
        let (c, a) = face_centre_and_area(&pts, &[0, 1, 2, 3], 3);
        assert!((c[0] - 0.5).abs() < 1e-15 && (c[1] - 0.5).abs() < 1e-15);
        assert!((a[2] - 1.0).abs() < 1e-15 && a[0].abs() < 1e-15);
    }

    /// The bottom edge of a counter-clockwise unit square has outward normal
    /// -y and unit length.
    #[test]
    fn edge_area_vector_points_out_of_a_ccw_cell() {
        let pts = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
        let (c, a) = face_centre_and_area(&pts, &[0, 1], 2);
        assert_eq!(c, [0.5, 0.0, 0.0]);
        assert_eq!(a, [0.0, -1.0, 0.0]);
    }

    #[test]
    fn reference_tet_volume() {
        let v = tet_signed_volume([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
        assert!((v - 1.0 / 6.0).abs() < 1e-16);
    }
}
