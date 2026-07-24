//! Castellated Cartesian **carve** — body-fit a closed surface (or a region
//! between surfaces) into a volume mesh by keeping the background-grid cells
//! that lie inside it.
//!
//! This is the core of cfMesh's `cartesianMesh` (and snappyHexMesh's
//! castellation): overlay a uniform Cartesian grid on the surface's bounding
//! box, decide which cells are *inside* the region, and keep them. The kept
//! cells form a [`VolumeMesh`] whose boundary is the "staircase" approximation
//! of the input surface(s) (run [`crate::snap::snap_to_surface`] to body-fit it).
//!
//! Two entry points:
//! - [`carve_box`] — the interior of a single closed surface.
//! - [`carve_region`] — the interior of an *outer* surface minus one or more
//!   inner *hole* surfaces (the shell/annular pattern: coolant around fuel,
//!   a vessel minus its internals).
//!
//! # v1 scope
//!
//! - **Staircase boundary** — cells are kept whole; snap the result to
//!   body-fit. - **Inside test** — ray parity (Möller–Trumbore) from each cell
//!   centre; correct for a **closed, watertight** triangle soup. All boundary
//!   faces land in a single `walls` patch (per-surface patch separation is a
//!   later refinement). Pure Rust, Android-safe.

use crate::math::Vec3;
use crate::volume_mesh::{orient_ring, BoundaryPatch, VolumeMesh};
use std::collections::HashMap;

/// Carve the closed surface (`points`, triangle indices `tris`) into a
/// [`VolumeMesh`] of uniform `cell_size` hexahedra: every cell whose centre is
/// inside the surface is kept.
///
/// Returns an **empty** mesh if `cell_size <= 0`, there are fewer than four
/// points, or no cell centre lands inside.
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, carve::carve_box};
///
/// // An axis-aligned box surface [0,2]³ carved at cell size 0.5 recovers the
/// // box exactly: 4³ = 64 cells, volume 8.
/// let (pts, tris) = box_surface(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
/// let m = carve_box(&pts, &tris, 0.5);
/// assert_eq!(m.cell_count(), 64);
/// assert!((m.total_volume() - 8.0).abs() < 1e-9);
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
    if cell_size <= 0.0 || points.len() < 4 || tris.is_empty() {
        return empty_mesh();
    }
    let (grid_min, nx, ny, nz) = grid_for(points, cell_size);
    let (kept, n_kept) = classify(nx, ny, nz, grid_min, cell_size, |c| inside(c, points, tris));
    assemble_kept(kept, n_kept, nx, ny, nz, grid_min, cell_size)
}

/// Carve the region **inside** `outer` (`outer_points`, `outer_tris`) but
/// **outside** every surface in `holes`, into a [`VolumeMesh`] of uniform
/// `cell_size` hexahedra.
///
/// Each hole is a `(points, tris)` closed surface. A cell is kept iff its centre
/// is inside `outer` and inside *no* hole — the shell/annular pattern (coolant
/// around fuel pins or pebbles, a vessel minus its internals). All exposed faces
/// (outer *and* hole boundaries) land in one `walls` patch in v1.
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, carve::carve_region};
///
/// // A hollow shell: box [0,4]³ minus box [1,3]³, at cell size 1 (grid-aligned).
/// let (op, ot) = box_surface(Vec3::ZERO, Vec3::new(4.0, 4.0, 4.0));
/// let (ip, it) = box_surface(Vec3::new(1.0, 1.0, 1.0), Vec3::new(3.0, 3.0, 3.0));
/// let m = carve_region(&op, &ot, &[(&ip[..], &it[..])], 1.0);
/// assert!((m.total_volume() - (64.0 - 8.0)).abs() < 1e-9); // 56
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
pub fn carve_region(
    outer_points: &[Vec3],
    outer_tris: &[[usize; 3]],
    holes: &[(&[Vec3], &[[usize; 3]])],
    cell_size: f64,
) -> VolumeMesh {
    if cell_size <= 0.0 || outer_points.len() < 4 || outer_tris.is_empty() {
        return empty_mesh();
    }
    let (grid_min, nx, ny, nz) = grid_for(outer_points, cell_size);
    let (kept, n_kept) = classify(nx, ny, nz, grid_min, cell_size, |c| {
        inside(c, outer_points, outer_tris) && holes.iter().all(|(hp, ht)| !inside(c, hp, ht))
    });
    assemble_kept(kept, n_kept, nx, ny, nz, grid_min, cell_size)
}

// ---------------------------------------------------------------------------
// Shared pipeline: grid → classify → assemble
// ---------------------------------------------------------------------------

fn empty_mesh() -> VolumeMesh {
    VolumeMesh { points: vec![], faces: vec![], owner: vec![], neighbour: vec![], n_cells: 0, patches: vec![] }
}

/// Uniform grid over the bounding box of `points`, with a one-cell margin so a
/// layer of cells lies outside the surface. Returns `(grid_min, nx, ny, nz)`.
fn grid_for(points: &[Vec3], cs: f64) -> (Vec3, usize, usize, usize) {
    let mut lo = points[0];
    let mut hi = points[0];
    for p in points {
        lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    let grid_min = lo.sub(Vec3::new(cs, cs, cs));
    let nx = (((hi.x - lo.x) / cs).ceil() as usize) + 2;
    let ny = (((hi.y - lo.y) / cs).ceil() as usize) + 2;
    let nz = (((hi.z - lo.z) / cs).ceil() as usize) + 2;
    (grid_min, nx, ny, nz)
}

/// Classify each grid cell by a keep predicate on its centre, assigning compact
/// ids to kept cells. Returns `(kept[flat] = Some(id), n_kept)`.
fn classify(
    nx: usize,
    ny: usize,
    nz: usize,
    grid_min: Vec3,
    cs: f64,
    keep: impl Fn(Vec3) -> bool,
) -> (Vec<Option<usize>>, usize) {
    let cell_center = |i: usize, j: usize, k: usize| {
        Vec3::new(
            grid_min.x + cs * (i as f64 + 0.5),
            grid_min.y + cs * (j as f64 + 0.5),
            grid_min.z + cs * (k as f64 + 0.5),
        )
    };
    let mut kept = vec![None; nx * ny * nz];
    let mut n = 0;
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                if keep(cell_center(i, j, k)) {
                    kept[i + nx * (j + ny * k)] = Some(n);
                    n += 1;
                }
            }
        }
    }
    (kept, n)
}

/// Assemble the [`VolumeMesh`] of the kept cells: internal faces between two
/// kept cells, exposed faces as the `walls` boundary patch; points and cells
/// compacted.
fn assemble_kept(
    kept: Vec<Option<usize>>,
    n_kept: usize,
    nx: usize,
    ny: usize,
    nz: usize,
    grid_min: Vec3,
    cs: f64,
) -> VolumeMesh {
    if n_kept == 0 {
        return empty_mesh();
    }
    let np = |i: usize, j: usize, k: usize| i + (nx + 1) * (j + (ny + 1) * k);
    let flat = |i: usize, j: usize, k: usize| i + nx * (j + ny * k);
    let lattice_pos = |i: usize, j: usize, k: usize| {
        Vec3::new(grid_min.x + cs * i as f64, grid_min.y + cs * j as f64, grid_min.z + cs * k as f64)
    };
    let cell_center =
        |i: usize, j: usize, k: usize| lattice_pos(i, j, k).add(Vec3::new(cs, cs, cs).scale(0.5));

    let mut new_positions: Vec<Vec3> = Vec::new();
    let mut point_remap: HashMap<usize, usize> = HashMap::new();
    let ring_of = |lattice: [usize; 4], positions: &mut Vec<Vec3>, remap: &mut HashMap<usize, usize>| -> Vec<usize> {
        lattice
            .iter()
            .map(|&l| {
                *remap.entry(l).or_insert_with(|| {
                    let i = l % (nx + 1);
                    let j = (l / (nx + 1)) % (ny + 1);
                    let k = l / ((nx + 1) * (ny + 1));
                    positions.push(lattice_pos(i, j, k));
                    positions.len() - 1
                })
            })
            .collect()
    };

    let mut int_faces: Vec<Vec<usize>> = Vec::new();
    let mut int_owner: Vec<usize> = Vec::new();
    let mut int_nb: Vec<usize> = Vec::new();
    let mut bnd_faces: Vec<Vec<usize>> = Vec::new();
    let mut bnd_owner: Vec<usize> = Vec::new();

    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let Some(cid) = kept[flat(i, j, k)] else { continue };
                let oc = cell_center(i, j, k);
                let sides: [(isize, isize, isize, [usize; 4], bool); 6] = [
                    (1, 0, 0, [np(i + 1, j, k), np(i + 1, j + 1, k), np(i + 1, j + 1, k + 1), np(i + 1, j, k + 1)], true),
                    (0, 1, 0, [np(i, j + 1, k), np(i + 1, j + 1, k), np(i + 1, j + 1, k + 1), np(i, j + 1, k + 1)], true),
                    (0, 0, 1, [np(i, j, k + 1), np(i + 1, j, k + 1), np(i + 1, j + 1, k + 1), np(i, j + 1, k + 1)], true),
                    (-1, 0, 0, [np(i, j, k), np(i, j + 1, k), np(i, j + 1, k + 1), np(i, j, k + 1)], false),
                    (0, -1, 0, [np(i, j, k), np(i + 1, j, k), np(i + 1, j, k + 1), np(i, j, k + 1)], false),
                    (0, 0, -1, [np(i, j, k), np(i + 1, j, k), np(i + 1, j + 1, k), np(i, j + 1, k)], false),
                ];
                for (di, dj, dk, corners, positive) in sides {
                    let (ni, nj, nk) = (i as isize + di, j as isize + dj, k as isize + dk);
                    let nbr = in_grid(ni, nj, nk, nx, ny, nz).and_then(|(a, b, c)| kept[flat(a, b, c)]);
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
                        (false, Some(_)) => {}
                    }
                }
            }
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

    /// V&V — carve_box: a grid-aligned box carves exactly (unchanged behaviour
    /// after the refactor): 64 cells, volume 8, 96 boundary faces, closed.
    #[test]
    fn grid_aligned_box_carves_exactly() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
        let m = carve_box(&p, &t, 0.5);
        assert_eq!(m.cell_count(), 64);
        assert!((m.total_volume() - 8.0).abs() < 1e-9);
        assert_eq!(m.n_boundary_faces(), 96);
        m.validate().expect("closed");
    }

    /// V&V — carve_box: an octahedron carve converges to its analytic volume.
    #[test]
    fn octahedron_carve_approximates_volume() {
        let (p, t) = octahedron(1.0);
        let m = carve_box(&p, &t, 0.05);
        let rel = (m.total_volume() - 4.0 / 3.0).abs() / (4.0 / 3.0);
        assert!(rel < 0.05, "carved volume within 5%");
        m.validate().expect("closed");
    }

    /// V&V — headline for carve_region. Methodology: outer box [0,4]³ minus
    /// inner box [1,3]³ at cell size 1 (both grid-aligned). Pass criterion: a
    /// hollow shell of the exact difference volume. Result: 64 − 8 = 56 cells
    /// (unit cells), volume 56, a valid closed mesh with a hollow interior; the
    /// boundary encloses both the outer surface and the inner cavity.
    #[test]
    fn shell_region_between_two_boxes() {
        let (op, ot) = box_surface(Vec3::ZERO, Vec3::new(4.0, 4.0, 4.0));
        let (ip, it) = box_surface(Vec3::new(1.0, 1.0, 1.0), Vec3::new(3.0, 3.0, 3.0));
        let m = carve_region(&op, &ot, &[(&ip[..], &it[..])], 1.0);
        assert_eq!(m.cell_count(), 56, "64 outer − 8 inner unit cells");
        assert!((m.total_volume() - 56.0).abs() < 1e-9, "exact shell volume");
        m.validate().expect("shell cells are closed");
        // The cavity means more boundary faces than a solid block of the same span.
        assert!(m.n_boundary_faces() > 96, "outer + inner cavity walls");
    }

    /// V&V — carve_region with no holes equals carve_box.
    #[test]
    fn region_with_no_holes_is_carve_box() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
        let a = carve_box(&p, &t, 0.5);
        let b = carve_region(&p, &t, &[], 0.5);
        assert_eq!(a.cell_count(), b.cell_count());
        assert!((a.total_volume() - b.total_volume()).abs() < 1e-12);
    }
}
