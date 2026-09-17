// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
// `offbeatLib/rheology/constitutiveLaws/creepModels/`
// (`creepModel.C/H`, `powerLaw.C/H`, `LimbackCreepModel.C/H`,
// `MatproCreepModel.C/H`).
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Creep — the slow, permanent flow of a solid under a load it can carry
//! elastically.
//!
//! # What creep is, for a reader who knows continuum mechanics but not fuel
//!
//! Plasticity has a threshold: below the yield stress, nothing permanent
//! happens. Creep has no threshold — it has a *rate*. Hold a bar at any stress
//! for long enough and it keeps deforming, at a rate that depends on stress and
//! (usually exponentially) on temperature. Over the seconds of a laboratory
//! test that is invisible; over the months a fuel rod spends in a reactor it is
//! the dominant deformation mechanism.
//!
//! Three mechanisms matter in a fuel rod, and the correlations here contain all
//! three:
//!
//! - **Thermal (secondary) creep** — diffusion and dislocation climb, going as
//!   `exp(−Q/RT)`. Dominant in the fuel pellet, which runs at 800–1800 K.
//! - **Irradiation creep** — fast neutrons continuously create point defects,
//!   which relieve local stress as they migrate. Its rate depends on the *flux*,
//!   barely on temperature, and it is roughly linear in stress. This is what
//!   lets 600 K cladding — far too cold for thermal creep to do anything —
//!   creep down onto the pellet over a fuel cycle under the coolant pressure.
//! - **Primary (transient) creep** — a fast initial transient after a load
//!   change, which saturates. Only the Zircaloy correlation
//!   ([`CreepModel::Limback`]) models it explicitly.
//!
//! # How the increment is computed
//!
//! Creep is integrated **implicitly**, which is the only way to take a timestep
//! of days. Given the elastic trial state, let `q_trial` be its von Mises
//! stress. A creep increment `Δε_c` relaxes that stress by `3μ Δε_c` (radial
//! return in the deviatoric plane), so the increment must satisfy
//!
//! `Δε_c = Δt · ε̇_c(q_trial − 3μ Δε_c)`
//!
//! which is a scalar nonlinear equation solved here by Newton iteration on
//! `r(Δε_c) = Δε_c − Δt ε̇_c(q)` with `r' = 1 + 3μ Δt · dε̇_c/dq`. Upstream
//! offers exactly this under its `NewtonRaphsonMethod` switch (see
//! `MatproCreepModel.C`); this port always uses it and reports failure as
//! [`OffbeatError::ConstitutiveNotConverged`] rather than returning an
//! unconverged stress.
//!
//! The creep strain *tensor* then follows the Prandtl–Reuss flow rule
//! `Δε_c = (3/2) Δε_c,eq · s_trial / q_trial`, i.e. flow in the direction of the
//! deviatoric stress, volume preserving.
//!
//! # Upstream defect reproduced deliberately: none
//!
//! Upstream wraps its creep update in `do { ... } while (tol > 1);` where `tol`
//! is built as `min(1, |Δ − Δ_prev| / max(Δ_prev, SMALL))`. Being capped at 1,
//! `tol > 1` is never true, so that loop always executes **exactly once** and
//! the creep increment is effectively explicit in the stress. This port does
//! not reproduce that; it iterates to a real tolerance.

use crate::error::{OffbeatError, Result};

use super::state::{RheologyInputs, RheologyState};

/// Universal gas constant \[J/mol/K\] as used by the Zircaloy correlations.
///
/// Upstream `LimbackCreepModel.C` hard-codes `8.314`; the MATPRO fuel
/// correlation hard-codes `8.3143`. Both are kept at their upstream values
/// rather than unified, so the numbers this port produces are traceable to the
/// correlation they came from.
const R_ZIRCALOY: f64 = 8.314;

/// Universal gas constant \[J/mol/K\] as used by the MATPRO fuel correlation.
const R_MATPRO: f64 = 8.3143;

/// Seconds in an hour. The Zircaloy and power-law correlations are written in
/// per-hour units; this port converts once, at the boundary.
const SECONDS_PER_HOUR: f64 = 3600.0;

/// Theoretical density of UO2 \[kg/m³\] used by MATPRO to form the percentage
/// of theoretical density. Upstream `MatproCreepModel.C` hard-codes `10970`.
const UO2_THEORETICAL_DENSITY_MATPRO: f64 = 10_970.0;

/// Maximum local safeguarded-Newton iterations for the creep increment.
///
/// Generous because the fallback is bisection, which halves the bracket every
/// iteration: 100 iterations reduce it by 2⁻¹⁰⁰ even if every Newton step is
/// rejected, so hitting this cap means the residual is not monotone rather than
/// that the iteration is merely slow.
const MAX_CREEP_ITERATIONS: usize = 100;

/// Bracket-collapse tolerance for the creep increment, as a fraction of the
/// full-relaxation increment `q_trial / 3μ`.
///
/// Relative rather than absolute because the increment spans many orders of
/// magnitude over the life of a rod — micro-strain per day early on, and
/// vanishing as the stress relaxes away — and an absolute tolerance that is
/// meaningfully tight at one end stalls the iteration at the other.
const CREEP_BRACKET_TOL: f64 = 1.0e-14;

/// Heat treatment of a zirconium-alloy cladding tube.
///
/// The heat treatment sets the dislocation structure, and thus how fast the
/// tube creeps: a stress-relief-annealed tube retains fabrication dislocations
/// and creeps differently from a fully recrystallised one. Upstream selects the
/// same four with its `cladType` keyword in `LimbackCreepModel.C`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ZircaloyCladType {
    /// Stress-relief annealed Zircaloy-2 or Zircaloy-4. Upstream's default.
    #[default]
    Sra,
    /// Recrystallisation annealed Zircaloy-2 or M5.
    Rxa,
    /// Partially recrystallisation annealed Zircaloy-2.
    Pra,
    /// Stress-relief annealed ZIRLO. Its thermal-creep stress exponent is
    /// itself a function of stress; see [`CreepModel::Limback`].
    Zirlo,
}

impl ZircaloyCladType {
    /// Thermal-creep pre-exponential `A` \[K/MPa/hr\].
    #[must_use]
    pub fn thermal_prefactor(self) -> f64 {
        match self {
            Self::Sra => 1.08e9,
            Self::Rxa => 5.47e8,
            Self::Pra => 7.06e8,
            Self::Zirlo => 8.64e8,
        }
    }

    /// Thermal-creep activation energy `Q` \[J/mol\].
    #[must_use]
    pub fn activation_energy(self) -> f64 {
        match self {
            Self::Sra | Self::Zirlo => 201e3,
            Self::Rxa => 198e3,
            Self::Pra => 199e3,
        }
    }

    /// Irradiation-creep flux coefficient `C0`
    /// \[(n/m²/s)^−0.85 · MPa⁻¹ · hr⁻¹\].
    #[must_use]
    pub fn irradiation_coefficient(self) -> f64 {
        match self {
            Self::Sra => 3.557e-24,
            Self::Rxa => 1.654e-24,
            Self::Pra => 2.714e-24,
            Self::Zirlo => 2.846e-24,
        }
    }

    /// Thermal-creep stress exponent `n` \[-\] at a given von Mises stress
    /// \[Pa\].
    ///
    /// Constant for every alloy except ZIRLO, whose exponent upstream makes
    /// piecewise in stress: 2.0 below 220 MPa, 2.6 up to 400 MPa, and
    /// `1.2667 + 3.333e-3 σ_MPa` above that.
    #[must_use]
    pub fn stress_exponent(self, von_mises_stress: f64) -> f64 {
        match self {
            Self::Sra => 2.0,
            Self::Rxa => 3.5,
            Self::Pra => 2.3,
            Self::Zirlo => {
                let s = von_mises_stress * 1.0e-6;
                if s < 220.0 {
                    2.0
                } else if s < 400.0 {
                    2.6
                } else {
                    1.2667 + 3.333e-3 * s
                }
            }
        }
    }
}

/// The result of integrating a creep law over one timestep in one cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CreepIncrement {
    /// Equivalent creep-strain increment `Δε_c,eq` \[-\], non-negative.
    pub equivalent: f64,
    /// The primary (transient) part of `Δε_c,eq` \[-\], already included in
    /// [`equivalent`](Self::equivalent). Non-zero only for
    /// [`CreepModel::Limback`].
    pub equivalent_primary: f64,
    /// Von Mises stress \[Pa\] after the creep relaxation,
    /// `q_trial − 3μ Δε_c,eq`.
    pub von_mises_stress: f64,
    /// Newton iterations taken.
    pub iterations: usize,
}

impl CreepIncrement {
    /// A zero increment leaving the trial stress untouched.
    #[must_use]
    fn none(von_mises_stress: f64) -> Self {
        Self {
            equivalent: 0.0,
            equivalent_primary: 0.0,
            von_mises_stress,
            iterations: 0,
        }
    }
}

/// A creep correlation.
///
/// Enum dispatch, per the workspace rule. Each variant is one upstream
/// `creepModel` subclass; the physics constants live in the implementation
/// rather than in a dictionary, because they are properties of the correlation
/// and not of the case.
///
/// # Units
///
/// Every variant takes von Mises stress in **pascal** and temperature in
/// **kelvin**, and returns a strain rate in **1/s**, regardless of the units
/// the published correlation was written in. The per-hour and per-MPa
/// conversions are done inside, once, and documented at each site.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CreepModel {
    /// No creep. Corresponds to upstream's base `creepModel`
    /// (`TypeName("fromLatestTime")`), which carries the creep-strain fields
    /// but never adds to them.
    None,

    /// A bare Norton power law `ε̇ = B (σ/σ_C)^n`.
    ///
    /// Upstream `powerLaw`. Not a correlation for any particular material — a
    /// fitting form for a material whose creep data you have, and the standard
    /// verification vehicle because its implicit increment has a closed-form
    /// solution when `n = 1`.
    PowerLaw {
        /// Pre-factor `B` \[**1/hr**\], matching upstream's per-hour timebase.
        b: f64,
        /// Reference stress `σ_C` \[**MPa**\], matching upstream's per-MPa
        /// normalisation.
        sigma_c: f64,
        /// Stress exponent `n` \[-\]. Typically 1 (diffusional) to 5
        /// (dislocation).
        n: f64,
    },

    /// The Limbäck–Andersson creep correlation for zirconium-alloy cladding.
    ///
    /// Upstream `LimbackCreepModel`. Three additive contributions:
    ///
    /// - **Irradiation creep** `ε̇_irr = C0 φ^0.85 σ_MPa` \[1/hr\], with `φ` the
    ///   fast flux in n/m²/s. Athermal and near-linear in stress.
    /// - **Secondary thermal creep**
    ///   `ε̇_th = A (E/T) sinh(a_i σ_MPa / E)^n exp(−Q/RT)` \[1/hr\], with
    ///   `E = 1.148e5 − 59.9 T` \[MPa\] a temperature-dependent modulus and
    ///   `a_i = a [1 − A1(1 − exp(−A2 Φ^A3))]` a fluence-dependent stress
    ///   multiplier that saturates as irradiation hardening sets in.
    /// - **Primary creep**, a saturating transient whose saturation value
    ///   depends on the secondary rate.
    ///
    /// # Fluence unit
    ///
    /// The fluence multiplier `a_i` is fitted with `Φ` in **n/cm²** (upstream
    /// documents `A2` as `[n/cm2]^-A3`), so this port converts the SI fluence
    /// on [`MaterialState`](crate::materials::MaterialState) by 1e-4. The flux
    /// in the irradiation term, by contrast, is used in SI n/m²/s — upstream's
    /// `phi*1e4` is a cm→m conversion of its own n/cm²/s field, not part of the
    /// correlation.
    ///
    /// # Validity
    ///
    /// Fitted for LWR cladding at roughly 550–700 K. Above about 1100 K the
    /// α→β phase transformation invalidates it entirely and upstream switches
    /// to `LimbackCreepModelLOCA`, which this port does not yet contain. The
    /// only hard error raised here is at `T ≥ 1916 K`, where the fitted modulus
    /// `E` turns negative and the expression becomes meaningless.
    Limback {
        /// Heat treatment of the tube, which selects the constants.
        clad_type: ZircaloyCladType,
    },

    /// The MATPRO creep correlation for UO2 and MOX fuel.
    ///
    /// Upstream `MatproCreepModel`. Three additive contributions:
    ///
    /// - a **diffusional** term, linear in stress and inversely proportional to
    ///   the square of the grain diameter,
    /// - a **dislocation** term, going as `σ^4.5`, which takes over at high
    ///   stress,
    /// - an **irradiation-enhanced** term, linear in stress and in fission
    ///   rate, athermal under the Sakai correction.
    ///
    /// The activation energies of the first two depend on the oxygen-to-metal
    /// ratio through a sigmoid `f`, because oxygen interstitials in
    /// hyperstoichiometric fuel accelerate cation diffusion dramatically.
    ///
    /// # Validity
    ///
    /// The density enters as `(D% − 87.7)` and `(D% − 90.5)` in denominators,
    /// so the correlation is meaningless at or below 90.5 % of theoretical
    /// density — it returns a *negative* creep rate there. Upstream does not
    /// guard this; this port returns [`OffbeatError::OutOfRange`].
    ///
    /// # Upstream defect deliberately **not** reproduced
    ///
    /// Upstream clamps the effective stress to `max(min(q, 1e10), 1e5)`. The
    /// **lower** clamp means a completely unstressed cell is treated as
    /// carrying 0.1 MPa and creeps forever, which is unphysical and would break
    /// any stress-free verification case. This port keeps the upper clamp as a
    /// numerical guard and drops the lower one.
    Matpro {
        /// Apply the Sakai correction to the irradiation-enhanced term.
        ///
        /// With the correction (upstream's default) the term is athermal,
        /// `ε̇3 = A7 F σ` with `A7 = 7.78e-37`; without it, the original MATPRO
        /// form `ε̇3 = A7 F σ exp(−Q3/RT)` with `A7 = 3.7226e-35` is used.
        sakai_correction: bool,
    },
}

impl CreepModel {
    /// Solve the implicit creep increment for one cell over one timestep.
    ///
    /// `cell` is used only to name the cell in a non-convergence error.
    /// `q_trial` is the von Mises stress \[Pa\] of the elastic trial state, and
    /// `shear_modulus` is `μ` \[Pa\].
    ///
    /// Returns a zero increment when `dt == 0`, when the model is
    /// [`None`](Self::None), or when the trial stress is not positive.
    ///
    /// # Algorithm — safeguarded (bracketed) Newton
    ///
    /// The residual `r(Δε_c) = Δε_c − Δt ε̇_c(q_trial − 3μ Δε_c)` is
    /// **non-decreasing** in `Δε_c`, because every rate law here is
    /// non-decreasing in stress, with `r(0) ≤ 0` and
    /// `r(q_trial/3μ) = q_trial/3μ ≥ 0` (a fully relaxed material has zero
    /// deviatoric stress and therefore zero creep rate). The root is thus
    /// bracketed from the outset, and a Newton step that leaves the bracket is
    /// replaced by a bisection.
    ///
    /// That safeguard is not decoration. The Jacobian omits the stress
    /// dependence of [`Limback`](Self::Limback)'s primary-creep transient
    /// (upstream leaves that derivative commented out), and the primary term
    /// can be an order of magnitude larger than the secondary one at the start
    /// of a hold. Plain Newton then degenerates into a fixed-point iteration
    /// whose contraction factor exceeds one and oscillates without ever
    /// converging; bisection makes the bracket collapse regardless.
    ///
    /// # Errors
    ///
    /// - [`OffbeatError::ConstitutiveNotConverged`] if the bracket has not
    ///   collapsed within 100 iterations, which given the bisection fallback
    ///   means the rate law is not monotone in stress.
    /// - [`OffbeatError::OutOfRange`] / [`OffbeatError::Unphysical`] if the
    ///   correlation is evaluated outside the range where it is defined (see
    ///   each variant).
    pub fn increment(
        &self,
        cell: usize,
        q_trial: f64,
        shear_modulus: f64,
        inputs: &RheologyInputs,
        state: &RheologyState,
    ) -> Result<CreepIncrement> {
        if matches!(self, Self::None) || inputs.dt <= 0.0 || !(q_trial > 0.0) {
            return Ok(CreepIncrement::none(q_trial));
        }
        if !(shear_modulus > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "shear modulus",
                value: shear_modulus,
                unit: "Pa",
                reason: "must be strictly positive for a creep return mapping",
            });
        }

        let dt = inputs.dt;
        // Upper bound: the increment that takes the deviatoric stress to zero.
        // Creep cannot overshoot it, because at zero stress the rate is zero.
        let full_relaxation = q_trial / (3.0 * shear_modulus);
        let bracket_tol = CREEP_BRACKET_TOL * full_relaxation;

        let mut lo = 0.0_f64;
        let mut hi = full_relaxation;
        let mut increment = 0.0_f64;
        let mut residual = 0.0_f64;

        for iteration in 1..=MAX_CREEP_ITERATIONS {
            let q = (q_trial - 3.0 * shear_modulus * increment).max(0.0);
            let (rate, drate_dq) = self.rate_and_derivative(q, inputs, state)?;
            residual = increment - dt * rate;
            if !residual.is_finite() {
                return Err(OffbeatError::ConstitutiveNotConverged {
                    cell,
                    residual: f64::INFINITY,
                    iterations: iteration,
                });
            }

            // Tighten the bracket from the sign of the residual.
            if residual > 0.0 {
                hi = increment;
            } else {
                lo = increment;
            }

            if residual == 0.0 || hi - lo <= bracket_tol {
                let equivalent = increment.max(0.0);
                return Ok(CreepIncrement {
                    equivalent,
                    equivalent_primary: self.primary_part(q, inputs, state).min(equivalent),
                    von_mises_stress: (q_trial - 3.0 * shear_modulus * equivalent).max(0.0),
                    iterations: iteration,
                });
            }

            let jacobian = 1.0 + 3.0 * shear_modulus * dt * drate_dq;
            let newton = if jacobian.is_finite() && jacobian > 0.0 {
                increment - residual / jacobian
            } else {
                f64::NAN
            };
            increment = if newton.is_finite() && newton > lo && newton < hi {
                newton
            } else {
                0.5 * (lo + hi)
            };
        }

        Err(OffbeatError::ConstitutiveNotConverged {
            cell,
            residual: residual.abs(),
            iterations: MAX_CREEP_ITERATIONS,
        })
    }

    /// Equivalent creep-strain rate \[1/s\] at a von Mises stress `q` \[Pa\],
    /// together with its derivative `dε̇/dq` \[1/(s·Pa)\].
    ///
    /// The derivative is what makes the Newton iteration in
    /// [`increment`](Self::increment) converge quadratically. Where it is only
    /// approximate — the primary-creep term of [`Limback`](Self::Limback) — the
    /// iteration still converges, just linearly, because the *residual* remains
    /// exact.
    ///
    /// # Errors
    ///
    /// See [`increment`](Self::increment).
    pub fn rate_and_derivative(
        &self,
        q: f64,
        inputs: &RheologyInputs,
        state: &RheologyState,
    ) -> Result<(f64, f64)> {
        match self {
            Self::None => Ok((0.0, 0.0)),
            Self::PowerLaw { b, sigma_c, n } => power_law_rate(q, *b, *sigma_c, *n),
            Self::Limback { clad_type } => limback_rate(q, *clad_type, inputs, state),
            Self::Matpro { sakai_correction } => matpro_rate(q, *sakai_correction, inputs),
        }
    }

    /// The primary-creep part of the equivalent increment \[-\] at a converged
    /// stress `q` \[Pa\]. Zero for every model but [`Limback`](Self::Limback).
    fn primary_part(&self, q: f64, inputs: &RheologyInputs, state: &RheologyState) -> f64 {
        match self {
            Self::Limback { clad_type } => {
                match limback_secondary_rate_per_hour(q, *clad_type, inputs) {
                    Ok((secondary, _)) => limback_primary_increment(
                        secondary,
                        state.equivalent_primary_creep_strain,
                        inputs.dt / SECONDS_PER_HOUR,
                    ),
                    Err(_) => 0.0,
                }
            }
            _ => 0.0,
        }
    }
}

/// Norton power law, upstream `powerLaw::correctCreep`.
///
/// `ε̇ = B (σ_MPa / σ_C)^n` in 1/hr, converted to 1/s on return.
fn power_law_rate(q: f64, b: f64, sigma_c: f64, n: f64) -> Result<(f64, f64)> {
    if !(sigma_c > 0.0) {
        return Err(OffbeatError::Unphysical {
            quantity: "power-law reference stress",
            value: sigma_c,
            unit: "MPa",
            reason: "must be strictly positive",
        });
    }
    if q <= 0.0 {
        // At zero stress the rate is zero. The derivative is finite only for
        // n <= 1; for n > 1 it is zero, which is also the correct limit.
        let derivative = if n <= 1.0 && n > 0.0 {
            b / (sigma_c * 1.0e6) / SECONDS_PER_HOUR
        } else {
            0.0
        };
        return Ok((0.0, if n == 1.0 { derivative } else { 0.0 }));
    }
    let x = q * 1.0e-6 / sigma_c;
    let rate_per_hour = b * x.powf(n);
    let drate_dq_per_hour = b * n * x.powf(n - 1.0) * 1.0e-6 / sigma_c;
    Ok((
        rate_per_hour / SECONDS_PER_HOUR,
        drate_dq_per_hour / SECONDS_PER_HOUR,
    ))
}

/// Secondary (irradiation + thermal) Zircaloy creep rate \[1/hr\] and its
/// derivative \[1/(hr·Pa)\].
///
/// Split out from [`limback_rate`] because the primary-creep transient is a
/// function of the *secondary* rate, so both callers need it.
fn limback_secondary_rate_per_hour(
    q: f64,
    clad_type: ZircaloyCladType,
    inputs: &RheologyInputs,
) -> Result<(f64, f64)> {
    let t = inputs.material.temperature;
    if !(t > 0.0) {
        return Err(OffbeatError::Unphysical {
            quantity: "temperature",
            value: t,
            unit: "K",
            reason: "absolute temperature must be strictly positive",
        });
    }

    // Upstream caps the effective stress at 3e10 Pa before applying the
    // correlation; kept as a numerical guard against a `sinh` overflow during
    // an early, badly scaled iterate.
    let q = q.clamp(0.0, 3.0e10);
    let q_mpa = q * 1.0e-6;

    // ---- Irradiation creep, upstream constants C0/C1/C2. ----
    let c0 = clad_type.irradiation_coefficient();
    const C1: f64 = 0.85;
    const C2: f64 = 1.0;
    let flux = inputs.irradiation.fast_flux.max(0.0);
    let flux_term = if flux > 0.0 { flux.powf(C1) } else { 0.0 };
    let (irr, d_irr) = if q_mpa > 0.0 {
        (
            c0 * flux_term * q_mpa.powf(C2),
            c0 * flux_term * C2 * q_mpa.powf(C2 - 1.0) * 1.0e-6,
        )
    } else {
        (0.0, c0 * flux_term * 1.0e-6)
    };

    // ---- Secondary thermal creep. ----
    // Temperature-dependent modulus [MPa]; the fit turns negative at ~1916 K,
    // far above where the correlation is meaningful anyway.
    let e_mod = 1.148e5 - 59.9 * t;
    if !(e_mod > 0.0) {
        return Err(OffbeatError::OutOfRange {
            quantity: "Limback Zircaloy creep modulus E = 1.148e5 - 59.9 T",
            value: t,
            low: 0.0,
            high: 1916.0,
            unit: "K",
        });
    }

    // Fluence-dependent stress multiplier. `A2` is fitted with the fluence in
    // n/cm², so convert from the SI n/m² carried by `MaterialState`.
    const A_STRESS: f64 = 650.0;
    const A1: f64 = 0.56;
    const A2: f64 = 1.4e-27;
    const A3: f64 = 1.3;
    let fluence_ncm2 = (inputs.material.fast_fluence.max(0.0)) * 1.0e-4;
    let saturation = if fluence_ncm2 > 0.0 {
        1.0 - (-A2 * fluence_ncm2.powf(A3)).exp()
    } else {
        0.0
    };
    let a_i = A_STRESS * (1.0 - A1 * saturation);

    let a = clad_type.thermal_prefactor();
    let qa = clad_type.activation_energy();
    let n = clad_type.stress_exponent(q);

    let x = a_i * q_mpa / e_mod;
    let arrhenius = a * e_mod / t * (-qa / (R_ZIRCALOY * t)).exp();
    let sinh_x = x.sinh();
    let (th, d_th) = if sinh_x > 0.0 {
        let th = arrhenius * sinh_x.powf(n);
        // d/dq [sinh(x)^n] = n sinh(x)^(n-1) cosh(x) · dx/dq, dx/dq = a_i/E·1e-6.
        let d_th = arrhenius * n * sinh_x.powf(n - 1.0) * x.cosh() * (a_i / e_mod) * 1.0e-6;
        (th, if d_th.is_finite() { d_th } else { 0.0 })
    } else {
        (0.0, 0.0)
    };

    let rate = irr + th;
    let derivative = d_irr + d_th;
    if !(rate.is_finite() && derivative.is_finite()) {
        return Err(OffbeatError::OutOfRange {
            quantity: "Limback Zircaloy creep rate",
            value: q,
            low: 0.0,
            high: 3.0e10,
            unit: "Pa",
        });
    }
    Ok((rate.max(0.0), derivative.max(0.0)))
}

/// Saturating primary-creep increment \[-\] over `dt_hours`, given the
/// secondary rate \[1/hr\] and the primary strain already accumulated.
///
/// Upstream `LimbackCreepModel.C`. The transient follows
/// `ε_p(t) = ε_p,sat [1 − exp(−C sqrt(ε̇_sec t))]`, and `τ` is the pseudo-time
/// at which that curve already sits at the accumulated value — the standard
/// strain-hardening (rather than time-hardening) way to restart a saturating
/// transient after a load change.
fn limback_primary_increment(
    secondary_rate_per_hour: f64,
    accumulated_primary: f64,
    dt_hours: f64,
) -> f64 {
    const B: f64 = 0.0216;
    const B_EXP: f64 = 0.109;
    const D: f64 = 35_500.0;
    const D_EXP: f64 = -2.05;
    const C: f64 = 52.0;

    if !(secondary_rate_per_hour > 0.0) || !(dt_hours > 0.0) {
        return 0.0;
    }
    let saturated = B
        * secondary_rate_per_hour.powf(B_EXP)
        * (2.0 - (D * secondary_rate_per_hour).tanh()).powf(D_EXP);
    if !(saturated > 0.0) || accumulated_primary >= saturated {
        return 0.0;
    }
    let ratio = 1.0 - accumulated_primary / saturated;
    if !(ratio > 0.0) {
        return 0.0;
    }
    let tau = ratio.ln().powi(2) / (C * C * secondary_rate_per_hour);
    let increment = saturated
        * (1.0 - (-C * (secondary_rate_per_hour * (tau + dt_hours)).sqrt()).exp())
        - accumulated_primary;
    if increment.is_finite() {
        increment.max(0.0)
    } else {
        0.0
    }
}

/// Total Limbäck creep rate \[1/s\] and its stress derivative \[1/(s·Pa)\].
///
/// The primary transient enters as an increment spread over the timestep,
/// exactly as upstream forms `fTrial = ε̇_sec + Δε_prim/Δt`. Its contribution to
/// the Jacobian is omitted — upstream leaves the corresponding derivative
/// commented out in `LimbackCreepModel.C` — so the Newton iteration converges
/// linearly rather than quadratically while still driving the true residual to
/// tolerance.
fn limback_rate(
    q: f64,
    clad_type: ZircaloyCladType,
    inputs: &RheologyInputs,
    state: &RheologyState,
) -> Result<(f64, f64)> {
    let (secondary, d_secondary) = limback_secondary_rate_per_hour(q, clad_type, inputs)?;
    let dt_hours = inputs.dt / SECONDS_PER_HOUR;
    let primary =
        limback_primary_increment(secondary, state.equivalent_primary_creep_strain, dt_hours);
    let rate_per_hour = if dt_hours > 0.0 {
        secondary + primary / dt_hours
    } else {
        secondary
    };
    Ok((
        rate_per_hour / SECONDS_PER_HOUR,
        d_secondary / SECONDS_PER_HOUR,
    ))
}

/// MATPRO UO2/MOX creep rate \[1/s\] and its stress derivative \[1/(s·Pa)\].
///
/// Upstream `MatproCreepModel::correctCreep`. All three terms are written here
/// as `coefficient × stress-power`, so the derivative is exact and free of the
/// division-by-stress that would blow up at zero stress.
fn matpro_rate(q: f64, sakai_correction: bool, inputs: &RheologyInputs) -> Result<(f64, f64)> {
    const A1: f64 = 0.3919;
    const A2: f64 = 1.3100e-19;
    const A3: f64 = -87.7;
    const A4: f64 = 2.0391e-25;
    const A6: f64 = -90.5;
    const A7_SAKAI: f64 = 7.78e-37;
    const A7_PLAIN: f64 = 3.7226e-35;
    const Q3: f64 = 21_759.0;

    let t = inputs.material.temperature;
    if !(t > 0.0) {
        return Err(OffbeatError::Unphysical {
            quantity: "temperature",
            value: t,
            unit: "K",
            reason: "absolute temperature must be strictly positive",
        });
    }

    // Upper clamp only; see the note on `CreepModel::Matpro` for why upstream's
    // 1e5 Pa lower clamp is deliberately not reproduced.
    let q = q.clamp(0.0, 1.0e10);

    // Grain diameter in micrometre. Diffusional creep goes as 1/G².
    let g = inputs.irradiation.grain_radius * 2.0 * 1.0e6;
    if !(g > 0.0) {
        return Err(OffbeatError::Unphysical {
            quantity: "fuel grain radius",
            value: inputs.irradiation.grain_radius,
            unit: "m",
            reason: "MATPRO diffusional creep goes as 1/(grain diameter)², so a \
                     zero or negative grain size is not evaluable",
        });
    }

    // Percentage of theoretical density. Upstream forms this as
    // `rho/10970*100`; here it comes from the porosity carried on
    // `MaterialState`, which is the same quantity with the same UO2 reference
    // density (kept in `UO2_THEORETICAL_DENSITY_MATPRO` for traceability).
    let _ = UO2_THEORETICAL_DENSITY_MATPRO;
    let d = inputs.material.density_fraction() * 100.0;
    if !(d > 90.5) {
        return Err(OffbeatError::OutOfRange {
            quantity: "MATPRO fuel creep density",
            value: d,
            low: 90.5,
            high: 100.0,
            unit: "% of theoretical density",
        });
    }

    // Oxygen-to-metal ratio. `oxygen_deviation` is the x in (U,Pu)O_{2+x}.
    let om = 2.0 + inputs.material.oxygen_deviation;
    // NOTE — upstream limitation reproduced: for hypostoichiometric fuel
    // (O/M < 2) `max(OM - 2, 1e-15)` collapses to the stoichiometric limit, so
    // the correlation cannot distinguish oxygen-deficient fuel from
    // stoichiometric fuel. Fast-reactor MOX is normally hypostoichiometric, so
    // this is a real restriction rather than a corner case.
    let f = 1.0 / ((-20.0 / (om - 2.0).max(1.0e-15).ln() - 8.0).exp() + 1.0);

    let q1 = 74_829.0 * f + 301_762.0;
    let q2 = 83_143.0 * f + 469_191.0;

    let fission_rate = inputs.irradiation.fission_rate.max(0.0);

    // Term 1 — diffusional, linear in stress.
    let c1 = (A1 + A2 * fission_rate) / ((A3 + d) * g * g) * (-q1 / (R_MATPRO * t)).exp();
    // Term 2 — dislocation, σ^4.5.
    let c2 = A4 / (A6 + d) * (-q2 / (R_MATPRO * t)).exp();
    // Term 3 — irradiation enhanced, linear in stress.
    let c3 = if sakai_correction {
        A7_SAKAI * fission_rate
    } else {
        A7_PLAIN * fission_rate * (-Q3 / (R_MATPRO * t)).exp()
    };

    let rate = c1 * q + c2 * q.powf(4.5) + c3 * q;
    let derivative = c1 + 4.5 * c2 * q.powf(3.5) + c3;

    if !(rate.is_finite() && derivative.is_finite()) {
        return Err(OffbeatError::OutOfRange {
            quantity: "MATPRO fuel creep rate",
            value: q,
            low: 0.0,
            high: 1.0e10,
            unit: "Pa",
        });
    }
    Ok((rate.max(0.0), derivative.max(0.0)))
}

/// Timestep control driven by how fast the material is creeping.
///
/// # Why a fuel code needs this
///
/// The creep integration above is implicit in the *stress*, but the material
/// state — accumulated strain, hardening, fluence — is explicit. Take a
/// timestep in which the cladding creeps by several per cent and the answer is
/// wrong no matter how well the local Newton converged. Upstream limits the
/// step from the previous step's increments in
/// `misesPlasticCreep::nextDeltaT`; this is the same rule.
///
/// # Units
///
/// Both limits are dimensionless strain increments; the returned timestep is in
/// seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CreepTimeStepControl {
    /// Largest volume-averaged equivalent inelastic increment \[-\] allowed in
    /// one step. Upstream's `maxAverageCreep`.
    pub max_average_increment: f64,
    /// Largest single-cell equivalent inelastic increment \[-\] allowed in one
    /// step. Upstream's `maxMaximumCreep`.
    pub max_maximum_increment: f64,
}

impl Default for CreepTimeStepControl {
    /// No limit — matching upstream, which defaults both limits to `GREAT`
    /// unless `adjustableTimeStep` is switched on.
    fn default() -> Self {
        Self {
            max_average_increment: f64::INFINITY,
            max_maximum_increment: f64::INFINITY,
        }
    }
}

impl CreepTimeStepControl {
    /// Timestep \[s\] to take next, from the increments the last step produced.
    ///
    /// `average_increment` and `max_increment` are the volume-averaged and
    /// largest equivalent inelastic (creep + plastic) increments \[-\] over the
    /// step of length `previous_dt` \[s\]. Returns [`f64::INFINITY`] when
    /// neither limit binds, so a caller can `min` this against every other
    /// physics module's suggestion.
    ///
    /// ```
    /// use outram_park_fork_offbeat::rheology::CreepTimeStepControl;
    ///
    /// let control = CreepTimeStepControl {
    ///     max_average_increment: 1.0e-4,
    ///     max_maximum_increment: 1.0e-3,
    /// };
    /// // Last step: 1e-4 average increment over 3600 s, so the average rate is
    /// // 2.8e-8 /s and the average limit allows exactly one more hour.
    /// let dt = control.next_time_step(1.0e-4, 2.0e-4, 3600.0);
    /// assert!((dt - 3600.0).abs() < 1.0e-6);
    /// ```
    #[must_use]
    pub fn next_time_step(
        &self,
        average_increment: f64,
        max_increment: f64,
        previous_dt: f64,
    ) -> f64 {
        if !(previous_dt > 0.0) {
            return f64::INFINITY;
        }
        let average_rate = (average_increment / previous_dt).max(f64::MIN_POSITIVE);
        let max_rate = (max_increment / previous_dt).max(f64::MIN_POSITIVE);
        (self.max_average_increment / average_rate).min(self.max_maximum_increment / max_rate)
    }
}
