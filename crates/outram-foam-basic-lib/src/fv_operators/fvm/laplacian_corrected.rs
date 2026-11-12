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

//! Non-orthogonality-corrected Laplacian — `Foam::gaussLaplacianScheme` with a
//! `corrected` / `limited` surface-normal-gradient scheme.
//!
//! **Two operators live here (gh:#1362, 2026-11-11).** [`laplacian_corrected`] is
//! upstream's `corrected` / `limited corrected` scheme, bit-identical to ESI
//! v2506 on two meshes. [`laplacian_corrected_face_centre`] is the port's
//! original formulation described in the rest of this module doc (0.5-weighted
//! face gradient, Dirichlet boundary correction, `Limited(w)` = scale by `w`):
//! it is NOT upstream's scheme, but recovers a linear field exactly on any mesh,
//! so [`solve_laplacian_non_orthogonal`] and `tests/non_orthogonal_laplacian.rs`
//! use it. Where the text below says `Limited` or "OpenFOAM's `limited`", it
//! describes the face-centre operator; ~~"OpenFOAM's `limited <coeff>`"~~
//! **CORRECTED**: upstream's limiter is `min(w|snGrad|/((1-w)|corr| + SMALL), 1)`.
//!
//! Translated from
//! `src/finiteVolume/finiteVolume/laplacianSchemes/gaussLaplacianScheme/`,
//! `.../snGradSchemes/correctedSnGrad/`, and the non-orthogonal geometry in
//! `src/finiteVolume/interpolation/surfaceInterpolation/surfaceInterpolation.C`.
//!
//! # The problem this solves
//!
//! [`fvm::laplacian`](super::laplacian) discretises the face flux as
//!
//! ```text
//!     Gamma_f |Sf| (phi_N - phi_P) / |d| ,      d = C_N - C_P
//! ```
//!
//! which silently assumes `Sf` is parallel to `d` — i.e. an **orthogonal** mesh.
//! On a non-orthogonal mesh (any tetrahedral, polyhedral or sheared grid) the
//! angle between the face normal and the cell-to-cell vector is not zero, and
//! that formula is not even exact for a *linear* field: the operator loses a
//! full order of accuracy, with no warning. Meshes with a maximum
//! non-orthogonality above ~70 degrees — routine for a polyhedral dual mesh —
//! make the error first-order and large.
//!
//! # The correction
//!
//! Following OpenFOAM, split the face normal into a part parallel to `d` and a
//! remainder, and treat the first implicitly and the second explicitly
//! (**deferred correction**):
//!
//! ```text
//!     snGrad(phi)_f = Delta_f (phi_N - phi_P)  +  k_f . (grad phi)_f
//! ```
//!
//! with `n = Sf/|Sf|`, the non-orthogonal delta coefficient
//! `Delta_f = 1 / max(n . d, 0.05 |d|)` and the correction vector
//! `k_f = n - d Delta_f`. On an orthogonal mesh `n . d = |d|`, so
//! `Delta_f = 1/|d|` and `k_f = 0` — the scheme reduces exactly to the
//! uncorrected one. For a **linear** field `phi = a . x` the correction is exact
//! for any mesh: `phi_N - phi_P = a . d`, so
//! `snGrad = (a.d)/(n.d) + (n - d/(n.d)) . a = n . a`.
//!
//! The `0.05 |d|` floor is OpenFOAM's guard against a division by zero at 90
//! degrees of non-orthogonality; it caps the implicit coefficient at `20/|d|`.
//!
//! # Deferred correction is iterative
//!
//! `(grad phi)_f` is built from the *current* `phi`, so the correction lags the
//! solution. Recovering the corrected answer needs **non-orthogonal corrector
//! iterations** — assemble, solve, recompute the gradient, re-assemble — exactly
//! as OpenFOAM's `nNonOrthogonalCorrectors` does. On a mesh of moderate
//! non-orthogonality two or three passes suffice; see
//! `tests/non_orthogonal_laplacian.rs` for measured convergence.
//!
//! # Scope and limitations
//!
//! - **Internal faces and pure-Dirichlet boundary faces only.** The
//!   `FixedValue` / `FixedField` / `NoSlip` patch arms are corrected by the same
//!   construction; `ZeroGradient` / `Symmetry` carry no flux and need none. All
//!   other boundary-condition arms (mixed/Robin, `fixedGradient`, the
//!   flux-switched inlet-outlet family, `totalPressure`,
//!   `fixedFluxPressure`, ...) and **all cyclic / cyclicAMI seam couplings**
//!   keep the orthogonal treatment inherited from
//!   [`fvm::laplacian`](super::laplacian), unchanged. Correcting a periodic seam
//!   needs the partner cell's gradient across the transform, which is not
//!   implemented. Do not rely on this operator to correct non-orthogonality at a
//!   periodic seam or on those patch types.
//! - **Not skewness correction.** Non-orthogonality (`Sf` not parallel to `d`)
//!   and skewness (the face centre not lying on the segment `C_P C_N`) are
//!   distinct mesh defects. This corrects the former only.
//! - **No human V&V.** The measured results in
//!   `tests/non_orthogonal_laplacian.rs` are verification against an analytic
//!   field, not validation against an external code or experiment.

use petir::mathf::RealMath;
use crate::fields::boundary::bc::BoundaryCondition;
use crate::fields::surface_field::SurfaceScalarField;
use crate::mesh::fv_mesh::PatchKind;
use crate::fields::vol_field::{VolScalarField, VolVectorField};
use crate::fv_operators::fvc::grad_least_squares_face_centre;
use crate::ldu_matrix::fv_matrix::FvMatrix;
use crate::primitives::Vector3;

/// OpenFOAM's floor on `n . d` as a fraction of `|d|`, guarding the division at
/// extreme non-orthogonality (`surfaceInterpolation::makeNonOrthDeltaCoeffs`).
///
/// At 90 degrees of non-orthogonality `n . d` would be zero; this caps the
/// implicit delta coefficient at `20/|d|` and pushes the rest of the flux into
/// the explicit correction.
const NON_ORTH_FLOOR: f64 = 0.05;

/// How much of the non-orthogonal correction to apply, chosen by enum
/// (workspace rule: no trait objects).
///
/// The variants correspond one-to-one to OpenFOAM's `snGradSchemes` entries.
/// All are dimensionless selectors carrying no state.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum NonOrthoScheme {
    /// No correction — bit-for-bit the behaviour of
    /// [`fvm::laplacian`](super::laplacian): `Delta_f = 1/|d|`, `k_f = 0`.
    /// Correct only on an orthogonal mesh. This is the **default** so that
    /// nothing silently changes for existing callers.
    #[default]
    Orthogonal,
    /// Full correction — OpenFOAM's `corrected`. Exact for a linear field on any
    /// mesh (given a converged corrector loop), but the explicit term is
    /// unbounded, so on a very poor mesh it can destabilise the outer iteration.
    Corrected,
    /// Correction scaled by `limiter` in `[0, 1]` — OpenFOAM's
    /// `limited <coeff>`. `1.0` is [`Corrected`](Self::Corrected), `0.0` is
    /// [`Orthogonal`](Self::Orthogonal), and the common practical setting is
    /// `0.33`-`0.5` on a mesh whose non-orthogonality exceeds ~70 degrees:
    /// it trades some accuracy for a bounded, stable explicit term. Values
    /// outside `[0, 1]` are clamped.
    Limited(f64),
}

impl NonOrthoScheme {
    /// The fraction of the non-orthogonal correction this scheme applies, in
    /// `[0, 1]` (dimensionless).
    pub fn correction_fraction(self) -> f64 {
        match self {
            NonOrthoScheme::Orthogonal => 0.0,
            NonOrthoScheme::Corrected => 1.0,
            NonOrthoScheme::Limited(w) => w.clamp(0.0, 1.0),
        }
    }
}

/// Per-face non-orthogonal geometry: the implicit delta coefficient and the
/// explicit correction vector.
///
/// `delta_coeff` has units `[1/m]`; `corr_vec` is dimensionless (it is a
/// difference of unit vectors and a `d`-scaled term).
#[derive(Debug, Clone, Copy)]
pub struct NonOrthoGeometry {
    /// `Delta_f = 1 / max(n . d, 0.05 |d|)` — the coefficient multiplying
    /// `(phi_N - phi_P)` in the implicit part `[1/m]`.
    pub delta_coeff: f64,
    /// `k_f = n - d Delta_f` — the vector dotted into the face gradient to form
    /// the explicit correction (dimensionless).
    pub corr_vec: Vector3,
    /// Non-orthogonality angle between the face normal `n` and `d`, in
    /// **degrees**, `0` (orthogonal) to `90`. This is the same quantity
    /// `checkMesh` reports as "max non-orthogonality".
    pub angle_deg: f64,
}

/// Compute the non-orthogonal geometry of a face from its area vector `sf`
/// `[m^2]` and its owner-to-neighbour vector `d` `[m]`.
///
/// Returns `None` for a degenerate face (zero area or coincident cell centres).
///
/// **Upstream form (gh:#1362, maintainer approval 2026-11-12).** The unit normal is
/// upstream's `unitArea = Sf/magSf` (componentwise divide), so `corr_vec` is
/// bit-identical to `fvc::non_orth_correction_vectors` and OpenFOAM's
/// `nonOrthCorrectionVectors` (differential row `nonOrthCorrVec`). ~~The unit
/// normal was `Sf*(1/magSf)`~~ **CHANGED 2026-11-12**: that previous form is kept as
/// [`non_ortho_geometry_port_original`]. The change moves results at the ulp level;
/// the moved numbers are recorded in
/// `verification_and_validation/upstream_ops_gh1362/README.md`.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::fv_operators::fvm::non_ortho_geometry;
/// use outram_foam_basic_lib::primitives::Vector3;
///
/// // Perfectly orthogonal face: Sf parallel to d.
/// let g = non_ortho_geometry(Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.5, 0.0, 0.0))
///     .unwrap();
/// assert!((g.delta_coeff - 2.0).abs() < 1e-12); // 1/|d| = 1/0.5
/// assert!(g.corr_vec.mag() < 1e-12);            // no correction needed
/// assert!(g.angle_deg < 1e-9);
/// ```
pub fn non_ortho_geometry(sf: Vector3, d: Vector3) -> Option<NonOrthoGeometry> {
    let mag_sf = sf.mag();
    if mag_sf < 1.0e-300 {
        return None;
    }
    // upstream `unitArea = Sf/magSf` (divide, not `Sf*(1/magSf)`)
    let n = Vector3::new(sf.x / mag_sf, sf.y / mag_sf, sf.z / mag_sf);
    non_ortho_geometry_from_unit_normal(n, d)
}

/// The **previous** form of [`non_ortho_geometry`], kept callable (gh:#1362,
/// maintainer decision 2026-11-12): identical except that the unit normal is
/// `Sf*(1/magSf)` (multiply by the reciprocal) instead of upstream's `Sf/magSf`.
/// It differs from upstream's `nonOrthCorrectionVectors` by an ulp or two on a
/// third to a half of the faces (hex 668 / 2367, polyDual 1069 / 3708 before the
/// switch). Not the default; use [`non_ortho_geometry`] unless reproducing a
/// result computed with the old form.
pub fn non_ortho_geometry_port_original(sf: Vector3, d: Vector3) -> Option<NonOrthoGeometry> {
    let mag_sf = sf.mag();
    if mag_sf < 1.0e-300 {
        return None;
    }
    let n = sf * (1.0 / mag_sf);
    non_ortho_geometry_from_unit_normal(n, d)
}

/// Shared tail of the two geometries: everything after the unit normal.
fn non_ortho_geometry_from_unit_normal(n: Vector3, d: Vector3) -> Option<NonOrthoGeometry> {
    let mag_d = d.mag();
    if mag_d < 1.0e-300 {
        return None;
    }
    let n_dot_d = n.dot(d);
    let cos_theta = (n_dot_d / mag_d).clamp(-1.0, 1.0);
    let angle_deg = cos_theta.r_acos().to_degrees();
    let delta_coeff = 1.0 / n_dot_d.max(NON_ORTH_FLOOR * mag_d);
    let corr_vec = n - d * delta_coeff;
    Some(NonOrthoGeometry {
        delta_coeff,
        corr_vec,
        angle_deg,
    })
}

/// Maximum internal-face non-orthogonality of a mesh, in **degrees**.
///
/// The angle between each internal face's normal and the vector joining the two
/// cell centres it separates; `0` is a perfectly orthogonal mesh. This is the
/// same statistic OpenFOAM's `checkMesh` prints as "Max non-orthogonality", and
/// the threshold conventionally quoted for concern is **70 degrees**. Returns
/// `0.0` for a mesh with no internal faces.
///
/// Use it to decide whether the correction is worth paying for, and to record an
/// honest mesh-quality figure alongside any result computed on that mesh.
pub fn max_non_orthogonality_deg(mesh: &crate::mesh::fv_mesh::FvMesh) -> f64 {
    let mut worst = 0.0_f64;
    for f in 0..mesh.n_internal_faces {
        let d = mesh.cell_centres[mesh.neighbour[f]] - mesh.cell_centres[mesh.owner[f]];
        if let Some(g) = non_ortho_geometry(mesh.face_area_vectors[f], d) {
            worst = worst.max(g.angle_deg);
        }
    }
    worst
}

/// Implicit Laplacian with an explicit non-orthogonal correction, using a
/// caller-supplied cell gradient — `fvm::laplacian(Gamma, phi)` with a
/// `corrected` snGrad scheme.
///
/// Sign convention matches [`fvm::laplacian`](super::laplacian): the returned
/// matrix is the **positive-definite** operator `-div(Gamma grad phi)` (positive
/// diagonal, negative off-diagonals), which is the opposite sign to OpenFOAM's
/// `fvm::laplacian`. With `scheme = NonOrthoScheme::Orthogonal` the result is
/// identical to [`fvm::laplacian`](super::laplacian) for every mesh.
///
/// # Which Laplacian: this one or the face-centre one (maintainer decision 2026-11-12, gh:#1336 / gh:#1362)
///
/// Both operators are kept; the choice between them is **left open for a
/// future study**.
///
/// - **This function is the upstream translation: the default, and
///   bit-identical to OpenFOAM** (ESI v2506 `gaussLaplacianScheme` with
///   `corrected` / `limited corrected`, on two meshes; differential rows in
///   `tests/operators_vs_openfoam.rs`).
/// - **[`laplacian_corrected_face_centre`] (port-original) is the more accurate
///   one**, because it also corrects the non-coupled boundary faces (a
///   Dirichlet boundary correction) and so recovers a linear field exactly at
///   non-orthogonal boundary cells. Upstream's corrected Laplacian does not
///   correct those faces. On the wavy mesh at `amp = 0.3` (10.5 deg) this one
///   reaches **1.264e-3** where the face-centre form, driven by
///   [`grad_least_squares_face_centre`](crate::fv_operators::fvc::grad_least_squares_face_centre),
///   reaches **~7e-12** (`tests/non_orthogonal_laplacian.rs`, 6.981e-12 in
///   its recorded table; upstream figure in
///   `verification_and_validation/upstream_ops_gh1362/README.md`). The upstream
///   figure is OpenFOAM's faithful behaviour, not a defect of this port.
///
/// # Arguments
/// - `gamma` — face diffusivity `[m^2/s]` for scalar transport, or thermal
///   conductivity `[W/(m.K)]` for conduction; whatever units make
///   `Gamma * grad(phi) * area` the flux the caller wants.
/// - `phi` — the transported scalar; its boundary conditions set the matrix's
///   boundary contributions exactly as in [`fvm::laplacian`](super::laplacian).
/// - `grad_phi` — the **current** cell gradient of `phi` `[phi/m]`, used only
///   for the explicit correction. Pass a least-squares gradient
///   ([`fvc::grad_least_squares`](crate::fv_operators::fvc::grad_least_squares))
///   on a non-orthogonal mesh: a Gauss gradient is itself inconsistent there, so
///   feeding it in caps the achievable accuracy.
/// - `scheme` — how much correction to apply.
///
/// # Face-gradient interpolation
///
/// ~~`(grad phi)_f` is the arithmetic mean of the two adjacent cell gradients on
/// an internal face, and the owner's gradient on a boundary face.~~ **CORRECTED
/// 2026-11-12** (that is [`laplacian_corrected_face_centre`]; this function is
/// upstream's `correctedSnGrad::fullGradCorrection`): internal faces use the
/// linear-weighted gradient `w (g_P - g_N) + g_N` dotted into the non-orthogonal
/// correction vector, and non-coupled boundary faces carry **no** correction
/// (upstream's correction vectors there are zero). Read from
/// `fvc::sn_grad_correction_with_grad`, which this calls.
///
/// # Iteration
///
/// This applies the correction **once**, from the gradient you pass in. To
/// actually converge the correction, loop: solve, recompute `grad_phi` from the
/// new solution, re-assemble, solve again. See
/// [`solve_laplacian_non_orthogonal`] for that loop packaged up.
pub fn laplacian_corrected(
    gamma: &SurfaceScalarField,
    phi: &VolScalarField,
    grad_phi: &VolVectorField,
    scheme: NonOrthoScheme,
) -> FvMatrix {
    let mesh = phi.mesh.clone();
    // Upstream `gaussLaplacianScheme::fvmLaplacian` (ESI v2506), `corrected` /
    // `limited corrected`: the implicit part is `fvmLaplacianUncorrected` with
    // `nonOrthDeltaCoeffs`; the explicit part is the face flux
    // `faceFluxCorrection = gamma*magSf*snGradCorrection` and its divergence goes
    // to the source, `source_ = -V*div(faceFluxCorrection)` (this matrix has the
    // opposite sign, so `+V*div`), BEFORE the boundary coefficients are added.
    // ~~Delta-coefficient increments on a matrix built with `1/|d|`, a
    // 0.5-weighted face gradient, the correction added face by face to the
    // source, and a Dirichlet boundary correction upstream does not have
    // (its non-coupled `nonOrthCorrectionVectors` are zero)~~ **CHANGED
    // 2026-11-11 (gh:#1362)**: bit-identical to upstream on the #1362 meshes.
    if scheme == NonOrthoScheme::Orthogonal {
        return super::laplacian(gamma, phi);
    }
    let corr = crate::fv_operators::fvc::sn_grad_correction_with_grad(phi, grad_phi, scheme);
    let mut sum = vec![0.0_f64; mesh.n_cells];
    for f in 0..mesh.n_internal_faces {
        let ffc = (gamma.internal[f] * mesh.face_areas[f]) * corr[f];
        sum[mesh.owner[f]] += ffc;
        sum[mesh.neighbour[f]] -= ffc;
    }
    // non-coupled boundary faces: the correction vector is zero, so the flux is 0
    let source0: Vec<f64> = (0..mesh.n_cells)
        .map(|c| mesh.cell_volumes[c] * (sum[c] / mesh.cell_volumes[c]))
        .collect();
    super::laplacian_with_delta_and_source(gamma, phi, super::DeltaCoeff::NonOrthogonal, Some(&source0))
}

/// **Port-original** corrected Laplacian (NOT upstream's `gaussLaplacianScheme`
/// `corrected`; see [`laplacian_corrected`] for that), kept because it recovers a
/// linear field exactly on any mesh when driven by
/// [`grad_least_squares_face_centre`](crate::fv_operators::fvc::grad_least_squares_face_centre),
/// the property `tests/non_orthogonal_laplacian.rs` verifies and
/// [`solve_laplacian_non_orthogonal`] uses (gh:#1362).
///
/// Differences from upstream: a 0.5-weighted mean face gradient, the explicit term
/// added face by face to the source, a Dirichlet boundary correction (upstream's
/// non-coupled correction vectors are zero), and `Limited(w)` scaling the
/// correction by `w` (upstream's `limited corrected w` uses the limiter
/// `min(w|snGrad|/((1-w)|corr| + SMALL), 1)`).
///
/// **Which one to use (maintainer decision 2026-11-12, gh:#1336 / gh:#1362):** both
/// are kept and the choice is left open for a future study. This is the **more
/// accurate** of the pair at non-orthogonal boundary cells (~7e-12 against
/// 1.264e-3 for [`laplacian_corrected`] on the `amp = 0.3` wavy mesh) because it
/// corrects the non-coupled boundary faces, which upstream does not. It is NOT
/// the default and NOT bit-identical to OpenFOAM; [`laplacian_corrected`] is the
/// upstream translation and the default. Measured in
/// `tests/non_orthogonal_laplacian.rs` and
/// `verification_and_validation/upstream_ops_gh1362/README.md`.
///
/// Original description follows.
///
/// Implicit Laplacian with an explicit non-orthogonal correction, using a
/// caller-supplied cell gradient — `fvm::laplacian(Gamma, phi)` with a
/// `corrected` snGrad scheme.
///
/// Sign convention matches [`fvm::laplacian`](super::laplacian): the returned
/// matrix is the **positive-definite** operator `-div(Gamma grad phi)` (positive
/// diagonal, negative off-diagonals), which is the opposite sign to OpenFOAM's
/// `fvm::laplacian`. With `scheme = NonOrthoScheme::Orthogonal` the result is
/// identical to [`fvm::laplacian`](super::laplacian) for every mesh.
///
/// # Arguments
/// - `gamma` — face diffusivity `[m^2/s]` for scalar transport, or thermal
///   conductivity `[W/(m.K)]` for conduction; whatever units make
///   `Gamma * grad(phi) * area` the flux the caller wants.
/// - `phi` — the transported scalar; its boundary conditions set the matrix's
///   boundary contributions exactly as in [`fvm::laplacian`](super::laplacian).
/// - `grad_phi` — the **current** cell gradient of `phi` `[phi/m]`, used only
///   for the explicit correction. Pass a least-squares gradient
///   ([`fvc::grad_least_squares`](crate::fv_operators::fvc::grad_least_squares))
///   on a non-orthogonal mesh: a Gauss gradient is itself inconsistent there, so
///   feeding it in caps the achievable accuracy.
/// - `scheme` — how much correction to apply.
///
/// # Face-gradient interpolation
///
/// `(grad phi)_f` is the arithmetic mean of the two adjacent cell gradients on
/// an internal face, and the owner's gradient on a boundary face. This is
/// OpenFOAM's `linear` interpolation of the gradient at the equal-weight limit;
/// distance weighting is not applied, and on a strongly graded mesh that is a
/// known (documented, uncorrected) approximation.
///
/// # Iteration
///
/// This applies the correction **once**, from the gradient you pass in. To
/// actually converge the correction, loop: solve, recompute `grad_phi` from the
/// new solution, re-assemble, solve again. See
/// [`solve_laplacian_non_orthogonal`] for that loop packaged up.
pub fn laplacian_corrected_face_centre(
    gamma: &SurfaceScalarField,
    phi: &VolScalarField,
    grad_phi: &VolVectorField,
    scheme: NonOrthoScheme,
) -> FvMatrix {
    let mesh = phi.mesh.clone();
    // Start from the existing operator so that boundary conditions, cyclic
    // seams and AMI couplings keep exactly their current (orthogonal) treatment
    // — this function only *adds* the internal-face non-orthogonal correction.
    let mut mat = super::laplacian(gamma, phi);
    let w = scheme.correction_fraction();

    for f in 0..mesh.n_internal_faces {
        let o = mesh.owner[f];
        let nb = mesh.neighbour[f];
        let d = mesh.cell_centres[nb] - mesh.cell_centres[o];
        let Some(g) = non_ortho_geometry(mesh.face_area_vectors[f], d) else {
            continue;
        };
        let gamma_a = gamma.internal[f] * mesh.face_areas[f];

        // --- implicit part: replace 1/|d| by the non-orthogonal Delta_f ---
        //
        // `super::laplacian` already stamped `coeff_orth = Gamma_f |Sf| / |d|`.
        // Undo it and stamp `coeff_corr = Gamma_f |Sf| Delta_f` instead. The
        // implicit coefficient is only switched when a correction is actually
        // being applied, so `Orthogonal` reproduces the old matrix bit-for-bit.
        if w > 0.0 {
            let coeff_orth = gamma_a / d.mag();
            let coeff_corr = gamma_a * g.delta_coeff;
            let delta = coeff_corr - coeff_orth;
            mat.ldu.diag[o] += delta;
            mat.ldu.diag[nb] += delta;
            mat.ldu.upper[f] -= delta;
            mat.ldu.lower[f] -= delta;

            // --- explicit part: Gamma_f |Sf| (k_f . (grad phi)_f) ---
            //
            // The matrix is the positive-definite `-div(Gamma grad phi)`, so a
            // flux *into* the owner appears on the right-hand side with a plus
            // sign for the owner and a minus for the neighbour.
            let grad_f = (grad_phi.internal[o] + grad_phi.internal[nb]) * 0.5;
            let corr = w * gamma_a * g.corr_vec.dot(grad_f);
            mat.source[o] += corr;
            mat.source[nb] -= corr;
        }
    }

    // --- Dirichlet boundary faces ---
    //
    // `super::laplacian` stamps `coeff_orth = Gamma_b |Sf| / |Cf - C_P|` on the
    // diagonal (plus `coeff_orth * value` on the source) for the pure Dirichlet
    // arms. That divides by the *full* distance to the face centre, which is the
    // same orthogonality assumption the internal faces make and is equally wrong
    // on a non-orthogonal mesh — leaving it uncorrected caps the accuracy of the
    // whole operator, so it is corrected here by the same construction.
    //
    // Only the pure-Dirichlet arms are touched. ZeroGradient/Symmetry carry no
    // flux and need no correction; the flux-switched, mixed and
    // pressure-specific BCs keep their existing orthogonal treatment (see the
    // module-level "Scope and limitations").
    if w > 0.0 {
        for (pi, patch) in mesh.patches.iter().enumerate() {
            if patch.kind == PatchKind::Cyclic || patch.kind == PatchKind::CyclicAmi {
                continue;
            }
            for fi in 0..patch.size {
                let gf = patch.start + fi;
                let owner = mesh.owner[gf];
                let d_b = mesh.face_centres[gf] - mesh.cell_centres[owner];
                let Some(g) = non_ortho_geometry(mesh.face_area_vectors[gf], d_b) else {
                    continue;
                };
                let gamma_a = gamma.boundary[pi].values[fi] * mesh.face_areas[gf];
                let coeff_orth = gamma_a / super::DeltaCoeff::Orthogonal.boundary_delta(&mesh, gf); // gh:#1488: the distance the uncorrected Laplacian used
                let coeff_corr = gamma_a * g.delta_coeff;
                let delta = coeff_corr - coeff_orth;

                // The Dirichlet value this patch imposes, if any.
                let value = match &phi.boundary[pi].bc {
                    BoundaryCondition::FixedValue(v) => Some(*v),
                    BoundaryCondition::FixedField(ff) => Some(ff[fi]),
                    BoundaryCondition::NoSlip => Some(0.0),
                    _ => None,
                };
                let Some(v) = value else { continue };

                mat.ldu.diag[owner] += delta;
                mat.source[owner] += delta * v;
                mat.source[owner] += w * gamma_a * g.corr_vec.dot(grad_phi.internal[owner]);
            }
        }
    }

    mat
}

/// Solve `-div(Gamma grad phi) = 0` with a **non-orthogonal corrector loop** —
/// the packaged form of OpenFOAM's `nNonOrthogonalCorrectors`.
///
/// # What it does
///
/// 1. Assemble and solve the uncorrected system to get a first `phi`.
/// 2. Repeat `n_correctors` times: recompute the least-squares gradient of the
///    current `phi`, re-assemble [`laplacian_corrected`] with it, and re-solve.
///
/// Each pass is a full linear solve; the correction converges geometrically for
/// a mesh of moderate non-orthogonality and can stall or diverge on a very poor
/// one, which is exactly why [`NonOrthoScheme::Limited`] exists.
///
/// # Arguments
/// - `gamma` — face diffusivity, units as in [`laplacian_corrected`].
/// - `phi` — supplies the mesh, the field name, and the **boundary conditions**;
///   its internal values are used only as the first gradient's input and are not
///   otherwise read.
/// - `scheme` — correction strength. [`NonOrthoScheme::Orthogonal`] makes this
///   equivalent to a single uncorrected solve, whatever `n_correctors` says.
/// - `n_correctors` — number of correction passes after the first solve. `0`
///   reproduces the uncorrected result. `2` is OpenFOAM's usual default for a
///   mesh of moderate non-orthogonality.
/// - `settings` — linear-solver tolerance and iteration cap, passed to
///   [`FvMatrix::solve_bicgstab`](crate::ldu_matrix::FvMatrix::solve_bicgstab).
///
/// # Returns
///
/// The corrected field and the [`SolverPerformance`](crate::ldu_matrix::SolverPerformance)
/// of the **final** linear solve only — it reports nothing about whether the
/// non-orthogonal correction loop itself converged. Check that by comparing
/// successive fields if it matters.
pub fn solve_laplacian_non_orthogonal(
    gamma: &SurfaceScalarField,
    phi: &VolScalarField,
    scheme: NonOrthoScheme,
    n_correctors: usize,
    settings: crate::ldu_matrix::SolverSettings,
) -> (VolScalarField, crate::ldu_matrix::SolverPerformance) {
    use crate::ldu_matrix::KrylovOptions;

    // Pass 0: the uncorrected system.
    let mat = super::laplacian(gamma, phi);
    let (mut current, mut perf) =
        mat.solve_bicgstab(phi.name.as_str(), KrylovOptions::default(), settings);
    // Carry the caller's boundary conditions onto the working field so that the
    // gradient and the re-assembled matrix see the same BCs each pass.
    current.boundary = phi.boundary.clone();

    for _ in 0..n_correctors {
        let g = grad_least_squares_face_centre(&current);
        let mat = laplacian_corrected_face_centre(gamma, &current, &g, scheme);
        let (next, p) = mat.solve_bicgstab(phi.name.as_str(), KrylovOptions::default(), settings);
        current.internal = next.internal;
        perf = p;
    }
    (current, perf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fv_operators::fvc::grad_least_squares;
    use crate::fields::field::Field;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
    use std::sync::Arc;

    /// Orthogonal two-cell mesh — the correction must be a no-op here.
    fn ortho_mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(2)
                .n_internal_faces(1)
                .owner(vec![0, 1, 0])
                .neighbour(vec![1])
                .patches(vec![
                    BoundaryPatch::new("right", 1, 1, PatchKind::Wall),
                    BoundaryPatch::new("left", 2, 1, PatchKind::Wall),
                ])
                .cell_volumes(vec![0.5, 0.5])
                .cell_centres(vec![
                    Vector3::new(0.25, 0.0, 0.0),
                    Vector3::new(0.75, 0.0, 0.0),
                ])
                .face_area_vectors(vec![
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(-1.0, 0.0, 0.0),
                ])
                .face_centres(vec![
                    Vector3::new(0.5, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(0.0, 0.0, 0.0),
                ])
                .build()
                .unwrap(),
        )
    }

    #[test]
    fn orthogonal_scheme_reproduces_the_uncorrected_matrix() {
        let mesh = ortho_mesh();
        let phi = VolScalarField::new(
            "T",
            mesh.clone(),
            Field::new(vec![1.0, 2.0]),
            mesh.patches
                .iter()
                .map(|p| crate::fields::boundary::bc::PatchField::zero_gradient(p.size))
                .collect(),
        );
        let gamma = SurfaceScalarField::uniform("Gamma", mesh.clone(), 1.0);
        let g = grad_least_squares(&phi);

        let a = super::super::laplacian(&gamma, &phi);
        let b = laplacian_corrected(&gamma, &phi, &g, NonOrthoScheme::Orthogonal);
        assert_eq!(a.ldu.diag, b.ldu.diag);
        assert_eq!(a.ldu.upper, b.ldu.upper);
        assert_eq!(a.ldu.lower, b.ldu.lower);
        for c in 0..mesh.n_cells {
            assert_eq!(a.source[c], b.source[c]);
        }
    }

    #[test]
    fn correction_is_a_no_op_on_an_orthogonal_mesh() {
        // Sf parallel to d everywhere: the correction vector is zero, so even
        // the fully `Corrected` scheme must leave the matrix unchanged.
        let mesh = ortho_mesh();
        assert!(max_non_orthogonality_deg(&mesh) < 1e-9);
        let phi = VolScalarField::new(
            "T",
            mesh.clone(),
            Field::new(vec![1.0, 2.0]),
            mesh.patches
                .iter()
                .map(|p| crate::fields::boundary::bc::PatchField::zero_gradient(p.size))
                .collect(),
        );
        let gamma = SurfaceScalarField::uniform("Gamma", mesh.clone(), 1.0);
        let g = grad_least_squares(&phi);

        let a = super::super::laplacian(&gamma, &phi);
        let b = laplacian_corrected(&gamma, &phi, &g, NonOrthoScheme::Corrected);
        for i in 0..a.ldu.diag.len() {
            assert!((a.ldu.diag[i] - b.ldu.diag[i]).abs() < 1e-12);
        }
        for c in 0..mesh.n_cells {
            assert!((a.source[c] - b.source[c]).abs() < 1e-12);
        }
    }

    #[test]
    fn geometry_reports_the_expected_angle_and_reduces_when_orthogonal() {
        // 45-degree non-orthogonality: Sf along +x, d at 45 degrees.
        let sf = Vector3::new(1.0, 0.0, 0.0);
        let d = Vector3::new(1.0, 1.0, 0.0);
        let g = non_ortho_geometry(sf, d).unwrap();
        assert!((g.angle_deg - 45.0).abs() < 1e-9, "angle {}", g.angle_deg);
        // n . d = 1, so Delta = 1; k = n - d = (0, -1, 0).
        assert!((g.delta_coeff - 1.0).abs() < 1e-12);
        assert!((g.corr_vec - Vector3::new(0.0, -1.0, 0.0)).mag() < 1e-12);

        // Degenerate inputs return None rather than NaN.
        assert!(non_ortho_geometry(Vector3::ZERO, d).is_none());
        assert!(non_ortho_geometry(sf, Vector3::ZERO).is_none());
    }

    #[test]
    fn port_original_geometry_agrees_with_upstream_form_to_round_off() {
        // gh:#1362: the two unit-normal forms (`Sf/magSf` and `Sf*(1/magSf)`) differ by
        // at most an ulp or two; both refuse degenerate input.
        let sf = Vector3::new(0.3, -1.1, 0.7);
        let d = Vector3::new(0.9, 0.2, -0.4);
        let a = non_ortho_geometry(sf, d).unwrap();
        let b = non_ortho_geometry_port_original(sf, d).unwrap();
        assert!((a.corr_vec - b.corr_vec).mag() < 1e-14);
        assert!((a.delta_coeff - b.delta_coeff).abs() < 1e-14 * a.delta_coeff.abs());
        assert!((a.angle_deg - b.angle_deg).abs() < 1e-9);
        assert!(non_ortho_geometry_port_original(Vector3::ZERO, d).is_none());
        assert!(non_ortho_geometry_port_original(sf, Vector3::ZERO).is_none());
    }

    #[test]
    fn limited_scheme_interpolates_between_the_two_extremes() {
        assert_eq!(NonOrthoScheme::Orthogonal.correction_fraction(), 0.0);
        assert_eq!(NonOrthoScheme::Corrected.correction_fraction(), 1.0);
        assert_eq!(NonOrthoScheme::Limited(0.33).correction_fraction(), 0.33);
        // Out-of-range limiters are clamped, not rejected.
        assert_eq!(NonOrthoScheme::Limited(-1.0).correction_fraction(), 0.0);
        assert_eq!(NonOrthoScheme::Limited(7.0).correction_fraction(), 1.0);
    }
}
