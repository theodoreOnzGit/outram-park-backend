// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/thermoMechanics/tractionDisplacement/GFTractionDisplacementFvPatchVectorField.{C,H}
//                    src/classes/thermoMechanics/tractionDisplacement/gapContactFvPatchVectorField.{C,H}
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal author: Carlo Fiorina (EPFL)
//   Upstream license: GPL-3.0 (the sources say GPL-2.0-or-later)
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate root or
// <https://www.gnu.org/licenses/> for the full license text.
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation, nor OpenCFD Limited,
// producer and distributor of the OpenFOAM(R) software.

//! # `GFTractionDisplacement` and `gapContact` boundary conditions of `disp` (gh:#1235, round 24)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Mesh-free: every function takes plain per-face arrays, so the finite-volume solver
//! ([`super::LegacyFvmThermoMechanics`]) and the later finite-element version (gh:#1782) call the same code.
//!
//! ## `GFTractionDisplacement` (`GFTractionDisplacementFvPatchVectorField.C`)
//!
//! A `fixedGradient` patch whose gradient is recomputed at every `updateCoeffs()` (lines 143-212):
//!
//! ```text
//! E = rhoE/rho;  mu = E/(2(1+nu));  lambda = nu E/((1+nu)(1-2nu));  twoMuLambda = 2 mu + lambda
//! gradient = ((traction - pressure n)/rho + twoMuLambda snGrad - (n & sigmaD))/twoMuLambda
//! gradient += n threeKalpha (TStruct - TStructRef)/twoMuLambda
//! ```
//!
//! ([`GfTractionDisplacement::update_gradient`]); `snGrad` is the BASE-class `fvPatchField<vector>::snGrad()`
//! (`deltaCoeffs (value - internal)`, [`base_sn_grad`]); after the solve `evaluate()` sets
//! `value = internal + gradient/deltaCoeffs` ([`evaluate_value`]).
//!
//! **Upstream defect kept (lines 160-164).** `planeStress` is read as
//! `found("rheologyOptions") ? subDict("rhelogyOptions").get<bool>("planeStress") : false`: the guard and the
//! sub-dictionary are spelled differently, and the class itself reads `rheologyOption`; so the BC is always plane
//! strain (and a case that has a `rheologyOptions` key aborts upstream). The port reproduces that; the intended
//! behaviour is the named option `honour_plane_stress` of [`TractionInputs`], used as a control only.
//!
//! ## `gapContact` (`gapContactFvPatchVectorField.C`)
//!
//! Derived from the traction BC: `pressure = interfaceP`, with the interface pressure of a penalty law between two
//! `regionCoupled` patches. Only the arithmetic is ported ([`GapContact`]): the two-region coupling (the neighbour
//! region's `disp`/`meshDisp`, `regionCoupledPatch().interpolate`) is gh:#1846 and the neighbour-side arrays are
//! INPUTS here. Not compared with upstream (nothing in GeN-Foam's tutorials runs it); tested against hand-computed
//! values.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use outram_foam_basic_lib::primitives::{SymmTensor, Vector3};

use crate::error::AppBuilderError;

/// `scalar*vector` as OpenFOAM evaluates it.
fn scale(s: f64, v: Vector3) -> Vector3 {
    Vector3::new(s * v.x, s * v.y, s * v.z)
}

/// `n & sigma` for a symmetric tensor (`Vector & SymmTensor` of OpenFOAM, left-to-right sums).
fn dot_symm(v: Vector3, s: &SymmTensor) -> Vector3 {
    Vector3::new(
        v.x * s.xx + v.y * s.xy + v.z * s.xz,
        v.x * s.xy + v.y * s.yy + v.z * s.yz,
        v.x * s.xz + v.y * s.yz + v.z * s.zz,
    )
}

/// `fvPatchField<vector>::snGrad()` (the base class, `deltaCoeffs*(value - patchInternalField)`), per face.
pub fn base_sn_grad(delta_coeffs: f64, value: Vector3, internal: Vector3) -> Vector3 {
    scale(delta_coeffs, value - internal)
}

/// `fixedGradientFvPatchField::evaluate()`: `patchInternalField + gradient/deltaCoeffs`, per face.
pub fn evaluate_value(internal: Vector3, gradient: Vector3, delta_coeffs: f64) -> Vector3 {
    Vector3::new(
        internal.x + gradient.x / delta_coeffs,
        internal.y + gradient.y / delta_coeffs,
        internal.z + gradient.z / delta_coeffs,
    )
}

/// What `updateCoeffs()` reads from the registry for one patch, one entry per face (the boundary values of the
/// `zeroGradient` fields `rho`, `E`, `nu`, `threeKalpha`, `TStruct`, `TStructRef`, the `calculated` `sigmaD`, the
/// unit normals `Sf/magSf`, and the base-class `snGrad()` of `disp`).
#[derive(Debug, Clone, PartialEq)]
pub struct TractionInputs {
    /// `rho`.
    pub rho: Vec<f64>,
    /// `rhoE` (the registered object called `E`).
    pub rho_e: Vec<f64>,
    /// `nu`.
    pub nu: Vec<f64>,
    /// `Sf/magSf`.
    pub n: Vec<Vector3>,
    /// `fvPatchField<vector>::snGrad()`.
    pub sn_grad: Vec<Vector3>,
    /// `sigmaD` on the patch.
    pub sigma_d: Vec<SymmTensor>,
    /// `threeKalpha`.
    pub three_k_alpha: Vec<f64>,
    /// `TStruct`.
    pub t_struct: Vec<f64>,
    /// `TStructRef`.
    pub t_struct_ref: Vec<f64>,
    /// `false` = upstream (plane strain always, see the module documentation); `true` = the intended form, a
    /// named CONTROL, not upstream's behaviour.
    pub honour_plane_stress: bool,
    /// The case's `rheologyOption.planeStress` (used only when `honour_plane_stress`).
    pub plane_stress: bool,
}

/// The state of a `GFTractionDisplacement` patch: the inputs `traction` and `pressure` and the computed `gradient`.
#[derive(Debug, Clone, PartialEq)]
pub struct GfTractionDisplacement {
    traction: Vec<Vector3>,
    pressure: Vec<f64>,
    gradient: Vec<Vector3>,
    updated: bool,
}

impl GfTractionDisplacement {
    /// A patch of `traction.len()` faces; the gradient starts at zero (the constructor's `gradient() = zero`).
    ///
    /// # Errors
    /// [`AppBuilderError::Case`] when the two lists differ in length.
    pub fn new(traction: Vec<Vector3>, pressure: Vec<f64>) -> Result<Self, AppBuilderError> {
        if traction.len() != pressure.len() {
            return Err(AppBuilderError::Case {
                case: crate::path::PathBuf::new(),
                msg: "GFTractionDisplacement: `traction` and `pressure` must have one entry per face".to_string(),
            });
        }
        let n = traction.len();
        Ok(Self { traction, pressure, gradient: vec![Vector3::ZERO; n], updated: false })
    }

    /// `uniform` traction and pressure on `n` faces.
    pub fn uniform(n: usize, traction: Vector3, pressure: f64) -> Self {
        Self { traction: vec![traction; n], pressure: vec![pressure; n], gradient: vec![Vector3::ZERO; n], updated: false }
    }

    /// Faces.
    pub fn len(&self) -> usize {
        self.traction.len()
    }

    /// No faces.
    pub fn is_empty(&self) -> bool {
        self.traction.is_empty()
    }

    /// `traction` (Pa).
    pub fn traction(&self) -> &[Vector3] {
        &self.traction
    }

    /// `pressure` (Pa).
    pub fn pressure(&self) -> &[f64] {
        &self.pressure
    }

    /// The current `gradient()` per face.
    pub fn gradient(&self) -> &[Vector3] {
        &self.gradient
    }

    /// The patch as the dictionary constructor leaves it after a READ (README round 60): `gradient() = Zero` and no `updateCoeffs()`
    /// yet. The `value` entry of the file is not used by the constructor (`operator=(patchInternalField())`), so the caller sets the patch values to
    /// the owner cells' too.
    pub fn reset_after_read(&mut self) {
        self.gradient.iter_mut().for_each(|g| *g = Vector3::ZERO);
        self.updated = false;
    }

    /// `pressure() = ...` (what `gapContact::updateCoeffs` does before calling this class's).
    ///
    /// # Panics
    /// If the length differs from the number of faces.
    pub fn set_pressure(&mut self, pressure: Vec<f64>) {
        assert_eq!(pressure.len(), self.pressure.len(), "one pressure per face");
        self.pressure = pressure;
    }

    /// `updated()`: `updateCoeffs()` has run since the last `evaluate()`.
    pub fn is_updated(&self) -> bool {
        self.updated
    }

    /// `evaluate()` was done: `updated()` becomes false again (`fvPatchField::evaluate`).
    pub fn clear_updated(&mut self) {
        self.updated = false;
    }

    /// `updateCoeffs()`: nothing when `updated()`, else recompute the gradient from `inputs` and set `updated()`.
    /// Returns whether the gradient was recomputed.
    ///
    /// # Panics
    /// If an input array does not have one entry per face.
    pub fn update_gradient(&mut self, inputs: &TractionInputs) -> bool {
        if self.updated {
            return false;
        }
        let n = self.len();
        assert!(
            inputs.rho.len() == n
                && inputs.rho_e.len() == n
                && inputs.nu.len() == n
                && inputs.n.len() == n
                && inputs.sn_grad.len() == n
                && inputs.sigma_d.len() == n
                && inputs.three_k_alpha.len() == n
                && inputs.t_struct.len() == n
                && inputs.t_struct_ref.len() == n,
            "one input per face"
        );
        for i in 0..n {
            let rho = inputs.rho[i];
            let nu = inputs.nu[i];
            // scalarField E(rhoE/rho); mu(E/(2.0*(1.0 + nu))); lambda(nu*E/((1.0 + nu)*(1.0 - 2.0*nu)))
            let e = inputs.rho_e[i] / rho;
            let mu = e / (2.0 * (1.0 + nu));
            let mut lambda = nu * e / ((1.0 + nu) * (1.0 - 2.0 * nu));
            if inputs.honour_plane_stress && inputs.plane_stress {
                lambda = nu * e / ((1.0 + nu) * (1.0 - nu));
            }
            let two_mu_lambda = 2.0 * mu + lambda;
            let nn = inputs.n[i];
            // (traction_ - pressure_*n)/rho
            let tp = self.traction[i] - scale(self.pressure[i], nn);
            let a = Vector3::new(tp.x / rho, tp.y / rho, tp.z / rho);
            // ((a + twoMuLambda*snGrad) - (n & sigmaD))/twoMuLambda
            let b = scale(two_mu_lambda, inputs.sn_grad[i]);
            let c = dot_symm(nn, &inputs.sigma_d[i]);
            let ab = a + b;
            let abc = ab - c;
            let mut g = Vector3::new(abc.x / two_mu_lambda, abc.y / two_mu_lambda, abc.z / two_mu_lambda);
            // gradient() += n*threeKalpha*(Tmech - TrefStructure)/twoMuLambda
            let dt = inputs.t_struct[i] - inputs.t_struct_ref[i];
            let tka = inputs.three_k_alpha[i];
            let th = Vector3::new(
                nn.x * tka * dt / two_mu_lambda,
                nn.y * tka * dt / two_mu_lambda,
                nn.z * tka * dt / two_mu_lambda,
            );
            g = Vector3::new(g.x + th.x, g.y + th.y, g.z + th.z);
            self.gradient[i] = g;
        }
        self.updated = true;
        true
    }
}

/// `gapContact` of one `regionCoupled` patch: `penaltyFactor` and `offset` (read on the OWNER side only; the
/// neighbour side keeps the constructor defaults 0.1 and 0, and receives its pressure by interpolation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GapContact {
    /// `penaltyFactor` (dictionary default 1e-2 on the owner).
    pub penalty_factor: f64,
    /// `offset` (m, default 0).
    pub offset: f64,
}

impl Default for GapContact {
    fn default() -> Self {
        Self { penalty_factor: 1.0e-2, offset: 0.0 }
    }
}

impl GapContact {
    /// `gapWidth()` (`gapContactFvPatchVectorField.C` lines 53-100): `((nbrCf - Cf) & nf) - offset` with
    /// `Cf = patch.Cf + meshDisp` and `nbrCf = interpolate(nbrPatch.Cf + meshDisp_nbr)`; the arguments are those two
    /// sums, the second already interpolated onto this patch.
    pub fn gap_width(&self, cf_plus_disp: &[Vector3], nbr_cf_plus_disp: &[Vector3], nf: &[Vector3]) -> Vec<f64> {
        (0..cf_plus_disp.len())
            .map(|i| {
                let d = nbr_cf_plus_disp[i] - cf_plus_disp[i];
                (d.x * nf[i].x + d.y * nf[i].y + d.z * nf[i].z) - self.offset
            })
            .collect()
    }

    /// `boundaryStiffness()` (lines 103-117): `gMin(2*(2 mu + lambda)*deltaCoeffs)` over the patch faces.
    pub fn boundary_stiffness(mu: &[f64], lambda: &[f64], delta_coeffs: &[f64]) -> f64 {
        let mut m = f64::INFINITY;
        for i in 0..mu.len() {
            let v = 2.0 * (2.0 * mu[i] + lambda[i]) * delta_coeffs[i];
            if v < m {
                m = v;
            }
        }
        m
    }

    /// The owner-side update of `updateCoeffs()` (lines 236-262): `gapWidth_ = 0.5 (gapWidth() + interpolate(nbr
    /// gapWidth()))` and `interfaceP = max(-penaltyFact*min(k, k_nbr)*gapWidth_, 0)`. Returns `(gapWidth_, interfaceP)`.
    /// The neighbour side then receives `interpolate` of both (caller).
    pub fn update_owner(&self, own_gap: &[f64], nbr_gap_interpolated: &[f64], k_own: f64, k_nbr: f64) -> (Vec<f64>, Vec<f64>) {
        let gap: Vec<f64> = (0..own_gap.len()).map(|i| 0.5 * (own_gap[i] + nbr_gap_interpolated[i])).collect();
        let k = k_own.min(k_nbr);
        let p = gap.iter().map(|g| (-self.penalty_factor * k * *g).max(0.0)).collect();
        (gap, p)
    }
}
