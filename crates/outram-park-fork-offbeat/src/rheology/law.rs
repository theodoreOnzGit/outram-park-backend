// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
// `offbeatLib/rheology/constitutiveLaws/constitutiveLaw.C/H`,
// `elasticity.C/H`, `misesPlasticity.C/H` and `misesPlasticCreep.C/H`.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! The constitutive laws themselves: elasticity, von Mises plasticity, and
//! plasticity with creep.

use outram_foam_basic_lib::primitives::SymmTensor;

use crate::error::{OffbeatError, Result};

use super::creep::CreepModel;
use super::state::{von_mises, RheologyInputs, RheologyState, StressCorrection};
use super::yield_stress::YieldStressModel;

/// Maximum local Newton iterations for the plastic return mapping.
const MAX_PLASTIC_ITERATIONS: usize = 50;

/// Relative convergence tolerance on the plastic consistency residual,
/// scaled by the trial deviatoric stress magnitude.
///
/// Scaled rather than absolute because the residual is a stress in pascal:
/// an absolute tolerance of 1 Pa is meaninglessly tight next to a 300 MPa yield
/// stress on one problem and unreachable on another.
const PLASTIC_REL_TOL: f64 = 1.0e-12;

/// A mechanical constitutive law: what stress a material carries for a given
/// strain, and what part of that strain is permanent.
///
/// # The three laws, and what each adds
///
/// | Variant | Upstream class | Adds |
/// |---|---|---|
/// | [`Elastic`](Self::Elastic) | `elasticity` | nothing — Hooke's law |
/// | [`MisesPlasticity`](Self::MisesPlasticity) | `misesPlasticity` | a yield threshold; deformation above it is permanent |
/// | [`MisesPlasticCreep`](Self::MisesPlasticCreep) | `misesPlasticCreep` | a time-dependent flow with no threshold |
///
/// Enum dispatch, per the workspace rule — no `Box<dyn>`, no trait objects. The
/// set of laws is closed and known at compile time, so adding one is a compile
/// error at every match site.
///
/// # The equation each law solves
///
/// All three share one additive decomposition of the **mechanical** strain
/// (i.e. the total strain with the eigenstrain already removed):
///
/// `ε_mech = ε_el + ε_p + ε_c`
///
/// and one stress–strain relation, isotropic and linear in the *elastic* part:
///
/// `σ = K tr(ε_el) I + 2μ dev(ε_el)`
///
/// What distinguishes them is how `ε_p` and `ε_c` evolve. Elasticity: they do
/// not. Plasticity: `ε_p` grows whenever the trial stress would otherwise
/// exceed the yield surface, by exactly enough to land back on it. Creep: `ε_c`
/// grows at a rate set by stress and temperature, always.
///
/// # Order of operations
///
/// Creep first, then plasticity — matching upstream's `misesPlasticCreep`,
/// which calls `creepModel_->correctCreep(...)` and then delegates to
/// `misesPlasticity::correct(...)` on the already creep-reduced strain. Creep
/// relaxes the stress, so doing it first means less of the increment has to be
/// taken up plastically; the reverse order over-predicts plastic strain.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstitutiveLaw {
    /// Isotropic linear elasticity — Hooke's law, no permanent deformation.
    ///
    /// Upstream `elasticity`. The right law for a material that never yields in
    /// the case being run (an unloaded spacer, a stiff structural part) and the
    /// baseline every other law must reduce to below yield.
    Elastic,

    /// Rate-independent von Mises (J2) plasticity with isotropic hardening.
    ///
    /// Upstream `misesPlasticity`. The material behaves elastically until the
    /// von Mises stress reaches `σ_y`, then flows so as to stay exactly on the
    /// yield surface, hardening as it goes. Solved by a **radial return
    /// mapping**: take the elastic trial stress, and if it lies outside the
    /// yield surface, pull it radially back onto the surface in the deviatoric
    /// plane. Volume-preserving, so the hydrostatic stress is untouched.
    MisesPlasticity {
        /// How `σ_y` depends on accumulated plastic strain, temperature,
        /// fluence and cold work.
        yield_stress: YieldStressModel,
    },

    /// Von Mises plasticity with a superimposed creep flow.
    ///
    /// Upstream `misesPlasticCreep`. This is the law an actual fuel rod needs:
    /// cladding creeps down onto the pellet over months (creep), and may also
    /// yield during a power ramp or a pellet-cladding interaction (plasticity).
    /// Neither alone reproduces the observed deformation history.
    MisesPlasticCreep {
        /// How `σ_y` depends on accumulated plastic strain and state.
        yield_stress: YieldStressModel,
        /// How fast the material creeps at a given stress and temperature.
        creep: CreepModel,
    },
}

impl ConstitutiveLaw {
    /// A perfectly plastic von Mises material with a fixed yield stress
    /// \[Pa\] — the simplest useful plasticity setup.
    ///
    /// ```
    /// use outram_park_fork_offbeat::rheology::ConstitutiveLaw;
    ///
    /// let law = ConstitutiveLaw::perfectly_plastic(300.0e6);
    /// assert!(matches!(law, ConstitutiveLaw::MisesPlasticity { .. }));
    /// ```
    #[must_use]
    pub fn perfectly_plastic(sigma_y: f64) -> Self {
        Self::MisesPlasticity {
            yield_stress: YieldStressModel::Constant { sigma_y },
        }
    }

    /// The creep model this law uses, or [`CreepModel::None`] if it has none.
    #[must_use]
    pub fn creep_model(&self) -> CreepModel {
        match self {
            Self::Elastic | Self::MisesPlasticity { .. } => CreepModel::None,
            Self::MisesPlasticCreep { creep, .. } => *creep,
        }
    }

    /// Integrate the law over one timestep in one cell, returning the corrected
    /// stress and every internal variable that advanced.
    ///
    /// This is the port of upstream's
    /// `constitutiveLaw::correct(sigmaHyd, sigmaDev, epsEl, addr)` followed by
    /// `updateStress(...)`, collapsed to a single cell and a single return
    /// value.
    ///
    /// # Arguments
    ///
    /// - `cell` — the cell index, used only to name the cell in a
    ///   non-convergence error. Pass 0 if there is no mesh.
    /// - `inputs` — this cell's elastic constants, **mechanical** strain
    ///   (eigenstrain already subtracted — see
    ///   [`RheologyInputs::mechanical_strain`]), material state, irradiation
    ///   state and timestep.
    /// - `state` — this cell's accumulated plastic and creep history at the
    ///   **start** of the step. Not mutated; commit the result with
    ///   [`RheologyState::advance`] once the outer mechanics iteration has
    ///   converged.
    ///
    /// # Errors
    ///
    /// - [`OffbeatError::ConstitutiveNotConverged`] if either local iteration
    ///   (creep Newton or plastic return map) fails to reach tolerance. The
    ///   stress is **not** returned in that case — an unconverged stress that
    ///   sits outside the yield surface would propagate silently into the
    ///   momentum balance and look like a stiff material.
    /// - [`OffbeatError::OutOfRange`] / [`OffbeatError::Unphysical`] if a
    ///   correlation is evaluated where it is not defined.
    ///
    /// # Example — elastic loading below yield returns Hooke's law untouched
    ///
    /// ```
    /// use outram_foam_basic_lib::primitives::SymmTensor;
    /// use outram_park_fork_offbeat::materials::MaterialState;
    /// use outram_park_fork_offbeat::mechanics::LinearElastic;
    /// use outram_park_fork_offbeat::rheology::{
    ///     ConstitutiveLaw, RheologyInputs, RheologyState,
    /// };
    ///
    /// let elastic = LinearElastic::new(100.0e9, 0.3).unwrap();
    /// // Pure shear of 1e-4: von Mises stress is 2 mu * 1e-4 * sqrt(3).
    /// let strain = SymmTensor::new(0.0, 1.0e-4, 0.0, 0.0, 0.0, 0.0);
    /// let inputs =
    ///     RheologyInputs::quasi_static(elastic, strain, MaterialState::fresh(600.0));
    ///
    /// let law = ConstitutiveLaw::perfectly_plastic(300.0e6);
    /// let out = law.correct(0, &inputs, &RheologyState::pristine()).unwrap();
    ///
    /// assert!(!out.yielding);
    /// assert!((out.stress.xy - 2.0 * elastic.shear_modulus() * 1.0e-4).abs() < 1.0);
    /// ```
    pub fn correct(
        &self,
        cell: usize,
        inputs: &RheologyInputs,
        state: &RheologyState,
    ) -> Result<StressCorrection> {
        if !(inputs.dt >= 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "timestep",
                value: inputs.dt,
                unit: "s",
                reason: "must be non-negative",
            });
        }

        let mu = inputs.elastic.shear_modulus();
        let bulk = inputs.elastic.three_k() / 3.0;

        // Step 1 — remove the inelastic strain already accumulated in previous
        // converged steps. Upstream does this in `correctEpsilonEl` (plastic)
        // and at the top of `correctCreep` (creep).
        let trial_strain = inputs.mechanical_strain - state.plastic_strain - state.creep_strain;

        // Step 2 — creep. The trial deviatoric stress and its von Mises norm.
        let s_trial = 2.0 * mu * trial_strain.dev();
        let q_trial = von_mises(s_trial);

        let creep = self
            .creep_model()
            .increment(cell, q_trial, mu, inputs, state)?;

        // Prandtl-Reuss flow: the creep strain flows along the deviatoric
        // stress direction, which radial return leaves unchanged. Guarding on
        // `q_trial` rather than on the increment keeps the direction well
        // defined at a hydrostatic state.
        let creep_increment_tensor = if q_trial > 0.0 && creep.equivalent > 0.0 {
            1.5 * (creep.equivalent / q_trial) * s_trial
        } else {
            SymmTensor::ZERO
        };
        let strain_after_creep = trial_strain - creep_increment_tensor;

        // Step 3 — plastic return mapping on the creep-reduced strain.
        let plastic = self.return_map(cell, inputs, state, strain_after_creep, mu)?;

        let elastic_strain = strain_after_creep - plastic.increment_tensor;

        // Step 4 — assemble the stress. Both inelastic increments are
        // deviatoric, so the trace (and hence the hydrostatic stress) is that of
        // the trial strain; writing it from `elastic_strain` keeps the one
        // formula `sigma = K tr(eps_el) I + 2 mu dev(eps_el)` visible in one
        // place, as upstream's `updateStress` does with sigmaHyd and sigmaDev.
        let stress =
            bulk * elastic_strain.tr() * SymmTensor::IDENTITY + 2.0 * mu * elastic_strain.dev();

        Ok(StressCorrection {
            stress,
            elastic_strain,
            plastic_strain_increment: plastic.increment_tensor,
            equivalent_plastic_strain_increment: plastic.equivalent_increment,
            creep_strain_increment: creep_increment_tensor,
            equivalent_creep_strain_increment: creep.equivalent,
            equivalent_primary_creep_strain_increment: creep.equivalent_primary,
            yield_stress: plastic.yield_stress,
            yielding: plastic.equivalent_increment > 0.0,
            iterations: creep.iterations + plastic.iterations,
        })
    }

    /// Radial return mapping onto the von Mises yield surface.
    ///
    /// Solves the consistency condition for the plastic multiplier `Δλ`:
    ///
    /// `|s_trial| − 2μ Δλ − sqrt(2/3) σ_y(ε_p,eq + sqrt(2/3) Δλ) = 0`
    ///
    /// by Newton iteration, whose derivative is `−2μ − (2/3) H` with
    /// `H = dσ_y/dε_p,eq`. For a constant or linearly hardening yield stress
    /// this converges in a single step and reproduces the closed-form
    /// `Δλ = f_trial / (2μ + 2H/3)` that upstream writes directly as
    /// `DLambda = fTrial/(2μ) / (1 + Hp/(3μ))`.
    ///
    /// # Why iterate where upstream does not
    ///
    /// Upstream takes `Hp` as a secant slope over the previous outer iteration
    /// and relies on the mechanics corrector loop to converge the pair. That
    /// works, but it means the returned stress can sit outside the yield
    /// surface at any intermediate iteration, and there is no local error to
    /// report. Iterating here makes the per-cell answer exact for any hardening
    /// curve, and gives
    /// [`OffbeatError::ConstitutiveNotConverged`] something honest to report.
    fn return_map(
        &self,
        cell: usize,
        inputs: &RheologyInputs,
        state: &RheologyState,
        strain: SymmTensor,
        mu: f64,
    ) -> Result<PlasticReturn> {
        let yield_model = match self {
            Self::Elastic => {
                return Ok(PlasticReturn {
                    increment_tensor: SymmTensor::ZERO,
                    equivalent_increment: 0.0,
                    yield_stress: 0.0,
                    iterations: 0,
                })
            }
            Self::MisesPlasticity { yield_stress } => yield_stress,
            Self::MisesPlasticCreep { yield_stress, .. } => yield_stress,
        };

        let eq_plastic_old = state.equivalent_plastic_strain.max(0.0);
        let s_trial = 2.0 * mu * strain.dev();
        let s_norm = s_trial.mag();
        let sigma_y_old = yield_model.yield_stress(eq_plastic_old, inputs)?;

        const SQRT_TWO_THIRDS: f64 = 0.816_496_580_927_726; // sqrt(2/3)

        // Trial yield function. Below zero the step is purely elastic and the
        // yield surface is untouched — this is the branch a well-behaved fuel
        // rod spends almost all of its life in.
        let f_trial = s_norm - SQRT_TWO_THIRDS * sigma_y_old;
        if f_trial <= 0.0 || !(mu > 0.0) {
            return Ok(PlasticReturn {
                increment_tensor: SymmTensor::ZERO,
                equivalent_increment: 0.0,
                yield_stress: sigma_y_old,
                iterations: 0,
            });
        }

        let tolerance = PLASTIC_REL_TOL * s_norm.max(sigma_y_old);
        let mut d_lambda = 0.0_f64;
        let mut iterations = 0;
        let mut residual = f_trial;

        for iteration in 1..=MAX_PLASTIC_ITERATIONS {
            iterations = iteration;
            let eq_plastic = eq_plastic_old + SQRT_TWO_THIRDS * d_lambda;
            let sigma_y = yield_model.yield_stress(eq_plastic, inputs)?;
            residual = s_norm - 2.0 * mu * d_lambda - SQRT_TWO_THIRDS * sigma_y;

            if residual.abs() <= tolerance {
                break;
            }

            let hardening = yield_model.hardening_modulus(eq_plastic, inputs)?;
            // d(residual)/d(d_lambda) = -2 mu - (2/3) H.
            let slope = -2.0 * mu - (2.0 / 3.0) * hardening;
            if !(slope.is_finite() && slope < 0.0) {
                return Err(OffbeatError::ConstitutiveNotConverged {
                    cell,
                    residual: residual.abs(),
                    iterations: iteration,
                });
            }
            let next = d_lambda - residual / slope;
            if !next.is_finite() {
                return Err(OffbeatError::ConstitutiveNotConverged {
                    cell,
                    residual: residual.abs(),
                    iterations: iteration,
                });
            }
            // The plastic multiplier cannot be negative, and it cannot exceed
            // the value that would take the deviatoric stress through zero.
            d_lambda = next.clamp(0.0, s_norm / (2.0 * mu));
        }

        if residual.abs() > tolerance {
            return Err(OffbeatError::ConstitutiveNotConverged {
                cell,
                residual: residual.abs(),
                iterations,
            });
        }

        // Return direction: the unit deviatoric trial stress. Radial return
        // preserves it, which is why the yield surface can be reached in one
        // scalar unknown rather than five.
        let normal = if s_norm > 0.0 {
            s_trial / s_norm
        } else {
            SymmTensor::ZERO
        };
        let equivalent_increment = SQRT_TWO_THIRDS * d_lambda;
        let eq_plastic_new = eq_plastic_old + equivalent_increment;

        Ok(PlasticReturn {
            increment_tensor: (d_lambda * normal).dev(),
            equivalent_increment,
            yield_stress: yield_model.yield_stress(eq_plastic_new, inputs)?,
            iterations,
        })
    }
}

/// Outcome of the radial return mapping. Private: it exists only to keep
/// [`ConstitutiveLaw::correct`] readable.
struct PlasticReturn {
    /// Plastic strain increment tensor `Δε_p` \[-\], deviatoric.
    increment_tensor: SymmTensor,
    /// Equivalent plastic strain increment `Δε_p,eq` \[-\], non-negative.
    equivalent_increment: f64,
    /// Yield stress \[Pa\] at the end of the step.
    yield_stress: f64,
    /// Newton iterations taken.
    iterations: usize,
}
