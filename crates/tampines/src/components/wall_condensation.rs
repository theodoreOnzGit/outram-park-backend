// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! A heat-structure surface facing a gas atmosphere: natural convection plus
//! condensation of steam by diffusion through the non-condensables, the
//! latent heat into the wall (gh:#1096, epic gh:#1091; MELCOR 2.2 HS RM
//! §2.6.1 and §2.7, cited, not quoted; clean room, no MELCOR source).
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! # Moved here verbatim from `sembawang`
//!
//! **The physics is `sembawang::bwr_mark1::pool_surface::structure_surface`**
//! (the Fukushima Unit 1 drywell wall, `Mechanisms::dw_wall_hs`, and the
//! torus shell, `Mechanisms::ww_shell`), moved here on 2026-11-01
//! (maintainer direction: `sembawang` depends on `tampines`, and the thermal
//! hydraulics, wall heat transfer included, belongs here). It moved with its
//! operation order unchanged, together with the gas-mixture transport
//! properties it needs ([`Atmosphere`]: MELCOR's steam and hydrogen tables,
//! the nitrogen power laws, Eucken's correlation for hydrogen's conductivity,
//! Wilke's and Wassijewa's mixing rules, the steam-air and steam-hydrogen
//! diffusivities). `sembawang::bwr_mark1::pool_surface` now re-exports
//! [`structure_surface`] and [`Atmosphere`], so every value is the same bits;
//! `crates/sembawang/tests/wall_condensation_bit_identity.rs` checks that
//! against hashes pinned on the pre-move code.
//!
//! Reused rather than copied: the N2 and H2 `c_v(T)` fits and the molar
//! masses from [`tampines_steam_tables::gas_mixture::melcor_ncg`] (gh:#1093,
//! moved from `sembawang` the same way); the saturation line ([`p_sat_pa`],
//! [`t_sat`]) and the containment's steam enthalpy ([`steam_h`]) from
//! [`super::wet_well`], which ported them from `sembawang` operation for
//! operation (gh:#1075). ~~**Known duplicate, stated:** `wet_well` also carries
//! a steam-and-N2-only copy of [`Atmosphere`] and its tables (gh:#1075);
//! folding it onto this module needs its own bit-identity check and is left
//! as follow-up.~~ **FOLDED 2026-11-02 (gh:#1118 item 2):** the wet well's
//! and the control volume's steam-and-N2 atmosphere
//! (`crate::control_volume::pool_surface::Atmosphere`) keeps its four-field
//! shape but takes every property from this [`Atmosphere`] with no
//! hydrogen; its tables, mixing rules and diffusivity copy are gone. The
//! result is bit-identical, pinned by
//! `tests/wet_well.rs::atmosphere_fold_is_bit_for_bit`.
//!
//! # The model (from the moved doc comment)
//!
//! Natural convection on an external rectangular surface of height `l_c`:
//! `Nu = 0.59 Ra^1/4` below `Ra = 1e9`, `0.10 Ra^1/3` above `1e10`, linear in
//! `Ra` between (HS RM Table 2.3, Eqs 2-82, 2-85 to 2-87); while condensing
//! the Grashof number comes from the density difference between the bulk
//! and the saturated gas at the wall. Condensation below the dew point by
//! diffusion through the non-condensables, `m'' = h_D ρ_v ln((P − P_srf) /
//! (P − P_stm))` with `h_D = Nu Sc^1/3 Pr^-1/3 D / l_c` (Eqs 2-134 to 2-136;
//! `ρ_v` the vapour's density at `T_sat(P)`); above `VPFRAC = 0.9995` of
//! steam the steam fraction is held at it. Transport properties at the film
//! temperature, density at the bulk. No film is kept (the condensate leaves
//! at once: to a pool or a drain), so nothing evaporates from the wall above
//! the dew point.
//!
//! # Coupled to [`HeatStructure`]
//!
//! [`WallCondensation::exchange`] evaluates the surface for every axial cell
//! of one side of a [`HeatStructure`] facing a [`WallGas`], at the start of
//! the step (explicit, as `sembawang`'s condensation is), and returns a
//! [`WallExchange`]: the [`StructureBoundary::HeatInflow`] to step the
//! structure with, the condensation rate (for the caller's pool or drain),
//! and every energy flow, which close exactly: the gas loses the sensible
//! heat plus `ṁ h_g`; the wall gains the sensible heat plus `ṁ (h_g − h_l)`;
//! the condensate carries `ṁ h_l`.
//!
//! | Default | Ablation | What the ablation does |
//! |---|---|---|
//! | MELCOR's natural convection plus condensation ([`structure_surface`]) | [`WallCondensation::without_melcor_surface`] | a fixed coefficient, no condensation (`sembawang`'s `dw_wall_h`) |
//! | IAPWS-IF97 enthalpies: `h_g` region 2 at the gas temperature and the steam's partial pressure, `h_l` region 1 at the wall temperature | [`WallCondensation::without_real_steam`] | the containment's convention (`sembawang`, [`super::wet_well`]): `h_g = 2.506e6 + 1500 (T − 373.15) + R T / M`, `h_l = 4186 (T − 273.15)` |
//!
//! Choose the enthalpy convention the atmosphere's own energy is carried on
//! (a control volume on the containment's convention takes the ablation), so
//! the energy the gas loses is the energy its own bookkeeping removes.
//!
//! # Limits (stated)
//!
//! - The wall temperature is the surface cell's centre temperature (the
//!   lumped wall of `sembawang` has no other), and the convective heat uses
//!   the structure's half-cell resistance ([`HeatStructure::heat_to_fluid`]).
//! - Explicit over the step: the surface is evaluated at the start-of-step
//!   state, the heat frozen. Condensation is limited to 90 % of the
//!   atmosphere's steam in a step (HS §2.7.3), scaled over the side's cells.
//! - No radiation, no forced convection, no condensate film resistance, no
//!   evaporation from a film.

use petir::mathf::RealMath;
use tampines_steam_tables::gas_mixture::melcor_ncg::{cv_h2, cv_n2, M_H2, M_H2O, M_N2, R_GAS};
use tampines_steam_tables::region_1_subcooled_liquid::intensive_properties::h_tp_1;
use tampines_steam_tables::region_2_vapour::intensive_properties::h_tp_2;
use uom::si::amount_of_substance::mole;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    AmountOfSubstance, AvailableEnergy, HeatTransfer, Length, Mass, MassRate, Power, Pressure,
    ThermodynamicTemperature, Time, Volume,
};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volume::cubic_meter;

pub use super::wet_well::{p_sat_pa, steam_h, t_sat};
use super::heat_structure::{HeatStructure, StructureBoundary, StructureSide};
use crate::TampinesError;

/// Gravity, m/s² (`sembawang::bwr_mark1::pool_surface::G`).
const G: f64 = 9.81;

/// Liquid water's constant `c_p`, J/(kg K): the containment's convention
/// (`sembawang::bwr_mark1::containment::CP_WATER`), used by
/// [`WallCondensation::without_real_steam`].
pub const CP_WATER_CONTAINMENT: f64 = 4186.0;

/// Linear interpolation in a table of `(T, value)`, held at its ends
/// (moved from `sembawang::bwr_mark1::pool_surface::table`).
pub fn interpolate_held(t: f64, xs: &[(f64, f64)]) -> f64 {
    if t <= xs[0].0 {
        return xs[0].1;
    }
    for w in xs.windows(2) {
        if t <= w[1].0 {
            return w[0].1 + (t - w[0].0) / (w[1].0 - w[0].0) * (w[1].1 - w[0].1);
        }
    }
    xs[xs.len() - 1].1
}

/// Steam's conductivity, W/(m K), MELCOR's default table (NCG/H2O RM
/// §4.1.2, its Ref. [3]), 273-1073 K.
const K_STEAM: [(f64, f64); 21] = [
    (273.15, 0.0176), (293.15, 0.0188), (313.15, 0.0201), (333.15, 0.0216), (353.15, 0.0231), (373.15, 0.0245), (393.15, 0.0260),
    (413.15, 0.0277), (433.15, 0.0295), (453.15, 0.0313), (473.15, 0.0331), (493.15, 0.0351), (513.15, 0.0371), (533.15, 0.0391),
    (553.15, 0.0412), (573.15, 0.0433), (593.15, 0.0455), (613.15, 0.0478), (633.15, 0.0501), (653.15, 0.0525), (673.15, 0.0548),
];
/// Steam's viscosity, Pa s (§5.1.2, its Ref. [8]), 273-673 K.
const MU_STEAM: [(f64, f64); 11] = [
    (273.15, 8.04e-6), (313.15, 9.66e-6), (353.15, 1.13e-5), (393.15, 1.29e-5), (433.15, 1.46e-5), (473.15, 1.62e-5), (513.15, 1.78e-5),
    (553.15, 1.94e-5), (593.15, 2.11e-5), (633.15, 2.27e-5), (673.15, 2.43e-5),
];
/// Hydrogen's viscosity, Pa s (§5.1.4, its Ref. [9]), 250-900 K.
const MU_H2: [(f64, f64); 8] = [
    (250.0, 7.9232e-6), (280.0, 8.5523e-6), (300.0, 8.9594e-6), (400.0, 1.0867e-5), (500.0, 1.2642e-5), (600.0, 1.4290e-5), (700.0, 1.5846e-5),
    (900.0, 1.8756e-5),
];

/// The three gases of a containment atmosphere.
#[derive(Clone, Copy, Debug)]
enum Gas {
    Steam,
    N2,
    H2,
}

impl Gas {
    fn molar_mass(self) -> f64 {
        match self {
            Gas::Steam => M_H2O,
            Gas::N2 => M_N2,
            Gas::H2 => M_H2,
        }
    }
    /// Viscosity, Pa s: steam's and hydrogen's tables, nitrogen's power
    /// law `3.9539785e-7 T^0.67288` (§5.4, Table 5.2).
    fn viscosity(self, t: f64) -> f64 {
        match self {
            Gas::Steam => interpolate_held(t, &MU_STEAM),
            Gas::N2 => 3.953_978_5e-7 * t.r_powf(0.672_88),
            Gas::H2 => interpolate_held(t, &MU_H2),
        }
    }
    /// Conductivity, W/(m K): steam's table, nitrogen's power law
    /// `3.704689e-4 T^0.74842` (§4.4, Table 4.1), hydrogen by Eucken's
    /// correlation `(c_v + 9 R / (4 M)) mu` (Eq 4-1; MELCOR has no default
    /// table for it).
    fn conductivity(self, t: f64) -> f64 {
        match self {
            Gas::Steam => interpolate_held(t, &K_STEAM),
            Gas::N2 => 3.704_689e-4 * t.r_powf(0.748_42),
            Gas::H2 => (cv_h2(t, true) / M_H2 + 9.0 * R_GAS / (4.0 * M_H2)) * interpolate_held(t, &MU_H2),
        }
    }
}

/// Wilke's `phi_ij` (NCG/H2O RM Eq 5-2), also Mason and Saxena's `A_ij`
/// (Eq 4-2): the same form.
fn phi(mu_i: f64, mu_j: f64, m_i: f64, m_j: f64) -> f64 {
    let a = 1.0 + (mu_i / mu_j).sqrt() * (m_j / m_i).r_powf(0.25);
    a * a / (8.0 * (1.0 + m_i / m_j)).sqrt()
}

/// A steam, nitrogen and hydrogen atmosphere's bulk state, in the SI units
/// `sembawang`'s containment carries (moved from
/// `sembawang::bwr_mark1::pool_surface::Atmosphere`, kept in plain `f64` so
/// the moved arithmetic is the same bits; the `uom` front end is
/// [`WallGas`]). Only the ratios of the three amounts enter the properties.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Atmosphere {
    /// Temperature, K.
    pub t_k: f64,
    /// Total pressure, Pa.
    pub p_pa: f64,
    /// Moles of steam.
    pub n_steam: f64,
    /// Moles of nitrogen.
    pub n_n2: f64,
    /// Moles of hydrogen.
    pub n_h2: f64,
}

impl Atmosphere {
    fn gases(&self) -> [(Gas, f64); 3] {
        let n = self.n_steam + self.n_n2 + self.n_h2;
        [(Gas::Steam, self.n_steam / n), (Gas::N2, self.n_n2 / n), (Gas::H2, self.n_h2 / n)]
    }
    /// Mixture viscosity, Pa s (Wilke, Eq 5-2) and conductivity, W/(m K)
    /// (Wassijewa with Mason and Saxena, Eq 4-2).
    pub fn viscosity_and_conductivity(&self) -> (f64, f64) {
        let g = self.gases();
        let mu: [f64; 3] = g.map(|(x, _)| x.viscosity(self.t_k));
        let k: [f64; 3] = g.map(|(x, _)| x.conductivity(self.t_k));
        let (mut mu_mix, mut k_mix) = (0.0, 0.0);
        for i in 0..3 {
            if g[i].1 <= 0.0 {
                continue;
            }
            let d: f64 = (0..3).map(|j| g[j].1 * phi(mu[i], mu[j], g[i].0.molar_mass(), g[j].0.molar_mass())).sum();
            mu_mix += g[i].1 * mu[i] / d;
            k_mix += g[i].1 * k[i] / d;
        }
        (mu_mix, k_mix)
    }
    /// Molar mass, kg/mol, density, kg/m^3, and `c_p`, J/(kg K) (steam
    /// `c_v` as the containment's, plus `R/M`; N2 and H2 MELCOR's `c_v(T)`).
    pub fn mass_properties(&self) -> (f64, f64, f64) {
        let n = self.n_steam + self.n_n2 + self.n_h2;
        let mass = self.n_steam * M_H2O + self.n_n2 * M_N2 + self.n_h2 * M_H2;
        let molar = mass / n;
        let cp_molar = self.n_steam * (CV_STEAM_MOLAR + R_GAS) + self.n_n2 * (cv_n2(self.t_k, true) + R_GAS) + self.n_h2 * (cv_h2(self.t_k, true) + R_GAS);
        (molar, self.p_pa * molar / (R_GAS * self.t_k), cp_molar / mass)
    }
    /// Diffusivity of water vapour in the non-condensables, m^2/s: MELCOR's
    /// steam-air (Eq 6-1, used for N2, stated) and steam-hydrogen (Eq 6-2)
    /// correlations, combined by Blanc's law over the non-condensables'
    /// mole fractions. `None` with no non-condensables.
    pub fn steam_diffusivity(&self) -> Option<f64> {
        let n_nc = self.n_n2 + self.n_h2;
        if n_nc <= 0.0 {
            return None;
        }
        let d_n2 = 4.7931e-5 * self.t_k.r_powf(1.9) / self.p_pa;
        let d_h2 = 6.606_39e-4 * self.t_k.r_powf(1.68) / self.p_pa;
        Some(1.0 / (self.n_n2 / n_nc / d_n2 + self.n_h2 / n_nc / d_h2))
    }
}

/// Steam's molar `c_v`, J/(mol K): the containment's constant (1500 J/(kg K)).
const CV_STEAM_MOLAR: f64 = 1500.0 * M_H2O;

/// MELCOR's heat-structure surface facing an atmosphere (HS RM §2.6.1,
/// §2.7): natural convection on an external rectangular surface of height
/// `l_c`, `Nu = 0.59 Ra^1/4` below `Ra = 1e9`, `0.10 Ra^1/3` above `1e10`,
/// linear in `Ra` between (Table 2.3, Eqs 2-82, 2-85 to 2-87), with the
/// Grashof number from the density difference while condensing; and
/// condensation below the dew point by diffusion through the
/// non-condensables, `m = h_D rho_v ln((P - P_srf) / (P - P_stm))` with
/// `h_D = Nu Sc^1/3 Pr^-1/3 D / l_c` (Eqs 2-134 to 2-136; `rho_v` the
/// vapour's density at `T_sat(P)`); above `VPFRAC = 0.9995` of steam the
/// steam fraction is held at it (stated: MELCOR then lets the
/// structure's conduction limit it). Transport properties at the film
/// temperature, density at the bulk. Returns the convective coefficient,
/// W/(m^2 K), and the condensation flux, kg/(m^2 s) (zero above the dew
/// point: no film is kept to evaporate, stated).
///
/// Moved verbatim from `sembawang::bwr_mark1::pool_surface::structure_surface`
/// (gh:#1096). The front end with units is [`WallCondensation`].
pub fn structure_surface(atm: &Atmosphere, t_wall_k: f64, l_c: f64) -> (f64, f64) {
    let t_f = 0.5 * (atm.t_k + t_wall_k);
    let (mu, k) = Atmosphere { t_k: t_f, ..*atm }.viscosity_and_conductivity();
    let (_, rho, cp) = atm.mass_properties();
    let pr = mu * cp / k;
    let n = atm.n_steam + atm.n_n2 + atm.n_h2;
    let p_stm = (atm.n_steam / n).min(0.9995) * atm.p_pa;
    let p_srf = p_sat_pa(t_wall_k.min(640.0));
    let condensing = t_wall_k < 647.2 && p_srf < p_stm && atm.n_n2 + atm.n_h2 > 0.0;
    let gr = if condensing {
        // The gas at the surface: saturated at the wall's temperature.
        let y_s = p_srf / atm.p_pa;
        let n_nc = atm.n_n2 + atm.n_h2;
        let m_srf = y_s * M_H2O + (1.0 - y_s) * (atm.n_n2 * M_N2 + atm.n_h2 * M_H2) / n_nc;
        let rho_srf = atm.p_pa * m_srf / (R_GAS * t_wall_k);
        G * (rho_srf - rho).abs() * l_c * l_c * l_c * rho / (mu * mu)
    } else {
        G * (atm.t_k - t_wall_k).abs() / t_f * l_c * l_c * l_c * rho * rho / (mu * mu)
    };
    let ra = gr * pr;
    let lam = 0.59 * ra.r_powf(0.25);
    let tur = 0.10 * ra.r_powf(1.0 / 3.0);
    let nu = if ra <= 1e9 {
        lam
    } else if ra >= 1e10 {
        tur
    } else {
        lam + (ra - 1e9) / 9e9 * (tur - lam)
    };
    let h = nu * k / l_c;
    if !condensing {
        return (h, 0.0);
    }
    let d = atm.steam_diffusivity().unwrap_or(0.0);
    let sc = mu / (rho * d);
    let h_d = nu * (sc / pr).r_powf(1.0 / 3.0) * d / l_c;
    let rho_v = atm.p_pa * M_H2O / (R_GAS * t_sat(atm.p_pa));
    (h, h_d * rho_v * ((atm.p_pa - p_srf) / (atm.p_pa - p_stm)).r_ln())
}

/// Steam's conductivity, W/(m K): MELCOR's default table (above), for film
/// correlations elsewhere (moved from `sembawang`'s `pool_surface`).
pub fn steam_conductivity(t_k: f64) -> f64 {
    interpolate_held(t_k, &K_STEAM)
}

/// Steam's viscosity, Pa s: MELCOR's default table (above).
pub fn steam_viscosity(t_k: f64) -> f64 {
    interpolate_held(t_k, &MU_STEAM)
}

// ── The front end: a HeatStructure side facing a gas atmosphere ─────────────

/// The gas a structure surface faces: temperature, total pressure and the
/// amounts of steam, nitrogen and hydrogen (as a control volume carries
/// them). [`WallGas::from_partial_pressures`] builds one from partial
/// pressures and the gas volume.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallGas {
    /// Bulk temperature.
    pub temperature: ThermodynamicTemperature,
    /// Total pressure.
    pub pressure: Pressure,
    /// Steam in the atmosphere.
    pub steam: AmountOfSubstance,
    /// Nitrogen.
    pub nitrogen: AmountOfSubstance,
    /// Hydrogen.
    pub hydrogen: AmountOfSubstance,
}

impl WallGas {
    /// From partial pressures in a gas volume `volume` (ideal gases,
    /// `n_i = p_i V / (R T)`); the total pressure is their sum (Dalton).
    pub fn from_partial_pressures(
        temperature: ThermodynamicTemperature,
        steam: Pressure,
        nitrogen: Pressure,
        hydrogen: Pressure,
        volume: Volume,
    ) -> Self {
        let (t, v) = (temperature.get::<kelvin>(), volume.get::<cubic_meter>());
        let n = |p: Pressure| AmountOfSubstance::new::<mole>(p.get::<pascal>() * v / (R_GAS * t));
        Self {
            temperature,
            pressure: steam + nitrogen + hydrogen,
            steam: n(steam),
            nitrogen: n(nitrogen),
            hydrogen: n(hydrogen),
        }
    }

    /// The moved kernel's state.
    pub fn atmosphere(&self) -> Atmosphere {
        Atmosphere {
            t_k: self.temperature.get::<kelvin>(),
            p_pa: self.pressure.get::<pascal>(),
            n_steam: self.steam.get::<mole>(),
            n_n2: self.nitrogen.get::<mole>(),
            n_h2: self.hydrogen.get::<mole>(),
        }
    }

    /// Steam's partial pressure, `x_steam P`.
    pub fn steam_partial_pressure(&self) -> Pressure {
        let a = self.atmosphere();
        Pressure::new::<pascal>(a.n_steam / (a.n_steam + a.n_n2 + a.n_h2) * a.p_pa)
    }
}

/// How the surface's heat-transfer coefficient is found.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SurfaceModel {
    /// MELCOR's natural convection plus condensation ([`structure_surface`]).
    Melcor,
    /// The ablation: a fixed coefficient, no condensation.
    FixedCoefficient(HeatTransfer),
}

/// The enthalpies the condensate is booked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CondensateEnthalpy {
    /// IAPWS-IF97: `h_g` in region 2 at the gas temperature and the steam's
    /// partial pressure (held at or below saturation), `h_l` in region 1 at
    /// the wall temperature, just above its saturation pressure.
    Iapws97,
    /// The containment's convention ([`steam_h`] and
    /// [`CP_WATER_CONTAINMENT`]): the ablation.
    ContainmentConstantCv,
}

/// A structure surface facing a gas: the model and its ablations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallCondensation {
    surface: SurfaceModel,
    enthalpy: CondensateEnthalpy,
}

impl Default for WallCondensation {
    fn default() -> Self {
        Self::new()
    }
}

/// One step's exchange between a gas and one side of a [`HeatStructure`].
#[derive(Debug, Clone, PartialEq)]
pub struct WallExchange {
    /// The boundary to step the structure's side with
    /// ([`StructureBoundary::HeatInflow`], per axial cell).
    pub boundary: StructureBoundary,
    /// The convective coefficient at each axial cell.
    pub heat_transfer_coefficient: Vec<HeatTransfer>,
    /// The condensation rate at each axial cell (after the step limit).
    pub condensation_per_cell: Vec<MassRate>,
    /// The total condensation rate: the condensate leaves the wall at once,
    /// for the caller's pool or drain.
    pub condensation_rate: MassRate,
    /// Sensible heat from the gas to the wall (negative: the wall heats the
    /// gas).
    pub convective_power: Power,
    /// Latent heat into the wall, `Σ ṁ_j (h_g − h_l,j)`.
    pub latent_power_to_wall: Power,
    /// Enthalpy the condensate carries away, `Σ ṁ_j h_l,j`.
    pub condensate_enthalpy_flow: Power,
    /// What the gas loses: the sensible heat plus `ṁ h_g`.
    pub gas_energy_loss: Power,
    /// The steam's specific enthalpy the condensation removes from the gas.
    pub steam_enthalpy: AvailableEnergy,
    /// Whether the 90 %-of-the-steam step limit scaled the condensation.
    pub limited: bool,
}

impl WallCondensation {
    /// MELCOR's surface with IF97 enthalpies: every mechanism on.
    pub fn new() -> Self {
        Self { surface: SurfaceModel::Melcor, enthalpy: CondensateEnthalpy::Iapws97 }
    }

    /// Ablation: a fixed coefficient `h` and no condensation.
    pub fn without_melcor_surface(mut self, h: HeatTransfer) -> Self {
        self.surface = SurfaceModel::FixedCoefficient(h);
        self
    }

    /// Ablation: the containment's constant-`c_v` enthalpies instead of
    /// IF97's (for an atmosphere carried on that convention).
    pub fn without_real_steam(mut self) -> Self {
        self.enthalpy = CondensateEnthalpy::ContainmentConstantCv;
        self
    }

    /// The surface model in use.
    pub fn surface_model(&self) -> SurfaceModel {
        self.surface
    }

    /// The enthalpy convention in use.
    pub fn condensate_enthalpy(&self) -> CondensateEnthalpy {
        self.enthalpy
    }

    /// The convective coefficient and the condensation flux, kg/(m² s), of a
    /// wall at `t_wall` of height `height` facing `gas`.
    pub fn surface(
        &self,
        gas: &WallGas,
        t_wall: ThermodynamicTemperature,
        height: Length,
    ) -> (HeatTransfer, f64) {
        match self.surface {
            SurfaceModel::Melcor => {
                let (h, m) = structure_surface(&gas.atmosphere(), t_wall.get::<kelvin>(), height.get::<meter>());
                (HeatTransfer::new::<watt_per_square_meter_kelvin>(h), m)
            }
            SurfaceModel::FixedCoefficient(h) => (h, 0.0),
        }
    }

    /// The steam's specific enthalpy in the gas, J/kg.
    fn h_g(&self, gas: &WallGas) -> f64 {
        let t = gas.temperature.get::<kelvin>();
        match self.enthalpy {
            CondensateEnthalpy::ContainmentConstantCv => steam_h(t),
            CondensateEnthalpy::Iapws97 => {
                let p_s = gas.steam_partial_pressure().get::<pascal>();
                let p = if t < 623.15 { p_s.min(p_sat_pa(t)) } else { p_s };
                h_tp_2(gas.temperature, Pressure::new::<pascal>(p.max(1.0))).get::<joule_per_kilogram>()
            }
        }
    }

    /// The condensate's specific enthalpy at the wall temperature, J/kg.
    fn h_l(&self, t_wall: f64, p: f64) -> f64 {
        match self.enthalpy {
            CondensateEnthalpy::ContainmentConstantCv => CP_WATER_CONTAINMENT * (t_wall - 273.15),
            CondensateEnthalpy::Iapws97 => {
                let t = t_wall.clamp(273.16, 623.15);
                let p = p.max(1.0001 * p_sat_pa(t));
                h_tp_1(ThermodynamicTemperature::new::<kelvin>(t), Pressure::new::<pascal>(p)).get::<joule_per_kilogram>()
            }
        }
    }

    /// The exchange between `gas` and side `side` of `structure` over a
    /// step `dt`, at the start-of-step state, the wall of height `height`
    /// (MELCOR's characteristic length). Condensation is limited to 90 % of
    /// the gas's steam in the step.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a non-positive `dt` or height, or
    /// a non-finite or non-positive gas temperature or pressure.
    pub fn exchange(
        &self,
        structure: &HeatStructure,
        side: StructureSide,
        gas: &WallGas,
        height: Length,
        dt: Time,
    ) -> Result<WallExchange, TampinesError> {
        let (dt_s, l, t_g, p) = (
            dt.get::<second>(),
            height.get::<meter>(),
            gas.temperature.get::<kelvin>(),
            gas.pressure.get::<pascal>(),
        );
        if ![dt_s, l, t_g, p].iter().all(|v| v.is_finite() && *v > 0.0) {
            return Err(TampinesError::InvalidInput(format!("dt {dt_s} s, height {l} m, gas {t_g} K, {p} Pa")));
        }
        let n_ax = structure.n_axial();
        let i_s = match side {
            StructureSide::Inner => 0,
            StructureSide::Outer => structure.n_cells() - 1,
        };
        let area = structure.surface_area(side).get::<uom::si::area::square_meter>();
        let mut hs = Vec::with_capacity(n_ax);
        let mut q_conv = Vec::with_capacity(n_ax);
        let mut m = Vec::with_capacity(n_ax);
        let mut t_w = Vec::with_capacity(n_ax);
        for j in 0..n_ax {
            let tw = structure
                .temperature(j, i_s)
                .ok_or_else(|| TampinesError::InvalidInput(format!("no cell {i_s} at axial {j}")))?;
            let (h, m_flux) = self.surface(gas, tw, height);
            let hh = h.get::<watt_per_square_meter_kelvin>();
            // Heat from the gas into the wall: minus what leaves the wall.
            q_conv.push(-structure.heat_to_fluid(side, j, t_g, hh));
            m.push(m_flux * area);
            hs.push(h);
            t_w.push(tw.get::<kelvin>());
        }
        // At most 90 % of the steam in a step (HS §2.7.3).
        let m_total: f64 = m.iter().sum();
        let steam_kg = gas.steam.get::<mole>() * M_H2O;
        let limited = m_total * dt_s > 0.9 * steam_kg;
        if limited {
            let f = 0.9 * steam_kg / (m_total * dt_s);
            m.iter_mut().for_each(|x| *x *= f);
        }
        let h_g = if m.iter().any(|x| *x > 0.0) { self.h_g(gas) } else { 0.0 };
        let mut inflow = Vec::with_capacity(n_ax);
        let (mut latent, mut carried) = (0.0, 0.0);
        for j in 0..n_ax {
            let h_l = if m[j] > 0.0 { self.h_l(t_w[j], p) } else { 0.0 };
            let lat = m[j] * (h_g - h_l);
            latent += lat;
            carried += m[j] * h_l;
            inflow.push(q_conv[j] + lat);
        }
        let conv: f64 = q_conv.iter().sum();
        let m_sum: f64 = m.iter().sum();
        Ok(WallExchange {
            boundary: StructureBoundary::HeatInflow(inflow),
            heat_transfer_coefficient: hs,
            condensation_per_cell: m.iter().map(|x| MassRate::new::<kilogram_per_second>(*x)).collect(),
            condensation_rate: MassRate::new::<kilogram_per_second>(m_sum),
            convective_power: Power::new::<watt>(conv),
            latent_power_to_wall: Power::new::<watt>(latent),
            condensate_enthalpy_flow: Power::new::<watt>(carried),
            gas_energy_loss: Power::new::<watt>(conv + m_sum * h_g),
            steam_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(h_g),
            limited,
        })
    }
}

/// Mass of steam a [`WallGas`] holds.
pub fn steam_mass(gas: &WallGas) -> Mass {
    Mass::new::<kilogram>(gas.steam.get::<mole>() * M_H2O)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nitrogen at 300 K (moved with the code from `sembawang`): MELCOR's
    /// power laws give, by hand, `3.9539785e-7 300^0.67288 = 1.836e-5` Pa s
    /// and `3.704689e-4 300^0.74842 = 0.02647` W/(m K); the steam-air
    /// diffusivity at 1 bar is `4.7931e-5 300^1.9 / 1e5 = 2.439e-5` m^2/s.
    #[test]
    fn nitrogen_properties_match_a_hand_value() {
        let a = Atmosphere { t_k: 300.0, p_pa: 1e5, n_steam: 0.0, n_n2: 1.0, n_h2: 0.0 };
        let (mu, k) = a.viscosity_and_conductivity();
        assert!((mu - 1.836e-5).abs() < 0.001e-5 && (k - 0.02647).abs() < 0.00001, "{mu} {k}");
        assert!((a.steam_diffusivity().unwrap() - 2.439e-5).abs() < 0.001e-5, "{:?}", a.steam_diffusivity());
        assert_eq!(steam_conductivity(373.15), 0.0245);
        assert_eq!(steam_viscosity(373.15 - 20.0), 1.13e-5);
    }

    /// Moved from `sembawang` with the function: a 400 K wall under 0.4 MPa
    /// of gas at 600 K, 30 % steam, 10 % H2: turbulent natural convection,
    /// by hand about 12 W/(m^2 K), independent of height; the dew point at
    /// 0.12 MPa of steam is 378 K, so that wall stays dry; a 350 K wall
    /// condenses.
    #[test]
    fn a_cold_wall_convects_and_condenses() {
        let n = 0.4e6 * 3410.0 / (R_GAS * 600.0);
        let atm = Atmosphere { t_k: 600.0, p_pa: 0.4e6, n_steam: 0.3 * n, n_n2: 0.6 * n, n_h2: 0.1 * n };
        let (h, m) = structure_surface(&atm, 400.0, 20.0);
        let (h2, _) = structure_surface(&atm, 400.0, 40.0);
        assert!(h > 9.0 && h < 16.0 && m == 0.0, "{h} {m}");
        assert!((h - h2).abs() < 0.02 * h, "{h} {h2}");
        let (_, m) = structure_surface(&atm, 350.0, 20.0);
        assert!(m > 0.0, "{m}");
    }
}
