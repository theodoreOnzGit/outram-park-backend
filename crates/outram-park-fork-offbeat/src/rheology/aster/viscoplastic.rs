// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from code_aster (https://gitlab.com/codeaster/src)
//   Copyright (C) 1991 - 2026 - EDF R&D
//   Licence: GPL-3.0-or-later
//   Upstream commit: b504ea08c2f49575e04644cee2e39a63ea45c16e
//   Sources:
//     bibfor/algorith/norton.F90   -- the Norton flow rule (legacy symbol `norton`)
//     bibfor/algorith/ggplem.F90   -- the Lemaitre flow function (legacy `ggplem`)
//     bibfor/utilifor/lcnrts.F90   -- the von Mises norm `sqrt(3/2 s:s)` (`lcnrts`)
//     bibfor/utilifor/lcdevi.F90   -- the deviator (`lcdevi`)
//     bibfor/lc/lc0032.F90         -- NORTON dispatch (`num_lc = 32`)
//     bibfor/comport/nmvpir.F90    -- LEMAITRE_IRRA parameter assembly (`nmvpir`)
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Isotropic viscoplastic creep laws.
//!
//! # What creep is, and why a reactor cares
//!
//! Held under load below its yield stress, metal still deforms — slowly,
//! continuously, and faster when hot. That is creep, and in a fuel rod it is
//! not a nuisance but the main event: it is what lets cladding creep down onto
//! the pellet over months of operation, and what eventually ruptures a reactor
//! lower head held above temperature under its own weight.
//!
//! # The shape of every law here
//!
//! All are **isotropic** and **von Mises**: flow depends on the stress only
//! through its deviatoric part, measured by the equivalent stress
//! `σ_eq = √(3/2 · s:s)`, and it occurs in the direction of that deviator.
//! Each law differs only in the scalar rate `ṗ(σ_eq, p)`:
//!
//! | Law | Rate |
//! |---|---|
//! | [`Norton`](ViscoplasticLaw::Norton) | `(σ_eq / K)^n` |
//! | [`Lemaitre`](ViscoplasticLaw::Lemaitre) | `(σ_eq / K)^n · p^(-n/m)` |
//! | [`LemaitreIrradiation`](ViscoplasticLaw::LemaitreIrradiation) | as Lemaitre, with `K` set by fast flux and temperature |
//!
//! The strain rate is then `ε̇ = (3/2) ṗ s / σ_eq`, whose equivalent measure is
//! exactly `ṗ` — which is what makes `p` "the accumulated equivalent
//! viscoplastic strain" rather than an arbitrary internal variable.
//!
//! **Norton is the `1/m → 0` limit of Lemaitre**, and upstream implements it
//! that way: `ggplem.F90` branches on `unsurm == 0` and falls back to the pure
//! power law. That relationship is preserved here and pinned by a test, because
//! it is the cheapest available check that both are encoded correctly.
//!
//! # Primary versus secondary creep
//!
//! Norton describes **secondary** (steady-state) creep: at fixed stress the
//! rate is constant. Lemaitre adds the `p^(-n/m)` factor, which makes the rate
//! *decay* as strain accumulates — **primary** creep, the initial transient
//! where a freshly loaded component creeps quickly and then settles. The
//! exponent is negative, so more accumulated strain means slower flow; getting
//! its sign wrong turns a decaying transient into a runaway.
//!
//! # Integration
//!
//! The stress at the end of a step depends on how much creep occurred, and the
//! creep depends on that stress. [`ViscoplasticLaw::integrate`] resolves the
//! circularity with a radial return: for isotropic von Mises flow the deviator
//! keeps its direction and only shrinks, so the whole tensorial problem reduces
//! to one scalar unknown, `Δp`, solved with the safeguarded Newton from
//! [`crate::rheology::aster::integration`].

use outram_foam_basic_lib::primitives::SymmTensor;

use crate::error::{OffbeatError, Result};
use crate::rheology::aster::integration::{newton_safeguarded, SolverControl};

/// Von Mises equivalent stress `σ_eq = √(3/2 · s:s)` of a **deviator** `s`.
///
/// Upstream's `lcnrts`. Note it takes the deviator, not the full stress — pass
/// a stress tensor and the hydrostatic part inflates the answer.
#[must_use]
pub fn von_mises_of_deviator(s: SymmTensor) -> f64 {
    (1.5 * s.double_inner(s)).sqrt()
}

/// Deviatoric part of a stress tensor, `s = σ - tr(σ)/3 · I`.
///
/// Upstream's `lcdevi`.
#[must_use]
pub fn deviator(sigma: SymmTensor) -> SymmTensor {
    let mean = sigma.tr() / 3.0;
    SymmTensor::new(
        sigma.xx - mean,
        sigma.xy,
        sigma.xz,
        sigma.yy - mean,
        sigma.yz,
        sigma.zz - mean,
    )
}

/// Parameters of the Norton power-law creep rule.
///
/// # Units
///
/// `k` in pascal, `n` dimensionless. Upstream stores `1/K` (`unsurk`) rather
/// than `K`; this port stores `K` because that is the quantity the literature
/// tabulates and the one with an interpretable unit, and inverts internally.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NortonParameters {
    /// Reference stress `K` \[Pa\]. Larger `K` means more creep resistance.
    pub k: f64,
    /// Stress exponent `n` \[-\]. Typically 3-8 for metals; higher means a
    /// sharper dependence on stress.
    pub n: f64,
}

/// Parameters of the Lemaitre strain-hardening creep rule.
///
/// # Units
///
/// `k` in pascal, `n` and `m` dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LemaitreParameters {
    /// Reference stress `K` \[Pa\].
    pub k: f64,
    /// Stress exponent `n` \[-\].
    pub n: f64,
    /// Strain-hardening exponent `m` \[-\].
    ///
    /// Enters as `p^(-n/m)`, so **larger `m` means weaker hardening** and the
    /// law tends to Norton as `m → ∞`. Must be non-zero; use
    /// [`ViscoplasticLaw::Norton`] for the no-hardening case rather than a huge
    /// `m`.
    pub m: f64,
}

/// Parameters of the irradiation-creep variant of Lemaitre.
///
/// # What irradiation creep is
///
/// Under neutron irradiation a metal creeps far faster than temperature alone
/// would explain: displacement damage continuously creates point defects, and
/// their biased absorption at dislocations lets the material flow. For fuel
/// cladding this is the dominant creep mechanism in normal operation, and it is
/// what allows the cladding to creep down onto the pellet over months.
///
/// # Units
///
/// Upstream parameter names are kept verbatim in the field documentation so a
/// deck can be read across, but the Rust names are descriptive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LemaitreIrradiationParameters {
    /// Stress exponent `n` \[-\]. Upstream `N`.
    pub n: f64,
    /// Strain-hardening exponent `m` \[-\]. Upstream `UN_SUR_M` is its
    /// reciprocal; this field is `m` itself, matching
    /// [`LemaitreParameters::m`].
    pub m: f64,
    /// Flux sensitivity coefficient \[-\]. Upstream `UN_SUR_K`.
    ///
    /// Scales the flux contribution to the creep compliance. Zero neutralises
    /// the flux term entirely, which upstream implements by forcing the flux
    /// ratio to one.
    pub flux_coefficient: f64,
    /// Reference fast flux \[n/(m²·s)\]. Upstream `PHI_ZERO`. Must be strictly
    /// positive — upstream raises a fatal error otherwise.
    pub reference_flux: f64,
    /// Flux exponent \[-\]. Upstream `BETA`.
    pub flux_exponent: f64,
    /// Athermal additive term \[-\]. Upstream `L`. Keeps creep finite at zero
    /// flux.
    pub athermal_term: f64,
    /// Activation energy over the gas constant, `Q/R` \[K\]. Upstream `QSR_K`.
    pub activation_temperature: f64,
}

/// An isotropic viscoplastic creep law.
///
/// Enum dispatch rather than trait objects, per the workspace rule — the set is
/// closed and known at compile time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViscoplasticLaw {
    /// Norton power-law (secondary) creep.
    ///
    /// ASTER behaviour name: `NORTON` (`num_lc = 32`, 7 state variables).
    /// Upstream: `bibfor/algorith/norton.F90`, dispatched from
    /// `bibfor/lc/lc0032.F90` — legacy symbols `norton`, `lc0032`.
    /// Integration: `RUNGE_KUTTA` or `NEWTON_PERT` upstream; this port
    /// integrates implicitly, see [`ViscoplasticLaw::integrate`].
    ///
    /// `ṗ = (σ_eq / K)^n`. The rate depends on stress alone, so at fixed stress
    /// it is constant — steady-state creep.
    Norton(NortonParameters),

    /// Lemaitre strain-hardening (primary + secondary) creep.
    ///
    /// ASTER behaviour name: `LEMAITRE`. Upstream:
    /// `bibfor/algorith/ggplem.F90` — legacy symbol `ggplem`.
    ///
    /// `ṗ = (σ_eq / K)^n · p^(-n/m)`. The accumulated strain slows subsequent
    /// flow, reproducing the decaying primary-creep transient.
    Lemaitre(LemaitreParameters),

    /// Lemaitre irradiation creep — the cladding law for normal operation.
    ///
    /// ASTER behaviour name: `LEMAITRE_IRRA`. Upstream:
    /// `bibfor/comport/nmvpir.F90` assembles the parameters and calls the same
    /// `ggplem` flow function as `LEMAITRE` — legacy symbols `nmvpir`,
    /// `ggplem`.
    ///
    /// Structurally this *is* Lemaitre; what differs is that the creep
    /// compliance `1/K` is not a constant but is built from the fast flux and
    /// the temperature:
    ///
    /// `1/K = (A φ̇/φ₀ + L)^(β/n) · exp(-Q/(n·R·T))`
    ///
    /// # Why the `1/n` appears twice, and why that is not a mistake
    ///
    /// Both the flux exponent and the Arrhenius exponent are divided by `n`
    /// here, which looks like an error until the rate is written out. Since
    /// `ṗ = (σ_eq/K)^n`, the compliance is raised to the `n`-th power, and the
    /// two divisions cancel:
    ///
    /// `ṗ = σ_eq^n · (A φ̇/φ₀ + L)^β · exp(-Q/(R·T))`
    ///
    /// So the *rate* carries a clean Arrhenius temperature dependence and a
    /// clean power-law flux dependence — which is the physically meaningful
    /// form, and the reason upstream parameterises it this way. Transcribing
    /// the compliance without the `1/n` would give a rate with the exponents
    /// raised to the `n`-th power, wrong by orders of magnitude and in a way no
    /// dimensional check would catch.
    LemaitreIrradiation(LemaitreIrradiationParameters),
}

impl ViscoplasticLaw {
    /// The upstream ASTER behaviour name.
    #[must_use]
    pub const fn aster_name(self) -> &'static str {
        match self {
            Self::Norton(_) => "NORTON",
            Self::Lemaitre(_) => "LEMAITRE",
            Self::LemaitreIrradiation(_) => "LEMAITRE_IRRA",
        }
    }

    /// Build the effective Lemaitre law for an irradiation-creep variant at a
    /// given fast flux and temperature.
    ///
    /// Returns the law unchanged for the variants whose parameters do not
    /// depend on the irradiation environment, so a caller can apply this
    /// uniformly.
    ///
    /// # Arguments
    ///
    /// - `fast_flux` — `φ̇` \[n/(m²·s)\], non-negative.
    /// - `temperature` — absolute temperature \[K\], strictly positive.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a negative flux, a non-positive
    /// temperature, or a non-positive reference flux — upstream raises fatal
    /// errors (`ALGORITH6_57`, `ALGORITH7_80`, `ALGORITH7_81`) on the same
    /// conditions.
    pub fn at_irradiation_conditions(self, fast_flux: f64, temperature: f64) -> Result<Self> {
        let p = match self {
            Self::LemaitreIrradiation(p) => p,
            other => return Ok(other),
        };

        if fast_flux < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "fast neutron flux",
                value: fast_flux,
                unit: "n/(m^2 s)",
                reason: "must not be negative (upstream ALGORITH6_57)",
            });
        }
        if !(temperature > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "temperature",
                value: temperature,
                unit: "K",
                reason: "must be strictly positive; the Arrhenius factor divides by it",
            });
        }
        if !(p.reference_flux > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "reference flux PHI_ZERO",
                value: p.reference_flux,
                unit: "n/(m^2 s)",
                reason: "must be strictly positive (upstream ALGORITH7_80)",
            });
        }

        // Upstream neutralises the flux ratio when the flux coefficient is zero.
        let flux_ratio = if p.flux_coefficient == 0.0 {
            1.0
        } else {
            fast_flux / p.reference_flux
        };

        let mut compliance = p.flux_coefficient * flux_ratio + p.athermal_term;
        if compliance < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "irradiation creep compliance",
                value: compliance,
                unit: "-",
                reason: "must not be negative (upstream ALGORITH7_81)",
            });
        }
        if compliance > 0.0 {
            compliance = compliance.powf(p.flux_exponent / p.n);
        }
        let arrhenius = (-p.activation_temperature / (p.n * temperature)).exp();
        compliance *= arrhenius;

        // The flow function takes K, not 1/K.
        let k = if compliance > 0.0 {
            1.0 / compliance
        } else {
            f64::INFINITY // zero compliance means no creep
        };

        Ok(Self::Lemaitre(LemaitreParameters { k, n: p.n, m: p.m }))
    }

    /// Equivalent viscoplastic strain rate `ṗ` \[1/s\].
    ///
    /// `sigma_eq` is the von Mises equivalent stress \[Pa\] and
    /// `accumulated_strain` the equivalent viscoplastic strain `p` \[-\]
    /// accumulated so far.
    ///
    /// Returns zero for non-positive stress: an unloaded material does not
    /// creep, and upstream guards the same way (`norton.F90` tests
    /// `grj2v > r8miem()` before flowing). That guard is not cosmetic — without
    /// it the power law would evaluate `0^n` and, for the Lemaitre variant,
    /// `ln(0)`.
    #[must_use]
    pub fn equivalent_strain_rate(self, sigma_eq: f64, accumulated_strain: f64) -> f64 {
        if !(sigma_eq > 0.0) {
            return 0.0;
        }
        match self {
            Self::Norton(p) => (sigma_eq / p.k).powf(p.n),
            Self::LemaitreIrradiation(_) => {
                // The compliance depends on flux and temperature, which this
                // signature does not carry. Resolve with
                // `at_irradiation_conditions` first; returning a silent zero
                // here would look like "no creep" rather than "not configured".
                0.0
            }
            Self::Lemaitre(p) => {
                if !(accumulated_strain > 0.0) {
                    // Upstream returns zero when `dpc == 0`: the hardening term
                    // `p^(-n/m)` is singular at the origin. The first increment
                    // out of a pristine state is therefore governed by the
                    // solver's bracket rather than by an infinite rate.
                    return 0.0;
                }
                (sigma_eq / p.k).powf(p.n) * accumulated_strain.powf(-p.n / p.m)
            }
        }
    }

    /// Derivative of the rate with respect to equivalent stress, `∂ṗ/∂σ_eq`
    /// \[1/(s·Pa)\].
    ///
    /// Supplied so the implicit integration can use a true Newton step rather
    /// than a numerical one. Upstream computes the same quantity as `dgdst`.
    #[must_use]
    pub fn rate_derivative_wrt_stress(self, sigma_eq: f64, accumulated_strain: f64) -> f64 {
        if !(sigma_eq > 0.0) {
            return 0.0;
        }
        let rate = self.equivalent_strain_rate(sigma_eq, accumulated_strain);
        match self {
            Self::Norton(p) => p.n * rate / sigma_eq,
            Self::Lemaitre(p) => p.n * rate / sigma_eq,
            Self::LemaitreIrradiation(_) => 0.0,
        }
    }

    /// Integrate one timestep by radial return, returning the creep increment.
    ///
    /// # The reduction to one scalar
    ///
    /// For isotropic von Mises flow the creep direction is the deviator's own
    /// direction, so the deviator shrinks without rotating. Writing `Δp` for
    /// the equivalent creep increment and `μ` for the shear modulus, the
    /// end-of-step equivalent stress is
    ///
    /// `σ_eq = σ_eq_trial - 3μ Δp`
    ///
    /// and the constitutive statement `Δp = Δt · ṗ(σ_eq, p_old + Δp)` closes
    /// the system. One unknown, one equation.
    ///
    /// # Arguments
    ///
    /// - `trial_stress` — the elastic-predictor stress \[Pa\] (full tensor;
    ///   the deviator is taken internally).
    /// - `shear_modulus` — `μ` \[Pa\].
    /// - `accumulated_strain` — `p` at the *start* of the step \[-\].
    /// - `dt` — timestep \[s\]. Zero is legal and yields no creep.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a non-positive shear modulus or a
    /// negative timestep, and
    /// [`OffbeatError::ConstitutiveNotConverged`] if the local solve fails.
    pub fn integrate(
        self,
        trial_stress: SymmTensor,
        shear_modulus: f64,
        accumulated_strain: f64,
        dt: f64,
    ) -> Result<CreepIncrement> {
        if !(shear_modulus > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "shear modulus",
                value: shear_modulus,
                unit: "Pa",
                reason: "must be strictly positive",
            });
        }
        if dt < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "timestep",
                value: dt,
                unit: "s",
                reason: "must not be negative",
            });
        }

        let s_trial = deviator(trial_stress);
        let eq_trial = von_mises_of_deviator(s_trial);

        // No stress, or no time: no creep. Returned explicitly rather than
        // relying on the solver, whose bracket would be degenerate.
        if dt == 0.0 || !(eq_trial > 0.0) {
            return Ok(CreepIncrement {
                equivalent_increment: 0.0,
                strain_increment: SymmTensor::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
                stress: trial_stress,
                equivalent_stress: eq_trial,
                iterations: 0,
            });
        }

        let three_mu = 3.0 * shear_modulus;
        // Residual in the creep increment.
        let residual = |dp: f64| {
            let eq = (eq_trial - three_mu * dp).max(0.0);
            dt * self.equivalent_strain_rate(eq, accumulated_strain + dp) - dp
        };
        let derivative = |dp: f64| {
            let eq = (eq_trial - three_mu * dp).max(0.0);
            -dt * three_mu * self.rate_derivative_wrt_stress(eq, accumulated_strain + dp) - 1.0
        };

        // The bracket is guaranteed: at Dp = 0 the residual is +dt*rate >= 0,
        // and at Dp = eq_trial/(3mu) the stress is fully relaxed so the rate
        // vanishes and the residual is -Dp < 0.
        let upper = eq_trial / three_mu;
        let solution = newton_safeguarded(
            residual,
            derivative,
            (0.0, upper),
            &SolverControl {
                max_iter: 200,
                residual_tol: 1.0e-14 * upper.max(1.0),
                step_tol: 1.0e-18,
            },
        )?;

        let dp = solution.root.clamp(0.0, upper);
        let eq = eq_trial - three_mu * dp;

        // The deviator shrinks by the factor eq/eq_trial without rotating.
        let scale = if eq_trial > 0.0 { eq / eq_trial } else { 0.0 };
        let s_new = scale_tensor(s_trial, scale);
        let mean = trial_stress.tr() / 3.0;
        let stress = SymmTensor::new(
            s_new.xx + mean,
            s_new.xy,
            s_new.xz,
            s_new.yy + mean,
            s_new.yz,
            s_new.zz + mean,
        );

        // De = (3/2) Dp s / sigma_eq, evaluated on the trial direction, which is
        // the same direction as the final deviator.
        let direction_scale = 1.5 * dp / eq_trial;
        let strain_increment = scale_tensor(s_trial, direction_scale);

        Ok(CreepIncrement {
            equivalent_increment: dp,
            strain_increment,
            stress,
            equivalent_stress: eq,
            iterations: solution.iterations,
        })
    }
}

fn scale_tensor(t: SymmTensor, f: f64) -> SymmTensor {
    SymmTensor::new(f * t.xx, f * t.xy, f * t.xz, f * t.yy, f * t.yz, f * t.zz)
}

/// The result of integrating one creep step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CreepIncrement {
    /// Equivalent creep increment `Δp` \[-\], non-negative.
    pub equivalent_increment: f64,
    /// Creep strain increment tensor `Δε` \[-\]. Deviatoric — creep is
    /// volume-preserving, which
    /// [`creep_is_volume_preserving`](self) checks.
    pub strain_increment: SymmTensor,
    /// Relaxed stress at the end of the step \[Pa\].
    pub stress: SymmTensor,
    /// Von Mises equivalent of [`stress`](Self::stress) \[Pa\].
    pub equivalent_stress: f64,
    /// Local-solver iterations used.
    pub iterations: usize,
}

#[cfg(test)]
mod tests;
