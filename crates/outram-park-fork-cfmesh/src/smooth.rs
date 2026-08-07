// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Algorithm references (re-implemented in Rust, not transcribed):
//
//   [1] "Smart" Laplacian smoothing — accept a vertex move only if it does not
//       degrade (here: does not invert) any incident cell.
//       Lori A. Freitag, "On combining Laplacian and optimization-based mesh
//       smoothing techniques", AMD-Vol. 220, Trends in Unstructured Mesh
//       Generation, ASME, 1997, pp. 37-43. Published method, no code reused.
//
//   [2] cfMesh — https://github.com/wyldckat/cfMesh
//       meshLibrary/utilities/smoothers/geometry (`meshOptimizer`), the
//       equivalent stage in the cfMesh workflow.
//       Copyright (C) 2014-2017 Creative Fields, Ltd. Licence: GPL-3.0-only.
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

//! Mesh-quality **smoothing** — smart Laplacian relaxation of interior vertices.
//!
//! This is the first, robust increment of the tet-quality-refinement roadmap
//! (`op-38z`, follow-up to the tet foundation). A raw centroid-subdivision tet
//! mesh (or any generated mesh) can carry poorly-shaped cells near a snapped or
//! graded boundary; **Laplacian smoothing** improves cell shape by moving each
//! *free* (non-boundary) vertex toward the centroid of its edge-connected
//! neighbours.
//!
//! # "Smart" — never inverts a cell
//!
//! Plain Laplacian smoothing can turn a cell inside-out in concave regions. This
//! is the **smart** variant (Freitag): a vertex only moves if *every* cell
//! incident to it keeps a positive volume after the move — otherwise it stays
//! put. So the operation can only maintain or improve validity: no cell is ever
//! inverted, and [`VolumeMesh::validate`] stays `Ok`.
//!
//! # Guarantees
//!
//! - **Boundary preserved.** Only interior vertices move; every boundary vertex
//!   (any vertex used by a boundary face) is pinned. The boundary faces are
//!   therefore unchanged, so the **total volume is conserved exactly** (the
//!   divergence-theorem volume depends only on the boundary).
//! - **Topology preserved.** Points move; faces / owner / neighbour / patches
//!   are untouched. This is smoothing, not remeshing.
//! - **No inversion.** By the smart-acceptance rule above.
//!
//! Sequential (Gauss–Seidel) sweeps: each vertex is relaxed against the
//! already-updated positions, which converges faster than a Jacobi sweep and
//! keeps the no-inversion invariant exact. Pure Rust, Android-safe.
//!
//! # Scope
//!
//! Smoothing improves *shape*; it does not change connectivity. Flip-based
//! Delaunay optimisation and size-driven point insertion (the rest of `op-38z`)
//! are separate, later increments.

use crate::math::Vec3;
use crate::volume_mesh::{cells_faces, VolumeMesh};
use std::collections::HashSet;

/// Smart-Laplacian-smooth `mesh` for `passes` sweeps, returning the relaxed
/// mesh. Interior vertices move toward their neighbour centroid only when the
/// move inverts no incident cell; boundary vertices are pinned. Topology and
/// total volume are preserved. See the module docs for the guarantees.
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface, carve::carve_box, tet::tetrahedralize, smooth::laplacian_smooth};
///
/// let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
/// let tets = tetrahedralize(&carve_box(&p, &t, 0.5));
/// let smoothed = laplacian_smooth(&tets, 5);
///
/// // Boundary is pinned, so the volume is conserved exactly, and no cell inverts.
/// assert!((smoothed.total_volume() - tets.total_volume()).abs() < 1e-9);
/// assert!(smoothed.validate().is_ok());
/// ```
pub fn laplacian_smooth(mesh: &VolumeMesh, passes: usize) -> VolumeMesh {
    let np = mesh.points.len();

    // Boundary vertices (used by any boundary face) are pinned.
    let mut pinned = vec![false; np];
    for f in 0..mesh.face_count() {
        if mesh.neighbour[f].is_none() {
            for &v in &mesh.faces[f] {
                pinned[v] = true;
            }
        }
    }

    // Edge-connected neighbours of each vertex (union over all face rings).
    let mut nbrs: Vec<HashSet<usize>> = vec![HashSet::new(); np];
    for ring in &mesh.faces {
        let k = ring.len();
        for i in 0..k {
            let a = ring[i];
            let b = ring[(i + 1) % k];
            nbrs[a].insert(b);
            nbrs[b].insert(a);
        }
    }

    // Per-cell outward face rings, and the cells incident to each vertex.
    let cells = cells_faces(mesh);
    let mut vert_cells: Vec<Vec<usize>> = vec![Vec::new(); np];
    for (c, faces) in cells.iter().enumerate() {
        let mut seen: HashSet<usize> = HashSet::new();
        for ring in faces {
            for &v in ring {
                seen.insert(v);
            }
        }
        for v in seen {
            vert_cells[v].push(c);
        }
    }

    let mut pts = mesh.points.clone();

    // Signed volume of a cell (outward-wound rings -> positive when valid).
    let cell_volume = |c: usize, pts: &[Vec3]| -> f64 {
        let mut v6 = 0.0;
        for ring in &cells[c] {
            // face area vector and centroid from current positions
            let mut area = Vec3::ZERO;
            let mut cen = Vec3::ZERO;
            let k = ring.len();
            for i in 0..k {
                let p = pts[ring[i]];
                let q = pts[ring[(i + 1) % k]];
                area = area.add(p.cross(q));
                cen = cen.add(p);
            }
            area = area.scale(0.5);
            cen = cen.scale(1.0 / k as f64);
            v6 += cen.dot(area);
        }
        v6 / 3.0
    };

    for _ in 0..passes {
        for v in 0..np {
            if pinned[v] || nbrs[v].is_empty() {
                continue;
            }
            // Candidate: centroid of the edge-connected neighbours.
            let mut c = Vec3::ZERO;
            for &n in &nbrs[v] {
                c = c.add(pts[n]);
            }
            let cand = c.scale(1.0 / nbrs[v].len() as f64);

            // Accept only if every incident cell stays positive-volume.
            let old = pts[v];
            pts[v] = cand;
            let ok = vert_cells[v].iter().all(|&cell| cell_volume(cell, &pts) > 1e-14);
            if !ok {
                pts[v] = old;
            }
        }
    }

    VolumeMesh {
        points: pts,
        faces: mesh.faces.clone(),
        owner: mesh.owner.clone(),
        neighbour: mesh.neighbour.clone(),
        n_cells: mesh.n_cells,
        patches: mesh.patches.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carve::carve_box;
    use crate::checks::check_quality;
    use crate::shapes::box_surface;
    use crate::tet::tetrahedralize;

    /// Find an interior vertex (one used by no boundary face).
    fn first_interior_vertex(mesh: &VolumeMesh) -> usize {
        let mut pinned = vec![false; mesh.points.len()];
        for f in 0..mesh.face_count() {
            if mesh.neighbour[f].is_none() {
                for &v in &mesh.faces[f] {
                    pinned[v] = true;
                }
            }
        }
        pinned.iter().position(|&p| !p).expect("mesh has an interior vertex")
    }

    /// V&V — headline. Methodology: tetrahedralize a unit box, **perturb one
    /// interior vertex** to degrade cell shape (raising the worst aspect ratio),
    /// then smart-Laplacian-smooth. Pass criteria: smoothing lowers the worst
    /// aspect ratio back down (shape improved), inverts no cell, and — because
    /// the boundary is pinned — conserves the volume exactly. Result: aspect
    /// ratio rises on perturb then falls on smooth; 0 negative-volume cells;
    /// volume 1.0 throughout; validate Ok.
    #[test]
    fn smoothing_recovers_perturbed_tet_quality() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let tets = tetrahedralize(&carve_box(&p, &t, 0.5));
        let base_ar = check_quality(&tets).max_aspect_ratio;

        // Perturb one interior vertex — a fixed offset that distorts without
        // inverting (verified by the assertions below).
        let v = first_interior_vertex(&tets);
        let mut bad = tets.clone();
        bad.points[v] = bad.points[v].add(Vec3::new(0.09, 0.06, -0.05));
        let bad_q = check_quality(&bad);
        assert_eq!(bad_q.n_negative_volume_cells, 0, "perturbation did not invert a cell");
        assert!(bad_q.max_aspect_ratio > base_ar, "perturbation worsened shape: {} -> {}", base_ar, bad_q.max_aspect_ratio);
        assert!((bad.total_volume() - 1.0).abs() < 1e-9, "interior move conserves volume");

        // Smooth it back.
        let good = laplacian_smooth(&bad, 20);
        let good_q = check_quality(&good);
        assert!(good_q.max_aspect_ratio < bad_q.max_aspect_ratio, "smoothing improved worst aspect ratio: {} -> {}", bad_q.max_aspect_ratio, good_q.max_aspect_ratio);
        assert_eq!(good_q.n_negative_volume_cells, 0, "smoothing inverted no cell");
        assert!((good.total_volume() - 1.0).abs() < 1e-9, "smoothing conserves volume exactly");
        good.validate().expect("smoothed mesh is closed");
    }

    /// V&V — smoothing an already-regular mesh is a safe no-op-ish pass: it never
    /// inverts a cell, never moves the boundary, and conserves volume. Result:
    /// bounding box unchanged (boundary pinned), 0 negatives, volume preserved.
    #[test]
    fn smoothing_pins_boundary_and_conserves_volume() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let tets = tetrahedralize(&carve_box(&p, &t, 0.5));
        let smoothed = laplacian_smooth(&tets, 10);

        // Boundary pinned -> bounding box still the unit cube.
        let (mut lo, mut hi) = (smoothed.points[0], smoothed.points[0]);
        for p in &smoothed.points {
            lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
        assert!(lo.length() < 1e-9 && hi.sub(Vec3::new(1.0, 1.0, 1.0)).length() < 1e-9, "boundary unchanged");
        assert_eq!(check_quality(&smoothed).n_negative_volume_cells, 0);
        assert!((smoothed.total_volume() - 1.0).abs() < 1e-9);
        smoothed.validate().expect("closed");
    }
}
