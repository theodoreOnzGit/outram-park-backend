// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
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

//! Small-strain constitutive laws, integrated at a quadrature point, with the
//! **consistent (algorithmic) tangent**.
//!
//! # What belongs in this module
//!
//! [`Material`] — a closed enum of laws — the per-point history it needs
//! ([`MaterialState`]), and the stress update
//! [`Material::update`], which returns stress *and* the tangent that makes the
//! global Newton iteration converge quadratically.
//!
//! # What does NOT belong here
//!
//! Anything that knows about a mesh, an element, or a quadrature rule. This
//! module maps one strain tensor and one history state to one stress tensor and
//! one tangent. Where those strains come from is [`crate::assembly`]'s problem.
//!
//! # Reuse decision: why this is not `outram-park-fork-offbeat`'s rheology
//!
//! `crates/outram-park-fork-offbeat/src/rheology/` already holds a large
//! `code_aster`- and OFFBEAT-derived constitutive library, including
//! `ConstitutiveLaw::MisesPlasticity` with a Newton-solved radial return
//! (`rheology/law.rs`, `return_map`) and `aster/isotropic.rs`'s
//! `IsotropicHardening::radial_return`. It was read before this module was
//! written, and it is a genuinely good implementation of the *stress update*.
//!
//! It does not compose with a finite-element Gauss loop for three reasons, in
//! descending order of importance:
//!
//! 1. **It returns no algorithmic tangent.** `StressCorrection` carries the
//!    stress, the elastic/plastic/creep strain increments, the yield stress,
//!    a yielding flag and an iteration count — and no `dsigma/depsilon`. That
//!    is not an oversight: it is driven by a finite-volume segregated solver
//!    that uses a fixed elastic operator and corrects by iteration, so it never
//!    needs one. `aster/isotropic.rs` makes the gap explicit, exposing
//!    `return_residual` with the comment that "a caller assembling a consistent
//!    tangent needs the same function". An implicit finite-element Newton needs
//!    the tangent itself; without it the iteration degrades from quadratic to
//!    linear, which the verification suite would immediately fail to reproduce.
//! 2. **Its inputs are fuel-rod shaped.** `RheologyInputs` requires a
//!    `MaterialState` (composition, temperature, burnup, irradiation history)
//!    and an `IrradiationState` before it will evaluate anything. A verification
//!    case on a unit square has no meaningful value for any of them.
//! 3. **The conventions differ.** OFFBEAT works in `SymmTensor`, the `aster`
//!    subtree in Mandel form (`AsterVoigt`, with `sqrt(2)` on the shears), and
//!    finite elements in engineering Voigt. Converting per quadrature point is
//!    possible, but a silent factor-of-two between three conventions in the
//!    innermost loop is precisely the class of error the workspace tries to
//!    design out.
//!
//! So the **logic** is ported rather than the code, and the two references are
//! cited here so they cannot drift unnoticed:
//!
//! - `crates/outram-park-fork-offbeat/src/rheology/law.rs`, `return_map` — the
//!   same consistency condition `|s_trial| - 2 mu dLambda - sqrt(2/3)
//!   sigma_y = 0`, and the same closed-form root for linear hardening.
//! - J. C. Simo and T. J. R. Hughes, *Computational Inelasticity*,
//!   Springer, 1998 — **Box 3.2** (the radial-return algorithm for J2 flow
//!   with isotropic hardening) and **Box 3.3 / equation (3.3.5)** (the
//!   consistent elastoplastic tangent). The tangent formula implemented here
//!   is that one, and the symbols below use its notation.
//!
//! # Units
//!
//! Young's modulus, shear modulus, bulk modulus, yield stress, hardening
//! modulus and every stress are in pascals (newton per square metre). Poisson's
//! ratio, every strain and the equivalent plastic strain are dimensionless
//! (metre per metre). The tangent is in pascals.

use crate::error::{FemError, Result};
use crate::tensor::{Tensor4, Voigt6};

/// Isotropic linear elasticity, stored as the two independent constants a user
/// actually has to hand.
///
/// # Units
///
/// `youngs_modulus` in pascals, `poissons_ratio` dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearElastic {
    youngs_modulus: f64,
    poissons_ratio: f64,
}

impl LinearElastic {
    /// Construct from Young's modulus and Poisson's ratio.
    ///
    /// # Arguments
    ///
    /// - `youngs_modulus` — `E` in pascals; must be finite and strictly
    ///   positive. Structural steel is about `200e9`.
    /// - `poissons_ratio` — `nu`, dimensionless; must satisfy
    ///   `-1 < nu < 0.5`. The open upper bound is not pedantry: at `nu = 0.5`
    ///   the bulk modulus is infinite and the stiffness matrix is singular, and
    ///   approaching it makes low-order elements lock volumetrically.
    ///
    /// # Errors
    ///
    /// [`FemError::MaterialOutOfRange`] outside those ranges.
    pub fn new(youngs_modulus: f64, poissons_ratio: f64) -> Result<Self> {
        if !(youngs_modulus > 0.0) || !youngs_modulus.is_finite() {
            return Err(FemError::MaterialOutOfRange {
                parameter: "youngs_modulus",
                value: youngs_modulus,
                unit: "Pa",
                reason: "must be finite and strictly positive",
            });
        }
        if !(poissons_ratio > -1.0 && poissons_ratio < 0.5) {
            return Err(FemError::MaterialOutOfRange {
                parameter: "poissons_ratio",
                value: poissons_ratio,
                unit: "dimensionless",
                reason: "must satisfy -1 < nu < 0.5 for a positive-definite isotropic tensor",
            });
        }
        Ok(Self {
            youngs_modulus,
            poissons_ratio,
        })
    }

    /// Young's modulus `E` \[Pa\].
    #[must_use]
    pub fn youngs_modulus(&self) -> f64 {
        self.youngs_modulus
    }

    /// Poisson's ratio `nu` \[-\].
    #[must_use]
    pub fn poissons_ratio(&self) -> f64 {
        self.poissons_ratio
    }

    /// Shear modulus `mu = E / (2 (1 + nu))` \[Pa\].
    #[must_use]
    pub fn shear_modulus(&self) -> f64 {
        self.youngs_modulus / (2.0 * (1.0 + self.poissons_ratio))
    }

    /// Bulk modulus `K = E / (3 (1 - 2 nu))` \[Pa\].
    #[must_use]
    pub fn bulk_modulus(&self) -> f64 {
        self.youngs_modulus / (3.0 * (1.0 - 2.0 * self.poissons_ratio))
    }

    /// First Lame parameter `lambda = K - 2 mu / 3` \[Pa\].
    #[must_use]
    pub fn lame_lambda(&self) -> f64 {
        self.bulk_modulus() - 2.0 * self.shear_modulus() / 3.0
    }

    /// The isotropic elastic stiffness `C_e` \[Pa\] in Voigt form.
    ///
    /// `sigma_V = C_e eps_V` with `eps_V` in engineering shear — see
    /// [`crate::tensor`].
    #[must_use]
    pub fn stiffness(&self) -> Tensor4 {
        Tensor4::isotropic(self.bulk_modulus(), self.shear_modulus())
    }
}

/// Rate-independent J2 (von Mises) plasticity with **linear isotropic
/// hardening**.
///
/// The yield surface is
///
/// `f(sigma, alpha) = q(sigma) - (sigma_y0 + H alpha)`,
///
/// with `q` the von Mises equivalent stress \[Pa\] and `alpha` the accumulated
/// equivalent plastic strain \[-\]. Flow is associated (Prandtl-Reuss), so the
/// plastic strain increment is purely deviatoric and the response is
/// pressure-insensitive and volume-preserving.
///
/// # Units
///
/// `initial_yield_stress` and `hardening_modulus` in pascals.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct J2LinearHardening {
    /// Elastic response below yield and during unloading.
    pub elastic: LinearElastic,
    /// `sigma_y0` \[Pa\], the yield stress at zero plastic strain. Strictly
    /// positive. Mild steel is roughly `250e6`.
    pub initial_yield_stress: f64,
    /// `H = d sigma_y / d alpha` \[Pa\], the linear isotropic hardening
    /// modulus. `0` is perfect plasticity; positive values harden. Negative
    /// (softening) values are accepted down to `-3 mu`, beyond which the radial
    /// return has no unique root.
    pub hardening_modulus: f64,
}

impl J2LinearHardening {
    /// Construct and validate.
    ///
    /// # Errors
    ///
    /// [`FemError::MaterialOutOfRange`] for a non-positive yield stress or a
    /// hardening modulus at or below `-3 mu`.
    pub fn new(
        elastic: LinearElastic,
        initial_yield_stress: f64,
        hardening_modulus: f64,
    ) -> Result<Self> {
        if !(initial_yield_stress > 0.0) || !initial_yield_stress.is_finite() {
            return Err(FemError::MaterialOutOfRange {
                parameter: "initial_yield_stress",
                value: initial_yield_stress,
                unit: "Pa",
                reason: "must be finite and strictly positive",
            });
        }
        let three_mu = 3.0 * elastic.shear_modulus();
        if !(hardening_modulus > -three_mu) || !hardening_modulus.is_finite() {
            return Err(FemError::MaterialOutOfRange {
                parameter: "hardening_modulus",
                value: hardening_modulus,
                unit: "Pa",
                reason: "must be finite and exceed -3 mu; softening at or beyond that \
                         makes the radial return non-unique",
            });
        }
        Ok(Self {
            elastic,
            initial_yield_stress,
            hardening_modulus,
        })
    }

    /// Current yield stress `sigma_y(alpha) = sigma_y0 + H alpha` \[Pa\].
    #[must_use]
    pub fn yield_stress(&self, equivalent_plastic_strain: f64) -> f64 {
        self.initial_yield_stress + self.hardening_modulus * equivalent_plastic_strain
    }

    /// The closed-form uniaxial tangent modulus in the plastic range,
    /// `E_t = E H / (E + H)` \[Pa\].
    ///
    /// This is what a uniaxial-stress test must reproduce as the slope of
    /// `sigma` against `epsilon` after yield, and it is the reference the
    /// verification suite compares against.
    #[must_use]
    pub fn uniaxial_tangent_modulus(&self) -> f64 {
        let e = self.elastic.youngs_modulus();
        let h = self.hardening_modulus;
        e * h / (e + h)
    }
}

/// The per-quadrature-point history a constitutive law carries between steps.
///
/// # Units
///
/// `plastic_strain` is dimensionless, in **engineering** Voigt form (shears
/// doubled) so it subtracts directly from the total strain the B-matrix
/// produces. `equivalent_plastic_strain` is dimensionless and non-decreasing.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MaterialState {
    /// Accumulated plastic strain tensor `eps_p` \[-\], engineering Voigt.
    /// Deviatoric: its trace is zero to rounding.
    pub plastic_strain: Voigt6,
    /// Accumulated equivalent plastic strain `alpha` \[-\], `>= 0`, never
    /// decreasing.
    pub equivalent_plastic_strain: f64,
}

impl MaterialState {
    /// The virgin state: no plastic strain, no history.
    #[must_use]
    pub fn pristine() -> Self {
        Self::default()
    }
}

/// The result of one constitutive integration at one point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StressUpdate {
    /// Cauchy stress `sigma` \[Pa\], tension positive, stress Voigt form.
    pub stress: Voigt6,
    /// The **consistent (algorithmic)** tangent `d sigma / d eps` \[Pa\], in
    /// the Voigt convention of [`crate::tensor::Tensor4`].
    ///
    /// This is the exact derivative of the *discrete* stress update, not the
    /// continuum elastoplastic modulus. The difference matters: only the
    /// algorithmic one gives a quadratically convergent global Newton
    /// iteration, which is the property the verification suite measures.
    pub tangent: Tensor4,
    /// The state at the **end** of the step. Commit it once the global
    /// iteration has converged, not during it.
    pub state: MaterialState,
    /// Whether this point flowed plastically during the step.
    pub yielding: bool,
}

/// The closed set of constitutive laws Farrer Park implements.
///
/// An enum, not a trait object: adding a law makes every `match` a compile
/// error until it is handled, which is what is wanted for a set this small.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Material {
    /// Isotropic linear elasticity. No history, tangent constant.
    Elastic(LinearElastic),
    /// J2 plasticity with linear isotropic hardening and radial return.
    J2(J2LinearHardening),
}

impl Material {
    /// Convenience constructor for isotropic linear elasticity.
    ///
    /// `youngs_modulus` in pascals, `poissons_ratio` dimensionless.
    ///
    /// # Errors
    ///
    /// As [`LinearElastic::new`].
    pub fn elastic(youngs_modulus: f64, poissons_ratio: f64) -> Result<Self> {
        Ok(Material::Elastic(LinearElastic::new(
            youngs_modulus,
            poissons_ratio,
        )?))
    }

    /// Convenience constructor for J2 plasticity with linear isotropic
    /// hardening.
    ///
    /// # Arguments
    ///
    /// - `youngs_modulus` \[Pa\], `poissons_ratio` \[-\] — the elastic
    ///   response.
    /// - `initial_yield_stress` \[Pa\] — `sigma_y0`.
    /// - `hardening_modulus` \[Pa\] — `H`; `0.0` gives perfect plasticity.
    ///
    /// # Errors
    ///
    /// As [`LinearElastic::new`] and [`J2LinearHardening::new`].
    pub fn j2_linear_hardening(
        youngs_modulus: f64,
        poissons_ratio: f64,
        initial_yield_stress: f64,
        hardening_modulus: f64,
    ) -> Result<Self> {
        let e = LinearElastic::new(youngs_modulus, poissons_ratio)?;
        Ok(Material::J2(J2LinearHardening::new(
            e,
            initial_yield_stress,
            hardening_modulus,
        )?))
    }

    /// The underlying elastic constants, whichever law this is.
    #[must_use]
    pub fn elastic_constants(&self) -> LinearElastic {
        match self {
            Material::Elastic(e) => *e,
            Material::J2(p) => p.elastic,
        }
    }

    /// The purely elastic stiffness `C_e` \[Pa\].
    ///
    /// Used as the predictor tangent on the first iteration of a load step, and
    /// as the fallback tangent for a modified-Newton run.
    #[must_use]
    pub fn elastic_stiffness(&self) -> Tensor4 {
        self.elastic_constants().stiffness()
    }

    /// Whether this law carries history (and therefore needs the state
    /// committed at the end of a converged step).
    #[must_use]
    pub fn is_history_dependent(&self) -> bool {
        matches!(self, Material::J2(_))
    }

    /// Integrate the law over one step: total strain in, stress and consistent
    /// tangent out.
    ///
    /// # Arguments
    ///
    /// - `total_strain` — the total small strain `eps` \[-\] at the **end** of
    ///   the step, in engineering Voigt form (shears doubled). This is a
    ///   *total*-strain formulation, correct for rate-independent plasticity
    ///   under monotonic or reversing proportional-in-time loading: the history
    ///   enters only through `state`.
    /// - `state` — the history at the **start** of the step. Not mutated; the
    ///   updated state is returned inside [`StressUpdate`].
    ///
    /// # Returns
    ///
    /// Stress \[Pa\], consistent tangent \[Pa\], updated state, and whether the
    /// point yielded.
    ///
    /// # Errors
    ///
    /// [`FemError::ConstitutiveNotConverged`] never occurs for the laws
    /// currently implemented — linear hardening gives a closed-form return —
    /// but the signature is fallible so a nonlinear hardening curve can be
    /// added without changing every caller.
    ///
    /// # Algorithm (J2 branch): radial return, Simo and Hughes Box 3.2
    ///
    /// 1. Elastic predictor: `sigma_tr = C_e (eps - eps_p^n)`.
    /// 2. Yield check on `f_tr = q(sigma_tr) - sigma_y(alpha_n)`. If
    ///    `f_tr <= 0` the step is elastic; return `sigma_tr` and `C_e`.
    /// 3. Otherwise solve the consistency condition. For linear hardening it is
    ///    linear: `d_alpha = f_tr / (3 mu + H)`.
    /// 4. Radial return in the deviatoric plane:
    ///    `s = s_tr (1 - 3 mu d_alpha / q_tr)`; the hydrostatic part is
    ///    untouched, because plastic flow is volume preserving.
    /// 5. Update `eps_p` along the flow direction and `alpha` by `d_alpha`.
    /// 6. Consistent tangent (Simo and Hughes Box 3.3):
    ///    `C = K 1(x)1 + 2 mu theta I_dev - 2 mu theta_bar n(x)n`, with
    ///    `theta = q_new / q_tr` and
    ///    `theta_bar = 1 / (1 + H / (3 mu)) - (1 - theta)`.
    pub fn update(&self, total_strain: Voigt6, state: &MaterialState) -> Result<StressUpdate> {
        match self {
            Material::Elastic(e) => {
                let c = e.stiffness();
                Ok(StressUpdate {
                    stress: c.apply(&total_strain),
                    tangent: c,
                    state: *state,
                    yielding: false,
                })
            }
            Material::J2(p) => {
                let mu = p.elastic.shear_modulus();
                let k = p.elastic.bulk_modulus();
                let h = p.hardening_modulus;
                let ce = p.elastic.stiffness();

                // 1. Elastic predictor on the trial elastic strain.
                let eps_e_trial = total_strain.minus(&state.plastic_strain);
                let sigma_trial = ce.apply(&eps_e_trial);
                let s_trial = sigma_trial.stress_deviator();
                let q_trial = (1.5 * s_trial.stress_double_dot(&s_trial)).sqrt();

                // 2. Yield check.
                let sigma_y = p.yield_stress(state.equivalent_plastic_strain);
                let f_trial = q_trial - sigma_y;
                if f_trial <= 0.0 || q_trial <= 0.0 {
                    return Ok(StressUpdate {
                        stress: sigma_trial,
                        tangent: ce,
                        state: *state,
                        yielding: false,
                    });
                }

                // 3. Consistency condition; linear for linear hardening.
                let d_alpha = f_trial / (3.0 * mu + h);

                // 4. Radial return. theta = q_new / q_trial.
                let theta = 1.0 - 3.0 * mu * d_alpha / q_trial;
                let s_new = s_trial.scaled(theta);
                let p_mean = sigma_trial.mean_stress();
                let stress = s_new.plus(&Voigt6::IDENTITY.scaled(p_mean));

                // 5. Flow direction and state update. n is the unit deviatoric
                //    direction; the plastic strain increment in ENGINEERING
                //    Voigt doubles the shear components.
                let s_norm = s_trial.stress_norm();
                let n = s_trial.scaled(1.0 / s_norm);
                let d_gamma = d_alpha * (1.5_f64).sqrt(); // dEps_p = d_gamma * n
                let mut dep = [0.0_f64; 6];
                for i in 0..3 {
                    dep[i] = d_gamma * n.0[i];
                }
                for i in 3..6 {
                    dep[i] = 2.0 * d_gamma * n.0[i];
                }
                let new_state = MaterialState {
                    plastic_strain: state.plastic_strain.plus(&Voigt6(dep)),
                    equivalent_plastic_strain: state.equivalent_plastic_strain + d_alpha,
                };

                // 6. Consistent tangent.
                let theta_bar = 1.0 / (1.0 + h / (3.0 * mu)) - (1.0 - theta);
                let tangent = Tensor4::outer(&Voigt6::IDENTITY, &Voigt6::IDENTITY)
                    .scaled(k)
                    .plus(&Tensor4::deviatoric_projector().scaled(2.0 * mu * theta))
                    .plus(&Tensor4::outer(&n, &n).scaled(-2.0 * mu * theta_bar));

                Ok(StressUpdate {
                    stress,
                    tangent,
                    state: new_state,
                    yielding: true,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::compute::ComputeBackend;
    use outram_foam_basic_lib::math::differentiate::{jacobian, DiffSettings};

    fn steel() -> J2LinearHardening {
        J2LinearHardening::new(
            LinearElastic::new(200.0e9, 0.3).unwrap(),
            250.0e6,
            2.0e9,
        )
        .unwrap()
    }

    /// Elastic constants must satisfy the standard identities.
    #[test]
    fn elastic_constant_identities() {
        let e = LinearElastic::new(200.0e9, 0.3).unwrap();
        let (mu, k, lam) = (e.shear_modulus(), e.bulk_modulus(), e.lame_lambda());
        assert!((mu - 200.0e9 / 2.6).abs() < 1.0);
        assert!((k - 200.0e9 / 1.2).abs() < 1.0);
        assert!((lam - (k - 2.0 * mu / 3.0)).abs() < 1.0);
        // E recovered from mu and lambda.
        let e_back = mu * (3.0 * lam + 2.0 * mu) / (lam + mu);
        assert!((e_back - 200.0e9).abs() / 200.0e9 < 1e-12);
    }

    /// Out-of-range parameters must be rejected, not clamped.
    #[test]
    fn bad_parameters_are_rejected() {
        assert!(LinearElastic::new(-1.0, 0.3).is_err());
        assert!(LinearElastic::new(1.0, 0.5).is_err());
        assert!(LinearElastic::new(1.0, -1.5).is_err());
        let e = LinearElastic::new(200.0e9, 0.3).unwrap();
        assert!(J2LinearHardening::new(e, 0.0, 0.0).is_err());
        assert!(J2LinearHardening::new(e, 1.0e6, -3.0 * e.shear_modulus()).is_err());
    }

    /// Below yield the J2 law must be indistinguishable from elasticity, in
    /// both stress and tangent.
    #[test]
    fn j2_reduces_to_elasticity_below_yield() {
        let p = steel();
        let m = Material::J2(p);
        let eps = Voigt6::new(5.0e-4, 0.0, 0.0, 0.0, 0.0, 0.0);
        let up = m.update(eps, &MaterialState::pristine()).unwrap();
        assert!(!up.yielding);
        let sig_e = p.elastic.stiffness().apply(&eps);
        assert!(up.stress.minus(&sig_e).abs_max() < 1e-6);
        assert!(up.tangent.max_abs_diff(&p.elastic.stiffness()) < 1e-3);
    }

    /// After a plastic step the returned stress must lie **exactly** on the
    /// updated yield surface — the defining property of the return map.
    #[test]
    fn returned_stress_lies_on_the_yield_surface() {
        let p = steel();
        let m = Material::J2(p);
        for e_xx in [2.0e-3, 5.0e-3, 2.0e-2] {
            // Uniaxial *strain* drive, which certainly yields.
            let eps = Voigt6::new(e_xx, 0.0, 0.0, 0.0, 0.0, 0.0);
            let up = m.update(eps, &MaterialState::pristine()).unwrap();
            assert!(up.yielding);
            let q = up.stress.von_mises();
            let sy = p.yield_stress(up.state.equivalent_plastic_strain);
            assert!(
                (q - sy).abs() < 1e-4 * sy,
                "q = {} Pa, sigma_y = {} Pa",
                q,
                sy
            );
            // Plastic flow is volume preserving.
            assert!(up.state.plastic_strain.trace().abs() < 1e-15);
        }
    }

    /// The consistent tangent must equal the numerical derivative of the stress
    /// update. Uses `outram-foam-basic-lib`'s `differentiate::jacobian`, which
    /// implements Code_Aster's perturbed-Newton stepping, rather than a
    /// hand-rolled difference.
    #[test]
    fn consistent_tangent_matches_numerical_jacobian() {
        let p = steel();
        let m = Material::J2(p);
        // A state well inside the plastic regime, with prior history and a
        // non-trivial shear so no component of the tangent is trivially zero.
        let state = {
            let warm = Voigt6::new(3.0e-3, -5.0e-4, 0.0, 0.0, 0.0, 1.0e-3);
            m.update(warm, &MaterialState::pristine()).unwrap().state
        };
        let eps = Voigt6::new(6.0e-3, -1.0e-3, 2.0e-4, 3.0e-4, -2.0e-4, 2.0e-3);
        let analytic = m.update(eps, &state).unwrap();
        assert!(analytic.yielding, "the check point must actually be plastic");

        let sol = jacobian(
            &eps.as_array(),
            DiffSettings::central(),
            ComputeBackend::Serial,
            |_, v: &[f64], out: &mut Vec<f64>| {
                let e = Voigt6([v[0], v[1], v[2], v[3], v[4], v[5]]);
                let s = m.update(e, &state).unwrap().stress;
                out.extend_from_slice(&s.as_array());
            },
        );
        let num = sol.matrix().expect("stress update is smooth in the plastic regime");
        let scale = analytic.tangent.abs_max();
        let mut worst: f64 = 0.0;
        for i in 0..6 {
            for j in 0..6 {
                let d = (num.get(i, j) - analytic.tangent.get(i, j)).abs() / scale;
                worst = worst.max(d);
            }
        }
        assert!(
            worst < 1e-5,
            "consistent tangent vs numerical jacobian: worst relative entry error {}",
            worst
        );
        println!("consistent tangent max relative entry error = {worst:.3e}");
    }

    /// The elastic tangent must likewise match its own numerical derivative —
    /// the control that shows the comparison machinery itself is sound.
    #[test]
    fn elastic_tangent_matches_numerical_jacobian() {
        let m = Material::elastic(200.0e9, 0.3).unwrap();
        let eps = Voigt6::new(1.0e-4, -2.0e-4, 3.0e-5, 1.0e-5, -4.0e-5, 6.0e-5);
        let analytic = m.update(eps, &MaterialState::pristine()).unwrap();
        let sol = jacobian(
            &eps.as_array(),
            DiffSettings::central(),
            ComputeBackend::Serial,
            |_, v: &[f64], out: &mut Vec<f64>| {
                let e = Voigt6([v[0], v[1], v[2], v[3], v[4], v[5]]);
                out.extend_from_slice(
                    &m.update(e, &MaterialState::pristine()).unwrap().stress.as_array(),
                );
            },
        );
        let num = sol.matrix().unwrap();
        let scale = analytic.tangent.abs_max();
        for i in 0..6 {
            for j in 0..6 {
                assert!((num.get(i, j) - analytic.tangent.get(i, j)).abs() / scale < 1e-6);
            }
        }
    }

    /// The consistent tangent must be symmetric — associated flow with
    /// isotropic hardening gives a symmetric algorithmic modulus, and an
    /// asymmetric one would mean a sign or transposition error.
    #[test]
    fn consistent_tangent_is_symmetric() {
        let m = Material::J2(steel());
        let eps = Voigt6::new(4.0e-3, -1.0e-3, 5.0e-4, 2.0e-4, 1.0e-4, 1.5e-3);
        let up = m.update(eps, &MaterialState::pristine()).unwrap();
        assert!(up.yielding);
        let s = up.tangent.abs_max();
        for i in 0..6 {
            for j in 0..6 {
                assert!((up.tangent.get(i, j) - up.tangent.get(j, i)).abs() / s < 1e-14);
            }
        }
    }

    /// Unloading from a plastic state must be elastic: the tangent reverts to
    /// `C_e` and no further plastic strain accumulates.
    #[test]
    fn unloading_is_elastic() {
        let p = steel();
        let m = Material::J2(p);
        let loaded = Voigt6::new(5.0e-3, 0.0, 0.0, 0.0, 0.0, 0.0);
        let after = m.update(loaded, &MaterialState::pristine()).unwrap();
        assert!(after.yielding);
        let alpha = after.state.equivalent_plastic_strain;

        let unloaded = Voigt6::new(4.0e-3, 0.0, 0.0, 0.0, 0.0, 0.0);
        let back = m.update(unloaded, &after.state).unwrap();
        assert!(!back.yielding);
        assert!((back.state.equivalent_plastic_strain - alpha).abs() < 1e-18);
        assert!(back.tangent.max_abs_diff(&p.elastic.stiffness()) < 1e-3);
    }

    /// Perfect plasticity (`H = 0`) must cap the von Mises stress at the yield
    /// stress no matter how far the strain is pushed.
    #[test]
    fn perfect_plasticity_caps_the_von_mises_stress() {
        let m = Material::j2_linear_hardening(200.0e9, 0.3, 250.0e6, 0.0).unwrap();
        let mut state = MaterialState::pristine();
        for k in 1..=20 {
            let eps = Voigt6::new(0.0, 0.0, 0.0, 0.0, 0.0, 1.0e-3 * k as f64);
            let up = m.update(eps, &state).unwrap();
            state = up.state;
            assert!(up.stress.von_mises() <= 250.0e6 * (1.0 + 1e-9));
        }
        assert!((state.equivalent_plastic_strain > 0.0) && state.equivalent_plastic_strain < 1.0);
    }
}
