// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Algorithm reference (re-implemented in Rust, not transcribed):
//   cfMesh — https://github.com/wyldckat/cfMesh
//   meshLibrary/tetMesh, and the cell -> tet decomposition in
//   meshLibrary/utilities/meshes/polyMeshGenModifier.
//   Copyright (C) 2014-2017 Creative Fields, Ltd. Licence: GPL-3.0-only.
//   The face-centroid + cell-centroid ("centroid subdivision") tiling is the
//   same one OpenFOAM `polyMesh::tetBasePtIs` / `tetDecomposition` uses:
//   Copyright (C) 2011-2016 OpenFOAM Foundation, GPL-3.0-only.
//
//   NOT derived from TetGen (AGPL) — deliberately, see the module docs.
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

//! **Tetrahedralization** — split every cell of a volume mesh into tetrahedra
//! (the all-tet foundation for the polyhedral-dual and boundary-layer paths).
//!
//! # Approach: centroid subdivision (not from-scratch Delaunay)
//!
//! Per the roadmap decision (`op-hzs.32`), this does **not** build a from-scratch
//! constrained-Delaunay mesher (TetGen is AGPL and off-limits). Instead it
//! follows the cfMesh-style route: take the crate's Cartesian/carved cell mesh
//! and decompose each cell into tets by **centroid subdivision** — the standard,
//! robust "polyhedron → tets" tiling:
//!
//! - add the **cell centroid** `gC` and, for every face, its **face centroid**
//!   `gF`;
//! - for each face `f` of the cell and each edge `(a, b)` of `f`, emit the
//!   tetrahedron `{a, b, gF, gC}`.
//!
//! A hex (6 faces × 4 edges) becomes 24 tets; a general polyhedral cell becomes
//! `Σ_faces (edges of face)` tets. The union tiles the cell exactly, so the tet
//! mesh conserves volume and its boundary is a triangulation of the input
//! surface (each boundary polygon fanned through its centroid).
//!
//! # Why it stays a valid FV mesh
//!
//! **Face centroids are shared globally** (one per primal face, keyed by face
//! index), so the two cells across an internal primal face emit the *same*
//! boundary triangle `{a, b, gF}` and [`from_cell_faces`] matches them into an
//! internal face; a primal boundary face's triangles stay boundary. Cell
//! centroids are per-cell, so the radial faces (`{a, b, gC}`, `{a, gF, gC}`,
//! `{b, gF, gC}`) match only among a cell's own tets. Winding and
//! owner/neighbour are recovered by [`from_cell_faces`].
//!
//! # Scope & limitations
//!
//! This is a *space-filling* tetrahedralization, **not** a quality
//! (Delaunay-refined) one: it guarantees positive-volume tets for convex /
//! star-shaped cells (the Cartesian/carved cells this crate produces) and exact
//! volume conservation, but it does not optimise tet shape or minimise count.
//! Delaunay-quality refinement is future work (gmsh — GPLv2+, GPLv3-compatible —
//! is a licence-clean reference for that). Pure Rust, Android-safe.
//!
//! # Honest V&V scope — the tet quality is UNMEASURED
//!
//! The two tests in this module assert exactly four things: the tet count
//! (`Σ_cells Σ_faces edges`), **positive volume** (no inverted tets), the
//! **boundary area** matching the input surface, and **exact volume
//! conservation** — plus that every cell really is a 4-triangle tet. They call
//! [`crate::checks::check_quality`] only for its `n_negative_volume_cells` /
//! `min_cell_volume` fields.
//!
//! **No mesh-quality metric is gated here.** Neither non-orthogonality,
//! skewness, nor aspect ratio of the produced tets is asserted anywhere, so the
//! *shape* quality of this tet primal is unverified — "valid" here means
//! "positively oriented and watertight", not "well-shaped".
//!
//! This matters because the tet primal is the **suspected dominant source of
//! the non-orthogonality** reported downstream. Centroid subdivision emits, per
//! face edge, a tet spanning a face edge, the face centroid and the cell
//! centroid — a systematically sliver-prone shape.
//! [`crate::delaunay`] already records the defect qualitatively: the centroid
//! subdivision "produces a valid, space-filling tet mesh, but not a *Delaunay*
//! one — some interior faces fail the empty-circumsphere (locally-Delaunay)
//! test, which is what leaves slivers". The whole-pipeline sphere table in
//! [`crate::pipeline`]'s tests measures max non-orthogonality in the 71-87 deg
//! band, but nothing isolates how much of that the tet stage contributes.
//!
//! **This attribution is a hypothesis, not a measurement.** Confirming it needs
//! a per-stage quality measurement (run [`crate::checks::check_quality`] on the
//! mesh immediately after this stage and compare against the carve that fed it)
//! — which does not exist yet. Do not cite the slivers as a proven cause.

use crate::math::Vec3;
use crate::volume_mesh::{from_cell_faces, VolumeMesh};

/// Tetrahedralize `mesh` by centroid subdivision: every cell is split into
/// `Σ_faces (edges of face)` tetrahedra. Returns an all-tet [`VolumeMesh`] that
/// conserves the domain volume and whose boundary triangulates the input
/// surface. See the module docs for the construction and its guarantees.
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface, carve::carve_box, tet::tetrahedralize};
///
/// // Carve a unit box into 2×2×2 hexes, then tetrahedralize: 8 hexes × 24 =
/// // 192 positive-volume tets that tile the same volume.
/// let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
/// let hex = carve_box(&p, &t, 0.5);
/// let tets = tetrahedralize(&hex);
///
/// assert_eq!(tets.cell_count(), 8 * 24);
/// assert!((tets.total_volume() - hex.total_volume()).abs() < 1e-9);
/// assert!(tets.validate().is_ok());
/// ```
pub fn tetrahedralize(mesh: &VolumeMesh) -> VolumeMesh {
    let n_faces = mesh.face_count();
    let n_cells = mesh.n_cells;

    // Primal cell -> its primal face indices (owner and neighbour both count).
    let mut cell_faces: Vec<Vec<usize>> = vec![Vec::new(); n_cells];
    for f in 0..n_faces {
        cell_faces[mesh.owner[f]].push(f);
        if let Some(nb) = mesh.neighbour[f] {
            cell_faces[nb].push(f);
        }
    }

    // Points: primal vertices, then one GLOBAL centroid per primal face (shared
    // by the two adjacent cells), then one centroid per cell.
    let mut points: Vec<Vec3> = mesh.points.clone();
    let face_cent_base = points.len();
    for f in 0..n_faces {
        points.push(mesh.face_centroid(f));
    }
    let cell_cent_base = points.len();
    for faces in &cell_faces {
        // Cell centroid: average of the cell's distinct vertices.
        let mut verts: Vec<usize> = Vec::new();
        for &f in faces {
            verts.extend_from_slice(&mesh.faces[f]);
        }
        verts.sort_unstable();
        verts.dedup();
        let mut g = Vec3::ZERO;
        for &v in &verts {
            g = g.add(mesh.points[v]);
        }
        points.push(g.scale(1.0 / verts.len().max(1) as f64));
    }

    // One cell per tetrahedron: 4 triangular face rings each. from_cell_faces
    // fixes winding/owner/neighbour, so any winding is fine here.
    let mut tets: Vec<Vec<Vec<usize>>> = Vec::new();
    for (c, faces) in cell_faces.iter().enumerate() {
        let gc = cell_cent_base + c;
        for &f in faces {
            let gf = face_cent_base + f;
            let ring = &mesh.faces[f];
            let k = ring.len();
            for i in 0..k {
                let a = ring[i];
                let b = ring[(i + 1) % k];
                tets.push(vec![
                    vec![a, b, gf], // on primal face f (boundary triangle if f is a boundary face)
                    vec![a, b, gc],
                    vec![a, gf, gc],
                    vec![b, gf, gc],
                ]);
            }
        }
    }

    from_cell_faces(points, &tets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carve::carve_box;
    use crate::checks::check_quality;
    use crate::shapes::box_surface;

    /// V&V — headline (the bead's pass criteria). Methodology: carve a unit box
    /// into 2×2×2 hexes, tetrahedralize, and check the four properties the bead
    /// names: (1) the tet count is `Σ cells·faces·edges` (8·24 = 192); (2) all
    /// tets have positive volume (`check_quality` reports no negatives); (3) the
    /// boundary triangulates the input surface (boundary area == cube area = 6);
    /// (4) `Σ tet volumes == enclosed volume` (= 1). Result: 192 cells; 0
    /// negative-volume; boundary area 6.0; volume 1.0; `validate` Ok.
    #[test]
    fn tet_of_hex_block_is_positive_closed_and_conserves_volume() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let hex = carve_box(&p, &t, 0.5);
        let tets = tetrahedralize(&hex);

        assert_eq!(tets.cell_count(), 8 * 24, "Σ cells·faces·edges tetrahedra");

        let q = check_quality(&tets);
        assert_eq!(q.n_negative_volume_cells, 0, "all tets have positive volume");
        assert!(q.min_cell_volume > 0.0, "min tet volume positive: {}", q.min_cell_volume);

        let mut area = 0.0;
        for f in 0..tets.face_count() {
            if tets.neighbour[f].is_none() {
                area += tets.face_area_vector(f).length();
            }
        }
        assert!((area - 6.0).abs() < 1e-9, "boundary triangulates the cube surface: {area}");

        assert!((tets.total_volume() - 1.0).abs() < 1e-9, "Σ tet volumes == enclosed volume");
        tets.validate().expect("every tet is closed");
    }

    /// V&V — every cell really is a tetrahedron: exactly 4 triangular faces
    /// each. Methodology: tetrahedralize a 3×3×3 carve and check, via the
    /// per-cell face inversion, that every cell has 4 faces and every face is a
    /// triangle. Result: all cells 4-faced, all faces 3-vertex.
    #[test]
    fn every_cell_is_a_four_triangle_tetrahedron() {
        use crate::volume_mesh::cells_faces;
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let hex = carve_box(&p, &t, 1.0 / 3.0);
        let tets = tetrahedralize(&hex);
        for cell in cells_faces(&tets) {
            assert_eq!(cell.len(), 4, "a tet has 4 faces");
            for face in &cell {
                assert_eq!(face.len(), 3, "a tet face is a triangle");
            }
        }
        assert!((tets.total_volume() - 1.0).abs() < 1e-9);
        tets.validate().expect("closed");
    }
}
