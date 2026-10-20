// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Ong Kay Chen Theodore, National University of Singapore
//
// This file is part of Outram Park (outram-park-backend).
//
// COPIED VERBATIM (`GasState` .. `exact_state`, lines 117-266) from
// `crates/outram-foam-appbuilder-lib/tests/sod_shock_tube_validation/main.rs`
// at commit 79b63cdbe (2026-10-16), so `tampines-steam-tables` tests use the
// same exact Riemann arbiter without depending on appbuilder (maintainer
// decision D2: no new dependency edges). Only `pub` and this header changed.
// If one copy is corrected, correct the other.
//
// Reference: Toro, E. F. (2009), *Riemann Solvers and Numerical Methods for
// Fluid Dynamics*, 3rd ed., Springer, ch. 4 (eqs. 4.6-4.9 and the `sample`
// routine). Perfect gas, GAMMA = 1.4.

//! Exact Riemann solver for the Euler equations of a perfect gas (Toro ch. 4),
//! the arbiter of the ideal-gas HEM-pipe tests.

#![allow(dead_code)]

/// Ratio of specific heats, as in the appbuilder copy.
use petir::mathf::RealMath;
pub const GAMMA: f64 = 1.4;

/// A primitive gas state: density, velocity, pressure. Unit-agnostic — feed a
/// consistent set (all SI, or all Sod-dimensionless).
#[derive(Clone, Copy, Debug)]
pub struct GasState {
    pub rho: f64,
    pub u: f64,
    pub p: f64,
}

pub fn sound_speed(s: GasState) -> f64 {
    (GAMMA * s.p / s.rho).sqrt()
}

/// Pressure function f_K(p*) and derivative for one side (Toro eqs. 4.6–4.7).
pub fn pressure_fn(p_star: f64, side: GasState) -> (f64, f64) {
    let c_k = sound_speed(side);
    if p_star > side.p {
        let a_k = 2.0 / ((GAMMA + 1.0) * side.rho);
        let b_k = (GAMMA - 1.0) / (GAMMA + 1.0) * side.p;
        let sqrt_term = (a_k / (b_k + p_star)).sqrt();
        let f = (p_star - side.p) * sqrt_term;
        let df = sqrt_term * (1.0 - (p_star - side.p) / (2.0 * (b_k + p_star)));
        (f, df)
    } else {
        let f = 2.0 * c_k / (GAMMA - 1.0)
            * ((p_star / side.p).r_powf((GAMMA - 1.0) / (2.0 * GAMMA)) - 1.0);
        let df = 1.0 / (side.rho * c_k) * (p_star / side.p).r_powf(-(GAMMA + 1.0) / (2.0 * GAMMA));
        (f, df)
    }
}

/// Star-region pressure P\* by Newton–Raphson (Toro §4.3.2).
pub fn star_pressure(l: GasState, r: GasState) -> f64 {
    let c_l = sound_speed(l);
    let c_r = sound_speed(r);
    let ppv = 0.5 * (l.p + r.p) - 0.125 * (r.u - l.u) * (l.rho + r.rho) * (c_l + c_r);
    let mut p = ppv.max(1e-6 * 0.5 * (l.p + r.p));
    for _ in 0..200 {
        let (fl, dfl) = pressure_fn(p, l);
        let (fr, dfr) = pressure_fn(p, r);
        let f = fl + fr + (r.u - l.u);
        let df = dfl + dfr;
        let p_new = (p - f / df).max(1e-12);
        if (p_new - p).abs() / (0.5 * (p_new + p)) < 1e-13 {
            return p_new;
        }
        p = p_new;
    }
    p
}

/// Star-region velocity u\* (Toro eq. 4.9).
pub fn star_velocity(l: GasState, r: GasState, p_star: f64) -> f64 {
    let (fl, _) = pressure_fn(p_star, l);
    let (fr, _) = pressure_fn(p_star, r);
    0.5 * (l.u + r.u) + 0.5 * (fr - fl)
}

/// Right-going shock speed (Sod's right wave is a shock).
pub fn right_shock_speed(r: GasState, p_star: f64) -> f64 {
    let c_r = sound_speed(r);
    r.u + c_r
        * ((GAMMA + 1.0) / (2.0 * GAMMA) * (p_star / r.p) + (GAMMA - 1.0) / (2.0 * GAMMA)).sqrt()
}

/// Sample the exact solution at ξ = (x − x₀)/t (Toro `sample`, Fig. 4.14).
pub fn sample(l: GasState, r: GasState, p_star: f64, u_star: f64, xi: f64) -> GasState {
    let c_l = sound_speed(l);
    let c_r = sound_speed(r);
    let g1 = (GAMMA - 1.0) / (GAMMA + 1.0);
    if xi <= u_star {
        if p_star > l.p {
            let s_l = l.u
                - c_l
                    * ((GAMMA + 1.0) / (2.0 * GAMMA) * (p_star / l.p)
                        + (GAMMA - 1.0) / (2.0 * GAMMA))
                        .sqrt();
            if xi <= s_l {
                l
            } else {
                let rho = l.rho * (p_star / l.p + g1) / (g1 * (p_star / l.p) + 1.0);
                GasState {
                    rho,
                    u: u_star,
                    p: p_star,
                }
            }
        } else {
            let c_star_l = c_l * (p_star / l.p).r_powf((GAMMA - 1.0) / (2.0 * GAMMA));
            let s_head = l.u - c_l;
            let s_tail = u_star - c_star_l;
            if xi <= s_head {
                l
            } else if xi >= s_tail {
                let rho = l.rho * (p_star / l.p).r_powf(1.0 / GAMMA);
                GasState {
                    rho,
                    u: u_star,
                    p: p_star,
                }
            } else {
                let u = 2.0 / (GAMMA + 1.0) * (c_l + (GAMMA - 1.0) / 2.0 * l.u + xi);
                let c = 2.0 / (GAMMA + 1.0) * (c_l + (GAMMA - 1.0) / 2.0 * (l.u - xi));
                let rho = l.rho * (c / c_l).r_powf(2.0 / (GAMMA - 1.0));
                let p = l.p * (c / c_l).r_powf(2.0 * GAMMA / (GAMMA - 1.0));
                GasState { rho, u, p }
            }
        }
    } else if p_star > r.p {
        let s_r = right_shock_speed(r, p_star);
        if xi >= s_r {
            r
        } else {
            let rho = r.rho * (p_star / r.p + g1) / (g1 * (p_star / r.p) + 1.0);
            GasState {
                rho,
                u: u_star,
                p: p_star,
            }
        }
    } else {
        let c_star_r = c_r * (p_star / r.p).r_powf((GAMMA - 1.0) / (2.0 * GAMMA));
        let s_head = r.u + c_r;
        let s_tail = u_star + c_star_r;
        if xi >= s_head {
            r
        } else if xi <= s_tail {
            let rho = r.rho * (p_star / r.p).r_powf(1.0 / GAMMA);
            GasState {
                rho,
                u: u_star,
                p: p_star,
            }
        } else {
            let u = 2.0 / (GAMMA + 1.0) * (-c_r + (GAMMA - 1.0) / 2.0 * r.u + xi);
            let c = 2.0 / (GAMMA + 1.0) * (c_r - (GAMMA - 1.0) / 2.0 * (r.u - xi));
            let rho = r.rho * (c / c_r).r_powf(2.0 / (GAMMA - 1.0));
            let p = r.p * (c / c_r).r_powf(2.0 * GAMMA / (GAMMA - 1.0));
            GasState { rho, u, p }
        }
    }
}

/// Exact primitive state at (x, t), diaphragm at x₀.
pub fn exact_state(l: GasState, r: GasState, x: f64, x0: f64, t: f64) -> GasState {
    let p_star = star_pressure(l, r);
    let u_star = star_velocity(l, r, p_star);
    sample(l, r, p_star, u_star, (x - x0) / t)
}
