// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Algorithm reference (re-implemented in Rust, not transcribed):
//   cfMesh — https://github.com/wyldckat/cfMesh
//   meshLibrary/utilities/checkMeshDict, polyMeshGenChecks (non-orthogonality,
//   skewness, aspect ratio, negative-volume detection).
//   Copyright (C) 2014-2017 Creative Fields, Ltd. Licence: GPL-3.0-only.
//   The metric definitions and the pyramid decomposition follow OpenFOAM
//   `primitiveMeshCheck` / `checkMesh`:
//   Copyright (C) 2011-2016 OpenFOAM Foundation
//   Copyright (C) 2016-2023 OpenCFD Ltd. Licence: GPL-3.0-only.
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

//! Mesh **quality checks** — the Rust analogue of cfMesh's `polyMeshGenChecks`.
//!
//! A generated volume mesh must be *checked* before it is handed to a solver:
//! finite-volume discretisation error grows with face **non-orthogonality** and
//! **skewness**, ill-conditioning grows with cell **aspect ratio**, and a
//! **negative-volume** cell makes a case unsolvable. [`check_quality`] computes
//! these over a [`VolumeMesh`] and returns a [`QualityReport`].
//!
//! Two of the metrics are specifically **sliver detectors**, because the angle
//! metrics above are blind to slivers — a cell can be arbitrarily flat while
//! every one of its owner→neighbour lines stays perfectly orthogonal to the
//! face it crosses:
//!
//! - [`QualityReport::min_face_pyramid_volume`] catches a cell centre that has
//!   fallen onto or through one of its own faces (local inversion / tangling);
//! - [`QualityReport::min_cell_determinant`] catches flatness itself — the face
//!   normals collapsing towards a plane or a line — before the cell volume ever
//!   goes negative.
//!
//! This matters here because this crate's tet primal is a **centroid
//! subdivision, not a Delaunay triangulation** (see [`crate::delaunay`]), so
//! slivers are an expected failure mode of the mesher rather than a rare one.
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
    /// Smallest **face pyramid volume** in the mesh (a volume, so `m³` if the
    /// points are in metres), minimised over every *(cell, face)* incidence —
    /// an internal face is checked twice, once from each side.
    ///
    /// For face `f` of cell `c`, the pyramid has the face as its base and the
    /// **cell centre** as its apex, so its signed volume is
    /// `V_pyr = (1/3) · (c_f − C_c) · S_f`, with `c_f` the face centroid and
    /// `S_f` the face area vector taken **outward from `c`**.
    ///
    /// Valid range: strictly `> 0` for every face of a well-formed cell — a
    /// cube of side `h` gives `h³/6` on all six faces, and the pyramid volumes
    /// of a cell sum exactly to its volume. A value `<= 0` means the cell
    /// centre lies **on or outside the plane of one of its own faces**: the
    /// cell is locally inverted or tangled, the face's flux stencil points the
    /// wrong way, and OpenFOAM's `checkMesh` fails such a mesh outright.
    ///
    /// This is not redundant with [`QualityReport::min_cell_volume`]: a
    /// strongly concave cell can have a healthily positive *total* volume and
    /// still have its centre outside one of its faces (verified in this
    /// module's `concave_cell_has_negative_pyramid_but_positive_volume` test).
    /// Conversely, a merely **flat** (sliver) cell keeps *positive* pyramid
    /// volumes — they just become small — so flatness is
    /// [`QualityReport::min_cell_determinant`]'s job, not this metric's.
    ///
    /// `0.0` for a mesh with no faces.
    pub min_face_pyramid_volume: f64,
    /// Number of *(cell, face)* incidences whose pyramid volume is `<= 0`
    /// (see [`QualityReport::min_face_pyramid_volume`]). `0` for a valid mesh;
    /// any non-zero count is a hard failure in OpenFOAM's `checkMesh`. An
    /// internal face can contribute up to `2` (once per adjacent cell).
    ///
    /// Deliberately **not** wired into [`QualityReport::is_solvable`], whose
    /// thresholds are left unchanged; test it explicitly if you want
    /// `checkMesh`'s stricter gate.
    pub n_negative_pyramid_faces: usize,
    /// Smallest **cell determinant** over the mesh (dimensionless) — the
    /// sliver detector.
    ///
    /// # Definition implemented here
    ///
    /// For cell `c` with faces `f`, area vectors `S_f` and unit normals
    /// `n_f = S_f / |S_f|`, form the area-weighted normal-orientation tensor
    ///
    /// ```text
    ///         Σ_f |S_f| (n_f ⊗ n_f)
    ///   D_c = ─────────────────────
    ///              Σ_f |S_f|
    /// ```
    ///
    /// and report `27 · det(D_c)`, minimised over the cells. (Sign-free: `n_f`
    /// enters only through an outer product, so the face winding is
    /// irrelevant.)
    ///
    /// # Valid range and interpretation
    ///
    /// `D_c` is symmetric positive semi-definite with `tr(D_c) = 1`, so by
    /// AM–GM `det(D_c) <= (1/3)³` and the reported value lies in `[0, 1]`:
    ///
    /// - `1.0` — the face normals are **isotropic** (`D_c = I/3`). An
    ///   axis-aligned cube and a regular tetrahedron both give exactly `1`;
    ///   both are verified in this module's tests.
    /// - `→ 0` — the normals collapse towards a plane (flat sliver) or a line
    ///   (needle), i.e. the cell is degenerate in at least one direction. The
    ///   tensor loses rank *long before* the cell volume changes sign, which is
    ///   what makes this the metric that sees slivers when non-orthogonality,
    ///   skewness and `min_cell_volume` all read healthy.
    ///
    /// A cell whose faces all have zero area contributes `0.0`; a mesh with no
    /// cells reports `0.0`.
    ///
    /// # Parity caveat — read before quoting this against OpenFOAM
    ///
    /// This is the same *form* and normalisation as the "cell determinant"
    /// (`wellposedness`) figure OpenFOAM's `checkMesh` prints — a normalised
    /// determinant of the summed face-normal outer products, scaled so a cube
    /// reads `1` — but it was implemented from that description and from the
    /// algebra above, **not** transcribed from OpenFOAM source, and it has
    /// **not** been compared numerically against a `checkMesh` run on the same
    /// mesh. Treat it as a documented equivalent with the properties proved and
    /// tested here, **not** as verified `checkMesh` parity. No threshold from
    /// `checkMesh` is reproduced here, and [`QualityReport::is_solvable`] does
    /// not gate on it.
    pub min_cell_determinant: f64,
}

impl QualityReport {
    /// A conservative "good enough to solve" gate: no negative-volume cells,
    /// non-orthogonality `< 70°`, and skewness `< 4` (OpenFOAM's default
    /// `checkMesh` thresholds).
    ///
    /// The sliver metrics ([`QualityReport::n_negative_pyramid_faces`],
    /// [`QualityReport::min_cell_determinant`]) are deliberately **not** part
    /// of this gate — it is kept at its historical thresholds so existing
    /// callers' behaviour does not change. Check them separately for
    /// `checkMesh`'s stricter criteria.
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

    // Sliver detectors: face pyramid volumes about the cell centres, and the
    // per-cell face-normal orientation determinant. Both reuse `centres` from
    // the pyramid decomposition above, so they see exactly the cell centres the
    // solver would.
    let mut min_pyr = f64::MAX;
    let mut n_neg_pyr = 0usize;
    // Per-cell Σ|Sf| (n̂ ⊗ n̂) as [xx, yy, zz, xy, xz, yz], and Σ|Sf|.
    let mut normal_tensor = vec![[0.0f64; 6]; mesh.n_cells];
    let mut normal_area = vec![0.0f64; mesh.n_cells];
    for f in 0..mesh.face_count() {
        let sf = mesh.face_area_vector(f);
        let area = sf.length();
        let fc = mesh.face_centroid(f);
        // The owner's outward normal is +Sf; the neighbour's is −Sf.
        let incident: [Option<(usize, f64)>; 2] = [
            Some((mesh.owner[f], 1.0)),
            mesh.neighbour[f].map(|nb| (nb, -1.0)),
        ];
        for (c, sign) in incident.into_iter().flatten() {
            let pyr = fc.sub(centres[c]).dot(sf.scale(sign)) / 3.0;
            min_pyr = min_pyr.min(pyr);
            if pyr <= 0.0 {
                n_neg_pyr += 1;
            }
            if area > 1e-300 {
                // n̂ ⊗ n̂ is sign-free, so accumulate once per incidence.
                let n = sf.scale(1.0 / area);
                let t = &mut normal_tensor[c];
                t[0] += area * n.x * n.x;
                t[1] += area * n.y * n.y;
                t[2] += area * n.z * n.z;
                t[3] += area * n.x * n.y;
                t[4] += area * n.x * n.z;
                t[5] += area * n.y * n.z;
                normal_area[c] += area;
            }
        }
    }

    let mut min_det = f64::MAX;
    for c in 0..mesh.n_cells {
        let a = normal_area[c];
        let det = if a > 1e-300 {
            let t = normal_tensor[c];
            let (xx, yy, zz, xy, xz, yz) =
                (t[0] / a, t[1] / a, t[2] / a, t[3] / a, t[4] / a, t[5] / a);
            // Determinant of the symmetric 3×3 [[xx,xy,xz],[xy,yy,yz],[xz,yz,zz]].
            let d = xx * (yy * zz - yz * yz) - xy * (xy * zz - yz * xz) + xz * (xy * yz - yy * xz);
            // The tensor is positive semi-definite, so a negative value here is
            // round-off on a degenerate cell; clamp to the true lower bound.
            (27.0 * d).max(0.0)
        } else {
            // A cell with no faces, or only zero-area ones: fully degenerate.
            0.0
        };
        min_det = min_det.min(det);
    }

    QualityReport {
        max_non_orthogonality_deg: max_non_orth,
        max_skewness: max_skew,
        max_aspect_ratio: max_ar,
        min_face_area: if min_face_area == f64::MAX {
            0.0
        } else {
            min_face_area
        },
        min_cell_volume: if min_vol == f64::MAX { 0.0 } else { min_vol },
        n_negative_volume_cells: n_neg,
        min_face_pyramid_volume: if min_pyr == f64::MAX { 0.0 } else { min_pyr },
        n_negative_pyramid_faces: n_neg_pyr,
        min_cell_determinant: if min_det == f64::MAX { 0.0 } else { min_det },
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
        centres[c] = if v.abs() > 1e-30 {
            csum.scale(1.0 / v)
        } else {
            apex
        };
    }
    (vols, centres)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartesian::cartesian_box;
    use crate::carve::carve_box;
    use crate::snap::snap_to_surface;
    use crate::volume_mesh::BoundaryPatch;

    /// V&V — a perfectly regular Cartesian cube mesh is ideal. Methodology:
    /// `cartesian_box` a unit box 3×3×3 (cubic cells) and check quality. Result:
    /// non-orthogonality and skewness ≈ 0, aspect ratio ≈ 1, no negative cells,
    /// min cell volume = (1/3)³; `is_solvable()` true.
    #[test]
    fn cartesian_cube_is_ideal() {
        let m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [3, 3, 3]);
        let q = check_quality(&m);
        assert!(
            q.max_non_orthogonality_deg < 1e-6,
            "orthogonal: {}",
            q.max_non_orthogonality_deg
        );
        assert!(q.max_skewness < 1e-9, "no skew: {}", q.max_skewness);
        assert!(
            (q.max_aspect_ratio - 1.0).abs() < 1e-9,
            "cubic cells: AR {}",
            q.max_aspect_ratio
        );
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
        assert!(
            q.max_aspect_ratio > 1.4,
            "stretched: AR {}",
            q.max_aspect_ratio
        );
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
        assert!(
            q.max_non_orthogonality_deg > 0.0,
            "snap introduces non-orthogonality"
        );
    }

    // ── Sliver-metric fixtures ─────────────────────────────────────────────
    //
    // These build meshes by hand rather than through `from_cell_faces`, which
    // re-orients every ring outward from the *vertex-average* centre — exactly
    // the mis-orientation the concave and inverted fixtures below need to keep.

    /// A one-cell mesh from `points` and already-outward-wound face rings.
    fn single_cell(points: Vec<Vec3>, faces: Vec<Vec<usize>>) -> VolumeMesh {
        let n = faces.len();
        VolumeMesh {
            points,
            faces,
            owner: vec![0; n],
            neighbour: vec![None; n],
            n_cells: 1,
            patches: vec![BoundaryPatch {
                name: "walls".into(),
                start_face: 0,
                n_faces: n,
            }],
        }
    }

    /// A one-cell tetrahedron `(a, b, c, d)`, wound outward (`b`/`c` swapped if
    /// the input is negatively oriented).
    fn tet_cell(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> VolumeMesh {
        let (b, c) = if b.sub(a).cross(c.sub(a)).dot(d.sub(a)) > 0.0 {
            (b, c)
        } else {
            (c, b)
        };
        single_cell(
            vec![a, b, c, d],
            vec![vec![0, 2, 1], vec![0, 1, 3], vec![1, 2, 3], vec![2, 0, 3]],
        )
    }

    /// A one-cell prism: the **counter-clockwise** polygon `poly` in the
    /// `z = z0` plane, extruded to `z1 > z0`, every face wound outward.
    fn extruded_prism(poly: &[(f64, f64)], z0: f64, z1: f64) -> VolumeMesh {
        let n = poly.len();
        let mut points = Vec::with_capacity(2 * n);
        for &(x, y) in poly {
            points.push(Vec3::new(x, y, z0));
        }
        for &(x, y) in poly {
            points.push(Vec3::new(x, y, z1));
        }
        let mut faces: Vec<Vec<usize>> = Vec::with_capacity(n + 2);
        faces.push((0..n).rev().collect()); // bottom, normal −z
        faces.push((n..2 * n).collect()); // top, normal +z
        for i in 0..n {
            let j = (i + 1) % n;
            faces.push(vec![i, j, n + j, n + i]); // side, normal outward
        }
        single_cell(points, faces)
    }

    /// Every face ring reversed — turns a valid cell inside out (all normals
    /// point inward), the canonical inverted cell.
    fn with_faces_reversed(mut m: VolumeMesh) -> VolumeMesh {
        for f in m.faces.iter_mut() {
            f.reverse();
        }
        m
    }

    /// Two flat tets sharing the triangle `abc` in the `z = 0` plane, with
    /// apexes at `±eps` **directly above/below that triangle's centroid**.
    ///
    /// The symmetry is deliberate: it puts both cell centres on the face normal
    /// *through the face centroid*, so the shared internal face has exactly
    /// zero non-orthogonality **and** zero skewness while both cells are pure
    /// slivers. That is the configuration the angle metrics cannot see.
    fn sliver_bipyramid(eps: f64) -> VolumeMesh {
        let points = vec![
            Vec3::new(0.0, 0.0, 0.0),        // 0 = a
            Vec3::new(1.0, 0.0, 0.0),        // 1 = b
            Vec3::new(0.5, 1.0, 0.0),        // 2 = c
            Vec3::new(0.5, 1.0 / 3.0, eps),  // 3 = d, apex of cell 0 (above)
            Vec3::new(0.5, 1.0 / 3.0, -eps), // 4 = e, apex of cell 1 (below)
        ];
        let faces = vec![
            vec![0, 2, 1], // shared abc: normal −z, owner (above) → neighbour (below)
            vec![0, 1, 3],
            vec![1, 2, 3],
            vec![2, 0, 3], // cell 0, outward
            vec![0, 2, 4],
            vec![2, 1, 4],
            vec![1, 0, 4], // cell 1, outward
        ];
        VolumeMesh {
            points,
            faces,
            owner: vec![0, 0, 0, 0, 1, 1, 1],
            neighbour: vec![Some(1), None, None, None, None, None, None],
            n_cells: 2,
            patches: vec![BoundaryPatch {
                name: "walls".into(),
                start_face: 1,
                n_faces: 6,
            }],
        }
    }

    /// V&V — the sliver metrics read their ideal values on a uniform Cartesian
    /// block. Methodology: `cartesian_box` on the unit cube, 3x3x3, so every
    /// cell is a cube of side `h = 1/3`; closed form says every face pyramid
    /// volume is `h³/6 = 1/162 = 6.1728e-3` (the six of them summing to the
    /// cell volume `h³`), and the normal tensor of a cube is `I/3`, giving
    /// `27·det = 1` exactly. Result (measured 2026-08-13):
    /// `min_face_pyramid_volume = 0.00617283950617283`, which agrees with
    /// `1/162` to `9.5e-18`; `n_negative_pyramid_faces = 0`;
    /// `min_cell_determinant = 0.9999999999999993`, i.e. `7e-16` (about 6 ulp)
    /// below the analytic value of 1.
    #[test]
    fn cartesian_cube_sliver_metrics_are_ideal() {
        let m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [3, 3, 3]);
        let q = check_quality(&m);
        let h = 1.0 / 3.0;
        assert_eq!(q.n_negative_pyramid_faces, 0);
        assert!(
            (q.min_face_pyramid_volume - h * h * h / 6.0).abs() < 1e-15,
            "cube face pyramid = h³/6: got {}",
            q.min_face_pyramid_volume
        );
        assert!(
            (q.min_cell_determinant - 1.0).abs() < 1e-12,
            "cube determinant = 1: got {}",
            q.min_cell_determinant
        );
    }

    /// V&V — the cell determinant is normalised so a **regular tetrahedron**
    /// also reads exactly 1, which pins the `27·det` scaling independently of
    /// the cube. Methodology: the regular tet on `(1,1,1)`, `(1,-1,-1)`,
    /// `(-1,1,-1)`, `(-1,-1,1)`; its four equal-area unit normals form a tight
    /// frame with `Σ n⊗n = (4/3)I`, so `D = I/3` and `27·det(D) = 1` in closed
    /// form. Result (measured 2026-08-13): `min_cell_determinant =
    /// 1.0000000000000002` (2 ulp above the analytic 1),
    /// `min_face_pyramid_volume = 0.6666666666666666` (`= V/4` with
    /// `V = 2.6666666666666665 = 8/3`), `n_negative_pyramid_faces = 0`.
    #[test]
    fn regular_tet_has_unit_cell_determinant() {
        let m = tet_cell(
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(1.0, -1.0, -1.0),
            Vec3::new(-1.0, 1.0, -1.0),
            Vec3::new(-1.0, -1.0, 1.0),
        );
        assert!(m.validate().is_ok(), "fixture is a closed cell");
        let q = check_quality(&m);
        assert!(
            (q.min_cell_determinant - 1.0).abs() < 1e-12,
            "regular tet determinant = 1: got {}",
            q.min_cell_determinant
        );
        assert_eq!(q.n_negative_pyramid_faces, 0);
        assert!(q.min_face_pyramid_volume > 0.0);
    }

    /// V&V — **the point of this metric pair**: a sliver that the existing
    /// angle metrics are blind to.
    ///
    /// Methodology: `sliver_bipyramid(1e-3)` — two tets sharing a triangle in
    /// the `z = 0` plane with apexes `±1e-3` directly over that triangle's
    /// centroid. By symmetry both cell centres sit on the shared face's normal
    /// and on its centroid, so non-orthogonality and skewness are *analytically
    /// zero* while both cells are 1000:1 flat. Assert both halves: the old
    /// metrics stay clean, the new determinant collapses.
    ///
    /// Result (measured 2026-08-13), `eps = 1e-3`:
    /// `max_non_orthogonality_deg = 0.0` exactly and `max_skewness =
    /// 1.11e-13` (pure round-off) — the angle metrics see nothing, and
    /// `is_solvable()` returns `true` — while `min_cell_determinant =
    /// 1.8224617280920192e-10`, ten orders of magnitude below the cube's
    /// `0.9999999999999993`. Squashing the cell tenfold further
    /// (`eps = 1e-4`) drops it to `1.8224996172750592e-14` — a factor `1e4`,
    /// i.e. `det ∝ eps⁴`, which is what the rank-deficiency argument predicts
    /// (two eigenvalues each collapsing as `eps²`).
    /// `max_aspect_ratio = 55.03` — the weak proxy does react, but only by a
    /// factor ~55 and it carries no pass/fail threshold.
    /// Pyramid volumes stay **positive** (`min = 4.166666666666665e-5`),
    /// correctly: these cells are flat, not inverted — exactly the division of
    /// labour documented on the two fields.
    #[test]
    fn flat_sliver_escapes_non_orthogonality_but_not_the_determinant() {
        let m = sliver_bipyramid(1e-3);
        assert!(m.validate().is_ok(), "fixture is a closed 2-cell mesh");
        let q = check_quality(&m);

        // Half 1 — the existing metrics do NOT flag it.
        assert!(
            q.max_non_orthogonality_deg < 1e-9,
            "sliver is invisible to non-orthogonality: got {}",
            q.max_non_orthogonality_deg
        );
        assert!(
            q.max_skewness < 1e-9,
            "sliver is invisible to skewness: got {}",
            q.max_skewness
        );
        assert_eq!(
            q.n_negative_volume_cells, 0,
            "a sliver still has positive volume"
        );
        assert!(q.is_solvable(), "the historical gate passes this sliver");

        // Half 2 — the new determinant does flag it.
        assert!(
            q.min_cell_determinant < 1e-3,
            "determinant must collapse on a sliver: got {}",
            q.min_cell_determinant
        );
        // ... and it is a *relative* collapse, not an artefact of small cells:
        let cube = check_quality(&cartesian_box(
            Vec3::ZERO,
            Vec3::new(1.0, 1.0, 1.0),
            [3, 3, 3],
        ));
        assert!(
            q.min_cell_determinant < cube.min_cell_determinant / 1e4,
            "sliver {} vs cube {}",
            q.min_cell_determinant,
            cube.min_cell_determinant
        );
        // Flat, not inverted: the pyramid check correctly stays quiet.
        assert!(q.min_face_pyramid_volume > 0.0);
        assert_eq!(q.n_negative_pyramid_faces, 0);

        // Flattening it further drives the determinant further toward 0.
        let flatter = check_quality(&sliver_bipyramid(1e-4));
        assert!(
            flatter.min_cell_determinant < q.min_cell_determinant,
            "flatter sliver must score worse: {} vs {}",
            flatter.min_cell_determinant,
            q.min_cell_determinant
        );
    }

    /// V&V — an **inverted** cell is caught and counted. Methodology: take a
    /// valid unit-cube cell and reverse every face ring, so all six normals
    /// point inward — the canonical inversion. Every face pyramid then has the
    /// cell centre on its outer side. Result (measured 2026-08-13):
    /// `n_negative_pyramid_faces = 6` (all six), `min_face_pyramid_volume =
    /// -0.16666666666666666` (`= -h³/6` with `h = 1`), and the pre-existing
    /// `n_negative_volume_cells = 1` with `min_cell_volume =
    /// -0.9999999999999999`, i.e. the new check agrees with the old one on a
    /// fully inverted cell. The un-reversed fixture reads
    /// `min_face_pyramid_volume = +0.16666666666666666` with zero negatives.
    #[test]
    fn inverted_cell_has_negative_pyramid_volumes() {
        let cube = extruded_prism(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)], 0.0, 1.0);
        let good = check_quality(&cube);
        assert_eq!(good.n_negative_pyramid_faces, 0, "sanity: fixture is valid");
        assert!((good.min_face_pyramid_volume - 1.0 / 6.0).abs() < 1e-15);

        let q = check_quality(&with_faces_reversed(cube));
        assert_eq!(q.n_negative_pyramid_faces, 6, "all six faces inverted");
        assert!(
            (q.min_face_pyramid_volume + 1.0 / 6.0).abs() < 1e-15,
            "inverted cube pyramid = −1/6: got {}",
            q.min_face_pyramid_volume
        );
        assert_eq!(q.n_negative_volume_cells, 1);
    }

    /// V&V — the pyramid check catches what a **volume** check cannot: a cell
    /// with a healthy positive volume whose centre still falls outside three of
    /// its own faces.
    ///
    /// Methodology: a U-shaped prism (the polygon
    /// `(0,0) (3,0) (3,3) (2,3) (2,1) (1,1) (1,3) (0,3)` extruded `z ∈ [0,1]`),
    /// volume 7. Its centre lies inside the notch's *span*, so the three faces
    /// lining the notch (`x = 2`, `y = 1`, `x = 1`) have the cell centre on
    /// their outward side. Result (measured 2026-08-13):
    /// `min_cell_volume = 7.000000000000002` and `n_negative_volume_cells = 0`
    /// — the volume check is perfectly happy — while
    /// `n_negative_pyramid_faces = 3` and `min_face_pyramid_volume =
    /// -0.33333333333333365`. Every other metric reads healthy:
    /// `max_non_orthogonality_deg = 0` and `max_skewness = 0` (a single cell
    /// has no internal faces), `max_aspect_ratio = 1.37`, and
    /// `min_cell_determinant = 0.84` — correctly, since a U-prism is *not* a
    /// sliver. The pyramid volume is the only metric in the report that sees
    /// this defect.
    #[test]
    fn concave_cell_has_negative_pyramid_but_positive_volume() {
        let u = extruded_prism(
            &[
                (0.0, 0.0),
                (3.0, 0.0),
                (3.0, 3.0),
                (2.0, 3.0),
                (2.0, 1.0),
                (1.0, 1.0),
                (1.0, 3.0),
                (0.0, 3.0),
            ],
            0.0,
            1.0,
        );
        assert!(u.validate().is_ok(), "fixture is a closed cell");
        let q = check_quality(&u);
        assert!(
            (q.min_cell_volume - 7.0).abs() < 1e-12,
            "U-prism volume = 7: got {}",
            q.min_cell_volume
        );
        assert_eq!(q.n_negative_volume_cells, 0, "volume check sees nothing");
        assert_eq!(
            q.n_negative_pyramid_faces, 3,
            "the three notch walls are on the wrong side of the cell centre"
        );
        assert!(
            q.min_face_pyramid_volume < -0.3,
            "min pyramid volume: got {}",
            q.min_face_pyramid_volume
        );
    }

    /// V&V — robustness. Methodology: run `check_quality` on (a) a mesh with no
    /// points, faces or cells and (b) a single-cell Cartesian box, and check
    /// that neither panics or produces a non-finite value. Result (measured
    /// 2026-08-13): the empty mesh returns all-zero (`min_face_pyramid_volume =
    /// 0`, `n_negative_pyramid_faces = 0`, `min_cell_determinant = 0`, matching
    /// the existing `min_face_area`/`min_cell_volume` convention for an empty
    /// mesh); the 1x1x1 box returns `min_face_pyramid_volume =
    /// 0.16666666666666666 = 1/6` and `min_cell_determinant = 1.0` exactly,
    /// with no negative pyramids.
    #[test]
    fn sliver_metrics_are_robust_on_empty_and_single_cell_meshes() {
        let empty = VolumeMesh {
            points: Vec::new(),
            faces: Vec::new(),
            owner: Vec::new(),
            neighbour: Vec::new(),
            n_cells: 0,
            patches: Vec::new(),
        };
        let q = check_quality(&empty);
        assert_eq!(q.min_face_pyramid_volume, 0.0);
        assert_eq!(q.n_negative_pyramid_faces, 0);
        assert_eq!(q.min_cell_determinant, 0.0);

        let one = check_quality(&cartesian_box(
            Vec3::ZERO,
            Vec3::new(1.0, 1.0, 1.0),
            [1, 1, 1],
        ));
        assert!(one.min_face_pyramid_volume.is_finite() && one.min_cell_determinant.is_finite());
        assert!((one.min_face_pyramid_volume - 1.0 / 6.0).abs() < 1e-15);
        assert!((one.min_cell_determinant - 1.0).abs() < 1e-12);
        assert_eq!(one.n_negative_pyramid_faces, 0);
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
            [0, 2, 4],
            [2, 1, 4],
            [1, 3, 4],
            [3, 0, 4],
            [0, 5, 2],
            [2, 5, 1],
            [1, 5, 3],
            [3, 5, 0],
        ];
        (v, t)
    }
}
