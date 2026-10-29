//! Mass and energy transfer at the surface of the suppression pool, between
//! the pool and the wetwell atmosphere (2026-10-29, gh:#1047 follow-up, the
//! `ww_atmosphere` mechanism). AI draft, clean room: written from the
//! MELCOR 2.2 reference manuals' equations, never from MELCOR source.
//!
//! - **The interface** (CVH/FL RM §5.1.1-5.1.2, Eqs 5-1 to 5-23): the
//!   surface temperature `T_l` satisfies, at once, the diffusion of water
//!   vapour through the non-condensables (Eq 5-1, the coefficient `C` from
//!   the heat-mass analogy, Eq 5-2), the energy balance at the surface (Eq
//!   5-7, the pool's and the atmosphere's heat flows by natural convection
//!   plus radiation, Eqs 5-9, 5-10, 5-18 to 5-25) and saturation at `T_l`
//!   (Eq 5-5). Solved by bisection on `T_l`, as MELCOR does ("bound and
//!   bisect"). With no non-condensables the surface is at the saturation
//!   temperature of the total pressure (Eq 5-6).
//! - **The bubbles** entering the pool at a vent (CVH/FL RM §6.1, Eqs 6-1 to
//!   6-3): an efficiency `epsilon = epsilon_z epsilon_T`, its full result the
//!   non-condensables leaving at the pool's temperature with water vapour at
//!   0.99 of its saturation pressure, interpolated linearly to no
//!   interaction at `epsilon = 0`.
//! - **Properties** (NCG/H2O RM §4-6): MELCOR's default tables for steam,
//!   water and hydrogen, the power-law fits for nitrogen, Eucken's
//!   correlation for hydrogen's conductivity, Wilke's mixing rule for
//!   viscosity and Wassijewa's with Mason and Saxena's `A_ij` for
//!   conductivity, and the steam-air and steam-hydrogen diffusivities
//!   (Eqs 6-1, 6-2; nitrogen takes air's, stated).
//!
//! **Simplifications, stated:** no forced convection (MELCOR's Eq 5-21
//! needs the volume's velocity, which this model does not have); the pool's
//! heat capacity is the model's constant `c_p`; the pool's expansion
//! coefficient is IAPWS-IF97's.

use super::gas::cv_h2;
use super::params::*;
use super::steam::p_sat_pa;
use petir::mathf::RealMath;
use tampines_steam_tables::prelude::alpha_v_tp_eqm_single_phase;
use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

/// Stefan-Boltzmann constant, W/(m^2 K^4).
const SIGMA: f64 = 5.670_374e-8;
const G: f64 = 9.81;

/// Linear interpolation in a table of `(T, value)`, held at its ends.
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
/// Liquid water's conductivity, W/(m K) (§4.1.1, its Ref. [3]), 273-473 K.
const K_WATER: [(f64, f64); 21] = [
    (273.15, 0.569), (283.15, 0.586), (293.15, 0.602), (303.15, 0.617), (313.15, 0.630), (323.15, 0.643), (333.15, 0.653), (343.15, 0.662),
    (353.15, 0.669), (363.15, 0.675), (373.15, 0.680), (383.15, 0.683), (393.15, 0.685), (403.15, 0.687), (413.15, 0.687), (423.15, 0.686),
    (433.15, 0.684), (443.15, 0.681), (453.15, 0.676), (463.15, 0.671), (473.15, 0.664),
];
/// Liquid water's viscosity, Pa s (§5.1.1, its Ref. [6]), 283-500 K.
const MU_WATER: [(f64, f64); 9] = [
    (283.148, 1.30962e-3), (310.926, 6.81596e-4), (338.704, 4.34554e-4), (366.482, 3.05081e-4), (394.260, 2.35136e-4), (422.038, 1.86025e-4),
    (449.816, 1.56261e-4), (477.594, 1.35426e-4), (499.850, 1.17267e-4),
];

/// The three gases of the wetwell atmosphere.
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
            Gas::Steam => table(t, &MU_STEAM),
            Gas::N2 => 3.953_978_5e-7 * t.r_powf(0.672_88),
            Gas::H2 => table(t, &MU_H2),
        }
    }
    /// Conductivity, W/(m K): steam's table, nitrogen's power law
    /// `3.704689e-4 T^0.74842` (§4.4, Table 4.1), hydrogen by Eucken's
    /// correlation `(c_v + 9 R / (4 M)) mu` (Eq 4-1; MELCOR has no default
    /// table for it).
    fn conductivity(self, t: f64) -> f64 {
        match self {
            Gas::Steam => table(t, &K_STEAM),
            Gas::N2 => 3.704_689e-4 * t.r_powf(0.748_42),
            Gas::H2 => (cv_h2(t, true) / M_H2 + 9.0 * R_GAS / (4.0 * M_H2)) * table(t, &MU_H2),
        }
    }
}

/// Wilke's `phi_ij` (NCG/H2O RM Eq 5-2), also Mason and Saxena's `A_ij`
/// (Eq 4-2): the same form.
fn phi(mu_i: f64, mu_j: f64, m_i: f64, m_j: f64) -> f64 {
    let a = 1.0 + (mu_i / mu_j).sqrt() * (m_j / m_i).r_powf(0.25);
    a * a / (8.0 * (1.0 + m_i / m_j)).sqrt()
}

/// The wetwell atmosphere's bulk properties.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Atmosphere {
    pub t_k: f64,
    pub p_pa: f64,
    /// Moles of steam, nitrogen and hydrogen.
    pub n_steam: f64,
    pub n_n2: f64,
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
    pub(crate) fn mass_properties(&self) -> (f64, f64, f64) {
        let n = self.n_steam + self.n_n2 + self.n_h2;
        let mass = self.n_steam * M_H2O + self.n_n2 * M_N2 + self.n_h2 * M_H2;
        let molar = mass / n;
        let cp_molar = self.n_steam * (CV_STEAM_MOLAR + R_GAS) + self.n_n2 * (super::gas::cv_n2(self.t_k, true) + R_GAS) + self.n_h2 * (cv_h2(self.t_k, true) + R_GAS);
        (molar, self.p_pa * molar / (R_GAS * self.t_k), cp_molar / mass)
    }
    /// Diffusivity of water vapour in the non-condensables, m^2/s: MELCOR's
    /// steam-air (Eq 6-1, used for N2, stated) and steam-hydrogen (Eq 6-2)
    /// correlations, combined by Blanc's law over the non-condensables'
    /// mole fractions. `None` with no non-condensables.
    pub(crate) fn steam_diffusivity(&self) -> Option<f64> {
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

/// The pool's bulk state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pool {
    pub t_k: f64,
    pub kg: f64,
    pub rho: f64,
    pub cp: f64,
}

/// Geometry of the surface: its area, m^2, and the depths of the pool and
/// of the gas space above it, m (each its volume over the area).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    pub area_m2: f64,
    pub pool_depth_m: f64,
    pub gas_depth_m: f64,
}

/// The surface's result for one step: evaporation, kg/s (negative is
/// condensation), and the heat flows from the pool and from the atmosphere
/// to the surface, W.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Exchange {
    pub evaporation_kg_s: f64,
    pub q_pool_w: f64,
    pub q_atm_w: f64,
    pub t_surface_k: f64,
}

/// Saturation temperature at `p`, K (bisection on IF97's saturation
/// pressure).
pub(crate) fn t_sat(p: f64) -> f64 {
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

/// Natural convection on a horizontal surface, W/(m^2 K), at least the
/// conduction limit `k / L` (Eqs 5-20, 5-22 to 5-25): `X = min(D_s, L)`,
/// `Gr = g beta |dT| X^3 (rho / mu)^2`; the pool's laminar and turbulent
/// forms are the same, `0.27 (Gr Pr)^1/4`, the atmosphere's `max(0.54 (Gr
/// Pr)^1/4, 0.14 (Gr Pr)^1/3)`.
fn h_free(pool_side: bool, k: f64, mu: f64, rho: f64, cp: f64, beta: f64, dt: f64, d_s: f64, l: f64) -> f64 {
    let x = d_s.min(l);
    let gr_pr = G * beta * dt.abs() * x * x * x * (rho / mu) * (rho / mu) * (mu * cp / k);
    let nu = if pool_side { 0.27 * gr_pr.r_powf(0.25) } else { (0.54 * gr_pr.r_powf(0.25)).max(0.14 * gr_pr.r_powf(1.0 / 3.0)) };
    (nu * k / x).max(k / l)
}

/// IAPWS-IF97's isobaric expansion coefficient of liquid water, 1/K, at
/// `t` and at least 1 % above its saturation pressure (region 1). Held
/// inside region 1 (274-620 K, at most 100 MPa) so that a malformed state
/// (the demo's truncated-message tests send NaN) cannot panic the tables.
fn water_beta(t: f64, p: f64) -> f64 {
    let t = if t.is_finite() { t.clamp(274.0, 620.0) } else { 300.0 };
    let p = if p.is_finite() { p.min(1.0e8) } else { 1.0e5 };
    let p = p.max(1.01 * p_sat_pa(t));
    alpha_v_tp_eqm_single_phase(ThermodynamicTemperature::new::<kelvin>(t), Pressure::new::<pascal>(p)).get::<per_kelvin>()
}

/// MELCOR's pool-surface exchange (CVH/FL RM §5.1.1-5.1.2), at the start
/// of a step of `dt` s and limited as MELCOR limits it: each phase's
/// sensible heat to its content above the surface temperature (Eq 5-17),
/// and condensation to 90 % of the atmosphere's vapour (C4407).
pub fn exchange(pool: &Pool, atm: &Atmosphere, surface: &Surface, dt: f64) -> Exchange {
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
    // Latent heat at bulk conditions (Eq 5-8): the pool's liquid enthalpy
    // as the containment carries it, the atmosphere's vapour likewise.
    let h_f = pool.cp * (pool.t_k - 273.15);
    let h_g = steam_h(atm.t_k);
    let h_fg = h_g - h_f;
    let p_w = atm.n_steam / (atm.n_steam + atm.n_n2 + atm.n_h2) * atm.p_pa;
    let t_top = t_sat(atm.p_pa);
    let t_l = match atm.steam_diffusivity() {
        // Pure steam (Eq 5-6 with P_A = P_w,A): the surface is saturated at
        // the total pressure, the flux whatever the heat flows give.
        None => t_top,
        Some(d) => {
            // C from the analogy (Eq 5-2), rho_v the saturated vapour's
            // density at the total pressure.
            let (_, rho_mix, _) = atm.mass_properties();
            let sc = mu_a / (rho_mix * d);
            let pr = mu_a * cp_a / k_a;
            let rho_v = atm.p_pa * M_H2O / (R_GAS * t_top);
            let c = |t_l: f64| rho_v * d * h_conv_a(t_l) / k_a * (sc / pr).r_powf(1.0 / 3.0);
            let flux = |t_l: f64| c(t_l) * ((atm.p_pa - p_w) / (atm.p_pa - p_sat_pa(t_l))).r_ln();
            // g decreases in T_l: >= 0 at or below the dew point and both
            // bulk temperatures, to minus infinity at T_sat(P_A).
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
    // Eq 5-17: sensible heat at most what lies above the surface.
    if q_p > 0.0 {
        q_p = q_p.min(pool.kg * pool.cp * (pool.t_k - t_l) / dt);
    }
    if q_a > 0.0 {
        let m_a = atm.n_steam * M_H2O + atm.n_n2 * M_N2 + atm.n_h2 * M_H2;
        q_a = q_a.min(m_a * cp_a * (atm.t_k - t_l) / dt);
    }
    // The rate from the energy balance (Eq 5-7) once limited; it equals
    // the diffusion flux times the area when neither limit binds.
    let mut m = (q_p + q_a) / h_fg;
    // Condensation at most 90 % of the vapour in a step; the heat flows
    // scale with it, so energy is conserved.
    let m_max = 0.9 * atm.n_steam * M_H2O / dt;
    if m < -m_max {
        let f = m_max / -m;
        m *= f;
        q_p *= f;
        q_a *= f;
    }
    Exchange { evaporation_kg_s: m, q_pool_w: q_p, q_atm_w: q_a, t_surface_k: t_l }
}

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

/// Steam's specific enthalpy as the containment carries it, J/kg: `u + R T
/// / M` with the constant `c_v` from IF97's saturated vapour at 100 degC.
pub(crate) fn steam_h(t: f64) -> f64 {
    2.5060e6 + 1500.0 * (t - 373.15) + R_GAS / M_H2O * t
}

/// MELCOR's bubble efficiency at a vent below the pool (CVH/FL RM Eqs 6-1
/// to 6-3): `epsilon_z = (depth - 0.01 m) / h` with `h` the opening's
/// height, and `epsilon_T = (T_sat(P_j) - T_P - 0.1 K) / 5 K` with `P_j`
/// the pressure at the opening's depth, each clamped to [0, 1].
pub fn bubble_efficiency(depth_m: f64, opening_m: f64, p_junction_pa: f64, t_pool_k: f64) -> f64 {
    let e_z = ((depth_m - 0.01) / opening_m).clamp(0.0, 1.0);
    let e_t = ((t_sat(p_junction_pa) - t_pool_k - 0.1) / 5.0).clamp(0.0, 1.0);
    e_z * e_t
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nitrogen at 300 K: MELCOR's power laws give, by hand, `3.9539785e-7
    /// 300^0.67288 = 1.836e-5` Pa s and `3.704689e-4 300^0.74842 = 0.02647`
    /// W/(m K) (Incropera Table A.4: 1.78e-5 and 0.0259, so the fits run 3 %
    /// and 2 % high here). Pure-gas mixing returns the pure gas; the
    /// steam-air diffusivity at 1 bar is `4.7931e-5 300^1.9 / 1e5 = 2.439e-5`
    /// m^2/s.
    #[test]
    fn nitrogen_properties_match_a_hand_value() {
        let a = Atmosphere { t_k: 300.0, p_pa: 1e5, n_steam: 0.0, n_n2: 1.0, n_h2: 0.0 };
        let (mu, k) = a.viscosity_and_conductivity();
        assert!((mu - 1.836e-5).abs() < 0.001e-5 && (k - 0.02647).abs() < 0.00001, "{mu} {k}");
        assert!((a.steam_diffusivity().unwrap() - 2.439e-5).abs() < 0.001e-5, "{:?}", a.steam_diffusivity());
    }

    /// A pool warmer than a dry, cooler atmosphere evaporates into it and
    /// heats it; a saturated atmosphere warmer than its pool condenses on
    /// it. Signs, the bisection's surface between the two, and energy: the
    /// latent heat equals the two heat flows.
    #[test]
    fn the_surface_evaporates_and_condenses_with_the_right_signs() {
        let surface = Surface { area_m2: 750.0, pool_depth_m: 2.3, gas_depth_m: 3.5 };
        let pool = Pool { t_k: 360.0, kg: 1.7e6, rho: 965.0, cp: 4186.0 };
        let n = 1e5 * 2620.0 / (R_GAS * 330.0);
        let dry = Atmosphere { t_k: 330.0, p_pa: 1e5, n_steam: 0.05 * n, n_n2: 0.95 * n, n_h2: 0.0 };
        let e = exchange(&pool, &dry, &surface, 1.0);
        assert!(e.evaporation_kg_s > 0.0 && e.q_pool_w > 0.0 && e.q_atm_w < 0.0, "{e:?}");
        assert!(e.t_surface_k > 330.0 && e.t_surface_k < 360.0, "{e:?}");
        let h_fg = steam_h(330.0) - 4186.0 * (360.0 - 273.15);
        assert!((e.evaporation_kg_s * h_fg - e.q_pool_w - e.q_atm_w).abs() < 1e-6 * e.q_pool_w.abs(), "{e:?}");
        let p_s = p_sat_pa(380.0);
        let humid = Atmosphere { t_k: 380.0, p_pa: p_s + 2e4, n_steam: p_s * 2620.0 / (R_GAS * 380.0), n_n2: 2e4 * 2620.0 / (R_GAS * 380.0), n_h2: 0.0 };
        let e = exchange(&pool, &humid, &surface, 1.0);
        assert!(e.evaporation_kg_s < 0.0 && e.q_pool_w < 0.0, "{e:?}");
        // Pure steam: the surface is at the saturation temperature.
        let steam = Atmosphere { n_n2: 0.0, ..humid };
        let e = exchange(&pool, &Atmosphere { p_pa: p_s, ..steam }, &surface, 1.0);
        assert!((e.t_surface_k - 380.0).abs() < 0.01 && e.evaporation_kg_s < 0.0, "{e:?}");
    }

    /// A 400 K wall under 0.4 MPa of gas at 600 K, 30 % steam, 10 % H2:
    /// turbulent natural convection, by hand `0.10 k (g beta dT rho^2 Pr /
    /// mu^2)^1/3` with beta at the 500 K film, rho 1.80, mu 2.8e-5, Pr 0.8
    /// and k about 0.05 gives about 12 W/(m^2 K), independent of height.
    /// The dew point at 0.12 MPa of steam is 378 K, so that wall stays dry;
    /// a 350 K wall condenses.
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

    /// Eq 6-2's subcooling term: 0 at saturation, 1 from 5.1 K below it; the
    /// downcomers' 1.2 m depth over a 0.5 m opening gives `epsilon_z = 1`.
    #[test]
    fn the_bubble_efficiency_follows_the_subcooling() {
        let p = 2.0e5;
        let ts = t_sat(p);
        assert!((ts - 393.36).abs() < 0.02, "{ts}");
        assert_eq!(bubble_efficiency(1.2, 0.5, p, ts), 0.0);
        assert_eq!(bubble_efficiency(1.2, 0.5, p, ts - 6.0), 1.0);
        assert!((bubble_efficiency(1.2, 0.5, p, ts - 2.6) - 0.5).abs() < 1e-9);
        assert!((bubble_efficiency(0.26, 0.5, p, ts - 6.0) - 0.5).abs() < 1e-9);
    }
}
