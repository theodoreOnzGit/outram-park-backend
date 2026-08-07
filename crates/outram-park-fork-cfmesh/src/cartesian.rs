// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Algorithm reference (re-implemented in Rust, not transcribed):
//   cfMesh — https://github.com/wyldckat/cfMesh
//   meshLibrary/cartesianMesh (the un-refined background Cartesian grid the
//   generator starts from).
//   Copyright (C) 2014-2017 Creative Fields, Ltd. Licence: GPL-3.0-only.
//   The single-block output is the analogue of OpenFOAM `blockMesh`:
//   Copyright (C) 2011-2016 OpenFOAM Foundation, GPL-3.0-only.
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

//! Structured **Cartesian block mesher** — an axis-aligned box filled with a
//! regular grid of hexahedral cells.
//!
//! This is the foundation cfMesh's `cartesianMesh` builds on: its generator
//! starts from a background Cartesian octree over the bounding box and then
//! refines/carves it to a surface. This module builds the un-refined,
//! un-carved base case — a full `nx × ny × nz` hex grid of a box — which is a
//! complete, valid [`VolumeMesh`] in its own right (the analogue of OpenFOAM's
//! `blockMesh` for a single block) and the milestone-1 output that proves the
//! [`VolumeMesh`] data structure and its conventions end to end.
//!
//! Later milestones add the octree refinement and surface intersection that
//! turn this background grid into a body-fitted mesh, plus boundary layers.

use crate::math::Vec3;
use crate::volume_mesh::{orient_ring, BoundaryPatch, VolumeMesh};

/// Mesh the axis-aligned box `[min, max]` into a regular `nx × ny × nz` grid of
/// hexahedral cells.
///
/// The result has `nx·ny·nz` cells, internal faces first (owner < neighbour,
/// normal pointing owner→neighbour), then six boundary patches
/// `xMin, xMax, yMin, yMax, zMin, zMax` (outward normals). Its
/// [`VolumeMesh::total_volume`] equals the box volume and every cell passes the
/// closure check in [`VolumeMesh::validate`].
///
/// # Panics
///
/// If any division count is zero, or `max <= min` on any axis.
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, cartesian::cartesian_box};
///
/// // Unit box as a 2×2×2 grid: 8 cells, volume 1.
/// let m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [2, 2, 2]);
/// assert_eq!(m.cell_count(), 8);
/// assert!((m.total_volume() - 1.0).abs() < 1e-12);
/// assert!(m.validate().is_ok());
/// ```
pub fn cartesian_box(min: Vec3, max: Vec3, divisions: [usize; 3]) -> VolumeMesh {
    let [nx, ny, nz] = divisions;
    assert!(nx >= 1 && ny >= 1 && nz >= 1, "each division count must be >= 1");
    assert!(max.x > min.x && max.y > min.y && max.z > min.z, "max must exceed min on every axis");

    let dx = (max.x - min.x) / nx as f64;
    let dy = (max.y - min.y) / ny as f64;
    let dz = (max.z - min.z) / nz as f64;

    // Lattice point index and cell index.
    let np = |i: usize, j: usize, k: usize| i + (nx + 1) * (j + (ny + 1) * k);
    let nc = |i: usize, j: usize, k: usize| i + nx * (j + ny * k);
    let cell_center = |i: usize, j: usize, k: usize| {
        Vec3::new(
            min.x + dx * (i as f64 + 0.5),
            min.y + dy * (j as f64 + 0.5),
            min.z + dz * (k as f64 + 0.5),
        )
    };

    // Points: k outer, j, i inner — matches `np`.
    let mut points = Vec::with_capacity((nx + 1) * (ny + 1) * (nz + 1));
    for k in 0..=nz {
        for j in 0..=ny {
            for i in 0..=nx {
                points.push(Vec3::new(
                    min.x + dx * i as f64,
                    min.y + dy * j as f64,
                    min.z + dz * k as f64,
                ));
            }
        }
    }

    let mut faces: Vec<Vec<usize>> = Vec::new();
    let mut owner: Vec<usize> = Vec::new();
    let mut neighbour: Vec<Option<usize>> = Vec::new();

    let mut push_face = |ring: Vec<usize>, o: usize, n: Option<usize>, oc: Vec3, pts: &[Vec3]| {
        faces.push(orient_ring(ring, oc, pts));
        owner.push(o);
        neighbour.push(n);
    };

    // ---- Internal faces (owner = lower-index cell) --------------------------
    // X-perpendicular internal faces.
    for k in 0..nz {
        for j in 0..ny {
            for i in 1..nx {
                let ring = vec![np(i, j, k), np(i, j + 1, k), np(i, j + 1, k + 1), np(i, j, k + 1)];
                push_face(ring, nc(i - 1, j, k), Some(nc(i, j, k)), cell_center(i - 1, j, k), &points);
            }
        }
    }
    // Y-perpendicular internal faces.
    for k in 0..nz {
        for j in 1..ny {
            for i in 0..nx {
                let ring = vec![np(i, j, k), np(i + 1, j, k), np(i + 1, j, k + 1), np(i, j, k + 1)];
                push_face(ring, nc(i, j - 1, k), Some(nc(i, j, k)), cell_center(i, j - 1, k), &points);
            }
        }
    }
    // Z-perpendicular internal faces.
    for k in 1..nz {
        for j in 0..ny {
            for i in 0..nx {
                let ring = vec![np(i, j, k), np(i + 1, j, k), np(i + 1, j + 1, k), np(i, j + 1, k)];
                push_face(ring, nc(i, j, k - 1), Some(nc(i, j, k)), cell_center(i, j, k - 1), &points);
            }
        }
    }

    // ---- Boundary patches ---------------------------------------------------
    let mut patches: Vec<BoundaryPatch> = Vec::new();
    // Each closure body pushes a run of boundary faces and records the patch.
    let add_patch = |name: &str,
                     rings: Vec<(Vec<usize>, usize, Vec3)>,
                         faces: &mut Vec<Vec<usize>>,
                         owner: &mut Vec<usize>,
                         neighbour: &mut Vec<Option<usize>>,
                         patches: &mut Vec<BoundaryPatch>,
                         pts: &[Vec3]| {
        let start = faces.len();
        let n = rings.len();
        for (ring, o, oc) in rings {
            faces.push(orient_ring(ring, oc, pts));
            owner.push(o);
            neighbour.push(None);
        }
        patches.push(BoundaryPatch { name: name.into(), start_face: start, n_faces: n });
    };

    // xMin (i = 0) / xMax (i = nx).
    let mut xmin = Vec::new();
    let mut xmax = Vec::new();
    for k in 0..nz {
        for j in 0..ny {
            xmin.push((
                vec![np(0, j, k), np(0, j + 1, k), np(0, j + 1, k + 1), np(0, j, k + 1)],
                nc(0, j, k),
                cell_center(0, j, k),
            ));
            xmax.push((
                vec![np(nx, j, k), np(nx, j + 1, k), np(nx, j + 1, k + 1), np(nx, j, k + 1)],
                nc(nx - 1, j, k),
                cell_center(nx - 1, j, k),
            ));
        }
    }
    // yMin (j = 0) / yMax (j = ny).
    let mut ymin = Vec::new();
    let mut ymax = Vec::new();
    for k in 0..nz {
        for i in 0..nx {
            ymin.push((
                vec![np(i, 0, k), np(i + 1, 0, k), np(i + 1, 0, k + 1), np(i, 0, k + 1)],
                nc(i, 0, k),
                cell_center(i, 0, k),
            ));
            ymax.push((
                vec![np(i, ny, k), np(i + 1, ny, k), np(i + 1, ny, k + 1), np(i, ny, k + 1)],
                nc(i, ny - 1, k),
                cell_center(i, ny - 1, k),
            ));
        }
    }
    // zMin (k = 0) / zMax (k = nz).
    let mut zmin = Vec::new();
    let mut zmax = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            zmin.push((
                vec![np(i, j, 0), np(i + 1, j, 0), np(i + 1, j + 1, 0), np(i, j + 1, 0)],
                nc(i, j, 0),
                cell_center(i, j, 0),
            ));
            zmax.push((
                vec![np(i, j, nz), np(i + 1, j, nz), np(i + 1, j + 1, nz), np(i, j + 1, nz)],
                nc(i, j, nz - 1),
                cell_center(i, j, nz - 1),
            ));
        }
    }
    add_patch("xMin", xmin, &mut faces, &mut owner, &mut neighbour, &mut patches, &points);
    add_patch("xMax", xmax, &mut faces, &mut owner, &mut neighbour, &mut patches, &points);
    add_patch("yMin", ymin, &mut faces, &mut owner, &mut neighbour, &mut patches, &points);
    add_patch("yMax", ymax, &mut faces, &mut owner, &mut neighbour, &mut patches, &points);
    add_patch("zMin", zmin, &mut faces, &mut owner, &mut neighbour, &mut patches, &points);
    add_patch("zMax", zmax, &mut faces, &mut owner, &mut neighbour, &mut patches, &points);

    VolumeMesh { points, faces, owner, neighbour, n_cells: nx * ny * nz, patches }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V&V — headline. Methodology: mesh the unit box `[0,1]³` as a 2×2×2 grid.
    /// Pass criterion: correct cell/face/patch counts, exact volume, closed
    /// cells. Result: 8 cells; 12 internal faces ((nx−1)ny nz + nx(ny−1)nz +
    /// nx ny(nz−1) = 4+4+4); 24 boundary faces in 6 patches of 4; total volume
    /// = 1; validate() Ok (every cell's Σ face-area = 0).
    #[test]
    fn unit_box_2x2x2_counts_volume_and_closure() {
        let m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [2, 2, 2]);
        assert_eq!(m.cell_count(), 8);
        assert_eq!(m.point_count(), 27, "(2+1)³ lattice");
        assert_eq!(m.n_internal_faces(), 12);
        assert_eq!(m.n_boundary_faces(), 24);
        assert_eq!(m.face_count(), 36);
        assert_eq!(m.patches.len(), 6);
        for p in &m.patches {
            assert_eq!(p.n_faces, 4, "each face of a 2×2 side has 4 boundary faces");
        }
        assert!((m.total_volume() - 1.0).abs() < 1e-12, "box volume = 1");
        m.validate().expect("every cell must be closed");
    }

    /// V&V — a non-cubic box, non-uniform divisions. A 3×2×1 grid of the box
    /// [(-1,0,0),(2,1,4)] has 6 cells and volume 3·1·4 = 12; counts follow the
    /// closed forms and every cell is closed.
    #[test]
    fn rectangular_box_counts_and_volume() {
        let m = cartesian_box(Vec3::new(-1.0, 0.0, 0.0), Vec3::new(2.0, 1.0, 4.0), [3, 2, 1]);
        assert_eq!(m.cell_count(), 6);
        // internal: (3-1)*2*1 + 3*(2-1)*1 + 3*2*(1-1) = 4 + 3 + 0 = 7.
        assert_eq!(m.n_internal_faces(), 7);
        // boundary: 2*(2*1 + 3*1 + 3*2) = 2*(2+3+6) = 22.
        assert_eq!(m.n_boundary_faces(), 22);
        assert!((m.total_volume() - 12.0).abs() < 1e-12);
        m.validate().expect("cells closed");
    }

    /// V&V — boundary normals point outward: summed over each patch, the area
    /// vector points along the expected axis with the expected sign.
    #[test]
    fn boundary_normals_point_outward() {
        let m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [2, 2, 2]);
        let patch_area = |name: &str| {
            let p = m.patches.iter().find(|p| p.name == name).unwrap();
            let mut a = Vec3::ZERO;
            for f in p.start_face..p.start_face + p.n_faces {
                a = a.add(m.face_area_vector(f));
            }
            a
        };
        assert!(patch_area("xMin").x < -0.9, "xMin normal points -X");
        assert!(patch_area("xMax").x > 0.9, "xMax normal points +X");
        assert!(patch_area("zMax").z > 0.9, "zMax normal points +Z");
    }
}
