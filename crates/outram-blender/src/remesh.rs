// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Voxel remesh and mesh <-> volume conversion. Follows the published behaviour
// of Blender's Remesh modifier / voxel remesh (source/blender/blenkernel/
// intern/mesh_remesh_voxel.cc) and the Mesh-to-Volume / Volume-to-Mesh
// modifiers (MOD_mesh_to_volume.cc / MOD_volume_to_mesh.cc), github.com/blender/
// blender, GPL-2.0-or-later: rasterise a closed mesh to a voxel occupancy grid
// and back to a "blocky" surface, with optional smoothing. Concepts only — no
// upstream source copied.
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

//! **Voxel remesh + Mesh ↔ Volume** (`op-hzs.54.30`, GH issue #37 §F).
//!
//! - [`VoxelGrid`] — a dense occupancy grid (one `bool` per cell).
//! - [`mesh_to_volume`] — rasterise a **closed** mesh: a cell is occupied if
//!   its centre is inside (ray-parity test). Blender's *Mesh to Volume*.
//! - [`volume_to_mesh`] — emit a quad for every occupied-cell face that borders
//!   an empty cell, giving a watertight blocky surface. Blender's *Volume to
//!   Mesh*.
//! - [`voxel_remesh`] — [`mesh_to_volume`] → [`volume_to_mesh`] →
//!   `smooth_iters` Laplacian passes. Blender's voxel Remesh (blocks mode at
//!   `smooth_iters = 0`, voxel/smooth otherwise).

use crate::math::Vec3;
use crate::mesh::{FaceId, Mesh};

/// A dense voxel occupancy grid.
#[derive(Debug, Clone)]
pub struct VoxelGrid {
    /// World position of the centre of cell `(0, 0, 0)`.
    pub origin: Vec3,
    /// Edge length of a cell.
    pub cell: f64,
    /// Grid resolution `[nx, ny, nz]`.
    pub dims: [usize; 3],
    /// `occ[x + nx*(y + ny*z)]` — whether that cell is inside the volume.
    pub occ: Vec<bool>,
}

impl VoxelGrid {
    fn index(&self, x: usize, y: usize, z: usize) -> usize {
        x + self.dims[0] * (y + self.dims[1] * z)
    }

    /// Whether cell `(x, y, z)` is occupied (`false` for out-of-range).
    pub fn get(&self, x: i64, y: i64, z: i64) -> bool {
        if x < 0 || y < 0 || z < 0 {
            return false;
        }
        let (x, y, z) = (x as usize, y as usize, z as usize);
        if x >= self.dims[0] || y >= self.dims[1] || z >= self.dims[2] {
            return false;
        }
        self.occ[self.index(x, y, z)]
    }

    /// The world-space centre of cell `(x, y, z)`.
    pub fn cell_center(&self, x: usize, y: usize, z: usize) -> Vec3 {
        self.origin.add(Vec3::new(
            x as f64 * self.cell,
            y as f64 * self.cell,
            z as f64 * self.cell,
        ))
    }

    /// Number of occupied cells.
    pub fn occupied_count(&self) -> usize {
        self.occ.iter().filter(|&&o| o).count()
    }
}

/// Rasterise `mesh` (assumed closed and consistently wound) into a
/// [`VoxelGrid`] of cell size `cell`. Adds one cell of padding on every side.
pub fn mesh_to_volume(mesh: &Mesh, cell: f64) -> VoxelGrid {
    let cell = cell.max(1e-6);
    let (lo, hi) = crate::measure::bounding_box(mesh);
    let dims = [
        (((hi.x - lo.x) / cell).ceil() as usize) + 3,
        (((hi.y - lo.y) / cell).ceil() as usize) + 3,
        (((hi.z - lo.z) / cell).ceil() as usize) + 3,
    ];
    let origin = lo.sub(Vec3::new(cell, cell, cell));
    let tris = triangles(mesh);

    let mut occ = vec![false; dims[0] * dims[1] * dims[2]];
    for z in 0..dims[2] {
        for y in 0..dims[1] {
            for x in 0..dims[0] {
                let c = origin.add(Vec3::new(x as f64 * cell, y as f64 * cell, z as f64 * cell));
                if inside(c, &tris) {
                    occ[x + dims[0] * (y + dims[1] * z)] = true;
                }
            }
        }
    }
    VoxelGrid {
        origin,
        cell,
        dims,
        occ,
    }
}

/// Emit a watertight blocky surface: one outward-facing quad per occupied-cell
/// face that borders an empty cell.
pub fn volume_to_mesh(grid: &VoxelGrid) -> Mesh {
    let mut positions: Vec<Vec3> = Vec::new();
    let mut faces: Vec<Vec<usize>> = Vec::new();
    let mut lookup: std::collections::HashMap<[i64; 3], usize> = std::collections::HashMap::new();
    let h = grid.cell * 0.5;

    let mut corner = |p: Vec3, positions: &mut Vec<Vec3>| -> usize {
        let key = [
            (p.x / (grid.cell * 0.5)).round() as i64,
            (p.y / (grid.cell * 0.5)).round() as i64,
            (p.z / (grid.cell * 0.5)).round() as i64,
        ];
        *lookup.entry(key).or_insert_with(|| {
            positions.push(p);
            positions.len() - 1
        })
    };

    // Face directions: (offset, the four corner offsets, wound CCW outward).
    let faces_def: [([i64; 3], [Vec3; 4]); 6] = [
        (
            [1, 0, 0],
            [
                Vec3::new(h, -h, -h),
                Vec3::new(h, h, -h),
                Vec3::new(h, h, h),
                Vec3::new(h, -h, h),
            ],
        ),
        (
            [-1, 0, 0],
            [
                Vec3::new(-h, -h, -h),
                Vec3::new(-h, -h, h),
                Vec3::new(-h, h, h),
                Vec3::new(-h, h, -h),
            ],
        ),
        (
            [0, 1, 0],
            [
                Vec3::new(-h, h, -h),
                Vec3::new(-h, h, h),
                Vec3::new(h, h, h),
                Vec3::new(h, h, -h),
            ],
        ),
        (
            [0, -1, 0],
            [
                Vec3::new(-h, -h, -h),
                Vec3::new(h, -h, -h),
                Vec3::new(h, -h, h),
                Vec3::new(-h, -h, h),
            ],
        ),
        (
            [0, 0, 1],
            [
                Vec3::new(-h, -h, h),
                Vec3::new(h, -h, h),
                Vec3::new(h, h, h),
                Vec3::new(-h, h, h),
            ],
        ),
        (
            [0, 0, -1],
            [
                Vec3::new(-h, -h, -h),
                Vec3::new(-h, h, -h),
                Vec3::new(h, h, -h),
                Vec3::new(h, -h, -h),
            ],
        ),
    ];

    for z in 0..grid.dims[2] {
        for y in 0..grid.dims[1] {
            for x in 0..grid.dims[0] {
                if !grid.get(x as i64, y as i64, z as i64) {
                    continue;
                }
                let c = grid.cell_center(x, y, z);
                for (off, corners) in &faces_def {
                    if grid.get(x as i64 + off[0], y as i64 + off[1], z as i64 + off[2]) {
                        continue;
                    }
                    let quad: Vec<usize> = corners
                        .iter()
                        .map(|&cc| corner(c.add(cc), &mut positions))
                        .collect();
                    faces.push(quad);
                }
            }
        }
    }
    Mesh::from_polygons(&positions, &faces)
}

/// Voxel remesh: rasterise, re-surface, then `smooth_iters` Laplacian passes
/// (`0` = the blocky result).
pub fn voxel_remesh(mesh: &Mesh, cell: f64, smooth_iters: u32) -> Mesh {
    let grid = mesh_to_volume(mesh, cell);
    if grid.occupied_count() == 0 {
        return mesh.clone();
    }
    let blocky = volume_to_mesh(&grid);
    if smooth_iters == 0 {
        return blocky;
    }
    crate::transform_ops::smooth_vertices(&blocky, &[], smooth_iters, 0.5, [true, true, true])
}

// --- helpers ---

type Tri = [Vec3; 3];

fn triangles(mesh: &Mesh) -> Vec<Tri> {
    let mut out = Vec::new();
    for f in 0..mesh.face_count() {
        let vs = mesh.face_vertices(FaceId(f));
        let p: Vec<Vec3> = vs
            .iter()
            .map(|v| mesh.vertex(*v).map(|x| x.position).unwrap_or(Vec3::ZERO))
            .collect();
        for i in 1..p.len().saturating_sub(1) {
            out.push([p[0], p[i], p[i + 1]]);
        }
    }
    out
}

/// Ray-parity point-in-mesh test (ray along +X, offset slightly in Y/Z to dodge
/// edge hits).
fn inside(p: Vec3, tris: &[Tri]) -> bool {
    // Asymmetric jitter so the ray never lies on a quad's fan diagonal
    // (y == z) or an axis-aligned edge.
    let dir = Vec3::new(1.0, 0.001_37, 0.000_79);
    let mut crossings = 0;
    for t in tris {
        if let Some(dist) = ray_tri(p, dir, t) {
            if dist > 1e-9 {
                crossings += 1;
            }
        }
    }
    crossings % 2 == 1
}

fn ray_tri(origin: Vec3, dir: Vec3, tri: &Tri) -> Option<f64> {
    let e1 = tri[1].sub(tri[0]);
    let e2 = tri[2].sub(tri[0]);
    let pv = dir.cross(e2);
    let det = e1.dot(pv);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let tv = origin.sub(tri[0]);
    let u = tv.dot(pv) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let qv = tv.cross(e1);
    let v = dir.dot(qv) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(qv) * inv;
    (t >= 0.0).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn cube_rasterises_to_a_solid_block() {
        let m = primitives::cube(4.0); // spans ±2
        let g = mesh_to_volume(&m, 1.0);
        // Roughly a 4×4×4 solid interior (plus padding), so ~64 occupied.
        assert!(g.occupied_count() >= 27 && g.occupied_count() <= 125);
        // A cell at the centre is inside; a cell far outside is not.
        let ci = ((Vec3::ZERO.sub(g.origin).x) / g.cell).round() as usize;
        assert!(g.get(ci as i64, ci as i64, ci as i64));
        assert!(!g.get(0, 0, 0));
    }

    #[test]
    fn round_trip_is_a_closed_blocky_surface() {
        let m = primitives::cube(4.0);
        let g = mesh_to_volume(&m, 1.0);
        let s = volume_to_mesh(&g);
        assert!(s.face_count() > 0);
        assert_eq!(
            s.euler_characteristic(),
            2,
            "blocky surface is closed genus-0"
        );
    }

    #[test]
    fn voxel_remesh_of_a_sphere_stays_closed() {
        let m = primitives::uv_sphere(16, 10, 2.0);
        let r = voxel_remesh(&m, 0.6, 0);
        assert!(r.face_count() > 0);
        assert_eq!(r.euler_characteristic(), 2);

        let smooth = voxel_remesh(&m, 0.6, 3);
        assert_eq!(smooth.euler_characteristic(), 2);
        // Smoothing pulled the blocky corners in → closer to radius 2.
        let (lo, hi) = crate::measure::bounding_box(&smooth);
        assert!(hi.sub(lo).x <= 5.0);
    }

    #[test]
    fn empty_grid_returns_the_input() {
        let mut m = Mesh::new();
        let a = m.add_vertex(Vec3::new(0.0, 0.0, 0.0));
        let b = m.add_vertex(Vec3::new(1.0, 0.0, 0.0));
        let c = m.add_vertex(Vec3::new(0.0, 1.0, 0.0));
        m.add_face(&[a, b, c]); // open, nothing inside
        let r = voxel_remesh(&m, 0.3, 0);
        assert_eq!(r.face_count(), 1);
    }
}
