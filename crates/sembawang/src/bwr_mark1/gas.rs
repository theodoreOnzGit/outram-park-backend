//! Heat capacities of the non-condensable gases, N2 and H2.
//!
//! Each has its own `c_v(T)`, from MELCOR 2.2's NCG library fits (Vol. 2,
//! SAND2023-10994 O, NCG/H2O-RM-33/34; the form is Vol. 1, SAND2017-0455 O,
//! NCG-UG-2, Eq 1.1; no source used):
//!
//! ```text
//! c_v(T) = cv0 + cv1 T + cv2 T^2 + cv3 T^3 + cvsqrt/sqrt(T) + cvm1/T + cvm2/T^2
//! ```
//!
//! The coefficients are mass-specific and apply inside `T_low..=T_up`. Outside
//! that range `c_v` is held at its boundary value. Internal energy is
//! measured from 0 K, the containment's existing convention, so it is
//! `c_v(T_low) T` below `T_low`, the integral of the fit inside the range,
//! and linear in `c_v(T_up)` above `T_up`. Before 2026-10-29 these were
//! constants, 20.8 (N2) and 20.5 (H2) J/(mol K), and the
//! `without_gas_cv_t` ablation restores them.

use super::params::{M_H2, M_N2, R_GAS};
use petir::mathf::RealMath;

/// A MELCOR NCG-library fit: mass-specific coefficients, J/(kg K) and the
/// powers above, and its range, K. \[P\]
struct Fit {
    m: f64,
    c: [f64; 7],
    t_lo: f64,
    t_up: f64,
}

/// H2 (MELCOR NCG library, NCG/H2O-RM-33): cv0, cv1, cv2, cv3, cvsqrt, cvm1,
/// cvm2; 100-6000 K. The library's molar mass is 0.0020162; the model's
/// `M_H2` is used, a 0.01 % difference.
const H2: Fit = Fit {
    m: M_H2,
    c: [-17849.0, 11.28298, -2.1081958e-3, 1.5635602e-7, 865616.0, -8188058.3, 1.925734e8],
    t_lo: 100.0,
    t_up: 6000.0,
};

/// N2 (NCG/H2O-RM-34): 300-5000 K. Library molar mass 0.02801.
const N2: Fit = Fit { m: M_N2, c: [1117.0, 0.0, 0.0, 0.0, 0.0, -2.880e5, 5.348e7], t_lo: 300.0, t_up: 5000.0 };

/// The constants used before 2026-10-29, J/(mol K): the ablation.
const CV_N2_CONST: f64 = 20.8;
const CV_H2_CONST: f64 = 20.5;

impl Fit {
    /// Mass-specific `c_v` at `t` inside the range.
    fn cv_in(&self, t: f64) -> f64 {
        let c = &self.c;
        c[0] + c[1] * t + c[2] * t * t + c[3] * t * t * t + c[4] / t.sqrt() + c[5] / t + c[6] / (t * t)
    }
    /// Antiderivative of `cv_in`.
    fn integral_in(&self, t: f64) -> f64 {
        let c = &self.c;
        c[0] * t + c[1] * t * t / 2.0 + c[2] * t.r_powi(3) / 3.0 + c[3] * t.r_powi(4) / 4.0 + 2.0 * c[4] * t.sqrt() + c[5] * t.r_ln()
            - c[6] / t
    }
    /// Molar `c_v`, J/(mol K), held at the range's ends.
    fn cv_molar(&self, t: f64) -> f64 {
        self.cv_in(t.clamp(self.t_lo, self.t_up)) * self.m
    }
    /// Molar internal energy from 0 K, J/mol.
    fn u_molar(&self, t: f64) -> f64 {
        let lo = self.cv_in(self.t_lo) * self.t_lo;
        let u = if t <= self.t_lo {
            self.cv_in(self.t_lo) * t
        } else if t <= self.t_up {
            lo + self.integral_in(t) - self.integral_in(self.t_lo)
        } else {
            lo + self.integral_in(self.t_up) - self.integral_in(self.t_lo) + self.cv_in(self.t_up) * (t - self.t_up)
        };
        u * self.m
    }
}

/// Molar `c_v` of N2 and of H2, J/(mol K); `t_dependent` false is the
/// constant-value ablation.
pub fn cv_n2(t: f64, t_dependent: bool) -> f64 {
    if t_dependent { N2.cv_molar(t) } else { CV_N2_CONST }
}
pub fn cv_h2(t: f64, t_dependent: bool) -> f64 {
    if t_dependent { H2.cv_molar(t) } else { CV_H2_CONST }
}

/// Molar internal energy of N2 and of H2 from 0 K, J/mol.
pub fn u_n2(t: f64, t_dependent: bool) -> f64 {
    if t_dependent { N2.u_molar(t) } else { CV_N2_CONST * t }
}
pub fn u_h2(t: f64, t_dependent: bool) -> f64 {
    if t_dependent { H2.u_molar(t) } else { CV_H2_CONST * t }
}

/// Internal energy of `n2` and `h2` moles at `t`, J. The ablation keeps the
/// operation order of the code before 2026-10-29, so it reproduces it bit
/// for bit.
pub fn ncg_u(n2: f64, h2: f64, t: f64, t_dependent: bool) -> f64 {
    if t_dependent { n2 * u_n2(t, true) + h2 * u_h2(t, true) } else { (n2 * CV_N2_CONST + h2 * CV_H2_CONST) * t }
}

/// Heat capacity of `n2` and `h2` moles at `t`, J/K.
pub fn ncg_c(n2: f64, h2: f64, t: f64, t_dependent: bool) -> f64 {
    n2 * cv_n2(t, t_dependent) + h2 * cv_h2(t, t_dependent)
}

/// Hydrogen's molar enthalpy from 0 K, `u + R T`, J/mol (the ablation as
/// before: `(c_v + R) T`).
pub fn h2_molar_enthalpy(t: f64, t_dependent: bool) -> f64 {
    if t_dependent { u_h2(t, true) + R_GAS * t } else { (CV_H2_CONST + R_GAS) * t }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fits against hand values from the manual's coefficients (molar
    /// `c_v` at 300 K: H2 20.51, N2 21.04 J/(mol K)), the energy is the
    /// integral of `c_v` (a centred difference recovers `c_v`), it is
    /// continuous at the ranges' ends, and the ablation is the old
    /// constants.
    #[test]
    fn gas_heat_capacities_follow_melcors_fits() {
        assert!((cv_h2(300.0, true) - 20.51).abs() < 0.01, "{}", cv_h2(300.0, true));
        assert!((cv_n2(300.0, true) - 21.04).abs() < 0.01, "{}", cv_n2(300.0, true));
        for t in [400.0, 800.0, 1500.0, 2500.0] {
            for (u, cv) in [(u_n2 as fn(f64, bool) -> f64, cv_n2 as fn(f64, bool) -> f64), (u_h2, cv_h2)] {
                let d = (u(t + 0.5, true) - u(t - 0.5, true)) / 1.0;
                assert!((d - cv(t, true)).abs() < 1e-3 * cv(t, true), "{t}: {d} vs {}", cv(t, true));
            }
        }
        for t in [300.0, 5000.0] {
            assert!((u_n2(t + 1e-6, true) - u_n2(t - 1e-6, true)).abs() < 1e-3);
        }
        assert_eq!((cv_n2(700.0, false), u_h2(500.0, false)), (20.8, 20.5 * 500.0));
        assert_eq!(ncg_u(2.0, 3.0, 400.0, false), (2.0 * 20.8 + 3.0 * 20.5) * 400.0);
        assert_eq!(ncg_c(2.0, 3.0, 400.0, true), 2.0 * cv_n2(400.0, true) + 3.0 * cv_h2(400.0, true));
        assert!((h2_molar_enthalpy(600.0, true) - u_h2(600.0, true) - R_GAS * 600.0).abs() < 1e-9);
        assert_eq!(h2_molar_enthalpy(600.0, false), (20.5 + R_GAS) * 600.0);
    }
}
