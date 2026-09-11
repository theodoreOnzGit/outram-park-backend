// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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
//
// ---------------------------------------------------------------------------
// Ported from:
//   Project:  PRISMS-Plasticity (prisms-center/plasticity)
//   Source:   src/materialModels/crystalPlasticity/MaterialModels/
//             RateDependentModel/calculatePlasticity.cc
//   Version:  commit ffdf4eb67b55b84f8b20cbb21407cf310ec3a7e4 (2026-08-27)
//   Copyright (c) 2016 The Regents of the University of Michigan, PRISMS Center
//   Licence:  LGPL-2.1-or-later upstream; relicensed to GPL-3.0-only here under
//             LGPL-2.1 section 3. See crates/farrer-park/NOTICE and
//             crates/farrer-park/docs/upstream-provenance.md.
//
//   WHAT IS TAKEN FROM UPSTREAM, AND WHAT IS NOT — read this before comparing
//   the two files, because they do not look alike:
//
//   Taken:  the slip-system tables and their ordering; `R S R^T` into sample
//           axes; the power-law flow rule and its parameter meanings; the
//           saturating hardening law with the q-matrix and, critically, the
//           clamp of s at the saturation stress; starting the local Newton
//           iteration from the previously converged stress; a line search on
//           the local residual.
//
//   NOT taken: the finite-deformation kinematics. Upstream carries an elastic
//           and a plastic deformation gradient with an exponential update of
//           Fp, a second Piola-Kirchhoff stress on the intermediate
//           configuration, lattice reorientation from the plastic spin, and
//           deformation-increment sub-stepping. Farrer Park is a **small
//           strain** code (see the crate `CLAUDE.md`), so this module uses the
//           additive split eps = eps_e + eps_p with eps_p driven by the
//           symmetric Schmid tensors. That is a deliberate reduction, not a
//           port defect; its consequences are listed under "Limitations" in
//           the type documentation of `CrystalPlasticity`.
// ---------------------------------------------------------------------------

//! The crystal-plasticity constitutive integrator: one strain in, one stress
//! and one **consistent algorithmic tangent** out.
//!
//! # Reuse
//!
//! The local `6 x 6` linear solves use
//! [`outram_foam_basic_lib::matrix::SquareMatrix`]'s LU with scaled partial
//! pivoting rather than a hand-rolled elimination. It is the workspace's dense
//! direct solver and it is already tested there.
//!
//! # Units
//!
//! Stresses, slip resistances and tangent entries in pascals; strains, slips
//! and exponents dimensionless; the time increment in seconds.

use outram_foam_basic_lib::matrix::SquareMatrix;

use crate::crystal::elastic::CrystalElasticity;
use crate::crystal::flow::{PowerLawFlow, SaturatingHardening};
use crate::crystal::orient::Orientation;
use crate::crystal::slip::{SlipFamily, MAX_SLIP_SYSTEMS};
use crate::error::{FemError, Result};
use crate::material::{LinearElastic, MaterialState, StressUpdate};
use crate::tensor::{Tensor4, Voigt6, VOIGT};

/// Maximum local Newton iterations before the update is declared failed.
const MAX_LOCAL_ITERATIONS: usize = 100;

/// Maximum backtracking halvings in the local line search.
const MAX_LINE_SEARCH_STEPS: usize = 60;

/// Slip below which a point is not counted as "yielding" in the load-step
/// report. A rate-dependent law gives a non-zero slip increment at *any*
/// non-zero resolved shear stress, so a strict `d gamma != 0` test would call
/// every point plastic on every step and make the count meaningless.
/// Dimensionless.
const YIELDING_SLIP_THRESHOLD: f64 = 1.0e-10;

/// Rate-dependent crystal plasticity for one phase: elasticity, slip family,
/// flow rule, hardening law and the time increment they are integrated over.
///
/// # The algorithm, stated plainly
///
/// Small-strain additive split `eps = eps_e + eps_p`, with
///
/// - `sigma = C(orientation) : eps_e`, the stiffness rotated into sample axes;
/// - `tau_a = sigma : P_a` on each slip system, `P_a` the symmetric Schmid
///   tensor rotated into sample axes;
/// - `d gamma_a = gamma_dot_0 dt |tau_a / s_a|^(1/m) sign(tau_a)`;
/// - `eps_p = eps_p^n + sum_a d gamma_a P_a`;
/// - `s_a` advanced by the saturating self-and-latent law of
///   [`SaturatingHardening`].
///
/// The first four are solved **simultaneously** by a local Newton iteration on
/// the six stress components, which is what makes the slip increments implicit
/// (backward Euler) and the algorithmic tangent exact.
///
/// # Where this is only semi-implicit, and what it costs
///
/// The slip resistances `s_a` in the flow rule are held at their
/// **start-of-step** values throughout the local solve; they are advanced once
/// afterwards. PRISMS-Plasticity instead wraps an outer fixed-point loop
/// around the whole solve until `s` stops moving.
///
/// The consequence is a first-order-in-`ds` error in the stress over a step,
/// which vanishes as the step is refined, and it buys a tangent that is the
/// **exact** derivative of the update as implemented — not an approximation to
/// it. That exactness is verified against a numerical Jacobian and is what
/// keeps the global Newton iteration quadratic. Making the hardening implicit
/// as well would need the `18 x 18` coupled system differentiated, and is bead
/// `op-q75c`'s recorded follow-up.
///
/// # Limitations, stated rather than discovered
///
/// - **No lattice reorientation.** Texture evolution needs the plastic spin,
///   which a small-strain formulation does not carry. Results are meaningful
///   for strains of a few per cent, not for rolling or drawing.
/// - **No kinematic hardening / backstress.** Upstream's Ohno-Wang backstress
///   is not ported, so the Bauschinger effect is absent and a cyclic
///   simulation will not close a stable hysteresis loop for the right reason.
///   This is the main gap for the fatigue work; see [`crate::fatigue`].
/// - **No twinning**, no non-Schmid effects, and no `{112}`/`{123}` BCC
///   families.
/// - **One phase, one set of constants.** Orientation varies per quadrature
///   point through [`CrystalState`]; every other constant is uniform.
/// - **The time increment is a material constant**, so it is the same in every
///   load step. If the Newton solver cuts a step back (halving the load
///   increment), the time increment does **not** halve with it, and a
///   rate-dependent result from a cut-back run is therefore inconsistent. Use
///   `n_load_steps` large enough that no cutback occurs, and check
///   `NewtonReport::cutbacks`.
///
/// # Units
///
/// All stiffnesses and resistances in pascals, the reference slip rate per
/// second, the time increment in seconds, exponents and ratios dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrystalPlasticity {
    elasticity: CrystalElasticity,
    family: SlipFamily,
    flow: PowerLawFlow,
    hardening: SaturatingHardening,
    initial_slip_resistance: f64,
    time_increment: f64,
}

impl CrystalPlasticity {
    /// Assemble a crystal-plasticity material from its four parts.
    ///
    /// # Arguments
    ///
    /// - `elasticity` — single-crystal elastic constants \[Pa\].
    /// - `family` — which slip systems operate.
    /// - `flow` — the rate-dependent flow rule.
    /// - `hardening` — the saturating hardening law.
    /// - `initial_slip_resistance` — `s_0` \[Pa\], the critical resolved shear
    ///   stress of a virgin system. `16 MPa` in PRISMS-Plasticity's FCC deck.
    ///   Must be finite, strictly positive, and strictly less than the
    ///   hardening law's saturation resistance — a crystal that starts at
    ///   saturation cannot harden and is almost always a units mistake.
    /// - `time_increment` — `dt` \[s\], the time one load step represents.
    ///   Finite and strictly positive. Together with the reference slip rate it
    ///   sets the scale of slip per step: `gamma_dot_0 dt` is the slip a system
    ///   loaded exactly at its resistance accumulates in one step.
    ///
    /// # Errors
    ///
    /// [`FemError::MaterialOutOfRange`] if `initial_slip_resistance` or
    /// `time_increment` is outside the range above.
    pub fn new(
        elasticity: CrystalElasticity,
        family: SlipFamily,
        flow: PowerLawFlow,
        hardening: SaturatingHardening,
        initial_slip_resistance: f64,
        time_increment: f64,
    ) -> Result<Self> {
        if !(initial_slip_resistance > 0.0) || !initial_slip_resistance.is_finite() {
            return Err(FemError::MaterialOutOfRange {
                parameter: "initial_slip_resistance",
                value: initial_slip_resistance,
                unit: "Pa",
                reason: "must be finite and strictly positive",
            });
        }
        if initial_slip_resistance >= hardening.saturation_resistance() {
            return Err(FemError::MaterialOutOfRange {
                parameter: "initial_slip_resistance",
                value: initial_slip_resistance,
                unit: "Pa",
                reason: "must be strictly below the hardening saturation resistance",
            });
        }
        if !(time_increment > 0.0) || !time_increment.is_finite() {
            return Err(FemError::MaterialOutOfRange {
                parameter: "time_increment",
                value: time_increment,
                unit: "s",
                reason: "must be finite and strictly positive",
            });
        }
        Ok(Self {
            elasticity,
            family,
            flow,
            hardening,
            initial_slip_resistance,
            time_increment,
        })
    }

    /// The single-crystal elastic constants \[Pa\].
    #[must_use]
    pub fn elasticity(&self) -> CrystalElasticity {
        self.elasticity
    }

    /// Which slip family operates.
    #[must_use]
    pub fn family(&self) -> SlipFamily {
        self.family
    }

    /// The rate-dependent flow rule.
    #[must_use]
    pub fn flow(&self) -> PowerLawFlow {
        self.flow
    }

    /// The saturating hardening law.
    #[must_use]
    pub fn hardening(&self) -> SaturatingHardening {
        self.hardening
    }

    /// Initial critical resolved shear stress `s_0` \[Pa\].
    #[must_use]
    pub fn initial_slip_resistance(&self) -> f64 {
        self.initial_slip_resistance
    }

    /// The time increment `dt` \[s\] one load step represents.
    #[must_use]
    pub fn time_increment(&self) -> f64 {
        self.time_increment
    }

    /// A copy of this material with a different time increment \[s\].
    ///
    /// # Errors
    ///
    /// [`FemError::MaterialOutOfRange`] if `dt` is not finite and positive.
    pub fn with_time_increment(&self, dt: f64) -> Result<Self> {
        CrystalPlasticity::new(
            self.elasticity,
            self.family,
            self.flow,
            self.hardening,
            self.initial_slip_resistance,
            dt,
        )
    }

    /// The virgin crystal state for the cube orientation: every slip
    /// resistance at `s_0`, no slip, no stress.
    ///
    /// Assign a real orientation with [`CrystalState::with_orientation`].
    #[must_use]
    pub fn pristine_state(&self) -> CrystalState {
        CrystalState {
            orientation: Orientation::identity(),
            slip_resistance: [self.initial_slip_resistance; MAX_SLIP_SYSTEMS],
            slip: [0.0; MAX_SLIP_SYSTEMS],
            total_slip: 0.0,
            stress: Voigt6::ZERO,
        }
    }

    /// An **isotropic-equivalent** elastic constant set \[Pa\], used only for
    /// scale estimates: the plane-stress initial guess and the convergence
    /// tolerances of [`crate::material::Material::update`].
    ///
    /// For [`CrystalElasticity::Isotropic`] it is the real thing. For
    /// [`CrystalElasticity::Cubic`] it is the **Voigt (uniform-strain)
    /// average** of the single crystal,
    ///
    /// `K = (c11 + 2 c12) / 3`,  `G = (c11 - c12 + 3 c44) / 5`,
    ///
    /// converted to `(E, nu)`. That is a genuine upper-bound polycrystal
    /// estimate, but it is **not** the constitutive law and is never used as
    /// one: the stress update always goes through the full rotated anisotropic
    /// tensor.
    ///
    /// # Panics
    ///
    /// Never for constants that passed [`CrystalElasticity::cubic`]'s Born
    /// stability check, which guarantees `K > 0` and `G > 0` and hence
    /// `-1 < nu < 0.5`.
    #[must_use]
    pub fn isotropic_equivalent(&self) -> LinearElastic {
        match self.elasticity {
            CrystalElasticity::Isotropic(e) => e,
            CrystalElasticity::Cubic { c11, c12, c44 } => {
                let k = (c11 + 2.0 * c12) / 3.0;
                let g = (c11 - c12 + 3.0 * c44) / 5.0;
                let e = 9.0 * k * g / (3.0 * k + g);
                let nu = (3.0 * k - 2.0 * g) / (2.0 * (3.0 * k + g));
                LinearElastic::new(e, nu)
                    .expect("Voigt average of Born-stable cubic constants is a valid isotropic pair")
            }
        }
    }

    /// Integrate one step at one quadrature point.
    ///
    /// # Arguments
    ///
    /// - `total_strain` — total small strain `eps` \[-\] at the **end** of the
    ///   step, engineering Voigt (shears doubled).
    /// - `state` — the committed history at the **start** of the step. Only
    ///   [`MaterialState::plastic_strain`],
    ///   [`MaterialState::equivalent_plastic_strain`] and
    ///   [`MaterialState::crystal`] are read.
    ///
    /// # Returns
    ///
    /// Stress \[Pa\], the consistent tangent `d sigma / d eps` \[Pa\], the
    /// end-of-step state, and whether the point slipped by more than
    /// `1e-10`.
    ///
    /// # Errors
    ///
    /// [`FemError::ConstitutiveNotConverged`] if the local Newton iteration
    /// does not reach a residual of `1e-9` times the stress scale of the point
    /// in 100 iterations, or if the `6 x 6` local Jacobian is singular. The
    /// element and point indices are filled with `0`; [`crate::assembly`]
    /// re-raises with the real ones.
    ///
    /// A failure here is nearly always a load step that is too large for the
    /// rate sensitivity: at `m = 0.02` the flow rule spans `2^50` over a
    /// factor-of-two change in `tau/s`, and no line search recovers from a
    /// starting guess that far out. Increase `NewtonSettings::n_load_steps`
    /// and shrink `time_increment` to match.
    pub fn update(&self, total_strain: Voigt6, state: &MaterialState) -> Result<StressUpdate> {
        let cs = state.crystal;
        let n = self.family.n_systems();

        // --- Geometry and elasticity in sample axes, computed once. --------
        let c = self.elasticity.stiffness_in_sample_frame(&cs.orientation);
        let mut p = [Voigt6::ZERO; MAX_SLIP_SYSTEMS]; // stress form
        let mut e = [Voigt6::ZERO; MAX_SLIP_SYSTEMS]; // engineering-strain form
        let mut ce = [Voigt6::ZERO; MAX_SLIP_SYSTEMS]; // C : P_a, a stress [Pa]
        for (a, sys) in self.family.schmid_tensors().iter().enumerate().take(n) {
            p[a] = cs.orientation.rotate_symmetric(sys);
            let v = p[a].as_array();
            e[a] = Voigt6::new(v[0], v[1], v[2], 2.0 * v[3], 2.0 * v[4], 2.0 * v[5]);
            ce[a] = c.apply(&e[a]);
        }

        // Guard against a hand-made state with a zero slip resistance, which
        // would divide by zero in the flow rule.
        let mut resistance = cs.slip_resistance;
        let mut s_min = f64::INFINITY;
        for r in resistance.iter_mut().take(n) {
            if !(*r > 0.0) || !r.is_finite() {
                *r = self.initial_slip_resistance;
            }
            s_min = s_min.min(*r);
        }

        let elastic_free = total_strain.minus(&state.plastic_strain);
        let sigma_trial = c.apply(&elastic_free);
        let scale = sigma_trial.abs_max().max(s_min).max(1.0);
        let tolerance = 1.0e-14 * scale;
        let failure_tolerance = 1.0e-9 * scale;

        // --- Initial guess. ------------------------------------------------
        // Upstream starts the local Newton from the previously converged
        // stress ("The guess stress to start the Newton-Raphson iteration is
        // the previously converged stress" — RateDependentModel header). On
        // the very first step there is none, so the elastic predictor is used,
        // with its DEVIATOR scaled back so that no system starts more than 5 %
        // above its resistance. Without that, |tau/s|^(1/m) at 1/m = 50 is
        // astronomically large and the first residual is infinite. The scaling
        // affects only where the iteration starts, never where it converges to;
        // the Schmid tensors are deviatoric, so the mean stress is untouched
        // and the guess stays a valid stress state.
        let mut sigma = if cs.stress == Voigt6::ZERO && cs.total_slip == 0.0 {
            let mut worst = 0.0_f64;
            for a in 0..n {
                worst = worst.max(sigma_trial.work_with_strain(&e[a]).abs() / resistance[a]);
            }
            if worst > 1.05 {
                let mean = sigma_trial.mean_stress();
                let dev = sigma_trial.stress_deviator().scaled(1.05 / worst);
                dev.plus(&Voigt6::IDENTITY.scaled(mean))
            } else {
                sigma_trial
            }
        } else {
            cs.stress
        };

        // --- Local Newton with backtracking line search. -------------------
        let mut d_gamma = [0.0_f64; MAX_SLIP_SYSTEMS];
        let mut residual = self.residual(&c, &e, &resistance, total_strain, state, n, &sigma, &mut d_gamma);
        let mut norm = voigt_norm(&residual);
        let mut jac = SquareMatrix::new(VOIGT);
        let mut rhs: Vec<f64> = vec![0.0; VOIGT];
        let mut trial_gamma = [0.0_f64; MAX_SLIP_SYSTEMS];
        let mut iterations = 0usize;

        while norm > tolerance {
            if iterations >= MAX_LOCAL_ITERATIONS {
                break;
            }
            // Jacobian dG/dsigma = I + sum_a k_a (C e_a) (x) e_a.
            jac.fill_zero();
            for i in 0..VOIGT {
                jac.set(i, i, 1.0);
            }
            for a in 0..n {
                let tau = sigma.work_with_strain(&e[a]);
                let k = self
                    .flow
                    .slip_increment_derivative(tau, resistance[a], self.time_increment);
                if k == 0.0 {
                    continue;
                }
                for i in 0..VOIGT {
                    for j in 0..VOIGT {
                        jac.add(i, j, k * ce[a].0[i] * e[a].0[j]);
                    }
                }
            }
            let pivot = jac.lu_decompose();
            for i in 0..VOIGT {
                if jac.get(i, i) == f64::EPSILON {
                    return Err(FemError::ConstitutiveNotConverged {
                        element: 0,
                        point: 0,
                        iterations,
                        residual: norm,
                    });
                }
            }
            for i in 0..VOIGT {
                rhs[i] = -residual.0[i];
            }
            jac.lu_back_substitute(&pivot, &mut rhs);
            let step = Voigt6([rhs[0], rhs[1], rhs[2], rhs[3], rhs[4], rhs[5]]);

            // Backtracking: accept the first alpha that reduces the residual.
            let mut alpha = 1.0_f64;
            let mut accepted = false;
            for _ in 0..MAX_LINE_SEARCH_STEPS {
                let candidate = sigma.plus(&step.scaled(alpha));
                let r = self.residual(
                    &c, &e, &resistance, total_strain, state, n, &candidate, &mut trial_gamma,
                );
                let nr = voigt_norm(&r);
                if nr.is_finite() && nr < norm * (1.0 - 1.0e-4 * alpha) {
                    sigma = candidate;
                    residual = r;
                    d_gamma = trial_gamma;
                    norm = nr;
                    accepted = true;
                    break;
                }
                alpha *= 0.5;
            }
            iterations += 1;
            if !accepted {
                // No descent direction left: either converged to round-off or
                // genuinely stuck. The check after the loop tells them apart.
                break;
            }
        }

        if norm > failure_tolerance {
            return Err(FemError::ConstitutiveNotConverged {
                element: 0,
                point: 0,
                iterations,
                residual: norm,
            });
        }

        // --- Consistent tangent: D = J^{-1} C. -----------------------------
        // Differentiating G(sigma(eps), eps) = 0 gives J dsigma = C deps, so
        // column j of the tangent is J^{-1} applied to column j of C.
        jac.fill_zero();
        for i in 0..VOIGT {
            jac.set(i, i, 1.0);
        }
        for a in 0..n {
            let tau = sigma.work_with_strain(&e[a]);
            let k = self
                .flow
                .slip_increment_derivative(tau, resistance[a], self.time_increment);
            if k == 0.0 {
                continue;
            }
            for i in 0..VOIGT {
                for j in 0..VOIGT {
                    jac.add(i, j, k * ce[a].0[i] * e[a].0[j]);
                }
            }
        }
        let pivot = jac.lu_decompose();
        let mut tangent = [[0.0_f64; VOIGT]; VOIGT];
        for j in 0..VOIGT {
            for i in 0..VOIGT {
                rhs[i] = c.0[i][j];
            }
            jac.lu_back_substitute(&pivot, &mut rhs);
            for i in 0..VOIGT {
                tangent[i][j] = rhs[i];
            }
        }

        // --- Commit the state. ---------------------------------------------
        let mut d_eps_p_stress = Voigt6::ZERO; // tensor (stress) form
        let mut d_eps_p_eng = Voigt6::ZERO; // engineering-strain form
        let mut total = 0.0_f64;
        let mut slip = cs.slip;
        for a in 0..n {
            d_eps_p_stress = d_eps_p_stress.plus(&p[a].scaled(d_gamma[a]));
            d_eps_p_eng = d_eps_p_eng.plus(&e[a].scaled(d_gamma[a]));
            slip[a] += d_gamma[a];
            total += d_gamma[a].abs();
        }
        let d_eq =
            (2.0 / 3.0 * d_eps_p_stress.stress_double_dot(&d_eps_p_stress)).sqrt();
        let q = self
            .family
            .latent_hardening_matrix(self.hardening.self_ratio(), self.hardening.latent_ratio());
        let new_resistance = self.hardening.advance(&q, &resistance, &d_gamma, n);

        Ok(StressUpdate {
            stress: sigma,
            tangent: Tensor4(tangent),
            state: MaterialState {
                plastic_strain: state.plastic_strain.plus(&d_eps_p_eng),
                equivalent_plastic_strain: state.equivalent_plastic_strain + d_eq,
                crystal: CrystalState {
                    orientation: cs.orientation,
                    slip_resistance: new_resistance,
                    slip,
                    total_slip: cs.total_slip + total,
                    stress: sigma,
                },
            },
            yielding: total > YIELDING_SLIP_THRESHOLD,
        })
    }

    /// The local residual `G(sigma) = sigma - C : (eps - eps_p^n - sum_a
    /// d gamma_a P_a)` \[Pa\], writing the slip increments it computed into
    /// `d_gamma` \[-\].
    #[allow(clippy::too_many_arguments)]
    fn residual(
        &self,
        c: &Tensor4,
        e: &[Voigt6; MAX_SLIP_SYSTEMS],
        resistance: &[f64; MAX_SLIP_SYSTEMS],
        total_strain: Voigt6,
        state: &MaterialState,
        n: usize,
        sigma: &Voigt6,
        d_gamma: &mut [f64; MAX_SLIP_SYSTEMS],
    ) -> Voigt6 {
        let mut eps_p = state.plastic_strain;
        for a in 0..n {
            let tau = sigma.work_with_strain(&e[a]);
            d_gamma[a] = self
                .flow
                .slip_increment(tau, resistance[a], self.time_increment);
            eps_p = eps_p.plus(&e[a].scaled(d_gamma[a]));
        }
        sigma.minus(&c.apply(&total_strain.minus(&eps_p)))
    }
}

/// Euclidean norm of the six components \[same units as the argument\].
/// Returns `+inf` if any component is not finite, so the line search rejects
/// the step rather than propagating a NaN.
#[inline]
fn voigt_norm(v: &Voigt6) -> f64 {
    let mut s = 0.0;
    for x in v.as_array() {
        if !x.is_finite() {
            return f64::INFINITY;
        }
        s += x * x;
    }
    s.sqrt()
}

/// The per-quadrature-point crystal-plasticity history.
///
/// Carried inside [`MaterialState`], so it exists at every quadrature point
/// whatever the material is; for an elastic or J2 analysis it stays at its
/// [`Default`] value and is never read.
///
/// # Units
///
/// `slip_resistance` and `stress` in pascals; `slip` and `total_slip`
/// dimensionless; `orientation` dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrystalState {
    /// The grain orientation at this point, crystal-to-sample. **Fixed for the
    /// life of the analysis** — there is no lattice reorientation.
    pub orientation: Orientation,
    /// Slip resistance `s_a` \[Pa\] on each system, non-decreasing, bounded
    /// above by the hardening law's saturation resistance.
    pub slip_resistance: [f64; MAX_SLIP_SYSTEMS],
    /// **Signed** accumulated slip `gamma_a` \[-\] on each system. Reversing
    /// the load reduces it; it is the quantity a fatigue indicator parameter
    /// takes a range of over a cycle.
    pub slip: [f64; MAX_SLIP_SYSTEMS],
    /// Accumulated slip magnitude `sum_a integral |d gamma_a|` \[-\], never
    /// decreasing. The scalar measure of how much this point has worked.
    pub total_slip: f64,
    /// The converged stress \[Pa\] at the end of the last committed step, used
    /// as the local Newton iteration's starting guess on the next one.
    pub stress: Voigt6,
}

impl Default for CrystalState {
    /// Cube orientation, zero resistances, no slip, no stress.
    ///
    /// **Zero slip resistances are not a usable crystal state** — they are the
    /// "this point is not a crystal" marker that an elastic or J2 analysis
    /// leaves in place. [`CrystalPlasticity::pristine_state`] is what a
    /// crystal-plasticity analysis starts from, and
    /// [`crate::material::Material::initial_state`] picks the right one.
    fn default() -> Self {
        Self {
            orientation: Orientation::identity(),
            slip_resistance: [0.0; MAX_SLIP_SYSTEMS],
            slip: [0.0; MAX_SLIP_SYSTEMS],
            total_slip: 0.0,
            stress: Voigt6::ZERO,
        }
    }
}

impl CrystalState {
    /// This state with a different grain orientation. Dimensionless argument.
    #[must_use]
    pub fn with_orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// The resolved shear stress `tau_a = sigma : P_a` \[Pa\] on every system
    /// of `family`, for the stress `sigma` \[Pa\] given in **sample** axes.
    ///
    /// Entries beyond `family.n_systems()` are zero. This is the quantity a
    /// Schmid-factor check and a fatigue indicator parameter both need, so it
    /// is public rather than buried in the update.
    #[must_use]
    pub fn resolved_shear_stresses(
        &self,
        family: SlipFamily,
        sigma: &Voigt6,
    ) -> [f64; MAX_SLIP_SYSTEMS] {
        let mut out = [0.0_f64; MAX_SLIP_SYSTEMS];
        for (a, s) in family.schmid_tensors().iter().enumerate().take(family.n_systems()) {
            out[a] = sigma.stress_double_dot(&self.orientation.rotate_symmetric(s));
        }
        out
    }
}
