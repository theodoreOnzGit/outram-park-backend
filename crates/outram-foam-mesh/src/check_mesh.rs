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
//! Where the three copies differed, the cfMesh formula is used and the
//! difference is recorded in
//! `verification_and_validation/check_mesh_1480/`:
//! non-orthogonality skips degenerate faces (`|d| <= 1e-30` or `|Sf| <= 1e-30`),
//! skewness multiplies by `1/(|sv|+guard)` where upstream divides, and the
//! `guard` is a parameter of [`face_skewness`] (cfMesh used `1e-300` in
//! [`check_quality`] and upstream's `rootVSmall = 1e-150` in
//! [`faces_failing_mesh_quality`]).
//! Boundary skewness has two definitions, selected explicitly by
//! [`BoundarySkewness`]: dev's `boundaryFaceSkewness` (normal projection, what
//! the cfMesh gate uses) and the mirrored-neighbour form `outram-foam-mesh`
//! used since before it ported dev. Which one snappyHexMesh's snapping gate
//! should use is a maintainer decision (changing it moves the gate).
//!
//! Pure Rust, wasm- and Android-safe.

use outram_foam_basic_lib::primitives::Vector3;
use petir::mathf::RealMath;

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
