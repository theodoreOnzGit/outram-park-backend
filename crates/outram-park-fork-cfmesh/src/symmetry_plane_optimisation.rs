// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/smoothers/geometry/meshOptimizer/
//     symmetryPlaneOptimisation/symmetryPlaneOptimisation.{C,H}
//   utilities/improveSymmetryPlanes/improveSymmetryPlanes.C
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae.
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later (cfMesh files
// say "either version 3 of the License, or (at your option) any later
// version"), conveyed here under GPL-3.0-only.
// OpenFOAM's `plane` (`nearestPoint`, `distance`, `planeIntersect`,
// `planePlaneIntersect`; src/OpenFOAM/meshes/primitiveShapes/plane/plane.C,
// GPL-3.0-or-later) is written from that routine; its source is not vendored
// in this workspace, and the tests below check it analytically.
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

//! cfMesh's **`symmetryPlaneOptimisation`** (gh:#1280): put every point of
//! a `symmetryPlane` patch back on that patch's plane. It is what
//! `renameBoundaryPatches::checkSymmetryPlanes` and the
//! `improveSymmetryPlanes` utility run.
//!
//! - `detectSymmetryPlanes`: per `symmetryPlane` patch, the plane through
//!   the mean of its face centres with the mean of its face normals
//!   (OpenFOAM `plane(c, n)` makes the normal unit).
//! - `pointInPlanes`: each point of such a patch gets the patch; if any
//!   point is more than half its face's largest centre-to-vertex distance
//!   from the plane, the patch is "not planar" and **nothing moves**
//!   (upstream warns and returns).
//! - `optimizeSymmetryPlanes`: a point in one plane goes to its nearest
//!   point on it; in two, to the nearest point of the planes' line
//!   (`planeIntersect`); in three, to the three planes' common point
//!   (`planePlaneIntersect`); in more than three it is left. Then
//!   `checkFacePyramids(VSMALL)` names the bad faces (upstream puts them in
//!   a face subset `invalidFaces`; here they are returned).
//!
//! Face normals are Newell area vectors and centres OpenFOAM's
//! `face::centre`, as elsewhere in the crate (`surface_engine`). The patch
//! types are given beside the mesh ([`crate::rename_boundary_patches`]
//! returns them); a [`VolumeMesh`] carries none.

use crate::correct_edges::foam_face_centre;
use crate::math::Vec3;
use crate::poly_mesh_gen_checks::check_face_pyramids;
use crate::surface_engine::ring_area_vector;
use crate::volume_mesh::VolumeMesh;
use std::collections::BTreeMap;

/// OpenFOAM's `VSMALL` for `double`.
const VSMALL: f64 = 1e-300;

/// OpenFOAM's `plane`: a point and a unit normal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    /// `refPoint()`.
    pub point: Vec3,
    /// `normal()`, unit.
    pub normal: Vec3,
}

impl Plane {
    /// `plane(basePoint, normalVector)`: the normal is made unit.
    pub fn new(point: Vec3, normal: Vec3) -> Self {
        let m = normal.length();
        Plane { point, normal: if m > VSMALL { normal.scale(1.0 / m) } else { normal } }
    }
    /// `distance(p)`: |(p − point)·n|.
    pub fn distance(&self, p: Vec3) -> f64 {
        p.sub(self.point).dot(self.normal).abs()
    }
    /// `nearestPoint(p)`: p − n ((p − point)·n).
    pub fn nearest_point(&self, p: Vec3) -> Vec3 {
        p.sub(self.normal.scale(p.sub(self.point).dot(self.normal)))
    }
    /// `planeIntersect(plane2)`: the line of two planes as (point, direction
    /// n1 × n2, not unit). The point has its coordinate along the largest
    /// component of the direction set to 0 and solves the two plane
    /// equations in the other two (OpenFOAM's choice of `iZero`, `i1`, `i2`).
    pub fn plane_intersect(&self, other: &Plane) -> (Vec3, Vec3) {
        let (n1, n2) = (self.normal, other.normal);
        let n1p1 = n1.dot(self.point);
        let n2p2 = n2.dot(other.point);
        let dir = n1.cross(n2);
        let (mx, my, mz) = (dir.x.abs(), dir.y.abs(), dir.z.abs());
        let (i_zero, i1, i2) = if mx > my {
            if mx > mz {
                (0, 1, 2)
            } else {
                (2, 0, 1)
            }
        } else if my > mz {
            (1, 2, 0)
        } else {
            (2, 0, 1)
        };
        let c = |v: Vec3, i: usize| [v.x, v.y, v.z][i];
        let mut pt = [0.0; 3];
        pt[i_zero] = 0.0;
        pt[i1] = (c(n2, i2) * n1p1 - c(n1, i2) * n2p2) / (c(n1, i1) * c(n2, i2) - c(n2, i1) * c(n1, i2));
        pt[i2] = (c(n2, i1) * n1p1 - c(n1, i1) * n2p2) / (c(n1, i2) * c(n2, i1) - c(n1, i1) * c(n2, i2));
        (Vec3::new(pt[0], pt[1], pt[2]), dir)
    }
    /// `planePlaneIntersect(plane2, plane3)`: the point common to three
    /// planes, `inv(A) & (−b)` with rows (n, −n·p) (by Cramer's rule here;
    /// OpenFOAM inverts the tensor, analytically the same).
    pub fn plane_plane_intersect(&self, p2: &Plane, p3: &Plane) -> Vec3 {
        let (a, b, c) = (self.normal, p2.normal, p3.normal);
        let d = Vec3::new(a.dot(self.point), b.dot(p2.point), c.dot(p3.point));
        let det = a.dot(b.cross(c));
        // Rows a, b, c: x = (d1 (b × c) + d2 (c × a) + d3 (a × b)) / det.
        b.cross(c).scale(d.x).add(c.cross(a).scale(d.y)).add(a.cross(b).scale(d.z)).scale(1.0 / det)
    }
}

/// What [`symmetry_plane_optimisation`] did.
#[derive(Debug, Clone, PartialEq)]
pub struct SymmetryReport {
    /// The plane of each `symmetryPlane` patch, by patch index.
    pub planes: BTreeMap<usize, Plane>,
    /// `pointInPlanes` found a point far off its plane: nothing was moved.
    pub problematic: bool,
    /// Points moved onto one, two or three planes.
    pub n_points_moved: usize,
    /// Points in more than three planes (left where they are).
    pub n_points_in_more_than_three_planes: usize,
    /// Faces failing `checkFacePyramids(VSMALL)` after the moves (upstream's
    /// `invalidFaces` subset).
    pub invalid_faces: Vec<usize>,
}

/// `detectSymmetryPlanes`.
pub fn detect_symmetry_planes(mesh: &VolumeMesh, patch_types: &[String]) -> BTreeMap<usize, Plane> {
    let mut planes = BTreeMap::new();
    for (pi, p) in mesh.patches.iter().enumerate() {
        if patch_types.get(pi).map(String::as_str) != Some("symmetryPlane") {
            continue;
        }
        let (mut cs, mut ns) = (Vec3::ZERO, Vec3::ZERO);
        for f in p.start_face..p.start_face + p.n_faces {
            cs = cs.add(foam_face_centre(&mesh.points, &mesh.faces[f]));
            ns = ns.add(ring_area_vector(&mesh.points, &mesh.faces[f]));
        }
        let n = p.n_faces as f64;
        planes.insert(pi, Plane::new(cs.scale(1.0 / n), ns.scale(1.0 / n)));
    }
    planes
}

/// `symmetryPlaneOptimisation(mesh).optimizeSymmetryPlanes()`, the patch
/// types beside the mesh (module docs).
pub fn symmetry_plane_optimisation(mesh: &mut VolumeMesh, patch_types: &[String]) -> SymmetryReport {
    let planes = detect_symmetry_planes(mesh, patch_types);
    // pointInPlanes
    let mut point_in: Vec<Vec<usize>> = vec![Vec::new(); mesh.points.len()];
    let mut problematic = false;
    for (&pi, plane) in &planes {
        let p = &mesh.patches[pi];
        for f in p.start_face..p.start_face + p.n_faces {
            let ring = &mesh.faces[f];
            let c = foam_face_centre(&mesh.points, ring);
            let max_dist = ring.iter().map(|&v| mesh.points[v].sub(c).length()).fold(0.0, f64::max);
            for &v in ring {
                if plane.distance(mesh.points[v]) > 0.5 * max_dist {
                    problematic = true;
                }
                if !point_in[v].contains(&pi) {
                    point_in[v].push(pi);
                }
            }
        }
    }
    let mut rep = SymmetryReport { planes: planes.clone(), problematic, n_points_moved: 0, n_points_in_more_than_three_planes: 0, invalid_faces: Vec::new() };
    if problematic {
        return rep;
    }
    for (v, ps) in point_in.iter().enumerate() {
        let p = mesh.points[v];
        let new = match ps.len() {
            0 => continue,
            1 => planes[&ps[0]].nearest_point(p),
            2 => {
                let (ref_point, dir) = planes[&ps[0]].plane_intersect(&planes[&ps[1]]);
                let n = dir.scale(1.0 / (dir.length() + VSMALL));
                ref_point.add(n.scale(p.sub(ref_point).dot(n)))
            }
            3 => planes[&ps[0]].plane_plane_intersect(&planes[&ps[1]], &planes[&ps[2]]),
            _ => {
                rep.n_points_in_more_than_three_planes += 1;
                continue;
            }
        };
        mesh.points[v] = new;
        rep.n_points_moved += 1;
    }
    let (_, bad) = check_face_pyramids(mesh, VSMALL, None);
    rep.invalid_faces = (0..bad.len()).filter(|&f| bad[f]).collect();
    rep
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartesian::cartesian_box;

    fn types(m: &VolumeMesh, sym: &[&str]) -> Vec<String> {
        m.patches.iter().map(|p| if sym.contains(&p.name.as_str()) { "symmetryPlane".to_string() } else { "patch".to_string() }).collect()
    }

    /// V&V (gh:#1280), the plan's test: a perturbed symmetry patch goes
    /// back on its plane. A 4×4×4 unit box; the points of the `zMin`
    /// patch (z = 0) are perturbed in z by ±1e-3 (deterministic pattern,
    /// zero mean), interior points untouched. **Predictions (written
    /// 2026-11-05, before the first run):** with `zMin` the one
    /// `symmetryPlane`: the plane is z ≈ 0 with a normal within 1e-3 of
    /// −z; every `zMin` point ends within 1e-12 of that plane; nothing
    /// is problematic; points of other patches not on `zMin` do not move.
    /// With `xMin`, `yMin` and `zMin` all `symmetryPlane` (unperturbed box
    /// plus the same z noise), the corner point (0, 0, 0) is in three
    /// planes and lands on their common point, within 1e-12 of all three;
    /// the edge points are within 1e-12 of both of their planes.
    #[test]
    fn perturbed_symmetry_planes_are_flattened() {
        let m0 = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [4, 4, 4]);
        let zmin = m0.patches.iter().position(|p| p.name.contains("zMin") || p.name.contains("zmin")).expect("zMin patch");
        let names: Vec<String> = m0.patches.iter().map(|p| p.name.clone()).collect();
        println!("patches {names:?}");
        let mut m = m0.clone();
        let p = &m0.patches[zmin];
        let mut on_z: Vec<usize> = (p.start_face..p.start_face + p.n_faces).flat_map(|f| m0.faces[f].clone()).collect();
        on_z.sort_unstable();
        on_z.dedup();
        for (k, &v) in on_z.iter().enumerate() {
            m.points[v].z += if k % 2 == 0 { 1e-3 } else { -1e-3 };
        }
        let before = m.clone();
        let t = types(&m, &[names[zmin].as_str()]);
        let rep = symmetry_plane_optimisation(&mut m, &t);
        let plane = rep.planes[&zmin];
        println!("one plane: {plane:?}, moved {}, problematic {}", rep.n_points_moved, rep.problematic);
        assert!(!rep.problematic);
        assert!(plane.normal.add(Vec3::new(0.0, 0.0, 1.0)).length() < 1e-3);
        for &v in &on_z {
            assert!(plane.distance(m.points[v]) < 1e-12);
        }
        for v in 0..m.points.len() {
            if !on_z.contains(&v) {
                assert_eq!(m.points[v], before.points[v]);
            }
        }
        assert!(rep.invalid_faces.is_empty());

        // Three planes.
        let mut m = before.clone();
        let sym: Vec<&str> = names.iter().filter(|n| n.contains("Min") || n.contains("min")).map(String::as_str).collect();
        assert_eq!(sym.len(), 3);
        let t = types(&m, &sym);
        let rep = symmetry_plane_optimisation(&mut m, &t);
        assert!(!rep.problematic);
        let corner = m0.points.iter().position(|q| q.length() < 1e-12).unwrap();
        for (pi, pl) in &rep.planes {
            let pa = &m.patches[*pi];
            for f in pa.start_face..pa.start_face + pa.n_faces {
                for &v in &m.faces[f] {
                    assert!(pl.distance(m.points[v]) < 1e-12, "point {v} off plane {pi}: {:e}", pl.distance(m.points[v]));
                }
            }
            assert!(pl.distance(m.points[corner]) < 1e-12);
        }
        println!("three planes: corner at {:?}, moved {}", m.points[corner], rep.n_points_moved);
    }

    /// OpenFOAM `plane` helpers, analytic: the x = 1 and y = 2 planes meet
    /// in the line (1, 2, t); with z = 3, the three meet at (1, 2, 3).
    #[test]
    fn plane_intersections() {
        let px = Plane::new(Vec3::new(1.0, 5.0, 7.0), Vec3::new(2.0, 0.0, 0.0));
        let py = Plane::new(Vec3::new(-3.0, 2.0, 1.0), Vec3::new(0.0, 1.0, 0.0));
        let pz = Plane::new(Vec3::new(0.0, 0.0, 3.0), Vec3::new(0.0, 0.0, -1.0));
        let (pt, dir) = px.plane_intersect(&py);
        assert!(px.distance(pt) < 1e-15 && py.distance(pt) < 1e-15);
        assert!(dir.cross(Vec3::new(0.0, 0.0, 1.0)).length() < 1e-15);
        assert_eq!(px.plane_plane_intersect(&py, &pz), Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(px.nearest_point(Vec3::new(4.0, 4.0, 4.0)), Vec3::new(1.0, 4.0, 4.0));
    }
}
