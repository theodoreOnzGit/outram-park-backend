//! Fission-product vapours: vapour pressures and condensation onto surfaces
//! (gh:#1047, second deliverable; not yet wired into the plant).
//!
//! **Vapour pressures** are MELCOR's default curves (Vol. 1, SAND2017-0455
//! O, RN sensitivity coefficients 7110; no source used),
//! `log10 P[mmHg] = -A/T + B + C log10 T` over temperature ranges, zero
//! below the first. The classes tracked so far: Xe (always a vapour), Cs
//! (released as CsOH, 7120), I2, and Te (as TeO). **One extracted
//! coefficient is corrected**: the text reads Cs's first-range `A` as
//! 9.4E4, but the manual says these curves were adjusted to be continuous,
//! and only `A = 9.4E3` meets the second range at 1553 K (log10 P = 3.570
//! from both sides; 9.4E4 gives -50.9, a 54-decade miss). The continuity of every
//! range boundary is pinned by a test, which is what makes the correction
//! checkable.
//!
//! **Condensation** onto heat structures, pools and aerosol sections is
//! TRAP-MELT2's, solved analytically over a step as MELCOR does (Vol. 2,
//! SAND2023-10994 O, RN §2.5.2, Eqs 2-96 to 2-101): each surface `i` takes
//! `A_i k_i (C_a - C_i^s)`, the atmosphere gives it up, and over the step
//! `C_a` relaxes exponentially toward the area-weighted saturation
//! concentration. This module does not limit evaporation to what a
//! surface holds; the caller must (stated).

use super::params::R_GAS;
use petir::mathf::RealMath;

const PA_PER_MMHG: f64 = 133.322_368;

/// A fission-product vapour class (MELCOR default names).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FpClass {
    Xe,
    Cs,
    I2,
    Te,
}

/// One range of a vapour-pressure curve: from `t_k`, coefficients A, B, C.
#[derive(Clone, Copy, Debug)]
struct Range {
    t_k: f64,
    a: f64,
    b: f64,
    c: f64,
}

impl FpClass {
    /// Molar mass of the released compound, kg/mol (C7120: CsOH, I2, TeO).
    pub fn molar_mass(self) -> f64 {
        match self {
            FpClass::Xe => 0.1313,
            FpClass::Cs => 0.149_913,
            FpClass::I2 => 0.253_800_8,
            FpClass::Te => 0.1436,
        }
    }

    fn ranges(self) -> &'static [Range] {
        const CS: [Range; 2] = [Range { t_k: 600.0, a: 9.4e3, b: 21.59, c: -3.75 }, Range { t_k: 1553.0, a: 6870.778, b: 7.994503, c: 0.0 }];
        const I2: [Range; 3] = [
            Range { t_k: 273.0, a: 3578.0, b: 17.72, c: -2.51 },
            Range { t_k: 387.0, a: 3205.0, b: 23.66536, c: -5.18 },
            Range { t_k: 457.0, a: 2176.912045, b: 7.637352, c: 0.0 },
        ];
        const TE: [Range; 1] = [Range { t_k: 273.0, a: 13940.0, b: 23.51, c: -3.52 }];
        match self {
            FpClass::Xe => &[],
            FpClass::Cs => &CS,
            FpClass::I2 => &I2,
            FpClass::Te => &TE,
        }
    }

    /// Vapour pressure at `t_k`, Pa: infinite for a permanent gas (Xe), zero
    /// below the curve's first temperature, the last range extrapolated.
    pub fn vapour_pressure_pa(self, t_k: f64) -> f64 {
        let r = self.ranges();
        if r.is_empty() {
            return f64::INFINITY;
        }
        if t_k < r[0].t_k {
            return 0.0;
        }
        let i = r.iter().rposition(|x| x.t_k <= t_k).unwrap_or(0);
        let x = r[i];
        10f64.r_powf(-x.a / t_k + x.b + x.c * t_k.r_log10()) * PA_PER_MMHG
    }

    /// Saturation concentration at `t_k`, kg/m^3 (ideal gas).
    pub fn saturation_concentration(self, t_k: f64) -> f64 {
        self.vapour_pressure_pa(t_k) * self.molar_mass() / (R_GAS * t_k)
    }
}

/// A surface the vapour can condense on: its `A k`, m^3/s (area times
/// mass-transfer coefficient), its saturation concentration at its own
/// temperature, kg/m^3, and the mass condensed on it, kg.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CondensingSurface {
    pub ak: f64,
    pub c_sat: f64,
    pub mass: f64,
}

/// TRAP-MELT2's analytic step (Eqs 2-98 to 2-101): the atmosphere's vapour
/// concentration `c_a`, kg/m^3, in `volume` m^3, against `surfaces`, over
/// `dt` s. Returns the new concentration and updates each surface's mass.
/// Mass is conserved exactly (`c_a V + sum M`).
pub fn condense(c_a: f64, volume: f64, surfaces: &mut [CondensingSurface], dt: f64) -> f64 {
    let sum_ak: f64 = surfaces.iter().map(|s| s.ak).sum();
    if sum_ak <= 0.0 {
        return c_a;
    }
    let alpha = sum_ak / volume;
    let b_over_a = surfaces.iter().map(|s| s.ak * s.c_sat).sum::<f64>() / sum_ak;
    let decay = (-alpha * dt).r_exp();
    let c_new = b_over_a - (b_over_a - c_a) * decay;
    for s in surfaces.iter_mut() {
        s.mass += s.ak * (b_over_a - s.c_sat) * dt - s.ak * (b_over_a - c_a) * (1.0 - decay) / alpha;
    }
    c_new
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The curves: continuous at every range boundary (to 1e-3 in log10 P,
    /// which pins the 9.4E3 correction), zero below the first range, Xe
    /// always a vapour, and CsOH at 1000 K about 8.7 mmHg by hand
    /// (`-9400/1000 + 21.59 - 3.75 x 3 = 0.94`).
    #[test]
    fn vapour_pressures_are_melcors_continuous_curves() {
        for class in [FpClass::Cs, FpClass::I2] {
            let r = class.ranges();
            for w in r.windows(2) {
                let t = w[1].t_k;
                let below = -w[0].a / t + w[0].b + w[0].c * t.r_log10();
                let above = -w[1].a / t + w[1].b + w[1].c * t.r_log10();
                assert!((below - above).abs() < 1e-3, "{class:?} at {t}: {below} vs {above}");
            }
        }
        assert_eq!(FpClass::Cs.vapour_pressure_pa(500.0), 0.0);
        assert!(FpClass::Xe.vapour_pressure_pa(300.0).is_infinite());
        let p = FpClass::Cs.vapour_pressure_pa(1000.0) / PA_PER_MMHG;
        assert!((p - 8.71).abs() < 0.05, "{p}");
    }

    /// TRAP-MELT2's step: mass conserved to round-off; onto one surface
    /// with zero saturation the vapour decays as `exp(-A k t / V)`; and a
    /// long step reaches the area-weighted saturation concentration.
    #[test]
    fn the_trap_melt_step_conserves_and_relaxes() {
        let v = 100.0;
        let mut s = [CondensingSurface { ak: 2.0, c_sat: 0.0, mass: 0.0 }, CondensingSurface { ak: 1.0, c_sat: 3e-3, mass: 0.5 }];
        let before = 1e-3 * v + 0.5;
        let c = condense(1e-3, v, &mut s, 30.0);
        assert!((c * v + s[0].mass + s[1].mass - before).abs() < 1e-12);
        let mut one = [CondensingSurface { ak: 2.0, c_sat: 0.0, mass: 0.0 }];
        let c = condense(1e-3, v, &mut one, 30.0);
        assert!((c - 1e-3 * (-2.0 * 30.0 / v).r_exp()).abs() < 1e-15);
        let c = condense(1e-3, v, &mut s, 1e6);
        assert!((c - 1e-3).abs() < 1e-12, "{c}"); // (2 x 0 + 1 x 3e-3)/3
    }
}
