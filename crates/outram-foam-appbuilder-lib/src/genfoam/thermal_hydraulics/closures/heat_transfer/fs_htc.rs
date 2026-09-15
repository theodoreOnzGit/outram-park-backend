// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/heatTransferModels/
//             FSHeatTransferCoefficientModels/{Nusselt,NusseltAndWall,Shah,Gorenflo}/
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream authors: Stefan Radman (EPFL), Thomas Guilbaud (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `fs_htc` — fluid-structure forced-convection & pool-boiling HTC correlations
//!
//! Two closed-enum families, each a faithful port of one GeN-Foam
//! `FSHeatTransferCoefficientModel` sub-family:
//!
//! | Enum | Upstream classes | Physical role |
//! |---|---|---|
//! | [`FsForcedConvectionHtc`] | `Nusselt`, `NusseltAndWall` | Single-/two-phase forced convection between the fluid and the wall |
//! | [`PoolBoilingHtc`] | `Shah`, `Gorenflo` | Saturated pool-boiling heat transfer at the wall |
//!
//! Both families are ultimately Nusselt-number correlations
//! `Nu = A + B * Re^C * Pr^D * (...)`; the conversion to a heat-transfer
//! coefficient is always `h = Nu * k / D_h` (`k` the fluid thermal
//! conductivity, `D_h` the hydraulic diameter) — the classic definition of
//! the Nusselt number. Each `impl` documents exactly which formula it
//! evaluates so the conversion is visible at the call site, not buried.
//!
//! ## References
//!
//! - Dittus, F.W. and Boelter, L.M.K., *University of California Publications
//!   in Engineering*, Vol. 2, 1930 (the archetype this family generalises).
//! - Walton, C.M., "Thermal-hydraulic loss coefficients for CANDU fuel
//!   bundles", 1992, eq. (10) "Wolf-McCarthy II" (the wall/fluid
//!   temperature-ratio example in `Nusselt`'s upstream doc comment).
//! - N.Z. Shah, pool-boiling correlation for liquid metals (sodium, Na-K),
//!   as used by GeN-Foam's `Shah` model.
//! - D. Gorenflo, "Pool Boiling," *VDI-Heat Atlas*, Sect. Ha, VDI-Verlag,
//!   Dusseldorf, 1993, via J.G. Collier and J.R. Thome, *Convective Boiling
//!   and Condensation*, 3rd ed., pp. 155-158, Oxford University Press, 1994.

use super::PrandtlNumber;
use crate::genfoam::thermal_hydraulics::units::{HeatFlux, HeatTransferCoefficient, ReynoldsNumber};
use uom::si::f64::{
    Length, Pressure, TemperatureInterval, ThermalConductivity, ThermodynamicTemperature,
};
use uom::si::heat_flux_density::watt_per_square_meter;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::temperature_interval::kelvin as kelvin_interval;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

/// Fluid-structure forced-convection heat-transfer coefficient.
///
/// Both variants are Nusselt-number correlations of the form
/// `Nu = A + B * Re^C * Pr^D`, converted to `h = Nu * k / D_h`. They differ in
/// what happens after that:
///
/// - [`FsForcedConvectionHtc::Nusselt`] optionally multiplies `Nu` by a
///   wall/fluid temperature-ratio term `(T_w / T_f)^E` before the conversion
///   (upstream `Nusselt`, `TypeName("NusseltReynoldsPrandtlPower")`).
/// - [`FsForcedConvectionHtc::NusseltAndWall`] has no temperature-ratio term,
///   but instead combines the resulting fluid-side `h` **in series** with a
///   fixed wall-resistance coefficient `H_wall`
///   (`1/H = 1/(Nu*k/D_h) + 1/H_wall`), modelling e.g. a cladding oxide layer
///   or contact resistance (upstream `NusseltAndWall`).
///
/// Evaluate with [`FsForcedConvectionHtc::heat_transfer_coefficient`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FsForcedConvectionHtc {
    /// `Nu = A + B * Re^C * Pr^D * (T_w / T_f)^E`. `wall_fluid_temperatures`
    /// is read only when `e != 0.0`; pass `None` (or `e = 0.0`) to omit the
    /// temperature-ratio term entirely, matching upstream's `E_` defaulting to
    /// `0` and its `Twall_[celli] > 0.0` guard.
    Nusselt {
        /// Additive (fully-developed-laminar-limit) constant `A`.
        a: f64,
        /// Leading coefficient `B` on the `Re^C Pr^D` term. If `0.0`, the
        /// whole `Re`/`Pr`/temperature-ratio term is skipped (matches
        /// upstream's `B_ != 0` guard) and `Nu = A`.
        b: f64,
        /// Reynolds-number exponent `C`.
        c: f64,
        /// Prandtl-number exponent `D`.
        d: f64,
        /// Wall/fluid temperature-ratio exponent `E` (`0.0` disables the
        /// term).
        e: f64,
    },
    /// `Nu = A + B * Re^C * Pr^D`, combined in series with a fixed
    /// wall-resistance coefficient `h_wall` via `h = h_fluid*h_wall /
    /// (h_fluid + h_wall)`.
    NusseltAndWall {
        /// Additive constant `A`.
        a: f64,
        /// Leading coefficient `B`. `0.0` skips the `Re`/`Pr` term (`Nu = A`).
        b: f64,
        /// Reynolds-number exponent `C`.
        c: f64,
        /// Prandtl-number exponent `D`.
        d: f64,
        /// Fixed wall-resistance heat-transfer coefficient `H_wall`
        /// (upstream dictionary entry `addH`).
        h_wall: HeatTransferCoefficient,
    },
}

impl FsForcedConvectionHtc {
    /// Evaluate the forced-convection heat-transfer coefficient.
    ///
    /// `re`, `pr` are the local (superficial, in two-phase) Reynolds and
    /// Prandtl numbers; `k` the fluid thermal conductivity; `d_h` the
    /// hydraulic diameter. `wall_fluid_temperatures = Some((T_wall,
    /// T_fluid))` supplies the temperature-ratio term for
    /// [`FsForcedConvectionHtc::Nusselt`] with `e != 0.0`; it is ignored by
    /// [`FsForcedConvectionHtc::NusseltAndWall`] (upstream has no such term
    /// there) and by a [`FsForcedConvectionHtc::Nusselt`] with `e == 0.0`.
    ///
    /// Faithful translation of each upstream model's `value(celli)`,
    /// including the `usePeclet_` fast path (`C == D` collapses
    /// `Re^C * Pr^D` to `(Re*Pr)^C`, i.e. `Pe^C`) and the `B == 0` early-out
    /// (`Nu = A`, no `pow()` calls at all).
    #[must_use]
    pub fn heat_transfer_coefficient(
        &self,
        re: ReynoldsNumber,
        pr: PrandtlNumber,
        k: ThermalConductivity,
        d_h: Length,
        wall_fluid_temperatures: Option<(ThermodynamicTemperature, ThermodynamicTemperature)>,
    ) -> HeatTransferCoefficient {
        let re_v = re.get::<ratio>();
        let pr_v = pr.get::<ratio>();
        let k_v = k.get::<watt_per_meter_kelvin>();
        let dh_v = d_h.get::<meter>();

        match *self {
            Self::Nusselt { a, b, c, d, e } => {
                let mut t_ratio = 1.0_f64;
                if e != 0.0 {
                    if let Some((t_wall, t_fluid)) = wall_fluid_temperatures {
                        let tw = t_wall.get::<kelvin>();
                        if tw > 0.0 {
                            t_ratio = (tw / t_fluid.get::<kelvin>()).powf(e);
                        }
                    }
                }
                let nu = if b != 0.0 {
                    a + b * re_pow_c_pr_pow_d(re_v, pr_v, c, d) * t_ratio
                } else {
                    a
                };
                HeatTransferCoefficient::new::<watt_per_square_meter_kelvin>(nu * k_v / dh_v)
            }
            Self::NusseltAndWall { a, b, c, d, h_wall } => {
                let nu = if b != 0.0 {
                    a + b * re_pow_c_pr_pow_d(re_v, pr_v, c, d)
                } else {
                    a
                };
                let h_fluid =
                    HeatTransferCoefficient::new::<watt_per_square_meter_kelvin>(nu * k_v / dh_v);
                (h_fluid * h_wall) / (h_fluid + h_wall)
            }
        }
    }
}

/// `Re^C * Pr^D`, using the `usePeclet_` fast path `(Re*Pr)^C` when `C == D`
/// (`Re*Pr = Pe`, the Peclet number) exactly as upstream precomputes at
/// construction time (`usePeclet_(C_ == D_)`).
fn re_pow_c_pr_pow_d(re: f64, pr: f64, c: f64, d: f64) -> f64 {
    if c == d {
        (re * pr).powf(c)
    } else {
        re.powf(c) * pr.powf(d)
    }
}

/// Saturated pool-boiling heat-transfer coefficient.
///
/// Both variants have the closed form `h_PB = C(pR) * q''^n * pR^m` for some
/// pressure-dependent coefficients, but `q''` is itself `h_PB * deltaT` — the
/// heat flux depends on the coefficient being solved for. Upstream breaks
/// this circularity by assuming `h ~ h_PB` (pool boiling dominates the total
/// wall heat transfer) and solving `h_PB = (C * deltaT^n * pR^m)^(1/(1-n))`
/// directly in terms of the wall/reference temperature difference — this is
/// [`PoolBoilingHtc::heat_transfer_coefficient_from_delta_t`]. Both models
/// also support skipping that assumption when the heat flux is already known
/// from elsewhere (upstream's `useExplicitHeatFlux_` dictionary option) —
/// [`PoolBoilingHtc::heat_transfer_coefficient_from_heat_flux`] evaluates the
/// original (non-circular) `h_PB = C * q''^n * pR^m` directly. Reduced
/// pressure `pR = p / p_crit` uses a **fixed** critical pressure per variant
/// (matching upstream, which hardcodes it rather than reading it from a
/// dictionary): `p_crit = 35 MPa` for [`PoolBoilingHtc::Shah`] ("specific to
/// Sodium" per upstream) and `p_crit = 22.09 MPa` for
/// [`PoolBoilingHtc::Gorenflo`] ("specific to Water").
///
/// Both methods return `0 W/(m^2 K)` when the driving quantity (`deltaT` or
/// `q''`) is non-positive, matching upstream's `if (deltaT > 0.0) ... else
/// return 0.0;` guard.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PoolBoilingHtc {
    /// Shah pool-boiling correlation, `p_crit = 35 MPa` (sodium). No
    /// configurable parameters — all coefficients (`C`, `m`, `n`) are fixed
    /// constants upstream, linearly blended across `pR in (5e-4, 1.5e-3)` to
    /// avoid the discontinuity Shah's original piecewise `pR = 1e-3` cutoff
    /// would otherwise introduce.
    Shah,
    /// Gorenflo (1993) pool-boiling correlation, `p_crit = 22.09 MPa`
    /// (water).
    Gorenflo {
        /// Absolute surface roughness `R` (metres). Upstream dictionary entry
        /// `absoluteSurfaceRoughness`, defaulting to the reference roughness
        /// `R0 = 4e-7 m` (i.e. `roughness_factor = (R/R0)^0.133 = 1`).
        surface_roughness: Length,
    },
}

impl PoolBoilingHtc {
    /// `h_PB = (C(pR) * deltaT^n * pR^m)^(1/(1-n))` for [`PoolBoilingHtc::Shah`],
    /// or `h_PB = (h0 * A * F(pR) * (deltaT/q0)^n)^(1/(1-n))` for
    /// [`PoolBoilingHtc::Gorenflo`] — see the enum doc for the assumption
    /// (`h ~ h_PB`) both formulas rely on. `deltaT` is the wall-to-reference
    /// temperature difference driving boiling: `T_wall - T_fluid` for Shah,
    /// `T_wall - T_sat` for Gorenflo (matching each upstream model's own
    /// reference field). `pressure` is the local system pressure.
    #[must_use]
    pub fn heat_transfer_coefficient_from_delta_t(
        &self,
        delta_t: TemperatureInterval,
        pressure: Pressure,
    ) -> HeatTransferCoefficient {
        let dt = delta_t.get::<kelvin_interval>();
        if dt <= 0.0 {
            return HeatTransferCoefficient::new::<watt_per_square_meter_kelvin>(0.0);
        }
        let p_r = reduced_pressure(*self, pressure);
        let h = match *self {
            Self::Shah => {
                let (c, m) = shah_interpolated_coefficients(p_r);
                (c * p_r.powf(m) * dt.powf(SHAH_N)).powf(SHAH_INV_ONE_MINUS_N)
            }
            Self::Gorenflo { surface_roughness } => {
                let a = gorenflo_roughness_factor(surface_roughness);
                let (f, n) = gorenflo_f_and_n(p_r);
                (GORENFLO_H0 * a * f * (dt / GORENFLO_Q0).powf(n)).powf(1.0 / (1.0 - n))
            }
        };
        HeatTransferCoefficient::new::<watt_per_square_meter_kelvin>(h)
    }

    /// The non-circular form `h_PB = C(pR) * q''^n * pR^m` (Shah) or
    /// `h_PB = h0 * F(pR) * (q''/q0)^n * A` (Gorenflo), used when the wall
    /// heat flux `q''` is already known (upstream's `useExplicitHeatFlux_ =
    /// true` dictionary option).
    #[must_use]
    pub fn heat_transfer_coefficient_from_heat_flux(
        &self,
        heat_flux: HeatFlux,
        pressure: Pressure,
    ) -> HeatTransferCoefficient {
        let q = heat_flux.get::<watt_per_square_meter>();
        if q <= 0.0 {
            return HeatTransferCoefficient::new::<watt_per_square_meter_kelvin>(0.0);
        }
        let p_r = reduced_pressure(*self, pressure);
        let h = match *self {
            Self::Shah => {
                let (c, m) = shah_interpolated_coefficients(p_r);
                c * q.powf(SHAH_N) * p_r.powf(m)
            }
            Self::Gorenflo { surface_roughness } => {
                let a = gorenflo_roughness_factor(surface_roughness);
                let (f, n) = gorenflo_f_and_n(p_r);
                GORENFLO_H0 * f * (q / GORENFLO_Q0).powf(n) * a
            }
        };
        HeatTransferCoefficient::new::<watt_per_square_meter_kelvin>(h)
    }
}

/// Shah's fixed pool-boiling exponent `n = 0.7` (same for both the
/// heat-flux and delta-T forms).
const SHAH_N: f64 = 0.7;
/// `1 / (1 - n)`, the outer exponent of the delta-T form.
const SHAH_INV_ONE_MINUS_N: f64 = 1.0 / (1.0 - SHAH_N);
/// Shah's critical pressure, Pa (35 MPa, "specific to Sodium" per upstream).
const SHAH_P_CRIT: f64 = 3.5e7;

/// Gorenflo's reference heat-transfer coefficient `h0`, W/(m^2 K).
const GORENFLO_H0: f64 = 5600.0;
/// Gorenflo's reference heat flux `q0`, W/m^2.
const GORENFLO_Q0: f64 = 20_000.0;
/// Gorenflo's reference (roughness-normalisation) absolute roughness `R0`, m.
const GORENFLO_R0: f64 = 4e-7;
/// Gorenflo's critical pressure, Pa (22.09 MPa, "specific to Water").
const GORENFLO_P_CRIT: f64 = 2.209e7;

/// `pR = p / p_crit` for the variant's fixed critical pressure.
fn reduced_pressure(model: PoolBoilingHtc, pressure: Pressure) -> f64 {
    let p_crit = match model {
        PoolBoilingHtc::Shah => SHAH_P_CRIT,
        PoolBoilingHtc::Gorenflo { .. } => GORENFLO_P_CRIT,
    };
    pressure.get::<pascal>() / p_crit
}

/// Shah's `(C, m)` pair, linearly interpolated across `pR in (pR0, pR0 +
/// deltaPR) = (5e-4, 1.5e-3)` to avoid the discontinuity of Shah's original
/// piecewise cutoff at `pR = 1e-3` (`C = 13.7, m = 0.22` below the cutoff;
/// `C = 6.9, m = 0.12` above it). Faithful port of upstream's `Shah::value`
/// blend.
fn shah_interpolated_coefficients(p_r: f64) -> (f64, f64) {
    const P_R0: f64 = 5e-4;
    const DELTA_P_R: f64 = 1e-3;
    let f = ((p_r - P_R0) / DELTA_P_R).clamp(0.0, 1.0);
    let c = f * 6.9 + (1.0 - f) * 13.7;
    let m = f * 0.12 + (1.0 - f) * 0.22;
    (c, m)
}

/// Gorenflo's absolute-surface-roughness correction `A = (R / R0)^0.133`.
fn gorenflo_roughness_factor(surface_roughness: Length) -> f64 {
    (surface_roughness.get::<meter>() / GORENFLO_R0).powf(0.133)
}

/// Gorenflo's pressure-dependent normalised heat-transfer function `F(pR)`
/// and boiling exponent `n(pR)`:
/// `F = 1.73*pR^0.27 + (6.1 + 0.68/(1-pR))*pR^2`, `n = 0.9 - 0.3*pR^0.15`.
/// By construction `F(0.1) ~= 1` — the correlation's reference state is
/// `pR = 0.1`, `q'' = q0`, giving `h_PB ~= h0` there (see the V&V tests).
fn gorenflo_f_and_n(p_r: f64) -> (f64, f64) {
    let f = 1.73 * p_r.powf(0.27) + (6.1 + 0.68 / (1.0 - p_r)) * p_r * p_r;
    let n = 0.9 - 0.3 * p_r.powf(0.15);
    (f, n)
}
