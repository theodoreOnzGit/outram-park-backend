//! Mesh **quality checks** — the Rust analogue of cfMesh's `polyMeshGenChecks`.
//!
//! A generated volume mesh must be *checked* before it is handed to a solver:
//! finite-volume discretisation error grows with face **non-orthogonality** and
//! **skewness**, ill-conditioning grows with cell **aspect ratio**, and a
//! **negative-volume** cell makes a case unsolvable. [`check_quality`] computes
//! these over a [`VolumeMesh`] and returns a [`QualityReport`].
//!
//! All geometry is the standard OpenFOAM pyramid decomposition — per-cell
//! volumes and centroids from the faces — so the metrics match what the solver
//! itself would compute. Pure Rust, no dependencies.

use crate::math::Vec3;
use crate::volume_mesh::VolumeMesh;

/// Summary of mesh-quality metrics (worst-case values over the mesh).
#[derive(Debug, Clone, PartialEq)]
pub struct QualityReport {
    /// Maximum face **non-orthogonality**, in degrees: the angle between an
    /// internal face's normal and the vector joining its two cell centres. `0°`
    /// is perfectly orthogonal; OpenFOAM flags `> 70°`.
    pub max_non_orthogonality_deg: f64,
    /// Maximum face **skewness**: the offset of the face centre from where the
    /// owner–neighbour centre line crosses the face, as a fraction of that
    /// line's length. OpenFOAM flags `> 4`.
    pub max_skewness: f64,
    /// Maximum cell **aspect ratio** (`Σ|Sf| / (6 · V^{2/3})`, which is `1` for
    /// a cube). Large values mean sliver cells.
    pub max_aspect_ratio: f64,
    /// Smallest face area in the mesh.
    pub min_face_area: f64,
    /// Smallest (signed) cell volume in the mesh.
    pub min_cell_volume: f64,
    /// Number of cells with non-positive volume (a broken mesh has `> 0`).
    pub n_negative_volume_cells: usize,
}

impl QualityReport {
    /// A conservative "good enough to solve" gate: no negative-volume cells,
    /// non-orthogonality `< 70°`, and skewness `< 4` (OpenFOAM's default
    /// `checkMesh` thresholds).
    pub fn is_solvable(&self) -> bool {
        self.n_negative_volume_cells == 0
            && self.max_non_orthogonality_deg < 70.0
            && self.max_skewness < 4.0
    }
}

/// Compute the [`QualityReport`] for `mesh`.
pub fn check_quality(mesh: &VolumeMesh) -> QualityReport {
    let (vols, centres) = cell_geometry(mesh);

    let mut min_face_area = f64::MAX;
    let mut max_non_orth = 0.0f64;
    let mut max_skew = 0.0f64;
    for f in 0..mesh.face_count() {
        let sf = mesh.face_area_vector(f);
        let area = sf.length();
        min_face_area = min_face_area.min(area);
        // Non-orthogonality + skewness are defined on internal faces.
        if let Some(nb) = mesh.neighbour[f] {
            let co = centres[mesh.owner[f]];
            let cn = centres[nb];
            let d = cn.sub(co);
            let dlen = d.length();
            if dlen > 1e-30 && area > 1e-30 {
                let cos = (d.dot(sf) / (dlen * area)).clamp(-1.0, 1.0);
                max_non_orth = max_non_orth.max(cos.acos().to_degrees());
                // Skewness: where the centre line crosses the face plane.
                let fc = mesh.face_centroid(f);
                let denom = d.dot(sf);
                if denom.abs() > 1e-30 {
                    let t = fc.sub(co).dot(sf) / denom;
                    let fi = co.add(d.scale(t));
                    max_skew = max_skew.max(fc.sub(fi).length() / dlen);
                }
            }
        }
    }

    let mut max_ar = 0.0f64;
    let mut min_vol = f64::MAX;
    let mut n_neg = 0usize;
    // Sum of face areas per cell for the aspect ratio.
    let mut cell_area_sum = vec![0.0f64; mesh.n_cells];
    for f in 0..mesh.face_count() {
        let a = mesh.face_area_vector(f).length();
        cell_area_sum[mesh.owner[f]] += a;
        if let Some(nb) = mesh.neighbour[f] {
            cell_area_sum[nb] += a;
        }
    }
    for c in 0..mesh.n_cells {
        let v = vols[c];
        min_vol = min_vol.min(v);
        if v <= 0.0 {
            n_neg += 1;
        } else {
            let ar = cell_area_sum[c] / (6.0 * v.powf(2.0 / 3.0));
            max_ar = max_ar.max(ar);
        }
    }

    QualityReport {
        max_non_orthogonality_deg: max_non_orth,
        max_skewness: max_skew,
        max_aspect_ratio: max_ar,
        min_face_area: if min_face_area == f64::MAX { 0.0 } else { min_face_area },
        min_cell_volume: if min_vol == f64::MAX { 0.0 } else { min_vol },
        n_negative_volume_cells: n_neg,
    }
}

/// Per-cell `(volume, centroid)` via the OpenFOAM pyramid decomposition: each
/// face forms a pyramid with the cell's face-centroid average as apex; the
/// cell volume and centroid are the signed sums over those pyramids.
fn cell_geometry(mesh: &VolumeMesh) -> (Vec<f64>, Vec<Vec3>) {
    // Collect each cell's (face, outward-sign): +1 for its owner faces, −1 for
    // faces where it is the neighbour (the stored normal points owner→neighbour).
    let mut cell_faces: Vec<Vec<(usize, f64)>> = vec![Vec::new(); mesh.n_cells];
    for f in 0..mesh.face_count() {
        cell_faces[mesh.owner[f]].push((f, 1.0));
        if let Some(nb) = mesh.neighbour[f] {
            cell_faces[nb].push((f, -1.0));
        }
    }

    let mut vols = vec![0.0; mesh.n_cells];
    let mut centres = vec![Vec3::ZERO; mesh.n_cells];
    for c in 0..mesh.n_cells {
        let faces = &cell_faces[c];
        if faces.is_empty() {
            continue;
        }
        // Apex = average of this cell's face centroids.
        let mut apex = Vec3::ZERO;
        for &(f, _) in faces {
            apex = apex.add(mesh.face_centroid(f));
        }
        apex = apex.scale(1.0 / faces.len() as f64);

        let mut v = 0.0;
        let mut csum = Vec3::ZERO;
        for &(f, s) in faces {
            let sf = mesh.face_area_vector(f).scale(s); // outward of this cell
            let fc = mesh.face_centroid(f);
            let pv = fc.sub(apex).dot(sf) / 3.0; // signed pyramid volume
            let pc = fc.scale(0.75).add(apex.scale(0.25)); // pyramid centroid
            v += pv;
            csum = csum.add(pc.scale(pv));
        }
        vols[c] = v;
        centres[c] = if v.abs() > 1e-30 { csum.scale(1.0 / v) } else { apex };
    }
    (vols, centres)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartesian::cartesian_box;
    use crate::carve::carve_box;
    use crate::snap::snap_to_surface;

    /// V&V — a perfectly regular Cartesian cube mesh is ideal. Methodology:
    /// `cartesian_box` a unit box 3×3×3 (cubic cells) and check quality. Result:
    /// non-orthogonality and skewness ≈ 0, aspect ratio ≈ 1, no negative cells,
    /// min cell volume = (1/3)³; `is_solvable()` true.
    #[test]
    fn cartesian_cube_is_ideal() {
        let m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [3, 3, 3]);
        let q = check_quality(&m);
        assert!(q.max_non_orthogonality_deg < 1e-6, "orthogonal: {}", q.max_non_orthogonality_deg);
        assert!(q.max_skewness < 1e-9, "no skew: {}", q.max_skewness);
        assert!((q.max_aspect_ratio - 1.0).abs() < 1e-9, "cubic cells: AR {}", q.max_aspect_ratio);
        assert_eq!(q.n_negative_volume_cells, 0);
        assert!((q.min_cell_volume - (1.0 / 27.0)).abs() < 1e-12);
        assert!(q.is_solvable());
    }

    /// V&V — a stretched box has aspect ratio > 1 but stays orthogonal. A
    /// 10×1×1 single cell: AR = (2·1 + 4·10)/(6·10^{2/3}) ≈ 1.51 > 1; non-orth
    /// still 0 (Cartesian).
    #[test]
    fn stretched_cell_has_high_aspect_ratio() {
        let m = cartesian_box(Vec3::ZERO, Vec3::new(10.0, 1.0, 1.0), [1, 1, 1]);
        let q = check_quality(&m);
        assert!(q.max_aspect_ratio > 1.4, "stretched: AR {}", q.max_aspect_ratio);
        assert!(q.max_non_orthogonality_deg < 1e-6, "still orthogonal");
        assert_eq!(q.n_negative_volume_cells, 0);
    }

    /// V&V — the checks detect the distortion a snap introduces. A snapped
    /// octahedron carve has non-zero, finite non-orthogonality and skewness
    /// (the metric sees the body-fitting), and reports its negative-cell count
    /// honestly — the very information a mesher needs to decide if a snap is
    /// safe.
    #[test]
    fn snapped_mesh_metrics_are_finite_and_detect_distortion() {
        let (p, t) = octahedron(1.0);
        let snapped = snap_to_surface(&carve_box(&p, &t, 0.1), &p, &t);
        let q = check_quality(&snapped);
        assert!(q.max_non_orthogonality_deg.is_finite() && q.max_skewness.is_finite());
        assert!(q.max_aspect_ratio.is_finite() && q.min_cell_volume.is_finite());
        assert!(q.max_non_orthogonality_deg > 0.0, "snap introduces non-orthogonality");
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
}
