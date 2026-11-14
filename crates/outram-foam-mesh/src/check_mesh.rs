// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Algorithm reference (re-implemented in Rust, not transcribed):
//   OpenFOAM-dev (OpenFOAM Foundation), commit 852527377c7e87e33d51b19ca8974b5db60af28b
//   src/meshCheck/checkMesh.C, src/meshCheck/polyMeshCheck/polyMeshCheckQuality.C,
//   src/meshCheck/primitiveMeshCheck/primitiveMeshCheck.C
//   (`meshCheck::checkMesh`, `faceSkewness`, `boundaryFaceSkewness`,
//   `faceOrthogonality`, `cellDeterminant`, `faceWeights`, `volRatio`) and
//   etc/caseDicts/mesh/generation/meshQualityDict.
//   Copyright (C) 2011-2025 OpenFOAM Foundation. Licence: GPL-3.0-or-later.
// Code lineage: moved here from `outram-park-fork-cfmesh` `src/checks.rs`
//   (gh:#1215, gh:#1202; cfMesh https://github.com/wyldckat/cfMesh,
//   Copyright (C) 2014-2017 Creative Fields, Ltd., GPL-3.0-only), which was
//   the most complete of the three copies of these checks; its behaviour is
//   the reference (gh:#1480).
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

//! **The** `checkMesh` of this workspace (gh:#1480): OpenFOAM's mesh-quality
//! checks, once.
//!
//! Before gh:#1480 these checks existed three times: `outram-park-fork-cfmesh`
//! `checks.rs` (the most complete, and the reference followed here),
//! [`crate::mesh_quality::assess_quality`] and
//! [`PolyPatchMesh::quality`](crate::snappy_hex_mesh::PolyPatchMesh::quality).
//! All three now call this module:
//!
//! - [`check_quality`] -> [`QualityReport`]: every metric (non-orthogonality,
//!   internal and boundary skewness, aspect ratio, volumes, face pyramids,
//!   cell determinant);
//! - [`faces_failing_mesh_quality`]: the per-face `meshQualityControls` gate
//!   ([`MeshQualityControls`], defaults = `meshQualityDict`).
//!
//! ## What is shared and what is not
//!
//! The *checks* are shared. The *geometry* they read is an input,
//! [`CheckGeometry`] (face area vectors and centres, cell centres and
//! volumes), because the three callers' meshes deliberately keep different
//! geometry: cfMesh uses vertex-average face centres, `outram-foam-mesh` uses
//! OpenFOAM's area-weighted ones (and sums the pyramids in different orders).
//! Unifying the geometry would move every pinned cfMesh mesh, so it is **not**
//! done here; it is listed as remaining work on gh:#1480.
//!
//! Where the three copies differed, **each difference is an explicit option**
//! ([`CheckOptions`]: [`BoundarySkewness`], [`SvHat`], the skewness guard,
//! [`DegenerateFaces`], and the aspect ratio reported in both forms), and every
//! caller passes the values that reproduce what it computed before, so no
//! number moved (maintainer decision, gh:#1480; bit-identity measured in
//! `tests/check_mesh_consolidation.rs` and the cfMesh pins). Which value
//! matches upstream dev `852527377c7e` is documented on each option; cfMesh
//! ([`CheckOptions::default`]) deviates in `svHat`, the report's guard,
//! degenerate-face skipping and its aspect-ratio field; `outram-foam-mesh`
//! ([`CheckOptions::foam_mesh_legacy`]) deviates in the boundary-skewness
//! definition (its mirrored form is identically zero, so the snapping gate
//! never sees a boundary skew). Whether callers should move to upstream's
//! definitions is a maintainer decision, filed as gh:#1486.
//!
//! Pure Rust, wasm- and Android-safe.

use outram_foam_basic_lib::primitives::Vector3;
use petir::mathf::RealMath;

use crate::snappy_hex_mesh::esi_snap::{self, SnapMesh};

/// OpenFOAM's `rootVSmall` (`1e-150`), the guard in `faceSkewness` and
/// friends.
pub const ROOT_V_SMALL: f64 = 1e-150;

/// Which definition of boundary-face skewness [`check_quality`] reports in
/// [`QualityReport::max_boundary_skewness`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundarySkewness {
    /// OpenFOAM-dev `meshCheck::boundaryFaceSkewness`: `d` is the normal
    /// projection of `Cf - C_own`, floor `0.4 |d|`. Used by cfMesh's gate.
    NormalProjection,
    /// The owner centre mirrored across the face, `d = 2 (Cf - C_own)`,
    /// floor `0.2 |d|`: what `outram-foam-mesh` computed before gh:#1480.
    Mirrored,
}

/// How `svHat = sv / (|sv| + guard)` is evaluated in [`face_skewness`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvHat {
    /// `sv / (|sv| + guard)`: **what upstream does** (`meshCheck::faceSkewness`)
    /// and what `outram-foam-mesh` did before gh:#1480.
    Divide,
    /// `sv * (1 / (|sv| + guard))`: what cfMesh did; differs from
    /// [`SvHat::Divide`] in the last bit.
    ReciprocalMultiply,
}

/// What [`check_quality`] does with a degenerate internal face
/// (`|d| <= 1e-30` or `|Sf| <= 1e-30`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegenerateFaces {
    /// Skip it in non-orthogonality and skewness (cfMesh's behaviour).
    Skip,
    /// Evaluate it with the `+1e-300` denominators, so a zero `d` reads 90
    /// degrees (`outram-foam-mesh`'s behaviour; upstream's `faceOrthogonality`
    /// has no skip either).
    Include,
}

/// Options of [`check_quality`]. Every difference between the three
/// pre-gh:#1480 copies is one explicit option here; each caller passes the
/// values that reproduce what it computed before, so no number moved. Which
/// value matches upstream dev `852527377c7e` is stated on each variant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CheckOptions {
    /// Boundary skewness definition; see [`BoundarySkewness`].
    pub boundary_skewness: BoundarySkewness,
    /// Non-orthogonality `[degrees]` above which a face counts as severely
    /// non-orthogonal (`checkMesh`'s `primitiveMesh::nonOrthThreshold`, 70).
    pub severe_non_ortho_deg: f64,
    /// `svHat` evaluation; see [`SvHat`].
    pub sv_hat: SvHat,
    /// The guard added to the skewness denominators (upstream `rootVSmall`
    /// `1e-150`; cfMesh's report and `outram-foam-mesh` used `1e-300`).
    pub skew_guard: f64,
    /// Degenerate-face handling; see [`DegenerateFaces`].
    pub degenerate_faces: DegenerateFaces,
    /// Compute the face-pyramid and cell-determinant sliver metrics
    /// (`false` leaves them `0`; the snapping loop does not read them).
    pub sliver_metrics: bool,
}

impl Default for CheckOptions {
    /// The cfMesh `check_quality` choices (the gh:#1480 reference).
    fn default() -> Self {
        CheckOptions {
            boundary_skewness: BoundarySkewness::NormalProjection,
            severe_non_ortho_deg: 70.0,
            sv_hat: SvHat::ReciprocalMultiply,
            skew_guard: 1e-300,
            degenerate_faces: DegenerateFaces::Skip,
            sliver_metrics: true,
        }
    }
}

impl CheckOptions {
    /// The choices `outram-foam-mesh` made before gh:#1480
    /// (`mesh_quality::assess_quality` and `PolyPatchMesh::quality`):
    /// mirrored boundary skewness, divide, `1e-300`, degenerate faces
    /// included. Deviates from upstream dev in the boundary definition.
    pub fn foam_mesh_legacy() -> Self {
        CheckOptions {
            boundary_skewness: BoundarySkewness::Mirrored,
            severe_non_ortho_deg: 70.0,
            sv_hat: SvHat::Divide,
            skew_guard: 1e-300,
            degenerate_faces: DegenerateFaces::Include,
            sliver_metrics: true,
        }
    }
}

/// The geometry the checks read, supplied by the caller's mesh type (see the
/// module docs for why it is an input). All arrays are in face / cell order.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckGeometry {
    /// Face area vectors `Sf` (owner -> neighbour / outward).
    pub face_areas: Vec<Vector3>,
    /// Face centres `Cf`.
    pub face_centres: Vec<Vector3>,
    /// Cell centres `C`.
    pub cell_centres: Vec<Vector3>,
    /// Signed cell volumes `V`.
    pub cell_volumes: Vec<f64>,
}

/// Non-orthogonality `[degrees]` of an internal face with centre-to-centre
/// vector `d` and area vector `sf`: `None` for a degenerate face
/// (`|d| <= 1e-30` or `|Sf| <= 1e-30`), which the cfMesh reference skips.
pub fn face_non_orthogonality_deg(d: Vector3, sf: Vector3) -> Option<f64> {
    let dlen = d.mag();
    let area = sf.mag();
    if dlen > 1e-30 && area > 1e-30 {
        let cos = (d.dot(sf) / (dlen * area)).clamp(-1.0, 1.0);
        Some(cos.r_acos().to_degrees())
    } else {
        None
    }
}

/// OpenFOAM `faceSkewness` / `boundaryFaceSkewness` core: the offset `sv` of
/// the face centre from where the line of `d` pierces the face plane, over the
/// face's own extent along `sv` (largest projection of a ring vertex from
/// `cf`), floored at `floor * |d|`. `own_cc` is the owner cell centre, `ring`
/// the face's point indices. `guard` is the small number added to the
/// denominators (`1e-150` upstream's `rootVSmall`, `1e-300` in cfMesh's
/// report). `sv_hat_kind` selects divide (upstream) or reciprocal-multiply
/// (cfMesh), see [`SvHat`].
#[allow(clippy::too_many_arguments)]
pub fn face_skewness(
    cf: Vector3,
    sf: Vector3,
    own_cc: Vector3,
    d: Vector3,
    floor: f64,
    guard: f64,
    sv_hat_kind: SvHat,
    ring: &[usize],
    points: &[Vector3],
) -> f64 {
    let cpf = cf - own_cc;
    let sv = cpf - d * (sf.dot(cpf) / (sf.dot(d) + guard));
    let sv_mag = sv.mag();
    let sv_hat = match sv_hat_kind {
        SvHat::Divide => sv / (sv_mag + guard),
        SvHat::ReciprocalMultiply => sv * (1.0 / (sv_mag + guard)),
    };
    let mut fd = floor * d.mag() + guard;
    for &v in ring {
        fd = fd.max(sv_hat.dot(points[v] - cf).abs());
    }
    sv_mag / fd
}

/// Skewness of a boundary face by the chosen [`BoundarySkewness`] definition.
fn boundary_face_skewness(
    opts: &CheckOptions,
    cf: Vector3,
    sf: Vector3,
    own_cc: Vector3,
    ring: &[usize],
    points: &[Vector3],
) -> f64 {
    match opts.boundary_skewness {
        BoundarySkewness::NormalProjection => {
            let nrm = sf * (1.0 / (sf.mag() + ROOT_V_SMALL));
            let d = nrm * nrm.dot(cf - own_cc);
            face_skewness(cf, sf, own_cc, d, 0.4, ROOT_V_SMALL, opts.sv_hat, ring, points)
        }
        BoundarySkewness::Mirrored => {
            let d = (cf - own_cc) * 2.0;
            face_skewness(cf, sf, own_cc, d, 0.2, opts.skew_guard, opts.sv_hat, ring, points)
        }
    }
}

/// Cell aspect ratio, OpenFOAM `checkClosedCells`:
/// `(1/6) (Σ|Sf_x| + Σ|Sf_y| + Σ|Sf_z|) / V^(2/3)`; `0` for `V <= 0`.
///
/// ~~OpenFOAM's `checkClosedCells` aspect ratio~~ **CORRECTED 2026-11-13 (gh:#1364,
/// checked against an OpenFOAM.com v2506 `checkMesh` run, `esi_v2506_differential_gh1364/`):**
/// this is only the second term of ESI v2506's `primitiveMeshTools::cellClosedness`,
/// which takes `max(maxCmpt/(minCmpt + rootVSmall), (1/6) Σ.../V^(2/3))` over the
/// solved directions (T3A: 3232.46 in ESI, 74.12 here). OpenFOAM-dev's form was not
/// re-checked (no dev source in this checkout). The ESI form is
/// [`esi_quality_fields`] (`aspect_ratio`); this function is unchanged.
pub fn cell_aspect_ratio_cmpt(sum_cmpt_mag: Vector3, volume: f64) -> f64 {
    if volume <= 0.0 {
        return 0.0;
    }
    (sum_cmpt_mag.x + sum_cmpt_mag.y + sum_cmpt_mag.z) / (6.0 * volume.r_powf(2.0 / 3.0))
}

/// Summary of mesh-quality metrics (worst-case values over the mesh).
#[derive(Debug, Clone, PartialEq)]
pub struct QualityReport {
    /// Maximum face **non-orthogonality**, in degrees: the angle between an
    /// internal face's normal and the vector joining its two cell centres. `0°`
    /// is perfectly orthogonal; OpenFOAM flags `> 70°`.
    pub max_non_orthogonality_deg: f64,
    /// Maximum face **skewness** over the internal faces, OpenFOAM's
    /// `primitiveMeshTools::faceSkewness`: the offset `s` of the face centre
    /// from where the owner–neighbour centre line crosses the face plane,
    /// divided by the face's own extent along `s` (the largest projection of
    /// a face vertex, measured from the face centre, on `s`), floored at
    /// `0.2 |d|`. OpenFOAM flags `> 4` (`maxInternalSkewness 4`).
    ///
    /// ~~as a fraction of that line's length~~ **CORRECTED 2026-11-04
    /// (gh:#1202):** that was this port's normalisation, not upstream's; on
    /// a stack of thin prism layers it read width/thickness (6.6–7.2 on
    /// 3-layer pipes) where upstream's reads the actual skew. Ported from
    /// the workspace's existing port, `outram_foam_mesh::mesh_quality::
    /// face_skewness` (OpenFOAM `primitiveMeshTools.C`).
    pub max_skewness: f64,
    /// Maximum cell **aspect ratio** (`Σ|Sf| / (6 · V^{2/3})`, which is `1` for
    /// a cube). Large values mean sliver cells. This is cfMesh's definition
    /// (sum of face-area magnitudes); OpenFOAM's `checkClosedCells` sums the
    /// per-component magnitudes instead, see
    /// [`QualityReport::max_aspect_ratio_cmpt`].
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
    /// still have its centre outside one of its faces (verified in cfMesh's
    /// `concave_cell_has_negative_pyramid_but_positive_volume` test).
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
    ///   both are verified in cfMesh's tests.
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
    /// Mean non-orthogonality `[degrees]` over the internal faces (degenerate
    /// faces count as 0 in the sum; the divisor is the internal-face count).
    pub mean_non_orthogonality_deg: f64,
    /// Number of internal faces above [`CheckOptions::severe_non_ortho_deg`].
    pub n_severely_non_orthogonal_faces: usize,
    /// Maximum **boundary-face** skewness, by the
    /// [`CheckOptions::boundary_skewness`] definition.
    pub max_boundary_skewness: f64,
    /// Maximum cell aspect ratio by OpenFOAM's `checkClosedCells` definition
    /// ([`cell_aspect_ratio_cmpt`]).
    pub max_aspect_ratio_cmpt: f64,
    /// Mean of [`cell_aspect_ratio_cmpt`] over all cells.
    pub mean_aspect_ratio_cmpt: f64,
    /// Largest signed cell volume.
    pub max_cell_volume: f64,
    /// Sum of the signed cell volumes.
    pub total_volume: f64,
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

/// Compute the [`QualityReport`] of a mesh given as points, face rings,
/// owner / neighbour (`None` for a boundary face) and its [`CheckGeometry`].
///
/// This is the cfMesh `check_quality` (gh:#1480 reference) extended with the
/// metrics `outram-foam-mesh` reported (boundary skewness, OpenFOAM aspect
/// ratio, mean / severe non-orthogonality, volume extrema and total).
pub fn check_quality<F: AsRef<[usize]>>(
    points: &[Vector3],
    faces: &[F],
    owner: &[usize],
    neighbour: &[Option<usize>],
    geom: &CheckGeometry,
    opts: &CheckOptions,
) -> QualityReport {
    let n_cells = geom.cell_volumes.len();
    let vols = &geom.cell_volumes;
    let centres = &geom.cell_centres;
    let nf = faces.len();

    let mut min_face_area = f64::MAX;
    let mut max_non_orth = 0.0f64;
    let mut sum_non_orth = 0.0f64;
    let mut n_severe = 0usize;
    let mut n_internal = 0usize;
    let mut max_skew = 0.0f64;
    let mut max_bnd_skew = 0.0f64;
    for f in 0..nf {
        let sf = geom.face_areas[f];
        let area = sf.mag();
        min_face_area = min_face_area.min(area);
        // Non-orthogonality + skewness are defined on internal faces.
        if let Some(nb) = neighbour[f] {
            n_internal += 1;
            let co = centres[owner[f]];
            let cn = centres[nb];
            let d = cn - co;
            let ang = match opts.degenerate_faces {
                DegenerateFaces::Skip => face_non_orthogonality_deg(d, sf),
                DegenerateFaces::Include => {
                    let cos = (d.dot(sf) / (d.mag() * sf.mag() + 1e-300)).clamp(-1.0, 1.0);
                    Some(cos.r_acos().to_degrees())
                }
            };
            let evaluated = match opts.degenerate_faces {
                DegenerateFaces::Skip => ang.is_some(),
                DegenerateFaces::Include => true,
            };
            if let Some(ang) = ang {
                max_non_orth = max_non_orth.max(ang);
                sum_non_orth += ang;
                if ang > opts.severe_non_ortho_deg {
                    n_severe += 1;
                }
            }
            if evaluated {
                // Skewness (OpenFOAM `meshCheck::faceSkewness`): the
                // face-centre offset from where the centre line pierces the
                // face plane, over the face's extent along that offset,
                // floored at 0.2 |d|.
                let s = face_skewness(
                    geom.face_centres[f],
                    sf,
                    co,
                    d,
                    0.2,
                    opts.skew_guard,
                    opts.sv_hat,
                    faces[f].as_ref(),
                    points,
                );
                max_skew = max_skew.max(s);
            }
        } else {
            let s = boundary_face_skewness(
                opts,
                geom.face_centres[f],
                sf,
                centres[owner[f]],
                faces[f].as_ref(),
                points,
            );
            max_bnd_skew = max_bnd_skew.max(s);
        }
    }

    let mut max_ar = 0.0f64;
    let mut min_vol = f64::MAX;
    let mut max_vol = f64::NEG_INFINITY;
    let mut total_vol = 0.0f64;
    let mut n_neg = 0usize;
    // Sum of face areas per cell for the aspect ratio, and OpenFOAM's
    // component-magnitude sums (face order, owner then neighbour).
    let mut cell_area_sum = vec![0.0f64; n_cells];
    let mut sum_cmpt_mag = vec![Vector3::ZERO; n_cells];
    for f in 0..nf {
        let sf = geom.face_areas[f];
        let a = sf.mag();
        let cmpt = Vector3::new(sf.x.abs(), sf.y.abs(), sf.z.abs());
        cell_area_sum[owner[f]] += a;
        sum_cmpt_mag[owner[f]] += cmpt;
        if let Some(nb) = neighbour[f] {
            cell_area_sum[nb] += a;
            sum_cmpt_mag[nb] += cmpt;
        }
    }
    let mut max_ar_cmpt = 0.0f64;
    let mut sum_ar_cmpt = 0.0f64;
    for c in 0..n_cells {
        let v = vols[c];
        min_vol = min_vol.min(v);
        max_vol = max_vol.max(v);
        total_vol += v;
        if v <= 0.0 {
            n_neg += 1;
        } else {
            let ar = cell_area_sum[c] / (6.0 * v.r_powf(2.0 / 3.0));
            max_ar = max_ar.max(ar);
        }
        let arc = cell_aspect_ratio_cmpt(sum_cmpt_mag[c], v);
        sum_ar_cmpt += arc;
        max_ar_cmpt = max_ar_cmpt.max(arc);
    }

    // Sliver detectors: face pyramid volumes about the cell centres, and the
    // per-cell face-normal orientation determinant. Both reuse `centres` from
    // the pyramid decomposition above, so they see exactly the cell centres the
    // solver would.
    let mut min_pyr = f64::MAX;
    let mut n_neg_pyr = 0usize;
    // Per-cell Σ|Sf| (n̂ ⊗ n̂) as [xx, yy, zz, xy, xz, yz], and Σ|Sf|.
    let mut normal_tensor = vec![[0.0f64; 6]; n_cells];
    let mut normal_area = vec![0.0f64; n_cells];
    for f in 0..(if opts.sliver_metrics { nf } else { 0 }) {
        let sf = geom.face_areas[f];
        let area = sf.mag();
        let fc = geom.face_centres[f];
        // The owner's outward normal is +Sf; the neighbour's is −Sf.
        let incident: [Option<(usize, f64)>; 2] =
            [Some((owner[f], 1.0)), neighbour[f].map(|nb| (nb, -1.0))];
        for (c, sign) in incident.into_iter().flatten() {
            let pyr = (fc - centres[c]).dot(sf * sign) / 3.0;
            min_pyr = min_pyr.min(pyr);
            if pyr <= 0.0 {
                n_neg_pyr += 1;
            }
            if area > 1e-300 {
                // n̂ ⊗ n̂ is sign-free, so accumulate once per incidence.
                let n = sf * (1.0 / area);
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
    for c in 0..(if opts.sliver_metrics { n_cells } else { 0 }) {
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
        min_face_area: if min_face_area == f64::MAX { 0.0 } else { min_face_area },
        min_cell_volume: if min_vol == f64::MAX { 0.0 } else { min_vol },
        n_negative_volume_cells: n_neg,
        min_face_pyramid_volume: if min_pyr == f64::MAX { 0.0 } else { min_pyr },
        n_negative_pyramid_faces: n_neg_pyr,
        min_cell_determinant: if min_det == f64::MAX { 0.0 } else { min_det },
        mean_non_orthogonality_deg: if n_internal > 0 {
            sum_non_orth / n_internal as f64
        } else {
            0.0
        },
        n_severely_non_orthogonal_faces: n_severe,
        max_boundary_skewness: max_bnd_skew,
        max_aspect_ratio_cmpt: max_ar_cmpt,
        mean_aspect_ratio_cmpt: if n_cells > 0 { sum_ar_cmpt / n_cells as f64 } else { 0.0 },
        max_cell_volume: if max_vol.is_finite() { max_vol } else { 0.0 },
        total_volume: total_vol,
    }
}

/// snappyHexMesh's mesh-quality controls (`meshQualityControls`, the file
/// `etc/caseDicts/mesh/generation/meshQualityDict` of OpenFOAM-dev
/// `852527377c7e`), as checked by [`faces_failing_mesh_quality`]. The
/// defaults are that file's values.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshQualityControls {
    /// `maxNonOrtho` [degrees]; 180 disables.
    pub max_non_ortho_deg: f64,
    /// `maxBoundarySkewness`; negative disables.
    pub max_boundary_skewness: f64,
    /// `maxInternalSkewness`; negative disables.
    pub max_internal_skewness: f64,
    /// `minTetQuality`: here only its sign is checked (a decomposition
    /// tetrahedron of non-positive volume fails); `None` disables.
    pub min_tet_quality: Option<f64>,
    /// `minTwist`; below −1 disables.
    pub min_twist: f64,
    /// `minDeterminant`; negative disables.
    pub min_determinant: f64,
    /// `minFaceWeight`; outside [0, 1) disables.
    pub min_face_weight: f64,
    /// `minVolRatio`; outside [0, 1) disables.
    pub min_vol_ratio: f64,
}

impl Default for MeshQualityControls {
    fn default() -> Self {
        MeshQualityControls {
            max_non_ortho_deg: 65.0,
            max_boundary_skewness: 20.0,
            max_internal_skewness: 4.0,
            min_tet_quality: Some(1e-15),
            min_twist: 0.02,
            min_determinant: 0.001,
            min_face_weight: 0.05,
            min_vol_ratio: 0.01,
        }
    }
}

/// The faces that fail `controls` (gh:#1215): the port of OpenFOAM's
/// `meshCheck::checkMesh(report, mesh, dict, checkFaces, baffles,
/// wrongFaces)` (OpenFOAM-dev `852527377c7e`, `src/meshCheck/checkMesh.C`,
/// with the per-check definitions of `polyMeshCheck/polyMeshCheckQuality.C`
/// and `primitiveMeshCheck/primitiveMeshCheck.C`), which snappyHexMesh's
/// layer driver runs on every layered mesh (`snappyLayerDriver::
/// checkAndUnmark`) and its snap driver on every motion step
/// (`motionSmoother`).
///
/// Checks ported: `checkFaceOrthogonality` (internal faces),
/// `checkFaceSkewness` (`faceSkewness` on internal faces,
/// `boundaryFaceSkewness` with its 0.4 |d| floor on boundary faces),
/// `checkFaceTets` (sign only), `checkFaceTwist`, `checkCellDeterminant`
/// (`cellDeterminant`: det of Σ (S/⟨|S|⟩)² over the cell's internal faces; a
/// failing cell marks all its faces), `checkFaceWeights`, `checkVolRatio`.
/// Not ported: `maxConcave` (`checkFaceAngles`), `minVol` (disabled, −1e30,
/// in the default dictionary) and `minFaceFlatness` (absent from it).
/// Centres are the caller's [`CheckGeometry`] (cfMesh passes vertex-average
/// face centres, not OpenFOAM's area-weighted ones).
pub fn faces_failing_mesh_quality<F: AsRef<[usize]>>(
    points: &[Vector3],
    faces: &[F],
    owner: &[usize],
    neighbour: &[Option<usize>],
    geom: &CheckGeometry,
    controls: &MeshQualityControls,
) -> Vec<bool> {
    let vols = &geom.cell_volumes;
    let centres = &geom.cell_centres;
    let n_cells = vols.len();
    let nf = faces.len();
    let mut bad = vec![false; nf];
    let cos_max = controls.max_non_ortho_deg.to_radians().r_cos();
    const RVS: f64 = ROOT_V_SMALL;
    let mut cell_faces: Vec<Vec<usize>> = vec![Vec::new(); n_cells];
    for f in 0..nf {
        cell_faces[owner[f]].push(f);
        if let Some(n) = neighbour[f] {
            cell_faces[n].push(f);
        }
    }
    for f in 0..nf {
        let s = geom.face_areas[f];
        let fc = geom.face_centres[f];
        let co = centres[owner[f]];
        let ring = faces[f].as_ref();
        let k = ring.len();
        let mut b = false;
        match neighbour[f] {
            Some(n) => {
                let cn = centres[n];
                let d = cn - co;
                if controls.max_non_ortho_deg < 180.0 && d.dot(s) / (d.mag() * s.mag() + RVS) < cos_max {
                    b = true;
                }
                if controls.max_internal_skewness > 0.0
                    && face_skewness(fc, s, co, d, 0.2, RVS, SvHat::ReciprocalMultiply, ring, points) > controls.max_internal_skewness
                {
                    b = true;
                }
                if (0.0..1.0).contains(&controls.min_face_weight) {
                    let d_own = s.dot(fc - co).abs();
                    let d_nei = s.dot(cn - fc).abs();
                    if d_own.min(d_nei) / (d_own + d_nei + 1e-300) < controls.min_face_weight {
                        b = true;
                    }
                }
                if (0.0..1.0).contains(&controls.min_vol_ratio) {
                    let (a, c) = (vols[owner[f]].abs(), vols[n].abs());
                    if a.min(c) / (a.max(c) + 1e-300) < controls.min_vol_ratio {
                        b = true;
                    }
                }
            }
            None => {
                if controls.max_boundary_skewness > 0.0 {
                    let nrm = s * (1.0 / (s.mag() + RVS));
                    let d = nrm * nrm.dot(fc - co);
                    if face_skewness(fc, s, co, d, 0.4, RVS, SvHat::ReciprocalMultiply, ring, points) > controls.max_boundary_skewness {
                        b = true;
                    }
                }
            }
        }
        if controls.min_tet_quality.is_some() {
            for i in 0..k {
                let p = points[ring[i]];
                let q = points[ring[(i + 1) % k]];
                // owner tets with the face and cell centres exchanged, as
                // upstream, so a valid one is positive
                if (q - p).cross(fc - p).dot(co - p) >= 0.0 {
                    b = true;
                }
                if let Some(n) = neighbour[f] {
                    if (q - p).cross(fc - p).dot(centres[n] - p) <= 0.0 {
                        b = true;
                    }
                }
            }
        }
        if controls.min_twist >= -1.0 && k > 3 {
            let nf_dir = match neighbour[f] {
                Some(n) => centres[n] - co,
                None => fc - co,
            };
            let nf_dir = nf_dir * (1.0 / (nf_dir.mag() + RVS));
            for i in 0..k {
                let p = points[ring[i]];
                let q = points[ring[(i + 1) % k]];
                let tri = (q - p).cross(fc - p) * 0.5;
                let m = tri.mag();
                if m > RVS && nf_dir.dot(tri * (1.0 / m)) < controls.min_twist {
                    b = true;
                }
            }
        }
        bad[f] = b;
    }
    if controls.min_determinant >= 0.0 {
        for cf in cell_faces.iter() {
            let internal: Vec<Vector3> = cf
                .iter()
                .filter(|&&f| neighbour[f].is_some())
                .map(|&f| geom.face_areas[f])
                .collect();
            let det = if internal.is_empty() {
                0.0
            } else {
                let avg = internal.iter().map(|s| s.mag()).sum::<f64>() / internal.len() as f64;
                let mut t = [[0.0f64; 3]; 3];
                for s in &internal {
                    let v = [s.x / avg, s.y / avg, s.z / avg];
                    for i in 0..3 {
                        for j in 0..3 {
                            t[i][j] += v[i] * v[j];
                        }
                    }
                }
                (t[0][0] * (t[1][1] * t[2][2] - t[1][2] * t[2][1])
                    - t[0][1] * (t[1][0] * t[2][2] - t[1][2] * t[2][0])
                    + t[0][2] * (t[1][0] * t[2][1] - t[1][1] * t[2][0]))
                    .abs()
            };
            if det < controls.min_determinant {
                for &f in cf {
                    bad[f] = true;
                }
            }
        }
    }
    bad
}

// ---------------------------------------------------------------------------
// ESI v2506 `meshQualityControls`: the nine face checks of `polyMeshGeometry`
// as `snappySnapDriver::scaleMesh` runs them (gh:#1467, round 4).
// ---------------------------------------------------------------------------

/// OpenFOAM double-precision constants used by the ESI checks.
const ESI_SMALL: f64 = 1e-15;
const ESI_VSMALL: f64 = 1e-300;
const ESI_GREAT: f64 = 1e15;
const ESI_VGREAT: f64 = 1e300;

/// The nine checks in the order `motionSmootherAlgo::checkMesh` runs them (and
/// ESI's log prints them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EsiCheck {
    /// `checkFaceDotProduct`: non-orthogonality.
    NonOrthogonality,
    /// `checkFacePyramids`.
    FacePyramids,
    /// `checkFaceTets` (face-decomposition tet quality and the base-point search).
    FaceTets,
    /// `checkFaceAngles` (concavity).
    Concavity,
    /// `checkFaceSkewness`.
    Skewness,
    /// `checkFaceWeights`.
    FaceWeights,
    /// `checkVolRatio`.
    VolRatio,
    /// `checkFaceTwist`.
    FaceTwist,
    /// `checkCellDeterminant`.
    CellDeterminant,
}

/// The `meshQualityControls` keys of ESI v2506 `etc/caseDicts/meshQualityDict`
/// that `motionSmootherAlgo::checkMesh` evaluates. A key disables its check with
/// ESI's own condition (`maxNonOrtho >= 180 - SMALL`, `minVol <= -GREAT`,
/// `minTetQuality <= -GREAT`, `maxConcave >= 180 - SMALL`, both skewness limits
/// not positive, `minFaceWeight` outside [0, 1), `minVolRatio < 0`, `minTwist <=
/// -1`, `minDeterminant <= -1`). `minArea`, `minTriangleTwist`, `minFaceFlatness`
/// and `minEdgeLength` are -1 in that dictionary, so those checks are not ported.
#[derive(Debug, Clone, PartialEq)]
pub struct EsiQualityControls {
    /// `maxNonOrtho` [degrees].
    pub max_non_ortho: f64,
    /// `minVol`: the face-pyramid threshold of the MESH overload of `checkMesh`
    /// (the one `scaleMesh` uses); the `polyMeshGeometry` overload passes
    /// `minTetQuality` instead.
    pub min_vol: f64,
    /// `minTetQuality`.
    pub min_tet_quality: f64,
    /// `maxConcave` [degrees].
    pub max_concave: f64,
    /// `maxInternalSkewness`.
    pub max_internal_skewness: f64,
    /// `maxBoundarySkewness`.
    pub max_boundary_skewness: f64,
    /// `minFaceWeight`.
    pub min_face_weight: f64,
    /// `minVolRatio`.
    pub min_vol_ratio: f64,
    /// `minTwist`.
    pub min_twist: f64,
    /// `minDeterminant`.
    pub min_determinant: f64,
}

impl Default for EsiQualityControls {
    /// The values of `etc/caseDicts/meshQualityDict` (v2506).
    fn default() -> Self {
        EsiQualityControls {
            max_non_ortho: 65.0,
            min_vol: 1e-13,
            min_tet_quality: 1e-15,
            max_concave: 80.0,
            max_internal_skewness: 4.0,
            max_boundary_skewness: 20.0,
            min_face_weight: 0.05,
            min_vol_ratio: 0.01,
            min_twist: 0.02,
            min_determinant: 0.001,
        }
    }
}

impl EsiQualityControls {
    /// Every check switched off, ESI's way: the base for a one-check run.
    pub fn all_disabled() -> Self {
        EsiQualityControls {
            max_non_ortho: 180.0,
            min_vol: -ESI_VGREAT,
            min_tet_quality: -ESI_VGREAT,
            max_concave: 180.0,
            max_internal_skewness: -1.0,
            max_boundary_skewness: -1.0,
            min_face_weight: -1.0,
            min_vol_ratio: -1.0,
            min_twist: -2.0,
            min_determinant: -2.0,
        }
    }
}

/// The mesh geometry `polyMesh::movePoints` leaves in the mesh
/// (`primitiveMeshTools::makeFaceCentresAndAreas` and `makeCellCentresAndVols`)
/// plus `primitiveMesh::cells()`: what the ESI checks read.
#[derive(Debug, Clone)]
pub struct EsiGeometry {
    /// Face centres.
    pub face_centres: Vec<[f64; 3]>,
    /// Face area vectors.
    pub face_areas: Vec<[f64; 3]>,
    /// Cell centres.
    pub cell_centres: Vec<[f64; 3]>,
    /// Cell volumes.
    pub cell_volumes: Vec<f64>,
    /// `primitiveMesh::cells()`: per cell the faces it owns (ascending) and then the
    /// faces it neighbours (ascending), as `calcCells` fills them.
    pub cell_faces: Vec<Vec<usize>>,
}

/// Geometry of `mesh` as ESI computes it after a point move.
pub fn esi_geometry(mesh: &SnapMesh) -> EsiGeometry {
    let (face_centres, face_areas) = esi_snap::face_centres_areas(&mesh.points, &mesh.faces);
    let (cell_centres, cell_volumes) = esi_snap::cell_centres_vols(mesh);
    let n_cells = cell_volumes.len();
    let mut cell_faces: Vec<Vec<usize>> = vec![Vec::new(); n_cells];
    for (f, &o) in mesh.owner.iter().enumerate() {
        cell_faces[o].push(f);
    }
    for (f, &n) in mesh.neighbour.iter().enumerate() {
        cell_faces[n].push(f);
    }
    EsiGeometry { face_centres, face_areas, cell_centres, cell_volumes, cell_faces }
}

/// `face::areaNormal(points)` (v2506 `face.C:576`): centre decomposition, centre
/// point last in each triangle, plain triangle for a 3-vertex face.
pub fn esi_face_area_normal(points: &[[f64; 3]], f: &[usize]) -> [f64; 3] {
    let n_points = f.len();
    let tri = |p0: [f64; 3], p1: [f64; 3], p2: [f64; 3]| {
        esi_snap::scale(0.5, esi_snap::cross(esi_snap::sub(p1, p0), esi_snap::sub(p2, p0)))
    };
    if n_points == 3 {
        return tri(points[f[0]], points[f[1]], points[f[2]]);
    }
    let mut centre_point = [0.0; 3];
    for &p in f {
        centre_point = esi_snap::add(centre_point, points[p]);
    }
    centre_point = esi_snap::div(centre_point, n_points as f64);
    let mut n = [0.0; 3];
    for pi in 0..n_points {
        let next_point = points[f[(pi + 1) % n_points]];
        n = esi_snap::add(n, tri(points[f[pi]], next_point, centre_point));
    }
    n
}

/// `normalised(v)`: zero when `|v| < ROOTVSMALL`, else `v/|v|`.
fn esi_normalised(v: [f64; 3]) -> [f64; 3] {
    let s = esi_snap::mag(v);
    if s < ROOT_V_SMALL {
        [0.0; 3]
    } else {
        esi_snap::div(v, s)
    }
}

/// `tetrahedron::quality()` for the tet (a, b, c, d): `mag/(8/(9 sqrt 3) *
/// min(circumRadius, GREAT)^3 + ROOTVSMALL)`, `mag = (1/6) ((b-a) x (c-a)) . (d-a)`.
pub fn esi_tet_quality(a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]) -> f64 {
    use esi_snap::{add, cross, dot, mag, mag_sqr, scale, sub};
    let tet_mag = (1.0 / 6.0) * dot(cross(sub(b, a), sub(c, a)), sub(d, a));
    // circumRadius
    let va = sub(b, a);
    let vb = sub(c, a);
    let vc = sub(d, a);
    let lambda = mag_sqr(vc) - dot(va, vc);
    let mu = mag_sqr(vb) - dot(va, vb);
    let ba = cross(vb, va);
    let ca = cross(vc, va);
    let num = sub(scale(lambda, ba), scale(mu, ca));
    let denom = dot(vc, ba);
    let circum_radius = if denom.abs() < ROOT_V_SMALL {
        ESI_GREAT
    } else {
        mag(scale(0.5, add(va, esi_snap::div(num, denom))))
    };
    let r = circum_radius.min(ESI_GREAT);
    let pow3 = r * (r * r);
    tet_mag / (8.0 / (9.0 * 3.0f64.sqrt()) * pow3 + ROOT_V_SMALL)
}

/// `checkFaceDotProduct` (internal faces; no coupled patches or baffles): faces whose
/// `d.S/(|d||S| + VSMALL)` is below `cos(degToRad(max_non_ortho))`,
/// `degToRad(x) = x*M_PI/180`.
pub fn esi_check_non_orthogonality(
    mesh: &SnapMesh,
    geom: &EsiGeometry,
    max_non_ortho: f64,
    check_faces: &[usize],
) -> Vec<usize> {
    use esi_snap::{dot, mag, sub};
    let threshold = (max_non_ortho * std::f64::consts::PI / 180.0).r_cos();
    let mut wrong = Vec::new();
    for &f in check_faces {
        if f < mesh.neighbour.len() {
            let d = sub(geom.cell_centres[mesh.neighbour[f]], geom.cell_centres[mesh.owner[f]]);
            let s = geom.face_areas[f];
            let d_dot_s = dot(d, s) / (mag(d) * mag(s) + ESI_VSMALL);
            if d_dot_s < threshold {
                wrong.push(f);
            }
        }
    }
    wrong
}

/// `checkFacePyramids`: the owner pyramid (volume must be below `-min_pyr_vol`) and the
/// neighbour pyramid (above `min_pyr_vol`); `pyramid::mag = (1/3) areaNormal . (apex -
/// face::centre)`.
pub fn esi_check_face_pyramids(
    mesh: &SnapMesh,
    geom: &EsiGeometry,
    min_pyr_vol: f64,
    check_faces: &[usize],
) -> Vec<usize> {
    use esi_snap::{dot, sub};
    let mut wrong = Vec::new();
    for &f in check_faces {
        let ring = &mesh.faces[f];
        let area = esi_face_area_normal(&mesh.points, ring);
        let centre = esi_snap::face_centre(&mesh.points, ring);
        let own_vol = (1.0 / 3.0) * dot(area, sub(geom.cell_centres[mesh.owner[f]], centre));
        let mut bad = own_vol > -min_pyr_vol;
        if f < mesh.neighbour.len() {
            let nei_vol = (1.0 / 3.0) * dot(area, sub(geom.cell_centres[mesh.neighbour[f]], centre));
            if nei_vol < min_pyr_vol {
                bad = true;
            }
        }
        if bad {
            wrong.push(f);
        }
    }
    wrong
}

/// `polyMeshTetDecomposition::minQuality(mesh, cC, fI, isOwner, faceBasePtI)`.
pub(crate) fn esi_tet_decomposition_min_quality(
    mesh: &SnapMesh,
    c_c: [f64; 3],
    f: usize,
    is_owner: bool,
    face_base_pt: usize,
) -> f64 {
    let ring = &mesh.faces[f];
    let n = ring.len();
    let tet_base_pt = mesh.points[ring[face_base_pt]];
    let mut min_q = ESI_VGREAT;
    for tet_pt in 1..n - 1 {
        let face_pt = (tet_pt + face_base_pt) % n;
        let other = (face_pt + 1) % n;
        let (a, b) = if is_owner { (ring[face_pt], ring[other]) } else { (ring[other], ring[face_pt]) };
        let q = esi_tet_quality(c_c, tet_base_pt, mesh.points[a], mesh.points[b]);
        if q < min_q {
            min_q = q;
        }
    }
    min_q
}

/// `checkFaceTets`: each face-decomposition tet of the owner (cell and face centre
/// exchanged, as upstream) and of the neighbour must have quality at least
/// `min_tet_quality`, and a base point must exist for which the fan decomposition of the
/// face is good for both cells (`findSharedBasePoint`, `findBasePoint` on a boundary
/// face).
pub fn esi_check_face_tets(
    mesh: &SnapMesh,
    geom: &EsiGeometry,
    min_tet_quality: f64,
    check_faces: &[usize],
) -> Vec<usize> {
    let tets_bad = |f: usize, fc: [f64; 3], cc: [f64; 3]| -> bool {
        let ring = &mesh.faces[f];
        (0..ring.len()).any(|fp| {
            let q = esi_tet_quality(mesh.points[ring[fp]], mesh.points[ring[(fp + 1) % ring.len()]], fc, cc);
            q < min_tet_quality
        })
    };
    let mut wrong = Vec::new();
    for &f in check_faces {
        let mut bad = tets_bad(f, geom.cell_centres[mesh.owner[f]], geom.face_centres[f]);
        let own_cc = geom.cell_centres[mesh.owner[f]];
        if f < mesh.neighbour.len() {
            let nei_cc = geom.cell_centres[mesh.neighbour[f]];
            if tets_bad(f, geom.face_centres[f], nei_cc) {
                bad = true;
            }
            let n = mesh.faces[f].len();
            let shared = (0..n).any(|base| {
                let own_q = esi_tet_decomposition_min_quality(mesh, own_cc, f, true, base);
                let nei_q = esi_tet_decomposition_min_quality(mesh, nei_cc, f, false, base);
                own_q.min(nei_q) > min_tet_quality
            });
            if !shared {
                bad = true;
            }
        } else {
            let n = mesh.faces[f].len();
            let base = (0..n).any(|base| esi_tet_decomposition_min_quality(mesh, own_cc, f, true, base) > min_tet_quality);
            if !base {
                bad = true;
            }
        }
        if bad {
            wrong.push(f);
        }
    }
    wrong
}

/// `checkFaceAngles`: a face is concave when at one vertex the normal of the two edges
/// (their sine at least `sin(degToRad(max_deg))`) points against the face normal.
pub fn esi_check_face_angles(
    mesh: &SnapMesh,
    geom: &EsiGeometry,
    max_deg: f64,
    check_faces: &[usize],
) -> Vec<usize> {
    use esi_snap::{cross, div, dot, mag, sub};
    let max_sin = (max_deg * std::f64::consts::PI / 180.0).r_sin();
    let mut wrong = Vec::new();
    for &f in check_faces {
        let ring = &mesh.faces[f];
        let n = ring.len();
        let face_normal = esi_normalised(geom.face_areas[f]);
        let p = &mesh.points;
        let mut e_prev = sub(p[ring[0]], p[ring[n - 1]]);
        let mut mag_e_prev = mag(e_prev);
        e_prev = div(e_prev, mag_e_prev + ESI_VSMALL);
        for fp0 in 0..n {
            let mut e10 = sub(p[ring[(fp0 + 1) % n]], p[ring[fp0]]);
            let mag_e10 = mag(e10);
            e10 = div(e10, mag_e10 + ESI_VSMALL);
            if mag_e_prev > ESI_SMALL && mag_e10 > ESI_SMALL {
                let mut edge_normal = cross(e_prev, e10);
                let mag_edge_normal = mag(edge_normal);
                if !(mag_edge_normal < max_sin) {
                    edge_normal = div(edge_normal, mag_edge_normal);
                    if dot(edge_normal, face_normal) < ESI_SMALL && wrong.last() != Some(&f) {
                        wrong.push(f);
                    }
                }
            }
            e_prev = e10;
            mag_e_prev = mag_e10;
        }
    }
    wrong
}

/// `primitiveMeshTools::faceSkewness` / `boundaryFaceSkewness` (v2506). `owner_cc`,
/// and `nei_cc` for an internal face.
fn esi_face_skewness(
    mesh: &SnapMesh,
    geom: &EsiGeometry,
    f: usize,
    owner_cc: [f64; 3],
    nei_cc: Option<[f64; 3]>,
) -> f64 {
    use esi_snap::{dot, mag, scale, sub};
    let fc = geom.face_centres[f];
    let fa = geom.face_areas[f];
    let cpf = sub(fc, owner_cc);
    let (d, floor) = match nei_cc {
        Some(n) => (sub(n, owner_cc), 0.2),
        None => {
            let normal = esi_normalised(fa);
            (scale(dot(normal, cpf), normal), 0.4)
        }
    };
    // `(S . Cpf)/((S . d) + ROOTVSMALL) * d`: scalar times vector.
    let q = dot(fa, cpf) / (dot(fa, d) + ROOT_V_SMALL);
    let sv = sub(cpf, scale(q, d));
    let sv_hat = esi_snap::div(sv, mag(sv) + ROOT_V_SMALL);
    let mut fd = floor * mag(d) + ROOT_V_SMALL;
    for &pi in &mesh.faces[f] {
        fd = fd.max(dot(sv_hat, sub(mesh.points[pi], fc)).abs());
    }
    mag(sv) / fd
}

/// `checkFaceSkewness`: internal faces above `internal_skew`, boundary faces above
/// `boundary_skew`.
pub fn esi_check_face_skewness(
    mesh: &SnapMesh,
    geom: &EsiGeometry,
    internal_skew: f64,
    boundary_skew: f64,
    check_faces: &[usize],
) -> Vec<usize> {
    let mut wrong = Vec::new();
    for &f in check_faces {
        let own_cc = geom.cell_centres[mesh.owner[f]];
        if f < mesh.neighbour.len() {
            let s = esi_face_skewness(mesh, geom, f, own_cc, Some(geom.cell_centres[mesh.neighbour[f]]));
            if s > internal_skew {
                wrong.push(f);
            }
        } else if esi_face_skewness(mesh, geom, f, own_cc, None) > boundary_skew {
            wrong.push(f);
        }
    }
    wrong
}

/// `checkFaceWeights`: internal faces with `min(dNei, dOwn)/(dNei + dOwn + VSMALL)`
/// below `warn_weight`.
pub fn esi_check_face_weights(
    mesh: &SnapMesh,
    geom: &EsiGeometry,
    warn_weight: f64,
    check_faces: &[usize],
) -> Vec<usize> {
    use esi_snap::{dot, sub};
    let mut wrong = Vec::new();
    for &f in check_faces {
        if f < mesh.neighbour.len() {
            let fc = geom.face_centres[f];
            let fa = geom.face_areas[f];
            let d_own = dot(fa, sub(fc, geom.cell_centres[mesh.owner[f]])).abs();
            let d_nei = dot(fa, sub(geom.cell_centres[mesh.neighbour[f]], fc)).abs();
            let weight = d_nei.min(d_own) / (d_nei + d_own + ESI_VSMALL);
            if weight < warn_weight {
                wrong.push(f);
            }
        }
    }
    wrong
}

/// `checkVolRatio`: internal faces whose neighbouring cells' volume ratio is below
/// `warn_ratio`.
pub fn esi_check_vol_ratio(
    mesh: &SnapMesh,
    geom: &EsiGeometry,
    warn_ratio: f64,
    check_faces: &[usize],
) -> Vec<usize> {
    let mut wrong = Vec::new();
    for &f in check_faces {
        if f < mesh.neighbour.len() {
            let own_vol = geom.cell_volumes[mesh.owner[f]].abs();
            let nei_vol = geom.cell_volumes[mesh.neighbour[f]].abs();
            let ratio = own_vol.min(nei_vol) / (own_vol.max(nei_vol) + ESI_VSMALL);
            if ratio < warn_ratio {
                wrong.push(f);
            }
        }
    }
    wrong
}

/// `checkFaceTwist`: for faces with more than 3 vertices the dot product of the cell-cell
/// (or cell-face) direction with each face-centre triangle's unit normal must be at
/// least `min_twist`.
pub fn esi_check_face_twist(
    mesh: &SnapMesh,
    geom: &EsiGeometry,
    min_twist: f64,
    check_faces: &[usize],
) -> Vec<usize> {
    use esi_snap::{cross, div, dot, mag, scale, sub};
    let mut wrong = Vec::new();
    for &f in check_faces {
        let ring = &mesh.faces[f];
        if ring.len() > 3 {
            let own_cc = geom.cell_centres[mesh.owner[f]];
            let nf = if f < mesh.neighbour.len() {
                esi_normalised(sub(geom.cell_centres[mesh.neighbour[f]], own_cc))
            } else {
                esi_normalised(sub(geom.face_centres[f], own_cc))
            };
            if nf != [0.0; 3] {
                let fc = geom.face_centres[f];
                for fp in 0..ring.len() {
                    let p0 = mesh.points[ring[fp]];
                    let p1 = mesh.points[ring[(fp + 1) % ring.len()]];
                    let tri_area = scale(0.5, cross(sub(p1, p0), sub(fc, p0)));
                    let mag_tri = mag(tri_area);
                    if mag_tri > ESI_VSMALL && dot(nf, div(tri_area, mag_tri)) < min_twist {
                        wrong.push(f);
                        break;
                    }
                }
            }
        }
    }
    wrong
}

/// `checkCellDeterminant`: every face of each cell (among `primitiveMesh::cells()` of the
/// faces in `check_faces`, `affectedCells`) whose scaled determinant of
/// `sum(S (x) S/(|S| + VSMALL))/(sum |S| + VSMALL)` over `0.037037037037037` is below
/// `warn_det`.
pub fn esi_check_cell_determinant(
    mesh: &SnapMesh,
    geom: &EsiGeometry,
    warn_det: f64,
    check_faces: &[usize],
) -> Vec<usize> {
    let mut affected: Vec<usize> = Vec::new();
    for &f in check_faces {
        affected.push(mesh.owner[f]);
        if f < mesh.neighbour.len() {
            affected.push(mesh.neighbour[f]);
        }
    }
    affected.sort_unstable();
    affected.dedup();
    let mut wrong = Vec::new();
    for c in affected {
        let mut t = [[0.0f64; 3]; 3];
        let mut mag_area_sum = 0.0;
        for &f in &geom.cell_faces[c] {
            let fa = geom.face_areas[f];
            let mag_area = esi_snap::mag(fa);
            mag_area_sum += mag_area;
            let b = esi_snap::div(fa, mag_area + ESI_VSMALL);
            for i in 0..3 {
                for j in 0..3 {
                    t[i][j] += fa[i] * b[j];
                }
            }
        }
        let s = mag_area_sum + ESI_VSMALL;
        let a = |i: usize, j: usize| t[i][j] / s;
        let det = a(0, 0) * a(1, 1) * a(2, 2) + a(0, 1) * a(1, 2) * a(2, 0) + a(0, 2) * a(1, 0) * a(2, 1)
            - a(0, 0) * a(1, 2) * a(2, 1)
            - a(0, 1) * a(1, 0) * a(2, 2)
            - a(0, 2) * a(1, 1) * a(2, 0);
        if det / 0.037037037037037 < warn_det {
            wrong.extend(geom.cell_faces[c].iter().copied());
        }
    }
    wrong.sort_unstable();
    wrong.dedup();
    wrong
}

/// Result of [`esi_check_mesh`].
#[derive(Debug, Clone, PartialEq)]
pub struct EsiCheckResult {
    /// Faces in error (`wrongFaces`), ascending.
    pub wrong_faces: Vec<usize>,
    /// For each enabled check, in ESI's order, the number of faces it ADDED to the set
    /// (ESI's log prints this, not the check's own count).
    pub new_faces: Vec<(EsiCheck, usize)>,
}

/// `motionSmootherAlgo::checkMesh(false, mesh, dict, checkFaces, baffles, wrongFaces)`
/// (v2506 `motionSmootherAlgoCheck.C:59`, the MESH overload `scaleMesh` calls), without
/// baffles or coupled patches: the nine checks in ESI's order on `check_faces`.
pub fn esi_check_mesh(
    mesh: &SnapMesh,
    geom: &EsiGeometry,
    controls: &EsiQualityControls,
    check_faces: &[usize],
) -> EsiCheckResult {
    let mut in_set = vec![false; mesh.faces.len()];
    let mut n_wrong = 0usize;
    let mut new_faces: Vec<(EsiCheck, usize)> = Vec::new();
    let mut add = |kind: EsiCheck, faces: Vec<usize>| {
        for f in faces {
            if !in_set[f] {
                in_set[f] = true;
                n_wrong += 1;
            }
        }
        let before: usize = new_faces.iter().map(|(_, n)| n).sum();
        new_faces.push((kind, n_wrong - before));
    };
    let c = controls;
    if c.max_non_ortho < 180.0 - ESI_SMALL {
        add(EsiCheck::NonOrthogonality, esi_check_non_orthogonality(mesh, geom, c.max_non_ortho, check_faces));
    }
    if c.min_vol > -ESI_GREAT {
        add(EsiCheck::FacePyramids, esi_check_face_pyramids(mesh, geom, c.min_vol, check_faces));
    }
    if c.min_tet_quality > -ESI_GREAT {
        add(EsiCheck::FaceTets, esi_check_face_tets(mesh, geom, c.min_tet_quality, check_faces));
    }
    if c.max_concave < 180.0 - ESI_SMALL {
        add(EsiCheck::Concavity, esi_check_face_angles(mesh, geom, c.max_concave, check_faces));
    }
    if c.max_internal_skewness > 0.0 || c.max_boundary_skewness > 0.0 {
        add(
            EsiCheck::Skewness,
            esi_check_face_skewness(mesh, geom, c.max_internal_skewness, c.max_boundary_skewness, check_faces),
        );
    }
    if c.min_face_weight >= 0.0 && c.min_face_weight < 1.0 {
        add(EsiCheck::FaceWeights, esi_check_face_weights(mesh, geom, c.min_face_weight, check_faces));
    }
    if c.min_vol_ratio >= 0.0 {
        add(EsiCheck::VolRatio, esi_check_vol_ratio(mesh, geom, c.min_vol_ratio, check_faces));
    }
    if c.min_twist > -1.0 {
        add(EsiCheck::FaceTwist, esi_check_face_twist(mesh, geom, c.min_twist, check_faces));
    }
    if c.min_determinant > -1.0 {
        add(EsiCheck::CellDeterminant, esi_check_cell_determinant(mesh, geom, c.min_determinant, check_faces));
    }
    let wrong_faces: Vec<usize> = (0..in_set.len()).filter(|&f| in_set[f]).collect();
    EsiCheckResult { wrong_faces, new_faces }
}

// ---------------------------------------------------------------------------
// ESI v2506 `checkMesh` metric FIELDS (gh:#1364): the per-face / per-cell values
// that `primitiveMesh::check*` reduce to the numbers `checkMesh` prints.
// ---------------------------------------------------------------------------

/// `polyMesh::geometricD()` for a mesh whose only direction-removing patches
/// are `empty` ones (no wedges): `emptyDirVec` is the sum over the faces of the
/// empty patches of `cmptMag(faceArea)`, normalised; a direction with a
/// component above `1e-6` is `-1` (removed), else `1`.
pub fn esi_geometric_d(face_areas: &[[f64; 3]], empty_faces: &[usize]) -> [i32; 3] {
    let mut d = [1i32; 3];
    if empty_faces.is_empty() {
        return d;
    }
    let mut v = [0.0f64; 3];
    for &f in empty_faces {
        for k in 0..3 {
            v[k] += face_areas[f][k].abs();
        }
    }
    let n = esi_snap::div(v, esi_snap::mag(v));
    for k in 0..3 {
        if n[k] > 1e-6 {
            d[k] = -1;
        }
    }
    d
}

/// Every field `checkMesh` derives its geometry report from, evaluated the way
/// ESI v2506's `primitiveMeshTools` does (see [`esi_quality_fields`]).
#[derive(Debug, Clone)]
pub struct EsiQualityFields {
    /// `geometricD` the cell fields were evaluated with (1 = solved direction).
    pub mesh_d: [i32; 3],
    /// Face centres and area vectors, cell centres and volumes.
    pub geometry: EsiGeometry,
    /// `faceOrthogonality`: `d.S/(|d||S| + rootVSmall)` per internal face.
    pub orthogonality: Vec<f64>,
    /// `faceSkewness` per face (internal then boundary definition).
    pub skewness: Vec<f64>,
    /// `facePyramidVolume`, owner pyramids (every face).
    pub own_pyramid: Vec<f64>,
    /// `facePyramidVolume`, neighbour pyramids (internal faces).
    pub nei_pyramid: Vec<f64>,
    /// `cellClosedness` openness per cell.
    pub openness: Vec<f64>,
    /// `cellClosedness` aspect ratio per cell.
    pub aspect_ratio: Vec<f64>,
    /// `faceFlatness` per face (1 for triangles).
    pub flatness: Vec<f64>,
    /// `faceConcavity(sin(10 deg))` per face.
    pub concavity: Vec<f64>,
    /// `cellDeterminant` per cell (internal faces only).
    pub determinant: Vec<f64>,
}

/// The ESI v2506 `checkMesh` fields of `mesh` (serial, no coupled patches):
/// `primitiveMeshTools::faceOrthogonality`, `faceSkewness`, `facePyramidVolume`,
/// `cellClosedness`, `faceFlatness`, `faceConcavity` and `cellDeterminant`, with
/// `mesh_d` = `polyMesh::geometricD()` ([`esi_geometric_d`]).
pub fn esi_quality_fields(mesh: &SnapMesh, mesh_d: [i32; 3]) -> EsiQualityFields {
    use esi_snap::{add, cross, div, dot, mag, scale, sub};
    let geom = esi_geometry(mesh);
    let n_int = mesh.neighbour.len();
    let n_faces = mesh.faces.len();
    let n_cells = geom.cell_volumes.len();

    // faceOrthogonality
    let mut orthogonality = Vec::with_capacity(n_int);
    for f in 0..n_int {
        let d = sub(geom.cell_centres[mesh.neighbour[f]], geom.cell_centres[mesh.owner[f]]);
        let s = geom.face_areas[f];
        orthogonality.push(dot(d, s) / (mag(d) * mag(s) + ROOT_V_SMALL));
    }

    // faceSkewness
    let mut skewness = Vec::with_capacity(n_faces);
    for f in 0..n_faces {
        let own_cc = geom.cell_centres[mesh.owner[f]];
        let nei_cc = if f < n_int { Some(geom.cell_centres[mesh.neighbour[f]]) } else { None };
        skewness.push(esi_face_skewness(mesh, &geom, f, own_cc, nei_cc));
    }

    // facePyramidVolume: -pyramid(face, ownCc).mag, +pyramid(face, neiCc).mag
    let mut own_pyramid = Vec::with_capacity(n_faces);
    let mut nei_pyramid = Vec::with_capacity(n_int);
    for f in 0..n_faces {
        let ring = &mesh.faces[f];
        let area = esi_face_area_normal(&mesh.points, ring);
        let centre = esi_snap::face_centre(&mesh.points, ring);
        own_pyramid.push(-((1.0 / 3.0) * dot(area, sub(geom.cell_centres[mesh.owner[f]], centre))));
        if f < n_int {
            nei_pyramid.push((1.0 / 3.0) * dot(area, sub(geom.cell_centres[mesh.neighbour[f]], centre)));
        }
    }

    // cellClosedness
    let mut sum_closed = vec![[0.0f64; 3]; n_cells];
    let mut sum_mag_closed = vec![[0.0f64; 3]; n_cells];
    let cmpt_mag = |a: [f64; 3]| [a[0].abs(), a[1].abs(), a[2].abs()];
    for f in 0..n_faces {
        let o = mesh.owner[f];
        sum_closed[o] = add(sum_closed[o], geom.face_areas[f]);
        sum_mag_closed[o] = add(sum_mag_closed[o], cmpt_mag(geom.face_areas[f]));
    }
    for f in 0..n_int {
        let n = mesh.neighbour[f];
        sum_closed[n] = sub(sum_closed[n], geom.face_areas[f]);
        sum_mag_closed[n] = add(sum_mag_closed[n], cmpt_mag(geom.face_areas[f]));
    }
    let n_dims = mesh_d.iter().filter(|&&d| d == 1).count();
    let mut openness = Vec::with_capacity(n_cells);
    let mut aspect_ratio = Vec::with_capacity(n_cells);
    for c in 0..n_cells {
        let mut max_openness = 0.0f64;
        for k in 0..3 {
            max_openness = max_openness.max(sum_closed[c][k].abs() / (sum_mag_closed[c][k] + ROOT_V_SMALL));
        }
        openness.push(max_openness);
        let mut min_cmpt = ESI_VGREAT;
        let mut max_cmpt = -ESI_VGREAT;
        for k in 0..3 {
            if mesh_d[k] == 1 {
                min_cmpt = min_cmpt.min(sum_mag_closed[c][k]);
                max_cmpt = max_cmpt.max(sum_mag_closed[c][k]);
            }
        }
        let mut aspect = max_cmpt / (min_cmpt + ROOT_V_SMALL);
        if n_dims == 3 {
            let v = ROOT_V_SMALL.max(geom.cell_volumes[c]);
            let s = sum_mag_closed[c];
            aspect = aspect.max((1.0 / 6.0) * (s[0] + s[1] + s[2]) / v.r_powf(2.0 / 3.0));
        }
        aspect_ratio.push(aspect);
    }

    // faceFlatness
    let mut flatness = vec![1.0f64; n_faces];
    for f in 0..n_faces {
        let ring = &mesh.faces[f];
        let mag_area = mag(geom.face_areas[f]);
        if ring.len() > 3 && mag_area > ROOT_V_SMALL {
            let fc = geom.face_centres[f];
            let mut sum_a = 0.0;
            for fp in 0..ring.len() {
                let this_point = mesh.points[ring[fp]];
                let next_point = mesh.points[ring[(fp + 1) % ring.len()]];
                let n = scale(0.5, cross(sub(next_point, this_point), sub(fc, this_point)));
                sum_a += mag(n);
            }
            flatness[f] = mag_area / (sum_a + ROOT_V_SMALL);
        }
    }

    // faceConcavity(sin(degToRad(10)))
    let max_sin = (10.0 * std::f64::consts::PI / 180.0).r_sin();
    let mut concavity = Vec::with_capacity(n_faces);
    for f in 0..n_faces {
        let ring = &mesh.faces[f];
        let face_normal = div(geom.face_areas[f], mag(geom.face_areas[f]) + ROOT_V_SMALL);
        let p = |i: usize| mesh.points[ring[i]];
        let mut e_prev = sub(p(0), p(ring.len() - 1));
        let mut mag_e_prev = mag(e_prev);
        e_prev = div(e_prev, mag_e_prev + ROOT_V_SMALL);
        let mut max_edge_sin = 0.0f64;
        for fp0 in 0..ring.len() {
            let mut e10 = sub(p((fp0 + 1) % ring.len()), p(fp0));
            let mag_e10 = mag(e10);
            e10 = div(e10, mag_e10 + ROOT_V_SMALL);
            if mag_e_prev > ESI_SMALL && mag_e10 > ESI_SMALL {
                let mut edge_normal = cross(e_prev, e10);
                let mag_edge_normal = mag(edge_normal);
                if !(mag_edge_normal < max_sin) {
                    edge_normal = div(edge_normal, mag_edge_normal);
                    if dot(edge_normal, face_normal) < ESI_SMALL {
                        max_edge_sin = max_edge_sin.max(mag_edge_normal);
                    }
                }
            }
            e_prev = e10;
            mag_e_prev = mag_e10;
        }
        concavity.push(max_edge_sin);
    }

    // cellDeterminant (internal faces only; serial)
    let mut determinant = Vec::with_capacity(n_cells);
    let two_d = mesh_d.iter().position(|&d| d != 1);
    for c in 0..n_cells {
        if n_dims == 1 {
            determinant.push(1.0);
            continue;
        }
        let mut avg_area = 0.0;
        let mut n_internal = 0usize;
        for &f in &geom.cell_faces[c] {
            if f < n_int {
                avg_area += mag(geom.face_areas[f]);
                n_internal += 1;
            }
        }
        if n_internal == 0 || avg_area < ROOT_V_SMALL {
            determinant.push(0.0);
            continue;
        }
        avg_area /= n_internal as f64;
        // symmTensor (xx, xy, xz, yy, yz, zz) += sqr(S/avgArea)
        let mut t = [0.0f64; 6];
        for &f in &geom.cell_faces[c] {
            if f < n_int {
                let s = div(geom.face_areas[f], avg_area);
                t[0] += s[0] * s[0];
                t[1] += s[0] * s[1];
                t[2] += s[0] * s[2];
                t[3] += s[1] * s[1];
                t[4] += s[1] * s[2];
                t[5] += s[2] * s[2];
            }
        }
        if n_dims == 2 {
            match two_d {
                Some(0) => t[0] = 1.0,
                Some(1) => t[3] = 1.0,
                _ => t[5] = 1.0,
            }
        }
        let (xx, xy, xz, yy, yz, zz) = (t[0], t[1], t[2], t[3], t[4], t[5]);
        let det = xx * yy * zz + xy * yz * xz + xz * xy * yz - xx * yz * yz - xy * xy * zz - xz * yy * xz;
        determinant.push(det.abs() / 8.0);
    }

    EsiQualityFields {
        mesh_d,
        geometry: geom,
        orthogonality,
        skewness,
        own_pyramid,
        nei_pyramid,
        openness,
        aspect_ratio,
        flatness,
        concavity,
        determinant,
    }
}

/// The aggregate numbers `checkMesh` prints (6 significant digits in ESI's
/// text) from [`EsiQualityFields`]: `Mesh non-orthogonality Max/average`,
/// `Max skewness`, `Max cell openness`, `Max aspect ratio`, volumes, minimum
/// face area, and the cell determinant minimum / average.
#[derive(Debug, Clone, PartialEq)]
pub struct EsiCheckSummary {
    /// `radToDeg(acos(min d.S/...))`.
    pub max_non_orthogonality_deg: f64,
    /// `radToDeg(acos(sum/nInternal))`.
    pub avg_non_orthogonality_deg: f64,
    /// Largest face skewness.
    pub max_skewness: f64,
    /// Largest cell openness.
    pub max_openness: f64,
    /// Largest cell aspect ratio.
    pub max_aspect_ratio: f64,
    /// Smallest cell volume.
    pub min_volume: f64,
    /// Largest cell volume.
    pub max_volume: f64,
    /// Sum of the cell volumes.
    pub total_volume: f64,
    /// Smallest face area magnitude.
    pub min_face_area: f64,
    /// Smallest cell determinant.
    pub min_determinant: f64,
    /// Average cell determinant.
    pub avg_determinant: f64,
}

/// Reduce [`EsiQualityFields`] as `primitiveMesh::check*` do (`min`, `max`,
/// `sum` in index order; angles with `rad*180.0/M_PI`).
pub fn esi_check_summary(q: &EsiQualityFields) -> EsiCheckSummary {
    let min = |v: &[f64]| v.iter().copied().fold(ESI_GREAT, f64::min);
    let max = |v: &[f64]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let sum = |v: &[f64]| v.iter().copied().fold(0.0, |a, b| a + b);
    let to_deg = |rad: f64| rad * 180.0 / std::f64::consts::PI;
    let n_int = q.orthogonality.len();
    let mag_areas: Vec<f64> = q.geometry.face_areas.iter().map(|&a| esi_snap::mag(a)).collect();
    EsiCheckSummary {
        max_non_orthogonality_deg: to_deg(min(&q.orthogonality).r_acos()),
        avg_non_orthogonality_deg: to_deg((sum(&q.orthogonality) / n_int as f64).r_acos()),
        max_skewness: max(&q.skewness),
        max_openness: max(&q.openness),
        max_aspect_ratio: max(&q.aspect_ratio),
        min_volume: min(&q.geometry.cell_volumes),
        max_volume: q.geometry.cell_volumes.iter().copied().fold(-ESI_GREAT, f64::max),
        total_volume: sum(&q.geometry.cell_volumes),
        min_face_area: min(&mag_areas),
        min_determinant: min(&q.determinant),
        avg_determinant: sum(&q.determinant) / q.determinant.len() as f64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit cube cell: 8 points, 6 outward-wound quads, one cell.
    fn unit_cube() -> (Vec<Vector3>, Vec<Vec<usize>>, CheckGeometry) {
        let p = |x: f64, y: f64, z: f64| Vector3::new(x, y, z);
        let points = vec![
            p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 1., 0.),
            p(0., 0., 1.), p(1., 0., 1.), p(1., 1., 1.), p(0., 1., 1.),
        ];
        let faces = vec![
            vec![0, 3, 2, 1], vec![4, 5, 6, 7], vec![0, 1, 5, 4],
            vec![2, 3, 7, 6], vec![0, 4, 7, 3], vec![1, 2, 6, 5],
        ];
        let areas = vec![
            p(0., 0., -1.), p(0., 0., 1.), p(0., -1., 0.),
            p(0., 1., 0.), p(-1., 0., 0.), p(1., 0., 0.),
        ];
        let centres = vec![
            p(0.5, 0.5, 0.), p(0.5, 0.5, 1.), p(0.5, 0., 0.5),
            p(0.5, 1., 0.5), p(0., 0.5, 0.5), p(1., 0.5, 0.5),
        ];
        let geom = CheckGeometry {
            face_areas: areas,
            face_centres: centres,
            cell_centres: vec![p(0.5, 0.5, 0.5)],
            cell_volumes: vec![1.0],
        };
        (points, faces, geom)
    }

    /// V&V (gh:#1480) — a unit cube is ideal under every metric and passes
    /// the full `meshQualityControls` gate. Methodology: closed-form cube
    /// geometry; expected cell determinant 1, aspect ratio 1 by both
    /// definitions, boundary skewness 0 by both definitions, pyramid volume
    /// 1/6, no failing face. Result (2026-11-11): all exact to 1e-12.
    #[test]
    fn unit_cube_is_ideal() {
        let (points, faces, geom) = unit_cube();
        let owner = vec![0usize; 6];
        let nb: Vec<Option<usize>> = vec![None; 6];
        for kind in [BoundarySkewness::NormalProjection, BoundarySkewness::Mirrored] {
            let opts = CheckOptions { boundary_skewness: kind, ..CheckOptions::default() };
            let q = check_quality(&points, &faces, &owner, &nb, &geom, &opts);
            assert!(q.max_boundary_skewness < 1e-12, "{kind:?}: {}", q.max_boundary_skewness);
            assert!((q.max_aspect_ratio - 1.0).abs() < 1e-12);
            assert!((q.max_aspect_ratio_cmpt - 1.0).abs() < 1e-12);
            assert!((q.min_cell_determinant - 1.0).abs() < 1e-12);
            assert!((q.min_face_pyramid_volume - 1.0 / 6.0).abs() < 1e-12);
            assert!((q.total_volume - 1.0).abs() < 1e-12);
            assert_eq!(q.n_negative_volume_cells, 0);
            assert_eq!(q.n_negative_pyramid_faces, 0);
        }
        // A lone cell has no internal faces, so `cellDeterminant` (internal
        // faces only) would fail it; that check is disabled here.
        let controls = MeshQualityControls { min_determinant: -1.0, ..MeshQualityControls::default() };
        let bad = faces_failing_mesh_quality(&points, &faces, &owner, &nb, &geom, &controls);
        assert!(bad.iter().all(|&b| !b), "{bad:?}");
    }

    /// A face whose neighbour centre is inverted fails the gate and is
    /// non-orthogonal beyond the limit; a degenerate face is skipped by
    /// [`face_non_orthogonality_deg`].
    #[test]
    fn inverted_pair_fails_and_degenerate_face_is_skipped() {
        let z = Vector3::ZERO;
        assert_eq!(face_non_orthogonality_deg(z, Vector3::new(1., 0., 0.)), None);
        assert_eq!(face_non_orthogonality_deg(Vector3::new(1., 0., 0.), z), None);
        let a = face_non_orthogonality_deg(Vector3::new(1., 0., 0.), Vector3::new(1., 0., 0.)).unwrap();
        assert!(a.abs() < 1e-12);
        let b = face_non_orthogonality_deg(Vector3::new(0., 1., 0.), Vector3::new(1., 0., 0.)).unwrap();
        assert!((b - 90.0).abs() < 1e-12);
    }
}
