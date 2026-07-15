// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/thermoMechanics/legacyThermoMechanics/
//                    include/calculateStress.H, include/solveThermalMechanics.H
//                    (constitutive law: sigmaD, thermal stress, sigmaEq)
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

//! # Linear-elastic thermal-stress constitutive law
//!
//! The pointwise Hooke's-law constitutive relation of GeN-Foam's
//! `legacyThermoMechanics` — the tensor algebra that turns a displacement
//! gradient (small-strain tensor) and a temperature rise into a Cauchy stress
//! tensor and its von Mises equivalent. Ported from `include/calculateStress.H`
//! and the `sigmaD_` update in `include/solveThermalMechanics.H`.
//!
//! ## The relations
//!
//! With small-strain tensor `ε = symm(∇u)` (dimensionless), shear modulus `μ`,
//! Lamé parameter `λ`, bulk-modulus term `3K`, expansion coefficient `α`, and
//! temperature rise above the stress-free reference `ΔT = T − T_ref`:
//!
//! ```text
//!   σ_D = 2μ ε + λ tr(ε) I              (isothermal deviatoric-plus-volumetric stress)
//!   σ   = σ_D − 3K α ΔT I               (full stress, thermal contraction removed)
//!   σ_eq = sqrt( (3/2) dev(σ) : dev(σ) )   (von Mises equivalent stress)
//! ```
//!
//! Upstream carries `σ_D` and `3Kα` in density-normalised (specific) units and
//! multiplies by `ρ` in `calculateStress.H` to recover physical stress:
//! `sigma = rho·sigmaD − I·rho·threeKalpha·(TStruct − TStructRef)`. This port
//! works in **physical pascals** directly, which is algebraically identical.
//!
//! ## Tensor representation
//!
//! Stress and strain are symmetric second-order tensors. There is no `uom`
//! tensor type, so tensors use [`outram_foam_basic_lib`]'s dimensionless
//! [`SymmTensor`] with the physical unit documented in each signature (strain is
//! dimensionless; the returned stress tensor is in **pascals**). Scalar outputs
//! ([`von_mises_stress`], [`hydrostatic_stress`]) are returned as `uom`
//! [`Pressure`] so the public boundary stays dimensioned.

use super::material::{BulkModulusTerm, LameLambda, ShearModulus, Stress};
use outram_foam_basic_lib::primitives::SymmTensor;
use uom::si::f64::TemperatureInterval;
use uom::si::pressure::pascal;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::temperature_interval::kelvin;

use super::material::ThermalExpansionCoeff;

/// Isothermal stress tensor `σ_D = 2μ ε + λ tr(ε) I` (pascals).
///
/// The mechanical (non-thermal) part of the Cauchy stress for a small-strain
/// linear-elastic isotropic solid. Upstream:
/// `sigmaD_ = mu_*twoSymm(gradD) + lambda_*(I*tr(gradD))`, where
/// `twoSymm(∇u) = ∇u + ∇uᵀ = 2ε`.
///
/// # Parameters
///
/// - `strain` — small-strain tensor `ε = symm(∇u)` (dimensionless).
/// - `mu` — shear modulus `μ` (Pa).
/// - `lambda` — Lamé's first parameter `λ` (Pa).
///
/// # Returns
///
/// The stress tensor `σ_D` in **pascals** (as a [`SymmTensor`]).
pub fn deviatoric_plus_volumetric_stress(
    strain: SymmTensor,
    mu: ShearModulus,
    lambda: LameLambda,
) -> SymmTensor {
    let mu_pa = mu.get::<pascal>();
    let lam_pa = lambda.get::<pascal>();
    // 2μ ε + λ tr(ε) I
    (2.0 * mu_pa) * strain + (lam_pa * strain.tr()) * SymmTensor::IDENTITY
}

/// Full Cauchy stress tensor including the thermal term (pascals).
///
/// `σ = σ_D − 3K α ΔT I`, i.e. the isothermal stress with the isotropic
/// thermal-expansion contribution subtracted (upstream `calculateStress.H`:
/// `sigma = rho·sigmaD − I·rho·threeKalpha·(TStruct − TStructRef)`).
///
/// # Parameters
///
/// - `strain` — small-strain tensor `ε` (dimensionless).
/// - `mu`, `lambda` — Lamé parameters (Pa).
/// - `three_k` — bulk-modulus term `3K` (Pa).
/// - `alpha` — linear thermal-expansion coefficient `α` (1/K).
/// - `delta_t` — temperature rise `ΔT = T − T_ref` above the stress-free
///   reference (kelvin interval).
///
/// # Returns
///
/// The full stress tensor `σ` in **pascals** (as a [`SymmTensor`]).
pub fn thermal_stress(
    strain: SymmTensor,
    mu: ShearModulus,
    lambda: LameLambda,
    three_k: BulkModulusTerm,
    alpha: ThermalExpansionCoeff,
    delta_t: TemperatureInterval,
) -> SymmTensor {
    let sigma_d = deviatoric_plus_volumetric_stress(strain, mu, lambda);
    // 3K α ΔT  (pascals): Pa · (1/K) · K = Pa.
    let thermal_pa = three_k.get::<pascal>() * alpha.get::<per_kelvin>() * delta_t.get::<kelvin>();
    sigma_d - thermal_pa * SymmTensor::IDENTITY
}

/// von Mises equivalent stress `σ_eq = sqrt((3/2) dev(σ) : dev(σ))` (Pa).
///
/// Upstream: `sigmaEq = sqrt((3/2)*magSqr(dev(sigma)))`. The double-inner
/// product `dev(σ):dev(σ)` is `SymmTensor::mag_sqr` of the deviator.
///
/// # Parameters
///
/// - `sigma` — a stress tensor whose components are in **pascals**.
pub fn von_mises_stress(sigma: SymmTensor) -> Stress {
    let s = sigma.dev();
    Stress::new::<pascal>((1.5 * s.mag_sqr()).sqrt())
}

/// Hydrostatic (mean) stress `σ_m = tr(σ)/3` (Pa).
///
/// Positive in tension. For a purely thermal, fully-constrained state this is
/// the whole stress (the deviator vanishes and `σ_eq = 0`).
///
/// # Parameters
///
/// - `sigma` — a stress tensor whose components are in **pascals**.
pub fn hydrostatic_stress(sigma: SymmTensor) -> Stress {
    Stress::new::<pascal>(sigma.tr() / 3.0)
}

#[cfg(test)]
mod tests {
    use super::super::material::{
        ElasticMaterial, RheologyOption, ThermalExpansionCoeff, YoungsModulus,
    };
    use super::*;
    use uom::si::f64::{MassDensity, Ratio, SpecificHeatCapacity, ThermalConductivity};
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::pressure::{gigapascal, megapascal};
    use uom::si::ratio::ratio;
    use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
    use uom::si::temperature_coefficient::per_kelvin;
    use uom::si::temperature_interval::kelvin;
    use uom::si::thermal_conductivity::watt_per_meter_kelvin;

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

    /// V&V — free thermal expansion. Methodology: an unconstrained body heated
    /// uniformly by ΔT expands with strain ε = αΔT·I and develops *zero* stress.
    /// Feed ε = αΔT·I into the full constitutive law and require σ = 0.
    /// Result (2026-07-15, steel, ΔT = 100 K, α = 1.2e-5/K): |σ| < 1e-3 Pa
    /// (machine round-off), σ_eq < 1e-3 Pa. Pass.
    #[test]
    fn free_expansion_is_stress_free() {
        let m = steel();
        let dt = TemperatureInterval::new::<kelvin>(100.0);
        let a = m.thermal_expansion().get::<per_kelvin>();
        let strain = (a * 100.0) * SymmTensor::IDENTITY; // ε = αΔT I
        let sigma = thermal_stress(
            strain,
            m.shear_modulus(),
            m.lame_lambda(RheologyOption::PlaneStrain),
            m.bulk_modulus_three_k(RheologyOption::PlaneStrain),
            m.thermal_expansion(),
            dt,
        );
        assert!(sigma.mag() < 1e-3, "‖σ‖ = {} Pa should vanish", sigma.mag());
        assert!(von_mises_stress(sigma).get::<pascal>().abs() < 1e-3);
    }

    /// V&V — fully constrained bar, uniform heating. Methodology: a body whose
    /// total strain is held at zero (ε = 0) while heated ΔT develops a purely
    /// hydrostatic thermal stress σ = −3K α ΔT · I, so σ_m = −3K α ΔT and the
    /// deviator (hence σ_eq) is zero. Analytical value for steel plane-strain
    /// (3K = 500 GPa, α = 1.2e-5/K, ΔT = 100 K): σ_m = −600 MPa.
    /// Result (2026-07-15): σ_m = −600.000 MPa (|err| < 1e-6 MPa), σ_eq = 0. Pass.
    #[test]
    fn fully_constrained_bar_thermal_stress() {
        let m = steel();
        let dt = TemperatureInterval::new::<kelvin>(100.0);
        let sigma = thermal_stress(
            SymmTensor::ZERO, // total strain held at zero
            m.shear_modulus(),
            m.lame_lambda(RheologyOption::PlaneStrain),
            m.bulk_modulus_three_k(RheologyOption::PlaneStrain),
            m.thermal_expansion(),
            dt,
        );
        let sigma_m = hydrostatic_stress(sigma).get::<megapascal>();
        assert!((sigma_m + 600.0).abs() < 1e-6, "σ_m = {sigma_m} MPa");
        // Purely hydrostatic ⇒ von Mises vanishes.
        assert!(von_mises_stress(sigma).get::<megapascal>().abs() < 1e-6);
    }

    /// V&V — uniaxial constraint (rod clamped along x only), von Mises check.
    /// Methodology: with ε = 0 imposed only along x and the transverse faces
    /// free, the classic 1-D restrained-bar result is σ_xx = −E α ΔT and
    /// σ_yy = σ_zz = 0. Here we instead exercise the deviatoric machinery with a
    /// prescribed uniaxial stress state σ = diag(s,0,0) and require
    /// σ_eq = |s|. Result (2026-07-15, s = −100 MPa): σ_eq = 100.000 MPa. Pass.
    #[test]
    fn uniaxial_stress_von_mises_equals_magnitude() {
        let s = -100.0e6; // Pa
        let sigma = SymmTensor::from_diag(s, 0.0, 0.0);
        let eq = von_mises_stress(sigma).get::<megapascal>();
        assert!((eq - 100.0).abs() < 1e-6, "σ_eq = {eq} MPa");
    }
}
