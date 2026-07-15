// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/pointKinetics/
//                    (pointKineticNeutronics.{C,H},
//                     include/solvePointKinetics.H, include/correctReactivity.H)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Stefan Radman, Thomas Guilbaud (EPFL)
//   Upstream license: GPL-3.0
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
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # Point-kinetics neutronics (0-D)
//!
//! The lumped-parameter ("point") reactor-kinetics model: the reactor is treated
//! as a single point whose fission power `P(t)` and delayed-neutron precursor
//! "powers" `C_i(t)` evolve in time under a prescribed total reactivity. This is
//! the fastest neutronics option in GeN-Foam and the physics core that its
//! spatial models (diffusion/SP3/SN) reduce to when only the amplitude is
//! tracked.
//!
//! ## The equations
//!
//! With `N` delayed-neutron precursor groups, prompt-neutron generation time
//! `Λ` (seconds), total reactivity `ρ` (dimensionless, `Δk/k`), optional
//! subcritical index `ζ` (dimensionless), and optional external source power
//! `S` (watts):
//!
//! ```text
//!   dP/dt   = (ρ − ζ − β)/Λ · P  +  Σ_i λ_i C_i  +  S/Λ
//!   dC_i/dt = β_i/Λ · P  −  λ_i C_i          for i = 1 … N
//! ```
//!
//! Here `β_i` is the effective delayed-neutron fraction of group `i`, `β = Σ β_i`
//! the total delayed fraction, and `λ_i` the decay constant of group `i` (`1/s`).
//! Following GeN-Foam, the precursor variables `C_i` are carried in **power units
//! (W)**, not neutron density, so that they scale directly with `P`.
//!
//! ## Numerical scheme — faithful to GeN-Foam
//!
//! GeN-Foam solves the system fully implicitly (backward Euler) as one dense
//! linear system per time step (`include/solvePointKinetics.H`). The unknown
//! vector is `x = [P, C_1, …, C_N, S]` of length `n = N + 2` (the `+2` is the
//! fission power and a trivial row that carries the external source into the
//! power balance). With `Δt` the time step and a superscript `old` denoting the
//! previous step's values, the assembled system `A x = B` is
//!
//! ```text
//!   row 0   (power):    [1/Δt − (ρ−ζ−β)/Λ] P  − Σ_i λ_i C_i  − (1/Λ) S = P^old / Δt
//!   row i   (group i):  −(β_i/Λ) P  +  [1/Δt + λ_i] C_i               = C_i^old / Δt
//!   row n−1 (source):   [1/Δt] S                                       = S^old / Δt
//! ```
//!
//! The dense solve uses [`outram_foam_basic_lib::matrix::SquareMatrix`] (Crout
//! LU with partial pivoting), the direct analogue of GeN-Foam's
//! `Foam::scalarSquareMatrix` + `solve()`.
//!
//! ## Scope of this port
//!
//! This module ports the **reactivity-driven 0-D ODE core** only. The full
//! GeN-Foam `pointKineticNeutronics` class additionally computes `ρ` from
//! temperature/density feedback fields on an `fvMesh` (Doppler, fuel/clad/coolant
//! /structure), GEM and control-rod-driveline reactivity, an external neutron
//! source with power-monitoring modulation, FMU inputs, and a liquid-fuel
//! precursor-advection variant. Those all require the mesh / thermal-hydraulics /
//! multi-region layers and are **deferred** (see `docs/genfoam-port-plan.md`).
//! Here, the total reactivity `ρ` and the external source power `S` are inputs
//! the caller supplies each step.
//!
//! ## Example
//!
//! ```
//! use outram_foam_appbuilder_lib::genfoam::neutronics::point_kinetics::{
//!     PointKineticsParameters, PointKineticsState,
//! };
//! use uom::si::f64::{Power, Ratio, Time, Frequency};
//! use uom::si::power::watt;
//! use uom::si::ratio::ratio;
//! use uom::si::time::second;
//! use uom::si::frequency::hertz;
//!
//! // One delayed group, purely illustrative constants.
//! let params = PointKineticsParameters::new(
//!     vec![Ratio::new::<ratio>(0.0065)],      // β_1
//!     vec![Frequency::new::<hertz>(0.08)],    // λ_1  [1/s]
//!     Time::new::<second>(1.0e-4),            // Λ    [s]
//! )
//! .unwrap();
//!
//! // Start critical: precursors at their equilibrium level for 1 MW.
//! let mut state = PointKineticsState::new_equilibrium(&params, Power::new::<watt>(1.0e6));
//!
//! // Advance one 1 ms step at zero reactivity, no external source.
//! state
//!     .step(
//!         &params,
//!         Time::new::<second>(1.0e-3),
//!         Ratio::new::<ratio>(0.0),
//!         Power::new::<watt>(0.0),
//!     )
//!     .unwrap();
//!
//! // At exact equilibrium and ρ = 0 the power is unchanged.
//! assert!((state.fission_power().get::<watt>() - 1.0e6).abs() < 1.0);
//! ```

use outram_foam_basic_lib::matrix::{MatrixError, SquareMatrix};
use uom::si::f64::{Frequency, Power, Ratio, Time};
use uom::si::frequency::hertz;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::time::second;

/// Total reactivity `ρ` or a delayed-neutron fraction `β` — a dimensionless
/// ratio (`Δk/k`). Named alias over `uom`'s [`Ratio`] for human readability.
pub type Reactivity = Ratio;

/// Delayed-neutron precursor decay constant `λ_i` — inverse time (`1/s`). Named
/// alias over `uom`'s [`Frequency`] (both are `s⁻¹`).
pub type DecayConstant = Frequency;

/// Prompt-neutron generation time `Λ` — a [`Time`] (seconds). Named alias for
/// readability at call sites.
pub type PromptGenerationTime = Time;

/// Errors from constructing or advancing a point-kinetics model.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PointKineticsError {
    /// The delayed-fraction and decay-constant lists have different lengths;
    /// each precursor group needs exactly one `β_i` and one `λ_i`.
    #[error(
        "mismatched delayed-group data: {betas} beta value(s) but {lambdas} decay constant(s)"
    )]
    MismatchedGroups {
        /// Number of `β_i` values supplied.
        betas: usize,
        /// Number of `λ_i` values supplied.
        lambdas: usize,
    },

    /// No delayed groups were supplied; the model needs at least one.
    #[error("at least one delayed-neutron group is required")]
    NoDelayedGroups,

    /// The prompt-neutron generation time `Λ` was not strictly positive.
    #[error("prompt generation time must be positive, got {0} s")]
    NonPositivePromptGenerationTime(f64),

    /// A decay constant `λ_i` was not strictly positive.
    #[error("decay constant of group {group} must be positive, got {value} 1/s")]
    NonPositiveDecayConstant {
        /// Zero-based group index of the offending `λ_i`.
        group: usize,
        /// The offending value in `1/s`.
        value: f64,
    },

    /// The requested time step was not strictly positive.
    #[error("time step must be positive, got {0} s")]
    NonPositiveTimeStep(f64),

    /// The implicit linear system was singular (degenerate parameters, e.g. a
    /// `Λ` or `Δt` that produced a zero pivot).
    #[error("point-kinetics linear system is singular at column {col}")]
    SingularSystem {
        /// Column at which the LU decomposition found a zero pivot.
        col: usize,
    },
}

impl From<MatrixError> for PointKineticsError {
    fn from(e: MatrixError) -> Self {
        match e {
            MatrixError::Singular { col } => PointKineticsError::SingularSystem { col },
        }
    }
}

/// Time-invariant kinetics parameters of a reactor: the delayed-neutron data and
/// the prompt-generation time.
///
/// All fields carry `uom` dimensioned quantities. Construct with [`Self::new`]
/// (subcritical index `ζ` defaults to zero) or [`Self::with_subcritical_index`].
///
/// # Valid ranges / assumptions
///
/// - At least one delayed group; `β_i > 0` for a physical fraction (not checked
///   beyond non-emptiness), `λ_i > 0`.
/// - `Λ > 0`. Typical values: `Λ ≈ 10⁻⁴ s` (thermal reactors) to `10⁻⁷ s` (fast).
/// - `β_i` are effective fractions; their sum `β = Σ β_i` is a few ×10⁻³.
#[derive(Debug, Clone, PartialEq)]
pub struct PointKineticsParameters {
    /// Effective delayed-neutron fraction `β_i` of each group (dimensionless).
    delayed_fractions: Vec<Reactivity>,
    /// Decay constant `λ_i` of each group (`1/s`); same length as
    /// `delayed_fractions`.
    decay_constants: Vec<DecayConstant>,
    /// Prompt-neutron generation time `Λ` (seconds).
    prompt_generation_time: PromptGenerationTime,
    /// Subcritical index `ζ` (dimensionless anti-reactivity for an intentionally
    /// subcritical, source-driven core); defaults to zero.
    subcritical_index: Reactivity,
}

impl PointKineticsParameters {
    /// Build kinetics parameters with subcritical index `ζ = 0`.
    ///
    /// # Parameters
    ///
    /// - `delayed_fractions` — the `β_i` of each precursor group (dimensionless).
    /// - `decay_constants` — the `λ_i` of each group (`1/s`), same length/order.
    /// - `prompt_generation_time` — `Λ` (seconds), strictly positive.
    ///
    /// # Errors
    ///
    /// [`PointKineticsError::NoDelayedGroups`],
    /// [`PointKineticsError::MismatchedGroups`],
    /// [`PointKineticsError::NonPositivePromptGenerationTime`], or
    /// [`PointKineticsError::NonPositiveDecayConstant`].
    pub fn new(
        delayed_fractions: Vec<Reactivity>,
        decay_constants: Vec<DecayConstant>,
        prompt_generation_time: PromptGenerationTime,
    ) -> Result<Self, PointKineticsError> {
        Self::with_subcritical_index(
            delayed_fractions,
            decay_constants,
            prompt_generation_time,
            Ratio::new::<ratio>(0.0),
        )
    }

    /// Build kinetics parameters with an explicit subcritical index `ζ`.
    ///
    /// `ζ` (dimensionless, `≥ 0`) is the anti-reactivity of an intentionally
    /// subcritical, externally-driven core; it appears as `(ρ − ζ − β)/Λ` in the
    /// prompt term. Pass `Ratio::new::<ratio>(0.0)` for a self-sustaining core.
    ///
    /// # Errors
    ///
    /// See [`Self::new`].
    pub fn with_subcritical_index(
        delayed_fractions: Vec<Reactivity>,
        decay_constants: Vec<DecayConstant>,
        prompt_generation_time: PromptGenerationTime,
        subcritical_index: Reactivity,
    ) -> Result<Self, PointKineticsError> {
        if delayed_fractions.is_empty() {
            return Err(PointKineticsError::NoDelayedGroups);
        }
        if delayed_fractions.len() != decay_constants.len() {
            return Err(PointKineticsError::MismatchedGroups {
                betas: delayed_fractions.len(),
                lambdas: decay_constants.len(),
            });
        }
        let lambda = prompt_generation_time.get::<second>();
        if lambda <= 0.0 || lambda.is_nan() {
            return Err(PointKineticsError::NonPositivePromptGenerationTime(lambda));
        }
        for (group, l) in decay_constants.iter().enumerate() {
            let v = l.get::<hertz>();
            if v <= 0.0 || v.is_nan() {
                return Err(PointKineticsError::NonPositiveDecayConstant { group, value: v });
            }
        }
        Ok(Self {
            delayed_fractions,
            decay_constants,
            prompt_generation_time,
            subcritical_index,
        })
    }

    /// Number of delayed-neutron precursor groups `N`.
    pub fn delayed_group_count(&self) -> usize {
        self.delayed_fractions.len()
    }

    /// The effective delayed-neutron fractions `β_i` (dimensionless), one per
    /// group, in group order.
    pub fn delayed_fractions(&self) -> &[Reactivity] {
        &self.delayed_fractions
    }

    /// The decay constants `λ_i` (`1/s`), one per group, in group order.
    pub fn decay_constants(&self) -> &[DecayConstant] {
        &self.decay_constants
    }

    /// The prompt-neutron generation time `Λ` (seconds).
    pub fn prompt_generation_time(&self) -> PromptGenerationTime {
        self.prompt_generation_time
    }

    /// The subcritical index `ζ` (dimensionless).
    pub fn subcritical_index(&self) -> Reactivity {
        self.subcritical_index
    }

    /// Total effective delayed-neutron fraction `β = Σ_i β_i` (dimensionless).
    pub fn total_delayed_fraction(&self) -> Reactivity {
        let sum: f64 = self
            .delayed_fractions
            .iter()
            .map(|b| b.get::<ratio>())
            .sum();
        Ratio::new::<ratio>(sum)
    }
}

/// Instantaneous state of the point-kinetics model: fission power and the
/// per-group precursor "powers", plus the external source power carried into the
/// implicit solve.
///
/// All quantities are [`Power`] (watts). The precursor `C_i` are carried in power
/// units (GeN-Foam convention) so they scale directly with the fission power.
#[derive(Debug, Clone, PartialEq)]
pub struct PointKineticsState {
    /// Fission power `P` (watts).
    fission_power: Power,
    /// Precursor "power" `C_i` of each group (watts), in group order.
    precursor_powers: Vec<Power>,
    /// External source power `S` (watts) used in the most recent step.
    external_source_power: Power,
}

impl PointKineticsState {
    /// Construct the **critical steady state** for a given fission power.
    ///
    /// At criticality with `ρ = 0`, `ζ = 0`, `S = 0`, each precursor group sits
    /// at its equilibrium level `C_i = β_i P / (Λ λ_i)` (from `dC_i/dt = 0`).
    /// The external source power is initialised to zero.
    ///
    /// This is the usual starting point for a reactivity-insertion transient.
    ///
    /// # Parameters
    ///
    /// - `params` — the kinetics parameters (their `Λ`, `β_i`, `λ_i` set the
    ///   equilibrium).
    /// - `fission_power` — the steady fission power `P₀` (watts).
    pub fn new_equilibrium(params: &PointKineticsParameters, fission_power: Power) -> Self {
        let p = fission_power.get::<watt>();
        let lambda_gen = params.prompt_generation_time.get::<second>();
        let precursor_powers = params
            .delayed_fractions
            .iter()
            .zip(params.decay_constants.iter())
            .map(|(beta, lam)| {
                let b = beta.get::<ratio>();
                let l = lam.get::<hertz>();
                Power::new::<watt>(b * p / (lambda_gen * l))
            })
            .collect();
        Self {
            fission_power,
            precursor_powers,
            external_source_power: Power::new::<watt>(0.0),
        }
    }

    /// Construct a state from explicit fission power and precursor powers.
    ///
    /// `precursor_powers.len()` must equal the number of delayed groups the
    /// parameters were built with, or [`Self::step`] will produce a mismatched
    /// system; callers are responsible for consistency. The external source
    /// power is initialised to zero.
    pub fn new(fission_power: Power, precursor_powers: Vec<Power>) -> Self {
        Self {
            fission_power,
            precursor_powers,
            external_source_power: Power::new::<watt>(0.0),
        }
    }

    /// Current fission power `P` (watts).
    pub fn fission_power(&self) -> Power {
        self.fission_power
    }

    /// Current precursor powers `C_i` (watts), in group order.
    pub fn precursor_powers(&self) -> &[Power] {
        &self.precursor_powers
    }

    /// External source power `S` (watts) from the most recent [`Self::step`].
    pub fn external_source_power(&self) -> Power {
        self.external_source_power
    }

    /// Advance the state by one implicit (backward-Euler) time step.
    ///
    /// Assembles and solves the dense `n × n` system (`n = N + 2`) exactly as
    /// GeN-Foam's `solvePointKinetics.H` does (see the module-level docs), using
    /// the current state as the "old" values, then overwrites the fission power
    /// and precursor powers with the solution.
    ///
    /// # Parameters
    ///
    /// - `params` — the kinetics parameters; `params.delayed_group_count()` must
    ///   equal `self.precursor_powers().len()`.
    /// - `dt` — the time step `Δt` (seconds), strictly positive.
    /// - `reactivity` — the total reactivity `ρ` (dimensionless) applied over
    ///   this step. In the full GeN-Foam model this is the sum of all feedback
    ///   and external contributions; here the caller supplies it.
    /// - `external_source_power` — the external source power `S` (watts) for this
    ///   step; pass `Power::new::<watt>(0.0)` for a source-free core.
    ///
    /// # Errors
    ///
    /// [`PointKineticsError::NonPositiveTimeStep`],
    /// [`PointKineticsError::MismatchedGroups`] (state vs. params group count),
    /// or [`PointKineticsError::SingularSystem`].
    pub fn step(
        &mut self,
        params: &PointKineticsParameters,
        dt: Time,
        reactivity: Reactivity,
        external_source_power: Power,
    ) -> Result<(), PointKineticsError> {
        let n_groups = params.delayed_group_count();
        if self.precursor_powers.len() != n_groups {
            return Err(PointKineticsError::MismatchedGroups {
                betas: n_groups,
                lambdas: self.precursor_powers.len(),
            });
        }
        let dt_s = dt.get::<second>();
        if dt_s <= 0.0 || dt_s.is_nan() {
            return Err(PointKineticsError::NonPositiveTimeStep(dt_s));
        }

        // Unpack to SI base units for the numerical assembly.
        let lambda_gen = params.prompt_generation_time.get::<second>(); // Λ [s]
        let rho = reactivity.get::<ratio>(); // ρ [-]
        let zeta = params.subcritical_index.get::<ratio>(); // ζ [-]
        let beta_tot = params.total_delayed_fraction().get::<ratio>(); // β [-]
        let betas: Vec<f64> = params
            .delayed_fractions
            .iter()
            .map(|b| b.get::<ratio>())
            .collect();
        let lambdas: Vec<f64> = params
            .decay_constants
            .iter()
            .map(|l| l.get::<hertz>())
            .collect();

        let fission_old = self.fission_power.get::<watt>();
        let source = external_source_power.get::<watt>();

        // n = N delayed groups + 1 fission power + 1 external source.
        let n = n_groups + 2;
        let mut a = SquareMatrix::new(n);
        let mut b = vec![0.0_f64; n];

        // Row 0 — power balance.
        a.set(0, 0, 1.0 / dt_s - (rho - zeta - beta_tot) / lambda_gen);
        for i in 1..n - 1 {
            a.set(0, i, -lambdas[i - 1]);
        }
        b[0] = fission_old / dt_s;
        // External-source coupling into the power balance.
        a.set(0, n - 1, -1.0 / lambda_gen);

        // Rows 1..=N — precursor balances.
        for i in 1..n - 1 {
            a.set(i, 0, -betas[i - 1] / lambda_gen);
            a.set(i, i, 1.0 / dt_s + lambdas[i - 1]);
            b[i] = self.precursor_powers[i - 1].get::<watt>() / dt_s;
        }

        // Row n-1 — trivial external-source equation (S = S over the step).
        a.set(n - 1, n - 1, 1.0 / dt_s);
        b[n - 1] = source / dt_s;

        let x = a.solve(&b)?;

        self.fission_power = Power::new::<watt>(x[0]);
        for i in 0..n_groups {
            self.precursor_powers[i] = Power::new::<watt>(x[i + 1]);
        }
        self.external_source_power = external_source_power;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::power::watt;

    /// Six-group U-235 thermal delayed-neutron constants (illustrative unit-test
    /// fixture; the same public Keepin set is used, with fuller provenance, in
    /// the `genfoam_point_kinetics_inhour` V&V test).
    fn u235_six_group() -> PointKineticsParameters {
        let betas = [0.000215, 0.001424, 0.001274, 0.002568, 0.000748, 0.000273];
        let lambdas = [0.0124, 0.0305, 0.111, 0.301, 1.14, 3.01];
        PointKineticsParameters::new(
            betas.iter().map(|&b| Ratio::new::<ratio>(b)).collect(),
            lambdas
                .iter()
                .map(|&l| Frequency::new::<hertz>(l))
                .collect(),
            Time::new::<second>(1.0e-4),
        )
        .unwrap()
    }

    #[test]
    fn total_delayed_fraction_sums_groups() {
        let p = u235_six_group();
        let expected = 0.000215 + 0.001424 + 0.001274 + 0.002568 + 0.000748 + 0.000273;
        assert!((p.total_delayed_fraction().get::<ratio>() - expected).abs() < 1e-12);
    }

    /// Null transient: starting at equilibrium with ρ = 0 and no source, the
    /// power must be invariant over many steps (this is the discrete steady
    /// state of the implicit scheme).
    #[test]
    fn null_transient_holds_steady() {
        let p = u235_six_group();
        let p0 = 1.0e6;
        let mut s = PointKineticsState::new_equilibrium(&p, Power::new::<watt>(p0));
        for _ in 0..10_000 {
            s.step(
                &p,
                Time::new::<second>(1.0e-3),
                Ratio::new::<ratio>(0.0),
                Power::new::<watt>(0.0),
            )
            .unwrap();
        }
        let rel = (s.fission_power().get::<watt>() - p0).abs() / p0;
        assert!(rel < 1e-9, "null transient drifted: rel error {rel:e}");
    }

    /// A positive step reactivity must raise the power; a negative one must lower
    /// it (basic monotonic sanity, direction of the prompt response).
    #[test]
    fn reactivity_sign_moves_power_correctly() {
        let p = u235_six_group();
        let p0 = 1.0e6;

        let mut up = PointKineticsState::new_equilibrium(&p, Power::new::<watt>(p0));
        up.step(
            &p,
            Time::new::<second>(1.0e-4),
            Ratio::new::<ratio>(0.001),
            Power::new::<watt>(0.0),
        )
        .unwrap();
        assert!(up.fission_power().get::<watt>() > p0);

        let mut down = PointKineticsState::new_equilibrium(&p, Power::new::<watt>(p0));
        down.step(
            &p,
            Time::new::<second>(1.0e-4),
            Ratio::new::<ratio>(-0.001),
            Power::new::<watt>(0.0),
        )
        .unwrap();
        assert!(down.fission_power().get::<watt>() < p0);
    }

    #[test]
    fn mismatched_group_lengths_error() {
        let err = PointKineticsParameters::new(
            vec![Ratio::new::<ratio>(0.0065)],
            vec![Frequency::new::<hertz>(0.08), Frequency::new::<hertz>(0.1)],
            Time::new::<second>(1.0e-4),
        )
        .unwrap_err();
        assert_eq!(
            err,
            PointKineticsError::MismatchedGroups {
                betas: 1,
                lambdas: 2
            }
        );
    }

    #[test]
    fn non_positive_time_step_error() {
        let p = u235_six_group();
        let mut s = PointKineticsState::new_equilibrium(&p, Power::new::<watt>(1.0e6));
        let err = s
            .step(
                &p,
                Time::new::<second>(0.0),
                Ratio::new::<ratio>(0.0),
                Power::new::<watt>(0.0),
            )
            .unwrap_err();
        assert_eq!(err, PointKineticsError::NonPositiveTimeStep(0.0));
    }
}
