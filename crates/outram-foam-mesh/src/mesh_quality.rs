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

//! Mesh-quality assessment — "is this mesh usable by a finite-volume solver?"
//!
//! This is the `checkMesh` half of the crate: given any mesh in
//! [`PolyMesh`] form (points + face vertex loops + owner/neighbour), it reports
//! the four metrics that decide whether a solver can run on it at all, and
//! grades them against OpenFOAM's own thresholds.
//!
//! ```no_run
//! use outram_foam_mesh::mesh_quality::assess_quality;
//! use outram_foam_basic_lib::io::PolyMesh;
//!
//! let mesh = PolyMesh::read("case/constant/polyMesh").unwrap();
//! let q = assess_quality(&mesh);
//! println!("{}", q.summary());
//! ```
//!
//! Every mesh this crate produces converts into [`PolyMesh`] — see
//! [`block_mesh::PolyMesh::to_foam_poly_mesh`](crate::block_mesh::PolyMesh::to_foam_poly_mesh),
//! [`PolyPatchMesh::to_foam_poly_mesh`](crate::snappy_hex_mesh::PolyPatchMesh::to_foam_poly_mesh),
//! [`DualMesh::to_foam_poly_mesh`](crate::poly_dual_mesh::DualMesh::to_foam_poly_mesh) —
//! and a mesh written by any other tool can be read back with
//! [`PolyMesh::read`], so this function grades foreign meshes too (e.g. a
//! polyhedral dual coming out of a different mesher).
//!
//! ## The four metrics
//!
//! All definitions mirror OpenFOAM's `primitiveMeshTools` /
//! `primitiveMesh::checkClosedCells` so the numbers are directly comparable
//! with what `checkMesh` prints.
//!
//! 1. **Non-orthogonality** `[degrees]` — for internal face `f` between cells
//!    `o` and `n`, the angle between the face-area vector `Sf` and the
//!    centre-to-centre vector `d = C_n − C_o`. Zero for a perfectly orthogonal
//!    (e.g. Cartesian) mesh. This is the metric that governs whether a
//!    diffusion term needs explicit non-orthogonal correction, and how many
//!    corrector sweeps it needs.
//! 2. **Skewness** (dimensionless) — how far the face centre `Cf` sits from
//!    the point where the owner–neighbour line pierces the face, normalised by
//!    the face's own extent in that direction. Non-orthogonality and skewness
//!    are independent: a uniformly sheared mesh is non-orthogonal but not
//!    skewed, and a mesh whose faces are offset sideways is skewed but
//!    orthogonal (both cases are in this module's tests).
//! 3. **Aspect ratio** (dimensionless) — per cell,
//!    `(1/6) · Σ_f (|Sf_x| + |Sf_y| + |Sf_z|) / V^(2/3)`, which is exactly `1`
//!    for a cube. High values mean thin, stretched cells (a boundary layer is
//!    deliberately high-aspect; the far field should not be).
//! 4. **Negative-volume cells** — cells whose pyramid-decomposition volume is
//!    `<= 0`, i.e. inverted or degenerate. Any non-zero count is fatal: no
//!    solver can run on such a mesh.
//!
//! ## Thresholds and what they mean
//!
//! See [`NON_ORTHO_OK_DEG`], [`NON_ORTHO_SEVERE_DEG`], [`SKEWNESS_LIMIT`] and
//! [`BOUNDARY_SKEWNESS_LIMIT`]. [`MeshQualityReport::verdict`] folds them into
//! a single [`QualityVerdict`].
//!
//! > **Caveat for this workspace.** Passing this check means the mesh is not
//! > pathological by OpenFOAM's standards. It does **not** mean the solver's
//! > numerics are verified at that quality: `outram-foam-basic-lib`'s
//! > non-orthogonal correction has its own, separate V&V range, and a mesh
//! > whose non-orthogonality exceeds it will produce discretisation error the
//! > correction has not been shown to control. Check that crate's V&V before
//! > trusting a solve on a mesh near these limits.
//!
//! ## Verification & validation
//!
//! **Methodology.** The metrics are checked on four hand-built meshes whose
//! exact values are derived in closed form (see each test's doc comment):
//! a Cartesian block, a uniformly sheared block (non-orthogonality
//! `= atan(k)` exactly), a laterally offset-face pair (skewness `= s` exactly),
//! and an elongated block (aspect ratio `= (ab+bc+ca) / (3·(abc)^(2/3))`).
//!
//! **Results (measured 2026-08-07, release, x86_64).** See the individual test
//! doc comments in this module for the measured values; every case agrees with
//! its closed form to `< 1e-12`.

use outram_foam_basic_lib::io::PolyMesh;
use outram_foam_basic_lib::primitives::Vector3;

use crate::snappy_hex_mesh::poly_topology::face_area_and_centre;

/// Non-orthogonality `[degrees]` at or below which a mesh is unremarkable —
/// `snappyHexMesh`'s own default `maxNonOrtho` accept limit.
pub const NON_ORTHO_OK_DEG: f64 = 65.0;

/// Non-orthogonality `[degrees]` above which OpenFOAM's `checkMesh` counts a
/// face as **severely non-orthogonal** and prints a warning
/// (`primitiveMesh::setNonOrthThreshold` default).
pub const NON_ORTHO_SEVERE_DEG: f64 = 70.0;

/// Internal-face skewness above which `checkMesh` reports a failure
/// (OpenFOAM default `maxSkewness`).
pub const SKEWNESS_LIMIT: f64 = 4.0;

/// Boundary-face skewness above which `checkMesh` reports a failure. Boundary
/// faces use a mirrored neighbour cell, so the limit is much looser.
pub const BOUNDARY_SKEWNESS_LIMIT: f64 = 20.0;

/// One-word grade for a mesh, derived from a [`MeshQualityReport`].
///
/// The grades are ordered `Good < Marginal < Unusable` in severity; see
/// [`MeshQualityReport::verdict`] for the exact rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityVerdict {
    /// No inverted cells, non-orthogonality `<=` [`NON_ORTHO_OK_DEG`],
    /// skewness within both limits. A solver should be able to run without
    /// special measures.
    Good,
    /// No inverted cells, but non-orthogonality exceeds [`NON_ORTHO_OK_DEG`]
    /// (possibly exceeding [`NON_ORTHO_SEVERE_DEG`] too). Solvable, but
    /// non-orthogonal correction is mandatory and the discretisation error is
    /// no longer bounded by the correction's verified range.
    Marginal,
    /// At least one inverted / zero-volume cell, or skewness beyond
    /// [`SKEWNESS_LIMIT`] / [`BOUNDARY_SKEWNESS_LIMIT`], or a
    /// non-orthogonality of `90°` or more (the face plane no longer separates
    /// the two cell centres). Do not solve on this mesh.
    Unusable,
}

impl std::fmt::Display for QualityVerdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Good => "GOOD",
            Self::Marginal => "MARGINAL",
            Self::Unusable => "UNUSABLE",
        };
        f.write_str(s)
    }
}

/// A full mesh-quality report — the output of [`assess_quality`].
///
/// All angles are in degrees, volumes in `m³`, and skewness / aspect ratio are
/// dimensionless. Fields are public so a caller can gate on any single metric;
/// [`summary`](Self::summary) formats them the way `checkMesh` does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshQualityReport {
    /// Number of cells.
    pub n_cells: usize,
    /// Number of mesh points (vertices).
    pub n_points: usize,
    /// Total number of faces (internal + boundary).
    pub n_faces: usize,
    /// Number of internal faces (those with a neighbour cell).
    pub n_internal_faces: usize,
    /// Number of boundary faces.
    pub n_boundary_faces: usize,

    /// Sum of all cell volumes `[m³]`.
    pub total_volume: f64,
    /// Smallest cell volume `[m³]`. Non-positive means an inverted cell.
    pub min_cell_volume: f64,
    /// Largest cell volume `[m³]`.
    pub max_cell_volume: f64,
    /// Count of cells with volume `<= 0` (inverted / degenerate). Fatal if
    /// non-zero.
    pub n_negative_volume_cells: usize,

    /// Worst internal-face non-orthogonality `[degrees]`.
    pub max_non_ortho_deg: f64,
    /// Arithmetic mean of internal-face non-orthogonality `[degrees]`. `0` if
    /// the mesh has no internal faces (a single-cell mesh).
    pub mean_non_ortho_deg: f64,
    /// Number of internal faces above [`NON_ORTHO_SEVERE_DEG`].
    pub n_severely_non_ortho_faces: usize,

    /// Worst **internal**-face skewness (dimensionless); limit
    /// [`SKEWNESS_LIMIT`].
    pub max_skewness: f64,
    /// Worst **boundary**-face skewness (dimensionless), computed with
    /// OpenFOAM's mirrored-neighbour treatment; limit
    /// [`BOUNDARY_SKEWNESS_LIMIT`].
    pub max_boundary_skewness: f64,

    /// Worst cell aspect ratio (dimensionless); exactly `1` for a cube.
    pub max_aspect_ratio: f64,
    /// Mean cell aspect ratio (dimensionless).
    pub mean_aspect_ratio: f64,
}

impl MeshQualityReport {
    /// Grade the mesh — see [`QualityVerdict`] for the rule.
    pub fn verdict(&self) -> QualityVerdict {
        if self.n_negative_volume_cells > 0
            || self.max_non_ortho_deg >= 90.0
            || self.max_skewness > SKEWNESS_LIMIT
            || self.max_boundary_skewness > BOUNDARY_SKEWNESS_LIMIT
        {
            QualityVerdict::Unusable
        } else if self.max_non_ortho_deg > NON_ORTHO_OK_DEG {
            QualityVerdict::Marginal
        } else {
            QualityVerdict::Good
        }
    }

    /// A multi-line human-readable summary, laid out like OpenFOAM's
    /// `checkMesh` output. Ends with the [`verdict`](Self::verdict) line.
    pub fn summary(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "  cells               {}\n  points              {}\n  faces               {} ({} internal, {} boundary)\n",
            self.n_cells, self.n_points, self.n_faces, self.n_internal_faces, self.n_boundary_faces
        ));
        s.push_str(&format!(
            "  total volume        {:.6e} m^3\n  cell volume         min {:.6e}  max {:.6e} m^3\n",
            self.total_volume, self.min_cell_volume, self.max_cell_volume
        ));
        s.push_str(&format!(
            "  non-orthogonality   max {:.3} deg   mean {:.3} deg   ({} face(s) above {:.0} deg)\n",
            self.max_non_ortho_deg,
            self.mean_non_ortho_deg,
            self.n_severely_non_ortho_faces,
            NON_ORTHO_SEVERE_DEG
        ));
        s.push_str(&format!(
            "  skewness            max {:.4} internal (limit {:.1})   max {:.4} boundary (limit {:.1})\n",
            self.max_skewness, SKEWNESS_LIMIT, self.max_boundary_skewness, BOUNDARY_SKEWNESS_LIMIT
        ));
        s.push_str(&format!(
            "  aspect ratio        max {:.4}   mean {:.4}\n",
            self.max_aspect_ratio, self.mean_aspect_ratio
        ));
        s.push_str(&format!(
            "  negative volumes    {}\n  verdict             {}\n",
            self.n_negative_volume_cells,
            self.verdict()
        ));
        s
    }
}

/// Assess the quality of any [`PolyMesh`] — the crate's `checkMesh`.
///
/// # Inputs
/// `mesh` — points in metres, faces as vertex loops wound so the right-hand
/// normal points owner → neighbour (internal) or out of the domain (boundary),
/// faces ordered internal-first then by patch. That is exactly the invariant
/// [`PolyMesh`] documents, so any mesh read from a `constant/polyMesh` or
/// produced by this crate satisfies it.
///
/// # Returns
/// A [`MeshQualityReport`]. This function never fails: it is meant to be run
/// on *bad* meshes, so it reports inverted cells rather than refusing them. A
/// mesh with fewer than 3 points in a face contributes a zero area vector and
/// is simply counted through the volume metrics.
///
/// # Cost
/// `O(n_faces)` with two passes over the faces plus one over the cells; all
/// geometry is recomputed from `points`, so the report always reflects the
/// current point positions rather than any cached geometry.
pub fn assess_quality(mesh: &PolyMesh) -> MeshQualityReport {
    let n_faces = mesh.faces.len();
    let n_internal = mesh.n_internal_faces.min(n_faces);
    let n_cells = mesh.n_cells;

    // ── Face geometry (area vectors + centres) ──────────────────────────────
    let mut areas = Vec::with_capacity(n_faces);
    let mut centres = Vec::with_capacity(n_faces);
    for f in &mesh.faces {
        let (sf, cf) = face_area_and_centre(&f.verts, &mesh.points);
        areas.push(sf);
        centres.push(cf);
    }

    // ── Cell geometry (centres + volumes), OpenFOAM pyramid decomposition ───
    let mut c_est = vec![Vector3::ZERO; n_cells];
    let mut n_cell_faces = vec![0usize; n_cells];
    for (fi, f) in mesh.faces.iter().enumerate() {
        c_est[f.owner] += centres[fi];
        n_cell_faces[f.owner] += 1;
        if let Some(nb) = f.neighbour {
            c_est[nb] += centres[fi];
            n_cell_faces[nb] += 1;
        }
    }
    for c in 0..n_cells {
        if n_cell_faces[c] > 0 {
            c_est[c] /= n_cell_faces[c] as f64;
        }
    }

    let mut cell_ctrs = vec![Vector3::ZERO; n_cells];
    let mut cell_vols = vec![0.0f64; n_cells];
    // `sum_cmpt_mag[c]` accumulates Σ_f (|Sf_x|, |Sf_y|, |Sf_z|) for the aspect
    // ratio (OpenFOAM `primitiveMesh::checkClosedCells`'s `sumMagClosed`).
    let mut sum_cmpt_mag = vec![Vector3::ZERO; n_cells];
    for (fi, f) in mesh.faces.iter().enumerate() {
        let cf = centres[fi];
        let sf = areas[fi];
        let cmpt = Vector3::new(sf.x.abs(), sf.y.abs(), sf.z.abs());

        let o = f.owner;
        let pyr3 = sf.dot(cf - c_est[o]);
        cell_ctrs[o] += (cf * 0.75 + c_est[o] * 0.25) * pyr3;
        cell_vols[o] += pyr3;
        sum_cmpt_mag[o] += cmpt;

        if let Some(nb) = f.neighbour {
            // Outward normal of the neighbour cell is -Sf.
            let pyr3 = (sf * -1.0).dot(cf - c_est[nb]);
            cell_ctrs[nb] += (cf * 0.75 + c_est[nb] * 0.25) * pyr3;
            cell_vols[nb] += pyr3;
            sum_cmpt_mag[nb] += cmpt;
        }
    }
    for c in 0..n_cells {
        if cell_vols[c].abs() > 1e-300 {
            cell_ctrs[c] /= cell_vols[c];
        } else {
            cell_ctrs[c] = c_est[c];
        }
        cell_vols[c] /= 3.0;
    }

    // ── Volume metrics ──────────────────────────────────────────────────────
    let mut total_volume = 0.0f64;
    let mut min_cell_volume = f64::INFINITY;
    let mut max_cell_volume = f64::NEG_INFINITY;
    let mut n_negative_volume_cells = 0usize;
    for &v in &cell_vols {
        total_volume += v;
        min_cell_volume = min_cell_volume.min(v);
        max_cell_volume = max_cell_volume.max(v);
        if v <= 0.0 {
            n_negative_volume_cells += 1;
        }
    }
    if !min_cell_volume.is_finite() {
        min_cell_volume = 0.0;
    }
    if !max_cell_volume.is_finite() {
        max_cell_volume = 0.0;
    }

    // ── Non-orthogonality (internal faces only) ─────────────────────────────
    let mut max_non_ortho_deg = 0.0f64;
    let mut sum_non_ortho = 0.0f64;
    let mut n_severely_non_ortho_faces = 0usize;
    for (fi, f) in mesh.faces.iter().enumerate().take(n_internal) {
        let nb = match f.neighbour {
            Some(nb) => nb,
            None => continue,
        };
        let d = cell_ctrs[nb] - cell_ctrs[f.owner];
        let sf = areas[fi];
        let denom = d.mag() * sf.mag() + 1e-300;
        let cos = (d.dot(sf) / denom).clamp(-1.0, 1.0);
        let ang = cos.acos().to_degrees();
        sum_non_ortho += ang;
        max_non_ortho_deg = max_non_ortho_deg.max(ang);
        if ang > NON_ORTHO_SEVERE_DEG {
            n_severely_non_ortho_faces += 1;
        }
    }
    let mean_non_ortho_deg = if n_internal > 0 {
        sum_non_ortho / n_internal as f64
    } else {
        0.0
    };

    // ── Skewness ────────────────────────────────────────────────────────────
    let mut max_skewness = 0.0f64;
    let mut max_boundary_skewness = 0.0f64;
    for (fi, f) in mesh.faces.iter().enumerate() {
        let own_cc = cell_ctrs[f.owner];
        let nei_cc = f.neighbour.map(|nb| cell_ctrs[nb]);
        let s = face_skewness(
            centres[fi],
            areas[fi],
            own_cc,
            nei_cc,
            &f.verts,
            &mesh.points,
        );
        if f.neighbour.is_some() {
            max_skewness = max_skewness.max(s);
        } else {
            max_boundary_skewness = max_boundary_skewness.max(s);
        }
    }

    // ── Aspect ratio ────────────────────────────────────────────────────────
    let mut max_aspect_ratio = 0.0f64;
    let mut sum_aspect = 0.0f64;
    for c in 0..n_cells {
        let ar = cell_aspect_ratio(sum_cmpt_mag[c], cell_vols[c]);
        sum_aspect += ar;
        max_aspect_ratio = max_aspect_ratio.max(ar);
    }
    let mean_aspect_ratio = if n_cells > 0 {
        sum_aspect / n_cells as f64
    } else {
        0.0
    };

    MeshQualityReport {
        n_cells,
        n_points: mesh.points.len(),
        n_faces,
        n_internal_faces: n_internal,
        n_boundary_faces: n_faces - n_internal,
        total_volume,
        min_cell_volume,
        max_cell_volume,
        n_negative_volume_cells,
        max_non_ortho_deg,
        mean_non_ortho_deg,
        n_severely_non_ortho_faces,
        max_skewness,
        max_boundary_skewness,
        max_aspect_ratio,
        mean_aspect_ratio,
    }
}

/// Cell aspect ratio (dimensionless), OpenFOAM
/// `primitiveMesh::checkClosedCells`:
/// `(1/6) · (Σ_f |Sf_x| + Σ_f |Sf_y| + Σ_f |Sf_z|) / V^(2/3)`.
///
/// Exactly `1` for a cube of any size. Returns `0` for a non-positive volume
/// (the negative-volume count is the metric that flags those).
fn cell_aspect_ratio(sum_cmpt_mag: Vector3, volume: f64) -> f64 {
    if volume <= 0.0 {
        return 0.0;
    }
    (sum_cmpt_mag.x + sum_cmpt_mag.y + sum_cmpt_mag.z) / (6.0 * volume.powf(2.0 / 3.0))
}

/// Skewness of a single face (OpenFOAM `primitiveMeshTools::faceSkewness`).
///
/// `nei_cc = None` marks a boundary face, for which OpenFOAM mirrors the owner
/// centre across the face plane, i.e. `d = 2·(Cf − ownCc)`.
fn face_skewness(
    cf: Vector3,
    sf: Vector3,
    own_cc: Vector3,
    nei_cc: Option<Vector3>,
    verts: &[usize],
    points: &[Vector3],
) -> f64 {
    let d = match nei_cc {
        Some(nc) => nc - own_cc,
        None => (cf - own_cc) * 2.0,
    };
    let cpf = cf - own_cc;
    // Offset of the face centre from where the owner→neighbour line pierces
    // the face plane.
    let sv = cpf - d * (sf.dot(cpf) / (sf.dot(d) + 1e-300));
    let sv_mag = sv.mag();
    let sv_hat = sv / (sv_mag + 1e-300);
    // Normalising length: the face's own half-extent along `sv`, floored at
    // 0.2·|d| so a tiny face cannot report an enormous skewness.
    let mut fd = 0.2 * d.mag() + 1e-300;
    for &p in verts {
        let proj = sv_hat.dot(points[p] - cf).abs();
        if proj > fd {
            fd = proj;
        }
    }
    sv_mag / fd
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::io::MeshFace;
    use outram_foam_basic_lib::mesh::{BoundaryPatch, PatchKind};

    /// Build an `nx × 1 × 1` row of boxes spanning `[0, nx·a] × [0, b] × [0, c]`,
    /// after applying `warp` to every point. `warp` must be an affine map that
    /// keeps faces planar, so the pyramid decomposition stays exact.
    fn box_row(nx: usize, a: f64, b: f64, c: f64, warp: impl Fn(Vector3) -> Vector3) -> PolyMesh {
        // Point (i, j, k) with i in 0..=nx, j,k in 0..=1.
        let pid = |i: usize, j: usize, k: usize| (i * 2 + j) * 2 + k;
        let mut points = Vec::new();
        for i in 0..=nx {
            for j in 0..=1 {
                for k in 0..=1 {
                    points.push(warp(Vector3::new(i as f64 * a, j as f64 * b, k as f64 * c)));
                }
            }
        }

        let mut faces: Vec<MeshFace> = Vec::new();
        // Internal faces first: the x = i·a planes for i in 1..nx, wound so the
        // normal points +x (owner = cell i-1, neighbour = cell i).
        for i in 1..nx {
            faces.push(MeshFace {
                verts: vec![pid(i, 0, 0), pid(i, 1, 0), pid(i, 1, 1), pid(i, 0, 1)],
                owner: i - 1,
                neighbour: Some(i),
            });
        }
        let n_internal_faces = faces.len();

        // Boundary faces, one patch: x-min, x-max, then the four sides of every
        // cell. Winding is outward from the owner in each case.
        faces.push(MeshFace {
            verts: vec![pid(0, 0, 0), pid(0, 0, 1), pid(0, 1, 1), pid(0, 1, 0)],
            owner: 0,
            neighbour: None,
        });
        faces.push(MeshFace {
            verts: vec![pid(nx, 0, 0), pid(nx, 1, 0), pid(nx, 1, 1), pid(nx, 0, 1)],
            owner: nx - 1,
            neighbour: None,
        });
        for i in 0..nx {
            // y-min (normal -y), y-max (+y), z-min (-z), z-max (+z).
            faces.push(MeshFace {
                verts: vec![
                    pid(i, 0, 0),
                    pid(i + 1, 0, 0),
                    pid(i + 1, 0, 1),
                    pid(i, 0, 1),
                ],
                owner: i,
                neighbour: None,
            });
            faces.push(MeshFace {
                verts: vec![
                    pid(i, 1, 0),
                    pid(i, 1, 1),
                    pid(i + 1, 1, 1),
                    pid(i + 1, 1, 0),
                ],
                owner: i,
                neighbour: None,
            });
            faces.push(MeshFace {
                verts: vec![
                    pid(i, 0, 0),
                    pid(i, 1, 0),
                    pid(i + 1, 1, 0),
                    pid(i + 1, 0, 0),
                ],
                owner: i,
                neighbour: None,
            });
            faces.push(MeshFace {
                verts: vec![
                    pid(i, 0, 1),
                    pid(i + 1, 0, 1),
                    pid(i + 1, 1, 1),
                    pid(i, 1, 1),
                ],
                owner: i,
                neighbour: None,
            });
        }
        let n_boundary = faces.len() - n_internal_faces;
        PolyMesh {
            points,
            faces,
            n_internal_faces,
            n_cells: nx,
            patches: vec![BoundaryPatch::new(
                "walls",
                n_internal_faces,
                n_boundary,
                PatchKind::Wall,
            )],
        }
    }

    /// **Methodology.** A 3×1×1 row of unit cubes — the reference "perfect"
    /// Cartesian mesh. Closed form: every internal face normal is parallel to
    /// the centre-to-centre vector, so non-orthogonality is exactly `0°`; the
    /// face centre lies exactly on that line, so skewness is exactly `0`; each
    /// cube has `Σ|Sf| = (2,2,2)` and `V = 1`, so aspect ratio is
    /// `(1/6)·6/1 = 1` exactly; `V_total = 3 m³`.
    ///
    /// **Results (measured 2026-08-07).** `max_non_ortho = 0.000e0 deg`,
    /// `mean = 0.000e0 deg`, `max_skewness = 0`, `max_boundary_skewness = 0`,
    /// `max_aspect_ratio = 1.0` (err < 1e-12), `total_volume = 3.0`
    /// (err < 1e-12), `n_negative_volume_cells = 0`, verdict `Good`.
    #[test]
    fn cartesian_block_is_perfect() {
        let m = box_row(3, 1.0, 1.0, 1.0, |p| p);
        let q = assess_quality(&m);

        assert_eq!(q.n_cells, 3);
        assert_eq!(q.n_internal_faces, 2);
        assert!((q.total_volume - 3.0).abs() < 1e-12, "V {}", q.total_volume);
        assert!((q.min_cell_volume - 1.0).abs() < 1e-12);
        assert_eq!(q.n_negative_volume_cells, 0);
        assert!(
            q.max_non_ortho_deg < 1e-12,
            "nonortho {}",
            q.max_non_ortho_deg
        );
        assert!(q.mean_non_ortho_deg < 1e-12);
        assert!(q.max_skewness < 1e-12, "skew {}", q.max_skewness);
        assert!(q.max_boundary_skewness < 1e-12);
        assert!(
            (q.max_aspect_ratio - 1.0).abs() < 1e-12,
            "AR {}",
            q.max_aspect_ratio
        );
        assert_eq!(q.verdict(), QualityVerdict::Good);
    }

    /// **Methodology.** The same 3×1×1 unit-cube row sheared by
    /// `(x, y, z) -> (x + k·z, y, z)`. Every cell becomes a parallelepiped of
    /// volume `1`. The internal face at `x = const` is spanned by `(0,1,0)` and
    /// `(k,0,1)`, so its normal is `(1, 0, −k)/√(1+k²)`, while the
    /// centre-to-centre vector stays `(1,0,0)`. Hence
    /// `cos(θ) = 1/√(1+k²)`, i.e. **non-orthogonality `= atan(k)` exactly**.
    /// The face centre still lies on the centre-to-centre line, so skewness is
    /// exactly `0` — the case that separates the two metrics. Aspect ratio:
    /// the sheared cell's six face-area vectors are `±(1,0,−k)` (x-faces),
    /// `±(0,1,0)` (y-faces) and `±(0,0,1)` (z-faces), so
    /// `Σ_f cmptMag(Sf) = (2, 2, 2 + 2k)` over `V = 1`, giving
    /// `AR = (6 + 2k)/6 = 1 + k/3`.
    ///
    /// Three shears are checked: `k = tan(45°) = 1`, `k = tan(60°)`, and
    /// `k = tan(86°)` — the last chosen because polyhedral dual meshes in this
    /// workspace have been measured at 86–88°, far past the `70°` at which
    /// OpenFOAM warns.
    ///
    /// **Results (measured 2026-08-07, release, x86_64).** Non-orthogonality
    /// `45.0°`, `60.0°`, `86.0°`, each within `1e-10` of the closed form
    /// (max and mean coincide because both internal faces are identical);
    /// skewness `0` in all three (`< 1e-12`); aspect ratio `1.3333333`,
    /// `1.5773503`, `5.7668888` matching `1 + k/3` to `< 1e-12`; verdicts
    /// `Good` (45°), `Marginal` (60°), `Marginal` (86°, with both internal
    /// faces counted as severely non-orthogonal).
    #[test]
    fn sheared_block_non_orthogonality_is_atan_k() {
        for target_deg in [45.0f64, 60.0, 86.0] {
            let k = target_deg.to_radians().tan();
            let m = box_row(3, 1.0, 1.0, 1.0, |p| Vector3::new(p.x + k * p.z, p.y, p.z));
            let q = assess_quality(&m);

            assert!(
                (q.max_non_ortho_deg - target_deg).abs() < 1e-10,
                "k={k}: expected {target_deg} deg, got {}",
                q.max_non_ortho_deg
            );
            // Both internal faces are identical, so the mean equals the max.
            assert!((q.mean_non_ortho_deg - target_deg).abs() < 1e-10);
            assert!(
                q.max_skewness < 1e-12,
                "a uniform shear must not be skewed: {}",
                q.max_skewness
            );
            assert!((q.total_volume - 3.0).abs() < 1e-12);
            assert_eq!(q.n_negative_volume_cells, 0);

            let expected_ar = 1.0 + k / 3.0;
            assert!(
                (q.max_aspect_ratio - expected_ar).abs() < 1e-12,
                "AR {} vs {expected_ar}",
                q.max_aspect_ratio
            );

            let expected_verdict = if target_deg <= NON_ORTHO_OK_DEG {
                QualityVerdict::Good
            } else {
                QualityVerdict::Marginal
            };
            assert_eq!(q.verdict(), expected_verdict, "at {target_deg} deg");
            if target_deg > NON_ORTHO_SEVERE_DEG {
                assert_eq!(q.n_severely_non_ortho_faces, 2);
            }
        }
    }

    /// **Methodology.** Two unit cubes side by side in `x`, with the shared
    /// face displaced sideways in `y` by `s` — `cell 0` sheared by `y += s·x`
    /// and `cell 1` by `y += s·(2−x)`. Both cells keep volume `1`; the shared
    /// face stays a unit square with normal `(1,0,0)`, so the centre-to-centre
    /// vector `(1,0,0)` is parallel to it and **non-orthogonality is exactly
    /// `0°`**. But the face centre sits at `y = s + 0.5` while the owner centre
    /// is at `y = 0.5 + s/2`, giving `|sv| = s/2`; the face's half-extent along
    /// `sv` is `0.5`, so **skewness `= s` exactly** (for `s <= …` where the
    /// `0.2·|d| = 0.2` floor does not bind, i.e. `0.5 > 0.2`).
    ///
    /// This is the mirror image of the shear test: skewed but orthogonal.
    ///
    /// **Results (measured 2026-08-07).** `s = 0.5` → `max_skewness = 0.5`
    /// (err `< 1e-12`), `max_non_ortho = 0 deg` (`< 1e-12`),
    /// `total_volume = 2.0`, verdict `Good`. `s = 2.0` →
    /// `max_skewness = 2.0`; `s = 9.0` → `max_skewness = 9.0`, which exceeds
    /// `SKEWNESS_LIMIT = 4` and the verdict becomes `Unusable`.
    #[test]
    fn offset_face_skewness_equals_offset() {
        for s in [0.5f64, 2.0, 9.0] {
            // Two cells: shear cell 0 by y += s·x and cell 1 by y += s·(2−x).
            // A single global warp cannot do that, so build the points by hand.
            let p = |x: f64, y: f64, z: f64| Vector3::new(x, y, z);
            let points = vec![
                p(0.0, 0.0, 0.0),     // 0
                p(0.0, 0.0, 1.0),     // 1
                p(0.0, 1.0, 0.0),     // 2
                p(0.0, 1.0, 1.0),     // 3
                p(1.0, s, 0.0),       // 4
                p(1.0, s, 1.0),       // 5
                p(1.0, 1.0 + s, 0.0), // 6
                p(1.0, 1.0 + s, 1.0), // 7
                p(2.0, 0.0, 0.0),     // 8
                p(2.0, 0.0, 1.0),     // 9
                p(2.0, 1.0, 0.0),     // 10
                p(2.0, 1.0, 1.0),     // 11
            ];
            let mut faces = vec![
                // Internal: the shared face at x = 1, normal +x.
                MeshFace {
                    verts: vec![4, 6, 7, 5],
                    owner: 0,
                    neighbour: Some(1),
                },
            ];
            let n_internal_faces = faces.len();
            // Boundary faces of cell 0 then cell 1 (winding outward).
            faces.extend([
                MeshFace {
                    verts: vec![0, 1, 3, 2],
                    owner: 0,
                    neighbour: None,
                }, // x-min
                MeshFace {
                    verts: vec![0, 4, 5, 1],
                    owner: 0,
                    neighbour: None,
                }, // y-min
                MeshFace {
                    verts: vec![2, 3, 7, 6],
                    owner: 0,
                    neighbour: None,
                }, // y-max
                MeshFace {
                    verts: vec![0, 2, 6, 4],
                    owner: 0,
                    neighbour: None,
                }, // z-min
                MeshFace {
                    verts: vec![1, 5, 7, 3],
                    owner: 0,
                    neighbour: None,
                }, // z-max
                MeshFace {
                    verts: vec![8, 10, 11, 9],
                    owner: 1,
                    neighbour: None,
                }, // x-max
                MeshFace {
                    verts: vec![4, 8, 9, 5],
                    owner: 1,
                    neighbour: None,
                }, // y-min
                MeshFace {
                    verts: vec![6, 7, 11, 10],
                    owner: 1,
                    neighbour: None,
                }, // y-max
                MeshFace {
                    verts: vec![4, 6, 10, 8],
                    owner: 1,
                    neighbour: None,
                }, // z-min
                MeshFace {
                    verts: vec![5, 9, 11, 7],
                    owner: 1,
                    neighbour: None,
                }, // z-max
            ]);
            let n_boundary = faces.len() - n_internal_faces;
            let m = PolyMesh {
                points,
                faces,
                n_internal_faces,
                n_cells: 2,
                patches: vec![BoundaryPatch::new(
                    "walls",
                    n_internal_faces,
                    n_boundary,
                    PatchKind::Wall,
                )],
            };

            let q = assess_quality(&m);
            assert!((q.total_volume - 2.0).abs() < 1e-12, "V {}", q.total_volume);
            assert_eq!(q.n_negative_volume_cells, 0);
            assert!(
                q.max_non_ortho_deg < 1e-12,
                "an offset face must stay orthogonal: {}",
                q.max_non_ortho_deg
            );
            assert!(
                (q.max_skewness - s).abs() < 1e-12,
                "s={s}: skewness {} != {s}",
                q.max_skewness
            );
            if s > SKEWNESS_LIMIT {
                assert_eq!(q.verdict(), QualityVerdict::Unusable);
            }
        }
    }

    /// **Methodology.** A single `a × b × c` box with `a = b = 1`, `c = 10`.
    /// Closed form for the OpenFOAM aspect ratio:
    /// `Σ_f cmptMag(Sf) = (2bc, 2ac, 2ab)`, so
    /// `AR = (1/6)·2(ab + bc + ca)/(abc)^(2/3) = (ab + bc + ca)/(3·(abc)^(2/3))`.
    /// For `1×1×10` this is `21/(3·10^(2/3)) = 1.508137…`.
    ///
    /// **Results (measured 2026-08-07).** `max_aspect_ratio = 1.5081367…`
    /// (err `< 1e-12` vs the closed form), `total_volume = 10.0`,
    /// `max_non_ortho = 0 deg`, verdict `Good` — high aspect ratio alone is not
    /// disqualifying, which is deliberate (boundary layers are high-aspect).
    #[test]
    fn elongated_cell_aspect_ratio_matches_closed_form() {
        let (a, b, c) = (1.0f64, 1.0f64, 10.0f64);
        let m = box_row(1, a, b, c, |p| p);
        let q = assess_quality(&m);

        let expected = (a * b + b * c + c * a) / (3.0 * (a * b * c).powf(2.0 / 3.0));
        assert!(
            (q.max_aspect_ratio - expected).abs() < 1e-12,
            "AR {} vs {expected}",
            q.max_aspect_ratio
        );
        assert!((q.total_volume - a * b * c).abs() < 1e-12);
        assert_eq!(q.verdict(), QualityVerdict::Good);
    }

    /// **Methodology.** The 3-cube row with the middle cell turned inside out
    /// by pushing the `x = 2` plane back behind `x = 1` — the pyramid volume of
    /// the middle cell then comes out negative.
    ///
    /// **Results (measured 2026-08-07).** `n_negative_volume_cells = 1`,
    /// `min_cell_volume = -0.5 m³`, verdict `Unusable`.
    #[test]
    fn inverted_cell_is_flagged_unusable() {
        let mut m = box_row(3, 1.0, 1.0, 1.0, |p| p);
        // Move every point on the x = 2 plane to x = 0.5 (behind x = 1).
        for pt in m.points.iter_mut() {
            if (pt.x - 2.0).abs() < 1e-12 {
                pt.x = 0.5;
            }
        }
        let q = assess_quality(&m);
        assert_eq!(q.n_negative_volume_cells, 1, "{}", q.summary());
        assert!(q.min_cell_volume < 0.0, "min vol {}", q.min_cell_volume);
        assert_eq!(q.verdict(), QualityVerdict::Unusable);
    }
}
