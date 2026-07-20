// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/thermoMechanics/legacyThermoMechanics/
//                    legacyThermoMechanics.C  (constructor: property derivation)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal author: Carlo Fiorina (EPFL)
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

//! # Linear-elastic, thermally-expanding material
//!
//! Per-material-zone constitutive data for GeN-Foam's `legacyThermoMechanics`
//! model, ported from the property block of its constructor
//! (`legacyThermoMechanics.C`). Each mesh cell zone in GeN-Foam carries one such
//! isotropic linear-elastic material: density, Young's modulus, Poisson's ratio,
//! specific heat, conductivity, and a linear thermal-expansion coefficient.
//!
//! ## Derived elastic moduli
//!
//! From Young's modulus `E` and Poisson's ratio `ν` this computes the Lamé
//! parameters and the bulk-modulus term used by the stress solve:
//!
//! ```text
//!   μ  = E / (2 (1 + ν))                         (shear modulus)
//!   λ  = ν E / ((1 + ν)(1 − 2ν))                 (Lamé's first parameter, plane strain)
//!   3K = E / (1 − 2ν)                            (bulk-modulus term,     plane strain)
//! ```
//!
//! For the plane-stress rheology option GeN-Foam substitutes
//! `λ = ν E / ((1 + ν)(1 − ν))` and `3K = E / (1 − ν)` (see [`RheologyOption`]).
//!
//! It also derives the thermal diffusivity `DT = k / (ρ c_p)` used by the
//! structure heat-diffusion solve.
//!
//! ## Units note vs. upstream
//!
//! GeN-Foam divides its displacement equation through by density, so upstream
//! `E_`, `mu_`, `lambda_`, `sigmaD_` are **specific** (per-unit-mass, `m²/s²`)
//! and the physical stress is recovered as `rho·sigmaD`. This port keeps every
//! modulus in **physical** SI units (pascals) throughout; the two formulations
//! are algebraically identical (see [`super::stress`]).

use uom::si::f64::{
    DiffusionCoefficient, Length, MassDensity, Pressure, Ratio, SpecificHeatCapacity,
    TemperatureCoefficient, ThermalConductivity,
};
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;

// ---------------------------------------------------------------------------
// Named uom aliases (human interface layer)
// ---------------------------------------------------------------------------

/// Young's modulus `E` — an elastic stiffness, a [`Pressure`] (pascals, Pa).
/// Named alias for readability at call sites.
pub type YoungsModulus = Pressure;

/// Shear modulus `μ` (Lamé's second parameter) — a [`Pressure`] (Pa).
pub type ShearModulus = Pressure;

/// Lamé's first parameter `λ` — a [`Pressure`] (Pa).
pub type LameLambda = Pressure;

/// Bulk-modulus term `3K` used in the thermal-stress relation — a [`Pressure`]
/// (Pa). Equals `E / (1 − 2ν)` (plane strain) or `E / (1 − ν)` (plane stress).
pub type BulkModulusTerm = Pressure;

/// A mechanical stress component — a [`Pressure`] (Pa).
pub type Stress = Pressure;

/// Linear thermal-expansion coefficient `α` — inverse temperature (`1/K`).
/// Named alias over `uom`'s [`TemperatureCoefficient`].
pub type ThermalExpansionCoeff = TemperatureCoefficient;

/// Thermal diffusivity `DT = k / (ρ c_p)` — [`DiffusionCoefficient`] (`m²/s`).
pub type ThermalDiffusivity = DiffusionCoefficient;

/// A displacement / elongation — a [`Length`] (metres). Named alias for the
/// mesh-displacement and thermal-expansion outputs of this module.
pub type Displacement = Length;

// ---------------------------------------------------------------------------
// Rheology option
// ---------------------------------------------------------------------------

/// Which 2-D idealisation the Lamé parameters `λ` and `3K` are formed under.
///
/// Ported from `legacyThermoMechanics`'s `planeStress_` switch
/// (`rheologyOption/planeStress` in the `thermoMechanicalProperties`
/// dictionary). In a full 3-D analysis the distinction is moot and
/// [`RheologyOption::PlaneStrain`] (the GeN-Foam default) is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RheologyOption {
    /// Plane strain (out-of-plane strain assumed zero) — GeN-Foam's default.
    /// `λ = ν E / ((1+ν)(1−2ν))`, `3K = E / (1−2ν)`.
    #[default]
    PlaneStrain,
    /// Plane stress (out-of-plane stress assumed zero) — for thin bodies.
    /// `λ = ν E / ((1+ν)(1−ν))`, `3K = E / (1−ν)`.
    PlaneStress,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors from constructing a [`ElasticMaterial`].
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MaterialError {
    /// Young's modulus was not strictly positive.
    #[error("Young's modulus must be positive, got {0} Pa")]
    NonPositiveYoungsModulus(f64),

    /// Density was not strictly positive.
    #[error("density must be positive, got {0} kg/m³")]
    NonPositiveDensity(f64),

    /// Specific heat capacity was not strictly positive.
    #[error("specific heat capacity must be positive, got {0} J/(kg·K)")]
    NonPositiveSpecificHeat(f64),

    /// Poisson's ratio was outside the thermodynamically admissible range
    /// `−1 < ν < 1/2` for an isotropic material. At `ν = 1/2` the material is
    /// incompressible and `3K → ∞`, so the derived moduli diverge.
    #[error("Poisson's ratio must satisfy -1 < ν < 0.5, got {0}")]
    PoissonRatioOutOfRange(f64),
}

// ---------------------------------------------------------------------------
// ElasticMaterial
// ---------------------------------------------------------------------------

/// An isotropic linear-elastic material with linear thermal expansion — the
/// per-zone material card of GeN-Foam's `legacyThermoMechanics`.
///
/// All fields carry `uom` dimensioned quantities. Construct with [`Self::new`],
/// which validates the ranges and is the only way to build one.
///
/// # Valid ranges / assumptions
///
/// - `E > 0` (pascals). Typical structural metals: `E ≈ 200 GPa` (steel).
/// - `−1 < ν < 1/2` (dimensionless). Typical metals `ν ≈ 0.3`.
/// - `ρ > 0` (kg/m³), `c_p > 0` (J/(kg·K)).
/// - `α ≥ 0` typically (1/K); metals `α ≈ 1–2 × 10⁻⁵ K⁻¹`.
/// - `k ≥ 0` (W/(m·K)).
///
/// Isotropy and small-strain linear elasticity are assumed (as in the upstream
/// model); no plasticity, creep, or anisotropy is represented.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElasticMaterial {
    /// Young's modulus `E` (Pa).
    youngs_modulus: YoungsModulus,
    /// Poisson's ratio `ν` (dimensionless).
    poisson_ratio: Ratio,
    /// Linear thermal-expansion coefficient `α` (1/K).
    thermal_expansion: ThermalExpansionCoeff,
    /// Mass density `ρ` (kg/m³).
    density: MassDensity,
    /// Specific heat capacity `c_p` (J/(kg·K)).
    specific_heat: SpecificHeatCapacity,
    /// Thermal conductivity `k` (W/(m·K)).
    conductivity: ThermalConductivity,
}

impl ElasticMaterial {
    /// Build a material from its physical properties, validating ranges.
    ///
    /// # Parameters
    ///
    /// - `youngs_modulus` — `E` (Pa), strictly positive.
    /// - `poisson_ratio` — `ν` (dimensionless), `−1 < ν < 1/2`.
    /// - `thermal_expansion` — `α` (1/K).
    /// - `density` — `ρ` (kg/m³), strictly positive.
    /// - `specific_heat` — `c_p` (J/(kg·K)), strictly positive.
    /// - `conductivity` — `k` (W/(m·K)).
    ///
    /// # Errors
    ///
    /// [`MaterialError`] if `E`, `ρ`, or `c_p` is non-positive, or `ν` is
    /// outside `(−1, 1/2)`.
    pub fn new(
        youngs_modulus: YoungsModulus,
        poisson_ratio: Ratio,
        thermal_expansion: ThermalExpansionCoeff,
        density: MassDensity,
        specific_heat: SpecificHeatCapacity,
        conductivity: ThermalConductivity,
    ) -> Result<Self, MaterialError> {
        use uom::si::mass_density::kilogram_per_cubic_meter;
        use uom::si::ratio::ratio;
        use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;

        // NaN-inclusive guards: `x <= 0.0 || x.is_nan()` rejects non-positive
        // and non-finite inputs (a bare `x <= 0.0` would let a NaN slip through).
        let e = youngs_modulus.get::<pascal>();
        if e <= 0.0 || e.is_nan() {
            return Err(MaterialError::NonPositiveYoungsModulus(e));
        }
        let nu = poisson_ratio.get::<ratio>();
        if nu.is_nan() || !(-1.0..0.5).contains(&nu) {
            return Err(MaterialError::PoissonRatioOutOfRange(nu));
        }
        let rho = density.get::<kilogram_per_cubic_meter>();
        if rho <= 0.0 || rho.is_nan() {
            return Err(MaterialError::NonPositiveDensity(rho));
        }
        let cp = specific_heat.get::<joule_per_kilogram_kelvin>();
        if cp <= 0.0 || cp.is_nan() {
            return Err(MaterialError::NonPositiveSpecificHeat(cp));
        }
        Ok(Self {
            youngs_modulus,
            poisson_ratio,
            thermal_expansion,
            density,
            specific_heat,
            conductivity,
        })
    }

    /// Young's modulus `E` (Pa).
    pub fn youngs_modulus(&self) -> YoungsModulus {
        self.youngs_modulus
    }

    /// Poisson's ratio `ν` (dimensionless).
    pub fn poisson_ratio(&self) -> Ratio {
        self.poisson_ratio
    }

    /// Linear thermal-expansion coefficient `α` (1/K).
    pub fn thermal_expansion(&self) -> ThermalExpansionCoeff {
        self.thermal_expansion
    }

    /// Mass density `ρ` (kg/m³).
    pub fn density(&self) -> MassDensity {
        self.density
    }

    /// Specific heat capacity `c_p` (J/(kg·K)).
    pub fn specific_heat(&self) -> SpecificHeatCapacity {
        self.specific_heat
    }

    /// Thermal conductivity `k` (W/(m·K)).
    pub fn conductivity(&self) -> ThermalConductivity {
        self.conductivity
    }

    /// Shear modulus `μ = E / (2 (1 + ν))` (Pa) — Lamé's second parameter.
    ///
    /// Rheology-independent (upstream: `mu_ = E_/(2(1+nu_))`).
    pub fn shear_modulus(&self) -> ShearModulus {
        let nu = self.poisson_ratio.get::<ratio>();
        self.youngs_modulus / (2.0 * (1.0 + nu))
    }

    /// Lamé's first parameter `λ` (Pa) under the given [`RheologyOption`].
    ///
    /// - Plane strain: `λ = ν E / ((1 + ν)(1 − 2ν))`
    /// - Plane stress: `λ = ν E / ((1 + ν)(1 − ν))`
    pub fn lame_lambda(&self, rheology: RheologyOption) -> LameLambda {
        let nu = self.poisson_ratio.get::<ratio>();
        let denom = match rheology {
            RheologyOption::PlaneStrain => (1.0 + nu) * (1.0 - 2.0 * nu),
            RheologyOption::PlaneStress => (1.0 + nu) * (1.0 - nu),
        };
        self.youngs_modulus * (nu / denom)
    }

    /// Bulk-modulus term `3K` (Pa) under the given [`RheologyOption`].
    ///
    /// - Plane strain: `3K = E / (1 − 2ν)`
    /// - Plane stress: `3K = E / (1 − ν)`
    ///
    /// This is the coefficient of the thermal-stress term `3K α ΔT` and equals
    /// `3λ + 2μ` in the plane-strain case.
    pub fn bulk_modulus_three_k(&self, rheology: RheologyOption) -> BulkModulusTerm {
        let nu = self.poisson_ratio.get::<ratio>();
        let denom = match rheology {
            RheologyOption::PlaneStrain => 1.0 - 2.0 * nu,
            RheologyOption::PlaneStress => 1.0 - nu,
        };
        self.youngs_modulus / denom
    }

    /// Thermal diffusivity `DT = k / (ρ c_p)` (m²/s) for the structure
    /// heat-diffusion solve (upstream: `DT_ = (rhoK_/rho_)/C_`).
    pub fn thermal_diffusivity(&self) -> ThermalDiffusivity {
        self.conductivity / (self.density * self.specific_heat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::pressure::gigapascal;
    use uom::si::ratio::ratio;
    use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
    use uom::si::temperature_coefficient::per_kelvin;
    use uom::si::thermal_conductivity::watt_per_meter_kelvin;

    /// Reference stainless-steel-like material used across the test suite.
    fn steel() -> ElasticMaterial {
        ElasticMaterial::new(
            YoungsModulus::new::<gigapascal>(200.0),
            Ratio::new::<ratio>(0.3),
            ThermalExpansionCoeff::new::<per_kelvin>(1.2e-5),
            MassDensity::new::<kilogram_per_cubic_meter>(7850.0),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(500.0),
            ThermalConductivity::new::<watt_per_meter_kelvin>(15.0),
        )
        .unwrap()
    }

    #[test]
    fn lame_parameters_match_hand_computation() {
        // E = 200 GPa, ν = 0.3.
        //   μ  = E / (2·1.3)          = 76.923 GPa
        //   λ  = 0.3·E / (1.3·0.4)    = 115.385 GPa   (plane strain)
        //   3K = E / 0.4              = 500 GPa       (plane strain)
        let m = steel();
        let mu = m.shear_modulus().get::<gigapascal>();
        let lam = m
            .lame_lambda(RheologyOption::PlaneStrain)
            .get::<gigapascal>();
        let k3 = m
            .bulk_modulus_three_k(RheologyOption::PlaneStrain)
            .get::<gigapascal>();
        assert!((mu - 76.9230769).abs() < 1e-6, "mu = {mu}");
        assert!((lam - 115.3846153).abs() < 1e-6, "lambda = {lam}");
        assert!((k3 - 500.0).abs() < 1e-6, "3K = {k3}");
        // Identity 3K = 3λ + 2μ must hold in plane strain.
        assert!((k3 - (3.0 * lam + 2.0 * mu)).abs() < 1e-6);
    }

    #[test]
    fn thermal_diffusivity_matches_hand_computation() {
        // DT = k / (ρ c_p) = 15 / (7850 · 500) = 3.8217e-6 m²/s.
        use uom::si::diffusion_coefficient::square_meter_per_second;
        let dt = steel()
            .thermal_diffusivity()
            .get::<square_meter_per_second>();
        assert!((dt - 3.821656e-6).abs() < 1e-11, "DT = {dt}");
    }

    #[test]
    fn rejects_incompressible_poisson_ratio() {
        let bad = ElasticMaterial::new(
            YoungsModulus::new::<gigapascal>(200.0),
            Ratio::new::<ratio>(0.5),
            ThermalExpansionCoeff::new::<per_kelvin>(1.2e-5),
            MassDensity::new::<kilogram_per_cubic_meter>(7850.0),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(500.0),
            ThermalConductivity::new::<watt_per_meter_kelvin>(15.0),
        );
        assert!(matches!(bad, Err(MaterialError::PoissonRatioOutOfRange(_))));
    }
}
