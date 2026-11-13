// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/boundaryLayers/triangulateNonPlanarBaseFaces/
//     triangulateNonPlanarBaseFaces.{C,H},
//     triangulateNonPlanarBaseFacesFunctions.C
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae.
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later (cfMesh files
// say "either version 3 of the License, or (at your option) any later
// version"), conveyed here under GPL-3.0-only.
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

//! cfMesh's **`triangulateNonPlanarBaseFaces`** (gh:#1280): before layers
//! are grown, a boundary face that is too twisted to be the base of a
//! prism is fanned into triangles about its centre, and its owner is split
//! into pyramids.
//!
//! `findNonPlanarBoundaryFaces`: for each boundary face that is not a
//! triangle, with `c` its centre (OpenFOAM `face::centre`) and `minDist`
//! the smallest distance from `c` to a vertex, each edge's triangle
//! `(p_i, p_{i+1}, c)` is checked against every vertex: a vertex more than
//! `tol × minDist` **above** the triangle's plane (signed, along the unit
//! normal) marks the face for decomposition and its owner for pyramids.
//! `tol` is `relFlatnessTol` (default 0.5, `boundaryLayers/
//! optimisationParameters` in `meshDict`). Then `decomposeFaces::
//! decomposeMeshFaces` ([`TopoMesh::decompose_mesh_faces`]) and
//! `decomposeCells::decomposeMesh` ([`decompose_cells`]), the shared ports.
//!
//! **No caller upstream** at this commit (grep of `meshLibrary/`,
//! `executables/` and `utilities/`: only its own files and `Make/files`).
//! It is ported as a callable step; no pipeline calls it.

use crate::correct_edges::foam_face_centre;
use crate::topology::{decompose_cells, TopoMesh};

/// OpenFOAM's `VSMALL` for `double`.
const VSMALL: f64 = 1e-300;
/// OpenFOAM's `VGREAT` for `double`.
const VGREAT: f64 = 1e300;

/// Upstream's default `relFlatnessTol`.
pub const DEFAULT_REL_FLATNESS_TOL: f64 = 0.5;

/// What [`triangulate_non_planar_base_faces`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TriangulateReport {
    /// Boundary faces fanned into triangles.
    pub n_faces_decomposed: usize,
    /// Cells split into pyramids.
    pub n_cells_decomposed: usize,
}

/// `findNonPlanarBoundaryFaces`: (faces to decompose, cells to split), one
/// flag per face and per cell of `w`.
pub fn find_non_planar_boundary_faces(w: &TopoMesh, tol: f64) -> (Vec<bool>, Vec<bool>) {
    let mut decompose_face = vec![false; w.faces.len()];
    let mut inverted_cell = vec![false; w.n_cells];
    for f in 0..w.faces.len() {
        if w.neighbour[f].is_some() {
            continue;
        }
        let bf = &w.faces[f];
        if bf.len() == 3 {
            continue;
        }
        let c = foam_face_centre(&w.points, bf);
        let mut min_dist = VGREAT;
        for &p in bf {
            min_dist = min_dist.min(c.sub(w.points[p]).length());
        }
        for e in 0..bf.len() {
            let (a, b) = (w.points[bf[e]], w.points[bf[(e + 1) % bf.len()]]);
            let tri_centre = a.add(b).add(c).scale(1.0 / 3.0);
            let n = b.sub(a).cross(c.sub(a)).scale(0.5);
            let n = n.scale(1.0 / (n.length() + VSMALL));
            for &p in bf {
                let d = w.points[p].sub(tri_centre).dot(n);
                if d > tol * min_dist {
                    inverted_cell[w.owner[f]] = true;
                    decompose_face[f] = true;
                }
            }
        }
    }
    (decompose_face, inverted_cell)
}

/// `triangulateNonPlanarBaseFaces(mesh).triangulateLayers()` with
/// `relFlatnessTol = tol`.
pub fn triangulate_non_planar_base_faces(w: &mut TopoMesh, tol: f64) -> TriangulateReport {
    let (decompose_face, inverted_cell) = find_non_planar_boundary_faces(w, tol);
    if !decompose_face.iter().any(|&d| d) {
        return TriangulateReport::default();
    }
    let n_faces_decomposed = w.decompose_mesh_faces(&decompose_face);
    decompose_cells(w, &inverted_cell);
    TriangulateReport { n_faces_decomposed, n_cells_decomposed: inverted_cell.iter().filter(|&&c| c).count() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartesian::cartesian_box;
    use crate::checks::cell_geometry;
    use crate::math::Vec3;

    /// V&V (gh:#1280), analytic. A 2×2×2 unit box is flat: no face is
    /// marked at the default tolerance. Then the box corner (1, 1, 1) is
    /// moved to (1, 1, 1.4): the two side quads at it stay in x = 1 and
    /// y = 1 (planar), the top quad of that cell is twisted. By hand: its
    /// centre is (0.75, 0.75, 1.1), minDist 0.367, so the bound is 0.184;
    /// the lifted vertex is 0.186 above the plane of the triangle on the
    /// opposite edge (y = 0.5).
    /// **Predictions (written 2026-11-05, before the first run):** flat
    /// box: nothing marked. Lifted corner: only the top quad is fanned
    /// (marginally: 0.186 against 0.184), its owner is the one cell split
    /// into pyramids (6 faces, so 8 − 1 + 6 + 3 = 16 cells: the fanned
    /// top quad gives 4 triangles, so the cell has 9 faces and 9 pyramids:
    /// 8 − 1 + 9 = 16), the mesh stays closed and its volume is unchanged
    /// by the split (to 1e-12).
    ///
    /// **Result at z = 1.4 (2026-11-05): missed.** Nothing is marked. The
    /// hand estimate used the vertex mean as the centre; OpenFOAM's
    /// `face::centre` (area-weighted, [`foam_face_centre`]) moves the centre
    /// and minDist, and the 0.186 / 0.184 margin does not survive it. The
    /// flat-box half held.
    ///
    /// **Second prediction (written after that miss, before its run):** at
    /// z = 2.0 the top quad is fanned (the lifted vertex is far above the
    /// opposite triangle), one cell becomes 9 pyramids (16 cells), closed,
    /// volume unchanged to 1e-12, every pyramid positive.
    #[test]
    fn twisted_boundary_faces_are_fanned() {
        let m0 = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [2, 2, 2]);
        let w0 = TopoMesh::from_volume_mesh(&m0);
        let (df, ic) = find_non_planar_boundary_faces(&w0, DEFAULT_REL_FLATNESS_TOL);
        assert!(!df.iter().any(|&d| d) && !ic.iter().any(|&c| c));
        let p = m0.points.iter().position(|q| q.sub(Vec3::new(1.0, 1.0, 1.0)).length() < 1e-12).unwrap();
        for (z, expect_fanned) in [(1.4, false), (2.0, true)] {
            let mut m = m0.clone();
            m.points[p] = Vec3::new(1.0, 1.0, z);
            let v0 = m.total_volume();
            let mut w = TopoMesh::from_volume_mesh(&m);
            let r = triangulate_non_planar_base_faces(&mut w, DEFAULT_REL_FLATNESS_TOL);
            let (out, _) = w.into_volume_mesh();
            out.validate().unwrap();
            let (vols, _) = cell_geometry(&out);
            let vmin = vols.iter().cloned().fold(f64::MAX, f64::min);
            println!("corner at z {z}: {r:?}, cells {}, volume {} (before {v0}), min cell volume {vmin:e}", out.n_cells, out.total_volume());
            assert!((out.total_volume() - v0).abs() < 1e-12);
            if expect_fanned {
                assert_eq!((r.n_faces_decomposed, r.n_cells_decomposed, out.n_cells), (1, 1, 16));
                assert!(vmin > 0.0);
            } else {
                assert_eq!(r, TriangulateReport::default());
            }
        }
    }
}
