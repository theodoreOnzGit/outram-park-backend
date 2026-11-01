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
//! **Since 2026-11-01 the gas-mixture properties and the heat-structure
//! surface ([`structure_surface`], [`Atmosphere`]) live in
//! `tampines::components::wall_condensation`** (gh:#1096), moved verbatim
//! and re-exported here; the pool-surface exchange below stays.
//!
//! **Simplifications, stated:** no forced convection (MELCOR's Eq 5-21
//! needs the volume's velocity, which this model does not have); the pool's
//! heat capacity is the model's constant `c_p`; the pool's expansion
//! coefficient is IAPWS-IF97's.

use super::params::*;
use super::steam::p_sat_pa;
use petir::mathf::RealMath;
use tampines_steam_tables::prelude::alpha_v_tp_eqm_single_phase;
use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

// The heat-structure surface and the gas-mixture transport properties it
// needs moved to `tampines` verbatim on 2026-11-01 (gh:#1096; maintainer
// direction: `sembawang` depends on `tampines`, and the thermal hydraulics,
// wall heat transfer included, lives there). Re-exported here so every path
// and every value is unchanged; `tests/wall_condensation_bit_identity.rs`
// pins the bits against the pre-move code.
pub use tampines::components::wall_condensation::{structure_surface, Atmosphere};
pub(crate) use tampines::components::wall_condensation::{steam_conductivity, steam_viscosity};
use tampines::components::wall_condensation::interpolate_held as table;

/// Stefan-Boltzmann constant, W/(m^2 K^4).
const SIGMA: f64 = 5.670_374e-8;
const G: f64 = 9.81;

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
