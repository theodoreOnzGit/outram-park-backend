// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/heatTransferModels/
//             FSHeatTransferCoefficientModels/subModels/{CHFModels,
//             flowEnhancementFactorModels,suppressionFactorModels,
//             subCooledBoilingFractionModels,TONBModels}/
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream authors: Stefan Radman (EPFL), Gauthier Lazare (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `chf` — critical-heat-flux and boiling sub-models
//!
//! The sub-models [`boiling::multi_regime_boiling_htc`](super::boiling::multi_regime_boiling_htc)
//! composes, each a closed enum:
//!
//! | Enum | Upstream classes | Role |
//! |---|---|---|
//! | [`CriticalHeatFlux`] | `constantCHF` | Critical heat flux (departure from nucleate boiling) |
//! | [`LeidenfrostTemperature`] | `GroeneveldStewart` | Minimum film-boiling ("rewetting") temperature |
//! | [`OnsetOfNucleateBoilingTemperature`] | `Basu` | Wall temperature at which nucleate boiling begins |
//! | [`FlowEnhancementFactor`] | `Chen`, `RezkallahSims`, `COBRA_TF` | Enhances forced convection under two-phase flow |
//! | [`SuppressionFactor`] | `Chen`, `COBRA_TF` | Suppresses pool boiling as flow becomes annular |
//! | [`SubcooledBoilingFraction`] | `constant`, `SahaZuber` | Fraction of the sub-cooled pool-boiling heat flux that nets vapour |
//!
//! ## Shared helper
//!
//! [`two_phase_mixture_reynolds_number`] is used by **two** upstream models
//! that independently compute the identical formula:
//! `flowEnhancementFactorModels::COBRA_TF` and `suppressionFactorModels::Chen`.
//! Both need a "mixture" Reynolds number assembled from *both* phases' local
//! density, velocity, viscosity, and void fraction — this port factors that
//! shared computation into one function instead of duplicating it.
//!
//! ## Velocity as a scalar (dimensional note)
//!
//! Upstream forms the mixture Reynolds number from the **vector** sum
//! `mag(alpha1*rho1*U1 + alpha2*rho2*U2)` (`U()` is a `volVectorField`, not
//! the scalar `magU()` used elsewhere in the same classes). This crate has no
//! shared vector/tensor field type in scope for this closure family (the rest
//! of `closures::*`, e.g. `ReynoldsNumber` in [`fs_drag`](super::super::fs_drag),
//! is already scalarised at the API boundary), so
//! [`two_phase_mixture_reynolds_number`] takes each phase's velocity as a
//! **signed scalar** (its component along the flow direction) and computes
//! `abs(alpha1*rho1*u1 + alpha2*rho2*u2)`. This is exact for co-linear
//! (1-D channel) flow — the common case for these porous-channel closures —
//! and is the same scalar reduction the rest of this crate already applies to
//! upstream's tensor-porous-media quantities.
//!
//! ## Deferred (see the parent module doc for the full list + rationale)
//!
//! CHF `lookUpTableCHF` (3-D table interpolation) and the post-CHF
//! `CachardLiquid`/`CachardVapour` models (dimensionally-inconsistent upstream
//! formula, `uom` correctly refuses to compile it) are **not** ported here.
//!
//! ## References
//!
//! - R.T. Lahey Jr. and F.J. Moody, *The Thermal-Hydraulics of a Boiling
//!   Water Nuclear Reactor*, ANS, 1993 (critical heat flux background).
//! - D.C. Groeneveld and J.C. Stewart, "The minimum film boiling temperature
//!   for water during film boiling collapse," *NUREG/CP-0022*, 1982.
//! - J.C. Chen, "Correlation for boiling heat transfer to saturated fluids in
//!   convective flow," *Ind. Eng. Chem. Process Des. Dev.* 5(3), 1966.
//! - K.S. Rezkallah and G.E. Sims, "An Examination of Correlations of Mean
//!   Heat Transfer Coefficients in Two-Phase Two-Component Flow in Vertical
//!   Tubes," *AIChE Symposium Series* 83(257), 1987.
//! - COBRA-TF (Coolant Boiling in Rod Arrays - Two Fluid) code manual,
//!   Pennsylvania State University / NRC, for the `COBRA_TF` flow-enhancement
//!   and suppression correlations.
//! - N. Basu, G.R. Warrier, V.K. Dhir, "Onset of Nucleate Boiling and Active
//!   Nucleation Site Density During Subcooled Flow Boiling," *J. Heat
//!   Transfer* 124, 2002, pp. 717-728.
//! - P. Saha and N. Zuber, "Point of net vapor generation and vapor void
//!   fraction in subcooled boiling," *Proc. 5th Int. Heat Transfer Conf.*,
//!   Tokyo, 1974, paper B4.7 (the published `Δ T_sub` correlation this port
//!   reproduces — see [`SubcooledBoilingFraction::SahaZuber`]'s doc for why
//!   it deviates from upstream's literal C++).

use super::{LatentHeat, PrandtlNumber};
use crate::genfoam::thermal_hydraulics::units::{
    HeatFlux, HeatTransferCoefficient, ReynoldsNumber,
};
use uom::si::angle::degree;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::Angle;
use uom::si::f64::{
    DynamicViscosity, Length, MassDensity, Pressure, Ratio, SurfaceTension, ThermalConductivity,
    ThermodynamicTemperature, Velocity,
};
use uom::si::heat_flux_density::watt_per_square_meter;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::surface_tension::newton_per_meter;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::velocity::meter_per_second;

/// Critical heat flux (CHF) — the wall heat flux at which nucleate boiling
/// gives way to film boiling (departure from nucleate boiling / dryout).
///
/// Only upstream's `constantCHF` is ported; `lookUpTableCHF` (3-D pressure /
/// mass-flux / quality table interpolation) is deferred — see the module doc.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CriticalHeatFlux {
    /// A fixed CHF value, read directly from phase properties upstream
    /// (`constantCHF`'s `value` dictionary entry).
    Constant(HeatFlux),
}

impl CriticalHeatFlux {
    /// The critical heat flux.
    #[must_use]
    pub fn value(&self) -> HeatFlux {
        match *self {
            Self::Constant(v) => v,
        }
    }
}

/// Leidenfrost (minimum film-boiling / rewetting) temperature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LeidenfrostTemperature {
    /// Groeneveld & Stewart (1982) correlation, as ported by TRACE and by
    /// GeN-Foam from it. Valid for `p < 9 MPa`; upstream notes it "lack[s]
    /// the term due to quality" (i.e. no flow-quality dependence is modelled)
    /// and linearly ramps to `T_sat` at the critical pressure above 9 MPa so
    /// the temperature is still defined (if not validated) there.
    GroeneveldStewart {
        /// Fluid critical pressure (Pa), used only above 9 MPa for the ramp.
        critical_pressure: Pressure,
    },
}

impl LeidenfrostTemperature {
    /// The Leidenfrost temperature `T_min`. `t_sat` is the local saturation
    /// temperature (used only in the `p >= 9 MPa` ramp).
    #[must_use]
    pub fn temperature(
        &self,
        pressure: Pressure,
        t_sat: ThermodynamicTemperature,
    ) -> ThermodynamicTemperature {
        match *self {
            Self::GroeneveldStewart { critical_pressure } => {
                let p = pressure.get::<pascal>();
                let p_mpa = p * 1e-6;
                const T_MIN_AT_9_MPA: f64 = 557.85 + 44.1 * 9.0 - 3.72 * 9.0 * 9.0;
                let t_min = if p < 9e6 {
                    557.85 + 44.1 * p_mpa - 3.72 * p_mpa * p_mpa
                } else {
                    let p_crit = critical_pressure.get::<pascal>();
                    let delta_t_min = T_MIN_AT_9_MPA - t_sat.get::<kelvin>();
                    t_sat.get::<kelvin>() + (p_crit - p) / (p_crit - 9e6) * delta_t_min
                };
                ThermodynamicTemperature::new::<kelvin>(t_min)
            }
        }
    }
}

/// Onset-of-nucleate-boiling (ONB) wall temperature — the wall superheat at
/// which the first vapour bubbles nucleate, below which flow boiling behaves
/// as single-phase forced convection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OnsetOfNucleateBoilingTemperature {
    /// Basu, Warrier & Dhir (2002) correlation, in terms of the local
    /// forced-convection heat-transfer coefficient (see
    /// [`OnsetOfNucleateBoilingTemperature::temperature`]).
    Basu {
        /// Liquid surface tension `sigma` (N/m).
        surface_tension: SurfaceTension,
        /// Liquid-structure contact angle (degrees).
        contact_angle: Angle,
    },
}

impl OnsetOfNucleateBoilingTemperature {
    /// The ONB temperature `T_ONB`.
    ///
    /// `htc_forced_convection_enhanced` is the (flow-enhancement-factor-
    /// corrected) two-phase forced-convection htc; `other_phase_density` and
    /// `latent_heat` are the vapour phase's density and the latent heat of
    /// vaporization; `thermal_conductivity` is the **liquid** phase's thermal
    /// conductivity.
    ///
    /// Implementation note: the Basu correlation is derived by solving a
    /// quadratic in `sqrt(deltaT_ONB,sat)` (a standard technique for ONB
    /// correlations, e.g. Bergles-Rohsenow / Frost-Dzakowic-style forms), so
    /// the algebra genuinely needs `sqrt` of a *temperature interval* as an
    /// intermediate step. `uom` has no ISQ quantity for "kelvin to the
    /// one-half power" (dimension exponents are integers), so that
    /// intermediate cannot be a typed `uom` quantity even though the overall
    /// expression recomposes to a valid temperature (squaring the sum of two
    /// square roots cancels the fractional exponent). This method therefore
    /// extracts plain kelvin magnitudes for that one step and re-wraps the
    /// result as a typed [`ThermodynamicTemperature`] at the end — the same
    /// "typed at the boundary, scalar in the core" pattern
    /// [`super::fs_drag::FsWallFriction::friction_factor`] already uses for
    /// its private `value()`.
    #[must_use]
    pub fn temperature(
        &self,
        htc_forced_convection_enhanced: HeatTransferCoefficient,
        t_fluid: ThermodynamicTemperature,
        t_sat: ThermodynamicTemperature,
        other_phase_density: MassDensity,
        latent_heat: LatentHeat,
        thermal_conductivity: ThermalConductivity,
    ) -> ThermodynamicTemperature {
        match *self {
            Self::Basu {
                surface_tension,
                contact_angle,
            } => {
                let theta = contact_angle.get::<uom::si::angle::radian>();
                let a = 2.0 * surface_tension.get::<newton_per_meter>()
                    / (1.0 - (-theta.powi(3) - 0.5 * theta).exp()).powi(2);

                let htc = htc_forced_convection_enhanced.get::<watt_per_square_meter_kelvin>();
                let t_sat_k = t_sat.get::<kelvin>();
                let t_fluid_k = t_fluid.get::<kelvin>();
                let rho_other = other_phase_density.get::<kilogram_per_cubic_meter>();
                let l = latent_heat.get::<joule_per_kilogram>().abs();
                let k = thermal_conductivity.get::<watt_per_meter_kelvin>();

                let delta_t_onb_sat = a * htc * t_sat_k / (rho_other * l * k);
                let inner = (delta_t_onb_sat + 4.0 * (t_sat_k - t_fluid_k)).max(0.0);
                let t_onb_k = t_fluid_k + 0.25 * (delta_t_onb_sat.sqrt() + inner.sqrt()).powi(2);
                ThermodynamicTemperature::new::<kelvin>(t_onb_k)
            }
        }
    }
}

/// Two-phase flow-enhancement factor `F` — multiplies the single-phase
/// forced-convection htc to account for bubble-induced turbulence and the
/// velocity increase from vapour generation.
///
/// Evaluate with [`FlowEnhancementFactor::value`], which takes a
/// [`FlowEnhancementInputs`] carrying every driving quantity any variant
/// might need (only the fields the active variant actually reads are used —
/// see each variant's doc).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlowEnhancementFactor {
    /// Chen (1966), keyed to the inverse Lockhart-Martinelli parameter
    /// `1/X_tt`. Uses [`FlowEnhancementInputs::inverse_lockhart_martinelli`].
    Chen {
        /// Maximum allowed value of `F` (upstream `maxValue_`, a
        /// `flowEnhancementFactorModel` base-class dictionary entry).
        max_value: Ratio,
    },
    /// Rezkallah & Sims (1987), keyed to the liquid void fraction; TRACE's
    /// numerically-preferred default over Chen at low pressure/flow (see the
    /// upstream doc comment, citing NRC ML120060218, pp. 263-267). Uses
    /// [`FlowEnhancementInputs::liquid_void_fraction`].
    RezkallahSims {
        /// Correlation exponent (upstream dictionary entry `exp`).
        exponent: f64,
        /// Maximum allowed value of `F`.
        max_value: Ratio,
    },
    /// COBRA-TF, keyed to the ratio of a two-phase "mixture" Reynolds number
    /// to the single-phase Reynolds number (see
    /// [`two_phase_mixture_reynolds_number`]). Uses
    /// [`FlowEnhancementInputs::mixture_reynolds_number`] and
    /// [`FlowEnhancementInputs::single_phase_reynolds_number`].
    CobraTf {
        /// Maximum allowed value of `F`.
        max_value: Ratio,
    },
}

/// Every driving quantity [`FlowEnhancementFactor::value`] might need; the
/// active variant reads only the field(s) documented on it.
#[derive(Debug, Clone, Copy)]
pub struct FlowEnhancementInputs {
    /// Inverse Lockhart-Martinelli parameter `1/X_tt` (dimensionless). Used
    /// by [`FlowEnhancementFactor::Chen`].
    pub inverse_lockhart_martinelli: Ratio,
    /// Liquid void fraction `alpha_l` (dimensionless). Used by
    /// [`FlowEnhancementFactor::RezkallahSims`].
    pub liquid_void_fraction: Ratio,
    /// Two-phase mixture Reynolds number (see
    /// [`two_phase_mixture_reynolds_number`]). Used by
    /// [`FlowEnhancementFactor::CobraTf`].
    pub mixture_reynolds_number: ReynoldsNumber,
    /// Single-phase Reynolds number of the phase this factor multiplies
    /// (upstream `pair_.Re()`). Used by [`FlowEnhancementFactor::CobraTf`].
    pub single_phase_reynolds_number: ReynoldsNumber,
}

impl FlowEnhancementFactor {
    /// Evaluate `F`.
    #[must_use]
    pub fn value(&self, inputs: FlowEnhancementInputs) -> Ratio {
        let clamped =
            |v: f64, max_value: Ratio| Ratio::new::<ratio>(v.min(max_value.get::<ratio>()));
        match *self {
            Self::Chen { max_value } => {
                let inv_x = inputs.inverse_lockhart_martinelli.get::<ratio>();
                if inv_x > 0.100_207_179_8 {
                    clamped(2.35 * (0.213 + inv_x).powf(0.736), max_value)
                } else {
                    Ratio::new::<ratio>(1.0)
                }
            }
            Self::RezkallahSims {
                exponent,
                max_value,
            } => {
                let alpha_l = inputs.liquid_void_fraction.get::<ratio>().max(1e-3);
                clamped(alpha_l.powf(-exponent), max_value)
            }
            Self::CobraTf { max_value } => {
                let re_mix = inputs.mixture_reynolds_number.get::<ratio>();
                let re_1p = inputs.single_phase_reynolds_number.get::<ratio>();
                let f = (re_mix / re_1p).powf(0.8).max(1.0);
                clamped(f, max_value)
            }
        }
    }
}

/// Pool-boiling suppression factor `S` — multiplies the pool-boiling htc to
/// "turn it off" as the flow regime transitions towards annular / film
/// boiling and pool boiling becomes physically implausible.
///
/// Evaluate with [`SuppressionFactor::value`], which takes a
/// [`SuppressionInputs`]; as with [`FlowEnhancementFactor`], only the
/// field(s) the active variant reads are used.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SuppressionFactor {
    /// Chen (1966), `S = 1 / (1 + 2.53e-6 * Re_mix^1.17)`, using the same
    /// two-phase mixture Reynolds number as
    /// [`FlowEnhancementFactor::CobraTf`] (see
    /// [`two_phase_mixture_reynolds_number`]).
    Chen,
    /// COBRA-TF, a linear temperature-ratio clamp
    /// `S = clamp((T_fluid - T_sat) / (T_wall - T_sat), 0, 1)`.
    CobraTf,
}

/// Every driving quantity [`SuppressionFactor::value`] might need.
#[derive(Debug, Clone, Copy)]
pub struct SuppressionInputs {
    /// Two-phase mixture Reynolds number. Used by [`SuppressionFactor::Chen`].
    pub mixture_reynolds_number: ReynoldsNumber,
    /// Bulk fluid temperature. Used by [`SuppressionFactor::CobraTf`].
    pub t_fluid: ThermodynamicTemperature,
    /// Wall temperature. Used by [`SuppressionFactor::CobraTf`].
    pub t_wall: ThermodynamicTemperature,
    /// Saturation temperature. Used by [`SuppressionFactor::CobraTf`].
    pub t_sat: ThermodynamicTemperature,
}

impl SuppressionFactor {
    /// Evaluate `S`.
    #[must_use]
    pub fn value(&self, inputs: SuppressionInputs) -> Ratio {
        match *self {
            Self::Chen => {
                let re_mix = inputs.mixture_reynolds_number.get::<ratio>();
                Ratio::new::<ratio>(1.0 / (1.0 + 2.53e-6 * re_mix.powf(1.17)))
            }
            Self::CobraTf => {
                let t_sat = inputs.t_sat.get::<kelvin>();
                let num = (inputs.t_fluid.get::<kelvin>() - t_sat).max(0.0);
                let den = (inputs.t_wall.get::<kelvin>() - t_sat).max(1e-3);
                Ratio::new::<ratio>((num / den).min(1.0))
            }
        }
    }
}

/// Two-phase "mixture" Reynolds number shared by
/// [`FlowEnhancementFactor::CobraTf`] and [`SuppressionFactor::Chen`]:
/// `Re_mix = max(|alpha_1*rho_1*u_1 + alpha_2*rho_2*u_2| * D_h /
/// (alpha_1*mu_1 + alpha_2*mu_2), 10)` — see the module doc for the
/// vector-to-scalar velocity note.
///
/// Takes both phases' void fraction, density, velocity, and viscosity plus the
/// hydraulic diameter (nine dimensioned inputs) — this is the mixture Reynolds
/// number's intrinsic argument list, so `too_many_arguments` is allowed here.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn two_phase_mixture_reynolds_number(
    alpha_1: Ratio,
    rho_1: MassDensity,
    u_1: Velocity,
    mu_1: DynamicViscosity,
    alpha_2: Ratio,
    rho_2: MassDensity,
    u_2: Velocity,
    mu_2: DynamicViscosity,
    hydraulic_diameter: Length,
) -> ReynoldsNumber {
    let a1 = alpha_1.get::<ratio>();
    let a2 = alpha_2.get::<ratio>();
    let momentum_flux =
        (a1 * rho_1.get::<kilogram_per_cubic_meter>() * u_1.get::<meter_per_second>()
            + a2 * rho_2.get::<kilogram_per_cubic_meter>() * u_2.get::<meter_per_second>())
        .abs();
    let mixture_viscosity = a1 * mu_1.get::<pascal_second>() + a2 * mu_2.get::<pascal_second>();
    let re = momentum_flux * hydraulic_diameter.get::<meter>() / mixture_viscosity;
    ReynoldsNumber::new::<ratio>(re.max(10.0))
}

/// Fraction of the "would-be" pool-boiling heat flux that actually results in
/// net vapour generation during sub-cooled (bulk liquid below `T_sat`)
/// boiling — `0` means all of it re-condenses into the sub-cooled bulk, `1`
/// means all of it nets vapour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SubcooledBoilingFraction {
    /// A fixed fraction, independent of the local heat flux.
    Constant {
        /// The fixed fraction (upstream dictionary entry `value`).
        value: Ratio,
    },
    /// Saha & Zuber (1974) point-of-net-vapour-generation correlation.
    SahaZuber,
}

impl SubcooledBoilingFraction {
    /// Evaluate the fraction. `heat_flux` is the local wall heat flux driving
    /// bubble growth; `thermal_conductivity`, `re`, `pr` are the liquid
    /// phase's properties.
    ///
    /// **Deviation from upstream's literal C++, documented:** Saha & Zuber's
    /// published correlation for the sub-cooling `Delta T_sub,d` at the point
    /// of net vapour generation is `Delta T_sub,d = q'' * D_h / (0.0065 * k *
    /// Pe)` for `Pe = Re*Pr > 70000` (`k` the liquid thermal conductivity).
    /// GeN-Foam's `SahaZuber::value()` computes `qi*Dh/(0.0065*max(Re*Pr,
    /// 7e4))` — **without** the `k` division. That literal expression has
    /// units of `W/m` (a heat flux times a length), not kelvin, and cannot be
    /// subtracted from a temperature; `uom` (correctly) refuses to compile
    /// it. This port includes the `k` division, matching the published Saha &
    /// Zuber (1974) formula, and is verified against that publication
    /// directly rather than against upstream's (dimensionally invalid) C++
    /// output — see the V&V test for the reproduction check.
    ///
    /// The Saha–Zuber subcooled-boiling fraction is a function of eight
    /// dimensioned local field values, so `too_many_arguments` is allowed here.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn fraction(
        &self,
        heat_flux: HeatFlux,
        thermal_conductivity: ThermalConductivity,
        hydraulic_diameter: Length,
        re: ReynoldsNumber,
        pr: PrandtlNumber,
        t_sat: ThermodynamicTemperature,
        t_fluid: ThermodynamicTemperature,
    ) -> Ratio {
        match *self {
            Self::Constant { value } => value,
            Self::SahaZuber => {
                let q = heat_flux.get::<watt_per_square_meter>();
                let k = thermal_conductivity.get::<watt_per_meter_kelvin>();
                let dh = hydraulic_diameter.get::<meter>();
                let peclet = (re.get::<ratio>() * pr.get::<ratio>()).max(7e4);
                let t_sat_k = t_sat.get::<kelvin>();
                let t_ld = (t_sat_k - q * dh / (0.0065 * k * peclet)).max(0.0);
                let denom = (t_sat_k - t_ld).max(1e-3);
                let frac = ((t_fluid.get::<kelvin>() - t_ld) / denom).clamp(0.0, 1.0);
                Ratio::new::<ratio>(frac)
            }
        }
    }
}

/// Convenience: build a [`SurfaceTension`]/[`Angle`] pair's contact angle
/// from degrees, matching upstream's dictionary entry
/// (`dict.get<scalar>("contactAngle")` in degrees, converted internally).
#[must_use]
pub fn contact_angle_from_degrees(degrees: f64) -> Angle {
    Angle::new::<degree>(degrees)
}
