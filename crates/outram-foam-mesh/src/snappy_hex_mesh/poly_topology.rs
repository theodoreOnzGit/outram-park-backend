// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

//! Point-based polyhedral mesh topology — the moving-mesh substrate the
//! snapping ([`crate::snappy_hex_mesh::snapping`]) and layer-addition
//! ([`crate::snappy_hex_mesh::layers`]) phases operate on.
//!
//! ## Why this module exists
//!
//! [`FvMesh`](outram_foam_basic_lib::mesh::FvMesh) stores only *geometry* —
//! cell volumes/centres and face areas/centres — and no point coordinates or
//! face→point connectivity. Snapping and layer addition **move and add points**
//! and must then recompute all that geometry. This module carries the missing
//! half: the deduplicated point list plus, for every face, the ordered list of
//! point indices that form it (its polygon). From points + connectivity the
//! finite-volume geometry is regenerated exactly, so the workflow is:
//!
//! 1. build / obtain a [`PolyPatchMesh`] (castellation produces one),
//! 2. move [`PolyPatchMesh::points`] and/or splice in new faces/cells,
//! 3. call [`PolyPatchMesh::build_fvmesh`] to get a fresh, validated `FvMesh`.
//!
//! ## Geometry provenance
//!
//! The face and cell geometry recomputation mirrors OpenFOAM's
//! `primitiveMeshTools`:
//! - face centre/area — `primitiveMeshFaceCentresAndAreas.C` /
//!   `makeFaceCentresAndAreas` (fan triangulation about the point average,
//!   area-weighted centroid). See
//!   `src/OpenFOAM/meshes/primitiveMesh/primitiveMeshCheck/primitiveMeshTools.C`.
//! - cell centre/volume — `primitiveMeshCellCentresAndVols.C` /
//!   `makeCellCentresAndVols` (face-pyramid decomposition about the face-centre
//!   average, `pyr3Vol = Sf · (Cf − cEst)`, volume ×1/3).
//! - non-orthogonality / skewness quality — `primitiveMeshTools::faceOrthogonality`
//!   and `faceSkewness`.
//!
//! ## Face-winding convention (REQUIRED of producers)
//!
//! Every face's point list must be ordered so the polygon's right-hand-rule
//! normal points **owner → neighbour** for an internal face and **outward from
//! the owner cell** for a boundary face — identical to OpenFOAM's `faceAreas`
//! sign convention and to what [`FvMesh`] expects. [`build_fvmesh`] trusts this
//! ordering rather than re-deriving it from cell centres.
//!
//! [`build_fvmesh`]: PolyPatchMesh::build_fvmesh

use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMesh, FvMeshBuilder};
use outram_foam_basic_lib::primitives::Vector3;

use crate::MeshError;

/// A polyhedral mesh in point + face-connectivity form.
///
/// Faces are stored in OpenFOAM order: the `n_internal_faces` internal faces
/// (both owner and neighbour) first, then the boundary faces grouped by patch
/// exactly as [`patches`](Self::patches) describes. See the module docs for the
/// face-winding convention this type requires.
#[derive(Debug, Clone)]
pub struct PolyPatchMesh {
    /// Mesh point coordinates `[m]`. Moving snapping/layer motion edits this.
    pub points: Vec<Vector3>,
    /// Per-face ordered point indices into [`points`](Self::points). Internal
    /// faces first (`[0, n_internal_faces)`), then boundary faces by patch.
    pub faces: Vec<Vec<usize>>,
    /// `owner[f]` — owning cell of face `f` (all faces).
    pub owner: Vec<usize>,
    /// `neighbour[f]` — neighbour cell of internal face `f`
    /// (length == `n_internal_faces`).
    pub neighbour: Vec<usize>,
    /// Number of internal faces (faces with a neighbour cell).
    pub n_internal_faces: usize,
    /// Number of cells.
    pub n_cells: usize,
    /// Boundary patch descriptors over the boundary faces (in face-index order).
    pub patches: Vec<BoundaryPatch>,
}

/// Mesh-quality summary — the metrics `snappyHexMesh` gates point motion and
/// layer addition on (a subset of `meshQualityControls`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshQuality {
    /// Worst internal-face non-orthogonality `[degrees]` — the angle between the
    /// face area vector and the owner→neighbour centre-to-centre vector. `0` is
    /// perfectly orthogonal; OpenFOAM's default reject threshold is `65°`.
    pub max_non_ortho_deg: f64,
    /// Worst face skewness (dimensionless) over all faces — the normalised
    /// offset of the face centre from the owner–neighbour line. OpenFOAM's
    /// default reject threshold is `4.0`.
    pub max_skewness: f64,
    /// Smallest cell volume `[m³]`. Non-positive means an inverted / degenerate
    /// cell (fatal).
    pub min_cell_volume: f64,
    /// Number of cells whose volume is `<= 0` (negative-volume / inverted).
    pub n_negative_volume_cells: usize,
}

/// Acceptance thresholds for [`MeshQuality`] — the quality gate.
///
/// Mirrors the handful of `meshQualityControls` limits the snapping and layer
/// phases enforce. A candidate mesh passes iff every metric is within limits.
#[derive(Debug, Clone, Copy)]
pub struct QualityLimits {
    /// Maximum allowed non-orthogonality `[degrees]` (OpenFOAM default `65`).
    pub max_non_ortho_deg: f64,
    /// Maximum allowed skewness (OpenFOAM default `4.0`).
    pub max_skewness: f64,
    /// Minimum allowed cell volume `[m³]` (OpenFOAM default `1e-13`).
    pub min_vol: f64,
}

impl Default for QualityLimits {
    fn default() -> Self {
        Self {
            max_non_ortho_deg: 65.0,
            max_skewness: 4.0,
            min_vol: 1e-13,
        }
    }
}

impl QualityLimits {
    /// True if `q` satisfies every limit (mesh is acceptable).
    pub fn accepts(&self, q: &MeshQuality) -> bool {
        q.n_negative_volume_cells == 0
            && q.min_cell_volume >= self.min_vol
            && q.max_non_ortho_deg <= self.max_non_ortho_deg
            && q.max_skewness <= self.max_skewness
    }
}

/// Area vector `[m²]` and centre `[m]` of one polygonal face, by fan triangulation
/// about the point average (OpenFOAM `makeFaceCentresAndAreas`).
///
/// The returned area vector obeys the right-hand rule over the point order; a
/// triangle is handled directly for exactness. Units: `points` in metres, area
/// in `m²`, centre in metres.
pub fn face_area_and_centre(face: &[usize], points: &[Vector3]) -> (Vector3, Vector3) {
    let n = face.len();
    if n < 3 {
        return (Vector3::ZERO, Vector3::ZERO);
    }
    if n == 3 {
        let (a, b, c) = (points[face[0]], points[face[1]], points[face[2]]);
        let area = (b - a).cross(c - a) * 0.5;
        let centre = (a + b + c) / 3.0;
        return (area, centre);
    }

    // Estimated centre = average of the face's points.
    let mut f_centre = Vector3::ZERO;
    for &p in face {
        f_centre += points[p];
    }
    f_centre /= n as f64;

    let mut sum_n = Vector3::ZERO; // Σ triangle area-normals (×2)
    let mut sum_a = 0.0f64; // Σ |n|
    let mut sum_ac = Vector3::ZERO; // Σ |n| · (thisP + nextP + fCentre)
    for i in 0..n {
        let this_p = points[face[i]];
        let next_p = points[face[(i + 1) % n]];
        let c = this_p + next_p + f_centre;
        let nrm = (next_p - this_p).cross(f_centre - this_p);
        let a = nrm.mag();
        sum_n += nrm;
        sum_a += a;
        sum_ac += c * a;
    }
    if sum_a < 1e-300 {
        (Vector3::ZERO, f_centre)
    } else {
        ((sum_n) * 0.5, (sum_ac / sum_a) / 3.0)
    }
}

impl PolyPatchMesh {
    /// Number of faces (internal + boundary).
    pub fn n_faces(&self) -> usize {
        self.faces.len()
    }

    /// Number of boundary faces.
    pub fn n_boundary_faces(&self) -> usize {
        self.faces.len() - self.n_internal_faces
    }

    /// Face area vectors `[m²]` and centres `[m]` for every face, in face order.
    pub fn face_geometry(&self) -> (Vec<Vector3>, Vec<Vector3>) {
        let mut areas = Vec::with_capacity(self.faces.len());
        let mut centres = Vec::with_capacity(self.faces.len());
        for f in &self.faces {
            let (sf, cf) = face_area_and_centre(f, &self.points);
            areas.push(sf);
            centres.push(cf);
        }
        (areas, centres)
    }

    /// Cell centres `[m]` and volumes `[m³]` from the face geometry, by the
    /// face-pyramid decomposition of OpenFOAM `makeCellCentresAndVols`.
    ///
    /// `face_areas`/`face_centres` must be the arrays returned by
    /// [`face_geometry`](Self::face_geometry).
    pub fn cell_geometry(
        &self,
        face_areas: &[Vector3],
        face_centres: &[Vector3],
    ) -> (Vec<Vector3>, Vec<f64>) {
        let nc = self.n_cells;
        // Approximate cell centre = average of its face centres.
        let mut c_est = vec![Vector3::ZERO; nc];
        let mut n_cell_faces = vec![0usize; nc];
        for (f, &o) in self.owner.iter().enumerate() {
            c_est[o] += face_centres[f];
            n_cell_faces[o] += 1;
        }
        for (f, &nb) in self.neighbour.iter().enumerate() {
            c_est[nb] += face_centres[f];
            n_cell_faces[nb] += 1;
        }
        for c in 0..nc {
            if n_cell_faces[c] > 0 {
                c_est[c] /= n_cell_faces[c] as f64;
            }
        }

        let mut ctrs = vec![Vector3::ZERO; nc];
        let mut vols = vec![0.0f64; nc];
        // Owner side: outward face normal.
        for (f, &o) in self.owner.iter().enumerate() {
            let fc = face_centres[f];
            let fa = face_areas[f];
            let pyr3 = fa.dot(fc - c_est[o]);
            let pc = fc * 0.75 + c_est[o] * 0.25;
            ctrs[o] += pc * pyr3;
            vols[o] += pyr3;
        }
        // Neighbour side: normal points into the cell, so negate.
        for (f, &nb) in self.neighbour.iter().enumerate() {
            let fc = face_centres[f];
            let fa = face_areas[f];
            let pyr3 = fa.dot(c_est[nb] - fc);
            let pc = fc * 0.75 + c_est[nb] * 0.25;
            ctrs[nb] += pc * pyr3;
            vols[nb] += pyr3;
        }
        for c in 0..nc {
            if vols[c].abs() > 1e-300 {
                ctrs[c] /= vols[c];
            } else {
                ctrs[c] = c_est[c];
            }
            vols[c] /= 3.0;
        }
        (ctrs, vols)
    }

    /// Regenerate a validated [`FvMesh`] from the current points + connectivity.
    ///
    /// Recomputes all finite-volume geometry (face areas/centres, cell
    /// volumes/centres) from [`points`](Self::points), then assembles and
    /// validates the mesh. This is the "rebuild" step of snapping and layer
    /// addition.
    ///
    /// # Errors
    /// [`MeshError::Construction`] if the assembled mesh fails
    /// [`FvMesh::validate`].
    pub fn build_fvmesh(&self) -> Result<FvMesh, MeshError> {
        let (areas, centres) = self.face_geometry();
        let (cell_ctrs, cell_vols) = self.cell_geometry(&areas, &centres);

        FvMeshBuilder::new()
            .n_cells(self.n_cells)
            .n_internal_faces(self.n_internal_faces)
            .owner(self.owner.clone())
            .neighbour(self.neighbour.clone())
            .patches(self.patches.clone())
            .cell_volumes(cell_vols)
            .cell_centres(cell_ctrs)
            .face_area_vectors(areas)
            .face_centres(centres)
            .build()
            .map_err(|e| MeshError::Construction(format!("rebuilt mesh invalid: {e}")))
    }

    /// Mesh-quality metrics (non-orthogonality, skewness, min volume) computed
    /// from the current point positions.
    ///
    /// Mirrors OpenFOAM `primitiveMeshTools::faceOrthogonality` /
    /// `faceSkewness`. Used by the snapping and layer phases to accept or reject
    /// a candidate point motion.
    pub fn quality(&self) -> MeshQuality {
        let (areas, centres) = self.face_geometry();
        let (cell_ctrs, cell_vols) = self.cell_geometry(&areas, &centres);

        let mut max_non_ortho = 0.0f64;
        for f in 0..self.n_internal_faces {
            let (o, nb) = (self.owner[f], self.neighbour[f]);
            let d = cell_ctrs[nb] - cell_ctrs[o];
            let s = areas[f];
            let denom = d.mag() * s.mag() + 1e-300;
            let cos = (d.dot(s) / denom).clamp(-1.0, 1.0);
            let ang = cos.acos().to_degrees();
            if ang > max_non_ortho {
                max_non_ortho = ang;
            }
        }

        let mut max_skew = 0.0f64;
        // Internal-face skewness.
        for f in 0..self.n_internal_faces {
            let (o, nb) = (self.owner[f], self.neighbour[f]);
            let s = self.face_skewness(f, &areas, &centres, cell_ctrs[o], Some(cell_ctrs[nb]));
            if s > max_skew {
                max_skew = s;
            }
        }
        // Boundary-face skewness (mirror-cell treatment: neighbour = None).
        for f in self.n_internal_faces..self.faces.len() {
            let o = self.owner[f];
            let s = self.face_skewness(f, &areas, &centres, cell_ctrs[o], None);
            if s > max_skew {
                max_skew = s;
            }
        }

        let mut min_vol = f64::INFINITY;
        let mut n_neg = 0usize;
        for &v in &cell_vols {
            if v < min_vol {
                min_vol = v;
            }
            if v <= 0.0 {
                n_neg += 1;
            }
        }
        if !min_vol.is_finite() {
            min_vol = 0.0;
        }

        MeshQuality {
            max_non_ortho_deg: max_non_ortho,
            max_skewness: max_skew,
            min_cell_volume: min_vol,
            n_negative_volume_cells: n_neg,
        }
    }

    /// Skewness of face `f` (OpenFOAM `primitiveMeshTools::faceSkewness`).
    ///
    /// For a boundary face pass `nei_cc = None`: OpenFOAM mirrors the owner
    /// centre across the face (`neiCc = ownCc + 2·(Cf − ownCc)`), i.e.
    /// `d = 2·(Cf − ownCc)`.
    fn face_skewness(
        &self,
        f: usize,
        areas: &[Vector3],
        centres: &[Vector3],
        own_cc: Vector3,
        nei_cc: Option<Vector3>,
    ) -> f64 {
        let cf = centres[f];
        let d = match nei_cc {
            Some(nc) => nc - own_cc,
            None => (cf - own_cc) * 2.0,
        };
        let sf = areas[f];
        let cpf = cf - own_cc;
        let sv = cpf - d * (sf.dot(cpf) / (sf.dot(d) + 1e-300));
        let sv_mag = sv.mag();
        let sv_hat = sv / (sv_mag + 1e-300);
        // Normalisation distance = max projection of face-point offsets onto sv.
        let mut fd = 0.2 * d.mag() + 1e-300;
        for &p in &self.faces[f] {
            let proj = sv_hat.dot(self.points[p] - cf).abs();
            if proj > fd {
                fd = proj;
            }
        }
        sv_mag / fd
    }

    /// Convert to the writable `outram-foam-basic-lib`
    /// [`PolyMesh`](outram_foam_basic_lib::io::PolyMesh) — the on-disk
    /// `constant/polyMesh` representation.
    ///
    /// This is a pure re-packing: points, face vertex loops, owner/neighbour
    /// and patches are carried across unchanged (both types use the same
    /// internal-faces-first ordering and the same winding convention), with
    /// each face's `neighbour` becoming `Some(..)` for the first
    /// [`n_internal_faces`](Self::n_internal_faces) faces and `None` after.
    /// No geometry is recomputed.
    ///
    /// Use it to write a snapped mesh to an OpenFOAM case
    /// ([`PolyMesh::write`](outram_foam_basic_lib::io::PolyMesh::write)) or to
    /// grade it with
    /// [`assess_quality`](crate::mesh_quality::assess_quality).
    pub fn to_foam_poly_mesh(&self) -> outram_foam_basic_lib::io::PolyMesh {
        use outram_foam_basic_lib::io::{MeshFace, PolyMesh};
        let faces = self
            .faces
            .iter()
            .enumerate()
            .map(|(f, verts)| MeshFace {
                verts: verts.clone(),
                owner: self.owner[f],
                neighbour: if f < self.n_internal_faces {
                    Some(self.neighbour[f])
                } else {
                    None
                },
            })
            .collect();
        PolyMesh {
            points: self.points.clone(),
            faces,
            n_internal_faces: self.n_internal_faces,
            n_cells: self.n_cells,
            patches: self.patches.clone(),
        }
    }

    /// Global face indices belonging to boundary patch `patch_index`.
    pub fn patch_face_ids(&self, patch_index: usize) -> std::ops::Range<usize> {
        let p = &self.patches[patch_index];
        p.start..p.end()
    }

    /// The set of point indices used by boundary patch `patch_index`
    /// (deduplicated, sorted). These are the points a wall patch's layers extrude
    /// from, or the points snapping projects.
    pub fn patch_point_ids(&self, patch_index: usize) -> Vec<usize> {
        let mut set: Vec<usize> = Vec::new();
        for f in self.patch_face_ids(patch_index) {
            for &p in &self.faces[f] {
                set.push(p);
            }
        }
        set.sort_unstable();
        set.dedup();
        set
    }

    /// A boolean flag per point: `true` if the point lies on any boundary face.
    pub fn boundary_point_flags(&self) -> Vec<bool> {
        let mut flags = vec![false; self.points.len()];
        for f in self.n_internal_faces..self.faces.len() {
            for &p in &self.faces[f] {
                flags[p] = true;
            }
        }
        flags
    }

    /// For each point, the global face indices that use it (point→face adjacency).
    pub fn point_faces(&self) -> Vec<Vec<usize>> {
        let mut pf = vec![Vec::new(); self.points.len()];
        for (fi, f) in self.faces.iter().enumerate() {
            for &p in f {
                pf[p].push(fi);
            }
        }
        pf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::mesh::PatchKind;

    /// Build a single unit-cube cell as a `PolyPatchMesh` with all 6 boundary
    /// faces wound outward.
    fn unit_cube() -> PolyPatchMesh {
        // 8 corners of [0,1]^3.
        let points = vec![
            Vector3::new(0.0, 0.0, 0.0), // 0
            Vector3::new(1.0, 0.0, 0.0), // 1
            Vector3::new(1.0, 1.0, 0.0), // 2
            Vector3::new(0.0, 1.0, 0.0), // 3
            Vector3::new(0.0, 0.0, 1.0), // 4
            Vector3::new(1.0, 0.0, 1.0), // 5
            Vector3::new(1.0, 1.0, 1.0), // 6
            Vector3::new(0.0, 1.0, 1.0), // 7
        ];
        // 6 outward-wound quad faces (right-hand normal points out of the cube).
        let faces = vec![
            vec![0, 4, 7, 3], // x-min (normal -x)
            vec![1, 2, 6, 5], // x-max (normal +x)
            vec![0, 1, 5, 4], // y-min (normal -y)
            vec![3, 7, 6, 2], // y-max (normal +y)
            vec![0, 3, 2, 1], // z-min (normal -z)
            vec![4, 5, 6, 7], // z-max (normal +z)
        ];
        let owner = vec![0usize; 6];
        let patches = vec![BoundaryPatch::new("walls", 0, 6, PatchKind::Wall)];
        PolyPatchMesh {
            points,
            faces,
            owner,
            neighbour: Vec::new(),
            n_internal_faces: 0,
            n_cells: 1,
            patches,
        }
    }

    /// Unit-cube geometry: volume 1, centre (0.5,0.5,0.5), each face area 1
    /// with correct outward normal. Measured 2026-07-17: vol = 1.0 (err < 1e-15),
    /// centre error < 1e-15, all six |Sf| = 1.
    #[test]
    fn unit_cube_geometry() {
        let m = unit_cube();
        let (areas, centres) = m.face_geometry();
        for (i, a) in areas.iter().enumerate() {
            assert!((a.mag() - 1.0).abs() < 1e-12, "face {i} area {}", a.mag());
        }
        // Outward normals: -x,+x,-y,+y,-z,+z.
        assert!(areas[0].x < -0.99 && areas[1].x > 0.99);
        assert!(areas[2].y < -0.99 && areas[3].y > 0.99);
        assert!(areas[4].z < -0.99 && areas[5].z > 0.99);

        let (ctrs, vols) = m.cell_geometry(&areas, &centres);
        assert!((vols[0] - 1.0).abs() < 1e-12, "vol {}", vols[0]);
        assert!((ctrs[0] - Vector3::new(0.5, 0.5, 0.5)).mag() < 1e-12);

        let fv = m.build_fvmesh().expect("cube rebuild valid");
        assert_eq!(fv.n_cells, 1);
        assert_eq!(fv.n_faces, 6);
        assert!((fv.cell_volumes[0] - 1.0).abs() < 1e-12);
    }

    /// A perfectly orthogonal cube has ~0 non-orthogonality and low skewness and
    /// passes the default quality limits. Measured 2026-07-17: max_non_ortho =
    /// 0° (no internal faces), max_skew ≈ 0, min_vol = 1.0.
    #[test]
    fn unit_cube_quality_passes() {
        let m = unit_cube();
        let q = m.quality();
        assert_eq!(q.n_negative_volume_cells, 0);
        assert!((q.min_cell_volume - 1.0).abs() < 1e-12);
        assert!(q.max_skewness < 0.5, "skew {}", q.max_skewness);
        assert!(QualityLimits::default().accepts(&q));
    }

    /// A collapsed (zero-height) cell is flagged negative/zero volume and
    /// rejected by the quality gate.
    #[test]
    fn collapsed_cell_rejected() {
        let mut m = unit_cube();
        // Flatten the top face onto the bottom → zero volume.
        for p in [4, 5, 6, 7] {
            m.points[p].z = 0.0;
        }
        let q = m.quality();
        assert!(q.min_cell_volume <= 1e-12, "vol {}", q.min_cell_volume);
        assert!(!QualityLimits::default().accepts(&q));
    }
}
