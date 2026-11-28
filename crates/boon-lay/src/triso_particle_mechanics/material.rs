// SPDX-License-Identifier: AGPL-3.0-only
//! Layer materials: isotropic elasticity, thermal expansion, conduction, and
//! the irradiation laws of the BISON Theory Manual (Hales et al. 2013,
//! INL/EXT-13-29930, Section 10).
//!
//! # Units of fluence
//!
//! Eqs (10.1)–(10.3) print rates "in units of 1/(10²⁵ n/m²)" with the fluence
//! `Φ` "in units of 10²⁵ n/m²" (Section 10.1). Everything public here takes
//! fluence in **n/m²** and converts internally. The manual calls it fast
//! fluence and does not give the energy cut-off.
//!
//! # The reduced (r, θ) notation
//!
//! Under spherical symmetry `σ_θθ = σ_φφ` and `ε_θθ = ε_φφ`, so a strain or a
//! stress is the pair `[r, θ]`. In that notation isotropic elasticity
//! (Eq (6.2)) is
//!
//! ```text
//! σ_r = (λ + 2μ) ε_r + 2λ ε_θ
//! σ_θ =  λ ε_r      + 2(λ + μ) ε_θ
//! ```
//!
//! and a linear creep law `ε̇_i = K[σ_i − ν(σ_j + σ_k)]Φ̇` is
//! `ε̇ = K Φ̇ M(ν) σ` with `M(ν) = [[1, −2ν], [−ν, 1 − ν]]`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use petir::mathf::RealMath;

use uom::si::f64::{Pressure, TemperatureCoefficient, ThermalConductivity};
use uom::si::pressure::pascal;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;

use outram_park_fork_offbeat::materials::properties::conductivity::ConductivityModel;
use outram_park_fork_offbeat::materials::properties::thermal_expansion::ThermalExpansionModel;
use outram_park_fork_offbeat::materials::properties::young_modulus::YoungModulusModel;
use outram_park_fork_offbeat::materials::MaterialState;

use super::ParticleMechanicsError;

/// The manual's fluence unit, 10²⁵ n/m² (Section 10.1), in n/m².
pub const BISON_FLUENCE_UNIT_PER_M2: f64 = 1.0e25;

/// SiC irradiation creep coefficient at 640 °C, `2 × 10⁻³⁷ (Pa·n/m²)⁻¹`,
/// as the BISON Theory Manual quotes it from Lewinsohn et al. (1998) for
/// Eq (10.7) (Section 10.2, p. 40).
///
/// The manual adds that "a figure in that reference seems to indicate that
/// typical values for K are about one-tenth those mentioned in the text".
/// Both readings are the caller's choice; this constant is the text value.
/// Eq (10.7) is scalar, `ε̇ = Kσφ`, and the manual gives no creep Poisson's
/// ratio for SiC, so [`IrradiationCreep::Linear`] makes the caller state one.
pub const SIC_CREEP_COEFFICIENT_640C_PER_PA_PER_FLUENCE: f64 = 2.0e-37;

/// SiC irradiation creep coefficient at 900 °C, `4 × 10⁻³⁷ (Pa·n/m²)⁻¹`,
/// as [`SIC_CREEP_COEFFICIENT_640C_PER_PA_PER_FLUENCE`] (Section 10.2, p. 40).
pub const SIC_CREEP_COEFFICIENT_900C_PER_PA_PER_FLUENCE: f64 = 4.0e-37;

/// The creep Poisson's ratio of PyC, `ν_c = 0.5` (Section 10.1, below
/// Eq (10.4)).
pub const PYC_CREEP_POISSON_RATIO: f64 = 0.5;

/// The irradiation-creep multiplier `M_Irr,Creep = 2` of Eq (10.5)
/// (Section 10.1).
pub const PYC_CREEP_MULTIPLIER: f64 = 2.0;

/// Irradiation-induced dimensional change: a strain that depends on fast
/// fluence only, `[ε_r, ε_θ](Φ)`, zero at zero fluence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DimensionalChange {
    /// No irradiation strain.
    None,
    /// A constant rate per unit fluence, strain per (n/m²), in each direction.
    ConstantRate {
        /// Radial strain per n/m² (dimensionless per n/m²).
        radial_per_fluence: f64,
        /// Tangential strain per n/m².
        tangential_per_fluence: f64,
    },
    /// Low-density PyC (the buffer), Eq (10.1):
    /// `ε̇_r = ε̇_θ = −0.176 exp(−1.75 Φ)`, per 10²⁵ n/m², `Φ` in 10²⁵ n/m².
    Bison2013Buffer,
    /// Dense PyC (IPyC, OPyC), Eqs (10.2)–(10.3):
    /// `ε̇_r = −0.077 exp(−Φ) + 0.031`,
    /// `ε̇_θ = −0.036 exp(−2.1 Φ) − 0.01`, per 10²⁵ n/m².
    ///
    /// As printed, with no dependence on density, anisotropy (BAF) or
    /// temperature.
    Bison2013DensePyc,
}

impl DimensionalChange {
    /// Strain rate per unit fluence, `[dε_r/dΦ, dε_θ/dΦ]`, per (n/m²), at
    /// fast fluence `fluence_m2` (n/m²).
    #[must_use]
    pub fn rate_per_fluence(&self, fluence_m2: f64) -> [f64; 2] {
        let x = fluence_m2 / BISON_FLUENCE_UNIT_PER_M2;
        let to_si = 1.0 / BISON_FLUENCE_UNIT_PER_M2;
        match self {
            Self::None => [0.0, 0.0],
            Self::ConstantRate {
                radial_per_fluence,
                tangential_per_fluence,
            } => [*radial_per_fluence, *tangential_per_fluence],
            Self::Bison2013Buffer => {
                let r = -0.176 * (-1.75 * x).r_exp();
                [r * to_si, r * to_si]
            }
            Self::Bison2013DensePyc => [
                (-0.077 * (-x).r_exp() + 0.031) * to_si,
                (-0.036 * (-2.1 * x).r_exp() - 0.01) * to_si,
            ],
        }
    }

    /// Accumulated strain `[ε_r, ε_θ]` (dimensionless) at fast fluence
    /// `fluence_m2` (n/m²): the exact integral of
    /// [`Self::rate_per_fluence`] from zero.
    ///
    /// For the BISON laws the integral is closed-form, e.g. for the buffer
    /// `ε = −(0.176/1.75)(1 − exp(−1.75 Φ))`. Integrating exactly rather than
    /// rate times step keeps the irradiation strain free of time-step error;
    /// `tests::dimensional_change_is_the_integral_of_the_printed_rate` checks
    /// the closed forms against a quadrature of the printed rates.
    #[must_use]
    pub fn strain_at(&self, fluence_m2: f64) -> [f64; 2] {
        let x = fluence_m2 / BISON_FLUENCE_UNIT_PER_M2;
        match self {
            Self::None => [0.0, 0.0],
            Self::ConstantRate {
                radial_per_fluence,
                tangential_per_fluence,
            } => [radial_per_fluence * fluence_m2, tangential_per_fluence * fluence_m2],
            Self::Bison2013Buffer => {
                let e = -(0.176 / 1.75) * (1.0 - (-1.75 * x).r_exp());
                [e, e]
            }
            Self::Bison2013DensePyc => [
                -0.077 * (1.0 - (-x).r_exp()) + 0.031 * x,
                -(0.036 / 2.1) * (1.0 - (-2.1 * x).r_exp()) - 0.01 * x,
            ],
        }
    }
}

/// Which sign the lateral term of the creep law carries. See the module
/// docs of [`super`], finding 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreepLawSign {
    /// `ε̇₁ = K[σ₁ − ν_c(σ₂ + σ₃)]Φ̇`: with `ν_c = 0.5` creep conserves
    /// volume. **The default.**
    VolumeConserving,
    /// `ε̇₁ = K[σ₁ + ν_c(σ₂ + σ₃)]Φ̇`, Eq (10.4) exactly as printed. Kept so
    /// the printed form can be run and compared; with `ν_c = 0.5` a
    /// hydrostatic stress creeps in volume.
    AsPrinted,
}

/// Irradiation creep law of a layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IrradiationCreep {
    /// No creep.
    None,
    /// A constant coefficient: `ε̇_i = K[σ_i ∓ ν_c(σ_j + σ_k)]Φ̇` with `K` in
    /// (Pa·n/m²)⁻¹. Eq (10.7) for SiC is this form with the caller's `ν_c`.
    Linear {
        /// `K`, (Pa·n/m²)⁻¹, >= 0.
        coefficient_per_pa_per_fluence: f64,
        /// `ν_c`, dimensionless.
        creep_poisson_ratio: f64,
        /// Sign of the lateral term.
        sign: CreepLawSign,
    },
    /// PyC, Eqs (10.4)–(10.6): `K = K₀[1 + 2.38(1.9 − ρ)]·2` with
    /// `K₀ = 1.996e-29 − 4.415e-32 T + 3.6544e-35 T²`, `T` in °C, `ρ` in
    /// g/cm³, `ν_c = 0.5`, evaluated at the local temperature each step. `K`
    /// read as (MPa·n/m²)⁻¹ (module docs of [`super`], finding 2).
    Bison2013Pyc {
        /// PyC density, g/cm³.
        density_g_per_cm3: f64,
        /// Sign of the lateral term.
        sign: CreepLawSign,
    },
}

/// The PyC creep coefficient of Eqs (10.5)–(10.6), in **(Pa·n/m²)⁻¹**.
///
/// `temperature_celsius` is °C, as the manual prints `K₀(T)`; `density` is
/// g/cm³. The manual's `K` is read as (MPa·n/m²)⁻¹ and divided by 10⁶ here
/// (module docs of [`super`], finding 2: an inference, not a printed unit).
///
/// At 1000 °C and 1.9 g/cm³: `K₀ = 1.996e-29 − 4.415e-29 + 3.6544e-29 =
/// 1.2354e-29`, `K = 2 K₀ = 2.4708e-29 (MPa·n/m²)⁻¹ = 2.4708e-35
/// (Pa·n/m²)⁻¹` (`tests::pyc_creep_coefficient_transcription`).
#[must_use]
pub fn pyc_creep_coefficient_bison2013(temperature_celsius: f64, density_g_per_cm3: f64) -> f64 {
    let t = temperature_celsius;
    let k0 = 1.996e-29 - 4.415e-32 * t + 3.6544e-35 * t * t;
    let k_per_mpa = k0 * (1.0 + 2.38 * (1.9 - density_g_per_cm3)) * PYC_CREEP_MULTIPLIER;
    k_per_mpa * 1.0e-6
}

impl IrradiationCreep {
    /// `(K, ν_eff)` at `temperature_k` (kelvin): the coefficient in
    /// (Pa·n/m²)⁻¹ and the signed lateral ratio of the reduced compliance
    /// `M(ν_eff)` (`ν_eff = ν_c` for [`CreepLawSign::VolumeConserving`],
    /// `−ν_c` for [`CreepLawSign::AsPrinted`]). `K = 0` means no creep.
    #[must_use]
    pub fn coefficient_and_ratio(&self, temperature_k: f64) -> (f64, f64) {
        fn signed(nu: f64, sign: CreepLawSign) -> f64 {
            match sign {
                CreepLawSign::VolumeConserving => nu,
                CreepLawSign::AsPrinted => -nu,
            }
        }
        match self {
            Self::None => (0.0, 0.0),
            Self::Linear {
                coefficient_per_pa_per_fluence,
                creep_poisson_ratio,
                sign,
            } => (*coefficient_per_pa_per_fluence, signed(*creep_poisson_ratio, *sign)),
            Self::Bison2013Pyc {
                density_g_per_cm3,
                sign,
            } => (
                // A temperature far enough outside the fitted range can drive
                // the quadratic K0 negative; creep never runs backwards, so it
                // floors at zero.
                pyc_creep_coefficient_bison2013(temperature_k - 273.15, *density_g_per_cm3)
                    .max(0.0),
                signed(PYC_CREEP_POISSON_RATIO, *sign),
            ),
        }
    }
}

/// The reduced creep compliance `M(ν) = [[1, −2ν], [−ν, 1 − ν]]` (module
/// docs): `ε̇ = K Φ̇ M σ` for `σ = [σ_r, σ_θ]`.
#[must_use]
pub fn reduced_creep_compliance(nu: f64) -> [[f64; 2]; 2] {
    [[1.0, -2.0 * nu], [-nu, 1.0 - nu]]
}

/// Young's modulus of a layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum YoungsModulus {
    /// A constant, Pa.
    Constant(f64),
    /// A correlation from `outram-park-fork-offbeat` (for TRISO layers:
    /// `ParfumeBuffer`, `ParfumePyC`, `ParfumeSiC`, `SneadSiC`), evaluated at
    /// the Gauss point's temperature and fast fluence every step.
    Offbeat(YoungModulusModel),
}

/// Thermal strain of a layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThermalStrain {
    /// `α (T − T_ref)` in every direction, `α` in 1/K, with `T_ref` the
    /// solver's stress-free temperature.
    Linear {
        /// Linear expansion coefficient, 1/K.
        alpha_per_k: f64,
    },
    /// A correlation from `outram-park-fork-offbeat` (`PARFUMEBuffer`,
    /// `PARFUMEPyC`, `PARFUMESiC`, `SneadSiC`, ...), which carries its OWN
    /// stress-free temperature. Its principal strains are used, so the
    /// transversely isotropic PARFUME PyC law gives `ε_r ≠ ε_θ`.
    Offbeat(ThermalExpansionModel),
}

/// Thermal conductivity of a layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Conductivity {
    /// A constant, W/(m·K).
    Constant(f64),
    /// A correlation from `outram-park-fork-offbeat` (`ParfumeBuffer`,
    /// `ParfumeSiC`, ...), evaluated at the local temperature; conduction is
    /// then nonlinear and [`super::thermal`] iterates.
    Offbeat(ConductivityModel),
}

/// The `offbeat` material state at temperature `t_k` (K) and fast fluence
/// `fluence_m2` (n/m²), everything else fresh (no burnup, no porosity).
#[must_use]
pub fn offbeat_state(t_k: f64, fluence_m2: f64) -> MaterialState {
    let mut s = MaterialState::fresh(t_k);
    s.fast_fluence = fluence_m2;
    s
}

/// One layer's material.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerMaterial {
    /// Young's modulus.
    pub youngs_modulus: YoungsModulus,
    /// Poisson's ratio, dimensionless, in (−1, 0.5). Always a constant:
    /// `outram-park-fork-offbeat` has no TRISO Poisson's-ratio correlation
    /// (checked 2026-11-28; its variants are UO2, MOX, Zircaloy, Mo, 15-15Ti).
    pub poisson_ratio: f64,
    /// Thermal strain. Zero unless given.
    pub thermal_strain: ThermalStrain,
    /// Thermal conductivity; `None` until given, and [`super::thermal`]
    /// refuses a layer without one.
    pub conductivity: Option<Conductivity>,
    /// Irradiation creep law.
    pub creep: IrradiationCreep,
    /// Irradiation dimensional change.
    pub dimensional_change: DimensionalChange,
}

fn check_poisson(poisson_ratio: f64) -> Result<(), ParticleMechanicsError> {
    if poisson_ratio > -1.0 && poisson_ratio < 0.5 {
        Ok(())
    } else {
        Err(ParticleMechanicsError::InvalidMaterial(format!(
            "Poisson's ratio must be in (-1, 0.5), got {poisson_ratio}"
        )))
    }
}

impl LayerMaterial {
    /// An isotropic elastic layer with a constant Young's modulus and no
    /// thermal strain, creep or dimensional change.
    ///
    /// # Errors
    ///
    /// [`ParticleMechanicsError::InvalidMaterial`] if `E <= 0`, non-finite,
    /// or `ν` is outside (−1, 0.5).
    pub fn elastic(
        youngs_modulus: Pressure,
        poisson_ratio: f64,
    ) -> Result<Self, ParticleMechanicsError> {
        let e = youngs_modulus.get::<pascal>();
        if !e.is_finite() || e <= 0.0 {
            return Err(ParticleMechanicsError::InvalidMaterial(format!(
                "Young's modulus must be positive, got {e} Pa"
            )));
        }
        match check_poisson(poisson_ratio) {
            Ok(()) => {}
            Err(err) => return Err(err),
        }
        Ok(Self {
            youngs_modulus: YoungsModulus::Constant(e),
            poisson_ratio,
            thermal_strain: ThermalStrain::Linear { alpha_per_k: 0.0 },
            conductivity: None,
            creep: IrradiationCreep::None,
            dimensional_change: DimensionalChange::None,
        })
    }

    /// A layer whose Young's modulus is an `outram-park-fork-offbeat`
    /// correlation, evaluated at each Gauss point's temperature and fluence
    /// every step. `poisson_ratio` is the caller's, from a cited source.
    ///
    /// # Errors
    ///
    /// [`ParticleMechanicsError::InvalidMaterial`] if `ν` is outside
    /// (−1, 0.5).
    pub fn offbeat_elastic(
        youngs_modulus: YoungModulusModel,
        poisson_ratio: f64,
    ) -> Result<Self, ParticleMechanicsError> {
        match check_poisson(poisson_ratio) {
            Ok(()) => {}
            Err(err) => return Err(err),
        }
        Ok(Self {
            youngs_modulus: YoungsModulus::Offbeat(youngs_modulus),
            poisson_ratio,
            thermal_strain: ThermalStrain::Linear { alpha_per_k: 0.0 },
            conductivity: None,
            creep: IrradiationCreep::None,
            dimensional_change: DimensionalChange::None,
        })
    }

    /// With a constant linear thermal expansion coefficient (1/K).
    #[must_use]
    pub fn with_thermal_expansion(mut self, alpha: TemperatureCoefficient) -> Self {
        self.thermal_strain = ThermalStrain::Linear {
            alpha_per_k: alpha.get::<per_kelvin>(),
        };
        self
    }

    /// With an `outram-park-fork-offbeat` thermal-expansion correlation
    /// (which carries its own stress-free temperature).
    #[must_use]
    pub fn with_offbeat_thermal_expansion(mut self, model: ThermalExpansionModel) -> Self {
        self.thermal_strain = ThermalStrain::Offbeat(model);
        self
    }

    /// With a constant thermal conductivity (W/(m·K)).
    #[must_use]
    pub fn with_thermal_conductivity(mut self, k: ThermalConductivity) -> Self {
        self.conductivity = Some(Conductivity::Constant(k.get::<watt_per_meter_kelvin>()));
        self
    }

    /// With an `outram-park-fork-offbeat` conductivity correlation.
    #[must_use]
    pub fn with_offbeat_conductivity(mut self, model: ConductivityModel) -> Self {
        self.conductivity = Some(Conductivity::Offbeat(model));
        self
    }

    /// With an irradiation creep law.
    #[must_use]
    pub fn with_irradiation_creep(mut self, creep: IrradiationCreep) -> Self {
        self.creep = creep;
        self
    }

    /// With an irradiation dimensional-change law.
    #[must_use]
    pub fn with_dimensional_change(mut self, change: DimensionalChange) -> Self {
        self.dimensional_change = change;
        self
    }

    /// Young's modulus at `t_k` (K) and fast fluence `fluence_m2` (n/m²), Pa.
    /// An `offbeat` correlation clamps its inputs to its validity range, as
    /// `offbeat`'s `value` does.
    #[must_use]
    pub fn youngs_modulus_at(&self, t_k: f64, fluence_m2: f64) -> f64 {
        match self.youngs_modulus {
            YoungsModulus::Constant(e) => e,
            YoungsModulus::Offbeat(m) => m.value(&offbeat_state(t_k, fluence_m2)),
        }
    }

    /// Thermal strain `[ε_r, ε_θ]` at `t_k` (K); `t_ref_k` (K) is used by
    /// [`ThermalStrain::Linear`] only.
    #[must_use]
    pub fn thermal_strain_at(&self, t_k: f64, t_ref_k: f64) -> [f64; 2] {
        match self.thermal_strain {
            ThermalStrain::Linear { alpha_per_k } => {
                let e = alpha_per_k * (t_k - t_ref_k);
                [e, e]
            }
            ThermalStrain::Offbeat(m) => {
                let p = m.principal_strains(&offbeat_state(t_k, 0.0));
                [p[0], p[1]]
            }
        }
    }

    /// Thermal conductivity at `t_k` (K), W/(m·K), or `None` if not given.
    #[must_use]
    pub fn conductivity_at(&self, t_k: f64) -> Option<f64> {
        match self.conductivity {
            None => None,
            Some(Conductivity::Constant(k)) => Some(k),
            Some(Conductivity::Offbeat(m)) => Some(m.value(&offbeat_state(t_k, 0.0))),
        }
    }

    /// Lamé parameters `(λ, μ)` at `t_k` (K) and `fluence_m2` (n/m²), Pa.
    #[must_use]
    pub fn lame_at(&self, t_k: f64, fluence_m2: f64) -> (f64, f64) {
        let e = self.youngs_modulus_at(t_k, fluence_m2);
        let nu = self.poisson_ratio;
        let lambda = e * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
        let mu = e / (2.0 * (1.0 + nu));
        (lambda, mu)
    }

    /// The reduced elasticity matrix `D` (module docs) at `t_k` (K) and
    /// `fluence_m2` (n/m²), Pa: `σ = D ε`.
    #[must_use]
    pub fn reduced_elasticity_at(&self, t_k: f64, fluence_m2: f64) -> [[f64; 2]; 2] {
        let (l, m) = self.lame_at(t_k, fluence_m2);
        [[l + 2.0 * m, 2.0 * l], [l, 2.0 * (l + m)]]
    }
}
