// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! NASA 7-coefficient ideal-gas heat capacities for the non-condensables
//! (gh:#1093).
//!
//! **Data:** GRI-Mech 3.0 (G. P. Smith, D. M. Golden, M. Frenklach et al.,
//! <http://combustion.berkeley.edu/gri-mech/>, open), as distributed in
//! Cantera v3.0.0's `data/gri30.yaml` (BSD-3-Clause), downloaded 2026-11-01
//! from `raw.githubusercontent.com/Cantera/cantera/v3.0.0/data/gri30.yaml`.
//! Only the five `c_p` coefficients of each range are used:
//!
//! ```text
//! c_p/R = a0 + a1 T + a2 T^2 + a3 T^3 + a4 T^4,   c_v = c_p − R
//! ```
//!
//! The internal energy is measured from 0 K with `sembawang`'s convention
//! (as [`super::melcor_ncg`]): `c_v(T_low) T` below the fit's lower bound,
//! the integral above it (continuous at `T_mid`, since the two GRI ranges
//! match there in value), and linear in `c_v(T_high)` above the upper bound.

use super::melcor_ncg::R_GAS;
use super::NonCondensable;

/// One gas: `(T_low, T_mid, T_high)` and the low and high `c_p/R`
/// coefficients.
struct Nasa7 {
    t: [f64; 3],
    low: [f64; 5],
    high: [f64; 5],
}

const N2: Nasa7 = Nasa7 {
    t: [300.0, 1000.0, 5000.0],
    low: [3.298677, 1.4082404e-03, -3.963222e-06, 5.641515e-09, -2.444854e-12],
    high: [2.92664, 1.4879768e-03, -5.68476e-07, 1.0097038e-10, -6.753351e-15],
};
const H2: Nasa7 = Nasa7 {
    t: [200.0, 1000.0, 3500.0],
    low: [2.34433112, 7.98052075e-03, -1.9478151e-05, 2.01572094e-08, -7.37611761e-12],
    high: [3.3372792, -4.94024731e-05, 4.99456778e-07, -1.79566394e-10, 2.00255376e-14],
};
const O2: Nasa7 = Nasa7 {
    t: [200.0, 1000.0, 3500.0],
    low: [3.78245636, -2.99673416e-03, 9.84730201e-06, -9.68129509e-09, 3.24372837e-12],
    high: [3.28253784, 1.48308754e-03, -7.57966669e-07, 2.09470555e-10, -2.16717794e-14],
};
const CO: Nasa7 = Nasa7 {
    t: [200.0, 1000.0, 3500.0],
    low: [3.57953347, -6.1035368e-04, 1.01681433e-06, 9.07005884e-10, -9.04424499e-13],
    high: [2.71518561, 2.06252743e-03, -9.98825771e-07, 2.30053008e-10, -2.03647716e-14],
};
const CO2: Nasa7 = Nasa7 {
    t: [200.0, 1000.0, 3500.0],
    low: [2.35677352, 8.98459677e-03, -7.12356269e-06, 2.45919022e-09, -1.43699548e-13],
    high: [3.85746029, 4.41437026e-03, -2.21481404e-06, 5.23490188e-10, -4.72084164e-14],
};
const AR: Nasa7 = Nasa7 { t: [300.0, 1000.0, 5000.0], low: [2.5, 0.0, 0.0, 0.0, 0.0], high: [2.5, 0.0, 0.0, 0.0, 0.0] };

fn data(g: NonCondensable) -> &'static Nasa7 {
    match g {
        NonCondensable::N2 => &N2,
        NonCondensable::H2 => &H2,
        NonCondensable::O2 => &O2,
        NonCondensable::CO => &CO,
        NonCondensable::CO2 => &CO2,
        NonCondensable::Ar => &AR,
    }
}

fn cp_r(a: &[f64; 5], t: f64) -> f64 {
    a[0] + t * (a[1] + t * (a[2] + t * (a[3] + t * a[4])))
}

/// `∫ c_v/R dT` antiderivative (`c_v/R = c_p/R − 1`).
fn int_cv_r(a: &[f64; 5], t: f64) -> f64 {
    (a[0] - 1.0) * t + a[1] * t * t / 2.0 + a[2] * t * t * t / 3.0 + a[3] * t * t * t * t / 4.0 + a[4] * t * t * t * t * t / 5.0
}

/// Molar `c_v` of `g` at `t` K, J/(mol K), held at the fit's ends.
pub fn cv_molar(g: NonCondensable, t: f64) -> f64 {
    let d = data(g);
    let tc = t.clamp(d.t[0], d.t[2]);
    let a = if tc < d.t[1] { &d.low } else { &d.high };
    R_GAS * (cp_r(a, tc) - 1.0)
}

/// Molar internal energy of `g` from 0 K at `t` K, J/mol.
pub fn u_molar(g: NonCondensable, t: f64) -> f64 {
    let d = data(g);
    let (t_lo, t_mid, t_hi) = (d.t[0], d.t[1], d.t[2]);
    let cv_lo = cv_molar(g, t_lo);
    if t <= t_lo {
        return cv_lo * t;
    }
    let base = cv_lo * t_lo;
    let seg = |a: &[f64; 5], x0: f64, x1: f64| R_GAS * (int_cv_r(a, x1) - int_cv_r(a, x0));
    let tt = t.min(t_hi);
    let mut u = base;
    if tt <= t_mid {
        u += seg(&d.low, t_lo, tt);
    } else {
        u += seg(&d.low, t_lo, t_mid) + seg(&d.high, t_mid, tt);
    }
    if t > t_hi {
        u += cv_molar(g, t_hi) * (t - t_hi);
    }
    u
}
