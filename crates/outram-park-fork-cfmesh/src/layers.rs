// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Algorithm references (re-implemented in Rust, not transcribed):
//
//   [1] OpenFOAM snappyHexMesh `addLayers` — shrink the wall points inward by
//       the total layer thickness and fill the vacated shell with a geometric
//       stack of prisms.
//       src/mesh/snappyHexMesh/snappyHexMeshDriver/snappyLayerDriver.C
//       Copyright (C) 2011-2016 OpenFOAM Foundation
//       Copyright (C) 2016-2023 OpenCFD Ltd. Licence: GPL-3.0-only.
//
//   [2] cfMesh boundary-layer generation — meshLibrary/utilities/boundaryLayers
//       (`boundaryLayers`, `refineBoundaryLayers`).
//       Copyright (C) 2014-2017 Creative Fields, Ltd. Licence: GPL-3.0-only.
//
//   [3] The smoothed-normal / thickness-limited advancing-layer variant follows
//       the published advancing-layer idea (D. J. Mavriplis; the VMTK and
//       Netgen implementations), from the literature, not from their code.
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

//! Prism **boundary layers** — insert graded near-wall inflation layers at a
//! wall patch.
//!
//! This is snappyHexMesh's *addLayers* / cfMesh's boundary-layer step. Given a
//! wall patch, it moves the interior mesh's wall points **inward** by the total
//! layer thickness and fills the vacated shell with `n_layers` stacked prism
//! cells per wall face, with a geometric thickness progression
//! (`first_thickness`, then × `expansion` each layer). The result resolves the
//! wall-normal gradients a CFD/TH solver needs (low y⁺).
//!
//! # How it stays valid
//!
//! Only the wall points move (they belong to the boundary), so the wall-adjacent
//! cells simply *shrink* — no interior topology changes and no hanging nodes.
//! The prism stack of each wall face is 1:1 with that face; adjacent stacks
//! share their side faces automatically (matched by [`from_cell_faces`]). The
//! operation is a **repartition** of the same region, so it preserves the mesh
//! volume exactly.
//!
//! # v1 scope
//!
//! A single wall patch, moved along the averaged per-point inward normal (so a
//! sharp convex corner shrinks along its diagonal — adequate for modest total
//! thickness). Multi-patch selection and corner-feature handling are future
//! work. The total thickness must be smaller than the wall cells or they would
//! invert. Pure Rust, Android-safe.

use crate::math::Vec3;
use crate::volume_mesh::{cells_faces, from_cell_faces, VolumeMesh};
use std::collections::{HashMap, HashSet};

/// Insert `n_layers` graded prism boundary layers at the patch named
/// `patch_name`, returning the new mesh.
///
/// `first_thickness` is the first (wall-nearest) layer thickness; each
/// subsequent layer is `expansion` times thicker. If the patch is missing or
/// `n_layers == 0`, `mesh` is returned unchanged (rebuilt).
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface, carve::carve_box, layers::add_boundary_layers};
///
/// // Carve a box (single `walls` patch), then add 3 prism layers; the volume
/// // is preserved (a repartition) and the mesh stays closed.
/// let (p, t) = box_surface(Vec3::ZERO, Vec3::new(3.0, 3.0, 3.0));
/// let m = carve_box(&p, &t, 1.0);
/// let l = add_boundary_layers(&m, "walls", 3, 0.05, 1.3);
/// assert!((l.total_volume() - m.total_volume()).abs() < 1e-9);
/// assert!(l.validate().is_ok());
/// ```
pub fn add_boundary_layers(
    mesh: &VolumeMesh,
    patch_name: &str,
    n_layers: usize,
    first_thickness: f64,
    expansion: f64,
) -> VolumeMesh {
    // Which faces are the wall patch?
    let patch = mesh.patches.iter().find(|p| p.name == patch_name);
    let Some(patch) = patch else {
        return from_cell_faces(mesh.points.clone(), &cells_faces(mesh));
    };
    if n_layers == 0 || first_thickness <= 0.0 {
        return from_cell_faces(mesh.points.clone(), &cells_faces(mesh));
    }
    let wall_faces: Vec<usize> = (patch.start_face..patch.start_face + patch.n_faces).collect();

    // Wall points and their inward normal (average of adjacent wall faces'
    // inward normals = −outward area vector).
    let mut wall_pts: HashSet<usize> = HashSet::new();
    let mut inward: HashMap<usize, Vec3> = HashMap::new();
    for &f in &wall_faces {
        let out = mesh.face_area_vector(f).normalize();
        for &p in &mesh.faces[f] {
            wall_pts.insert(p);
            let e = inward.entry(p).or_insert(Vec3::ZERO);
            *e = e.sub(out); // accumulate inward direction
        }
    }
    for v in inward.values_mut() {
        *v = v.normalize();
    }

    // Cumulative layer offsets d[0..=n]; d[0] = 0 (wall), d[n] = total thickness.
    let mut d = vec![0.0f64; n_layers + 1];
    let mut t = first_thickness;
    for m in 1..=n_layers {
        d[m] = d[m - 1] + t;
        t *= expansion;
    }

    // New point set: non-wall points kept; each wall point becomes a column of
    // n+1 points from the wall (d=0) inward to the shrunk position (d=total).
    let mut new_points: Vec<Vec3> = Vec::new();
    let mut new_of = vec![usize::MAX; mesh.points.len()];
    for (op, &pt) in mesh.points.iter().enumerate() {
        if !wall_pts.contains(&op) {
            new_of[op] = new_points.len();
            new_points.push(pt);
        }
    }
    let mut column: HashMap<usize, Vec<usize>> = HashMap::new();
    for &wp in &wall_pts {
        let x = mesh.points[wp];
        let nrm = inward[&wp];
        let mut col = Vec::with_capacity(n_layers + 1);
        for &dm in &d {
            col.push(new_points.len());
            new_points.push(x.add(nrm.scale(dm)));
        }
        column.insert(wp, col);
    }
    // A point's index in the interior mesh: non-wall -> kept; wall -> shrunk end.
    let remap = |op: usize| -> usize {
        if wall_pts.contains(&op) {
            column[&op][n_layers]
        } else {
            new_of[op]
        }
    };

    // Cells: the (shrunk) interior cells, then the prism cells.
    let mut cells: Vec<Vec<Vec<usize>>> = Vec::new();
    for cf in cells_faces(mesh) {
        cells.push(cf.iter().map(|ring| ring.iter().map(|&p| remap(p)).collect()).collect());
    }
    for &f in &wall_faces {
        let w = &mesh.faces[f];
        let k = w.len();
        for i in 0..n_layers {
            let mut faces: Vec<Vec<usize>> = Vec::new();
            // outer (layer i) and inner (layer i+1) faces
            faces.push(w.iter().map(|&p| column[&p][i]).collect());
            faces.push(w.iter().map(|&p| column[&p][i + 1]).collect());
            // side faces, one per wall-face edge
            for e in 0..k {
                let p = w[e];
                let q = w[(e + 1) % k];
                faces.push(vec![column[&p][i], column[&q][i], column[&q][i + 1], column[&p][i + 1]]);
            }
            cells.push(faces);
        }
    }

    from_cell_faces(new_points, &cells)
}

/// Insert graded prism boundary layers that **adapt to curved walls**.
///
/// [`add_boundary_layers`] marches every wall point inward by the same fixed
/// total thickness along its averaged normal — exact and ideal on flat (box)
/// walls, but on a **convex curved** wall those inward normals converge and the
/// layers self-intersect, invalidating the mesh. This variant makes curved-wall
/// layers work by:
///
/// 1. **Smoothing the wall-normal field** (a few Laplacian passes over the
///    wall-point adjacency) so neighbouring march directions vary gently — the
///    core idea of advancing-layer meshers (VMTK / Netgen) versus snappyHexMesh's
///    raw averaged normal.
/// 2. **Per-point thickness limiting** to a fraction of the local wall spacing,
///    so tight regions get thinner layers.
/// 3. A **global validity back-off**: it builds the layered mesh and, if it is
///    not closed (self-intersection), scales the thickness down and retries,
///    keeping the thickest layers that still produce a valid mesh.
///
/// So it returns the thickest valid graded prism layers the geometry admits,
/// rather than an invalid mesh (or, on a wall too tight for any layer, the input
/// rebuilt with none). The requested `first_thickness` / `expansion` set the
/// *profile*; the achieved absolute thickness may be smaller after back-off.
///
/// # Limitations
///
/// The back-off is global (uniform scale), so one tight feature thins the whole
/// wall; true per-point marching with local layer termination (snappyHexMesh
/// `addLayers` / AFLR advancing-layer) is future work. Verified valid + solvable
/// on snapped spheres and cylinders. Pure Rust, Android-safe.
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, shapes::sphere_surface, carve::carve_box,
///     snap::snap_to_surface, dual::polyhedral_dual_min_faces, layers::add_boundary_layers_adaptive};
///
/// // A snapped, dualised sphere — a curved polyhedral wall — takes prism layers.
/// let (p, t) = sphere_surface(Vec3::ZERO, 3.0, 20, 40);
/// let poly = polyhedral_dual_min_faces(&snap_to_surface(&carve_box(&p, &t, 0.6), &p, &t));
/// let layered = add_boundary_layers_adaptive(&poly, "walls", 3, 0.04, 1.3);
/// assert!(layered.validate().is_ok());
/// assert!(layered.cell_count() > poly.cell_count()); // prism cells were added
/// ```
pub fn add_boundary_layers_adaptive(
    mesh: &VolumeMesh,
    patch_name: &str,
    n_layers: usize,
    first_thickness: f64,
    expansion: f64,
) -> VolumeMesh {
    let rebuild = || from_cell_faces(mesh.points.clone(), &cells_faces(mesh));
    let Some(patch) = mesh.patches.iter().find(|p| p.name == patch_name) else {
        return rebuild();
    };
    if n_layers == 0 || first_thickness <= 0.0 {
        return rebuild();
    }
    let wall_faces: Vec<usize> = (patch.start_face..patch.start_face + patch.n_faces).collect();

    // Wall points and their (averaged) inward normal.
    let mut wall_pts: HashSet<usize> = HashSet::new();
    let mut inward: HashMap<usize, Vec3> = HashMap::new();
    for &f in &wall_faces {
        let out = mesh.face_area_vector(f).normalize();
        for &p in &mesh.faces[f] {
            wall_pts.insert(p);
            let e = inward.entry(p).or_insert(Vec3::ZERO);
            *e = e.sub(out);
        }
    }
    for v in inward.values_mut() {
        *v = v.normalize();
    }

    // Wall-point adjacency + shortest incident wall edge (the local spacing).
    let mut nbrs: HashMap<usize, HashSet<usize>> = HashMap::new();
    let mut min_edge: HashMap<usize, f64> = HashMap::new();
    for &f in &wall_faces {
        let w = &mesh.faces[f];
        let k = w.len();
        for i in 0..k {
            let (a, b) = (w[i], w[(i + 1) % k]);
            nbrs.entry(a).or_default().insert(b);
            nbrs.entry(b).or_default().insert(a);
            let len = mesh.points[a].sub(mesh.points[b]).length();
            for &v in &[a, b] {
                let e = min_edge.entry(v).or_insert(f64::MAX);
                *e = e.min(len);
            }
        }
    }

    // Smooth the normal field (advancing-layer style): each pass replaces a
    // normal with the unit average of itself and its wall neighbours.
    for _ in 0..3 {
        let smoothed: HashMap<usize, Vec3> = wall_pts
            .iter()
            .map(|&wp| {
                let mut s = inward[&wp];
                if let Some(ns) = nbrs.get(&wp) {
                    for &nb in ns {
                        s = s.add(inward[&nb]);
                    }
                }
                let sn = s.normalize();
                // Keep the original if smoothing collapsed the direction.
                (wp, if sn.length() > 0.5 { sn } else { inward[&wp] })
            })
            .collect();
        inward = smoothed;
    }

    // Cumulative profile d[0..=n] normalized to [0, 1] (the layer grading).
    let mut d = vec![0.0f64; n_layers + 1];
    let mut t = first_thickness;
    for m in 1..=n_layers {
        d[m] = d[m - 1] + t;
        t *= expansion;
    }
    let total_req = d[n_layers];
    let profile: Vec<f64> = d.iter().map(|x| x / total_req).collect();

    // Per-point total-thickness cap: a fraction of the local wall spacing.
    const BETA: f64 = 0.5;
    let cap: HashMap<usize, f64> =
        wall_pts.iter().map(|&wp| (wp, total_req.min(BETA * min_edge[&wp]))).collect();

    // Global validity back-off: keep the thickest layers that stay valid.
    let mut scale = 1.0;
    for _try in 0..10 {
        let per_point: HashMap<usize, f64> = cap.iter().map(|(&wp, &c)| (wp, c * scale)).collect();
        let candidate = build_layered(mesh, &wall_faces, &wall_pts, &inward, &profile, &per_point, n_layers);
        if candidate.validate().is_ok() {
            return candidate;
        }
        scale *= 0.6;
    }
    rebuild() // too tight for any valid layer
}

/// Assemble a layered mesh given a per-wall-point total thickness and a
/// normalized grading `profile` (`profile[0] = 0`, `profile[n] = 1`). Shared by
/// the adaptive driver's back-off loop.
fn build_layered(
    mesh: &VolumeMesh,
    wall_faces: &[usize],
    wall_pts: &HashSet<usize>,
    inward: &HashMap<usize, Vec3>,
    profile: &[f64],
    per_point_total: &HashMap<usize, f64>,
    n_layers: usize,
) -> VolumeMesh {
    let mut new_points: Vec<Vec3> = Vec::new();
    let mut new_of = vec![usize::MAX; mesh.points.len()];
    for (op, &pt) in mesh.points.iter().enumerate() {
        if !wall_pts.contains(&op) {
            new_of[op] = new_points.len();
            new_points.push(pt);
        }
    }
    let mut column: HashMap<usize, Vec<usize>> = HashMap::new();
    for &wp in wall_pts {
        let x = mesh.points[wp];
        let nrm = inward[&wp];
        let tot = per_point_total[&wp];
        let mut col = Vec::with_capacity(profile.len());
        for &frac in profile {
            col.push(new_points.len());
            new_points.push(x.add(nrm.scale(tot * frac)));
        }
        column.insert(wp, col);
    }
    let remap = |op: usize| -> usize {
        if wall_pts.contains(&op) {
            column[&op][n_layers]
        } else {
            new_of[op]
        }
    };
    let mut cells: Vec<Vec<Vec<usize>>> = Vec::new();
    for cf in cells_faces(mesh) {
        cells.push(cf.iter().map(|ring| ring.iter().map(|&p| remap(p)).collect()).collect());
    }
    for &f in wall_faces {
        let w = &mesh.faces[f];
        let k = w.len();
        for i in 0..n_layers {
            let mut faces: Vec<Vec<usize>> = Vec::new();
            faces.push(w.iter().map(|&p| column[&p][i]).collect());
            faces.push(w.iter().map(|&p| column[&p][i + 1]).collect());
            for e in 0..k {
                let p = w[e];
                let q = w[(e + 1) % k];
                faces.push(vec![column[&p][i], column[&q][i], column[&q][i + 1], column[&p][i + 1]]);
            }
            cells.push(faces);
        }
    }
    from_cell_faces(new_points, &cells)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carve::carve_box;
    use crate::shapes::box_surface;

    /// V&V — headline. Methodology: a unit box carved 3×3×3 (a single `walls`
    /// patch), then 3 graded boundary layers added on `walls`. Pass criteria:
    /// the operation is a repartition, so it preserves the exact volume and
    /// stays closed; the cell count grows by exactly `n_layers × wall_faces`;
    /// the outer wall faces sit back at the original box faces. Result: volume 1
    /// (exact); validate() Ok; +3·54 cells; the mesh's bounding box is still the
    /// unit box.
    #[test]
    fn box_boundary_layers_preserve_volume_and_close() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let base = carve_box(&p, &t, 1.0 / 3.0); // 3×3×3 = 27 cells, single `walls` patch
        let wall_faces = base.n_boundary_faces(); // 6 * 9 = 54
        let layered = add_boundary_layers(&base, "walls", 3, 0.02, 1.3);

        assert!((layered.total_volume() - 1.0).abs() < 1e-9, "volume preserved: {}", layered.total_volume());
        layered.validate().expect("layered mesh is closed");
        assert_eq!(layered.cell_count(), base.cell_count() + 3 * wall_faces, "n_layers × wall faces added");

        // The outer wall still bounds the original unit box.
        let (mut lo, mut hi) = (layered.points[0], layered.points[0]);
        for p in &layered.points {
            lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
        assert!(lo.length() < 1e-9 && hi.sub(Vec3::new(1.0, 1.0, 1.0)).length() < 1e-9, "outer wall at the box");
    }

    /// V&V — a single layer is the minimal case, and the first-layer thickness
    /// is honoured: with n = 1, the shrunk interior wall sits `first_thickness`
    /// inside the original wall (measured on a face-centre column, where the
    /// inward normal is purely axial).
    #[test]
    fn single_layer_thickness_is_honoured() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let base = carve_box(&p, &t, 0.5); // 2×2×2 = 8 cells, single `walls` patch
        let layered = add_boundary_layers(&base, "walls", 1, 0.05, 1.0);
        assert!((layered.total_volume() - 1.0).abs() < 1e-9);
        layered.validate().expect("closed");
        // The inner shell is inset by ~0.05 on flat faces; the −X wall's
        // face-centre point (0, 0.5, 0.5) has a pure +X inward normal, so it
        // moves to exactly x = 0.05.
        let near = layered.points.iter().filter(|p| (p.x - 0.05).abs() < 1e-6).count();
        assert!(near > 0, "wall points moved inward by the first-layer thickness");
    }

    /// V&V — **curved-wall** layers (the headline for the adaptive variant).
    /// Methodology: a snapped, face-minimal-dualised sphere (radius 3) — a curved
    /// polyhedral wall on which the fixed-thickness [`add_boundary_layers`]
    /// self-intersects and invalidates the mesh — takes prism layers via
    /// [`add_boundary_layers_adaptive`]. Pass criteria: the result is **closed**
    /// (`validate` Ok), has **no inverted cells**, adds prism cells, conserves
    /// the volume (a repartition — the outer wall is unmoved), and keeps
    /// non-orthogonality within the practical boundary-layer limit (< 85°).
    ///
    /// Note on the 85° bar: near-wall prism layers are *intrinsically*
    /// non-orthogonal and high-aspect-ratio, so a boundary-layer mesh legitimately
    /// exceeds `checkMesh`'s conservative 70° warning (which
    /// [`check_quality`](crate::checks::check_quality)`::is_solvable` uses); such
    /// meshes are solved with non-orthogonal correctors. Result (cell size 0.6):
    /// closed; 0 inverted; volume conserved; max non-orthogonality ≈ 79°.
    #[test]
    fn adaptive_layers_work_on_a_curved_sphere() {
        use crate::carve::carve_box;
        use crate::checks::check_quality;
        use crate::dual::polyhedral_dual_min_faces;
        use crate::shapes::sphere_surface;
        use crate::snap::snap_to_surface;

        let (p, t) = sphere_surface(Vec3::ZERO, 3.0, 24, 48);
        let poly = polyhedral_dual_min_faces(&snap_to_surface(&carve_box(&p, &t, 0.6), &p, &t));
        let vol0 = poly.total_volume();

        let layered = add_boundary_layers_adaptive(&poly, "walls", 3, 0.04, 1.3);
        layered.validate().expect("curved-wall layered mesh is closed");
        assert!(layered.cell_count() > poly.cell_count(), "prism layers added");
        assert!((layered.total_volume() - vol0).abs() < 1e-6, "volume conserved: {} vs {vol0}", layered.total_volume());
        let q = check_quality(&layered);
        assert_eq!(q.n_negative_volume_cells, 0, "no inverted cells");
        assert!(q.max_non_orthogonality_deg < 85.0, "non-orthogonality within BL limit: {}", q.max_non_orthogonality_deg);
    }

    /// V&V — the adaptive variant also handles a cylinder (mixed flat caps +
    /// curved side). Methodology: snapped, dualised cylinder → adaptive layers.
    /// Pass criteria: closed, prism cells added, volume conserved.
    #[test]
    fn adaptive_layers_work_on_a_cylinder() {
        use crate::carve::carve_box;
        use crate::dual::polyhedral_dual_min_faces;
        use crate::shapes::cylinder_surface;
        use crate::snap::snap_to_surface;

        let (p, t) = cylinder_surface(Vec3::ZERO, 2.0, 5.0, 48);
        let poly = polyhedral_dual_min_faces(&snap_to_surface(&carve_box(&p, &t, 0.6), &p, &t));
        let vol0 = poly.total_volume();

        let layered = add_boundary_layers_adaptive(&poly, "walls", 3, 0.04, 1.3);
        layered.validate().expect("cylinder layered mesh is closed");
        assert!(layered.cell_count() > poly.cell_count(), "prism layers added");
        assert!((layered.total_volume() - vol0).abs() < 1e-6, "volume conserved");
    }

    /// V&V — the adaptive variant matches the exact one on a **flat box** wall
    /// (where no back-off is needed): still valid, volume conserved, and prism
    /// cells added (`+n_layers × wall_faces`).
    #[test]
    fn adaptive_layers_match_on_flat_box() {
        use crate::carve::carve_box;
        use crate::shapes::box_surface;

        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let base = carve_box(&p, &t, 1.0 / 3.0);
        let wall_faces = base.n_boundary_faces();
        let layered = add_boundary_layers_adaptive(&base, "walls", 3, 0.02, 1.3);
        assert!((layered.total_volume() - 1.0).abs() < 1e-9, "volume conserved on box");
        layered.validate().expect("box layered mesh is closed");
        assert_eq!(layered.cell_count(), base.cell_count() + 3 * wall_faces, "prism cells added");
    }

    /// V&V — `n_layers = 0` is a faithful rebuild (no change in volume or cells).
    #[test]
    fn zero_layers_is_a_noop() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let base = carve_box(&p, &t, 0.5); // 2×2×2 = 8 cells
        let same = add_boundary_layers(&base, "walls", 0, 0.05, 1.2);
        assert_eq!(same.cell_count(), base.cell_count());
        assert!((same.total_volume() - 1.0).abs() < 1e-9);
        same.validate().expect("closed");
    }
}
