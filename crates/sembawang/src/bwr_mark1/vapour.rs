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
//!
//! **Vapour diffusivity** is Chapman-Enskog's (Vol. 2, RN §2.5.2, Eqs 2-104,
//! 2-105, after Bird, Stewart and Lightfoot), with the Lennard-Jones
//! parameters of the classes (Vol. 1, RN coefficients 7111) and of the bulk
//! gases (Vol. 2, NCG/H2O §5.2), and the collision integral of Vol. 2's
//! NCG/H2O Table 6.1 with its extrapolations. An aerosol section takes
//! vapour with the mass-transfer coefficient of Sherwood number 2 (RN
//! §2.5.2).

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

/// A bulk gas of the containment or vessel atmosphere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BulkGas {
    Steam,
    H2,
    N2,
}

impl BulkGas {
    /// Lennard-Jones sigma, Angstrom, and epsilon/k, K (NCG/H2O §5.2), and
    /// molar mass, g/mol.
    fn lj(self) -> (f64, f64, f64) {
        match self {
            BulkGas::Steam => (2.641, 809.1, 18.015),
            BulkGas::H2 => (2.827, 59.7, 2.016),
            BulkGas::N2 => (3.798, 71.4, 28.014),
        }
    }
}

/// The diffusion collision integral at reduced temperature `t_star`
/// (NCG/H2O Table 6.1, linear between its points; `2.662 (T*/0.3)^-0.5`
/// below 0.3 and `0.5170 (T*/100)^-0.155` from 100, as there).
fn collision_integral(t_star: f64) -> f64 {
    const T: [f64; 75] = [
        0.30, 0.35, 0.40, 0.45, 0.50, 0.55, 0.60, 0.65, 0.70, 0.75, 0.80, 0.85, 0.90, 0.95, 1.00, 1.05, 1.10, 1.15, 1.20, 1.25, 1.30, 1.35, 1.40,
        1.45, 1.50, 1.55, 1.60, 1.65, 1.70, 1.75, 1.80, 1.85, 1.90, 1.95, 2.00, 2.10, 2.20, 2.30, 2.40, 2.50, 2.60, 2.70, 2.80, 2.90, 3.00, 3.10,
        3.20, 3.30, 3.40, 3.50, 3.60, 3.70, 3.80, 3.90, 4.00, 4.10, 4.20, 4.30, 4.40, 4.50, 4.60, 4.70, 4.80, 4.90, 5.00, 6.0, 7.0, 8.0, 9.0, 10.0,
        20.0, 30.0, 40.0, 50.0, 60.0,
    ];
    const O: [f64; 75] = [
        2.662, 2.476, 2.318, 2.184, 2.066, 1.966, 1.877, 1.798, 1.729, 1.667, 1.612, 1.562, 1.517, 1.476, 1.439, 1.406, 1.375, 1.346, 1.320, 1.296,
        1.273, 1.253, 1.233, 1.215, 1.198, 1.182, 1.167, 1.153, 1.140, 1.128, 1.116, 1.105, 1.094, 1.084, 1.075, 1.057, 1.041, 1.026, 1.012, 0.9996,
        0.9878, 0.9770, 0.9672, 0.9576, 0.9490, 0.9406, 0.9328, 0.9256, 0.9186, 0.9120, 0.9058, 0.8998, 0.8942, 0.8888, 0.8836, 0.8788, 0.8740,
        0.8694, 0.8652, 0.8610, 0.8568, 0.8530, 0.8492, 0.8456, 0.8422, 0.8124, 0.7896, 0.7712, 0.7556, 0.7424, 0.6640, 0.6232, 0.5960, 0.5756,
        0.5596,
    ];
    const T_HI: [f64; 5] = [60.0, 70.0, 80.0, 90.0, 100.0];
    const O_HI: [f64; 5] = [0.5596, 0.5464, 0.5352, 0.5256, 0.5170];
    if t_star < 0.3 {
        return 2.662 * (t_star / 0.3).r_powf(-0.5);
    }
    if t_star >= 100.0 {
        return 0.5170 * (t_star / 100.0).r_powf(-0.155);
    }
    if t_star >= 60.0 {
        let i = T_HI.partition_point(|&x| x <= t_star).min(4);
        let (t0, t1, o0, o1) = (T_HI[i - 1], T_HI[i], O_HI[i - 1], O_HI[i]);
        return o0 + (t_star - t0) / (t1 - t0) * (o1 - o0);
    }
    let i = T.partition_point(|&x| x <= t_star).min(T.len() - 1);
    let (t0, t1, o0, o1) = (T[i - 1], T[i], O[i - 1], O[i]);
    o0 + (t_star - t0) / (t1 - t0) * (o1 - o0)
}

/// Binary diffusivity of molecule A in B, m^2/s (Eq 2-105, cm^2/s with P
/// in atm, converted), from their Lennard-Jones parameters (sigma A,
/// epsilon/k K, molar mass g/mol).
pub fn binary_diffusivity(a: (f64, f64, f64), b: (f64, f64, f64), t_k: f64, p_pa: f64) -> f64 {
    let sigma = 0.5 * (a.0 + b.0);
    let omega = collision_integral(t_k / (a.1 * b.1).sqrt());
    let p_atm = p_pa / 101_325.0;
    0.0018583 * (t_k.r_powi(3) * (1.0 / a.2 + 1.0 / b.2)).sqrt() / (p_atm * sigma * sigma * omega) * 1.0e-4
}

impl FpClass {
    /// Lennard-Jones sigma, Angstrom, epsilon/k, K (RN 7111: Xe and I2
    /// their own; Cs and Te the default 3.617 and 97), and molar mass,
    /// g/mol.
    fn lj(self) -> (f64, f64, f64) {
        let m = self.molar_mass() * 1000.0;
        match self {
            FpClass::Xe => (4.055, 229.0, m),
            FpClass::I2 => (4.982, 550.0, m),
            FpClass::Cs | FpClass::Te => (3.617, 97.0, m),
        }
    }

    /// Diffusivity of this vapour (a trace) in a gas of the mole fractions
    /// `y` of `gases`, m^2/s (Eq 2-104 with `y_k` to 0).
    pub fn diffusivity(self, gases: &[(BulkGas, f64)], t_k: f64, p_pa: f64) -> f64 {
        let y_sum: f64 = gases.iter().map(|g| g.1).sum();
        1.0 / gases.iter().map(|&(g, y)| y / y_sum / binary_diffusivity(self.lj(), g.lj(), t_k, p_pa)).sum::<f64>()
    }

    /// Mass-transfer coefficient of the vapour onto an aerosol particle of
    /// diameter `d`, m/s: Sherwood number 2 (RN §2.5.2).
    pub fn aerosol_mass_transfer(self, gases: &[(BulkGas, f64)], t_k: f64, p_pa: f64, d: f64) -> f64 {
        2.0 * self.diffusivity(gases, t_k, p_pa) / d
    }

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

    /// Diffusivity, two independent checks: Chapman-Enskog for steam in H2
    /// at 1000 K and 1 bar against MELCOR's own correlation for that pair,
    /// `6.60639e-4 T^1.68 / P` (NCG/H2O Eq 6-2) = 7.24e-4 m^2/s, within
    /// 10 % (hand: 6.89e-4); the collision integral at a table point and
    /// continuous across its range ends; a trace vapour in one gas has that
    /// pair's diffusivity; and Sh 2 gives `2 D / d`.
    #[test]
    fn vapour_diffusivity_is_chapman_enskogs() {
        let d = binary_diffusivity(BulkGas::Steam.lj(), BulkGas::H2.lj(), 1000.0, 1.0e5);
        let melcor = 6.60639e-4 * 1000f64.r_powf(1.68) / 1.0e5;
        assert!((d / melcor - 1.0).abs() < 0.10, "{d} vs {melcor}");
        assert!((d - 6.89e-4).abs() < 0.05e-4, "{d}");
        assert_eq!(collision_integral(1.0), 1.439);
        for t in [0.3, 60.0, 100.0] {
            assert!((collision_integral(t * (1.0 - 1e-9)) - collision_integral(t * (1.0 + 1e-9))).abs() < 1e-6, "{t}");
        }
        let one = FpClass::I2.diffusivity(&[(BulkGas::N2, 1.0)], 500.0, 3.0e5);
        assert!((one - binary_diffusivity(FpClass::I2.lj(), BulkGas::N2.lj(), 500.0, 3.0e5)).abs() < 1e-18);
        assert!((FpClass::I2.aerosol_mass_transfer(&[(BulkGas::N2, 1.0)], 500.0, 3.0e5, 1e-6) - 2.0 * one / 1e-6).abs() < 1e-9);
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
