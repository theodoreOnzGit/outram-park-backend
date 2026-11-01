// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! MELCOR's pool surface and bubble efficiency, shared by the wet well
//! ([`crate::components::wet_well`], gh:#1075) and the control volume
//! ([`crate::control_volume`], gh:#1092).
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! **Moved here verbatim from `components/wet_well.rs` on 2026-11-01**
//! (gh:#1092), so the two components share one copy; no operation was
//! changed, so every number is bit-identical to what the wet well computed
//! before, and `crates/sembawang/tests/wet_well_vs_tampines.rs` still checks
//! the functions bit for bit against `sembawang`'s originals (through the
//! wet well's re-exports). Each function names its counterpart in
//! `sembawang::bwr_mark1` (MELCOR 2.2 CVH/FL reference manual, cited by
//! section and equation, not quoted; no MELCOR source used).
//!
//! The gas is steam plus **one** non-condensable with nitrogen's properties
//! ([`Atmosphere::n_n2`]); the control volume passes the total
//! non-condensable moles there (see its module documentation for the limit
//! this sets).

use petir::mathf::RealMath;
use tampines_steam_tables::gas_mixture::melcor_ncg::cv_n2;
use tampines_steam_tables::prelude::{alpha_v_tp_eqm_single_phase, sat_pressure_4};
use tampines_steam_tables::region_1_subcooled_liquid::intensive_properties::v_tp_1;
use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

/// Molar masses, kg/mol, and the gas constant, J/(mol K)
/// (`sembawang::bwr_mark1::params::{M_H2O, M_N2, R_GAS}`).
pub const M_H2O: f64 = 0.018_015;
/// Nitrogen's molar mass, kg/mol.
pub const M_N2: f64 = 0.028_014;
/// The gas constant, J/(mol K).
pub const R_GAS: f64 = 8.314_462;
/// Gravity, m/s² (`pool_surface::G` and `Containment::bubble_through`'s
/// 9.81).
pub(crate) const G: f64 = 9.81;
/// Stefan-Boltzmann constant, W/(m² K⁴) (`pool_surface::SIGMA`).
const SIGMA: f64 = 5.670_374e-8;

// ── Water and steam (IF97 where `sembawang` uses it) ─────────────────────────

/// Saturation pressure, Pa (IF97 region 4; `steam::p_sat_pa`).
pub fn p_sat_pa(t_k: f64) -> f64 {
    sat_pressure_4(ThermodynamicTemperature::new::<kelvin>(t_k)).get::<pascal>()
}

/// Saturation temperature at `p`, K, by bisection on IF97's saturation
/// pressure (`pool_surface::t_sat`, operation for operation).
pub fn t_sat(p: f64) -> f64 {
    let (mut a, mut b) = (273.16, 640.0);
    for _ in 0..50 {
        let m = 0.5 * (a + b);
        if p_sat_pa(m) > p {
            b = m;
        } else {
            a = m;
        }
    }
    0.5 * (a + b)
}

/// Steam's specific enthalpy, `u + R T / M`, J/kg (`pool_surface::steam_h`).
pub fn steam_h(t: f64) -> f64 {
    2.5060e6 + 1500.0 * (t - 373.15) + R_GAS / M_H2O * t
}

/// Liquid density, kg/m³, IF97 region 1 at `t` and at least just above its
/// saturation pressure (`sparc::Pool::rho_l`, its clamps kept).
pub(crate) fn rho_liquid(t_k: f64, p_pa: f64) -> f64 {
    let t = ThermodynamicTemperature::new::<kelvin>(t_k.min(640.0));
    let p = Pressure::new::<pascal>(p_pa.max(p_sat_pa(t_k.min(640.0)) * 1.0001));
    1.0 / v_tp_1(t, p).get::<cubic_meter_per_kilogram>()
}

// ── Pool surface (`sembawang::bwr_mark1::pool_surface`, steam and N2) ────────

/// Linear interpolation in a table of `(T, value)`, held at its ends
/// (`pool_surface::table`).
fn table(t: f64, xs: &[(f64, f64)]) -> f64 {
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
/// §4.1.2), 273–673 K (`pool_surface::K_STEAM`).
const K_STEAM: [(f64, f64); 21] = [
    (273.15, 0.0176), (293.15, 0.0188), (313.15, 0.0201), (333.15, 0.0216), (353.15, 0.0231), (373.15, 0.0245), (393.15, 0.0260),
    (413.15, 0.0277), (433.15, 0.0295), (453.15, 0.0313), (473.15, 0.0331), (493.15, 0.0351), (513.15, 0.0371), (533.15, 0.0391),
    (553.15, 0.0412), (573.15, 0.0433), (593.15, 0.0455), (613.15, 0.0478), (633.15, 0.0501), (653.15, 0.0525), (673.15, 0.0548),
];
/// Steam's viscosity, Pa s (§5.1.2), 273–673 K (`pool_surface::MU_STEAM`).
const MU_STEAM: [(f64, f64); 11] = [
    (273.15, 8.04e-6), (313.15, 9.66e-6), (353.15, 1.13e-5), (393.15, 1.29e-5), (433.15, 1.46e-5), (473.15, 1.62e-5), (513.15, 1.78e-5),
    (553.15, 1.94e-5), (593.15, 2.11e-5), (633.15, 2.27e-5), (673.15, 2.43e-5),
];
/// Liquid water's conductivity, W/(m K) (§4.1.1), 273–473 K
/// (`pool_surface::K_WATER`).
const K_WATER: [(f64, f64); 21] = [
    (273.15, 0.569), (283.15, 0.586), (293.15, 0.602), (303.15, 0.617), (313.15, 0.630), (323.15, 0.643), (333.15, 0.653), (343.15, 0.662),
    (353.15, 0.669), (363.15, 0.675), (373.15, 0.680), (383.15, 0.683), (393.15, 0.685), (403.15, 0.687), (413.15, 0.687), (423.15, 0.686),
    (433.15, 0.684), (443.15, 0.681), (453.15, 0.676), (463.15, 0.671), (473.15, 0.664),
];
/// Liquid water's viscosity, Pa s (§5.1.1), 283–500 K
/// (`pool_surface::MU_WATER`).
const MU_WATER: [(f64, f64); 9] = [
    (283.148, 1.30962e-3), (310.926, 6.81596e-4), (338.704, 4.34554e-4), (366.482, 3.05081e-4), (394.260, 2.35136e-4), (422.038, 1.86025e-4),
    (449.816, 1.56261e-4), (477.594, 1.35426e-4), (499.850, 1.17267e-4),
];

/// Liquid water's conductivity at `t_k`, W/(m K): MELCOR's table
/// ([`K_WATER`], NCG/H2O RM §4.1.1), held at its ends. For the layered
/// pool's conduction (gh:#1100); the table is the one the surface uses.
pub(crate) fn k_water(t_k: f64) -> f64 {
    table(t_k, &K_WATER)
}

/// Steam's molar `c_v`, J/(mol K) (`pool_surface::CV_STEAM_MOLAR`).
const CV_STEAM_MOLAR: f64 = 1500.0 * M_H2O;

/// Wilke's `phi_ij` (NCG/H2O RM Eq 5-2), also Mason and Saxena's `A_ij`
/// (Eq 4-2) (`pool_surface::phi`).
fn phi(mu_i: f64, mu_j: f64, m_i: f64, m_j: f64) -> f64 {
    let a = 1.0 + (mu_i / mu_j).sqrt() * (m_j / m_i).r_powf(0.25);
    a * a / (8.0 * (1.0 + m_i / m_j)).sqrt()
}

/// The gas space's bulk state, for the surface correlations
/// (`pool_surface::Atmosphere` without hydrogen).
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
}

impl Atmosphere {
    /// Viscosity, Pa s, and conductivity, W/(m K), of steam and of N2:
    /// MELCOR's steam tables and N2's power laws (`pool_surface::Gas`).
    fn pure(t: f64) -> [(f64, f64, f64); 2] {
        [
            (M_H2O, table(t, &MU_STEAM), table(t, &K_STEAM)),
            (M_N2, 3.953_978_5e-7 * t.r_powf(0.672_88), 3.704_689e-4 * t.r_powf(0.748_42)),
        ]
    }
    /// Mixture viscosity, Pa s (Wilke, Eq 5-2) and conductivity, W/(m K)
    /// (Wassijewa with Mason and Saxena, Eq 4-2)
    /// (`pool_surface::Atmosphere::viscosity_and_conductivity`).
    pub fn viscosity_and_conductivity(&self) -> (f64, f64) {
        let n = self.n_steam + self.n_n2;
        let x = [self.n_steam / n, self.n_n2 / n];
        let g = Self::pure(self.t_k);
        let (mut mu_mix, mut k_mix) = (0.0, 0.0);
        for i in 0..2 {
            if x[i] <= 0.0 {
                continue;
            }
            let d: f64 = (0..2).map(|j| x[j] * phi(g[i].1, g[j].1, g[i].0, g[j].0)).sum();
            mu_mix += x[i] * g[i].1 / d;
            k_mix += x[i] * g[i].2 / d;
        }
        (mu_mix, k_mix)
    }
    /// Molar mass, kg/mol, density, kg/m³, and `c_p`, J/(kg K)
    /// (`pool_surface::Atmosphere::mass_properties`).
    pub fn mass_properties(&self) -> (f64, f64, f64) {
        let n = self.n_steam + self.n_n2;
        let mass = self.n_steam * M_H2O + self.n_n2 * M_N2;
        let molar = mass / n;
        let cp_molar = self.n_steam * (CV_STEAM_MOLAR + R_GAS) + self.n_n2 * (cv_n2(self.t_k, true) + R_GAS);
        (molar, self.p_pa * molar / (R_GAS * self.t_k), cp_molar / mass)
    }
    /// Diffusivity of water vapour in N2, m²/s: MELCOR's steam-air
    /// correlation (Eq 6-1, used for N2 as `sembawang` states); `None`
    /// with no N2 (`pool_surface::Atmosphere::steam_diffusivity`).
    pub fn steam_diffusivity(&self) -> Option<f64> {
        let n_nc = self.n_n2;
        if n_nc <= 0.0 {
            return None;
        }
        let d_n2 = 4.7931e-5 * self.t_k.r_powf(1.9) / self.p_pa;
        Some(1.0 / (self.n_n2 / n_nc / d_n2))
    }
}

/// The pool's bulk state for the surface (`pool_surface::Pool`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PoolState {
    /// Temperature, K.
    pub t_k: f64,
    /// Mass, kg.
    pub kg: f64,
    /// Density, kg/m³.
    pub rho: f64,
    /// Heat capacity, J/(kg K).
    pub cp: f64,
}

/// Geometry of the surface: its area, m², and the depths of the pool and of
/// the gas space, m (`pool_surface::Surface`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceGeometry {
    /// Area, m².
    pub area_m2: f64,
    /// Pool depth, m.
    pub pool_depth_m: f64,
    /// Gas-space depth, m.
    pub gas_depth_m: f64,
}

/// The surface's result for one step (`pool_surface::Exchange`):
/// evaporation, kg/s (negative is condensation), the heat flows from the pool
/// and from the gas to the surface, W, and the surface temperature, K.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceExchange {
    /// Evaporation, kg/s.
    pub evaporation_kg_s: f64,
    /// Heat from the pool to the surface, W.
    pub q_pool_w: f64,
    /// Heat from the gas to the surface, W.
    pub q_atm_w: f64,
    /// Surface temperature, K.
    pub t_surface_k: f64,
}

/// Natural convection on a horizontal surface, W/(m² K), at least `k / L`
/// (MELCOR CVH/FL RM Eqs 5-20, 5-22 to 5-25; `pool_surface::h_free`).
#[allow(clippy::too_many_arguments)]
fn h_free(pool_side: bool, k: f64, mu: f64, rho: f64, cp: f64, beta: f64, dt: f64, d_s: f64, l: f64) -> f64 {
    let x = d_s.min(l);
    let gr_pr = G * beta * dt.abs() * x * x * x * (rho / mu) * (rho / mu) * (mu * cp / k);
    let nu = if pool_side { 0.27 * gr_pr.r_powf(0.25) } else { (0.54 * gr_pr.r_powf(0.25)).max(0.14 * gr_pr.r_powf(1.0 / 3.0)) };
    (nu * k / x).max(k / l)
}

/// IF97's isobaric expansion coefficient of liquid water, 1/K, held inside
/// region 1 (`pool_surface::water_beta`).
fn water_beta(t: f64, p: f64) -> f64 {
    let t = if t.is_finite() { t.clamp(274.0, 620.0) } else { 300.0 };
    let p = if p.is_finite() { p.min(1.0e8) } else { 1.0e5 };
    let p = p.max(1.01 * p_sat_pa(t));
    alpha_v_tp_eqm_single_phase(ThermodynamicTemperature::new::<kelvin>(t), Pressure::new::<pascal>(p)).get::<per_kelvin>()
}

/// MELCOR's pool-surface exchange (CVH/FL RM §5.1.1–5.1.2) at the start of a
/// step of `dt` s, limited as MELCOR limits it (Eq 5-17; condensation at most
/// 90 % of the vapour). **Port of `sembawang::bwr_mark1::pool_surface::exchange`**,
/// operation for operation with hydrogen removed; ~~`tests/wet_well.rs` checks~~ **CORRECTED 2026-11-01**: `crates/sembawang/tests/wet_well_vs_tampines.rs` checks
/// it bit for bit against the original with no hydrogen.
///
/// The surface temperature satisfies, at once, vapour diffusion through the
/// N2 (Eq 5-1, `C` from the heat-mass analogy, Eq 5-2), the surface energy
/// balance (Eq 5-7: natural convection plus radiation on each side) and
/// saturation (Eq 5-5), solved by bisection. Without N2 the surface is at the
/// saturation temperature of the total pressure (Eq 5-6).
pub fn exchange(pool: &PoolState, atm: &Atmosphere, surface: &SurfaceGeometry, dt: f64) -> SurfaceExchange {
    let (mu_a, k_a) = atm.viscosity_and_conductivity();
    let (_, rho_a, cp_a) = atm.mass_properties();
    let (mu_p, k_p) = (table(pool.t_k, &MU_WATER), table(pool.t_k, &K_WATER));
    let beta_p = water_beta(pool.t_k, atm.p_pa);
    let d_s = (4.0 * surface.area_m2 / std::f64::consts::PI).sqrt();
    let h_p = |t_l: f64| {
        h_free(true, k_p, mu_p, pool.rho, pool.cp, beta_p, pool.t_k - t_l, d_s, surface.pool_depth_m)
            + SIGMA * (pool.t_k * pool.t_k + t_l * t_l) * (pool.t_k + t_l)
    };
    let h_conv_a = |t_l: f64| h_free(false, k_a, mu_a, rho_a, cp_a, 1.0 / atm.t_k, atm.t_k - t_l, d_s, surface.gas_depth_m);
    let h_a = |t_l: f64| h_conv_a(t_l) + SIGMA * (atm.t_k * atm.t_k + t_l * t_l) * (atm.t_k + t_l);
    let h_f = pool.cp * (pool.t_k - 273.15);
    let h_g = steam_h(atm.t_k);
    let h_fg = h_g - h_f;
    let p_w = atm.n_steam / (atm.n_steam + atm.n_n2) * atm.p_pa;
    let t_top = t_sat(atm.p_pa);
    let t_l = match atm.steam_diffusivity() {
        None => t_top,
        Some(d) => {
            let (_, rho_mix, _) = atm.mass_properties();
            let sc = mu_a / (rho_mix * d);
            let pr = mu_a * cp_a / k_a;
            let rho_v = atm.p_pa * M_H2O / (R_GAS * t_top);
            let c = |t_l: f64| rho_v * d * h_conv_a(t_l) / k_a * (sc / pr).r_powf(1.0 / 3.0);
            let flux = |t_l: f64| c(t_l) * ((atm.p_pa - p_w) / (atm.p_pa - p_sat_pa(t_l))).r_ln();
            let g = |t_l: f64| h_p(t_l) * (pool.t_k - t_l) + h_a(t_l) * (atm.t_k - t_l) - h_fg * flux(t_l);
            let mut lo = pool.t_k.min(atm.t_k).min(t_sat(p_w)).min(t_top) - 1e-3;
            let mut hi = t_top;
            for _ in 0..60 {
                let mid = 0.5 * (lo + hi);
                if g(mid) > 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            0.5 * (lo + hi)
        }
    };
    let mut q_p = h_p(t_l) * (pool.t_k - t_l) * surface.area_m2;
    let mut q_a = h_a(t_l) * (atm.t_k - t_l) * surface.area_m2;
    if q_p > 0.0 {
        q_p = q_p.min(pool.kg * pool.cp * (pool.t_k - t_l) / dt);
    }
    if q_a > 0.0 {
        let m_a = atm.n_steam * M_H2O + atm.n_n2 * M_N2;
        q_a = q_a.min(m_a * cp_a * (atm.t_k - t_l) / dt);
    }
    let mut m = (q_p + q_a) / h_fg;
    let m_max = 0.9 * atm.n_steam * M_H2O / dt;
    if m < -m_max {
        let f = m_max / -m;
        m *= f;
        q_p *= f;
        q_a *= f;
    }
    SurfaceExchange { evaporation_kg_s: m, q_pool_w: q_p, q_atm_w: q_a, t_surface_k: t_l }
}

/// MELCOR's bubble efficiency at a vent below the pool (CVH/FL RM Eqs 6-1 to
/// 6-3): `ε_z = (depth − 0.01 m) / h` with `h` the opening's height, and
/// `ε_T = (T_sat(P_j) − T_P − 0.1 K) / 5 K` with `P_j` the pressure at the
/// opening's depth, each clamped to [0, 1]. **Port of
/// `sembawang::bwr_mark1::pool_surface::bubble_efficiency`.**
pub fn bubble_efficiency(depth_m: f64, opening_m: f64, p_junction_pa: f64, t_pool_k: f64) -> f64 {
    let e_z = ((depth_m - 0.01) / opening_m).clamp(0.0, 1.0);
    let e_t = ((t_sat(p_junction_pa) - t_pool_k - 0.1) / 5.0).clamp(0.0, 1.0);
    e_z * e_t
}
