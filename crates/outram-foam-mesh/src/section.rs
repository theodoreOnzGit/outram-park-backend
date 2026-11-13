//! Plane sections and boundary edges of a [`PolyMesh`], for drawing the mesh
//! the solver gets and the cell fields on it (gh:#1155, Foam Studio).
//!
//! [`plane_section`] cuts every cell with an axis-aligned plane: each face
//! edge that crosses the plane gives a point to both cells of the face, and a
//! cell's section is the convex hull of its points. That is exact for convex
//! cells (every hex of [`block_mesh`](crate::block_mesh)); for a non-convex
//! polyhedron it is the hull of the true section. A vertex lying on the plane
//! counts as above it, so a plane through a layer of faces gives one polygon
//! per cell below it rather than two.
use crate::block_mesh::PolyMesh;
use outram_foam_basic_lib::primitives::Vector3;
use uom::si::f64::Length;
use uom::si::length::meter;

/// The normal of an axis-aligned cutting plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Plane x = const; in-plane coordinates (y, z).
    X,
    /// Plane y = const; in-plane coordinates (x, z).
    Y,
    /// Plane z = const; in-plane coordinates (x, y).
    Z,
}

impl Axis {
    fn normal(self, p: Vector3) -> f64 {
        match self {
            Axis::X => p.x,
            Axis::Y => p.y,
            Axis::Z => p.z,
        }
    }

    /// The two in-plane coordinates of `p`, in metres.
    pub fn in_plane(self, p: Vector3) -> [f64; 2] {
        match self {
            Axis::X => [p.y, p.z],
            Axis::Y => [p.x, p.z],
            Axis::Z => [p.x, p.y],
        }
    }
}

/// One cell's section: the cell index and its polygon (counter-clockwise,
/// in-plane coordinates in metres, see [`Axis`]).
#[derive(Debug, Clone, PartialEq)]
pub struct CellSection {
    /// Cell index in the mesh (the solver's cell order).
    pub cell: usize,
    /// Polygon vertices \[m\].
    pub polygon: Vec<[f64; 2]>,
}

/// Cut `mesh` with the plane `axis = offset`; one [`CellSection`] per cell the
/// plane passes through, in cell order.
pub fn plane_section(mesh: &PolyMesh, axis: Axis, offset: Length) -> Vec<CellSection> {
    let h = offset.get::<meter>();
    // A point within round-off of the plane is ON it (treated as the
    // non-negative side). Node coordinates carry a few ulp of blend round-off
    // (gh:#1464: blockMesh's 12-edge blend), so a plane that is meant to pass
    // exactly through a layer of nodes must not split that layer.
    let extent = mesh
        .points
        .iter()
        .map(|p| axis.normal(*p).abs())
        .fold(h.abs(), f64::max);
    let tol = 1.0e-12 * extent;
    let snap = |d: f64, tol: f64| if d.abs() <= tol { 0.0 } else { d };
    let mut pts: Vec<Vec<[f64; 2]>> = vec![Vec::new(); mesh.n_cells];
    for f in &mesh.faces {
        let n = f.verts.len();
        for i in 0..n {
            let a = mesh.points[f.verts[i]];
            let b = mesh.points[f.verts[(i + 1) % n]];
            let (da, db) = (snap(axis.normal(a) - h, tol), snap(axis.normal(b) - h, tol));
            if (da < 0.0) == (db < 0.0) {
                continue;
            }
            let t = da / (da - db);
            let p = axis.in_plane(a + (b - a) * t);
            pts[f.owner].push(p);
            if let Some(nb) = f.neighbour {
                pts[nb].push(p);
            }
        }
    }
    pts.into_iter()
        .enumerate()
        .filter_map(|(cell, p)| {
            let hull = convex_hull(p);
            (hull.len() >= 3).then_some(CellSection { cell, polygon: hull })
        })
        .collect()
}

/// Andrew's monotone chain; drops duplicates and collinear points.
fn convex_hull(mut p: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    p.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    p.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-14 && (a[1] - b[1]).abs() < 1e-14);
    if p.len() < 3 {
        return p;
    }
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let mut h: Vec<[f64; 2]> = Vec::with_capacity(2 * p.len());
    for pass in 0..2 {
        let start = h.len();
        let iter: Vec<[f64; 2]> = if pass == 0 { p.clone() } else { p.iter().rev().copied().collect() };
        for q in iter {
            while h.len() >= start + 2 && cross(h[h.len() - 2], h[h.len() - 1], q) <= 1e-30 {
                h.pop();
            }
            h.push(q);
        }
        h.pop();
    }
    h
}

/// Every distinct edge of the boundary faces, as two points \[m\] and the
/// index of the patch it was first met on: the wireframe of what the solver
/// meshes.
pub fn boundary_edges(mesh: &PolyMesh) -> Vec<([f64; 3], [f64; 3], usize)> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for (pi, patch) in mesh.patches.iter().enumerate() {
        for f in &mesh.faces[patch.start..patch.start + patch.size] {
            let n = f.verts.len();
            for i in 0..n {
                let (a, b) = (f.verts[i], f.verts[(i + 1) % n]);
                if seen.insert((a.min(b), a.max(b))) {
                    let (p, q) = (mesh.points[a], mesh.points[b]);
                    out.push(([p.x, p.y, p.z], [q.x, q.y, q.z], pi));
                }
            }
        }
    }
    out
}

/// Area of a polygon (shoelace), in the square of its coordinates' unit.
pub fn polygon_area(p: &[[f64; 2]]) -> f64 {
    let n = p.len();
    0.5 * (0..n).map(|i| p[i][0] * p[(i + 1) % n][1] - p[(i + 1) % n][0] * p[i][1]).sum::<f64>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_primitives::{BoxBlock, PipeOGrid};

    fn m(v: f64) -> Length {
        Length::new::<meter>(v)
    }

    #[test]
    fn pipe_cross_section_tiles_the_polygon() {
        let g = PipeOGrid::new(m(0.01), m(0.2), 6, 4, 40);
        let pm = g.mesh().unwrap();
        let s = plane_section(&pm, Axis::Z, m(0.1013));
        assert_eq!(s.len(), 36 + 96, "one layer of cells");
        let area: f64 = s.iter().map(|c| polygon_area(&c.polygon)).sum();
        assert!((area / g.polygon_area() - 1.0).abs() < 1e-12, "area {area}");
        // A plane exactly on a layer of faces still gives one layer.
        assert_eq!(plane_section(&pm, Axis::Z, m(0.1)).len(), 132);
        let ax = plane_section(&pm, Axis::Y, m(1.3e-4));
        let a: f64 = ax.iter().map(|c| polygon_area(&c.polygon)).sum();
        assert!(a > 0.0 && (a - 0.02 * 0.2).abs() < 0.01 * 0.02 * 0.2, "axial area {a}");
    }

    #[test]
    fn box_edges_and_degenerate_hulls() {
        let b = BoxBlock::new([m(1.0), m(1.0), m(1.0)], [2, 2, 2]);
        let pm = b.mesh().unwrap();
        // 6 faces x 4 quads; distinct edges on the surface of a 2x2x2 block: 48.
        assert_eq!(boundary_edges(&pm).len(), 48);
        assert!(plane_section(&pm, Axis::X, m(2.0)).is_empty());
        assert!(convex_hull(vec![[0.0, 0.0], [1.0, 1.0]]).len() == 2);
        let sq = convex_hull(vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.5, 0.5], [1.0, 0.5]]);
        assert_eq!(sq.len(), 4);
        assert!((polygon_area(&sq) - 1.0).abs() < 1e-15);
    }
}
