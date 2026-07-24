//! The **polyhedral dual** mesh — one polyhedral cell per primal *vertex*, the
//! equivalent of OpenFOAM's `polyDualMesh` (voro++ is the reference for the
//! Voronoi/median-dual idea).
//!
//! A finite-volume solver is usually happier on a **polyhedral** mesh than on a
//! hex/tet mesh of the same region: polyhedra pack more neighbours per cell
//! (better gradient reconstruction) at a much lower cell count. The classic way
//! to get one is to take the *dual* of a primal mesh — turn every primal
//! **vertex** into a cell.
//!
//! # The median dual (works on any primal mesh)
//!
//! This is the **median** (a.k.a. Donald / vertex-centred) dual, not the
//! circumcentre Voronoi dual, so it is well-defined for *any* primal mesh — the
//! carved hex mesh this crate produces, not only a Delaunay tetrahedralisation.
//!
//! Each primal cell is split into one **corner sub-cell** per vertex: the part
//! of the cell nearest that vertex, bounded by
//!
//! - the primal vertex `v`,
//! - the **midpoints** of the primal edges at `v`,
//! - the **centroids** of the primal faces at `v`,
//! - the primal **cell centroid**.
//!
//! The dual cell of `v` is the union of the corner sub-cells of every primal
//! cell that touches `v`. Two kinds of quad make up a sub-cell's boundary:
//!
//! - **inner quads** `[edge-midpoint, face-centroid, cell-centroid,
//!   face-centroid]` — one per primal edge inside the cell. The inner quad of
//!   edge `(v, w)` is listed by *both* dual cells `v` and `w`, so it becomes an
//!   **internal** dual face separating them.
//! - **outer quads** `[v, edge-midpoint, face-centroid, edge-midpoint]` on the
//!   primal *boundary* faces — these tile each boundary face over its vertices
//!   and become the **boundary** faces of the dual mesh.
//!
//! Because the sub-cells partition the domain exactly, the dual tiles the same
//! region: `Σ dual-cell volumes == primal domain volume`, every internal dual
//! face is shared by exactly two dual cells, and the dual boundary surface
//! coincides with the primal boundary surface.
//!
//! # Implementation
//!
//! Winding and owner/neighbour are recovered by [`from_cell_faces`], which
//! matches shared faces by vertex set and orients every face outward from its
//! owning cell's centroid — so this routine only has to emit each dual cell's
//! quads (in any winding) and hand them over. Geometry points (edge midpoints,
//! face/cell centroids) are allocated **per primal cell**; the inner quad of an
//! edge is always built inside a single cell, so the two endpoints reference the
//! same indices and the shared-face match is exact.
//!
//! # v1 scope & limitations
//!
//! The median dual splits the face between two dual cells into one quad per
//! surrounding primal cell (rather than merging them into a single polygon), so
//! the dual carries more, smaller faces than a minimal `polyDualMesh` — correct
//! and watertight, just not face-minimal. Orientation assumes each dual cell is
//! star-shaped about its primal vertex (true for structured/graded blocks);
//! [`VolumeMesh::validate`] is the gate that catches any cell that is not. Pure
//! Rust, Android-safe.

use crate::math::Vec3;
use crate::volume_mesh::{from_cell_faces, VolumeMesh};
use std::collections::HashMap;

/// Build the polyhedral **median dual** of `mesh`: one polyhedral cell per
/// primal vertex. See the module docs for the construction and its guarantees.
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface, carve::carve_box, dual::polyhedral_dual};
///
/// // Carve a unit box into 2×2×2 hexes (27 vertices), then take the dual:
/// // 27 polyhedral cells that tile the same volume.
/// let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
/// let hex = carve_box(&p, &t, 0.5);
/// let dual = polyhedral_dual(&hex);
///
/// assert_eq!(dual.cell_count(), hex.point_count()); // one dual cell per vertex
/// assert!((dual.total_volume() - hex.total_volume()).abs() < 1e-9);
/// assert!(dual.validate().is_ok());
/// ```
pub fn polyhedral_dual(mesh: &VolumeMesh) -> VolumeMesh {
    let n_cells = mesh.n_cells;
    let n_faces = mesh.face_count();

    // Primal cell -> its primal face indices (owner and neighbour both count).
    let mut cell_faces: Vec<Vec<usize>> = vec![Vec::new(); n_cells];
    for f in 0..n_faces {
        cell_faces[mesh.owner[f]].push(f);
        if let Some(nb) = mesh.neighbour[f] {
            cell_faces[nb].push(f);
        }
    }

    // Dual points start with every primal vertex (only boundary vertices are
    // actually referenced — by the outer quads — but keeping them all keeps the
    // indexing trivial; unused points are harmless).
    let mut points: Vec<Vec3> = mesh.points.clone();
    let n_primal_pts = points.len();

    // One dual cell (a list of face rings) per primal vertex.
    let mut dual_cells: Vec<Vec<Vec<usize>>> = vec![Vec::new(); n_primal_pts];

    let key = |a: usize, b: usize| if a < b { (a, b) } else { (b, a) };

    for faces in &cell_faces {
        // Cell centroid: average of the cell's distinct vertices.
        let mut verts: Vec<usize> = Vec::new();
        for &f in faces {
            verts.extend_from_slice(&mesh.faces[f]);
        }
        verts.sort_unstable();
        verts.dedup();
        let mut gc = Vec3::ZERO;
        for &v in &verts {
            gc = gc.add(mesh.points[v]);
        }
        let gc = gc.scale(1.0 / verts.len().max(1) as f64);
        let gc_idx = points.len();
        points.push(gc);

        // Face centroids for this cell's faces (one dual point each).
        let mut gf_idx: HashMap<usize, usize> = HashMap::new();
        for &f in faces {
            let idx = points.len();
            points.push(mesh.face_centroid(f));
            gf_idx.insert(f, idx);
        }

        // Per-cell edge midpoints, and for each edge the two incident face
        // centroids (an edge touches exactly two faces of a manifold cell).
        let mut mid_idx: HashMap<(usize, usize), usize> = HashMap::new();
        let mut edge_faces: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
        for &f in faces {
            let ring = &mesh.faces[f];
            let k = ring.len();
            for i in 0..k {
                let a = ring[i];
                let b = ring[(i + 1) % k];
                let e = key(a, b);
                mid_idx.entry(e).or_insert_with(|| {
                    let m = mesh.points[a].add(mesh.points[b]).scale(0.5);
                    let idx = points.len();
                    points.push(m);
                    idx
                });
                edge_faces.entry(e).or_default().push(gf_idx[&f]);
            }
        }

        // Inner quads: one per primal edge of the cell, shared by the two
        // endpoints' dual cells -> internal dual faces. (Winding is fixed by
        // from_cell_faces.)
        for (&(a, b), fcs) in &edge_faces {
            if fcs.len() != 2 {
                continue; // non-manifold edge within the cell — skip defensively
            }
            let m = mid_idx[&key(a, b)];
            let quad = vec![m, fcs[0], gc_idx, fcs[1]];
            dual_cells[a].push(quad.clone());
            dual_cells[b].push(quad);
        }

        // Outer quads on boundary primal faces: split each boundary face over
        // its vertices -> the boundary faces of the dual mesh.
        for &f in faces {
            if mesh.neighbour[f].is_some() {
                continue; // interior primal face -> internal to one dual cell, omit
            }
            let ring = &mesh.faces[f];
            let k = ring.len();
            let gf = gf_idx[&f];
            for i in 0..k {
                let v = ring[i];
                let prev = ring[(i + k - 1) % k];
                let next = ring[(i + 1) % k];
                let m1 = mid_idx[&key(prev, v)];
                let m2 = mid_idx[&key(v, next)];
                dual_cells[v].push(vec![v, m1, gf, m2]);
            }
        }
    }

    from_cell_faces(points, &dual_cells)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carve::carve_box;
    use crate::shapes::box_surface;
    use crate::volume_mesh::cells_faces;

    /// V&V — headline. Methodology: a unit box carved into 2×2×2 hexes (27
    /// vertices), then its median dual. Pass criteria (from the bead): one dual
    /// cell per primal vertex; the dual tiles the same region so `Σ vols ==`
    /// domain volume (exact); every cell is closed (`validate` Ok); every
    /// internal dual face is shared by exactly two cells; the dual is genuinely
    /// **polyhedral** (the interior vertex's cell has far more than 6 faces).
    /// Result: 27 cells; volume 1.0 (exact); validate Ok; interior cell has 24
    /// faces.
    #[test]
    fn dual_of_hex_block_is_closed_and_conserves_volume() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let hex = carve_box(&p, &t, 0.5); // 8 hexes, 27 vertices
        let dual = polyhedral_dual(&hex);

        assert_eq!(dual.cell_count(), hex.point_count(), "one dual cell per primal vertex");
        assert!(
            (dual.total_volume() - hex.total_volume()).abs() < 1e-9,
            "dual conserves the domain volume: {} vs {}",
            dual.total_volume(),
            hex.total_volume()
        );
        dual.validate().expect("every dual cell is closed");

        // Each internal dual face has a neighbour; each boundary face does not —
        // i.e. every face is shared by exactly two cells or is a single boundary.
        for f in 0..dual.face_count() {
            let internal = f < dual.n_internal_faces();
            assert_eq!(dual.neighbour[f].is_some(), internal, "face {f} internal/boundary split");
        }

        // Polyhedral: the cell around the interior vertex has > 6 faces (6 edges
        // × 4 surrounding hexes = 24 inner quads).
        let max_faces = cells_faces(&dual).iter().map(|c| c.len()).max().unwrap();
        assert!(max_faces > 6, "polyhedral cells present (max faces/cell = {max_faces})");
    }

    /// V&V — the dual boundary surface coincides with the primal boundary, so
    /// the dual boundary faces enclose the same volume and every boundary vertex
    /// contributes. Methodology: a 3×3×3 carve, dual, compare boundary-face area
    /// sum to the unit cube's surface area (6). Result: total boundary area 6.0
    /// (exact); volume preserved.
    #[test]
    fn dual_boundary_matches_primal_surface() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let hex = carve_box(&p, &t, 1.0 / 3.0); // 27 hexes
        let dual = polyhedral_dual(&hex);

        let mut area = 0.0;
        for f in 0..dual.face_count() {
            if dual.neighbour[f].is_none() {
                area += dual.face_area_vector(f).length();
            }
        }
        assert!((area - 6.0).abs() < 1e-9, "dual boundary area == cube surface area: {area}");
        assert!((dual.total_volume() - 1.0).abs() < 1e-9);
        dual.validate().expect("closed");
    }
}
