//! Molten core-concrete interaction on the drywell floor, a reduced form of
//! MELCOR's CAV package (CORCON-Mod3; Vol. 2, CAV RM §3; 2026-10-29,
//! `Mechanisms::mcci`, plan item F8). AI draft, clean room: written from the
//! manuals' description, never from MELCOR or CORCON source.
//!
//! - **One debris layer**, lumped: UO2, metallic Zr and oxides (ZrO2 and the
//!   ablated concrete's oxides) at one temperature, heated by its decay
//!   power and by the oxidation of its Zr. No layering (MELCOR stratifies
//!   and mixes layers, §3.2). ~~No phase change, no crust (stated).~~
//!   **Since 2026-10-30, with `Mechanisms::cavity_phases`:** the oxide
//!   phase melts over a solidus-liquidus range from MELCOR's ideal-solution
//!   pseudo-binary (§3.10: fuel, UO2 and ZrO2, the high-melting component,
//!   the concrete's oxides the low-melting one), its enthalpy linear in
//!   between, metallic Zr melting at MELCOR's 2098 K; and the debris's
//!   interior conducts, at `6 k / H` to each face of a layer `H` deep, when
//!   solid, convects at `h` when liquid, linearly by liquid fraction between
//!   (§3.3: "the interior of a layer may be fluid ... while one or more of
//!   its ... surfaces is covered by a solid crust, with heat transfer by
//!   conduction"; stated: the crust is the whole layer below the solidus).
//! - **Downward**, MELCOR's quasi-steady ablation (§3.3, 3.4): while the
//!   debris is above the concrete's ablation temperature, the heat to the
//!   concrete is `h A (T - T_a)` and it ablates `q / h_a` kg/s, the preheated
//!   region neglected, as MELCOR neglects it. `h` is \[A\]: CORCON's gas-film
//!   and bubbling correlations are not held.
//! - **Upward**, with no water above (§3.3: "radiation and convection in the
//!   absence of overlying water"): from the debris's interior to its top
//!   surface by the same `h`, and from the surface by gray radiation to the
//!   drywell's structures at MELCOR's default emissivities; the surface
//!   temperature from the continuity of the two fluxes.
//! - **Chemistry** (§3.5): the concrete's H2O and CO2 pass through the debris
//!   and oxidise its metallic Zr to H2 and CO, completely while Zr lasts
//!   (equilibrium favours it; MELCOR's default suppresses coking, so no
//!   condensed carbon), the reaction heat staying in the debris; the
//!   concrete's other constituents join the oxides. Reduction of the
//!   concrete's oxides by Zr is not modelled. The gases leave at the
//!   ablation temperature (stated: MELCOR equilibrates them with the layers
//!   they pass).
//!
//! - **Under water** (§3.3: "a complete pool boiling curve"): the top
//!   surface in film boiling, Berenson's horizontal-surface correlation
//!   (J. Heat Transfer 83, 1961) with the vapour's properties at the film
//!   temperature, plus radiation across the vapour film to the water at
//!   the debris's emissivity; the surface temperature again from the
//!   continuity of the fluxes. The nucleate and transition regimes are not
//!   separated (stated: the debris surface here stays far above
//!   saturation). The concrete ablates beneath, as CORCON's does.

use super::params::*;
use super::pool_surface::{steam_conductivity, steam_viscosity};
use super::sparc;
use petir::mathf::RealMath;

/// Metallic Zr's melting point, K, and heat of fusion, J/kg: MELCOR's MP
/// defaults (Tables 7 and 8), as the core uses.
const ZR_MELT_K: f64 = 2098.0;
const ZR_FUSION_J_PER_KG: f64 = 2.25e5;

const M_ZR: f64 = 0.091_224;
const M_ZRO2: f64 = 0.123_222;
const M_CO2: f64 = 0.044_01;
const SIGMA: f64 = 5.670_374e-8;

/// The debris on the drywell floor.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cavity {
    /// UO2, metallic Zr and oxides, kg, and the debris temperature, K.
    pub uo2_kg: f64,
    pub zr_kg: f64,
    pub oxide_kg: f64,
    pub t_k: f64,
    /// The core's debris inventory last seen (what has arrived), kg.
    pub seen_uo2_kg: f64,
    pub seen_zr_kg: f64,
    /// Concrete ablated, kg, and the H2 and CO made, kg.
    pub ablated_kg: f64,
    pub h2_kg: f64,
    pub co_kg: f64,
    /// The ZrO2 formed here, kg (counted in `oxide_kg` too; the melting
    /// range counts it as fuel, `cavity_phases`).
    pub zro2_kg: f64,
}

/// What one step of the cavity sends to the drywell.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CavityFlows {
    /// Radiated up to the drywell's structures, W.
    pub q_up_w: f64,
    /// Gases released, at `gas_k`: steam kg, H2, CO and CO2 mol.
    pub steam_kg: f64,
    pub h2_mol: f64,
    pub co_mol: f64,
    pub co2_mol: f64,
    pub gas_k: f64,
}

impl Cavity {
    /// The debris's heat capacity, J/K.
    fn heat_capacity(&self, prm: &Params) -> f64 {
        self.uo2_kg * prm.uo2_cp + self.zr_kg * prm.zr_cp + self.oxide_kg * prm.concrete_cp
    }

    /// Debris arriving: the core's inventory now is `uo2_kg` and `zr_kg`
    /// (metal) at `t_k`; what is new mixes in at that temperature.
    pub fn arrive(&mut self, prm: &Params, uo2_kg: f64, zr_kg: f64, t_k: f64) {
        let (du, dz) = ((uo2_kg - self.seen_uo2_kg).max(0.0), (zr_kg - self.seen_zr_kg).max(0.0));
        self.seen_uo2_kg = self.seen_uo2_kg.max(uo2_kg);
        self.seen_zr_kg = self.seen_zr_kg.max(zr_kg);
        if du + dz <= 0.0 {
            return;
        }
        let c_old = self.heat_capacity(prm);
        let c_new = du * prm.uo2_cp + dz * prm.zr_cp;
        self.t_k = (c_old * self.t_k + c_new * t_k) / (c_old + c_new);
        self.uo2_kg += du;
        self.zr_kg += dz;
    }

    /// The top surface's temperature, K, and the heat it radiates, W, to
    /// surroundings at `t_wall_k`: `h A (T - T_s) = sigma e A (T_s^4 -
    /// T_w^4)` by bisection (zero when the debris is not hotter).
    fn top_loss(&self, prm: &Params, t_wall_k: f64, h_int: f64) -> (f64, f64) {
        if self.t_k <= t_wall_k {
            return (self.t_k, 0.0);
        }
        let e = 1.0 / (2.0 / prm.cavity_emissivity - 1.0);
        let rad = |ts: f64| SIGMA * e * prm.cavity_area_m2 * (ts.r_powi(4) - t_wall_k.r_powi(4));
        let conv = |ts: f64| h_int * prm.cavity_area_m2 * (self.t_k - ts);
        let (mut lo, mut hi) = (t_wall_k, self.t_k);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if rad(mid) < conv(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let ts = 0.5 * (lo + hi);
        (ts, conv(ts))
    }

    /// The top surface's loss under water at saturation `t_sat` and
    /// pressure `p_pa`, W: `h A (T - T_s)` equal to film boiling (Berenson)
    /// plus radiation across the film, by bisection on `T_s`.
    fn wet_top_loss(&self, prm: &Params, t_sat: f64, p_pa: f64, h_int: f64) -> f64 {
        if self.t_k <= t_sat {
            return 0.0;
        }
        let pool = sparc::Pool::submerged(t_sat, p_pa, 0.0);
        let (rho_l, sigma) = (pool.rho_l(), pool.sigma());
        let h_fg = 2.257e6;
        let film = |ts: f64| {
            let dt_s = ts - t_sat;
            let t_f = 0.5 * (ts + t_sat);
            let rho_v = p_pa * M_H2O / (R_GAS * t_f);
            let (k_v, mu_v) = (steam_conductivity(t_f), steam_viscosity(t_f));
            let lambda = (sigma / (9.81 * (rho_l - rho_v))).sqrt();
            let h_fg_eff = h_fg + 0.5 * 2000.0 * dt_s;
            let h_b = 0.425 * (k_v * k_v * k_v * rho_v * (rho_l - rho_v) * 9.81 * h_fg_eff / (mu_v * dt_s * lambda)).r_powf(0.25);
            h_b * dt_s + SIGMA * prm.cavity_emissivity * (ts.r_powi(4) - t_sat.r_powi(4))
        };
        let conv = |ts: f64| h_int * (self.t_k - ts);
        let (mut lo, mut hi) = (t_sat + 1e-6, self.t_k);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if film(mid) < conv(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        conv(0.5 * (lo + hi)) * prm.cavity_area_m2
    }

    /// The oxide phase's fuel and low-melting moles: UO2 and ZrO2 against
    /// the concrete's oxides (as SiO2, `concrete_oxide_m`, \[A\]).
    fn oxide_moles(&self, prm: &Params) -> (f64, f64) {
        (self.uo2_kg / 0.270_03 + self.zro2_kg / M_ZRO2, (self.oxide_kg - self.zro2_kg).max(0.0) / prm.concrete_oxide_m)
    }

    /// The oxide phase's solidus and liquidus, K: MELCOR's pseudo-binary with
    /// ideal liquid and solid solutions (CAV RM §3.10), from each
    /// component's melting point and heat of fusion: with `K_i = exp((dH_i
    /// / R)(1/T - 1/T_i))` the liquid's fuel fraction is `(1 - K_c) / (K_f
    /// - K_c)` and the solid's `K_f` times it; the liquidus is where the
    /// liquid's fraction is the debris's, the solidus where the solid's is.
    pub(crate) fn melting_range(&self, prm: &Params) -> (f64, f64) {
        let (n_f, n_c) = self.oxide_moles(prm);
        let (t_f, t_c) = (prm.fuel_melt_k, prm.concrete_ablation_k);
        if n_f + n_c <= 0.0 || n_c <= 0.0 {
            return (t_f, t_f);
        }
        if n_f <= 0.0 {
            return (t_c, t_c);
        }
        let x = n_f / (n_f + n_c);
        let (h_f, h_c) = (prm.fuel_fusion_j_per_kg * 0.270_03, prm.concrete_oxide_fusion_j_per_mol);
        let k = |h: f64, tm: f64, t: f64| (h / R_GAS * (1.0 / t - 1.0 / tm)).r_exp();
        let liquid = |t: f64| {
            let (kf, kc) = (k(h_f, t_f, t), k(h_c, t_c, t));
            (1.0 - kc) / (kf - kc)
        };
        let solid = |t: f64| k(h_f, t_f, t) * liquid(t);
        let solve = |g: &dyn Fn(f64) -> f64| {
            let (mut lo, mut hi) = (t_c + 1e-9, t_f - 1e-9);
            for _ in 0..80 {
                let mid = 0.5 * (lo + hi);
                if g(mid) < x {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            0.5 * (lo + hi)
        };
        (solve(&solid), solve(&liquid))
    }

    /// The debris's enthalpy at `t`, J, above 0 K on its constant specific
    /// heats, with the oxide phase's latent heat spread linearly over its
    /// melting range and the metallic Zr's over 10 K from 2098 K
    /// (`cavity_phases`); and the oxide's liquid fraction at `t`.
    fn enthalpy(&self, prm: &Params, t: f64, range: (f64, f64)) -> (f64, f64) {
        let (t_s, t_l) = range;
        let f_ox = if t_l - t_s < 1.0 { ((t - t_s) / 1.0).clamp(0.0, 1.0) } else { ((t - t_s) / (t_l - t_s)).clamp(0.0, 1.0) };
        let f_zr = ((t - ZR_MELT_K) / 10.0).clamp(0.0, 1.0);
        let (_, n_c) = self.oxide_moles(prm);
        let latent = (self.uo2_kg + self.zro2_kg) * prm.fuel_fusion_j_per_kg + n_c * prm.concrete_oxide_fusion_j_per_mol;
        (self.heat_capacity(prm) * t + latent * f_ox + self.zr_kg * ZR_FUSION_J_PER_KG * f_zr, f_ox)
    }

    /// The temperature at which the debris holds enthalpy `e`, K (bisection;
    /// the enthalpy rises with temperature).
    fn temperature_of(&self, prm: &Params, e: f64, range: (f64, f64)) -> f64 {
        let (mut lo, mut hi) = (1.0, 6000.0);
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if self.enthalpy(prm, mid, range).0 < e {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    }

    /// The interior's conductance to each face, W/(m^2 K): `h` when the
    /// oxide is liquid, `6 k / H` when solid (a layer `H` deep with uniform
    /// heating, its mean to a face), linear in the liquid fraction between.
    fn interior_h(&self, prm: &Params, f_liquid: f64) -> f64 {
        let v = self.uo2_kg / prm.uo2_density + self.zr_kg / prm.zr_density + self.zro2_kg / prm.zro2_density
            + (self.oxide_kg - self.zro2_kg).max(0.0) / prm.concrete_density;
        let depth = (v / prm.cavity_area_m2).max(1e-3);
        let solid = 6.0 * prm.debris_k / depth;
        solid + f_liquid * (prm.cavity_h - solid)
    }

    /// One step of `dt` s with decay power `q_w` and the drywell's
    /// structures at `t_wall_k`; `water` is `Some((T_sat, p))` when water
    /// lies over the debris, whose top then boils it (`q_up_w` is then the
    /// heat into the water) rather than radiating to the structures.
    /// With `phases` (`cavity_phases`) the melting range, latent heats and
    /// the interior's conductance by phase apply.
    pub fn step(&mut self, prm: &Params, q_w: f64, t_wall_k: f64, water: Option<(f64, f64)>, dt: f64, phases: bool) -> CavityFlows {
        let c = self.heat_capacity(prm);
        if c <= 0.0 {
            return CavityFlows { gas_k: prm.concrete_ablation_k, ..Default::default() };
        }
        if phases {
            return self.step_phases(prm, q_w, t_wall_k, water, dt);
        }
        let q_up = match water {
            Some((t_sat, p)) => self.wet_top_loss(prm, t_sat, p, prm.cavity_h),
            None => self.top_loss(prm, t_wall_k, prm.cavity_h).1,
        };
        let t_a = prm.concrete_ablation_k;
        let q_down = if self.t_k > t_a { prm.cavity_h * prm.cavity_area_m2 * (self.t_k - t_a) } else { 0.0 };
        let m_c = q_down * dt / prm.concrete_ablation_j_per_kg;
        let n_h2o = m_c * prm.concrete_h2o_fraction / M_H2O;
        let n_co2 = m_c * prm.concrete_co2_fraction / M_CO2;
        // Zr + 2 H2O -> ZrO2 + 2 H2 and Zr + 2 CO2 -> ZrO2 + 2 CO, both while
        // Zr lasts, in proportion when it runs short.
        let need = 0.5 * (n_h2o + n_co2);
        let frac = if need > 0.0 { (self.zr_kg / M_ZR / need).min(1.0) } else { 0.0 };
        let (n_h2, n_co) = (frac * n_h2o, frac * n_co2);
        let (zr_h2o, zr_co2) = (0.5 * n_h2 * M_ZR, 0.5 * n_co * M_ZR);
        let q_rx = zr_h2o * prm.oxidation_heat_j_per_kg_zr + zr_co2 * prm.zr_co2_heat_j_per_kg;
        self.zr_kg = (self.zr_kg - zr_h2o - zr_co2).max(0.0);
        let t_new = self.t_k + ((q_w - q_up - q_down) * dt + q_rx) / c;
        // ZrO2 formed, and the concrete's condensed products, at the
        // ablation temperature, mix in.
        let zro2 = (zr_h2o + zr_co2) / M_ZR * M_ZRO2;
        let ox = m_c * (1.0 - prm.concrete_h2o_fraction - prm.concrete_co2_fraction);
        let c_after = c - (zr_h2o + zr_co2) * prm.zr_cp;
        let c_add = (zro2 + ox) * prm.concrete_cp;
        self.t_k = (c_after * t_new + c_add * t_a) / (c_after + c_add);
        self.oxide_kg += zro2 + ox;
        self.zro2_kg += zro2;
        self.ablated_kg += m_c;
        self.h2_kg += n_h2 * M_H2;
        self.co_kg += n_co * 0.028_01;
        CavityFlows { q_up_w: q_up, steam_kg: (n_h2o - n_h2) * M_H2O, h2_mol: n_h2, co_mol: n_co, co2_mol: n_co2 - n_co, gas_k: t_a }
    }

    /// The step with phases (`cavity_phases`): the same fluxes and chemistry,
    /// the interior's conductance by phase, and the energy balance on the
    /// enthalpy with latent heats. The ablated concrete's oxides join
    /// molten at the ablation temperature (CORCON's ablation enthalpy
    /// melts them), the ZrO2 formed solid at it (as before, stated).
    fn step_phases(&mut self, prm: &Params, q_w: f64, t_wall_k: f64, water: Option<(f64, f64)>, dt: f64) -> CavityFlows {
        let range = self.melting_range(prm);
        let (e0, f_liq) = self.enthalpy(prm, self.t_k, range);
        let h_int = self.interior_h(prm, f_liq);
        let q_up = match water {
            Some((t_sat, p)) => self.wet_top_loss(prm, t_sat, p, h_int),
            None => self.top_loss(prm, t_wall_k, h_int).1,
        };
        let t_a = prm.concrete_ablation_k;
        let q_down = if self.t_k > t_a { h_int * prm.cavity_area_m2 * (self.t_k - t_a) } else { 0.0 };
        let m_c = q_down * dt / prm.concrete_ablation_j_per_kg;
        let n_h2o = m_c * prm.concrete_h2o_fraction / M_H2O;
        let n_co2 = m_c * prm.concrete_co2_fraction / M_CO2;
        let need = 0.5 * (n_h2o + n_co2);
        let frac = if need > 0.0 { (self.zr_kg / M_ZR / need).min(1.0) } else { 0.0 };
        let (n_h2, n_co) = (frac * n_h2o, frac * n_co2);
        let (zr_h2o, zr_co2) = (0.5 * n_h2 * M_ZR, 0.5 * n_co * M_ZR);
        let q_rx = zr_h2o * prm.oxidation_heat_j_per_kg_zr + zr_co2 * prm.zr_co2_heat_j_per_kg;
        // The Zr consumed leaves with its enthalpy at the debris's
        // temperature, the products join at the ablation temperature.
        let zr_used = (zr_h2o + zr_co2).min(self.zr_kg);
        let f_zr = ((self.t_k - ZR_MELT_K) / 10.0).clamp(0.0, 1.0);
        let e_zr_out = zr_used * (prm.zr_cp * self.t_k + ZR_FUSION_J_PER_KG * f_zr);
        let zro2 = zr_used / M_ZR * M_ZRO2;
        let ox = m_c * (1.0 - prm.concrete_h2o_fraction - prm.concrete_co2_fraction);
        let e_in = (zro2 + ox) * prm.concrete_cp * t_a + ox / prm.concrete_oxide_m * prm.concrete_oxide_fusion_j_per_mol;
        let e = e0 + (q_w - q_up - q_down) * dt + q_rx - e_zr_out + e_in;
        self.zr_kg -= zr_used;
        self.oxide_kg += zro2 + ox;
        self.zro2_kg += zro2;
        self.t_k = self.temperature_of(prm, e, self.melting_range(prm));
        self.ablated_kg += m_c;
        self.h2_kg += n_h2 * M_H2;
        self.co_kg += n_co * 0.028_01;
        CavityFlows { q_up_w: q_up, steam_kg: (n_h2o - n_h2) * M_H2O, h2_mol: n_h2, co_mol: n_co, co2_mol: n_co2 - n_co, gas_k: t_a }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hand case: 80 t of UO2 and 5 t of Zr at 2400 K over 21.2 m^2 with
    /// h = 1000: the bottom takes `1000 x 21.2 x (2400 - 1450)` = 20.14 MW,
    /// so 20.14 / 1.816 = 11.09 kg of concrete in one second, its 0.650 kg of
    /// water and 0.166 kg of CO2 all reduced by the Zr (36.1 + 3.78 mol), and
    /// the top
    /// radiates less than the bottom conducts. Mass: Zr used is half the gas
    /// moles.
    #[test]
    fn a_hot_pool_ablates_and_reduces_the_concrete_gases() {
        let prm = Params::default();
        let mut c = Cavity::default();
        c.arrive(&prm, 80_000.0, 5_000.0, 2400.0);
        assert_eq!(c.t_k, 2400.0);
        let f = c.step(&prm, 4.7e6, 650.0, None, 1.0, false);
        let m_c = 1000.0 * 21.2 * 950.0 / prm.concrete_ablation_j_per_kg;
        assert!((c.ablated_kg - m_c).abs() < 1e-9 * m_c && (m_c - 11.09).abs() < 0.01, "{m_c}");
        assert!((f.h2_mol - m_c * 0.0586 / M_H2O).abs() < 1e-9 && f.steam_kg.abs() < 1e-12, "{f:?}");
        assert!((f.co_mol - m_c * 0.015 / M_CO2).abs() < 1e-9 && f.co2_mol.abs() < 1e-12, "{f:?}");
        assert!((5_000.0 - c.zr_kg - 0.5 * (f.h2_mol + f.co_mol) * M_ZR).abs() < 1e-9);
        assert!(f.q_up_w > 0.0 && f.q_up_w < 20.1e6, "{f:?}");
        // Without Zr the gases leave as steam and CO2.
        let mut d = Cavity::default();
        d.arrive(&prm, 80_000.0, 0.0, 2400.0);
        let g = d.step(&prm, 0.0, 650.0, None, 1.0, false);
        assert!(g.h2_mol == 0.0 && g.steam_kg > 0.6 && g.co2_mol > 3.7, "{g:?}");
        // Below the ablation temperature nothing ablates.
        let mut e = Cavity::default();
        e.arrive(&prm, 80_000.0, 5_000.0, 1400.0);
        let h = e.step(&prm, 0.0, 650.0, None, 1.0, false);
        assert!(e.ablated_kg == 0.0 && h.h2_mol == 0.0);
        // Under water at 1 bar the top boils off a film-boiling flux: by hand
        // Berenson's h at a 1500 K surface is of order 200-300 W/(m^2 K), and
        // radiation adds about 0.25 MW/m^2, so 0.4-1 MW/m^2 over 21.2 m^2.
        let mut w = Cavity::default();
        w.arrive(&prm, 80_000.0, 5_000.0, 2400.0);
        let q = w.wet_top_loss(&prm, 373.15, 1.0e5, prm.cavity_h);
        assert!(q > 0.4e6 * 21.2 && q < 1.2e6 * 21.2, "{q}");
    }
}
