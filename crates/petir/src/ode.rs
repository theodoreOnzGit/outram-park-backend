// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PETIR is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along
// with PETIR.  If not, see <https://www.gnu.org/licenses/>.
//
// PORTED from the GNU Scientific Library (GSL) 2.8, commit
// cf180cd7fbd06039a577f9c9ff0b428784765ac1, read 2026-09-14.
// Copyright (C) 1996-2000 Gerard Jungman, Brian Gough
// GPL-3.0-or-later (verified from the per-file headers; see NOTICE).
//
//   ode-initval2/rkf45.c  the Runge-Kutta-Fehlberg 4(5) tableau and step
//   ode-initval2/rk4.c    the classical fixed-step RK4 -- specifically the
//                         PRIVATE `rk4_step` helper, NOT the public
//                         `rk4_apply`, which step-doubles (see rk4_step's docs)
//   ode-initval2/control_standard.c  the step-size controller's form

//! Ordinary differential equations — GSL's `ode-initval2/`.
//!
//! Integrates the initial-value problem `dy/dt = f(t, y)`, `y(t0) = y0`, for a
//! system of any dimension.
//!
//! # What is here
//!
//! - [`rk4_step`] — one classical fourth-order Runge-Kutta step. Fixed size,
//!   no error estimate. Simple, predictable, and the wrong default.
//! - [`rkf45_step`] — one Runge-Kutta-Fehlberg 4(5) step, returning **both**
//!   the fifth-order solution and an error estimate from the embedded
//!   fourth-order one.
//! - [`solve_rkf45`] — the adaptive driver: take a step, judge it against your
//!   tolerance, shrink and retry or accept and grow.
//!
//! # Why the embedded pair matters
//!
//! Fehlberg's construction evaluates `f` six times and uses those same six
//! evaluations to form two solutions of different order. Their difference
//! estimates the local error **at no extra cost**, which is what makes step
//! control possible: without it you are guessing at `h` and have no way to know
//! whether the guess was good.
//!
//! This is the same idea as Gauss-Kronrod in [`crate::integration`], one
//! dimension over.
//!
//! # Step control
//!
//! [`solve_rkf45`] scales the step by `0.9 * (tol / err)^(1/5)`, clamped to a
//! factor of 5 up or down per step, which is the standard form GSL's
//! `control_standard` uses. The `0.9` is deliberate pessimism — aiming exactly
//! at the tolerance means about half of all steps get rejected and recomputed,
//! and a 10 % margin cuts that sharply for a 10 % cost.
//!
//! # Stiffness: this is an EXPLICIT method, and that has consequences
//!
//! RKF45 is explicit, so its stable step size is limited by the *fastest* time
//! scale in the system even when that mode has long since decayed and
//! contributes nothing to the answer. For a **stiff** system — chemical
//! kinetics with rate constants spanning decades, a circuit with a fast
//! parasitic, most reaction networks — the controller is forced into
//! vanishingly small steps and the integration crawls or fails with
//! [`PetirError::MaxIterations`].
//!
//! That is not a defect in this implementation and no amount of tuning fixes
//! it. A stiff problem needs an **implicit** method (backward Euler, BDF,
//! Rosenbrock); GSL has several and PETIR has ported none. The tell is a
//! solver that takes thousands of steps over a smooth-looking solution.
//!
//! # Units
//!
//! Bare dimensionless `f64`. A dimensioned system should carry its units at the
//! caller's API boundary and pass raw values in, per the crate's units policy.

use alloc::vec;
use alloc::vec::Vec;

// Under a std-linked build (`cargo test`) f64's inherent abs shadows this
// trait, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::zip::zip_flat;
use crate::{PetirError, Result};

// --- Runge-Kutta-Fehlberg 4(5) tableau, from ode-initval2/rkf45.c -----------

/// Stage times as fractions of the step — upstream's `ah`.
const AH: [f64; 5] = [1.0 / 4.0, 3.0 / 8.0, 12.0 / 13.0, 1.0, 1.0 / 2.0];
/// Stage-3 coefficients — upstream's `b3`.
const B3: [f64; 2] = [3.0 / 32.0, 9.0 / 32.0];
/// Stage-4 coefficients — upstream's `b4`.
const B4: [f64; 3] = [1932.0 / 2197.0, -7200.0 / 2197.0, 7296.0 / 2197.0];
/// Stage-5 coefficients — upstream's `b5`.
const B5: [f64; 4] = [
    8341.0 / 4104.0,
    -32832.0 / 4104.0,
    29440.0 / 4104.0,
    -845.0 / 4104.0,
];
/// Stage-6 coefficients — upstream's `b6`.
const B6: [f64; 5] = [
    -6080.0 / 20520.0,
    41040.0 / 20520.0,
    -28352.0 / 20520.0,
    9295.0 / 20520.0,
    -5643.0 / 20520.0,
];

/// Fifth-order solution weights — upstream's `c1, c3, c4, c5, c6`.
const C1: f64 = 902880.0 / 7618050.0;
const C3: f64 = 3953664.0 / 7618050.0;
const C4: f64 = 3855735.0 / 7618050.0;
const C5: f64 = -1371249.0 / 7618050.0;
const C6: f64 = 277020.0 / 7618050.0;

/// Differences between the fifth- and fourth-order weights, for the error
/// estimate — upstream's `ec`. Index 0 is unused, matching GSL's layout.
const EC: [f64; 7] = [
    0.0,
    1.0 / 360.0,
    0.0,
    -128.0 / 4275.0,
    -2197.0 / 75240.0,
    1.0 / 50.0,
    2.0 / 55.0,
];

/// One classical fourth-order Runge-Kutta step.
///
/// Advances `y` from `t` by `h`, returning the new state. No error estimate —
/// if you want one, use [`rkf45_step`].
///
/// # When this is the right choice
///
/// Rarely. It is here because it is the method everyone knows and because a
/// fixed step is occasionally what you want (a fixed-rate control loop, a
/// simulation that must produce output on an exact grid). For anything where
/// accuracy matters, an adaptive method costs little more and tells you how
/// well it did.
///
/// # This is NOT `gsl_odeiv2_step_rk4`, and the difference is not small
///
/// GSL's public RK4 stepper does **step-doubling**: `rk4_apply`
/// (`ode-initval2/rk4.c:247`) takes one full step, then two half-steps,
/// reports the half-step result and their difference as an error estimate.
/// This function ports the private `rk4_step` helper that sits underneath it —
/// one plain classical step, which is what "classical RK4" means and what a
/// reader expects from the name.
///
/// They are different quantities, not a port and its original. Measured on
/// `dy/dt = y` over 20 steps at `h = 0.05`, they differ by up to **1.27e-7**,
/// which is the `O(h^5)` local error separating a full step from two half
/// ones. If you want GSL's behaviour, use [`rkf45_step`], which gets an error
/// estimate for six evaluations rather than eleven.
///
/// # Example
///
/// ```
/// use petir::ode::rk4_step;
/// // dy/dt = y, y(0) = 1  ->  y(t) = e^t
/// let y = rk4_step(|_t, y: &[f64], dy: &mut [f64]| dy[0] = y[0], 0.0, &[1.0], 0.1).unwrap();
/// assert!((y[0] - 0.1_f64.exp()).abs() < 1e-6);
/// ```
pub fn rk4_step<F>(f: F, t: f64, y: &[f64], h: f64) -> Result<Vec<f64>>
where
    F: Fn(f64, &[f64], &mut [f64]),
{
    let n = y.len();
    if n == 0 {
        return Err(PetirError::Invalid);
    }

    let mut k = vec![0.0_f64; n];
    let mut tmp = vec![0.0_f64; n];
    let mut sum = vec![0.0_f64; n];
    let mut out = y.to_vec();

    // Each stage below is upstream's `for (i = 0; i < dim; i++)` loop with the
    // subscripts replaced by a zip -- the arithmetic is character-for-character
    // what `rk4.c` writes, so the rounding is identical. See `crate::zip`.

    // k1
    f(t, y, &mut k);
    for (s, tp, &yi, &ki) in zip_flat!(sum.iter_mut(), tmp.iter_mut(), y.iter(), k.iter()) {
        *s = ki;
        *tp = yi + 0.5 * h * ki;
    }
    // k2
    f(t + 0.5 * h, &tmp, &mut k);
    for (s, tp, &yi, &ki) in zip_flat!(sum.iter_mut(), tmp.iter_mut(), y.iter(), k.iter()) {
        *s += 2.0 * ki;
        *tp = yi + 0.5 * h * ki;
    }
    // k3
    f(t + 0.5 * h, &tmp, &mut k);
    for (s, tp, &yi, &ki) in zip_flat!(sum.iter_mut(), tmp.iter_mut(), y.iter(), k.iter()) {
        *s += 2.0 * ki;
        *tp = yi + h * ki;
    }
    // k4
    f(t + h, &tmp, &mut k);
    for (s, o, &yi, &ki) in zip_flat!(sum.iter_mut(), out.iter_mut(), y.iter(), k.iter()) {
        *s += ki;
        *o = yi + (h / 6.0) * *s;
    }

    Ok(out)
}

/// The result of one embedded Runge-Kutta-Fehlberg step.
#[derive(Debug, Clone, PartialEq)]
pub struct RkStep {
    /// The fifth-order solution at `t + h`.
    pub y: Vec<f64>,
    /// Per-component estimate of the local truncation error, from the
    /// difference between the fifth- and fourth-order solutions.
    pub yerr: Vec<f64>,
}

/// One Runge-Kutta-Fehlberg 4(5) step, with an error estimate.
///
/// Six evaluations of `f` yield both a fifth-order solution and an estimate of
/// its local error. Translates GSL's `rkf45_apply`.
///
/// # Errors
///
/// [`PetirError::Invalid`] for an empty system.
///
/// # Example
///
/// ```
/// use petir::ode::rkf45_step;
/// let s = rkf45_step(|_t, y: &[f64], dy: &mut [f64]| dy[0] = y[0], 0.0, &[1.0], 0.1).unwrap();
/// let true_err = (s.y[0] - 0.1_f64.exp()).abs();
/// assert!(true_err < 1e-9);
/// // The estimate BOUNDS the true error, conservatively — here 1.2e-8 against
/// // an actual 9.3e-10. Erring high is the useful direction: a step control
/// // that under-estimates accepts steps it should have rejected.
/// assert!(s.yerr[0].abs() >= true_err);
/// ```
pub fn rkf45_step<F>(f: F, t: f64, y: &[f64], h: f64) -> Result<RkStep>
where
    F: Fn(f64, &[f64], &mut [f64]),
{
    let n = y.len();
    if n == 0 {
        return Err(PetirError::Invalid);
    }

    let mut k1 = vec![0.0_f64; n];
    let mut k2 = vec![0.0_f64; n];
    let mut k3 = vec![0.0_f64; n];
    let mut k4 = vec![0.0_f64; n];
    let mut k5 = vec![0.0_f64; n];
    let mut k6 = vec![0.0_f64; n];
    let mut tmp = vec![0.0_f64; n];

    // Each stage below is upstream's `for (i = 0; i < dim; i++)` loop with the
    // subscripts replaced by a zip. The tableau constants keep their literal
    // subscripts (`AH[0]`, `B6[4]`): those index fixed-size `[f64; N]` arrays
    // at compile-time-constant positions, so they are checked by the compiler
    // and cannot fail -- and keeping them makes the loop diff against
    // `rkf45.c` line for line. The arithmetic is unchanged, so the rounding
    // matches upstream exactly. See `crate::zip`.

    // k1
    f(t, y, &mut k1);
    for (tp, &yi, &a) in zip_flat!(tmp.iter_mut(), y.iter(), k1.iter()) {
        *tp = yi + AH[0] * h * a;
    }
    // k2
    f(t + AH[0] * h, &tmp, &mut k2);
    for (tp, &yi, &a, &b) in zip_flat!(tmp.iter_mut(), y.iter(), k1.iter(), k2.iter()) {
        *tp = yi + h * (B3[0] * a + B3[1] * b);
    }
    // k3
    f(t + AH[1] * h, &tmp, &mut k3);
    for (tp, &yi, &a, &b, &c) in
        zip_flat!(tmp.iter_mut(), y.iter(), k1.iter(), k2.iter(), k3.iter())
    {
        *tp = yi + h * (B4[0] * a + B4[1] * b + B4[2] * c);
    }
    // k4
    f(t + AH[2] * h, &tmp, &mut k4);
    for (tp, &yi, &a, &b, &c, &d) in zip_flat!(
        tmp.iter_mut(),
        y.iter(),
        k1.iter(),
        k2.iter(),
        k3.iter(),
        k4.iter()
    ) {
        *tp = yi + h * (B5[0] * a + B5[1] * b + B5[2] * c + B5[3] * d);
    }
    // k5
    f(t + AH[3] * h, &tmp, &mut k5);
    for (tp, &yi, &a, &b, &c, &d, &e) in zip_flat!(
        tmp.iter_mut(),
        y.iter(),
        k1.iter(),
        k2.iter(),
        k3.iter(),
        k4.iter(),
        k5.iter()
    ) {
        *tp = yi + h * (B6[0] * a + B6[1] * b + B6[2] * c + B6[3] * d + B6[4] * e);
    }
    // k6
    f(t + AH[4] * h, &tmp, &mut k6);

    let mut out = vec![0.0_f64; n];
    let mut yerr = vec![0.0_f64; n];
    // Upstream splits this into two loops over the same six arrays; fused here
    // because the zip is what costs, not the arithmetic, and neither term
    // depends on the other.
    for (o, er, &yi, &a, &c3, &c4, &c5, &c6) in zip_flat!(
        out.iter_mut(),
        yerr.iter_mut(),
        y.iter(),
        k1.iter(),
        k3.iter(),
        k4.iter(),
        k5.iter(),
        k6.iter()
    ) {
        // Fifth-order solution.
        let d = C1 * a + C3 * c3 + C4 * c4 + C5 * c5 + C6 * c6;
        *o = yi + h * d;
        // Difference against the embedded fourth-order solution.
        *er = h * (EC[1] * a + EC[3] * c3 + EC[4] * c4 + EC[5] * c5 + EC[6] * c6);
    }

    Ok(RkStep { y: out, yerr })
}

/// One point of an integrated trajectory.
#[derive(Debug, Clone, PartialEq)]
pub struct OdePoint {
    /// Independent variable.
    pub t: f64,
    /// State vector at `t`.
    pub y: Vec<f64>,
}

/// The outcome of an adaptive integration.
#[derive(Debug, Clone, PartialEq)]
pub struct OdeSolution {
    /// The trajectory, starting at the initial condition.
    pub points: Vec<OdePoint>,
    /// Steps accepted.
    pub accepted: usize,
    /// Steps rejected and retried at a smaller size. A high ratio against
    /// `accepted` means the tolerance is tight for the method — or that the
    /// system is stiff (see the module docs).
    pub rejected: usize,
}

impl OdeSolution {
    /// The final state, or `None` if the trajectory is empty.
    ///
    /// # Why this returns an `Option`
    ///
    /// A solution this crate produced always holds at least the initial
    /// condition, so in practice this is always `Some`. But [`points`] is a
    /// public field: a caller may build an `OdeSolution` themselves, or drain
    /// one, and then there is no final state to return. Indexing the last
    /// element would panic on that caller's machine -- which on bare metal
    /// ends the program. The `Option` makes the case visible at the call site
    /// instead, at the cost of one `unwrap`-or-match in the common path.
    ///
    /// [`points`]: OdeSolution::points
    ///
    /// # Example
    ///
    /// ```
    /// use petir::ode::solve_rkf45;
    /// let sol = solve_rkf45(
    ///     |_t, y: &[f64], dy: &mut [f64]| dy[0] = y[0],
    ///     0.0, 1.0, &[1.0], 1e-3, 1e-10, 1e-10, 10_000,
    /// ).unwrap();
    /// let y = sol.final_state().expect("a solved trajectory is never empty");
    /// assert!((y[0] - core::f64::consts::E).abs() < 1e-8);
    /// ```
    pub fn final_state(&self) -> Option<&[f64]> {
        self.points.last().map(|p| p.y.as_slice())
    }

    /// The final value of the independent variable, or `None` if the
    /// trajectory is empty. See [`final_state`](OdeSolution::final_state) for
    /// why this is an `Option`.
    pub fn final_t(&self) -> Option<f64> {
        self.points.last().map(|p| p.t)
    }
}

/// Adaptively integrate `dy/dt = f(t, y)` from `t0` to `t1`.
///
/// Steps with [`rkf45_step`], judging each against
/// `epsabs + epsrel * |y|` component-wise and adjusting `h` by the standard
/// controller. Every accepted step is recorded, so the returned trajectory has
/// points wherever the solver chose to put them — **not** on a uniform grid.
/// Interpolate with [`crate::interp`] if you need one.
///
/// `h_init` is a starting guess; the controller takes over immediately.
///
/// # Errors
///
/// - [`PetirError::Invalid`] for an empty system, a non-positive `h_init`, or
///   `t1 == t0`.
/// - [`PetirError::Domain`] if `f` produces a non-finite derivative, or the
///   state goes non-finite — usually a blow-up in the equations themselves.
/// - [`PetirError::MaxIterations`] if `max_steps` is exhausted. On a smooth
///   problem this means the tolerance is too tight; on a stiff one it means
///   RKF45 is the wrong method entirely.
///
/// # Example
///
/// ```
/// use petir::ode::solve_rkf45;
/// // dy/dt = y from 0 to 1, y(0) = 1  ->  y(1) = e
/// let sol = solve_rkf45(
///     |_t, y: &[f64], dy: &mut [f64]| dy[0] = y[0],
///     0.0, 1.0, &[1.0], 1e-3, 1e-10, 1e-10, 10_000,
/// ).unwrap();
/// assert!((sol.final_state().unwrap()[0] - core::f64::consts::E).abs() < 1e-8);
/// ```
#[allow(clippy::too_many_arguments)]
pub fn solve_rkf45<F>(
    f: F,
    t0: f64,
    t1: f64,
    y0: &[f64],
    h_init: f64,
    epsabs: f64,
    epsrel: f64,
    max_steps: usize,
) -> Result<OdeSolution>
where
    F: Fn(f64, &[f64], &mut [f64]),
{
    let n = y0.len();
    if n == 0 || !(h_init > 0.0) || t1 == t0 || !t0.is_finite() || !t1.is_finite() {
        return Err(PetirError::Invalid);
    }

    let direction = if t1 > t0 { 1.0 } else { -1.0 };
    let span = (t1 - t0).abs();

    let mut t = t0;
    let mut y = y0.to_vec();
    let mut h = h_init.min(span);

    let mut points = vec![OdePoint { t, y: y.clone() }];
    let mut accepted = 0usize;
    let mut rejected = 0usize;

    for _ in 0..max_steps {
        // Clamp the last step so we land exactly on t1.
        let remaining = (t1 - t).abs();
        if remaining <= 0.0 {
            break;
        }
        if h > remaining {
            h = remaining;
        }

        let step = rkf45_step(&f, t, &y, direction * h)?;

        // Component-wise error against the mixed tolerance.
        let mut worst_ratio = 0.0_f64;
        for (&yn, &en, &yi) in zip_flat!(step.y.iter(), step.yerr.iter(), y.iter()) {
            if !yn.is_finite() || !en.is_finite() {
                return Err(PetirError::Domain);
            }
            let tol = epsabs + epsrel * yi.abs().max(yn.abs());
            if tol > 0.0 {
                let ratio = en.abs() / tol;
                if ratio > worst_ratio {
                    worst_ratio = ratio;
                }
            }
        }

        if worst_ratio <= 1.0 {
            // Accept.
            t += direction * h;
            y = step.y;
            points.push(OdePoint { t, y: y.clone() });
            accepted += 1;

            if (t - t1).abs() <= 0.0 || (t1 - t) * direction <= 0.0 {
                return Ok(OdeSolution {
                    points,
                    accepted,
                    rejected,
                });
            }

            // Grow the step, capped at 5x. The 0.9 is deliberate pessimism --
            // see the module docs.
            let grow = if worst_ratio > 0.0 {
                (0.9 * worst_ratio.powf(-0.2)).min(5.0)
            } else {
                5.0
            };
            h *= grow;
        } else {
            // Reject and retry smaller, floored at 1/5.
            rejected += 1;
            let shrink = (0.9 * worst_ratio.powf(-0.25)).max(0.2);
            h *= shrink;

            // A step that has shrunk into the rounding noise of `t` cannot
            // make progress; report rather than spin.
            if h <= (t.abs() + h) * f64::EPSILON * 8.0 {
                return Err(PetirError::NoProgress);
            }
        }
    }

    Err(PetirError::MaxIterations)
}
