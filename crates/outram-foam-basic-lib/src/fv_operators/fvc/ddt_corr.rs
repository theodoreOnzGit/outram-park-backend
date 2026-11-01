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

use super::interpolate;
use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
use crate::fields::field::Field;
use crate::fields::surface_field::SurfaceScalarField;
use crate::fields::vol_field::VolVectorField;

/// Guard in the `|phiCorr| / (|phi| + SMALL)` ratio of the `fvcDdtPhiCoeff`
/// limiter, shared by [`ddt_corr`] and [`ddt_corr_backward`] so the BDF2 form
/// reduces to the Euler form bit for bit on its start-up step.
///
/// ~~Matches OpenFOAM's SMALL guard in the |phiCorr|/|phi| ratio.~~
/// **CORRECTED 2026-11-01** (gh:#1067) — it does not match: OpenFOAM's
/// double-precision `small` (`doubleScalarSmall`) is `1e-15`, and `1e-37` is
/// the *single*-precision `floatScalarVSmall`. **Not re-checked** against a
/// vendored `doubleScalar.H` (none is vendored here; the value is from the
/// OpenFOAM source as known to the porting agent). The two only differ on a
/// face whose old flux is below ~1e-15 m³/s, where the limiter is ~0 either
/// way. Left at `1e-37` so this change does not move the Euler path; aligning
/// it is a separate, measurable change.
const SMALL: f64 = 1e-37;

/// Rhie–Chow time-derivative flux correction (Euler scheme), with OpenFOAM's
/// `fvcDdtPhiCoeff` limiter.
///
/// ```text
/// phiCorr[f] = phi_old[f] − (interpolate(U_old) · S_f)
/// coeff[f]   = 1 − min( |phiCorr[f]| / (|phi_old[f]| + SMALL),  1 )
/// ddtCorr[f] = coeff[f] · phiCorr[f] / Δt
/// ```
///
/// `phiCorr` is the discrepancy between the stored face flux (which carries the
/// pressure-driven, Rhie–Chow part of the velocity) and the flux you would get
/// by simply interpolating the cell velocity. Re-injecting it into `phiHbyA`
/// before the pressure solve (as `interpolate(rAU)·ddtCorr`) keeps the face
/// flux coupled to its own history, which is what suppresses pressure–velocity
/// (checkerboard) decoupling and lets the PISO/PIMPLE loop stay stable at
/// Courant numbers approaching 1. **Without the limiter** the raw `phiCorr/Δt`
/// can overshoot and is itself destabilising; the `coeff` blend (→ 0 where the
/// correction is large relative to the flux, → 1 where it is small) is the
/// stabilising ingredient.
///
/// The coefficient is zeroed on boundary faces — OpenFOAM zeros it on
/// fixed-value (and cyclicAMI) patches, and incompressible solvers re-impose the
/// boundary flux via `constrainHbyA`/`adjustPhi` regardless — so this returns a
/// zero boundary field.
///
/// # Euler only — pair it with an Euler ddt; use [`ddt_corr_backward`] for BDF2
///
/// ~~**This function hardcodes the first-order implicit-Euler correction
/// `phiCorr/Δt`.** It takes no ddt-scheme argument and there is no
/// `Backward`/BDF2 counterpart in this crate, so a caller that selects a
/// second-order backward ddt scheme gets a *mismatched* pair of terms.~~
/// **CORRECTED 2026-11-01** (gh:#1067) — the BDF2 counterpart now exists:
/// [`ddt_corr_backward`], a port of `backwardDdtScheme<Type>::fvcDdtPhiCorr`.
/// This function is still the Euler form only, and pairing it with a BDF2 ddt
/// remains a mismatch:
///
/// * A BDF2 ddt, `∂U/∂t ≈ (3U^n − 4U^{n−1} + U^{n−2})/(2Δt)`, puts **`1.5·V/Δt`**
///   on the momentum diagonal and `V(2U^{n−1} − ½U^{n−2})/Δt` in the source, so
///   the `H/A` a PISO loop forms carries `rAU·(2U^{n−1} − ½U^{n−2})/Δt`.
/// * This function corrects only the Euler history `rAU·U^{n−1}/Δt`, so the
///   face flux it re-injects does not match the history term in `H`.
///
/// Measured consequence when the two were paired
/// (`outram-foam-appbuilder-lib`, `tests/fv_scheme_selection.rs`, 2026-08-07):
/// Euler-vs-`Backward` lid-driven-cavity steady states differing by 1.0e-2 to
/// 2.9e-2 m/s (1–3 % of `U_lid`), with the gap *growing* as `Δt` is refined.
/// The temporal-order study that followed the fix
/// (`outram-foam-appbuilder-lib`, `tests/bdf2_temporal_order.rs`, 2026-11-01)
/// records the observed orders with each pairing.
///
/// # Upstream reference
///
/// OpenFOAM signature: `fvc::ddtCorr(U, phi)` (Euler; Δt from `runTime`). Here
/// `dt` is passed explicitly. Mirrors
/// `EulerDdtScheme<Type>::fvcDdtPhiCorr` + `ddtScheme<Type>::fvcDdtPhiCoeff`
/// (standard version, `ddtPhiCoeff_ < 0`) in
/// `src/finiteVolume/finiteVolume/ddtSchemes/`.
pub fn ddt_corr(
    u_old: &VolVectorField,
    phi_old: &SurfaceScalarField,
    dt: f64,
) -> SurfaceScalarField {
    let mesh = &u_old.mesh;
    let u_f = interpolate(u_old);
    let r_dt = 1.0 / dt;

    let internal = Field::from_fn(mesh.n_internal_faces, |f| {
        let u_dot_sf = u_f.internal[f].dot(mesh.face_area_vectors[f]);
        let phi_corr = phi_old.internal[f] - u_dot_sf;
        let coeff = 1.0 - (phi_corr.abs() / (phi_old.internal[f].abs() + SMALL)).min(1.0);
        coeff * phi_corr * r_dt
    });

    // coeff = 0 on boundaries (see doc note).
    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField {
            bc: BoundaryCondition::ZeroGradient,
            values: Field::zeros(p.size),
        })
        .collect();

    SurfaceScalarField::new("ddtCorr", phi_old.mesh.clone(), internal, boundary)
}

/// The three time-level weights of OpenFOAM's second-order backward (BDF2)
/// ddt scheme, for a current step `Δt` and a previous step `Δt₀`.
///
/// ```text
/// coefft   = 1 + Δt/(Δt + Δt₀)
/// coefft00 = Δt² / (Δt₀ (Δt + Δt₀))
/// coefft0  = coefft + coefft00
/// ∂ψ/∂t  ≈ (coefft·ψⁿ − coefft0·ψⁿ⁻¹ + coefft00·ψⁿ⁻²) / Δt
/// ```
///
/// For a constant step (`Δt₀ = Δt`) these are exactly `1.5`, `2` and `0.5`,
/// i.e. `(3ψⁿ − 4ψⁿ⁻¹ + ψⁿ⁻²)/(2Δt)`. The same three expressions open every
/// member function of upstream `backwardDdtScheme.C`
/// (`src/finiteVolume/finiteVolume/ddtSchemes/backwardDdtScheme/`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackwardDdtCoeffs {
    /// Weight of the new time level ψⁿ (dimensionless).
    pub coefft: f64,
    /// Weight of the old time level ψⁿ⁻¹ (dimensionless).
    pub coefft0: f64,
    /// Weight of the old-old time level ψⁿ⁻² (dimensionless).
    pub coefft00: f64,
}

impl BackwardDdtCoeffs {
    /// Weights for current step `dt` [s] and previous step `dt0` [s], both > 0.
    ///
    /// Operation order is upstream's, so a constant step gives the exact
    /// binary values `1.5`, `2.0`, `0.5`.
    pub fn new(dt: f64, dt0: f64) -> Self {
        let coefft = 1.0 + dt / (dt + dt0);
        let coefft00 = dt * dt / (dt0 * (dt + dt0));
        Self {
            coefft,
            coefft0: coefft + coefft00,
            coefft00,
        }
    }
}

/// Rhie–Chow time-derivative flux correction for the **second-order backward
/// (BDF2)** ddt scheme, with OpenFOAM's `fvcDdtPhiCoeff` limiter — the partner
/// [`ddt_corr`] lacks.
///
/// ```text
/// coeff[f]   = 1 − min( |phi_old[f] − S_f·interpolate(U_old)[f]| / (|phi_old[f]| + SMALL), 1 )
/// ddtCorr[f] = coeff[f] / Δt
///            · [ (coefft0·phi_old − coefft00·phi_old_old)[f]
///                − S_f · interpolate(coefft0·U_old − coefft00·U_old_old)[f] ]
/// ```
///
/// with the weights of [`BackwardDdtCoeffs::new`]`(dt, dt0)`.
///
/// # Why a BDF2 run needs this and not [`ddt_corr`]
///
/// A PISO/PIMPLE loop forms `phiHbyA = flux(H/A) + interpolate(rAU)·ddtCorr`.
/// With a BDF2 ddt, `H` carries the history source `V(coefft0·Uⁿ⁻¹ −
/// coefft00·Uⁿ⁻²)/Δt`, so `flux(H/A)` contains the *interpolated* history
/// `rAU_f·S_f·interpolate(coefft0·Uⁿ⁻¹ − coefft00·Uⁿ⁻²)/Δt`. This correction
/// swaps exactly that for the stored-flux history `rAU_f·(coefft0·φⁿ⁻¹ −
/// coefft00·φⁿ⁻²)/Δt`, which is what keeps the face flux coupled to its own
/// history at second order. [`ddt_corr`] swaps only the Euler history
/// `S_f·interpolate(Uⁿ⁻¹)` for `φⁿ⁻¹`, so paired with a BDF2 ddt it leaves the
/// `Uⁿ⁻²` part uncorrected and the `Uⁿ⁻¹` part corrected with the wrong weight.
///
/// Note the limiter `coeff` is formed from the **single** old level (`U_old`,
/// `phi_old`), not from the BDF2 combination — that is what upstream does
/// (`this->fvcDdtPhiCoeff(U.oldTime(), phi.oldTime())`).
///
/// # Start-up
///
/// If `u_old_old` or `phi_old_old` is `None` (the first step of a run), this
/// returns [`ddt_corr`]`(u_old, phi_old, dt)` exactly. Upstream reaches the
/// same place by setting `Δt₀ = great` when the field has fewer than two old
/// times (`backwardDdtScheme::deltaT0_(vf)`), which makes `coefft → 1`,
/// `coefft00 → 0` to within ~1e-15; returning the Euler form exactly keeps
/// this consistent with the exact-Euler start-up of the momentum ddt in
/// `outram-foam-appbuilder-lib`'s `ddt_vec_scheme`.
///
/// # Boundaries
///
/// Zeroed, as in [`ddt_corr`] (upstream zeros `coeff` on fixed-value and
/// cyclicAMI patches; incompressible solvers re-impose the boundary flux
/// regardless).
///
/// # Upstream reference
///
/// `backwardDdtScheme<Type>::fvcDdtPhiCorr(const VolField<Type>& U, const
/// fluxFieldType& phi)` in OpenFOAM-dev
/// `src/finiteVolume/finiteVolume/ddtSchemes/backwardDdtScheme/backwardDdtScheme.C`
/// (Copyright (C) 2011-2026 OpenFOAM Foundation; read from a copy of the file
/// taken 2026-11-01, not vendored in this repository), plus
/// `ddtScheme<Type>::fvcDdtPhiCoeff` for the limiter (shared with
/// [`ddt_corr`]; `ddtScheme.C` was **not** re-read for this port — the limiter
/// is the one [`ddt_corr`] already carries). `Δt` and `Δt₀` are passed
/// explicitly instead of read from `runTime`.
///
/// # V&V
///
/// Unit tests below pin the coefficients, the start-up reduction and the
/// constant-step formula. The solver-level temporal-order study is
/// `outram-foam-appbuilder-lib`'s `tests/bdf2_temporal_order.rs` and
/// `verification_and_validation/bdf2_temporal_order.md` (2026-11-01).
pub fn ddt_corr_backward(
    u_old: &VolVectorField,
    u_old_old: Option<&VolVectorField>,
    phi_old: &SurfaceScalarField,
    phi_old_old: Option<&SurfaceScalarField>,
    dt: f64,
    dt0: f64,
) -> SurfaceScalarField {
    let (Some(u_oo), Some(phi_oo)) = (u_old_old, phi_old_old) else {
        return ddt_corr(u_old, phi_old, dt);
    };
    let BackwardDdtCoeffs {
        coefft0, coefft00, ..
    } = BackwardDdtCoeffs::new(dt, dt0);
    let mesh = &u_old.mesh;
    let r_dt = 1.0 / dt;

    // Limiter from the single old level, as upstream.
    let u_f0 = interpolate(u_old);
    // Interpolate the BDF2 history combination (upstream operation order).
    let u_hist_f = interpolate(&(u_old.clone() * coefft0 - u_oo.clone() * coefft00));

    let internal = Field::from_fn(mesh.n_internal_faces, |f| {
        let sf = mesh.face_area_vectors[f];
        let phi_corr0 = phi_old.internal[f] - u_f0.internal[f].dot(sf);
        let coeff = 1.0 - (phi_corr0.abs() / (phi_old.internal[f].abs() + SMALL)).min(1.0);
        let phi_hist = coefft0 * phi_old.internal[f] - coefft00 * phi_oo.internal[f];
        coeff * r_dt * (phi_hist - u_hist_f.internal[f].dot(sf))
    });

    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField {
            bc: BoundaryCondition::ZeroGradient,
            values: Field::zeros(p.size),
        })
        .collect();

    SurfaceScalarField::new("ddtCorr", phi_old.mesh.clone(), internal, boundary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::boundary::bc::PatchField;
    use crate::fields::field::Field;
    use crate::fields::surface_field::SurfaceScalarField;
    use crate::fields::vol_field::VolVectorField;
    use crate::fv_operators::fvc::flux;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use crate::primitives::Vector3;
    use std::sync::Arc;

    fn unit_mesh() -> Arc<crate::mesh::fv_mesh::FvMesh> {
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
                .cell_volumes(vec![1.0, 1.0])
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
    fn consistent_phi_gives_zero_correction() {
        // If phi_old == flux(U_old), ddtCorr == 0
        let m = unit_mesh();
        let u = VolVectorField::uniform("U", m.clone(), Vector3::new(1.0, 0.0, 0.0));
        let phi = flux(&u);
        let corr = ddt_corr(&u, &phi, 0.1);
        assert!(corr.internal[0].abs() < 1e-12);
        assert!(corr.boundary[0].values[0].abs() < 1e-12);
    }

    #[test]
    fn nonzero_correction_for_inconsistent_phi() {
        let m = unit_mesh();
        let u = VolVectorField::uniform("U", m.clone(), Vector3::new(1.0, 0.0, 0.0));
        // phi_old = 2 everywhere (doesn't match U·Sf = 1)
        let bnd: Vec<_> = m
            .patches
            .iter()
            .map(|p| PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::uniform(p.size, 2.0),
            })
            .collect();
        let phi_old = SurfaceScalarField::new("phi", m.clone(), Field::uniform(1, 2.0), bnd);
        let corr = ddt_corr(&u, &phi_old, 1.0);
        // phiCorr = 2 − 1 = 1; coeff = 1 − min(|1|/(|2|+SMALL), 1) = 0.5;
        // ddtCorr = coeff·phiCorr/dt = 0.5·1/1 = 0.5
        assert!((corr.internal[0] - 0.5).abs() < 1e-12);
    }

    #[test]
    fn coeff_limits_large_correction_to_zero() {
        // When |phiCorr| ≥ |phi_old|, coeff → 0, so the correction is fully
        // damped (this is what keeps ddtCorr from destabilising the flux).
        let m = unit_mesh();
        let u = VolVectorField::uniform("U", m.clone(), Vector3::new(1.0, 0.0, 0.0));
        // phi_old = 0 (U·Sf = 1 → |phiCorr| = 1 ≫ |phi_old| = 0)
        let bnd: Vec<_> = m
            .patches
            .iter()
            .map(|p| PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::zeros(p.size),
            })
            .collect();
        let phi_old = SurfaceScalarField::new("phi", m.clone(), Field::zeros(1), bnd);
        let corr = ddt_corr(&u, &phi_old, 1.0);
        assert!(
            corr.internal[0].abs() < 1e-12,
            "coeff must damp the correction to ~0"
        );
    }

    fn flux_field(m: &Arc<crate::mesh::fv_mesh::FvMesh>, v: f64) -> SurfaceScalarField {
        let bnd: Vec<_> = m
            .patches
            .iter()
            .map(|p| PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::uniform(p.size, v),
            })
            .collect();
        SurfaceScalarField::new("phi", m.clone(), Field::uniform(m.n_internal_faces, v), bnd)
    }

    #[test]
    fn backward_coeffs_constant_step_are_exact_bdf2() {
        let c = BackwardDdtCoeffs::new(0.37, 0.37);
        assert_eq!(c.coefft, 1.5);
        assert_eq!(c.coefft0, 2.0);
        assert_eq!(c.coefft00, 0.5);
    }

    #[test]
    fn backward_coeffs_variable_step_differentiate_a_quadratic_exactly() {
        // BDF2 with variable steps is exact for psi(t) = t^2 at t = t_n:
        // (coefft*psi_n - coefft0*psi_{n-1} + coefft00*psi_{n-2})/dt = 2 t_n.
        let (dt, dt0) = (0.3, 0.7);
        let c = BackwardDdtCoeffs::new(dt, dt0);
        let tn = 2.0_f64;
        let (t1, t2) = (tn - dt, tn - dt - dt0);
        let d = (c.coefft * tn * tn - c.coefft0 * t1 * t1 + c.coefft00 * t2 * t2) / dt;
        assert!((d - 2.0 * tn).abs() < 1e-12, "d = {d}");
    }

    #[test]
    fn backward_corr_without_old_old_is_euler_bit_for_bit() {
        let m = unit_mesh();
        let u = VolVectorField::uniform("U", m.clone(), Vector3::new(1.0, 0.0, 0.0));
        let phi = flux_field(&m, 2.0);
        let e = ddt_corr(&u, &phi, 0.25);
        let b = ddt_corr_backward(&u, None, &phi, Some(&phi), 0.25, 0.25);
        assert_eq!(e.internal[0].to_bits(), b.internal[0].to_bits());
        let b2 = ddt_corr_backward(&u, Some(&u), &phi, None, 0.25, 0.25);
        assert_eq!(e.internal[0].to_bits(), b2.internal[0].to_bits());
    }

    #[test]
    fn backward_corr_constant_step_matches_hand_value() {
        // U_old = 1, U_old_old = 0.5 (x), phi_old = 2, phi_old_old = 1.5, dt = 1.
        // coeff = 1 - min(|2 - 1|/2, 1) = 0.5
        // hist  = (2*2 - 0.5*1.5) - (2*1 - 0.5*0.5) = 3.25 - 1.75 = 1.5
        // corr  = 0.5 * 1.5 / 1 = 0.75
        let m = unit_mesh();
        let u0 = VolVectorField::uniform("U", m.clone(), Vector3::new(1.0, 0.0, 0.0));
        let u00 = VolVectorField::uniform("U", m.clone(), Vector3::new(0.5, 0.0, 0.0));
        let p0 = flux_field(&m, 2.0);
        let p00 = flux_field(&m, 1.5);
        let c = ddt_corr_backward(&u0, Some(&u00), &p0, Some(&p00), 1.0, 1.0);
        assert!((c.internal[0] - 0.75).abs() < 1e-12, "{}", c.internal[0]);
        assert_eq!(c.boundary[0].values[0], 0.0);
        // Consistent history (phi == flux(U) at both levels) gives zero.
        let c0 = ddt_corr_backward(&u0, Some(&u00), &flux(&u0), Some(&flux(&u00)), 1.0, 1.0);
        assert!(c0.internal[0].abs() < 1e-12);
    }
}
