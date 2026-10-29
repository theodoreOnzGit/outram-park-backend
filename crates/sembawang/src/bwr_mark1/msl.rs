//! Creep rupture of a main steam line (gh:#1037, plan item F6).
//!
//! The relief valves sit on the main steam lines inside the drywell, so the
//! gas they pass flows through the lines first. When the core's exit gas is
//! hot, it heats the line's wall. Under the vessel's pressure the wall
//! creeps and, if damage reaches 1, one line ruptures and opens its flow
//! area to the drywell. The failure model is MELCOR's pair of control
//! functions for exactly this (no source used; Vol. 1, SAND2017-0455 O, CF
//! §2.1.9, §2.1.10, §7.8):
//!
//! ```text
//! PIPE-STR:  sigma = ((R_o^2 + R_i^2) P_i - 2 R_o^2 P_o) / ((R_o + R_i)(R_o - R_i))
//! LM-CREEP:  t_R = 10^(P_LM/T - C),  P_LM = a1 log10(sigma) + b1,
//!            damage = sum dt / t_R,  failure at 1
//! ```
//!
//! MELCOR has no built-in steam-line model; its users build this from the
//! two functions and a heat structure, as here. The wall is one lumped node
//! per metre of line (the length cancels):
//!
//! - inside, forced convection from the relief flow, Dittus-Boelter on the
//!   line's diameter with a laminar floor Nu = 4.36 when the valves are
//!   shut, at ~~the core's exit gas temperature. The gas temperature is an
//!   **upper bound**: the steam dome's separators and dryers would cool it,
//!   and those heat structures are not modelled (the other half of F6);~~
//!   **CORRECTED 2026-10-29:** the temperature of the gas after the steam
//!   dome's internals (`Plant::gas_out_k`; the internals heat structure,
//!   `Mechanisms::dome_internals`, on by default), the core's exit gas only
//!   with that mechanism ablated;
//! - outside, insulation to the drywell gas (`msl_insulation_h`, \[A\]);
//! - the wall's enthalpy is 304L stainless steel's (`core::steel_h`), a
//!   stated substitution: no carbon-steel property set exists in the
//!   workspace, and carbon steel's magnetic transition near 1033 K (a `c_p`
//!   peak) is missed.
//!
//! The ablation, [`Mechanisms::without_msl_creep`](super::Mechanisms), is
//! the model before 2026-10-29: no steam-line failure.

use super::core::{steel_h, steel_rho, steel_t};
use super::params::Params;
use petir::mathf::RealMath;
use tampines_steam_tables::dynamic_viscosity::mu_tp_eqm_single_phase;
use tampines_steam_tables::prelude::cp_tp_eqm_single_phase;
use tampines_steam_tables::thermal_conductivity::lambda_tp_eqm_single_phase;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

/// One main steam line's wall: temperature, accumulated creep damage, and
/// whether it has ruptured.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SteamLine {
    pub wall_k: f64,
    pub damage: f64,
    pub failed: bool,
}

/// MELCOR's PIPE-STR: the inner-surface stress of a thick-walled pipe, Pa.
pub fn pipe_stress(r_i: f64, r_o: f64, p_in: f64, p_out: f64) -> f64 {
    ((r_o * r_o + r_i * r_i) * p_in - 2.0 * r_o * r_o * p_out) / ((r_o + r_i) * (r_o - r_i))
}

/// MELCOR's LM-CREEP time to rupture, s, at stress `sigma` (Pa) and wall
/// temperature `t` (K), with the material's `a1`, `b1`, `c`.
pub fn rupture_time_s(prm: &Params, sigma: f64, t: f64) -> f64 {
    if sigma <= 0.0 {
        return f64::INFINITY;
    }
    let p_lm = prm.msl_lm_a1 * sigma.r_log10() + prm.msl_lm_b1;
    10f64.r_powf(p_lm / t - prm.msl_lm_c)
}

/// Inside heat transfer coefficient, W/(m^2 K), for gas flow `w_line`
/// (kg/s) through one line at film temperature `t_film` and pressure `p`.
fn inside_h(prm: &Params, w_line: f64, t_film: f64, p_pa: f64) -> f64 {
    let t = ThermodynamicTemperature::new::<kelvin>(t_film.clamp(300.0, 2200.0));
    let p = Pressure::new::<pascal>(p_pa.max(1.0e4));
    let cp = cp_tp_eqm_single_phase(t, p).get::<joule_per_kilogram_kelvin>();
    let k = lambda_tp_eqm_single_phase(t, p).get::<watt_per_meter_kelvin>();
    let mu = mu_tp_eqm_single_phase(t, p).get::<pascal_second>();
    let d = 2.0 * prm.msl_inner_radius_m;
    let re = 4.0 * w_line / (std::f64::consts::PI * d * mu);
    let pr = mu * cp / k;
    let nu = (0.023 * re.r_powf(0.8) * pr.r_powf(0.4)).max(4.36);
    nu * k / d
}

impl SteamLine {
    pub fn new(t_k: f64) -> Self {
        Self { wall_k: t_k, damage: 0.0, failed: false }
    }

    /// Advance by `dt` s: relief flow `w_relief` (kg/s, shared by the
    /// lines) at gas temperature `t_gas`, vessel pressure `p`, drywell
    /// pressure `p_dw` and temperature `t_dw`. The wall is implicit in its
    /// two exchanges at the start-of-step heat capacity, then set from its
    /// enthalpy, so its energy is conserved.
    pub fn step(&mut self, prm: &Params, w_relief: f64, t_gas: f64, p: f64, p_dw: f64, t_dw: f64, dt: f64) {
        if self.failed {
            return;
        }
        let (r_i, r_o) = (prm.msl_inner_radius_m, prm.msl_inner_radius_m + prm.msl_wall_m);
        let pi = std::f64::consts::PI;
        let m = pi * (r_o * r_o - r_i * r_i) * steel_rho();
        let t = self.wall_k;
        let c = m * (steel_h(t + 0.5) - steel_h(t - 0.5));
        let w_line = w_relief / prm.msl_lines;
        let ha_in = inside_h(prm, w_line, 0.5 * (t + t_gas), p) * 2.0 * pi * r_i;
        let ha_out = prm.msl_insulation_h * 2.0 * pi * r_o;
        let t_new = (c * t + dt * (ha_in * t_gas + ha_out * t_dw)) / (c + dt * (ha_in + ha_out));
        let q = ha_in * (t_gas - t_new) - ha_out * (t_new - t_dw);
        self.wall_k = steel_t(steel_h(t) + q * dt / m);
        let sigma = pipe_stress(r_i, r_o, p, p_dw);
        self.damage += dt / rupture_time_s(prm, sigma, self.wall_k);
        if self.damage >= 1.0 {
            self.failed = true;
        }
    }

    /// The ruptured line's flow area, m^2 (one line).
    pub fn break_area_m2(prm: &Params) -> f64 {
        std::f64::consts::PI * prm.msl_inner_radius_m * prm.msl_inner_radius_m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pieces against hand values:
    /// - PIPE-STR at the default line (R_i 0.254, R_o 0.279 m) under 8 MPa
    ///   inside and 0.1 MPa outside: (0.14235 x 8e6 - 0.155682 x 1e5) /
    ///   (0.533 x 0.025) = 84.3 MPa (the thin-wall p r / t is 81 MPa);
    /// - LM-CREEP, A-508, at 67 MPa: P_LM = -5335 x 7.826 + 62291 = 20540
    ///   K, so t_R is 1.26e4 s at 1000 K and 170 s at 1100 K;
    /// - a wall with no flow and the drywell at its own temperature stays
    ///   put; one swept by hot gas heats, and a line held hot under
    ///   pressure fails.
    #[test]
    fn a_steam_line_creeps_by_melcors_functions() {
        let prm = Params::default();
        let s = pipe_stress(0.254, 0.279, 8.0e6, 1.0e5);
        assert!((s / 84.3e6 - 1.0).abs() < 2e-3, "{s}");
        let t1 = rupture_time_s(&prm, 67.0e6, 1000.0);
        assert!((t1.r_log10() - 1.26e4f64.r_log10()).abs() < 0.01, "{t1}");
        let t2 = rupture_time_s(&prm, 67.0e6, 1100.0);
        assert!((t2.r_log10() - 170f64.r_log10()).abs() < 0.02, "{t2}");
        assert!(rupture_time_s(&prm, 0.0, 1500.0).is_infinite());
        let mut l = SteamLine::new(560.0);
        l.step(&prm, 0.0, 560.0, 7.0e6, 1.0e5, 560.0, 1.0);
        assert!((l.wall_k - 560.0).abs() < 1e-9 && !l.failed, "{l:?}");
        let mut hot = SteamLine::new(560.0);
        for _ in 0..600 {
            hot.step(&prm, 50.0, 1500.0, 8.0e6, 1.0e5, 400.0, 1.0);
        }
        assert!(hot.wall_k > 1000.0, "{hot:?}");
        assert!(hot.failed, "{hot:?}");
        assert!((SteamLine::break_area_m2(&prm) - 0.2027).abs() < 1e-3);
    }
}
