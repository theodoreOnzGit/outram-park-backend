// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! MELCOR's NCG-library `c_v(T)` fits for N2 and H2 (gh:#1093).
//!
//! **Moved here verbatim from `sembawang::bwr_mark1::gas`** (2026-11-01,
//! maintainer direction: `sembawang` depends on `tampines`, so the thermal
//! hydraulics, gas properties included, lives on this side). The operation
//! order is unchanged, so every value is bit-identical to what `sembawang`
//! computed before; `sembawang::bwr_mark1::gas` now forwards here.
//!
//! The fits are MELCOR 2.2's (Vol. 2, SAND2023-10994 O, NCG/H2O-RM-33/34;
//! the form is Vol. 1, SAND2017-0455 O, NCG-UG-2, Eq 1.1; cited, not
//! quoted; no MELCOR source used):
//!
//! ```text
//! c_v(T) = cv0 + cv1 T + cv2 T^2 + cv3 T^3 + cvsqrt/sqrt(T) + cvm1/T + cvm2/T^2
//! ```
//!
//! mass-specific, inside `T_low..=T_up`, held at the boundary value outside.
//! Internal energy is from 0 K: `c_v(T_low) T` below `T_low`, the integral
//! of the fit inside, linear in `c_v(T_up)` above. `t_dependent = false` is
//! the constant-value ablation (20.8 and 20.5 J/(mol K)).

use petir::mathf::RealMath;

/// Molar mass of H2O, kg/mol (`sembawang::bwr_mark1::params::M_H2O`).
pub const M_H2O: f64 = 0.018_015;
/// Molar mass of H2, kg/mol.
pub const M_H2: f64 = 0.002_016;
/// Molar mass of N2, kg/mol.
pub const M_N2: f64 = 0.028_014;
/// Gas constant, J/(mol K).
pub const R_GAS: f64 = 8.314_462;

/// A MELCOR NCG-library fit.
struct Fit {
    m: f64,
    c: [f64; 7],
    t_lo: f64,
    t_up: f64,
}

/// H2 (NCG/H2O-RM-33), 100-6000 K. The library's molar mass is 0.0020162;
/// `M_H2` is used, a 0.01 % difference (as in sembawang).
const H2: Fit = Fit {
    m: M_H2,
    c: [-17849.0, 11.28298, -2.1081958e-3, 1.5635602e-7, 865616.0, -8188058.3, 1.925734e8],
    t_lo: 100.0,
    t_up: 6000.0,
};

/// N2 (NCG/H2O-RM-34), 300-5000 K. Library molar mass 0.02801.
const N2: Fit = Fit { m: M_N2, c: [1117.0, 0.0, 0.0, 0.0, 0.0, -2.880e5, 5.348e7], t_lo: 300.0, t_up: 5000.0 };

/// The constant values, J/(mol K): the ablation.
pub const CV_N2_CONST: f64 = 20.8;
/// The constant value for H2, J/(mol K).
pub const CV_H2_CONST: f64 = 20.5;

impl Fit {
    fn cv_in(&self, t: f64) -> f64 {
        let c = &self.c;
        c[0] + c[1] * t + c[2] * t * t + c[3] * t * t * t + c[4] / t.sqrt() + c[5] / t + c[6] / (t * t)
    }
    fn integral_in(&self, t: f64) -> f64 {
        let c = &self.c;
        c[0] * t + c[1] * t * t / 2.0 + c[2] * t.r_powi(3) / 3.0 + c[3] * t.r_powi(4) / 4.0 + 2.0 * c[4] * t.sqrt() + c[5] * t.r_ln()
            - c[6] / t
    }
    fn cv_molar(&self, t: f64) -> f64 {
        self.cv_in(t.clamp(self.t_lo, self.t_up)) * self.m
    }
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

/// Molar `c_v` of N2, J/(mol K).
pub fn cv_n2(t: f64, t_dependent: bool) -> f64 {
    if t_dependent { N2.cv_molar(t) } else { CV_N2_CONST }
}
/// Molar `c_v` of H2, J/(mol K).
pub fn cv_h2(t: f64, t_dependent: bool) -> f64 {
    if t_dependent { H2.cv_molar(t) } else { CV_H2_CONST }
}
/// Molar internal energy of N2 from 0 K, J/mol.
pub fn u_n2(t: f64, t_dependent: bool) -> f64 {
    if t_dependent { N2.u_molar(t) } else { CV_N2_CONST * t }
}
/// Molar internal energy of H2 from 0 K, J/mol.
pub fn u_h2(t: f64, t_dependent: bool) -> f64 {
    if t_dependent { H2.u_molar(t) } else { CV_H2_CONST * t }
}
/// Internal energy of `n2` and `h2` moles at `t`, J (the ablation keeps the
/// operation order sembawang used before 2026-10-29).
pub fn ncg_u(n2: f64, h2: f64, t: f64, t_dependent: bool) -> f64 {
    if t_dependent { n2 * u_n2(t, true) + h2 * u_h2(t, true) } else { (n2 * CV_N2_CONST + h2 * CV_H2_CONST) * t }
}
/// Heat capacity of `n2` and `h2` moles at `t`, J/K.
pub fn ncg_c(n2: f64, h2: f64, t: f64, t_dependent: bool) -> f64 {
    n2 * cv_n2(t, t_dependent) + h2 * cv_h2(t, t_dependent)
}
/// Hydrogen's molar enthalpy from 0 K, `u + R T`, J/mol.
pub fn h2_molar_enthalpy(t: f64, t_dependent: bool) -> f64 {
    if t_dependent { u_h2(t, true) + R_GAS * t } else { (CV_H2_CONST + R_GAS) * t }
}
/// Hydrogen's `c_p`, J/(kg K).
pub fn cp_h2_per_kg(t: f64, t_dependent: bool) -> f64 {
    (cv_h2(t, t_dependent) + R_GAS) / M_H2
}
/// Hydrogen's enthalpy above 298.15 K, J/kg.
pub fn h2_enthalpy_above_298(t: f64) -> f64 {
    (h2_molar_enthalpy(t, true) - h2_molar_enthalpy(298.15, true)) / M_H2
}

#[cfg(test)]
mod tests {
    use super::*;

    /// sembawang's own test, moved with the code: hand values (molar `c_v`
    /// at 300 K: H2 20.51, N2 21.04 J/(mol K)), `u` integrates `c_v`,
    /// continuity at the range ends, the ablation is the old constants.
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
        assert!((cp_h2_per_kg(2000.0, true) * M_H2 - 34.2).abs() < 0.1, "{}", cp_h2_per_kg(2000.0, true) * M_H2);
        assert_eq!(cp_h2_per_kg(2000.0, false), (20.5 + R_GAS) / M_H2);
        let slope = h2_enthalpy_above_298(1500.5) - h2_enthalpy_above_298(1499.5);
        assert!((slope / cp_h2_per_kg(1500.0, true) - 1.0).abs() < 1e-4);
        assert_eq!(h2_enthalpy_above_298(298.15), 0.0);
    }
}
