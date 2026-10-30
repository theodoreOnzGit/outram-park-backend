//! Debris falling through the pedestal's water: a reduced form of MELCOR's
//! FDI package, low-pressure melt ejection (Vol. 2, FDI RM §2.2, the
//! Corradini et al. model; 2026-10-30, `Mechanisms::fdi_lpme`). AI draft,
//! clean room: written from the manual's description, never from MELCOR
//! source.
//!
//! - **The stream** leaves the breach (`breach_diameter_m`) at MELCOR's
//!   Bernoulli velocity (COR RM §6, Eq 6-34), `Cd (2 dP / rho + 2 g
//!   dz)^(1/2)`, `dz` the bottom cell's height (`lp_bottom_level_m`,
//!   \[A\]). Above `fdi_hpme_velocity` (C4602, 10 m/s) MELCOR switches to
//!   high-pressure ejection, which needs user transport fractions; that
//!   case is flagged and still falls as here (stated).
//! - **Breakup** while molten, by Chu's correlation as MELCOR modifies it
//!   (Eqs 2-11 to 2-14): `D(z) = D0 exp(-Z)`, `Z = (C1 We^0.254 / D0)
//!   (rho_c / rho_f)^(1/2) (z_pool - z)`, `C1 = 0.1232 - 0.149 (rho_c /
//!   rho_f)^(1/2)`, `We = rho_c U^2 D0 / sigma_f`; once solid (below Zr's
//!   2098 K, the debris's first component to melt, \[A\]) the diameter
//!   stays.
//! - **Heat** by the dominant one of radiation to the coolant (unit
//!   emissivity) and Bromley's film boiling (Eqs 2-15, 2-16), Eq 2-17
//!   integrated over the depth.
//! - **Steam**: all of it boils water, `Q / (h_g - h_pool)`, the pool's
//!   temperature unchanged (Eq 2-20); capped at the pool, the debris's
//!   temperature then back-calculated.
//! - **Not modelled**: the time of flight inside a step (Eqs 2-1 to 2-6;
//!   seconds against the hour scale, each packet lands in the step it
//!   leaves), oxidation of the falling metal (MELCOR's LPME has none
//!   either), and a steam explosion (nor does MELCOR).

use super::params::*;
use super::pool_surface::{steam_conductivity, steam_viscosity};
use petir::mathf::RealMath;

const SIGMA: f64 = 5.670_374e-8;
const G: f64 = 9.81;
/// Metallic Zr's melting point, K: MELCOR's MP default, as the cavity uses.
const ZR_MELT_K: f64 = 2098.0;
/// Steps of the integration over the depth (RK4).
const STEPS: usize = 200;

/// MELCOR's ejection velocity, m/s (COR RM Eq 6-34): `Cd (2 dP / rho + 2
/// g dz)^(1/2)`, zero when the bracket is negative.
pub(crate) fn ejection_velocity(cd: f64, dp_pa: f64, rho_f: f64, dz_m: f64) -> f64 {
    cd * (2.0 * dp_pa / rho_f + 2.0 * G * dz_m).max(0.0).sqrt()
}

/// Bromley's film-boiling coefficient as MELCOR writes it (FDI RM Eq
/// 2-16), W/(m^2 K): `(1/2) k_g (k_g mu_g D (T_g - T_sat) / (rho_g (rho_c
/// - rho_g) g i_fg))^(-1/4)`, the film at `T_g`, the mean of the debris's
/// and saturation temperatures.
pub(crate) fn bromley_h(t_f: f64, t_sat: f64, d: f64, p_pa: f64, rho_c: f64) -> f64 {
    let t_g = 0.5 * (t_f + t_sat);
    let rho_g = p_pa * M_H2O / (R_GAS * t_g);
    let (k_g, mu_g) = (steam_conductivity(t_g), steam_viscosity(t_g));
    let i_fg = 2.257e6;
    0.5 * k_g * (k_g * mu_g * d * (t_g - t_sat) / (rho_g * (rho_c - rho_g) * G * i_fg)).r_powf(-0.25)
}

/// The falling stream's state: the coolant and debris properties.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fall {
    /// Debris density, kg/m^3, specific heat, J/(kg K), surface tension,
    /// N/m.
    pub rho_f: f64,
    pub c_f: f64,
    pub sigma_f: f64,
    /// Coolant density, kg/m^3, its temperature and saturation, K, and the
    /// pressure, Pa.
    pub rho_c: f64,
    pub t_c: f64,
    pub t_sat: f64,
    pub p_pa: f64,
    /// Stream velocity, m/s, and initial diameter, m.
    pub u: f64,
    pub d0: f64,
}

impl Fall {
    /// The heat flux, W/m^2, at debris temperature `t` and diameter `d`:
    /// the larger of radiation and film boiling, which is MELCOR's switch
    /// at the transition temperature of Eq 2-15.
    pub fn flux(&self, t: f64, d: f64) -> f64 {
        if t <= self.t_sat {
            return 0.0;
        }
        let rad = SIGMA * (t.r_powi(4) - self.t_c.r_powi(4));
        let film = bromley_h(t, self.t_sat, d, self.p_pa, self.rho_c) * (t - self.t_sat);
        rad.max(film)
    }

    /// The breakup exponent per metre fallen, `Z / (z_pool - z)` (Eq 2-14).
    pub fn z_per_m(&self) -> f64 {
        let r = (self.rho_c / self.rho_f).sqrt();
        let c1 = 0.1232 - 0.149 * r;
        let we = self.rho_c * self.u * self.u * self.d0 / self.sigma_f;
        (c1 * we.r_powf(0.254) / self.d0 * r).max(0.0)
    }

    /// The debris's temperature, K, at the floor after falling `depth` m
    /// from the surface at `t0`: Eq 2-17, `rho_f c_f (D / 6) U dT/dz = -q`,
    /// by RK4 over the depth; the diameter follows Eq 2-13 while molten and
    /// freezes at solidification.
    pub fn bottom_temperature(&self, t0: f64, depth: f64) -> f64 {
        if depth <= 0.0 || self.u <= 0.0 {
            return t0;
        }
        let zm = self.z_per_m();
        let h = depth / STEPS as f64;
        let (mut t, mut d_frozen) = (t0, if t0 > ZR_MELT_K { None } else { Some(self.d0) });
        for i in 0..STEPS {
            let s = i as f64 * h;
            let d_at = |s: f64, d_frozen: Option<f64>| d_frozen.unwrap_or(self.d0 * (-zm * s).r_exp());
            let rate = |s: f64, t: f64| -6.0 * self.flux(t, d_at(s, d_frozen)) / (self.rho_f * self.c_f * d_at(s, d_frozen) * self.u);
            let k1 = rate(s, t);
            let k2 = rate(s + 0.5 * h, t + 0.5 * h * k1);
            let k3 = rate(s + 0.5 * h, t + 0.5 * h * k2);
            let k4 = rate(s + h, t + h * k3);
            t = (t + h / 6.0 * (k1 + 2.0 * k2 + 2.0 * k3 + k4)).max(self.t_sat);
            if d_frozen.is_none() && t <= ZR_MELT_K {
                d_frozen = Some(d_at(s + h, None));
            }
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MELCOR's LPME pieces by hand (FDI RM §2.2):
    /// - Eq 6-34 at no pressure difference over 0.5 m of debris, Cd 0.6:
    ///   `0.6 (2 x 9.81 x 0.5)^(1/2)` = 1.8793 m/s; with 0.2 MPa across
    ///   debris of 8000 kg/m^3, `0.6 (50 + 9.81)^(1/2)` = 4.640 m/s.
    /// - Breakup: coolant 958, debris 8000 kg/m^3, `(rho_c / rho_f)^(1/2)`
    ///   = 0.3460, `C1` = 0.07164; at 1.8793 m/s, 0.1 m and 0.45 N/m, `We`
    ///   = 750.9, `We^0.254` = 5.384, so `Z` = 1.335 per metre.
    /// - At 2800 K over coolant at 373 K, radiation 3.50 MW/m^2 outweighs
    ///   film boiling; at 1000 K film boiling is the larger.
    /// - Falling 1 m from 2800 K: a ~0.06 m mean droplet at ~3.5 MW/m^2
    ///   for 0.53 s, `6 q t / (rho c D)` ~ 140 K: it lands between 2600 and
    ///   2750 K; solid debris (2000 K) does not break up and cools less per
    ///   metre than it would broken up.
    #[test]
    fn debris_falls_through_the_pedestal_water_as_melcor_says() {
        assert!((ejection_velocity(0.6, 0.0, 8000.0, 0.5) - 1.8793).abs() < 1e-4);
        assert!((ejection_velocity(0.6, 0.2e6, 8000.0, 0.5) - 4.6403).abs() < 1e-3);
        assert_eq!(ejection_velocity(0.6, -1e6, 8000.0, 0.5), 0.0);
        let f = Fall { rho_f: 8000.0, c_f: 330.0, sigma_f: 0.45, rho_c: 958.0, t_c: 373.15, t_sat: 373.15, p_pa: 1.013e5, u: 1.8793, d0: 0.1 };
        assert!((f.z_per_m() / 1.335 - 1.0).abs() < 2e-3, "{}", f.z_per_m());
        let rad = SIGMA * (2800.0_f64.powi(4) - 373.15_f64.powi(4));
        assert!((rad / 3.50e6 - 1.0).abs() < 0.01 && f.flux(2800.0, 0.05) == rad);
        assert!(f.flux(1000.0, 0.05) > SIGMA * (1000.0_f64.powi(4) - 373.15_f64.powi(4)));
        let t = f.bottom_temperature(2800.0, 1.0);
        assert!(t > 2600.0 && t < 2750.0, "{t}");
        assert_eq!(f.bottom_temperature(2800.0, 0.0), 2800.0);
        // Solid debris keeps its diameter, so it cools less in a metre than
        // a molten stream from the same temperature would broken up.
        let solid = f.bottom_temperature(2000.0, 1.0);
        assert!(solid < 2000.0 && 2000.0 - solid < 2800.0 - t, "{solid} {t}");
    }
}
