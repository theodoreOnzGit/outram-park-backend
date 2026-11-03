// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! Steam + non-condensable gas mixtures, by Dalton's law (gh:#1093, part of
//! the MELCOR-style control-volume epic gh:#1091).
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! A gas space holds steam and any of six non-condensables
//! ([`NonCondensable`]: N2, H2, O2, CO, CO2, Ar) in one volume at one
//! temperature. Each component fills the whole volume at its own partial
//! pressure (Dalton); the total pressure is their sum.
//!
//! - **Non-condensables** are ideal gases, `p_i = n_i R T / V`. Their `c_v(T)`
//!   and internal energy (from 0 K) come from MELCOR's NCG fits for N2 and H2
//!   ([`melcor_ncg`], moved verbatim from `sembawang`) and from the NASA
//!   7-coefficient polynomials of GRI-Mech 3.0 for O2, CO, CO2 and Ar
//!   ([`nasa7`]); [`NcgHeatCapacity`] selects, with the MELCOR-where-available
//!   choice the default.
//! - **Steam** is IAPWS-IF97 region 2 at its partial pressure, `ρ_s = 1/v(T,
//!   p_s)`, solved for `p_s` by Newton (the default, [`SteamModel::Iapws97`]),
//!   or `sembawang`'s ideal steam with a constant `c_v` referenced to IF97's
//!   saturated vapour at 100 °C ([`SteamModel::IdealConstantCv`], the
//!   ablation: `u = 2.506e6 + 1500 (T − 373.15)` J/kg).
//! - **Energy references differ by component** (non-condensables from 0 K,
//!   IF97 steam from the triple-point liquid). "From 0 K" means `u(0) = 0`
//!   with `c_v(T_low) T` below each fit's lower bound (`sembawang`'s
//!   convention), **not** JANAF's `H°(T) − H°(0)`: at 298.15 K the molar
//!   enthalpies differ from JANAF by −896 J/mol (H2, MELCOR fit), +518 (CO2),
//!   +83 (N2, MELCOR fit), +14 (CO), +12 (O2), +0.4 (Ar) (gh:#1098 item 18,
//!   measured 2026-11-01). Energy changes inside a fit's range are
//!   unaffected; anything anchored to formation enthalpies must anchor at
//!   298.15 K, as `sembawang::bwr_mark1::reaction` does, never at 0 K. Only energy *changes* and
//!   enthalpies carried on the same references are meaningful, as in
//!   `sembawang`; a control volume using this keeps them consistent
//!   (audited 2026-11-01, gh:#1098 item 2: `tampines::control_volume` and
//!   `tampines::cv_network` move each species with its own reference, pinned
//!   by `cv_network::tests::gas_flux_keeps_each_species_on_its_own_energy_reference`).
//!
//! Steam above its saturation pressure at `T` is reported, not condensed:
//! condensing it (and where the liquid goes) is the control volume's job.
//!
//! **Steam colder than the triple point is refused, on purpose** (maintainer
//! decision 2026-11-02, gh:#1115: "if steam goes below 0C shld crash, not
//! quite physical"): IF97 region 2 has no state below 273.15 K, physically the
//! vapour would deposit as ice or condense as fog, and a gas space that cold
//! points to an upstream problem. Metastable supercooled vapour, ice/fog and an
//! ideal-steam fallback were considered and rejected; see
//! [`STEAM_BELOW_TRIPLE_POINT`].

pub mod melcor_ncg;
pub mod nasa7;

use uom::si::amount_of_substance::mole;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::energy::joule;
use uom::si::f64::{
    AmountOfSubstance, Energy, HeatCapacity, Mass, Pressure, ThermodynamicTemperature, Volume,
};
use uom::si::heat_capacity::joule_per_kelvin;
use uom::si::mass::kilogram;
use uom::si::pressure::pascal;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::volume::cubic_meter;

use crate::region_2_vapour::{u_tp_2, v_tp_2};
use crate::region_4_vap_liq_equilibrium::{sat_pressure_4, sat_temp_4};

pub use melcor_ncg::{M_H2O, R_GAS};

/// A non-condensable gas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NonCondensable {
    /// Nitrogen.
    N2,
    /// Hydrogen.
    H2,
    /// Oxygen.
    O2,
    /// Carbon monoxide.
    CO,
    /// Carbon dioxide.
    CO2,
    /// Argon.
    Ar,
}

impl NonCondensable {
    /// Every non-condensable, in the order of [`GasMixture::ncg_moles`].
    pub const ALL: [NonCondensable; 6] = [Self::N2, Self::H2, Self::O2, Self::CO, Self::CO2, Self::Ar];

    /// Index in [`GasMixture::ncg_moles`].
    pub fn index(self) -> usize {
        match self {
            Self::N2 => 0,
            Self::H2 => 1,
            Self::O2 => 2,
            Self::CO => 3,
            Self::CO2 => 4,
            Self::Ar => 5,
        }
    }

    /// Molar mass, kg/mol. N2 and H2 are `sembawang`'s values (kept so the
    /// moved fits stay bit-identical); O2, CO, CO2 and Ar from the IUPAC
    /// standard atomic weights (C 12.011, O 15.999, Ar 39.948).
    pub fn molar_mass(self) -> f64 {
        match self {
            Self::N2 => melcor_ncg::M_N2,
            Self::H2 => melcor_ncg::M_H2,
            Self::O2 => 0.031_998,
            Self::CO => 0.028_010,
            Self::CO2 => 0.044_009,
            Self::Ar => 0.039_948,
        }
    }
}

/// Which `c_v(T)` the non-condensables use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NcgHeatCapacity {
    /// MELCOR's NCG fits for N2 and H2, NASA-7 (GRI-Mech 3.0) for the rest.
    /// The default.
    #[default]
    MelcorWhereAvailable,
    /// NASA-7 (GRI-Mech 3.0) for every gas.
    Nasa7,
    /// Constants (the ablation): N2 20.8 and H2 20.5 J/(mol K) as
    /// `sembawang`'s, the others their NASA-7 `c_v` at 300 K.
    Constant,
}

/// How steam in the gas is closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SteamModel {
    /// IAPWS-IF97 region 2 at the partial pressure. The default.
    #[default]
    Iapws97,
    /// `sembawang`'s ideal steam: `p_s = m R_s T / V`, `u = 2.506e6 + 1500
    /// (T − 373.15)` J/kg (the ablation).
    IdealConstantCv,
}

/// `sembawang`'s ideal-steam constants (`bwr_mark1::containment`).
const CV_STEAM_IDEAL: f64 = 1500.0;
const U_STEAM_REF_IDEAL: f64 = 2.5060e6;
const T_REF_IDEAL: f64 = 373.15;

/// Why the closure refuses steam below IF97 region 2's 273.15 K lower bound
/// (gh:#1115, maintainer decision 2026-11-02). Part of the error message of
/// [`GasMixtureModel::state`] and [`GasMixtureModel::temperature`].
pub const STEAM_BELOW_TRIPLE_POINT: &str = "steam below the triple point (273.16 K): no IF97 state; \
physically the vapour would condense to fog or ice — a state this cold indicates an upstream \
problem (gh:#1115)";

/// The contents of a gas space.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GasMixture {
    /// Moles of each non-condensable, in [`NonCondensable::ALL`] order.
    pub ncg_moles: [f64; 6],
    /// Mass of steam, kg.
    pub steam_kg: f64,
}

impl GasMixture {
    /// A mixture of `steam` and the listed non-condensables.
    pub fn new(steam: Mass, ncg: &[(NonCondensable, AmountOfSubstance)]) -> Self {
        let mut m = Self { steam_kg: steam.get::<kilogram>(), ..Self::default() };
        for (g, n) in ncg {
            m.ncg_moles[g.index()] += n.get::<mole>();
        }
        m
    }

    /// Moles of `gas`.
    pub fn moles(&self, gas: NonCondensable) -> AmountOfSubstance {
        AmountOfSubstance::new::<mole>(self.ncg_moles[gas.index()])
    }

    /// Total non-condensable moles.
    pub fn ncg_total_moles(&self) -> f64 {
        self.ncg_moles.iter().sum()
    }

    /// Total mass, kg.
    pub fn total_mass(&self) -> Mass {
        let ncg: f64 = NonCondensable::ALL.iter().map(|g| self.ncg_moles[g.index()] * g.molar_mass()).sum();
        Mass::new::<kilogram>(self.steam_kg + ncg)
    }
}

/// The state of a gas space at `(V, T)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GasMixtureState {
    /// Total pressure.
    pub pressure: Pressure,
    /// Partial pressure of steam.
    pub steam_partial_pressure: Pressure,
    /// Partial pressure of each non-condensable, [`NonCondensable::ALL`] order, Pa.
    pub ncg_partial_pressures: [f64; 6],
    /// Internal energy of the gas (references as in the module docs).
    pub internal_energy: Energy,
    /// `p_s / p_sat(T)`; above 1 the steam is supersaturated (metastable
    /// region 2; the caller should condense it).
    pub relative_humidity: f64,
    /// Saturation temperature at the steam partial pressure (the dew
    /// point), when there is steam.
    pub dew_point: Option<ThermodynamicTemperature>,
}

/// Errors of the gas-mixture closure.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GasMixtureError {
    /// Inputs out of range (non-positive volume or temperature, negative
    /// contents, IF97 steam below 273.15 K, which is refused on purpose: see
    /// [`STEAM_BELOW_TRIPLE_POINT`]).
    #[error("gas mixture: invalid input: {0}")]
    InvalidInput(String),
    /// An iteration did not converge.
    #[error("gas mixture: {0} did not converge")]
    NotConverged(&'static str),
}

/// The closure: how heat capacities and steam are modelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GasMixtureModel {
    /// Non-condensable heat capacities.
    pub heat_capacity: NcgHeatCapacity,
    /// Steam.
    pub steam: SteamModel,
}

impl GasMixtureModel {
    /// The default: MELCOR fits where available, IF97 steam.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ablation: `sembawang`'s ideal constant-`c_v` steam.
    pub fn without_real_steam(mut self) -> Self {
        self.steam = SteamModel::IdealConstantCv;
        self
    }

    /// Ablation: constant non-condensable heat capacities.
    pub fn without_ncg_cv_t(mut self) -> Self {
        self.heat_capacity = NcgHeatCapacity::Constant;
        self
    }

    /// Molar `c_v` of `gas` at `t` K, J/(mol K).
    pub fn ncg_cv(&self, gas: NonCondensable, t: f64) -> f64 {
        match (self.heat_capacity, gas) {
            (NcgHeatCapacity::MelcorWhereAvailable, NonCondensable::N2) => melcor_ncg::cv_n2(t, true),
            (NcgHeatCapacity::MelcorWhereAvailable, NonCondensable::H2) => melcor_ncg::cv_h2(t, true),
            (NcgHeatCapacity::Constant, NonCondensable::N2) => melcor_ncg::CV_N2_CONST,
            (NcgHeatCapacity::Constant, NonCondensable::H2) => melcor_ncg::CV_H2_CONST,
            (NcgHeatCapacity::Constant, g) => nasa7::cv_molar(g, 300.0),
            (_, g) => nasa7::cv_molar(g, t),
        }
    }

    /// Molar internal energy of `gas` from 0 K at `t` K, J/mol.
    pub fn ncg_u(&self, gas: NonCondensable, t: f64) -> f64 {
        match (self.heat_capacity, gas) {
            (NcgHeatCapacity::MelcorWhereAvailable, NonCondensable::N2) => melcor_ncg::u_n2(t, true),
            (NcgHeatCapacity::MelcorWhereAvailable, NonCondensable::H2) => melcor_ncg::u_h2(t, true),
            (NcgHeatCapacity::Constant, g) => self.ncg_cv(g, t) * t,
            (_, g) => nasa7::u_molar(g, t),
        }
    }

    /// Steam partial pressure for `steam_kg` in `v` m³ at `t` K, Pa.
    fn steam_pressure(&self, steam_kg: f64, v: f64, t: f64) -> Result<f64, GasMixtureError> {
        if steam_kg <= 0.0 {
            return Ok(0.0);
        }
        let rho = steam_kg / v;
        let r_s = R_GAS / M_H2O;
        let ideal = rho * r_s * t;
        match self.steam {
            SteamModel::IdealConstantCv => Ok(ideal),
            SteamModel::Iapws97 => {
                if t < 273.15 {
                    return Err(GasMixtureError::InvalidInput(format!("{STEAM_BELOW_TRIPLE_POINT}: T = {t} K")));
                }
                let tk = ThermodynamicTemperature::new::<kelvin>(t);
                let rho_of = |p: f64| 1.0 / v_tp_2(tk, Pressure::new::<pascal>(p)).get::<cubic_meter_per_kilogram>();
                // Newton on rho(p) = rho, from the ideal-gas pressure; the
                // derivative by a relative difference.
                let mut p = ideal;
                for _ in 0..60 {
                    let f = rho_of(p) - rho;
                    let dp = 1e-6 * p;
                    let d = (rho_of(p + dp) - rho_of(p - dp)) / (2.0 * dp);
                    let step = f / d;
                    let p_new = (p - step).max(0.5 * p);
                    if (p_new - p).abs() <= 1e-13 * p {
                        return Ok(p_new);
                    }
                    p = p_new;
                }
                Err(GasMixtureError::NotConverged("IF97 steam partial pressure"))
            }
        }
    }

    /// The temperature at which saturated vapour has density `rho`, K
    /// (bisection on `ρ_g,sat(T)`, which rises with `T`), at least 273.16 K
    /// and at most 640 K.
    fn steam_saturation_temperature(&self, rho: f64) -> f64 {
        let rho_g = |t: f64| {
            let tk = ThermodynamicTemperature::new::<kelvin>(t);
            1.0 / v_tp_2(tk, sat_pressure_4(tk)).get::<cubic_meter_per_kilogram>()
        };
        let (mut a, mut b) = (273.16, 640.0);
        if rho <= rho_g(a) {
            return a;
        }
        if rho >= rho_g(b) {
            return b;
        }
        for _ in 0..60 {
            let m = 0.5 * (a + b);
            if rho_g(m) > rho {
                b = m;
            } else {
                a = m;
            }
        }
        b
    }

    /// Specific internal energy of steam at `(t, p_s)`, J/kg.
    fn steam_u(&self, t: f64, p_s: f64) -> f64 {
        match self.steam {
            SteamModel::IdealConstantCv => U_STEAM_REF_IDEAL + CV_STEAM_IDEAL * (t - T_REF_IDEAL),
            SteamModel::Iapws97 => {
                if p_s <= 0.0 {
                    // The zero-pressure limit of region 2 (ideal-gas part);
                    // only reached with no steam, where it is multiplied by 0.
                    0.0
                } else {
                    u_tp_2(ThermodynamicTemperature::new::<kelvin>(t), Pressure::new::<pascal>(p_s)).get::<joule_per_kilogram>()
                }
            }
        }
    }

    fn check(mix: &GasMixture, v: f64, t: f64) -> Result<(), GasMixtureError> {
        if !(v.is_finite() && v > 0.0 && t.is_finite() && t > 0.0)
            || !(mix.steam_kg.is_finite() && mix.steam_kg >= 0.0)
            || mix.ncg_moles.iter().any(|n| !(n.is_finite() && *n >= 0.0))
        {
            return Err(GasMixtureError::InvalidInput(format!("V {v} m3, T {t} K, {mix:?}")));
        }
        Ok(())
    }

    /// The state of `mix` in `volume` at `temperature`.
    ///
    /// # Errors
    ///
    /// [`GasMixtureError::InvalidInput`] on non-positive `V` or `T`,
    /// negative contents, or IF97 steam below 273.15 K;
    /// [`GasMixtureError::NotConverged`] if the steam pressure does not
    /// converge.
    pub fn state(
        &self,
        mix: &GasMixture,
        volume: Volume,
        temperature: ThermodynamicTemperature,
    ) -> Result<GasMixtureState, GasMixtureError> {
        let (v, t) = (volume.get::<cubic_meter>(), temperature.get::<kelvin>());
        Self::check(mix, v, t)?;
        let mut pp = [0.0; 6];
        for g in NonCondensable::ALL {
            pp[g.index()] = mix.ncg_moles[g.index()] * R_GAS * t / v;
        }
        let p_s = self.steam_pressure(mix.steam_kg, v, t)?;
        let u_ncg: f64 = NonCondensable::ALL.iter().map(|g| mix.ncg_moles[g.index()] * self.ncg_u(*g, t)).sum();
        let u = u_ncg + mix.steam_kg * self.steam_u(t, p_s);
        let (rh, dew) = if p_s > 0.0 {
            let rh = if t < 647.096 {
                p_s / sat_pressure_4(temperature).get::<pascal>()
            } else {
                0.0
            };
            let dew = if p_s < 22.064e6 && p_s > 611.657 {
                Some(sat_temp_4(Pressure::new::<pascal>(p_s)))
            } else {
                None
            };
            (rh, dew)
        } else {
            (0.0, None)
        };
        Ok(GasMixtureState {
            pressure: Pressure::new::<pascal>(pp.iter().sum::<f64>() + p_s),
            steam_partial_pressure: Pressure::new::<pascal>(p_s),
            ncg_partial_pressures: pp,
            internal_energy: Energy::new::<joule>(u),
            relative_humidity: rh,
            dew_point: dew,
        })
    }

    /// Heat capacity at constant volume, `∂U/∂T|_V`, J/K: analytic for the
    /// non-condensables, a centred difference for IF97 steam.
    ///
    /// # Errors
    ///
    /// As [`Self::state`].
    pub fn heat_capacity(
        &self,
        mix: &GasMixture,
        volume: Volume,
        temperature: ThermodynamicTemperature,
    ) -> Result<HeatCapacity, GasMixtureError> {
        let (v, t) = (volume.get::<cubic_meter>(), temperature.get::<kelvin>());
        Self::check(mix, v, t)?;
        let c_ncg: f64 = NonCondensable::ALL.iter().map(|g| mix.ncg_moles[g.index()] * self.ncg_cv(*g, t)).sum();
        let c_s = if mix.steam_kg > 0.0 {
            match self.steam {
                SteamModel::IdealConstantCv => mix.steam_kg * CV_STEAM_IDEAL,
                SteamModel::Iapws97 => {
                    let dt = 1e-3;
                    let us = |tt: f64| -> Result<f64, GasMixtureError> {
                        let p = self.steam_pressure(mix.steam_kg, v, tt)?;
                        Ok(self.steam_u(tt, p))
                    };
                    mix.steam_kg * (us(t + dt)? - us(t - dt)?) / (2.0 * dt)
                }
            }
        } else {
            0.0
        };
        Ok(HeatCapacity::new::<joule_per_kelvin>(c_ncg + c_s))
    }

    /// The temperature at which `mix` in `volume` holds `internal_energy`,
    /// by Newton on `U(T)` safeguarded by bisection, from `guess`.
    ///
    /// # Errors
    ///
    /// As [`Self::state`]; [`GasMixtureError::InvalidInput`] with
    /// [`STEAM_BELOW_TRIPLE_POINT`] if IF97 steam is present and the energy
    /// lies below the mixture's energy at 273.16 K (gh:#1115);
    /// [`GasMixtureError::NotConverged`] if no other bracket in 200–3000 K
    /// holds the energy.
    pub fn temperature(
        &self,
        mix: &GasMixture,
        volume: Volume,
        internal_energy: Energy,
        guess: ThermodynamicTemperature,
    ) -> Result<ThermodynamicTemperature, GasMixtureError> {
        let target = internal_energy.get::<joule>();
        let u_at = |t: f64| -> Result<f64, GasMixtureError> {
            Ok(self.state(mix, volume, ThermodynamicTemperature::new::<kelvin>(t))?.internal_energy.get::<joule>())
        };
        // Below the temperature at which the steam's density is saturated
        // vapour the steam would condense (the control volume's job), and
        // IF97's metastable region 2 is not trusted there: the search starts
        // at that temperature (found 2026-11-01: 2 kg in 10 m3 at 273 K is
        // 40x supersaturated and the partial-pressure Newton fails).
        let lo0 = if self.steam == SteamModel::Iapws97 && mix.steam_kg > 0.0 {
            self.steam_saturation_temperature(mix.steam_kg / volume.get::<cubic_meter>())
        } else {
            200.0
        };
        let (mut lo, mut hi) = (lo0, 3000.0);
        let (f_lo, f_hi) = (u_at(lo)? - target, u_at(hi)? - target);
        if f_lo > 0.0 && lo0 <= 273.16 && self.steam == SteamModel::Iapws97 && mix.steam_kg > 0.0 {
            // The energy lies below the gas's energy at the triple point with
            // subsaturated steam: the state is colder than 273.16 K (gh:#1115).
            return Err(GasMixtureError::InvalidInput(format!(
                "{STEAM_BELOW_TRIPLE_POINT}: U = {target} J is below U(273.16 K) = {} J",
                f_lo + target
            )));
        }
        if f_lo > 0.0 || f_hi < 0.0 {
            return Err(GasMixtureError::NotConverged("temperature from energy (no bracket)"));
        }
        let mut t = guess.get::<kelvin>().clamp(lo, hi);
        for _ in 0..100 {
            let f = u_at(t)? - target;
            if f.abs() <= 1e-12 * target.abs().max(1.0) {
                return Ok(ThermodynamicTemperature::new::<kelvin>(t));
            }
            if f > 0.0 {
                hi = t;
            } else {
                lo = t;
            }
            let c = self.heat_capacity(mix, volume, ThermodynamicTemperature::new::<kelvin>(t))?.get::<joule_per_kelvin>();
            let mut t_new = t - f / c;
            if !(t_new > lo && t_new < hi) {
                t_new = 0.5 * (lo + hi);
            }
            if (t_new - t).abs() <= 1e-12 * t {
                return Ok(ThermodynamicTemperature::new::<kelvin>(t_new));
            }
            t = t_new;
        }
        Err(GasMixtureError::NotConverged("temperature from energy"))
    }
}

#[cfg(test)]
mod tests;
