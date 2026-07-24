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
