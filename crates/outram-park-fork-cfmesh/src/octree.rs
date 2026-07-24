//! Octree near-wall **refinement** — grade the mesh finer near the surface,
//! keeping it conforming by splitting the coarse transition faces.
//!
//! [`crate::carve::carve_box`] produces a *uniform* grid. This module refines
//! the cells adjacent to the boundary one level finer (cfMesh's octree
//! `meshOctree` refinement, and snappyHexMesh's castellation refinement). Where
//! a coarse cell meets four finer cells, the coarse cell's shared face is
//! represented as the **four fine sub-faces** — the hanging-node treatment that
//! keeps the mesh conforming and turns the coarse transition cell into a
//! genuine **polyhedron** (more than six faces).
//!
//! # v1 scope
//!
//! A single level of refinement on the boundary-adjacent cells (interior stays
//! coarse). One level is automatically 2:1-balanced (levels differ by at most
//! one everywhere), so no balancing pass is needed yet; multi-level graded
//! refinement with a balancing pass is the next step. Pure Rust, Android-safe.

use crate::math::Vec3;
use crate::volume_mesh::{orient_ring, BoundaryPatch, VolumeMesh};
use std::collections::{HashMap, HashSet};

/// A leaf cell: `(level, i, j, k)`. A level-`L` cell has edge `base / 2^L` and
/// occupies finest-resolution integer coordinates `[i·m, (i+1)·m]` per axis,
/// with `m = 2^(MAX_LEVEL − L)`.
type Cell = (u8, i64, i64, i64);

/// v1 refines to one level below the base grid.
const MAX_LEVEL: u8 = 1;

/// Carve the closed surface (`points`, `tris`) at `base_cell_size`, then refine
/// the boundary-adjacent cells one level finer, returning the graded
/// [`VolumeMesh`]. Coarse cells at a refinement interface become polyhedral
/// (their shared face is the four fine sub-faces).
///
/// Returns an empty mesh for a degenerate input (see [`crate::carve::carve_box`]).
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, octree::refine_near_boundary};
///
/// // A box refined near its walls keeps the exact box volume and stays closed.
/// let (p, t) = box_surface(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
/// let m = refine_near_boundary(&p, &t, 0.5);
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
pub fn refine_near_boundary(points: &[Vec3], tris: &[[usize; 3]], base_cell_size: f64) -> VolumeMesh {
    if base_cell_size <= 0.0 || points.len() < 4 || tris.is_empty() {
        return empty();
    }
    let cs = base_cell_size;

    // Base grid over the bounding box (one-cell margin), same as the carver.
    let (mut lo, mut hi) = (points[0], points[0]);
    for p in points {
        lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    let origin = lo.sub(Vec3::new(cs, cs, cs));
    let nx = (((hi.x - lo.x) / cs).ceil() as i64) + 2;
    let ny = (((hi.y - lo.y) / cs).ceil() as i64) + 2;
    let nz = (((hi.z - lo.z) / cs).ceil() as i64) + 2;

    // Level-0 cell centre and inside test.
    let center0 = |i: i64, j: i64, k: i64| {
        Vec3::new(
            origin.x + cs * (i as f64 + 0.5),
            origin.y + cs * (j as f64 + 0.5),
            origin.z + cs * (k as f64 + 0.5),
        )
    };
    let mut kept0: HashSet<(i64, i64, i64)> = HashSet::new();
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                if inside(center0(i, j, k), points, tris) {
                    kept0.insert((i, j, k));
                }
            }
        }
    }
    if kept0.is_empty() {
        return empty();
    }

    // A boundary cell has a face-neighbour that is not kept. Refine those.
    let is_boundary = |c: &(i64, i64, i64)| {
        const D: [(i64, i64, i64); 6] =
            [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)];
        D.iter().any(|(di, dj, dk)| !kept0.contains(&(c.0 + di, c.1 + dj, c.2 + dk)))
    };

    // Leaf set: interior cells stay level 0; boundary cells split into 8
    // level-1 children.
    let mut leaves: HashSet<Cell> = HashSet::new();
    for &(i, j, k) in &kept0 {
        if is_boundary(&(i, j, k)) {
            for a in 0..2 {
                for b in 0..2 {
                    for c in 0..2 {
                        leaves.insert((1, 2 * i + a, 2 * j + b, 2 * k + c));
                    }
                }
            }
        } else {
            leaves.insert((0, i, j, k));
        }
    }

    build_mesh(&leaves, origin, cs)
}

fn empty() -> VolumeMesh {
    VolumeMesh { points: vec![], faces: vec![], owner: vec![], neighbour: vec![], n_cells: 0, patches: vec![] }
}

/// Build the [`VolumeMesh`] from a 2:1-balanced leaf set (levels 0 and 1 only).
fn build_mesh(leaves: &HashSet<Cell>, origin: Vec3, base_cs: f64) -> VolumeMesh {
    let h_fine = base_cs / (1u32 << MAX_LEVEL) as f64; // finest cell edge
    let span = |level: u8| 1i64 << (MAX_LEVEL - level); // finest units per cell edge

    // Compact cell ids and point ids (points keyed by finest integer coord).
    let cell_id: HashMap<Cell, usize> = leaves.iter().enumerate().map(|(n, &c)| (c, n)).collect();
    let mut points: Vec<Vec3> = Vec::new();
    let mut pt_id: HashMap<(i64, i64, i64), usize> = HashMap::new();
    let mut pt_of = |fi: i64, fj: i64, fk: i64, points: &mut Vec<Vec3>| -> usize {
        *pt_id.entry((fi, fj, fk)).or_insert_with(|| {
            points.push(Vec3::new(
                origin.x + h_fine * fi as f64,
                origin.y + h_fine * fj as f64,
                origin.z + h_fine * fk as f64,
            ));
            points.len() - 1
        })
    };

    // Cell centre (physical) and the 4 finest-coord corners of a face.
    let center = |c: Cell| {
        let m = span(c.0) as f64;
        Vec3::new(
            origin.x + h_fine * (c.1 as f64 * m + m / 2.0),
            origin.y + h_fine * (c.2 as f64 * m + m / 2.0),
            origin.z + h_fine * (c.3 as f64 * m + m / 2.0),
        )
    };
    // Face corners (finest coords) for axis (0/1/2) and sign (+1/-1).
    let face_corners = |c: Cell, axis: usize, sign: i64| -> [(i64, i64, i64); 4] {
        let m = span(c.0);
        let base = [c.1 * m, c.2 * m, c.3 * m];
        let fixed = if sign > 0 { base[axis] + m } else { base[axis] };
        let (u, v) = match axis {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        let corner = |du: i64, dv: i64| {
            let mut p = [base[0], base[1], base[2]];
            p[axis] = fixed;
            p[u] += du;
            p[v] += dv;
            (p[0], p[1], p[2])
        };
        [corner(0, 0), corner(m, 0), corner(m, m), corner(0, m)]
    };

    // Neighbour classification across a face.
    let same_neighbour = |c: Cell, axis: usize, sign: i64| -> Cell {
        let mut n = [c.1, c.2, c.3];
        n[axis] += sign;
        (c.0, n[0], n[1], n[2])
    };
    let refined = |nc: Cell, leaves: &HashSet<Cell>| -> bool {
        // nc is a level-`L` coord; is its region split into level L+1 leaves?
        nc.0 < MAX_LEVEL && leaves.contains(&(nc.0 + 1, 2 * nc.1, 2 * nc.2, 2 * nc.3))
    };
    let coarser_leaf = |nc: Cell, leaves: &HashSet<Cell>| -> Option<Cell> {
        if nc.0 == 0 {
            return None;
        }
        let p = (nc.0 - 1, nc.1.div_euclid(2), nc.2.div_euclid(2), nc.3.div_euclid(2));
        if leaves.contains(&p) {
            Some(p)
        } else {
            None
        }
    };

    let mut int_faces: Vec<Vec<usize>> = Vec::new();
    let mut int_owner: Vec<usize> = Vec::new();
    let mut int_nb: Vec<usize> = Vec::new();
    let mut bnd_faces: Vec<Vec<usize>> = Vec::new();
    let mut bnd_owner: Vec<usize> = Vec::new();

    const DIRS: [(usize, i64); 6] = [(0, 1), (0, -1), (1, 1), (1, -1), (2, 1), (2, -1)];
    for (&c, &cid) in &cell_id {
        let oc = center(c);
        for (axis, sign) in DIRS {
            let nc = same_neighbour(c, axis, sign);
            // 1. Neighbour region refined finer -> this is the coarse side; the
            //    finer cells emit the sub-faces. Skip.
            if refined(nc, leaves) {
                continue;
            }
            // 2. Same-level neighbour: emit once, from the positive-direction side.
            if let Some(&nid) = cell_id.get(&nc) {
                if sign > 0 {
                    let ring = ring_from(face_corners(c, axis, sign), &mut pt_of, &mut points);
                    int_faces.push(orient_ring(ring, oc, &points));
                    int_owner.push(cid);
                    int_nb.push(nid);
                }
                continue;
            }
            // 3. Coarser neighbour: this (finer) cell owns the split sub-face.
            if let Some(coarse) = coarser_leaf(nc, leaves) {
                let nid = cell_id[&coarse];
                let ring = ring_from(face_corners(c, axis, sign), &mut pt_of, &mut points);
                int_faces.push(orient_ring(ring, oc, &points));
                int_owner.push(cid);
                int_nb.push(nid);
                continue;
            }
            // 4. No kept neighbour: boundary face.
            let ring = ring_from(face_corners(c, axis, sign), &mut pt_of, &mut points);
            bnd_faces.push(orient_ring(ring, oc, &points));
            bnd_owner.push(cid);
        }
    }

    let n_internal = int_faces.len();
    let mut faces = int_faces;
    let mut owner = int_owner;
    let mut neighbour: Vec<Option<usize>> = int_nb.into_iter().map(Some).collect();
    let n_boundary = bnd_faces.len();
    faces.extend(bnd_faces);
    owner.extend(bnd_owner);
    neighbour.extend(std::iter::repeat(None).take(n_boundary));
    let patches = vec![BoundaryPatch { name: "walls".into(), start_face: n_internal, n_faces: n_boundary }];
    VolumeMesh { points, faces, owner, neighbour, n_cells: leaves.len(), patches }
}

/// Resolve a face's finest-coord corners to compacted point indices.
fn ring_from(
    corners: [(i64, i64, i64); 4],
    pt_of: &mut impl FnMut(i64, i64, i64, &mut Vec<Vec3>) -> usize,
    points: &mut Vec<Vec3>,
) -> Vec<usize> {
    corners.iter().map(|&(i, j, k)| pt_of(i, j, k, points)).collect()
}

/// Point-in-closed-surface test by ray parity (Möller–Trumbore).
fn inside(p: Vec3, points: &[Vec3], tris: &[[usize; 3]]) -> bool {
    let dir = Vec3::new(0.131_537, 0.755_605, 0.642_020);
    let mut crossings = 0usize;
    for t in tris {
        if ray_triangle(p, dir, points[t[0]], points[t[1]], points[t[2]]) {
            crossings += 1;
        }
    }
    crossings % 2 == 1
}

fn ray_triangle(orig: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> bool {
    let e1 = b.sub(a);
    let e2 = c.sub(a);
    let pv = dir.cross(e2);
    let det = e1.dot(pv);
    if det.abs() < 1e-12 {
        return false;
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

    /// Number of faces referencing each cell (owner or neighbour) — a cell with
    /// more than six is polyhedral (a refinement transition cell).
    fn faces_per_cell(m: &VolumeMesh) -> Vec<usize> {
        let mut n = vec![0usize; m.n_cells];
        for f in 0..m.face_count() {
            n[m.owner[f]] += 1;
            if let Some(nb) = m.neighbour[f] {
                n[nb] += 1;
            }
        }
        n
    }

    /// V&V — headline. Methodology: carve a box [0,4]³ at base cell size 1, then
    /// refine the boundary-adjacent cells one level. Pass criteria: cells still
    /// tile the box exactly, every cell (including polyhedral transition cells)
    /// is closed, refinement added cells, and at least one cell has > 6 faces.
    /// Result: total volume = 64 (exact); validate() Ok; cell count grows from
    /// 64 to well above it; the coarse cells bordering the refined shell are
    /// polyhedra (their split face is 4 sub-faces).
    #[test]
    fn refined_box_is_exact_closed_and_polyhedral() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(4.0, 4.0, 4.0));
        let m = refine_near_boundary(&p, &t, 1.0);
        assert!((m.total_volume() - 64.0).abs() < 1e-9, "refinement preserves exact volume: {}", m.total_volume());
        m.validate().expect("every cell (incl. polyhedral) is closed");
        assert!(m.cell_count() > 64, "refinement added cells: {}", m.cell_count());
        let fpc = faces_per_cell(&m);
        assert!(fpc.iter().any(|&n| n > 6), "some transition cell is polyhedral (> 6 faces)");
    }

    /// V&V — refinement really did happen near the wall: the refined mesh has
    /// more, smaller cells than the uniform carve of the same box.
    #[test]
    fn refinement_increases_resolution() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(4.0, 4.0, 4.0));
        let refined = refine_near_boundary(&p, &t, 1.0);
        // Uniform carve would be 4³ = 64 cells; the interior 2³ = 8 stay coarse,
        // the 56-cell boundary shell splits into 8 each -> 8 + 56*8 = 456.
        assert_eq!(refined.cell_count(), 456);
        let fpc = faces_per_cell(&refined);
        assert!(fpc.iter().sum::<usize>() > 6 * 64, "more faces than a uniform mesh");
    }
}
