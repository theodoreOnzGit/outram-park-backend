// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
// `offbeatLib/materials/materialModel/thermoMechanicalPropertiesModels/PoissonRatio/`,
// specifically the files:
//   PoissonRatioConstant.{C,H}            -> PoissonRatioModel::Constant
//   constantPoissonRatioUO2.{C,H}         -> PoissonRatioModel::MatproUo2
//   constantPoissonRatioUPuO2.{C,H}       -> PoissonRatioModel::MatproMox
//   constantPoissonRatioZy.{C,H}          -> PoissonRatioModel::ConstantZircaloy
//   constantPoissonRatioMolybdenum.{C,H}  -> PoissonRatioModel::ConstantMolybdenum
//   PoissonRatioMatproZy.{C,H}            -> PoissonRatioModel::MatproZircaloy
//   PoissonRatioTobbe1515Ti.{C,H}         -> PoissonRatioModel::Tobbe1515Ti
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Poisson's ratio correlations \[-\].
//!
//! # What this module computes
//!
//! Poisson's ratio `nu` — the negative ratio of transverse to axial strain
//! under uniaxial load — for fuel, cladding and structural materials, as a pure
//! function of the local [`MaterialState`]. The quantity is **dimensionless**;
//! every value returned here is a bare number, not a percentage.
//!
//! Most of the upstream models are simply the constant a given material is
//! conventionally assigned (0.316 for UO2, 0.276 for MOX, 0.3 for Zircaloy);
//! only two vary with state, and one of those does so by dividing a Young's
//! modulus by a shear modulus.
//!
//! # Why the mechanics solve needs it
//!
//! With Young's modulus `E` from
//! [`young_modulus`](crate::materials::properties::young_modulus), Poisson's
//! ratio gives the two Lame parameters an isotropic-elasticity momentum
//! equation consumes:
//!
//! $$ \mu = \frac{E}{2(1 + \nu)} $$
//!
//! $$ \lambda = \frac{E \nu}{(1 + \nu)(1 - 2\nu)} $$
//!
//! **This module does not build them and does not solve anything** — that is
//! [`crate::mechanics`]. What lives here is only the property lookup.
//!
//! The `1 - 2*nu` in `lambda` is the reason this module takes admissibility
//! seriously. As `nu` approaches 0.5 the material becomes incompressible and
//! `lambda` diverges; at `nu = 0.5` exactly it is a division by zero, and above
//! 0.5 `lambda` changes sign and the elasticity tensor stops being positive
//! definite.
//!
//! # Thermodynamic admissibility
//!
//! For an isotropic linear-elastic solid, positive-definiteness of the strain
//! energy requires
//!
//! $$ -1 < \nu < 0.5 $$
//!
//! This is a real physical constraint, not a modelling convention: `nu <= -1`
//! makes the bulk modulus negative and `nu >= 0.5` makes it infinite or
//! negative. The unit tests below check it for every variant across its valid
//! range, and **one variant fails it in part of that range** — see
//! [`MatproZircaloy`](PoissonRatioModel::MatproZircaloy). That failure is
//! reported here rather than papered over, because it is a genuine property of
//! the upstream correlation pair. Use
//! [`is_admissible`](PoissonRatioModel::is_admissible) to test a result.
//!
//! # Validity ranges, clamping and checking
//!
//! Same contract as the companion Young's-modulus module:
//! [`value`](PoissonRatioModel::value) clamps out-of-range temperatures to the
//! endpoints and always returns a number;
//! [`value_checked`](PoissonRatioModel::value_checked) returns
//! [`OffbeatError::OutOfRange`] instead of extrapolating.
//!
//! # Known divergences from upstream
//!
//! 1. **Isotropic cracking is not implemented here.** Upstream's UO2 and MOX
//!    Poisson models optionally rescale `nu` by a crack factor driven by a
//!    `nCracks` field. That is damage-model state, not a pure function of
//!    [`MaterialState`]. All variants return the uncracked value.
//! 2. **`MatproZircaloy` composes its own Young's modulus.** Upstream looks the
//!    Young's-modulus *field* `E` up on the mesh registry, whatever model
//!    produced it, and divides it by the MATPRO shear modulus. This port pairs
//!    the MATPRO shear modulus with the MATPRO Young's modulus
//!    ([`YoungModulusModel::MatproZircaloy`]) — the internally consistent
//!    combination MATPRO itself defines. Mixing a MATPRO shear modulus with,
//!    say, a constant Young's modulus (which upstream permits) is not
//!    reproducible here, and should not be wanted.
//! 3. **Fast fluence is in n/m^2.** Upstream's Young's-modulus Zircaloy model
//!    scales the fluence field by `1e4` and its Poisson counterpart does not —
//!    an internal inconsistency. This port uses
//!    [`MaterialState::fast_fluence`] in n/m^2 in both. In the alpha phase the
//!    choice is immaterial to `nu` anyway: the fluence factor cancels exactly
//!    between `E` and `G` (there is a test for this).
//! 4. **`ConstantMolybdenum` returns 0.31.** Upstream's
//!    `constantPoissonRatioMolybdenum` initialises the member to `0.31` but
//!    declares the *dictionary* default as `0.316`, so a case with no
//!    `PoissonRatio` sub-dictionary gets 0.31 and one with an empty
//!    sub-dictionary gets 0.316. This port takes the no-dictionary value, 0.31.
//!
//! [`MaterialState::fast_fluence`]: crate::materials::MaterialState::fast_fluence
//! [`YoungModulusModel::MatproZircaloy`]: crate::materials::properties::young_modulus::YoungModulusModel::MatproZircaloy

use crate::error::{OffbeatError, Result};
use crate::materials::properties::young_modulus::matpro_zircaloy_young;
use crate::materials::MaterialState;

/// Lower bound \[-\] of the thermodynamically admissible range of Poisson's
/// ratio for an isotropic linear-elastic solid, exclusive.
///
/// At `nu = -1` the elasticity tensor stops being positive definite: the bulk
/// modulus `E/(3(1-2nu))` and the shear modulus can no longer both be
/// positive.
pub const POISSON_RATIO_MIN: f64 = -1.0;

/// Upper bound \[-\] of the thermodynamically admissible range of Poisson's
/// ratio for an isotropic linear-elastic solid, exclusive.
///
/// At `nu = 0.5` the material is incompressible: the bulk modulus and the Lame
/// parameter `lambda = E*nu/((1+nu)(1-2nu))` both diverge. Above 0.5 `lambda`
/// changes sign.
pub const POISSON_RATIO_MAX: f64 = 0.5;

/// Poisson's ratio `nu` \[-\] of a fuel, cladding or structural material.
///
/// # What it is
///
/// The negative ratio of transverse to axial strain in uniaxial loading: how
/// much a bar thins when you stretch it. Dimensionless, and for a
/// thermodynamically admissible isotropic solid strictly between
/// [`POISSON_RATIO_MIN`] and [`POISSON_RATIO_MAX`].
///
/// # Dispatch
///
/// An enum, not a trait object — see the workspace `CLAUDE.md` "No trait
/// objects" rule and the [module documentation](crate::materials).
///
/// # Example
///
/// ```
/// use outram_park_fork_offbeat::materials::MaterialState;
/// use outram_park_fork_offbeat::materials::properties::poisson_ratio::PoissonRatioModel;
///
/// let state = MaterialState::fresh(900.0);
/// assert_eq!(PoissonRatioModel::MatproUo2.value(&state), 0.316);
/// assert!(PoissonRatioModel::MatproUo2.is_admissible(&state));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PoissonRatioModel {
    /// A user-supplied constant Poisson's ratio \[-\], independent of state.
    ///
    /// Upstream: `PoissonRatioConstant`, which reads `nu` from the material
    /// dictionary.
    ///
    /// **Valid range:** none in temperature. The payload should lie strictly
    /// between [`POISSON_RATIO_MIN`] and [`POISSON_RATIO_MAX`];
    /// [`value_checked`](Self::value_checked) reports it as
    /// [`OffbeatError::Unphysical`] otherwise.
    Constant(f64),

    /// UO2 fuel: the constant 0.316 from MATPRO-11.
    ///
    /// Upstream: `constantPoissonRatioUO2`, whose hard-coded default is
    /// `0.316`. The same number appears inside upstream's UO2
    /// Young's-modulus model as the `nui` used by its crack-softening factor.
    ///
    /// **Inputs used:** none — the value is a constant.
    ///
    /// **Valid range:** 300 K to 3113 K (room temperature to the UO2 melting
    /// point), **port-imposed**. The value itself does not vary with
    /// temperature; the range records where the fuel model is meant to be used
    /// and keeps the two fuel-property families consistent with each other.
    ///
    /// **Source:** MATPRO-11 (Rev. 2), as transcribed in OFFBEAT.
    MatproUo2,

    /// (U,Pu)O2 MOX fuel: the constant 0.276 from MATPRO-11.
    ///
    /// Upstream: `constantPoissonRatioUPuO2`, whose hard-coded default is
    /// `0.276` — the same `nui` used inside upstream's MOX Young's-modulus
    /// models.
    ///
    /// **Inputs used:** none — the value is a constant.
    ///
    /// **Valid range:** 300 K to 3023 K (room temperature to the approximate
    /// MOX melting point), **port-imposed**, for the same reason as
    /// [`MatproUo2`](Self::MatproUo2).
    ///
    /// **Source:** MATPRO-11 (Rev. 2), as transcribed in OFFBEAT.
    MatproMox,

    /// Zircaloy cladding: the conventional constant 0.3.
    ///
    /// Upstream: `constantPoissonRatioZy`, hard-coded default `0.3`. Use this
    /// when a temperature-independent cladding Poisson's ratio is wanted;
    /// [`MatproZircaloy`](Self::MatproZircaloy) is the state-dependent
    /// alternative, and the two differ by more than 0.6 at the top of the
    /// range.
    ///
    /// **Inputs used:** none — the value is a constant.
    ///
    /// **Valid range:** 290 K to 1800 K, **port-imposed** to match the MATPRO
    /// Zircaloy models' stated range.
    ///
    /// **Source:** the conventional value for Zircaloy, as transcribed in
    /// OFFBEAT.
    ConstantZircaloy,

    /// Molybdenum structure: the constant 0.31.
    ///
    /// Upstream: `constantPoissonRatioMolybdenum`. **Note the upstream
    /// inconsistency:** the member initialiser is `0.31` while the dictionary
    /// default it advertises is `0.316`, so upstream returns 0.31 for a case
    /// with no `PoissonRatio` sub-dictionary and 0.316 for one with an empty
    /// sub-dictionary. This port takes 0.31, the no-dictionary value. Use
    /// [`Constant`](Self::Constant) if you specifically want 0.316.
    ///
    /// **Inputs used:** none — the value is a constant.
    ///
    /// **Valid range:** 300 K to 2896 K (room temperature to the melting point
    /// of molybdenum), **port-imposed**.
    ///
    /// **Source:** as transcribed in OFFBEAT.
    ConstantMolybdenum,

    /// Zircaloy cladding, MATPRO-11: derived from the MATPRO Young's and shear
    /// moduli.
    ///
    /// Upstream: `PoissonRatioMatproZy`. Rather than tabulating `nu`, MATPRO
    /// fits `E` and the shear modulus `G` independently and forms
    ///
    /// ```text
    /// nu = E / (2 * G) - 1
    /// ```
    ///
    /// with `G` from the same three-branch phase structure as the Young's
    /// modulus — see [`matpro_zircaloy_shear_modulus`]:
    ///
    /// ```text
    /// K1 = (7.07e11 - 2.315e8 * T) * C_ox        oxygen effect
    /// K2 = -2.6e10 * C_cw                        cold-work effect
    /// K3 = 0.88 + 0.12 * exp(-phi / 1e25)        fast-fluence effect
    ///
    /// alpha (T < 1073 K):  G = (4.04e10 - 2.168e7 * T + K1 + K2) / K3
    /// beta  (T >= 1273 K): G = 3.49e10 - 1.66e7 * T
    /// 1073 <= T < 1273 K:  linear interpolation, alpha value at 1073 K and
    ///                      beta value at 1273 K
    /// ```
    ///
    /// Note the sign difference from the Young's-modulus oxygen term:
    /// `+par2*T` there, `-par2*T` here. That asymmetry is upstream's (and
    /// MATPRO's) form, not a transcription slip.
    ///
    /// # This variant can leave the admissible range
    ///
    /// Because `E` and `G` are two *independently fitted* lines, their ratio is
    /// not constrained to keep `nu < 0.5`, and in the beta phase it does not:
    ///
    /// - Unirradiated, uncold-worked cladding crosses `nu = 0.5` at
    ///   **T = 1354.84 K** and reaches `nu = 0.912` at the top of the range
    ///   (1800 K).
    /// - At 600 K, a retained cold-work fraction above **0.1197** also pushes
    ///   `nu` past 0.5, because `K2` subtracts the same absolute amount from
    ///   both numerators and `G` is roughly a third of `E`.
    ///
    /// Neither is a port error — both are properties of the upstream
    /// correlation pair, verified against the transcribed coefficients and
    /// pinned down by the unit tests. Call
    /// [`is_admissible`](Self::is_admissible) before handing the result to a
    /// mechanics solve, or use [`ConstantZircaloy`](Self::ConstantZircaloy) in
    /// the beta phase.
    ///
    /// **Inputs used:** [`temperature`](MaterialState::temperature),
    /// [`fast_fluence`](MaterialState::fast_fluence) \[n/m^2\],
    /// [`cold_work`](MaterialState::cold_work),
    /// [`oxygen_content`](MaterialState::oxygen_content) \[weight fraction\].
    ///
    /// **Valid range:** 290 K to 1800 K — **stated by upstream**, which warns
    /// outside it.
    ///
    /// **Source:** MATPRO-11 (Rev. 2), as transcribed in OFFBEAT.
    MatproZircaloy,

    /// 15-15 Ti austenitic stainless-steel cladding, Tobbe correlation (1975).
    ///
    /// Upstream: `PoissonRatioTobbe1515Ti`.
    ///
    /// ```text
    /// nu = 0.277 + 6e-5 * T_C
    /// ```
    ///
    /// with `T_C` the temperature in **degrees Celsius** (`T - 273.15`). The
    /// only variant here that rises smoothly with temperature, and it stays
    /// comfortably admissible: at the top of its range (1273 K, i.e. 999.85 C)
    /// it reaches 0.337.
    ///
    /// **Inputs used:** [`temperature`](MaterialState::temperature).
    ///
    /// **Valid range:** 293 K to 1273 K, **port-imposed** — upstream's Poisson
    /// model performs no check, but its Young's-modulus counterpart
    /// (`YoungModulusTobbe1515Ti`) warns outside exactly this range, and the
    /// two come from the same 1975 correlation set.
    ///
    /// **Source:** Tobbe (1975), as named in upstream's
    /// `PoissonRatioTobbe1515Ti.H`.
    Tobbe1515Ti,
}

impl PoissonRatioModel {
    /// Human-readable name of the correlation, used in error messages.
    ///
    /// Stable enough to match on in a log, but not a serialisation format —
    /// use the enum itself for that.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Constant(_) => "constant Poisson's ratio",
            Self::MatproUo2 => "MATPRO UO2 Poisson's ratio",
            Self::MatproMox => "MATPRO (U,Pu)O2 Poisson's ratio",
            Self::ConstantZircaloy => "constant Zircaloy Poisson's ratio",
            Self::ConstantMolybdenum => "constant molybdenum Poisson's ratio",
            Self::MatproZircaloy => "MATPRO Zircaloy Poisson's ratio",
            Self::Tobbe1515Ti => "Tobbe 15-15 Ti Poisson's ratio",
        }
    }

    /// Temperature validity range `(low, high)` \[K\] of this correlation.
    ///
    /// Only [`MatproZircaloy`](Self::MatproZircaloy) has a range upstream
    /// states; the rest are port-imposed and each variant's documentation says
    /// so. A [`Constant`](Self::Constant) ratio is valid everywhere and reports
    /// the whole positive axis up to a nominal 1e5 K.
    #[must_use]
    pub fn temperature_range(&self) -> (f64, f64) {
        match self {
            Self::Constant(_) => (0.0, 1.0e5),
            Self::MatproUo2 => (300.0, 3113.0),
            Self::MatproMox => (300.0, 3023.0),
            Self::ConstantZircaloy | Self::MatproZircaloy => (290.0, 1800.0),
            Self::ConstantMolybdenum => (300.0, 2896.0),
            Self::Tobbe1515Ti => (293.0, 1273.0),
        }
    }

    /// Poisson's ratio \[-\] at the given state, **clamping** an out-of-range
    /// temperature to the endpoints of the validity range.
    ///
    /// # Clamping — read this before trusting a number
    ///
    /// This method never fails and never extrapolates: the temperature is
    /// clamped into [`temperature_range`](Self::temperature_range) before the
    /// correlation is evaluated. A call at 2500 K on the Zircaloy model returns
    /// the 1800 K value. That matches the spirit of upstream, which warns and
    /// carries on, and it is what a solver loop wants. Use
    /// [`value_checked`](Self::value_checked) when you need to know that the
    /// correlation does not cover your conditions.
    ///
    /// **Clamping does not guarantee admissibility.** The returned value can
    /// still fall outside `(-1, 0.5)` for
    /// [`MatproZircaloy`](Self::MatproZircaloy); check
    /// [`is_admissible`](Self::is_admissible) if the caller cannot tolerate
    /// that.
    ///
    /// # Example
    ///
    /// ```
    /// use outram_park_fork_offbeat::materials::MaterialState;
    /// use outram_park_fork_offbeat::materials::properties::poisson_ratio::PoissonRatioModel;
    ///
    /// let model = PoissonRatioModel::Tobbe1515Ti;    // valid 293-1273 K
    /// let hot = MaterialState::fresh(2000.0);
    /// let edge = MaterialState::fresh(1273.0);
    /// assert_eq!(model.value(&hot), model.value(&edge)); // clamped
    /// ```
    #[must_use]
    pub fn value(&self, state: &MaterialState) -> f64 {
        let (low, high) = self.temperature_range();
        let temperature = state.temperature.clamp(low, high);
        self.evaluate(temperature, state)
    }

    /// Poisson's ratio \[-\] at the given state, or
    /// [`OffbeatError::OutOfRange`] if the correlation was not fitted there.
    ///
    /// Checks performed, in order:
    ///
    /// 1. Temperature is positive — otherwise [`OffbeatError::Unphysical`].
    /// 2. Temperature lies in [`temperature_range`](Self::temperature_range).
    /// 3. For [`Constant`](Self::Constant), the payload lies strictly between
    ///    [`POISSON_RATIO_MIN`] and [`POISSON_RATIO_MAX`].
    ///
    /// The *returned* value is deliberately **not** checked for admissibility,
    /// because for [`MatproZircaloy`](Self::MatproZircaloy) an inadmissible
    /// result is a faithful reproduction of the correlation rather than an
    /// input error. Test the output with
    /// [`is_admissible`](Self::is_admissible) if that matters to the caller.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::OutOfRange`] when the temperature is outside the fit's
    /// validity range, [`OffbeatError::Unphysical`] for a non-positive
    /// temperature or an inadmissible constant payload.
    ///
    /// # Example
    ///
    /// ```
    /// use outram_park_fork_offbeat::materials::MaterialState;
    /// use outram_park_fork_offbeat::materials::properties::poisson_ratio::PoissonRatioModel;
    ///
    /// let model = PoissonRatioModel::MatproZircaloy;   // valid 290-1800 K
    /// assert!(model.value_checked(&MaterialState::fresh(600.0)).is_ok());
    /// assert!(model.value_checked(&MaterialState::fresh(2500.0)).is_err());
    /// ```
    pub fn value_checked(&self, state: &MaterialState) -> Result<f64> {
        if !state.temperature.is_finite() || state.temperature <= 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: self.name(),
                value: state.temperature,
                unit: "K",
                reason: "absolute temperature must be strictly positive",
            });
        }

        let (low, high) = self.temperature_range();
        if state.temperature < low || state.temperature > high {
            return Err(OffbeatError::OutOfRange {
                quantity: self.name(),
                value: state.temperature,
                low,
                high,
                unit: "K",
            });
        }

        if let Self::Constant(nu) = *self {
            if !(nu > POISSON_RATIO_MIN && nu < POISSON_RATIO_MAX) {
                return Err(OffbeatError::Unphysical {
                    quantity: self.name(),
                    value: nu,
                    unit: "-",
                    reason: "Poisson's ratio must lie strictly within (-1, 0.5)",
                });
            }
        }

        Ok(self.evaluate(state.temperature, state))
    }

    /// Whether [`value`](Self::value) at this state is thermodynamically
    /// admissible, i.e. strictly inside `(-1, 0.5)`.
    ///
    /// Outside that interval the isotropic elasticity tensor is not positive
    /// definite and the Lame parameter `lambda = E*nu/((1+nu)(1-2nu))` is
    /// singular or negative, so a mechanics solve fed such a value produces
    /// nonsense rather than an inaccurate answer.
    ///
    /// Only [`MatproZircaloy`](Self::MatproZircaloy) can return `false` for a
    /// physically sensible state — above about 1355 K, or above a retained
    /// cold-work fraction of about 0.12 — and that is a property of the MATPRO
    /// correlation pair, not of this port. See the variant documentation.
    ///
    /// # Example
    ///
    /// ```
    /// use outram_park_fork_offbeat::materials::MaterialState;
    /// use outram_park_fork_offbeat::materials::properties::poisson_ratio::PoissonRatioModel;
    ///
    /// let model = PoissonRatioModel::MatproZircaloy;
    /// assert!(model.is_admissible(&MaterialState::fresh(600.0)));
    /// assert!(!model.is_admissible(&MaterialState::fresh(1700.0)));
    /// ```
    #[must_use]
    pub fn is_admissible(&self, state: &MaterialState) -> bool {
        let nu = self.value(state);
        nu > POISSON_RATIO_MIN && nu < POISSON_RATIO_MAX
    }

    /// Evaluate the correlation at an already-validated (or already-clamped)
    /// temperature \[K\].
    ///
    /// Split out so [`value`](Self::value) and
    /// [`value_checked`](Self::value_checked) share one copy of the physics.
    fn evaluate(&self, temperature: f64, state: &MaterialState) -> f64 {
        match *self {
            Self::Constant(nu) => nu,
            Self::MatproUo2 => 0.316,
            Self::MatproMox => 0.276,
            Self::ConstantZircaloy => 0.3,
            Self::ConstantMolybdenum => 0.31,
            Self::MatproZircaloy => {
                let e = matpro_zircaloy_young(temperature, state);
                let g = matpro_zircaloy_shear_modulus(temperature, state);
                e / (2.0 * g) - 1.0
            }
            Self::Tobbe1515Ti => {
                let t_c = temperature - CELSIUS_OFFSET;
                0.277 + 6.0e-5 * t_c
            }
        }
    }
}

/// Offset \[K\] between the kelvin and Celsius scales.
///
/// The Tobbe fit is published in degrees Celsius; this is the only place the
/// conversion is written down in this module.
const CELSIUS_OFFSET: f64 = 273.15;

/// MATPRO-11 shear modulus `G` \[Pa\] of Zircaloy at temperature `t` \[K\].
///
/// # What it is
///
/// The elastic resistance to shear — the companion fit to
/// [`YoungModulusModel::MatproZircaloy`]. MATPRO fits `E` and `G` separately
/// and derives Poisson's ratio from them as `nu = E/(2G) - 1`, which is what
/// [`PoissonRatioModel::MatproZircaloy`] does.
///
/// Exposed publicly because a mechanics layer that wants the shear modulus
/// should not have to rebuild it from `E` and `nu`: that round trip loses
/// precision and hides which quantity was actually fitted.
///
/// # Structure
///
/// Alpha phase below 1073 K with oxygen, cold-work and fast-fluence
/// corrections; beta phase at and above 1273 K as a bare line in temperature;
/// linear interpolation between, with the alpha endpoint taken at 1073 K and
/// the beta endpoint at 1273 K.
///
/// # Inputs
///
/// - `t` — temperature \[K\]. Valid 290 K to 1800 K (upstream's stated range).
///   **Not clamped here**: callers are expected to have clamped or checked
///   already, as [`PoissonRatioModel::value`] does.
/// - `state` — supplies [`fast_fluence`](MaterialState::fast_fluence)
///   \[n/m^2\], [`cold_work`](MaterialState::cold_work) \[-\] and
///   [`oxygen_content`](MaterialState::oxygen_content) \[weight fraction\].
///
/// # Source
///
/// MATPRO-11 (Rev. 2), as transcribed in upstream's `PoissonRatioMatproZy.C`.
///
/// # Example
///
/// ```
/// use outram_park_fork_offbeat::materials::MaterialState;
/// use outram_park_fork_offbeat::materials::properties::poisson_ratio::matpro_zircaloy_shear_modulus;
///
/// // Unirradiated Zircaloy at 300 K: G = 4.04e10 - 2.168e7 * 300 Pa.
/// let g = matpro_zircaloy_shear_modulus(300.0, &MaterialState::fresh(300.0));
/// assert!((g - 3.3896e10).abs() < 1.0);
/// ```
///
/// [`YoungModulusModel::MatproZircaloy`]: crate::materials::properties::young_modulus::YoungModulusModel::MatproZircaloy
#[must_use]
pub fn matpro_zircaloy_shear_modulus(t: f64, state: &MaterialState) -> f64 {
    const PAR1: f64 = 7.07e11;
    const PAR2: f64 = 2.315e8;
    const PAR3: f64 = 2.6e10;
    const PAR4: f64 = 0.88;
    const PAR5: f64 = 0.12;
    const PAR6: f64 = 1.0e25;
    const PAR7: f64 = 4.04e10;
    const PAR8: f64 = 2.168e7;
    const PAR9: f64 = 3.49e10;
    const PAR10: f64 = 1.66e7;
    const T_ALPHA: f64 = 1073.0;
    const T_BETA: f64 = 1273.0;

    let alpha = |temp: f64| {
        // Note the minus sign on PAR2 — the Young's-modulus counterpart has a
        // plus. That asymmetry is MATPRO's, and is reproduced deliberately.
        let k1 = (PAR1 - PAR2 * temp) * state.oxygen_content;
        let k2 = -PAR3 * state.cold_work;
        let k3 = PAR4 + PAR5 * (-state.fast_fluence / PAR6).exp();
        (PAR7 - PAR8 * temp + k1 + k2) / k3
    };
    let beta = |temp: f64| PAR9 - PAR10 * temp;

    if t < T_ALPHA {
        alpha(t)
    } else if t < T_BETA {
        let g_alpha = alpha(T_ALPHA);
        let g_beta = beta(T_BETA);
        g_alpha * (T_BETA - t) / (T_BETA - T_ALPHA) + g_beta * (t - T_ALPHA) / (T_BETA - T_ALPHA)
    } else {
        beta(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::materials::properties::young_modulus::YoungModulusModel;

    /// Temperature \[K\] above which the MATPRO Zircaloy `E`/`G` pair yields
    /// `nu >= 0.5` for unirradiated, uncold-worked cladding.
    ///
    /// Derived analytically: `nu = 0.5` requires `E = 3G`, i.e.
    /// `9.21e10 - 4.05e7*T = 3*(3.49e10 - 1.66e7*T)`, giving
    /// `T = 1.26e10 / 9.3e6 = 1354.8387 K`. The unit test below confirms it
    /// numerically by bisection.
    const MATPRO_ZY_ADMISSIBILITY_LIMIT: f64 = 1354.8387;

    /// Relative-difference helper for the reference checks below.
    fn rel_diff(a: f64, b: f64) -> f64 {
        (a - b).abs() / b.abs()
    }

    /// Every variant, for sweep-style tests.
    fn all_variants() -> Vec<PoissonRatioModel> {
        vec![
            PoissonRatioModel::Constant(0.33),
            PoissonRatioModel::MatproUo2,
            PoissonRatioModel::MatproMox,
            PoissonRatioModel::ConstantZircaloy,
            PoissonRatioModel::ConstantMolybdenum,
            PoissonRatioModel::MatproZircaloy,
            PoissonRatioModel::Tobbe1515Ti,
        ]
    }

    // ---------------------------------------------------------------------
    // The constant-valued models
    // ---------------------------------------------------------------------

    /// Reference-checked against the values stated in the upstream sources.
    ///
    /// Methodology: four of the seven variants are fixed numbers upstream
    /// hard-codes — `constantPoissonRatioUO2` 0.316,
    /// `constantPoissonRatioUPuO2` 0.276, `constantPoissonRatioZy` 0.3 and
    /// `constantPoissonRatioMolybdenum` 0.31 (its member initialiser; see the
    /// variant docs for upstream's 0.31/0.316 inconsistency). Each is evaluated
    /// at 300 K, 900 K and 1500 K. Pass criterion: exact equality with the
    /// upstream constant at every temperature.
    ///
    /// Result: 0.316, 0.276, 0.3 and 0.31 recovered exactly at all three
    /// temperatures.
    #[test]
    fn constant_models_return_the_upstream_hard_coded_values() {
        let cases = [
            (PoissonRatioModel::MatproUo2, 0.316),
            (PoissonRatioModel::MatproMox, 0.276),
            (PoissonRatioModel::ConstantZircaloy, 0.3),
            (PoissonRatioModel::ConstantMolybdenum, 0.31),
        ];
        for (model, expected) in cases {
            for t in [300.0, 900.0, 1500.0] {
                assert_eq!(
                    model.value(&MaterialState::fresh(t)),
                    expected,
                    "{} at {t} K",
                    model.name()
                );
            }
        }
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: the user-supplied constant must be returned unchanged, and
    /// a payload outside `(-1, 0.5)` must be reported as `Unphysical` rather
    /// than silently used, since it would make the Lame parameter `lambda`
    /// singular or negative. Inputs: 0.33 (valid), 0.5 and -1.2 (invalid).
    /// Pass criterion: exact echo for the valid payload, `Err(Unphysical)` for
    /// both invalid ones.
    ///
    /// Result: 0.33 echoed exactly; both invalid payloads rejected.
    #[test]
    fn constant_echoes_its_payload_and_rejects_inadmissible_ones() {
        let s = MaterialState::fresh(600.0);
        assert_eq!(PoissonRatioModel::Constant(0.33).value(&s), 0.33);
        for bad in [0.5, -1.2] {
            assert!(
                matches!(
                    PoissonRatioModel::Constant(bad).value_checked(&s),
                    Err(OffbeatError::Unphysical { .. })
                ),
                "accepted nu = {bad}"
            );
        }
    }

    // ---------------------------------------------------------------------
    // Tobbe 15-15 Ti
    // ---------------------------------------------------------------------

    /// Reference-checked against the upstream expression (port verification).
    ///
    /// Methodology: `PoissonRatioTobbe1515Ti.C` computes
    /// `nu = par1 + par2 * T_C` with `par1 = 0.277`, `par2 = 6e-5`, `T_C` in
    /// degrees Celsius. Hand-evaluated at 20 C: `0.277 + 0.0012 = 0.2782`; at
    /// the top of the range (1273 K, i.e. 999.85 C):
    /// `0.277 + 0.059991 = 0.336991`. Tolerance: 1e-12 relative.
    ///
    /// Result: 0.2782 at 293.15 K and 0.336991 at 1273 K, both matching the
    /// hand evaluation and both comfortably inside `(-1, 0.5)`.
    #[test]
    fn tobbe_15_15_ti_matches_the_hand_evaluated_expression() {
        let m = PoissonRatioModel::Tobbe1515Ti;
        assert!(rel_diff(m.value(&MaterialState::fresh(293.15)), 0.2782) < 1.0e-12);
        let top = m.value(&MaterialState::fresh(1273.0));
        assert!(
            rel_diff(top, 0.277 + 6.0e-5 * 999.85) < 1.0e-12,
            "got {top}"
        );
        assert!(top < POISSON_RATIO_MAX);
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: the Tobbe fit is linear with a positive slope, so `nu` must
    /// increase with temperature and the increment over a 100 K interval must
    /// be exactly `6e-5 * 100 = 6e-3`. Inputs: 400 K and 500 K. Pass criterion:
    /// increment within 1e-12 of 6e-3.
    ///
    /// Result: increment 6.0e-3, matching to better than 1e-15.
    #[test]
    fn tobbe_15_15_ti_rises_linearly_with_temperature() {
        let m = PoissonRatioModel::Tobbe1515Ti;
        let delta = m.value(&MaterialState::fresh(500.0)) - m.value(&MaterialState::fresh(400.0));
        assert!((delta - 6.0e-3).abs() < 1.0e-12, "got {delta}");
    }

    // ---------------------------------------------------------------------
    // MATPRO Zircaloy
    // ---------------------------------------------------------------------

    /// Reference-checked against the upstream expression (port verification).
    ///
    /// Methodology: at 300 K, unirradiated and uncold-worked, MATPRO gives
    /// `E = 1.088e11 - 5.475e7*300 = 9.2375e10 Pa` and
    /// `G = 4.04e10 - 2.168e7*300 = 3.3896e10 Pa`, hence
    /// `nu = 9.2375e10/(2 * 3.3896e10) - 1 = 0.3626239084`. Tolerance: 1e-9
    /// relative. Pass criterion: the port reproduces that number, and rebuilds
    /// it from the `E` its companion Young's-modulus module returns.
    ///
    /// Result: `nu = 0.36262390843`, matching to better than 1e-15 relative;
    /// the rebuild from `YoungModulusModel::MatproZircaloy` and
    /// `matpro_zircaloy_shear_modulus` agreed to 1e-15. For context, the
    /// commonly quoted room-temperature Poisson's ratio of Zircaloy-4 is about
    /// 0.37 — the same to two significant figures — but this test verifies the
    /// transcription, not the correlation.
    #[test]
    fn matpro_zircaloy_matches_the_hand_evaluated_expression_at_300_k() {
        let s = MaterialState::fresh(300.0);
        let nu = PoissonRatioModel::MatproZircaloy.value(&s);
        assert!(rel_diff(nu, 0.3626239084) < 1.0e-9, "got {nu}");

        let e = YoungModulusModel::MatproZircaloy.value(&s);
        let g = matpro_zircaloy_shear_modulus(300.0, &s);
        assert!(rel_diff(e / (2.0 * g) - 1.0, nu) < 1.0e-15);
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: in the alpha phase the fast-fluence factor
    /// `K3 = 0.88 + 0.12*exp(-phi/1e25)` divides **both** the Young's and the
    /// shear modulus, so it must cancel exactly in `nu = E/(2G) - 1`. This is a
    /// structural property of the MATPRO pair and a good check that the port
    /// applies `K3` in the same place in both files. Inputs at 600 K: fluence
    /// 0, 1e25, 1e26 and 1e27 n/m^2. Pass criterion: all four values agree to
    /// 1e-12 absolute.
    ///
    /// Result: all four returned 0.386353679907 (spread 2.2e-16, i.e.
    /// floating-point round-off only), confirming exact cancellation.
    #[test]
    fn matpro_zircaloy_poisson_ratio_is_fluence_independent_in_the_alpha_phase() {
        let m = PoissonRatioModel::MatproZircaloy;
        let at = |phi: f64| {
            let mut s = MaterialState::fresh(600.0);
            s.fast_fluence = phi;
            m.value(&s)
        };
        let reference = at(0.0);
        assert!(rel_diff(reference, 0.3863536799065421) < 1.0e-10);
        for phi in [1.0e25, 1.0e26, 1.0e27] {
            assert!((at(phi) - reference).abs() < 1.0e-12, "phi = {phi:e}");
        }
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: `nu` must be continuous across the alpha/interpolation and
    /// interpolation/beta boundaries, since both `E` and `G` are continuous
    /// there by construction. Inputs: pairs straddling 1073 K and 1273 K by
    /// 1e-6 K. Pass criterion: absolute jump below 1e-6 (the finite sampling
    /// offset alone contributes of order 1e-10).
    ///
    /// Result: jumps of 2.98e-10 at 1073 K and 3.79e-10 at 1273 K — sampling
    /// noise, i.e. continuous.
    #[test]
    fn matpro_zircaloy_poisson_ratio_is_continuous_across_both_phase_boundaries() {
        let m = PoissonRatioModel::MatproZircaloy;
        for boundary in [1073.0, 1273.0] {
            let below = m.value(&MaterialState::fresh(boundary - 1.0e-6));
            let above = m.value(&MaterialState::fresh(boundary + 1.0e-6));
            assert!((below - above).abs() < 1.0e-6, "jump at {boundary} K");
        }
    }

    /// Reference-checked against an analytically derived limit (port
    /// verification), documenting a genuine limitation of the correlation.
    ///
    /// # Methodology
    ///
    /// MATPRO fits the Zircaloy Young's and shear moduli independently, so
    /// nothing constrains their ratio to keep `nu = E/(2G) - 1` below the
    /// thermodynamic limit of 0.5. In the beta phase (`T >= 1273 K`) the fits
    /// are `E = 9.21e10 - 4.05e7*T` and `G = 3.49e10 - 1.66e7*T`, so `nu = 0.5`
    /// (i.e. `E = 3G`) at
    ///
    /// ```text
    /// 9.21e10 - 4.05e7*T = 1.047e11 - 4.98e7*T
    /// T = 1.26e10 / 9.3e6 = 1354.8387 K
    /// ```
    ///
    /// The test bisects `nu(T) - 0.5` on `[1273, 1800]` K for unirradiated,
    /// uncold-worked cladding and compares the root with that analytical limit.
    /// Tolerance: 0.01 K. It also records `nu` at the top of upstream's stated
    /// range.
    ///
    /// # Result
    ///
    /// Measured crossover **1354.8387 K**, matching the analytical limit to
    /// better than 1e-3 K. Measured `nu(1800 K) = 0.91235`, far outside the
    /// admissible interval; `nu(1273 K) = 0.47236`, just inside it.
    ///
    /// # Interpretation
    ///
    /// This is **not a port defect** — it reproduces the upstream C++ exactly,
    /// and upstream neither detects nor guards against it. It means the MATPRO
    /// Zircaloy Poisson model must not be used above about 1355 K in a
    /// mechanics solve: `lambda = E*nu/((1+nu)(1-2nu))` is singular at the
    /// crossover and negative beyond it. Callers working in the beta phase
    /// should use [`PoissonRatioModel::ConstantZircaloy`], or gate on
    /// [`PoissonRatioModel::is_admissible`].
    #[test]
    fn matpro_zircaloy_poisson_ratio_exceeds_the_admissible_limit_in_the_beta_phase() {
        let m = PoissonRatioModel::MatproZircaloy;
        let nu = |t: f64| m.value(&MaterialState::fresh(t));

        assert!(
            nu(1273.0) < POISSON_RATIO_MAX,
            "1273 K should still be inside"
        );
        assert!(
            rel_diff(nu(1273.0), 0.4723602214) < 1.0e-8,
            "got {}",
            nu(1273.0)
        );
        assert!(nu(1800.0) > POISSON_RATIO_MAX, "1800 K should be outside");
        assert!(
            rel_diff(nu(1800.0), 0.9123505976) < 1.0e-8,
            "got {}",
            nu(1800.0)
        );

        let (mut low, mut high) = (1273.0_f64, 1800.0_f64);
        for _ in 0..200 {
            let mid = 0.5 * (low + high);
            if nu(mid) < POISSON_RATIO_MAX {
                low = mid;
            } else {
                high = mid;
            }
        }
        assert!(
            (low - MATPRO_ZY_ADMISSIBILITY_LIMIT).abs() < 0.01,
            "measured crossover {low} K"
        );
        assert!(!m.is_admissible(&MaterialState::fresh(1500.0)));
    }

    /// Self-consistency check (no external reference), documenting a second
    /// inadmissibility regime.
    ///
    /// # Methodology
    ///
    /// The cold-work term `K2 = -2.6e10 * C_cw` is subtracted from **both** the
    /// Young's and the shear-modulus numerators. Since `G` is roughly a third
    /// of `E`, the same absolute subtraction hurts `G` proportionally more,
    /// driving `nu` up. Setting `E - k = 3(G - k)` with `k = 2.6e10 * C_cw` at
    /// 600 K (`E = 7.595e10 Pa`, `G = 2.7392e10 Pa`) gives
    /// `k = (3G - E)/2 = 3.113e9`, i.e. `C_cw = 0.11973`. The test evaluates
    /// `nu` at cold-work fractions 0.0, 0.10 and 0.20 at 600 K and bisects for
    /// the threshold. Tolerance on the threshold: 1e-4.
    ///
    /// # Result
    ///
    /// `nu(0.0) = 0.38635`, `nu(0.10) = 0.47931`, `nu(0.20) = 0.59404`.
    /// Measured threshold **0.11973**, matching the analytical value. Retained
    /// cold-work fractions above roughly 0.12 therefore make the MATPRO
    /// Zircaloy Poisson model inadmissible even at ordinary operating
    /// temperature.
    ///
    /// # Interpretation
    ///
    /// Again a faithful reproduction of upstream, not a port defect, and worth
    /// knowing before running cold-worked (rather than recrystallised) cladding
    /// through the mechanics solve.
    #[test]
    fn matpro_zircaloy_poisson_ratio_exceeds_the_admissible_limit_at_high_cold_work() {
        let m = PoissonRatioModel::MatproZircaloy;
        let nu = |cw: f64| {
            let mut s = MaterialState::fresh(600.0);
            s.cold_work = cw;
            m.value(&s)
        };

        assert!(nu(0.0) < POISSON_RATIO_MAX);
        assert!(nu(0.10) < POISSON_RATIO_MAX);
        assert!(nu(0.20) > POISSON_RATIO_MAX);

        let (mut low, mut high) = (0.0_f64, 0.20_f64);
        for _ in 0..200 {
            let mid = 0.5 * (low + high);
            if nu(mid) < POISSON_RATIO_MAX {
                low = mid;
            } else {
                high = mid;
            }
        }
        assert!((low - 0.11973).abs() < 1.0e-4, "measured threshold {low}");
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: upstream states the 290-1800 K range for the MATPRO
    /// Zircaloy model (it warns outside). `value_checked` must reject both ends
    /// while `value` clamps. Inputs: 200 K and 2500 K. Pass criterion:
    /// `Err(OutOfRange)` from `value_checked`, and `value` equal to the
    /// corresponding endpoint evaluation.
    ///
    /// Result: both rejected; clamped values matched the 290 K and 1800 K
    /// evaluations exactly.
    #[test]
    fn matpro_zircaloy_range_check_fires_and_value_clamps() {
        let m = PoissonRatioModel::MatproZircaloy;
        assert!(matches!(
            m.value_checked(&MaterialState::fresh(200.0)),
            Err(OffbeatError::OutOfRange { .. })
        ));
        assert!(matches!(
            m.value_checked(&MaterialState::fresh(2500.0)),
            Err(OffbeatError::OutOfRange { .. })
        ));
        assert_eq!(
            m.value(&MaterialState::fresh(200.0)),
            m.value(&MaterialState::fresh(290.0))
        );
        assert_eq!(
            m.value(&MaterialState::fresh(2500.0)),
            m.value(&MaterialState::fresh(1800.0))
        );
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: the shear modulus exposed for the mechanics layer must be
    /// finite, strictly positive and monotonically decreasing over the MATPRO
    /// Zircaloy range — a non-positive shear modulus would mean a material
    /// unstable in shear. Swept over 128 points from 290 K to 1800 K for
    /// unirradiated, uncold-worked cladding. Pass criterion: positive and
    /// non-increasing throughout.
    ///
    /// Result: `G` fell monotonically from 3.41128e10 Pa at 290 K to 5.020e9 Pa
    /// at 1800 K, positive throughout.
    #[test]
    fn matpro_zircaloy_shear_modulus_is_positive_and_decreasing() {
        let mut previous = f64::INFINITY;
        for i in 0..128 {
            let t = 290.0 + (1800.0 - 290.0) * f64::from(i) / 127.0;
            let g = matpro_zircaloy_shear_modulus(t, &MaterialState::fresh(t));
            assert!(g.is_finite() && g > 0.0, "G = {g:e} at {t} K");
            assert!(g <= previous, "G rose at {t} K");
            previous = g;
        }
    }

    // ---------------------------------------------------------------------
    // Cross-variant sweeps
    // ---------------------------------------------------------------------

    /// **The thermodynamic-admissibility test.** Checked against a physical
    /// constraint, not against a dataset.
    ///
    /// # Methodology
    ///
    /// For an isotropic linear-elastic solid, positive-definiteness of the
    /// strain-energy density requires `-1 < nu < 0.5` ([`POISSON_RATIO_MIN`],
    /// [`POISSON_RATIO_MAX`]). Outside that interval the bulk modulus is
    /// negative or infinite and `lambda = E*nu/((1+nu)(1-2nu))` is singular or
    /// sign-flipped, so a mechanics solve would return nonsense rather than an
    /// inaccurate answer.
    ///
    /// Every variant is swept over 256 points spanning its declared temperature
    /// range for unirradiated, uncold-worked material. Pass criterion:
    /// `-1 < nu < 0.5` at every point, with
    /// [`PoissonRatioModel::is_admissible`] agreeing.
    ///
    /// **One documented exclusion:** [`PoissonRatioModel::MatproZircaloy`] is
    /// swept only up to 1354.8387 K, because above that the upstream `E`/`G`
    /// pair genuinely produces `nu > 0.5`. That is not swept under the carpet —
    /// it is the subject of its own test,
    /// `matpro_zircaloy_poisson_ratio_exceeds_the_admissible_limit_in_the_beta_phase`,
    /// which pins the crossover and explains it. Excluding it here keeps this
    /// test a statement about where each correlation *is* admissible.
    ///
    /// # Result
    ///
    /// All seven variants passed over the swept ranges. Extremes observed:
    /// 0.276 (MOX, constant) at the low end and 0.4999999964 (MATPRO Zircaloy,
    /// at its crossover) at the high end; every value strictly inside
    /// `(-1, 0.5)`.
    #[test]
    fn poisson_ratio_is_thermodynamically_admissible_for_every_variant() {
        for model in all_variants() {
            let (mut low, mut high) = model.temperature_range();
            if matches!(model, PoissonRatioModel::Constant(_)) {
                // Declared range is the whole positive axis; sample a sane part.
                low = 300.0;
                high = 3000.0;
            }
            if matches!(model, PoissonRatioModel::MatproZircaloy) {
                high = MATPRO_ZY_ADMISSIBILITY_LIMIT;
            }

            for i in 0..256 {
                let t = low + (high - low) * f64::from(i) / 255.0;
                let s = MaterialState::fresh(t);
                let nu = model.value(&s);
                assert!(
                    nu > POISSON_RATIO_MIN && nu < POISSON_RATIO_MAX,
                    "{} gave nu = {nu} at {t} K, outside (-1, 0.5)",
                    model.name()
                );
                assert!(model.is_admissible(&s), "{} at {t} K", model.name());
            }
        }
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: the documented contract is that `value` clamps and
    /// `value_checked` refuses. For every variant with a finite range, evaluate
    /// 1 K below the low bound and 1 K above the high bound. Pass criterion:
    /// `value` equals the endpoint evaluation, `value_checked` returns
    /// `OutOfRange`.
    ///
    /// Result: all six range-limited variants clamped and reported correctly
    /// (the `Constant` variant is exempt: it declares no meaningful range).
    #[test]
    fn value_clamps_where_value_checked_reports_out_of_range() {
        for model in all_variants() {
            if matches!(model, PoissonRatioModel::Constant(_)) {
                continue;
            }
            let (low, high) = model.temperature_range();
            for (outside, endpoint) in [(low - 1.0, low), (high + 1.0, high)] {
                let s_out = MaterialState::fresh(outside);
                let s_end = MaterialState::fresh(endpoint);
                assert_eq!(
                    model.value(&s_out),
                    model.value(&s_end),
                    "{} did not clamp at {outside} K",
                    model.name()
                );
                assert!(
                    matches!(
                        model.value_checked(&s_out),
                        Err(OffbeatError::OutOfRange { .. })
                    ),
                    "{} did not report {outside} K",
                    model.name()
                );
            }
        }
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: a non-positive absolute temperature is impossible and must
    /// be reported as `Unphysical`, for every variant. Inputs: 0 K and -10 K.
    /// Pass criterion: `Err(Unphysical { .. })`.
    ///
    /// Result: all seven variants reported `Unphysical` at both inputs.
    #[test]
    fn non_positive_temperature_is_unphysical_for_every_variant() {
        for model in all_variants() {
            for t in [0.0, -10.0] {
                assert!(
                    matches!(
                        model.value_checked(&MaterialState::fresh(t)),
                        Err(OffbeatError::Unphysical { .. })
                    ),
                    "{} accepted {t} K",
                    model.name()
                );
            }
        }
    }
}
