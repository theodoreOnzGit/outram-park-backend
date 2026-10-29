//! Water and steam properties for the plant model, all from IAPWS-IF97 as
//! `tampines-steam-tables` implements it (`tampines_steam_tables`, ~~a
//! dev-dependency of this crate; ~~through the metacrate~~ directly since the
//! demo moved here on 2026-10-27~~ a normal dependency of `sembawang` since
//! the model moved out of the `dhoby-ghaut` demo on 2026-10-29, gh:#1033).
//!
//! The RPV is solved every step for the saturation pressure whose mixture
//! matches the vessel's mass and energy, which takes a few dozen saturation
//! evaluations. So the saturation line is **tabulated once** at start-up
//! from the steam tables (500 points, log-spaced in pressure from 0.02 to
//! 9.5 MPa) and interpolated linearly in `ln p`. Every worker builds the same
//! table from the same calls, so it is identical everywhere (the three-worker
//! step is bit-for-bit the one-worker step).
//!
//! **Interpolation error** (measured by `the_table_matches_the_steam_tables_between_its_points`):
//! below 3e-4 relative on `T_sat`, `v_f`, `v_g`, `u_f`, `u_g`, `h_f`, `h_g`
//! at the midpoints of the table between 0.05 and 9 MPa.

use petir::mathf::RealMath;
use tampines_steam_tables::prelude::*;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;

/// Lowest and highest pressure in the table, Pa.
pub const P_MIN_PA: f64 = 2.0e4;
pub const P_MAX_PA: f64 = 9.5e6;
const N: usize = 500;

/// Saturated-water properties at one pressure.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sat {
    pub p_pa: f64,
    pub t_k: f64,
    pub v_f: f64,
    pub v_g: f64,
    pub u_f: f64,
    pub u_g: f64,
    pub h_f: f64,
    pub h_g: f64,
}

/// One saturation point straight from the steam tables.
pub fn sat_from_tables(p_pa: f64) -> Sat {
    let p = Pressure::new::<pascal>(p_pa);
    let t = sat_temp_4(p);
    let v_f = v_tp_eqm_two_phase(t, p, 0.0).get::<cubic_meter_per_kilogram>();
    let v_g = v_tp_eqm_two_phase(t, p, 1.0).get::<cubic_meter_per_kilogram>();
    let h_f = h_tp_eqm_two_phase(t, p, 0.0).get::<joule_per_kilogram>();
    let h_g = h_tp_eqm_two_phase(t, p, 1.0).get::<joule_per_kilogram>();
    // u = h - p v, exactly the IF97 identity (the tables' own u_tp agrees).
    Sat { p_pa, t_k: t.get::<kelvin>(), v_f, v_g, u_f: h_f - p_pa * v_f, u_g: h_g - p_pa * v_g, h_f, h_g }
}

/// Saturation pressure at a temperature, Pa (IF97 region 4, closed form).
pub fn p_sat_pa(t_k: f64) -> f64 {
    sat_pressure_4(ThermodynamicTemperature::new::<kelvin>(t_k)).get::<pascal>()
}

/// The tabulated saturation line.
#[derive(Clone, Debug)]
pub struct SatTable {
    ln_p: Vec<f64>,
    rows: Vec<Sat>,
}

impl SatTable {
    pub fn new() -> Self {
        let (a, b) = (P_MIN_PA.r_ln(), P_MAX_PA.r_ln());
        let ln_p: Vec<f64> = (0..N).map(|i| a + (b - a) * i as f64 / (N - 1) as f64).collect();
        let rows = ln_p.iter().map(|l| sat_from_tables(l.r_exp())).collect();
        Self { ln_p, rows }
    }

    /// Saturation properties at `p_pa`, clamped to the table's range.
    pub fn at(&self, p_pa: f64) -> Sat {
        let l = p_pa.clamp(P_MIN_PA, P_MAX_PA).r_ln();
        let step = self.ln_p[1] - self.ln_p[0];
        let i = (((l - self.ln_p[0]) / step).floor() as usize).min(N - 2);
        let w = (l - self.ln_p[i]) / step;
        let (r0, r1) = (&self.rows[i], &self.rows[i + 1]);
        let mix = |a: f64, b: f64| a + w * (b - a);
        Sat {
            p_pa: l.r_exp(),
            t_k: mix(r0.t_k, r1.t_k),
            v_f: mix(r0.v_f, r1.v_f),
            v_g: mix(r0.v_g, r1.v_g),
            u_f: mix(r0.u_f, r1.u_f),
            u_g: mix(r0.u_g, r1.u_g),
            h_f: mix(r0.h_f, r1.h_f),
            h_g: mix(r0.h_g, r1.h_g),
        }
    }

    /// The saturated mixture whose specific volume is `v` and specific
    /// internal energy `u` (J/kg): bisection on `ln p`. Returns the state
    /// and the quality `x`. `None` when no saturated state in the table's
    /// range matches (the vessel has boiled dry or left the range): the
    /// model's limit, reported by the caller, never papered over.
    pub fn mixture(&self, v: f64, u: f64) -> Option<(Sat, f64)> {
        let resid = |l: f64| {
            let s = self.at(l.r_exp());
            let x = (v - s.v_f) / (s.v_g - s.v_f);
            (s.u_f + x * (s.u_g - s.u_f) - u, s, x)
        };
        let (mut a, mut b) = (P_MIN_PA.r_ln(), P_MAX_PA.r_ln());
        let (fa, _, _) = resid(a);
        let (fb, _, _) = resid(b);
        if fa > 0.0 || fb < 0.0 {
            return None;
        }
        for _ in 0..60 {
            let m = 0.5 * (a + b);
            if resid(m).0 > 0.0 {
                b = m;
            } else {
                a = m;
            }
        }
        let (_, s, x) = resid(0.5 * (a + b));
        (0.0..=1.0).contains(&x).then_some((s, x))
    }
}

impl SatTable {
    /// Superheated steam past dry-out, continued from the saturated vapour
    /// with the same specific volume `v`: `T = T_sat + (u - u_g)/c_v`,
    /// `p = p_sat T/T_sat` (the saturated vapour's compressibility kept).
    /// Continuous with [`SatTable::mixture`] at `x = 1`. `c_v` constant. \[A\]
    /// Returns `(p, T)`.
    pub fn superheated(&self, v: f64, u: f64) -> (f64, f64) {
        const CV_STEAM: f64 = 1700.0;
        let (mut a, mut b) = (P_MIN_PA.r_ln(), P_MAX_PA.r_ln());
        for _ in 0..60 {
            let m = 0.5 * (a + b);
            // v_g falls with pressure.
            if self.at(m.r_exp()).v_g > v {
                a = m;
            } else {
                b = m;
            }
        }
        let s = self.at((0.5 * (a + b)).r_exp());
        let t = s.t_k + ((u - s.u_g) / CV_STEAM).max(0.0);
        (s.p_pa * t / s.t_k, t)
    }
}

impl Default for SatTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: compare the table at the midpoint between every pair of
    /// rows (the worst place for linear interpolation) with a direct
    /// steam-table evaluation, between 0.05 and 9 MPa. Result (2026-10-21):
    /// worst relative error below 3e-4 on every property; the bound is the
    /// assertion.
    #[test]
    fn the_table_matches_the_steam_tables_between_its_points() {
        let t = SatTable::new();
        let mut worst: f64 = 0.0;
        for i in 0..N - 1 {
            let p = (0.5 * (t.ln_p[i] + t.ln_p[i + 1])).r_exp();
            if !(5.0e4..=9.0e6).contains(&p) {
                continue;
            }
            let (a, b) = (t.at(p), sat_from_tables(p));
            for (x, y) in [(a.t_k, b.t_k), (a.v_f, b.v_f), (a.v_g, b.v_g), (a.h_f, b.h_f), (a.h_g, b.h_g), (a.u_f, b.u_f), (a.u_g, b.u_g)] {
                worst = worst.max(((x - y) / y).abs());
            }
        }
        assert!(worst < 3e-4, "worst relative interpolation error {worst:e}");
    }

    /// A mixture built at a known pressure and quality is found again.
    #[test]
    fn the_mixture_solve_inverts_the_table() {
        let t = SatTable::new();
        let s = t.at(7.0e6);
        let x = 0.03;
        let v = s.v_f + x * (s.v_g - s.v_f);
        let u = s.u_f + x * (s.u_g - s.u_f);
        let (m, xm) = t.mixture(v, u).expect("in range");
        assert!((m.p_pa - 7.0e6).abs() < 7.0e6 * 1e-9 && (xm - x).abs() < 1e-9, "{} {}", m.p_pa, xm);
    }
}
