//! Castellated Cartesian **carve** — body-fit a closed surface into a volume
//! mesh by keeping the background-grid cells that lie inside it.
//!
//! This is the core of cfMesh's `cartesianMesh` (and snappyHexMesh's
//! castellation): overlay a uniform Cartesian grid on the surface's bounding
//! box, decide which cells are *inside* the surface, and keep them. The kept
//! cells form a [`VolumeMesh`] whose boundary is the "staircase" approximation
//! of the input surface.
//!
//! # v1 scope
//!
//! - **Staircase boundary** — cells are kept whole; boundary points are **not**
//!   yet snapped to the surface, so the wall is a voxelised approximation that
//!   sharpens as `cell_size` shrinks. Octree refinement near the surface and
//!   point snapping are the next milestones.
//! - **Inside test** — a ray-parity test (Möller–Trumbore ray/triangle) from
//!   each cell centre along a fixed generic direction; correct for a **closed,
//!   watertight** triangle soup. Cell centres never lie on the surface, so
//!   grazing degeneracies are rare; a genuinely grazing ray is not handled in
//!   v1.
//!
//! Self-contained: the carver takes a triangle soup (`points` + `tris`), so it
//! needs no dependency on the surface-authoring crate; a thin bridge from an
//! `outram-blender` `Mesh` is a later add. Pure Rust, Android-safe.

use crate::math::Vec3;
use crate::volume_mesh::{orient_ring, BoundaryPatch, VolumeMesh};
use std::collections::HashMap;

/// Carve the closed surface (`points`, triangle indices `tris`) into a
/// [`VolumeMesh`] of uniform `cell_size` hexahedra.
///
/// A uniform grid of `cell_size` cubes is laid over the surface bounding box
/// (with a one-cell margin); every cell whose centre is inside the surface is
/// kept. Internal faces separate two kept cells; the remaining exposed faces
/// form the carved boundary, collected in a single `walls` patch. Points and
/// cells are compacted, so only kept geometry appears in the result.
///
/// Returns an **empty** mesh if `cell_size <= 0`, there are fewer than four
/// points, or no cell centre lands inside.
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, carve::carve_box};
///
/// // An axis-aligned box surface [0,2]³ (8 corners, 12 triangles) carved at
/// // cell size 0.5 recovers the box exactly: 4³ = 64 cells, volume 8.
/// let (pts, tris) = box_surface(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
/// let m = carve_box(&pts, &tris, 0.5);
/// assert_eq!(m.cell_count(), 64);
/// assert!((m.total_volume() - 8.0).abs() < 1e-9);
/// assert!(m.validate().is_ok());
/// # fn box_surface(a: Vec3, b: Vec3) -> (Vec<Vec3>, Vec<[usize; 3]>) {
/// #     let v = vec![
/// #         Vec3::new(a.x, a.y, a.z), Vec3::new(b.x, a.y, a.z), Vec3::new(b.x, b.y, a.z), Vec3::new(a.x, b.y, a.z),
/// #         Vec3::new(a.x, a.y, b.z), Vec3::new(b.x, a.y, b.z), Vec3::new(b.x, b.y, b.z), Vec3::new(a.x, b.y, b.z)];
/// #     let q = |a:usize,b:usize,c:usize,d:usize| vec![[a,b,c],[a,c,d]];
/// #     let mut t = Vec::new();
/// #     for f in [q(0,3,2,1), q(4,5,6,7), q(0,1,5,4), q(2,3,7,6), q(1,2,6,5), q(0,4,7,3)] { t.extend(f); }
/// #     (v, t)
/// # }
/// ```
pub fn carve_box(points: &[Vec3], tris: &[[usize; 3]], cell_size: f64) -> VolumeMesh {
    let empty =
        || VolumeMesh { points: vec![], faces: vec![], owner: vec![], neighbour: vec![], n_cells: 0, patches: vec![] };
    if cell_size <= 0.0 || points.len() < 4 || tris.is_empty() {
        return empty();
    }

    // Bounding box + one-cell margin so a boundary layer of cells is outside.
    let mut lo = points[0];
    let mut hi = points[0];
    for p in points {
        lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    let cs = cell_size;
    let grid_min = lo.sub(Vec3::new(cs, cs, cs));
    let nx = (((hi.x - lo.x) / cs).ceil() as usize) + 2;
    let ny = (((hi.y - lo.y) / cs).ceil() as usize) + 2;
    let nz = (((hi.z - lo.z) / cs).ceil() as usize) + 2;

    let np = |i: usize, j: usize, k: usize| i + (nx + 1) * (j + (ny + 1) * k);
    let lattice_pos = |i: usize, j: usize, k: usize| {
        Vec3::new(grid_min.x + cs * i as f64, grid_min.y + cs * j as f64, grid_min.z + cs * k as f64)
    };
    let cell_center =
        |i: usize, j: usize, k: usize| lattice_pos(i, j, k).add(Vec3::new(cs, cs, cs).scale(0.5));

    // Inside/outside classification of each cell, and a compact id for kept cells.
    let flat = |i: usize, j: usize, k: usize| i + nx * (j + ny * k);
    let mut kept: Vec<Option<usize>> = vec![None; nx * ny * nz];
    let mut n_kept = 0usize;
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                if inside(cell_center(i, j, k), points, tris) {
                    kept[flat(i, j, k)] = Some(n_kept);
                    n_kept += 1;
                }
            }
        }
    }
    if n_kept == 0 {
        return empty();
    }

    // Point compaction: lattice index -> output index.
    let mut new_positions: Vec<Vec3> = Vec::new();
    let mut point_remap: HashMap<usize, usize> = HashMap::new();
    let ring_of = |lattice: [usize; 4], positions: &mut Vec<Vec3>, remap: &mut HashMap<usize, usize>| -> Vec<usize> {
        lattice
            .iter()
            .map(|&l| {
                *remap.entry(l).or_insert_with(|| {
                    // Recover (i,j,k) from the lattice index to place the point.
                    let i = l % (nx + 1);
                    let j = (l / (nx + 1)) % (ny + 1);
                    let k = l / ((nx + 1) * (ny + 1));
                    positions.push(lattice_pos(i, j, k));
                    positions.len() - 1
                })
            })
            .collect()
    };

    // Internal faces first, boundary faces second (VolumeMesh ordering).
    let mut int_faces: Vec<Vec<usize>> = Vec::new();
    let mut int_owner: Vec<usize> = Vec::new();
    let mut int_nb: Vec<usize> = Vec::new();
    let mut bnd_faces: Vec<Vec<usize>> = Vec::new();
    let mut bnd_owner: Vec<usize> = Vec::new();

    // The six sides of cell (i,j,k) as (neighbour offset, lattice corner ring).
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let Some(cid) = kept[flat(i, j, k)] else { continue };
                let oc = cell_center(i, j, k);
                // +X, +Y, +Z: internal when neighbour kept, else boundary.
                // -X, -Y, -Z: boundary only (internal handled by the neighbour).
                let sides: [(isize, isize, isize, [usize; 4], bool); 6] = [
                    (1, 0, 0, [np(i + 1, j, k), np(i + 1, j + 1, k), np(i + 1, j + 1, k + 1), np(i + 1, j, k + 1)], true),
                    (0, 1, 0, [np(i, j + 1, k), np(i + 1, j + 1, k), np(i + 1, j + 1, k + 1), np(i, j + 1, k + 1)], true),
                    (0, 0, 1, [np(i, j, k + 1), np(i + 1, j, k + 1), np(i + 1, j + 1, k + 1), np(i, j + 1, k + 1)], true),
                    (-1, 0, 0, [np(i, j, k), np(i, j + 1, k), np(i, j + 1, k + 1), np(i, j, k + 1)], false),
                    (0, -1, 0, [np(i, j, k), np(i + 1, j, k), np(i + 1, j, k + 1), np(i, j, k + 1)], false),
                    (0, 0, -1, [np(i, j, k), np(i + 1, j, k), np(i + 1, j + 1, k), np(i, j + 1, k)], false),
                ];
                for (di, dj, dk, corners, positive) in sides {
                    let ni = i as isize + di;
                    let nj = j as isize + dj;
                    let nk = k as isize + dk;
                    let nbr = in_grid(ni, nj, nk, nx, ny, nz)
                        .and_then(|(a, b, c)| kept[flat(a, b, c)]);
                    match (positive, nbr) {
                        (true, Some(nid)) => {
                            let ring = ring_of(corners, &mut new_positions, &mut point_remap);
                            int_faces.push(orient_ring(ring, oc, &new_positions));
                            int_owner.push(cid);
                            int_nb.push(nid);
                        }
                        (_, None) => {
                            let ring = ring_of(corners, &mut new_positions, &mut point_remap);
                            bnd_faces.push(orient_ring(ring, oc, &new_positions));
                            bnd_owner.push(cid);
                        }
                        (false, Some(_)) => { /* internal, handled from the +side neighbour */ }
                    }
                }
            }
        }
    }

    // Concatenate: internal faces, then the single boundary "walls" patch.
    let n_internal = int_faces.len();
    let mut faces = int_faces;
    let mut owner = int_owner;
    let mut neighbour: Vec<Option<usize>> = int_nb.into_iter().map(Some).collect();
    let n_boundary = bnd_faces.len();
    faces.extend(bnd_faces);
    owner.extend(bnd_owner);
    neighbour.extend(std::iter::repeat(None).take(n_boundary));

    let patches = vec![BoundaryPatch { name: "walls".into(), start_face: n_internal, n_faces: n_boundary }];
    VolumeMesh { points: new_positions, faces, owner, neighbour, n_cells: n_kept, patches }
}

/// Clamp a signed cell coordinate into the grid, returning `Some((i,j,k))` if in
/// range.
fn in_grid(i: isize, j: isize, k: isize, nx: usize, ny: usize, nz: usize) -> Option<(usize, usize, usize)> {
    if i < 0 || j < 0 || k < 0 || i as usize >= nx || j as usize >= ny || k as usize >= nz {
        None
    } else {
        Some((i as usize, j as usize, k as usize))
    }
}

/// Point-in-closed-surface test by ray parity: cast a ray from `p` along a fixed
/// generic direction and count triangle crossings; odd ⇒ inside.
fn inside(p: Vec3, points: &[Vec3], tris: &[[usize; 3]]) -> bool {
    // A generic (non-axis, irrational-ish) direction to dodge edge/vertex hits.
    let dir = Vec3::new(0.131_537, 0.755_605, 0.642_020);
    let mut crossings = 0usize;
    for t in tris {
        if ray_triangle(p, dir, points[t[0]], points[t[1]], points[t[2]]) {
            crossings += 1;
        }
    }
    crossings % 2 == 1
}

/// Möller–Trumbore ray/triangle intersection; `true` if the ray from `orig`
/// along `dir` hits triangle `a,b,c` at a strictly positive parameter.
fn ray_triangle(orig: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> bool {
    let e1 = b.sub(a);
    let e2 = c.sub(a);
    let pv = dir.cross(e2);
    let det = e1.dot(pv);
    if det.abs() < 1e-12 {
        return false; // ray parallel to the triangle
    }
    let inv = 1.0 / det;
    let tv = orig.sub(a);
    let u = tv.dot(pv) * inv;
    if !(0.0..=1.0).contains(&u) {
        return false;
    }
    let qv = tv.cross(e1);
    let v = dir.dot(qv) * inv;
    if v < 0.0 || u + v > 1.0 {
        return false;
    }
    e2.dot(qv) * inv > 1e-9
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A box surface [a,b] as 8 corners + 12 triangles (outward winding).
    fn box_surface(a: Vec3, b: Vec3) -> (Vec<Vec3>, Vec<[usize; 3]>) {
        let v = vec![
            Vec3::new(a.x, a.y, a.z),
            Vec3::new(b.x, a.y, a.z),
            Vec3::new(b.x, b.y, a.z),
            Vec3::new(a.x, b.y, a.z),
            Vec3::new(a.x, a.y, b.z),
            Vec3::new(b.x, a.y, b.z),
            Vec3::new(b.x, b.y, b.z),
            Vec3::new(a.x, b.y, b.z),
        ];
        let q = |a: usize, b: usize, c: usize, d: usize| vec![[a, b, c], [a, c, d]];
        let mut t = Vec::new();
        for f in [q(0, 3, 2, 1), q(4, 5, 6, 7), q(0, 1, 5, 4), q(2, 3, 7, 6), q(1, 2, 6, 5), q(0, 4, 7, 3)] {
            t.extend(f);
        }
        (v, t)
    }

    /// An octahedron |x|+|y|+|z| <= r as 6 verts + 8 triangles. Volume (4/3)r³.
    fn octahedron(r: f64) -> (Vec<Vec3>, Vec<[usize; 3]>) {
        let v = vec![
            Vec3::new(r, 0.0, 0.0),
            Vec3::new(-r, 0.0, 0.0),
            Vec3::new(0.0, r, 0.0),
            Vec3::new(0.0, -r, 0.0),
            Vec3::new(0.0, 0.0, r),
            Vec3::new(0.0, 0.0, -r),
        ];
        let t = vec![
            [0, 2, 4], [2, 1, 4], [1, 3, 4], [3, 0, 4],
            [0, 5, 2], [2, 5, 1], [1, 5, 3], [3, 5, 0],
        ];
        (v, t)
    }

    /// V&V — headline. Methodology: carve an axis-aligned box surface [0,2]³ at
    /// cell size 0.5, which divides the box exactly. Pass criterion: the carve
    /// recovers the box exactly. Result: 4³ = 64 cells; total volume 8 (exact);
    /// 6·4² = 96 boundary faces in one `walls` patch; every cell closed.
    #[test]
    fn grid_aligned_box_carves_exactly() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
        let m = carve_box(&p, &t, 0.5);
        assert_eq!(m.cell_count(), 64, "4×4×4 interior cells");
        assert!((m.total_volume() - 8.0).abs() < 1e-9, "exact box volume 8");
        assert_eq!(m.n_boundary_faces(), 96, "6 sides × 4×4 boundary faces");
        assert_eq!(m.patches.len(), 1);
        assert_eq!(m.patches[0].name, "walls");
        m.validate().expect("every carved cell is closed");
    }

    /// V&V — the carved volume converges to the true volume as the cell size
    /// shrinks. Methodology: carve an octahedron of radius 1 (true volume
    /// (4/3)·1³ ≈ 1.3333) at cell size 0.05. Pass criterion: staircase volume
    /// within 5 % of analytic, and a valid closed mesh. Result: |V − 4/3| /
    /// (4/3) < 0.05; validate() Ok.
    #[test]
    fn octahedron_carve_approximates_volume_and_is_closed() {
        let (p, t) = octahedron(1.0);
        let m = carve_box(&p, &t, 0.05);
        let exact = 4.0 / 3.0;
        let rel = (m.total_volume() - exact).abs() / exact;
        assert!(rel < 0.05, "carved volume {} within 5% of {exact} (rel {rel})", m.total_volume());
        assert!(m.cell_count() > 1000, "a resolved carve has many cells");
        m.validate().expect("carved octahedron cells are closed");
    }

    /// V&V — a coarser carve of the octahedron is still a valid closed mesh
    /// (fewer, larger cells), confirming the face bookkeeping holds at low
    /// resolution too.
    #[test]
    fn coarse_carve_is_still_valid() {
        let (p, t) = octahedron(1.0);
        let m = carve_box(&p, &t, 0.25);
        assert!(m.cell_count() >= 1);
        assert_eq!(m.n_internal_faces() + m.n_boundary_faces(), m.face_count());
        m.validate().expect("coarse carve is closed");
    }
}
