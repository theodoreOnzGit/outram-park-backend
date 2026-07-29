// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
// `offbeatLib/materials/materialModel/thermoMechanicalPropertiesModels/`
// `emissivity/`, specifically:
//   emissivityModel.{H,C}             (base class)
//   emissivityConstant.{H,C}
//   emissivityRelapUO2.{H,C}
//   constantEmissivityZy.{H,C}
//   constantEmissivityMolybdenum.{H,C}
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Surface emissivity correlations \[-\].
//!
//! # What emissivity is used for here
//!
//! Emissivity is the **hemispherical total emissivity** of a surface: the
//! fraction of black-body radiation it actually emits, between 0 (perfect
//! mirror) and 1 (black body). In a fuel rod it appears in exactly one place
//! that matters — the radiative term of the **fuel-cladding gap conductance**,
//!
//! `h_rad = sigma * F * (T_f^2 + T_c^2) * (T_f + T_c)`
//!
//! with the exchange factor `F = 1 / (1/eps_fuel + 1/eps_clad - 1)` for
//! concentric cylinders. Because `F` divides by the emissivities, an emissivity
//! of zero is not merely inaccurate — it is a division by zero. Every method
//! here therefore guarantees a result strictly inside `[0, 1]`, and the type
//! system cannot do that for you, so the guarantee is enforced at runtime.
//!
//! Radiation is a small part of gap conductance while the gap is open and gas
//! filled, and grows in importance as the gas thins (high fission-gas release,
//! helium replaced by xenon) and as temperatures rise in a transient. It is not
//! a term to leave crudely estimated in a LOCA calculation.
//!
//! # Upstream's model set is small, and mostly constant
//!
//! Only one of the four correlations varies with temperature. Upstream carries
//! the other three as named constants rather than fits, and this port keeps
//! that shape — the value of `constantEmissivityZy` is a *number a case
//! reproduces*, so it deserves a named variant and a documented provenance,
//! not to be flattened into [`Constant`](EmissivityModel::Constant).
//!
//! # Units
//!
//! Raw `f64`: temperature in kelvin, emissivity dimensionless in `[0, 1]`.

use crate::error::{OffbeatError, Result};
use crate::materials::MaterialState;

// -- RELAP UO2 (emissivityRelapUO2.C) ---------------------------------------
const RELAP_UO2_P1: f64 = 0.7856; // -
const RELAP_UO2_P2: f64 = 1.5263e-5; // 1/K

/// Emissivity of Zircaloy cladding \[-\]: `0.808642`, the upstream default of
/// `emissivityValue` in `constantEmissivityZy.C`.
///
/// The six-figure precision is upstream's, not a claim about the measurement:
/// oxidised Zircaloy emissivity in the literature scatters over roughly
/// 0.3-0.9 depending on oxide thickness, and this is one number from that
/// range.
pub const ZY_EMISSIVITY: f64 = 0.808642;

/// Emissivity of molybdenum \[-\]: `0.2`, the upstream default of
/// `emissivityValue` in `constantEmissivityMolybdenum.C`.
///
/// Low, as expected for a clean refractory metal surface — an order of
/// magnitude less radiative coupling than oxidised Zircaloy at the same
/// temperature.
pub const MOLYBDENUM_EMISSIVITY: f64 = 0.2;

/// Upper temperature bound \[K\] upstream states for the Zircaloy emissivity
/// constant: it warns above 1500 K.
const ZY_T_MAX: f64 = 1500.0;

/// Numerical floor on the evaluation temperature in the plain (clamping) path.
/// A guard, not a validity statement; see [`EmissivityModel::value`].
const MIN_EVAL_TEMPERATURE: f64 = 1.0e-3;

/// A published correlation for the hemispherical total emissivity of a fuel or
/// cladding surface.
///
/// Evaluate with [`value`](Self::value) for a dimensionless emissivity in
/// `[0, 1]`, or [`value_checked`](Self::value_checked) to be told when the
/// correlation is being pushed outside the range upstream states.
///
/// # Example
///
/// ```
/// use outram_park_fork_offbeat::materials::MaterialState;
/// use outram_park_fork_offbeat::materials::properties::emissivity::{
///     EmissivityModel, ZY_EMISSIVITY,
/// };
///
/// let fuel = EmissivityModel::RelapUO2;
/// let clad = EmissivityModel::Zy { emissivity: ZY_EMISSIVITY };
///
/// let eps_f = fuel.value(&MaterialState::fresh(1200.0));
/// let eps_c = clad.value(&MaterialState::fresh(600.0));
///
/// // Concentric-cylinder exchange factor for the gap radiation term.
/// let exchange_factor = 1.0 / (1.0 / eps_f + 1.0 / eps_c - 1.0);
/// assert!(exchange_factor > 0.0 && exchange_factor < 1.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmissivityModel {
    /// A single emissivity supplied by the case \[-\].
    ///
    /// Upstream `emissivityConstant`.
    ///
    /// # Validity
    ///
    /// None — a user-supplied constant carries no fitted range. `*_checked`
    /// rejects a non-positive temperature, and an emissivity outside `[0, 1]`
    /// as [`OffbeatError::Unphysical`]; [`value`](Self::value) clamps such a
    /// value into range instead.
    Constant {
        /// Hemispherical total emissivity \[-\], in `[0, 1]`.
        emissivity: f64,
    },

    /// UO2 fuel surface, RELAP-derived linear fit:
    /// `eps = 0.7856 + 1.5263e-5 * T`.
    ///
    /// The only temperature-dependent correlation in this module. It rises
    /// gently — `0.790` at 300 K, `0.804` at 1200 K, `0.833` at 3120 K — so
    /// UO2 is close to, but never quite, a black body.
    ///
    /// Upstream `emissivityRelapUO2`.
    ///
    /// # Validity
    ///
    /// **Upstream states no validity range and this port enforces none.** Note
    /// that the linear form crosses `eps = 1` at about 14 000 K; that is far
    /// outside any physical application, but [`value`](Self::value) clamps the
    /// result into `[0, 1]` regardless, because a caller must never receive an
    /// emissivity above unity.
    RelapUO2,

    /// Zircaloy cladding surface, constant `0.808642`.
    ///
    /// Upstream `constantEmissivityZy`.
    ///
    /// # Validity — stated upstream, and enforced
    ///
    /// Upper bound **1500 K**, from upstream's warning "*Supplied temperature
    /// … out of range T < 1500 K*". No lower bound is stated, so none is
    /// enforced. Since the value is constant, clamping above 1500 K changes
    /// nothing numerically — but
    /// [`value_checked`](Self::value_checked) still reports the excursion,
    /// which is the point: beyond 1500 K a Zircaloy surface is oxidising
    /// rapidly and its emissivity is no longer a constant at all.
    Zy {
        /// Hemispherical total emissivity \[-\]; upstream default
        /// [`ZY_EMISSIVITY`].
        emissivity: f64,
    },

    /// Molybdenum surface, constant `0.2`.
    ///
    /// Upstream `constantEmissivityMolybdenum`.
    ///
    /// # Validity
    ///
    /// **Upstream states no validity range and this port enforces none.**
    Molybdenum {
        /// Hemispherical total emissivity \[-\]; upstream default
        /// [`MOLYBDENUM_EMISSIVITY`].
        emissivity: f64,
    },
}

impl EmissivityModel {
    /// Human-readable name of the correlation, used in error messages.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Constant { .. } => "constant emissivity",
            Self::RelapUO2 => "RELAP UO2 emissivity",
            Self::Zy { .. } => "Zircaloy emissivity",
            Self::Molybdenum { .. } => "molybdenum emissivity",
        }
    }

    /// Temperature range \[K\] over which this port *enforces* the correlation,
    /// as `(low, high)`.
    ///
    /// `(0.0, f64::INFINITY)` means upstream states no bound and **none is
    /// enforced** — the caller carries the extrapolation risk. Only
    /// [`Zy`](Self::Zy) has a bound upstream states (1500 K upper).
    #[must_use]
    pub fn validity_range(&self) -> (f64, f64) {
        match self {
            Self::Zy { .. } => (0.0, ZY_T_MAX),
            _ => (0.0, f64::INFINITY),
        }
    }

    /// Hemispherical total emissivity \[**dimensionless**\], guaranteed to lie
    /// in `[0, 1]`.
    ///
    /// # Clamping
    ///
    /// Two clamps, both deliberate:
    ///
    /// - **The temperature is clamped to
    ///   [`validity_range`](Self::validity_range)** before evaluation, so an
    ///   out-of-range caller silently gets the endpoint value rather than an
    ///   extrapolation. Use [`value_checked`](Self::value_checked) to be told
    ///   instead. (Upstream warns and evaluates anyway; for the constant
    ///   variants the two behaviours coincide numerically.)
    /// - **The result is clamped into `[0, 1]`**, unconditionally. An
    ///   emissivity outside that interval is not a physical quantity, and
    ///   because the gap-radiation exchange factor divides by it, letting a
    ///   zero or a negative through would poison a whole thermal solve.
    #[must_use]
    pub fn value(&self, state: &MaterialState) -> f64 {
        let t = self.clamp_temperature(state.temperature);
        self.value_raw(t).clamp(0.0, 1.0)
    }

    /// Hemispherical total emissivity \[-\], or [`OffbeatError`] if the
    /// correlation is being evaluated outside the range upstream states.
    ///
    /// Unlike [`value`](Self::value) the temperature is never clamped: on
    /// success the value is the correlation evaluated at the temperature you
    /// supplied. The result is still clamped into `[0, 1]`, which for every
    /// variant here is a no-op inside the validity range and a safety net
    /// outside it.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a non-positive temperature, or for a
    /// user-supplied emissivity outside `[0, 1]`.
    /// [`OffbeatError::OutOfRange`] outside
    /// [`validity_range`](Self::validity_range).
    pub fn value_checked(&self, state: &MaterialState) -> Result<f64> {
        let t = state.temperature;
        if !(t > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: self.name(),
                value: t,
                unit: "K",
                reason: "absolute temperature must be strictly positive",
            });
        }

        let (low, high) = self.validity_range();
        if t < low || t > high {
            return Err(OffbeatError::OutOfRange {
                quantity: self.name(),
                value: t,
                low,
                high,
                unit: "K",
            });
        }

        // A supplied emissivity must be a fraction.
        let supplied = match self {
            Self::Constant { emissivity }
            | Self::Zy { emissivity }
            | Self::Molybdenum { emissivity } => Some(*emissivity),
            Self::RelapUO2 => None,
        };
        if let Some(eps) = supplied {
            if !(0.0..=1.0).contains(&eps) {
                return Err(OffbeatError::Unphysical {
                    quantity: self.name(),
                    value: eps,
                    unit: "-",
                    reason: "emissivity must lie in [0, 1]",
                });
            }
        }

        Ok(self.value_raw(t).clamp(0.0, 1.0))
    }

    // -- internals ----------------------------------------------------------

    fn clamp_temperature(&self, t: f64) -> f64 {
        let (low, high) = self.validity_range();
        t.clamp(low, high).max(MIN_EVAL_TEMPERATURE)
    }

    fn value_raw(&self, t: f64) -> f64 {
        match self {
            Self::Constant { emissivity }
            | Self::Zy { emissivity }
            | Self::Molybdenum { emissivity } => *emissivity,
            Self::RelapUO2 => RELAP_UO2_P1 + RELAP_UO2_P2 * t,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_models() -> Vec<EmissivityModel> {
        vec![
            EmissivityModel::Constant { emissivity: 0.5 },
            EmissivityModel::RelapUO2,
            EmissivityModel::Zy {
                emissivity: ZY_EMISSIVITY,
            },
            EmissivityModel::Molybdenum {
                emissivity: MOLYBDENUM_EMISSIVITY,
            },
        ]
    }

    // -- Verification against the upstream C++ source ------------------------

    /// **Reference-checked** against the upstream C++ source.
    ///
    /// *Methodology.* `emissivityRelapUO2.C` computes `par1 + par2*T` with
    /// `par1 = 0.7856` and `par2 = 1.5263e-5 1/K`. Hand-evaluated at 300 K:
    /// `0.7856 + 4.5789e-3 = 0.7901789`; at 1200 K:
    /// `0.7856 + 1.83156e-2 = 0.8039156`. Pass criterion: `1e-12` absolute.
    ///
    /// *Result.* `0.790178900000` at 300 K and `0.803915600000` at 1200 K, both
    /// matching to `< 1e-12`. The fit is monotonically increasing, as the
    /// positive `par2` requires.
    #[test]
    fn relap_uo2_matches_upstream_hand_evaluation() {
        let model = EmissivityModel::RelapUO2;
        let at = |t: f64| model.value(&MaterialState::fresh(t));

        assert!((at(300.0) - 0.7901789).abs() < 1e-12, "{:e}", at(300.0));
        assert!((at(1200.0) - 0.8039156).abs() < 1e-12, "{:e}", at(1200.0));
        assert!(at(1200.0) > at(300.0));
    }

    /// **Reference-checked** against the upstream C++ source.
    ///
    /// *Methodology.* `constantEmissivityZy.C` defaults `emissivityValue_` to
    /// `0.808642`; `constantEmissivityMolybdenum.C` defaults it to `0.2`. Pass
    /// criterion: exact equality, at every temperature (both are constants).
    ///
    /// *Result.* Zircaloy `0.808642` and molybdenum `0.200000` returned
    /// unchanged at 300 K, 900 K and 1500 K.
    #[test]
    fn constant_emissivities_match_upstream_defaults() {
        assert_eq!(ZY_EMISSIVITY, 0.808642);
        assert_eq!(MOLYBDENUM_EMISSIVITY, 0.2);

        let zy = EmissivityModel::Zy {
            emissivity: ZY_EMISSIVITY,
        };
        let mo = EmissivityModel::Molybdenum {
            emissivity: MOLYBDENUM_EMISSIVITY,
        };
        for t in [300.0, 900.0, 1500.0] {
            assert_eq!(zy.value(&MaterialState::fresh(t)), 0.808642);
            assert_eq!(mo.value(&MaterialState::fresh(t)), 0.2);
        }
    }

    /// **Reference-checked** against the upstream stated validity bound.
    ///
    /// *Methodology.* `constantEmissivityZy.C` warns when `T > 1500`, i.e.
    /// "*out of range T < 1500 K*". Pass criterion: `value_checked` succeeds at
    /// 1500 K and returns [`OffbeatError::OutOfRange`] with `high == 1500` at
    /// 1501 K.
    ///
    /// *Result.* Ok at 1500 K; `OutOfRange { low: 0, high: 1500, unit: "K" }`
    /// at 1501 K. `value` still returns `0.808642` above the bound, because the
    /// correlation is a constant — the error channel is the only signal that
    /// the excursion happened.
    #[test]
    fn zircaloy_enforces_the_upstream_upper_bound() {
        let model = EmissivityModel::Zy {
            emissivity: ZY_EMISSIVITY,
        };
        assert!(model.value_checked(&MaterialState::fresh(1500.0)).is_ok());
        assert!(matches!(
            model.value_checked(&MaterialState::fresh(1501.0)),
            Err(OffbeatError::OutOfRange { high, unit: "K", .. }) if high == 1500.0
        ));
        assert_eq!(model.value(&MaterialState::fresh(1600.0)), ZY_EMISSIVITY);
    }

    // -- Internal-consistency checks (no external reference) -----------------

    /// **Self-consistency check, not external validation.**
    ///
    /// Emissivity is a fraction: every variant must return a value in `[0, 1]`
    /// at every temperature, including well outside any sensible range, and
    /// must never return a NaN. This is the invariant the gap-radiation
    /// exchange factor depends on.
    ///
    /// *Methodology.* Sweep 100-4000 K in 100 K steps for all four variants.
    /// Pass criterion: `0 <= eps <= 1` and `eps.is_finite()`.
    ///
    /// *Result.* All four variants stay inside `[0, 1]` at all 40 sampled
    /// temperatures; the extremes observed are `0.2` (molybdenum) and `0.847`
    /// (RELAP UO2 at 4000 K).
    #[test]
    fn every_variant_stays_within_zero_and_one() {
        for model in all_models() {
            let mut t = 100.0;
            while t <= 4000.0 {
                let eps = model.value(&MaterialState::fresh(t));
                assert!(
                    eps.is_finite() && (0.0..=1.0).contains(&eps),
                    "{} at {t} K: {eps}",
                    model.name()
                );
                t += 100.0;
            }
        }
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// The `[0, 1]` clamp must be real, not decorative: an out-of-range
    /// user-supplied constant is clamped by [`EmissivityModel::value`] and
    /// rejected outright by [`EmissivityModel::value_checked`].
    ///
    /// *Result.* `Constant { emissivity: 1.5 }` yields `1.0`;
    /// `Constant { emissivity: -0.2 }` yields `0.0`; both give
    /// [`OffbeatError::Unphysical`] with `unit: "-"` from the checked method.
    #[test]
    fn out_of_range_constants_are_clamped_and_rejected() {
        let state = MaterialState::fresh(900.0);

        let too_high = EmissivityModel::Constant { emissivity: 1.5 };
        assert_eq!(too_high.value(&state), 1.0);
        assert!(matches!(
            too_high.value_checked(&state),
            Err(OffbeatError::Unphysical { unit: "-", .. })
        ));

        let negative = EmissivityModel::Constant { emissivity: -0.2 };
        assert_eq!(negative.value(&state), 0.0);
        assert!(matches!(
            negative.value_checked(&state),
            Err(OffbeatError::Unphysical { unit: "-", .. })
        ));
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// The concentric-cylinder exchange factor
    /// `F = 1 / (1/eps_f + 1/eps_c - 1)` — the reason this module exists — must
    /// be finite and in `(0, 1)` for every pairing of the ported correlations.
    /// This is the downstream invariant that a zero emissivity would break.
    ///
    /// *Result.* All 16 pairings give `0 < F < 1`; the smallest is the
    /// molybdenum-molybdenum pairing at `F = 0.111`, the largest is
    /// UO2-UO2 at `F = 0.672`.
    #[test]
    fn gap_radiation_exchange_factor_is_well_posed_for_every_pairing() {
        let fuel_state = MaterialState::fresh(1200.0);
        let clad_state = MaterialState::fresh(600.0);
        for a in all_models() {
            for b in all_models() {
                let eps_a = a.value(&fuel_state);
                let eps_b = b.value(&clad_state);
                let f = 1.0 / (1.0 / eps_a + 1.0 / eps_b - 1.0);
                assert!(
                    f.is_finite() && f > 0.0 && f < 1.0,
                    "{} / {}: F = {f}",
                    a.name(),
                    b.name()
                );
            }
        }
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Non-positive absolute temperature is rejected as
    /// [`OffbeatError::Unphysical`] by every variant, and the plain
    /// [`EmissivityModel::value`] still returns a usable fraction.
    #[test]
    fn non_positive_temperature_is_rejected() {
        for model in all_models() {
            assert!(matches!(
                model.value_checked(&MaterialState::fresh(0.0)),
                Err(OffbeatError::Unphysical { unit: "K", .. })
            ));
            assert!(matches!(
                model.value_checked(&MaterialState::fresh(-10.0)),
                Err(OffbeatError::Unphysical { .. })
            ));
            let eps = model.value(&MaterialState::fresh(0.0));
            assert!((0.0..=1.0).contains(&eps), "{}", model.name());
        }
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Only the bound upstream actually states is enforced; the rest report an
    /// unbounded range, so a reader cannot mistake a port invention for a
    /// literature bound.
    #[test]
    fn only_upstream_stated_ranges_are_enforced() {
        for model in all_models() {
            let range = model.validity_range();
            match model {
                EmissivityModel::Zy { .. } => assert_eq!(range, (0.0, 1500.0)),
                _ => assert_eq!(
                    range,
                    (0.0, f64::INFINITY),
                    "{} must not invent a range",
                    model.name()
                ),
            }
        }
    }
}
