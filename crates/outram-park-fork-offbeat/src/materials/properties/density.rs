// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
// `offbeatLib/materials/materialModel/thermoMechanicalPropertiesModels/`
// `density/`, specifically:
//   densityModel.{H,C}                (base class)
//   densityConstant.{H,C}
//   constantDensityUO2.{H,C}
//   constantDensityUPuO2.{H,C}
//   constantDensityMolybdenum.{H,C}
//   densityIAEAZy.{H,C}
//   densitySchumann1515Ti.{H,C}
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Density correlations \[kg/m^3\].
//!
//! # Porosity is the thing to get right here
//!
//! "Density" means two different numbers in fuel-performance work, and mixing
//! them up misstates the fuel mass in a rod by five per cent:
//!
//! - the **theoretical density** of the fully dense crystal (10 960 kg/m^3 for
//!   UO2), and
//! - the **smeared density** of the as-fabricated pellet, which contains a few
//!   per cent of fabrication porosity and is therefore lower.
//!
//! Every variant in this module states explicitly, in its own doc comment,
//! whether the number it returns **already includes** the porosity correction
//! or whether it is a fully dense value. The rule for this port:
//!
//! - [`UO2`](DensityModel::UO2) and [`UPuO2`](DensityModel::UPuO2) apply
//!   [`MaterialState::density_fraction`] internally. **Do not apply it again.**
//! - [`Constant`](DensityModel::Constant),
//!   [`Molybdenum`](DensityModel::Molybdenum),
//!   [`IAEAZy`](DensityModel::IAEAZy) and
//!   [`Schumann1515Ti`](DensityModel::Schumann1515Ti) return the density of the
//!   dense material and ignore [`porosity`](MaterialState::porosity) entirely —
//!   which is right for cladding and structure, and is a caller responsibility
//!   if one of them is ever pointed at a porous body.
//!
//! # Temperature dependence
//!
//! Only two of the six vary with temperature. That is faithful to upstream:
//! OFFBEAT holds the *fuel* density fixed and carries fuel volume change
//! through the strain field (thermal expansion, swelling, densification) rather
//! than through `rho`, so making the fuel density temperature-dependent as well
//! would double-count. [`IAEAZy`](DensityModel::IAEAZy) and
//! [`Schumann1515Ti`](DensityModel::Schumann1515Ti) are cladding correlations
//! published directly as `rho(T)` and are ported as such.
//!
//! # Units
//!
//! Raw `f64` in strict SI: temperature in kelvin, density in kg/m^3.

use crate::error::{OffbeatError, Result};
use crate::materials::MaterialState;

/// Theoretical (fully dense, pore-free) density of UO2 at room temperature
/// \[kg/m^3\]: `10960.0`.
///
/// Upstream default of `theoreticalDensity` in `constantDensityUO2.C`.
pub const UO2_THEORETICAL_DENSITY: f64 = 10960.0;

/// Upstream default fraction of theoretical density for UO2 pellets \[-\]:
/// `0.95`.
///
/// Provided for reference only — this port takes the density fraction from
/// [`MaterialState::density_fraction`] (i.e. from
/// [`porosity`](MaterialState::porosity)) so that fabrication porosity and its
/// in-service evolution are described in one place. A caller reproducing an
/// upstream case with the default should set `porosity = 0.05`.
pub const UO2_DEFAULT_DENSITY_FRACTION: f64 = 0.95;

/// Theoretical (fully dense, pore-free) density of (U,Pu)O2 MOX at room
/// temperature \[kg/m^3\]: `10430.0`.
///
/// Upstream default of `theoreticalDensity` in `constantDensityUPuO2.C`. Lower
/// than UO2 because PuO2 is lighter per unit cell than UO2.
pub const UPUO2_THEORETICAL_DENSITY: f64 = 10430.0;

/// Upstream default fraction of theoretical density for MOX pellets \[-\]:
/// `0.945`. See [`UO2_DEFAULT_DENSITY_FRACTION`] for how this port uses it.
pub const UPUO2_DEFAULT_DENSITY_FRACTION: f64 = 0.945;

/// Density of molybdenum \[kg/m^3\]: `10280.0`, the upstream default of
/// `densityValue` in `constantDensityMolybdenum.C`.
pub const MOLYBDENUM_DENSITY: f64 = 10280.0;

/// Reference density of 15-15Ti austenitic stainless steel at 20 °C
/// \[kg/m^3\]: `7900.0`, the upstream default `rho0_` in
/// `densitySchumann1515Ti.C`.
pub const SCHUMANN_1515TI_REFERENCE_DENSITY: f64 = 7900.0;

// -- IAEA Zircaloy (densityIAEAZy.C) ----------------------------------------
const IAEA_ZY_ALPHA_A: f64 = 6595.2; // kg/m^3
const IAEA_ZY_ALPHA_B: f64 = 0.1477; // kg/(m^3 K)
const IAEA_ZY_BETA_A: f64 = 6690.0; // kg/m^3
const IAEA_ZY_BETA_B: f64 = 0.1855; // kg/(m^3 K)
const IAEA_ZY_T_ALPHA: f64 = 1083.0; // K, end of the alpha phase
const IAEA_ZY_T_BETA: f64 = 1144.0; // K, start of the beta phase
const IAEA_ZY_T_MAX: f64 = 1800.0; // K, upstream's stated upper bound

// -- Schumann 15-15Ti (densitySchumann1515Ti.C) -----------------------------
// The linear-expansion polynomial in degrees Celsius. These are numerically the
// same three coefficients as the Gehr (1973) thermal-expansion correlation in
// `super::thermal_expansion::ThermalExpansionModel::Gehr1515Ti`; they are
// repeated here rather than imported so this file reads standalone, and the
// agreement is pinned by a test.
const SCHUMANN_1515TI_EXPANSION: [f64; 3] = [-3.101e-4, 1.545e-5, 2.75e-9];

/// Numerical floor on the evaluation temperature in the plain (clamping) path.
/// A guard, not a validity statement; see [`DensityModel::value`].
const MIN_EVAL_TEMPERATURE: f64 = 1.0e-3;

/// A published correlation for the mass density of a fuel, cladding or
/// structural material.
///
/// Evaluate with [`value`](Self::value) for kg/m^3, or
/// [`value_checked`](Self::value_checked) to be told when the correlation is
/// being pushed outside the range it was fitted over.
///
/// Read the [module documentation](self) on porosity before choosing a variant:
/// two of the six apply [`MaterialState::density_fraction`] internally and four
/// do not.
///
/// # Example
///
/// ```
/// use outram_park_fork_offbeat::materials::MaterialState;
/// use outram_park_fork_offbeat::materials::properties::density::{
///     DensityModel, UO2_THEORETICAL_DENSITY,
/// };
///
/// // A 95 %-dense UO2 pellet: 5 % fabrication porosity.
/// let model = DensityModel::UO2 {
///     theoretical_density: UO2_THEORETICAL_DENSITY,
/// };
/// let mut state = MaterialState::fresh(900.0);
/// state.porosity = 0.05;
///
/// // The porosity is already in the answer — do not multiply by 0.95 again.
/// assert!((model.value(&state) - 0.95 * 10960.0).abs() < 1e-9);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DensityModel {
    /// A single density supplied by the case \[kg/m^3\].
    ///
    /// Upstream `densityConstant`.
    ///
    /// # Porosity
    ///
    /// **Not applied.** The number is returned exactly as given, so whatever
    /// porosity correction is wanted must already be baked into it.
    ///
    /// # Validity
    ///
    /// None — a user-supplied constant carries no fitted range. `*_checked`
    /// only rejects a non-positive temperature or a non-positive density.
    Constant {
        /// Density \[kg/m^3\]. Must be strictly positive.
        density: f64,
    },

    /// UO2 fuel: `rho = density_fraction * theoretical_density`.
    ///
    /// Upstream `constantDensityUO2`, which multiplies a dictionary
    /// `densityFraction` (default 0.95) by `theoreticalDensity` (default
    /// 10 960 kg/m^3). This port takes the fraction from the material state
    /// instead, so that as-fabricated porosity and its evolution live in one
    /// place.
    ///
    /// # Porosity
    ///
    /// **Applied internally**, via [`MaterialState::density_fraction`] — which
    /// is `1 - porosity`, floored at 0.05. Do not multiply by it again.
    ///
    /// # Temperature
    ///
    /// Independent of temperature by design: fuel volume change is carried by
    /// the strain field (see the [module documentation](self)), not by `rho`.
    ///
    /// # Validity
    ///
    /// **Upstream states no validity range and this port enforces none.**
    UO2 {
        /// Fully dense, pore-free density \[kg/m^3\]; upstream default
        /// [`UO2_THEORETICAL_DENSITY`].
        theoretical_density: f64,
    },

    /// (U,Pu)O2 MOX fuel: `rho = density_fraction * theoretical_density`.
    ///
    /// Upstream `constantDensityUPuO2`, which is explicit about the intent: it
    /// looks up the live `porosity` field where one exists so that pore
    /// migration and central-void formation change the local density, falling
    /// back to `1 - densityFraction` (default fraction 0.945) otherwise. In
    /// this port [`MaterialState::porosity`] *is* the live value, so the two
    /// paths collapse into one.
    ///
    /// # Porosity
    ///
    /// **Applied internally**, via [`MaterialState::density_fraction`]. Do not
    /// multiply by it again.
    ///
    /// # Validity
    ///
    /// **Upstream states no validity range and this port enforces none.**
    UPuO2 {
        /// Fully dense, pore-free density \[kg/m^3\]; upstream default
        /// [`UPUO2_THEORETICAL_DENSITY`].
        theoretical_density: f64,
    },

    /// Molybdenum structural material, constant `10 280 kg/m^3`.
    ///
    /// Upstream `constantDensityMolybdenum`.
    ///
    /// # Porosity
    ///
    /// **Not applied** — the value is that of the dense metal.
    ///
    /// # Validity
    ///
    /// **Upstream states no validity range and this port enforces none.**
    Molybdenum {
        /// Density \[kg/m^3\]; upstream default [`MOLYBDENUM_DENSITY`].
        density: f64,
    },

    /// Zircaloy cladding, IAEA correlation, with the alpha → beta phase
    /// transition.
    ///
    /// Two linear branches and a blend across the transformation:
    ///
    /// - `T < 1083 K` (alpha): `rho = 6595.2 - 0.1477*T`
    /// - `1083 <= T < 1144 K`: linear blend between the two branches
    /// - `1144 <= T <= 1800 K` (beta): `rho = 6690.0 - 0.1855*T`
    ///
    /// The density **rises** across the blend (about `6435` to `6478 kg/m^3`)
    /// because the hexagonal-to-cubic transformation contracts the metal. That
    /// is the same physics as the negative expansion coefficient reported by
    /// [`ThermalExpansionModel::MatproZy`](super::thermal_expansion::ThermalExpansionModel::MatproZy)
    /// over its own 1073-1273 K transition window, and the two are
    /// cross-checked against each other in this module's tests.
    ///
    /// Upstream `densityIAEAZy`.
    ///
    /// # Porosity
    ///
    /// **Not applied** — cladding is treated as fully dense.
    ///
    /// # Validity — stated upstream, and enforced
    ///
    /// Upper bound **1800 K**, from upstream's warning "*Supplied temperature
    /// … above maximum of 1800 K*". No lower bound is stated, so none is
    /// enforced.
    ///
    /// # Deviation from upstream
    ///
    /// Above 1800 K upstream warns and then returns a density of **zero**,
    /// which is not a density and would divide by zero in any mass or
    /// heat-capacity term downstream. This port clamps to the 1800 K value in
    /// [`value`](Self::value) and returns [`OffbeatError::OutOfRange`] from
    /// [`value_checked`](Self::value_checked) instead.
    IAEAZy,

    /// 15-15Ti austenitic stainless cladding, Schumann (1970).
    ///
    /// `rho(T) = rho_0 / (1 + eps(T))^3` with the linear thermal strain
    /// `eps(T) = -3.101e-4 + 1.545e-5*T_C + 2.75e-9*T_C^2`, `T_C = T - 273.15`,
    /// and `rho_0 = 7900 kg/m^3` at 20 °C. The cube converts the linear strain
    /// to a volumetric one, so this is mass conservation applied to the Gehr
    /// thermal-expansion fit and nothing more — the same three coefficients
    /// appear in
    /// [`ThermalExpansionModel::Gehr1515Ti`](super::thermal_expansion::ThermalExpansionModel::Gehr1515Ti).
    ///
    /// Upstream `densitySchumann1515Ti`.
    ///
    /// # Porosity
    ///
    /// **Not applied** — cladding is treated as fully dense.
    ///
    /// # Validity
    ///
    /// **Upstream states no validity range and this port enforces none.** Note
    /// that unlike the Gehr expansion model this correlation has no 293 K
    /// cut-off: below 20 °C it returns a density slightly above `rho_0`, which
    /// is the physically correct direction.
    Schumann1515Ti {
        /// Density at 20 °C \[kg/m^3\]; upstream default
        /// [`SCHUMANN_1515TI_REFERENCE_DENSITY`].
        reference_density: f64,
    },
}

impl DensityModel {
    /// Human-readable name of the correlation, used in error messages.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Constant { .. } => "constant density",
            Self::UO2 { .. } => "UO2 density",
            Self::UPuO2 { .. } => "(U,Pu)O2 density",
            Self::Molybdenum { .. } => "molybdenum density",
            Self::IAEAZy => "IAEA Zircaloy density",
            Self::Schumann1515Ti { .. } => "Schumann 15-15Ti density",
        }
    }

    /// Temperature range \[K\] over which this port *enforces* the correlation,
    /// as `(low, high)`.
    ///
    /// `(0.0, f64::INFINITY)` means upstream states no bound and **none is
    /// enforced** — the caller carries the extrapolation risk. Only
    /// [`IAEAZy`](Self::IAEAZy) has a bound upstream states (1800 K upper).
    #[must_use]
    pub fn validity_range(&self) -> (f64, f64) {
        match self {
            Self::IAEAZy => (0.0, IAEA_ZY_T_MAX),
            _ => (0.0, f64::INFINITY),
        }
    }

    /// Mass density \[**kg/m^3**\].
    ///
    /// Whether the returned value already accounts for porosity depends on the
    /// variant — see the [module documentation](self) and the variant's own doc
    /// comment. [`UO2`](Self::UO2) and [`UPuO2`](Self::UPuO2) apply
    /// [`MaterialState::density_fraction`] internally; the other four do not.
    ///
    /// # Clamping
    ///
    /// **The temperature is clamped to
    /// [`validity_range`](Self::validity_range) before evaluation**, so an
    /// out-of-range caller silently gets the endpoint density rather than an
    /// extrapolation (or, for [`IAEAZy`](Self::IAEAZy) above 1800 K, rather
    /// than upstream's zero). Use [`value_checked`](Self::value_checked) to be
    /// told instead.
    #[must_use]
    pub fn value(&self, state: &MaterialState) -> f64 {
        let t = self.clamp_temperature(state.temperature);
        self.value_raw(t, state)
    }

    /// Mass density \[kg/m^3\], or [`OffbeatError`] if the correlation is being
    /// evaluated outside the range it was fitted over.
    ///
    /// Unlike [`value`](Self::value) this never clamps: on success the value is
    /// the correlation evaluated at the temperature you supplied.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a non-positive temperature, or for a
    /// non-positive user-supplied density.
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

        // A supplied density must actually be a density.
        let supplied = match self {
            Self::Constant { density } | Self::Molybdenum { density } => Some(*density),
            Self::UO2 {
                theoretical_density,
            }
            | Self::UPuO2 {
                theoretical_density,
            } => Some(*theoretical_density),
            Self::Schumann1515Ti { reference_density } => Some(*reference_density),
            Self::IAEAZy => None,
        };
        if let Some(rho) = supplied {
            if !(rho > 0.0) {
                return Err(OffbeatError::Unphysical {
                    quantity: self.name(),
                    value: rho,
                    unit: "kg/m^3",
                    reason: "density must be strictly positive",
                });
            }
        }

        Ok(self.value_raw(t, state))
    }

    // -- internals ----------------------------------------------------------

    fn clamp_temperature(&self, t: f64) -> f64 {
        let (low, high) = self.validity_range();
        t.clamp(low, high).max(MIN_EVAL_TEMPERATURE)
    }

    fn value_raw(&self, t: f64, state: &MaterialState) -> f64 {
        match self {
            Self::Constant { density } | Self::Molybdenum { density } => *density,

            Self::UO2 {
                theoretical_density,
            }
            | Self::UPuO2 {
                theoretical_density,
            } => state.density_fraction() * theoretical_density,

            Self::IAEAZy => iaea_zy(t),

            Self::Schumann1515Ti { reference_density } => {
                let strain = schumann_linear_strain(t);
                reference_density / (1.0 + strain).powi(3)
            }
        }
    }
}

/// IAEA Zircaloy density \[kg/m^3\] with the alpha → beta blend.
fn iaea_zy(t: f64) -> f64 {
    let rho_alpha = IAEA_ZY_ALPHA_A - IAEA_ZY_ALPHA_B * t;
    let rho_beta = IAEA_ZY_BETA_A - IAEA_ZY_BETA_B * t;
    if t < IAEA_ZY_T_ALPHA {
        rho_alpha
    } else if t < IAEA_ZY_T_BETA {
        let w = (t - IAEA_ZY_T_ALPHA) / (IAEA_ZY_T_BETA - IAEA_ZY_T_ALPHA);
        rho_alpha + (rho_beta - rho_alpha) * w
    } else {
        rho_beta
    }
}

/// Linear thermal strain \[-\] of 15-15Ti used by the Schumann density fit.
fn schumann_linear_strain(t: f64) -> f64 {
    let tc = t - 273.15;
    SCHUMANN_1515TI_EXPANSION[0]
        + SCHUMANN_1515TI_EXPANSION[1] * tc
        + SCHUMANN_1515TI_EXPANSION[2] * tc * tc
}

#[cfg(test)]
mod tests {
    use super::super::thermal_expansion::ThermalExpansionModel;
    use super::*;

    fn all_models() -> Vec<DensityModel> {
        vec![
            DensityModel::Constant { density: 8000.0 },
            DensityModel::UO2 {
                theoretical_density: UO2_THEORETICAL_DENSITY,
            },
            DensityModel::UPuO2 {
                theoretical_density: UPUO2_THEORETICAL_DENSITY,
            },
            DensityModel::Molybdenum {
                density: MOLYBDENUM_DENSITY,
            },
            DensityModel::IAEAZy,
            DensityModel::Schumann1515Ti {
                reference_density: SCHUMANN_1515TI_REFERENCE_DENSITY,
            },
        ]
    }

    // -- Verification against the upstream C++ source ------------------------

    /// **Reference-checked** against the upstream C++ source.
    ///
    /// *Methodology.* `constantDensityUO2.C` returns
    /// `densityFrac_ * theoreticalDensity_` with defaults `0.95` and
    /// `10960.0 kg/m^3`; `constantDensityUPuO2.C` returns
    /// `(1 - porosity) * theoreticalDensity_` with a default fraction `0.945`
    /// and `10430.0 kg/m^3`; `constantDensityMolybdenum.C` returns a flat
    /// `10280 kg/m^3`. Reproduced here by setting
    /// [`MaterialState::porosity`] to `1 - fraction`. Pass criterion: `1e-9`
    /// absolute.
    ///
    /// *Result.* UO2 `10412.000000 kg/m^3` (= 0.95 x 10960);
    /// MOX `9856.350000 kg/m^3` (= 0.945 x 10430);
    /// Mo `10280.000000 kg/m^3`. All exact to `< 1e-9`.
    #[test]
    fn oxide_and_molybdenum_densities_match_upstream_defaults() {
        assert_eq!(UO2_THEORETICAL_DENSITY, 10960.0);
        assert_eq!(UPUO2_THEORETICAL_DENSITY, 10430.0);
        assert_eq!(MOLYBDENUM_DENSITY, 10280.0);

        let mut uo2_state = MaterialState::fresh(900.0);
        uo2_state.porosity = 1.0 - UO2_DEFAULT_DENSITY_FRACTION;
        let uo2 = DensityModel::UO2 {
            theoretical_density: UO2_THEORETICAL_DENSITY,
        };
        assert!((uo2.value(&uo2_state) - 10412.0).abs() < 1e-9);

        let mut mox_state = MaterialState::fresh(900.0);
        mox_state.porosity = 1.0 - UPUO2_DEFAULT_DENSITY_FRACTION;
        let mox = DensityModel::UPuO2 {
            theoretical_density: UPUO2_THEORETICAL_DENSITY,
        };
        assert!((mox.value(&mox_state) - 9856.35).abs() < 1e-9);

        let mo = DensityModel::Molybdenum {
            density: MOLYBDENUM_DENSITY,
        };
        assert_eq!(mo.value(&MaterialState::fresh(1500.0)), 10280.0);
    }

    /// **Reference-checked** against the upstream C++ source.
    ///
    /// *Methodology.* `densityIAEAZy.C` computes `rho1 = 6595.2 - 0.1477*T`
    /// below 1083 K and `rho2 = 6690.0 - 0.1855*T` above 1144 K, blending
    /// linearly between. Hand-evaluated at 300 K:
    /// `6595.2 - 44.31 = 6550.89 kg/m^3`; at 1500 K:
    /// `6690.0 - 278.25 = 6411.75 kg/m^3`. Pass criterion: `1e-9` absolute.
    ///
    /// Continuity of the blend is checked at both ends by comparing the blend
    /// endpoints against the branch expressions they must meet:
    /// `rho_alpha(1083) = 6435.2409` and `rho_beta(1144) = 6477.788 kg/m^3`.
    /// The blend spans 61 K and jumps `53.86 kg/m^3` at its lower end, so a
    /// `1e-6 K` offset around the branch point moves the density by `8.8e-7`
    /// — the continuity tolerance is therefore `1e-5 kg/m^3`, not `1e-9`.
    ///
    /// *Result.* `6550.890000 kg/m^3` at 300 K and `6411.750000 kg/m^3` at
    /// 1500 K, both matching to `< 1e-9`. The blend meets `rho_alpha` at
    /// 1083 K and `rho_beta` at 1144 K exactly (`< 1e-9`), and is continuous
    /// across both branch points to `< 1e-5`.
    #[test]
    fn iaea_zy_matches_upstream_hand_evaluation() {
        let model = DensityModel::IAEAZy;
        let at = |t: f64| model.value(&MaterialState::fresh(t));

        assert!((at(300.0) - 6550.89).abs() < 1e-9, "{:e}", at(300.0));
        assert!((at(1500.0) - 6411.75).abs() < 1e-9, "{:e}", at(1500.0));

        // The blend must start on the alpha branch and end on the beta branch.
        assert!((at(1083.0) - 6435.2409).abs() < 1e-9, "{:e}", at(1083.0));
        assert!((at(1144.0) - 6477.788).abs() < 1e-9, "{:e}", at(1144.0));

        // ... and be continuous across both branch points.
        assert!((at(1083.0 - 1e-6) - at(1083.0 + 1e-6)).abs() < 1e-5);
        assert!((at(1144.0 - 1e-6) - at(1144.0 + 1e-6)).abs() < 1e-5);
    }

    /// **Reference-checked** against the upstream stated validity bound.
    ///
    /// *Methodology.* `densityIAEAZy.C` warns "*above maximum of 1800 K*" and
    /// then returns **zero**. Pass criterion: `value_checked` succeeds at
    /// 1800 K and returns [`OffbeatError::OutOfRange`] with `high == 1800` at
    /// 1801 K; and — the deliberate deviation from upstream — `value` clamps to
    /// the 1800 K density rather than returning zero.
    ///
    /// *Result.* Ok at 1800 K (`6356.100000 kg/m^3`); `OutOfRange` at 1801 K;
    /// `value(3000 K) == value(1800 K) == 6356.1 kg/m^3`, never zero.
    #[test]
    fn iaea_zy_enforces_the_upstream_upper_bound_and_never_returns_zero() {
        let model = DensityModel::IAEAZy;
        assert!(model.value_checked(&MaterialState::fresh(1800.0)).is_ok());
        assert!(matches!(
            model.value_checked(&MaterialState::fresh(1801.0)),
            Err(OffbeatError::OutOfRange { high, unit: "K", .. }) if high == 1800.0
        ));

        let clamped = model.value(&MaterialState::fresh(3000.0));
        assert!((clamped - model.value(&MaterialState::fresh(1800.0))).abs() < 1e-12);
        assert!((clamped - 6356.1).abs() < 1e-9, "{clamped:e}");
        assert!(clamped > 6000.0, "must not be upstream's zero: {clamped}");
    }

    /// **Reference-checked** against the upstream C++ source.
    ///
    /// *Methodology.* `densitySchumann1515Ti.C` computes
    /// `rho0 * (1/(1 + eps))^3` with
    /// `eps = -3.101e-4 + 1.545e-5*T_C + 2.75e-9*T_C^2` and `rho0 = 7900`.
    /// Recomputed independently here at 973.15 K (`T_C = 700`). Pass criterion:
    /// `1e-9` relative.
    ///
    /// *Result.* `eps(700 C) = 1.185240e-2`, `rho = 7625.5 kg/m^3`, matching
    /// the independent evaluation to `< 1e-9` relative. At 20 °C the strain
    /// vanishes and the density returns `rho0` to within `0.001 kg/m^3`.
    #[test]
    fn schumann_1515ti_matches_upstream_hand_evaluation() {
        let model = DensityModel::Schumann1515Ti {
            reference_density: SCHUMANN_1515TI_REFERENCE_DENSITY,
        };
        let tc: f64 = 700.0;
        let eps: f64 = -3.101e-4 + 1.545e-5 * tc + 2.75e-9 * tc * tc;
        let expected = 7900.0 * (1.0 / (1.0 + eps)).powi(3);
        let got = model.value(&MaterialState::fresh(tc + 273.15));
        assert!((got - expected).abs() / expected < 1e-9, "{got:e}");
        assert!((got - 7625.5).abs() < 1.0, "{got:e}");

        // At the fit's own 20 C reference the density is rho0.
        let at_ref = model.value(&MaterialState::fresh(293.15));
        assert!((at_ref - 7900.0).abs() < 1e-3, "{at_ref:e}");
    }

    // -- Internal-consistency checks (no external reference) -----------------

    /// **Self-consistency check, not external validation.**
    ///
    /// The Schumann density fit and the Gehr thermal-expansion fit are the same
    /// physics written two ways: `rho(T) = rho_0 / (1 + eps(T))^3`. This
    /// asserts that identity against the independently ported
    /// [`ThermalExpansionModel::Gehr1515Ti`], which is the only cross-check in
    /// this module tying two separately transcribed coefficient sets together.
    ///
    /// *Methodology.* Sample 400-1300 K in 100 K steps (above the Gehr model's
    /// 293 K cut-off, where the two are directly comparable). Pass criterion:
    /// relative difference below `1e-12`.
    ///
    /// *Result.* Agreement to better than `1e-12` relative at every sampled
    /// temperature.
    #[test]
    fn schumann_density_is_the_gehr_expansion_fit_under_mass_conservation() {
        let density = DensityModel::Schumann1515Ti {
            reference_density: SCHUMANN_1515TI_REFERENCE_DENSITY,
        };
        let expansion = ThermalExpansionModel::Gehr1515Ti;

        let mut t = 400.0;
        while t <= 1300.0 {
            let state = MaterialState::fresh(t);
            let strain = expansion.strain(&state);
            let expected = SCHUMANN_1515TI_REFERENCE_DENSITY / (1.0 + strain).powi(3);
            let got = density.value(&state);
            assert!(
                (got - expected).abs() / expected < 1e-12,
                "at {t} K: {got:e} vs {expected:e}"
            );
            t += 100.0;
        }
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Density falls as a solid is heated, away from a phase transformation.
    /// Checked for the two temperature-dependent variants over intervals that
    /// exclude the Zircaloy alpha → beta window.
    ///
    /// *Result.* IAEA Zircaloy falls monotonically over 300-1000 K and over
    /// 1200-1800 K; Schumann 15-15Ti falls monotonically over 300-1300 K.
    #[test]
    fn density_falls_with_temperature_away_from_phase_transitions() {
        let zy = DensityModel::IAEAZy;
        for window in [(300.0, 1000.0), (1200.0, 1800.0)] {
            let mut t = window.0;
            while t + 50.0 <= window.1 {
                let cold = zy.value(&MaterialState::fresh(t));
                let hot = zy.value(&MaterialState::fresh(t + 50.0));
                assert!(hot < cold, "Zy at {t} K: {cold} -> {hot}");
                t += 50.0;
            }
        }

        let steel = DensityModel::Schumann1515Ti {
            reference_density: SCHUMANN_1515TI_REFERENCE_DENSITY,
        };
        let mut t = 300.0;
        while t + 50.0 <= 1300.0 {
            let cold = steel.value(&MaterialState::fresh(t));
            let hot = steel.value(&MaterialState::fresh(t + 50.0));
            assert!(hot < cold, "15-15Ti at {t} K: {cold} -> {hot}");
            t += 50.0;
        }
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Zircaloy's alpha → beta transformation is a contraction, so it must show
    /// up as a density *rise* in the IAEA fit and as a *negative* expansion
    /// coefficient in the MATPRO fit. Two independently ported correlations,
    /// from different sources, describing the same transformation — if they
    /// disagreed in sign, at least one transcription would be wrong.
    ///
    /// *Methodology.* Compare IAEA Zy density at 1083 K and 1144 K (the ends of
    /// its blend), and the MATPRO Zy expansion coefficient at 1150 K (inside
    /// its 1073-1273 K blend). Pass criterion: density rises, coefficient is
    /// negative.
    ///
    /// *Result.* Density rises from `6435.24` to `6477.78 kg/m^3` (+0.66 %),
    /// and the MATPRO expansion coefficient is `-1.0953e-5 1/K`. Note that the
    /// two correlations place the transformation over different temperature
    /// windows (1083-1144 K against 1073-1273 K), so the agreement is
    /// qualitative — this test asserts the sign, not the magnitude.
    #[test]
    fn zircaloy_phase_transformation_agrees_in_sign_across_two_correlations() {
        let zy = DensityModel::IAEAZy;
        let before = zy.value(&MaterialState::fresh(1083.0));
        let after = zy.value(&MaterialState::fresh(1144.0));
        assert!(after > before, "contraction expected: {before} -> {after}");

        let expansion = ThermalExpansionModel::MatproZy { t_ref: 300.0 };
        let alpha = expansion.coefficient(&MaterialState::fresh(1150.0));
        assert!(alpha < 0.0, "contraction expected, got alpha = {alpha:e}");
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Porosity lowers the density of the two fuel variants and leaves the
    /// other four untouched — the split the [module documentation](self)
    /// promises. Also pins [`MaterialState::density_fraction`]'s 0.05 floor
    /// showing through, so a nearly-void cell returns 5 % of theoretical rather
    /// than zero.
    #[test]
    fn porosity_is_applied_by_the_fuel_variants_only() {
        let dense = MaterialState::fresh(900.0);
        let mut porous = dense;
        porous.porosity = 0.10;

        for model in all_models() {
            let applies_porosity =
                matches!(model, DensityModel::UO2 { .. } | DensityModel::UPuO2 { .. });
            let a = model.value(&dense);
            let b = model.value(&porous);
            if applies_porosity {
                assert!((b - 0.9 * a).abs() < 1e-9, "{}: {a} -> {b}", model.name());
            } else {
                assert_eq!(a, b, "{} must ignore porosity", model.name());
            }
        }

        let mut nearly_void = dense;
        nearly_void.porosity = 0.999;
        let uo2 = DensityModel::UO2 {
            theoretical_density: UO2_THEORETICAL_DENSITY,
        };
        assert!((uo2.value(&nearly_void) - 0.05 * UO2_THEORETICAL_DENSITY).abs() < 1e-9);
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Every variant returns a finite, strictly positive density over
    /// 300-1800 K, inside the plausible band for oxide fuel, Zr alloys and
    /// steels (2000-12000 kg/m^3). A correlation leaving that band is broken,
    /// not merely inaccurate.
    #[test]
    fn every_variant_returns_a_plausible_positive_density() {
        for model in all_models() {
            let mut t = 300.0;
            while t <= 1800.0 {
                let rho = model.value(&MaterialState::fresh(t));
                assert!(
                    rho.is_finite() && (2000.0..=12000.0).contains(&rho),
                    "{} at {t} K: {rho}",
                    model.name()
                );
                t += 100.0;
            }
        }
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Non-positive temperature and non-positive supplied density are both
    /// rejected as [`OffbeatError::Unphysical`], and the plain
    /// [`DensityModel::value`] stays finite regardless.
    #[test]
    fn unphysical_inputs_are_rejected() {
        for model in all_models() {
            assert!(matches!(
                model.value_checked(&MaterialState::fresh(0.0)),
                Err(OffbeatError::Unphysical { unit: "K", .. })
            ));
            assert!(model.value(&MaterialState::fresh(0.0)).is_finite());
        }

        let negative = DensityModel::Constant { density: -1.0 };
        assert!(matches!(
            negative.value_checked(&MaterialState::fresh(900.0)),
            Err(OffbeatError::Unphysical {
                unit: "kg/m^3",
                ..
            })
        ));
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
                DensityModel::IAEAZy => assert_eq!(range, (0.0, 1800.0)),
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
