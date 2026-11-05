// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/triSurfaceTools/triSurfaceExtrude2DEdges/triSurfaceExtrude2DEdges.{C,H}
//   meshLibrary/utilities/triSurfaceTools/triSurface2DCheck/triSurface2DCheck.{C,H}
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

//! The 2D workflow's input surface (gh:#1275): cfMesh's
//! **`triSurfaceExtrude2DEdges`** (a closed x-y outline of edges becomes a
//! ribbon of triangles, two per edge, `0.1 x` the outline's bounding-box
//! diagonal high) and **`triSurface2DCheck::is2DSurface`** (every facet
//! normal lies in the x-y plane, and every point sits at the lowest or the
//! highest z).
//!
//! Upstream's extruder puts every triangle in one patch, `patch0`; this port
//! takes an optional region per edge so a channel can carry named patches
//! (`None` is upstream). `createSubsets` (the `badFacets` / `badPointsId`
//! subsets written for the user to look at) is not ported.

use crate::math::Vec3;
use crate::mesh_octree::SMALL;
use outram_foam_basic_lib::primitives::eigen::{eigen_values_symm, eigen_vectors_symm_with};
use outram_foam_basic_lib::primitives::SymmTensor;

/// `triSurfaceExtrude2DEdges::extrudeSurface`: `points` (all at one z) and
/// the outline `edges` become a ribbon. Point `i + n` is point `i` raised by
/// `0.1 |bb|`; edge `e` gives triangles `(a, b, b + n)` and
/// `(a, b + n, a + n)`. Returns the points, triangles and each triangle's
/// region (edge `e`'s, or 0). `Err` where upstream aborts (points not in
/// one x-y plane).
pub fn extrude_2d_edges(points: &[Vec3], edges: &[[usize; 2]], edge_regions: Option<&[usize]>) -> Result<(Vec<Vec3>, Vec<[usize; 3]>, Vec<usize>), String> {
    let bb = crate::mesh_octree::BoundBox::of_points(points);
    if (bb.max.z - bb.min.z).abs() > SMALL {
        return Err("Cannot extrude edges which are not in the x - y plane!".into());
    }
    let n = points.len();
    let z_offset = 0.1 * bb.span().length();
    let mut pts = points.to_vec();
    pts.extend(points.iter().map(|p| Vec3::new(p.x, p.y, p.z + z_offset)));
    let mut tris = Vec::with_capacity(2 * edges.len());
    let mut regions = Vec::with_capacity(2 * edges.len());
    for (e, &[a, b]) in edges.iter().enumerate() {
        let r = edge_regions.map_or(0, |x| x[e]);
        tris.push([a, b, b + n]);
        tris.push([a, b + n, a + n]);
        regions.push(r);
        regions.push(r);
    }
    Ok((pts, tris, regions))
}

/// `triSurface2DCheck::is2DSurface`: the covariance `Σ n̂ n̂` of the unit
/// facet normals has a zero eigenvalue (`|λ₀| <= SMALL`), the direction
/// `e₁ x e₂` of the other two eigenvectors is z, and every point lies at the
/// lowest or highest z (within `SMALL`). Returns the first failure's
/// warning, as upstream prints it.
pub fn is_2d_surface(points: &[Vec3], tris: &[[usize; 3]]) -> Result<(), String> {
    let (mut xx, mut xy, mut xz, mut yy, mut yz, mut zz) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for t in tris {
        let n = points[t[1]].sub(points[t[0]]).cross(points[t[2]].sub(points[t[0]])).scale(0.5);
        let n = n.scale(1.0 / (n.length() + crate::mesh_octree::VSMALL));
        xx += n.x * n.x;
        xy += n.x * n.y;
        xz += n.x * n.z;
        yy += n.y * n.y;
        yz += n.y * n.z;
        zz += n.z * n.z;
    }
    let c = SymmTensor::new(xx, xy, xz, yy, yz, zz);
    let ev = eigen_values_symm(c);
    if ev.x.abs() > SMALL {
        return Err(format!("Surface mesh is in 3D space! This may result in an invalid mesh! (eigenvalues {:e} {:e} {:e})", ev.x, ev.y, ev.z));
    }
    let v = eigen_vectors_symm_with(c, ev);
    let (ey, ez) = (v.row_y(), v.row_z());
    let n = Vec3::new(ey.x, ey.y, ey.z).cross(Vec3::new(ez.x, ez.y, ez.z));
    if n.x.abs() > SMALL || n.y.abs() > SMALL {
        return Err("The surface mesh IS NOT IN THE X - Y PLANE!!!! This will result in a mesh without any cells".into());
    }
    let bb = crate::mesh_octree::BoundBox::of_points(points);
    if points.iter().any(|p| (p.z - bb.max.z).abs() > SMALL && (p.z - bb.min.z).abs() > SMALL) {
        return Err("z coordinates of the 2D surface are not uniform".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V&V (gh:#1275), prediction D1: the extruded rectangle is a 2D
    /// surface (8 triangles, 8 points, height 0.1 x the diagonal); a
    /// sphere and a tilted ribbon are not.
    ///
    /// ~~**D1 missed on the rectangle (2026-11-05):** its normal covariance
    /// is `diag(4, 4, 0)`, and `outram_foam_basic_lib`'s
    /// `eigen_values_symm` returns `(-1.68e-8, -0, 4)` for it, not
    /// `(0, 4, 4)`. So the check reports "3D space". The double root is
    /// lost in the cubic solve: that is a defect in the eigen routine (another
    /// crate's), not in this port. The rectangle's result is printed, not
    /// asserted.~~ **CORRECTED 2026-11-05 (gh:#1288):** `eigen_values_symm`
    /// lacked upstream ESI v2306's effectively-diagonal branch
    /// (`xy² + xz² + yz² < ROOTSMALL` returns the sorted diagonal). With it
    /// the rectangle gives `(0, 4, 4)` exactly and is 2D; D1 now holds and
    /// is asserted. A covariance with a repeated root and off-diagonals
    /// above the threshold still loses a root in upstream's cubic (see
    /// `a_rotated_repeated_root_goes_through_the_cubic_as_upstream` in
    /// that crate). An irregular quadrilateral (distinct eigenvalues) is
    /// the second asserted 2D case.
    #[test]
    fn extruded_outline_is_a_2d_surface() {
        let p = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0), Vec3::new(4.0, 3.0, 0.0), Vec3::new(0.0, 3.0, 0.0)];
        let e = [[0, 1], [1, 2], [2, 3], [3, 0]];
        let (pts, tris, reg) = extrude_2d_edges(&p, &e, None).unwrap();
        assert_eq!((pts.len(), tris.len()), (8, 8));
        assert!(reg.iter().all(|&r| r == 0));
        assert!((pts[4].z - 0.5).abs() < 1e-15, "0.1 x the diagonal 5");
        is_2d_surface(&pts, &tris).unwrap();
        let q = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(4.0, 0.5, 0.0), Vec3::new(3.0, 3.0, 0.0), Vec3::new(0.5, 2.0, 0.0)];
        let (qp, qt, _) = extrude_2d_edges(&q, &e, None).unwrap();
        is_2d_surface(&qp, &qt).unwrap();
        let (sp, st) = crate::shapes::sphere_surface(Vec3::ZERO, 1.0, 6, 12);
        assert!(is_2d_surface(&sp, &st).is_err());
        let tilted: Vec<Vec3> = pts.iter().map(|q| Vec3::new(q.x, q.y + 0.1 * q.z, q.z)).collect();
        assert!(is_2d_surface(&tilted, &tris).is_err());
        let off = vec![Vec3::new(0.0, 0.0, 0.1), p[1], p[2], p[3]];
        assert!(extrude_2d_edges(&off, &e, None).is_err());
    }
}
