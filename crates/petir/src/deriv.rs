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
// Copyright (C) 2004, 2007 Brian Gough
// GPL-3.0-or-later (verified from the per-file headers; see NOTICE).
//
//   deriv/deriv.c  central_deriv        -> central_at_h
//                  gsl_deriv_central    -> central
//                  forward_deriv        -> forward_at_h
//                  gsl_deriv_forward    -> forward
//                  gsl_deriv_backward   -> backward

//! Numerical differentiation — GSL's `deriv/`.
//!
//! # Read this before differentiating anything numerically
//!
//! A numerical derivative is a subtraction of two nearly equal numbers divided
//! by a small one. Both halves of that are hostile:
//!
//! - Make `h` **smaller** and the truncation error falls, but `f(x+h) - f(x-h)`
//!   loses significant digits to cancellation, and dividing by a tiny `h`
//!   amplifies what is left.
//! - Make `h` **larger** and the cancellation improves, but the finite
//!   difference stops approximating the derivative.
//!
//! The best achievable accuracy is therefore **far worse than machine
//! precision**: about `eps^(2/3)`, roughly `4e-11` relative, for the central
//! rule. For the one-sided rules it is `eps^(1/2)`, about `1.5e-8`. No choice
//! of `h` beats those, and a routine claiming otherwise is not measuring its
//! own error.
//!
//! Every function here therefore returns `(value, abserr)` — **use the error
//! estimate**. It is not decoration; it is the only honest statement about how
//! much of the answer is real.
//!
//! # If you have an analytic derivative, use it
//!
//! This module exists for functions you cannot differentiate by hand — a
//! tabulated property, a black-box model, a composition too long to trust. Where
//! `f'` can be written down, writing it down is both faster and about ten
//! decimal digits more accurate. That is worth saying because the convenience of
//! [`central`] makes it tempting to skip the algebra.
//!
//! # Choosing a rule
//!
//! | Rule | Error | Evaluations | Use when |
//! |---|---|---|---|
//! | [`central`] | `O(h^2)`, best ~`4e-11` | 4 | **the default** — `f` is defined on both sides |
//! | [`forward`] | `O(h)`, best ~`1.5e-8` | 4 | `x` is at a lower boundary of `f`'s domain |
//! | [`backward`] | `O(h)`, best ~`1.5e-8` | 4 | `x` is at an upper boundary |
//!
//! # The step is refined automatically
//!
//! Each routine evaluates at the `h` you give, splits the error into rounding
//! and truncation parts, and — **when TRUNCATION dominates**, i.e. when `h` is
//! too large — retries at the smaller `h` that balances the two
//! (`h * (round / 2 trunc)^(1/3)` for the central rule). The refined value is
//! accepted only if it is both more accurate *and* consistent with the original
//! estimate's error bars, so a wildly different second answer is rejected
//! rather than trusted.
//!
//! **The refinement is one-directional, and that asymmetry matters.** Upstream
//! guards it with `round < trunc`, so an `h` that is too SMALL is never
//! rescued: once cancellation has eaten the significant digits there is nothing
//! left to recover, and enlarging `h` would need evaluations further from `x`
//! than the caller asked for. A too-small `h` instead shows up as a large
//! [`Derivative::abserr`] — at `h = 1e-13` the central rule loses about four
//! decimal digits, and reports that it has.
//!
//! So `h` is a hint that protects you from being too large, not from being too
//! small. A reasonable hint is a small fraction of the scale over which `f`
//! varies, and the error estimate is how you check it was reasonable.
//!
//! # Units
//!
//! Bare dimensionless `f64`.

// Under a std-linked build (`cargo test`) f64's inherent abs shadows this
// trait, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::scalar::DBL_EPSILON;

/// A differentiation result: the derivative and an estimate of its absolute
/// error.
///
/// Returned rather than a bare `f64` because the error is the load-bearing
/// half — see the module documentation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Derivative {
    /// The estimated derivative `f'(x)`.
    pub value: f64,
    /// Estimated absolute error in [`Self::value`].
    ///
    /// The sum of a rounding term (which grows as `h` shrinks) and a
    /// truncation term (which shrinks as `h` shrinks).
    pub abserr: f64,
}

/// The central-difference rule at a fixed `h`, with its error split.
///
/// Returns `(result, abserr_round, abserr_trunc)`. Translates GSL's
/// `central_deriv`.
fn central_at_h<F>(f: &F, x: f64, h: f64) -> (f64, f64, f64)
where
    F: Fn(f64) -> f64,
{
    // Compute the derivative using the 5-point rule (x-h, x-h/2, x, x+h/2,
    // x+h). Note that the central point is not used.
    //
    // Compute the error using the difference between the 5-point and the
    // 3-point rule (x-h, x, x+h). Again the central point is not used.
    let fm1 = f(x - h);
    let fp1 = f(x + h);

    let fmh = f(x - h / 2.0);
    let fph = f(x + h / 2.0);

    let r3 = 0.5 * (fp1 - fm1);
    let r5 = (4.0 / 3.0) * (fph - fmh) - (1.0 / 3.0) * r3;

    let e3 = (fp1.abs() + fm1.abs()) * DBL_EPSILON;
    let e5 = 2.0 * (fph.abs() + fmh.abs()) * DBL_EPSILON + e3;

    // The next term in the Taylor series is h^2 f'''/6; the rounding error
    // from evaluating x +/- h dominates when h is small.
    let dy = (r3 / h).abs().max((r5 / h).abs()) * (x.abs() / h) * DBL_EPSILON;

    let result = r5 / h;
    let abserr_trunc = ((r5 - r3) / h).abs(); // O(h^2)
    let abserr_round = (e5 / h).abs() + dy; // cancellation

    (result, abserr_round, abserr_trunc)
}

/// The forward-difference rule at a fixed `h`, with its error split.
///
/// Translates GSL's `forward_deriv`. A negative `h` makes it the backward rule,
/// which is exactly how [`backward`] is implemented — upstream does the same.
fn forward_at_h<F>(f: &F, x: f64, h: f64) -> (f64, f64, f64)
where
    F: Fn(f64) -> f64,
{
    // 4-point rule at (x+h/4, x+h/2, x+3h/4, x+h), with the error taken from
    // the difference against the 2-point rule (x+h/2, x+h).
    let f1 = f(x + h / 4.0);
    let f2 = f(x + h / 2.0);
    let f3 = f(x + (3.0 / 4.0) * h);
    let f4 = f(x + h);

    let r2 = 2.0 * (f4 - f2);
    let r4 = (22.0 / 3.0) * (f4 - f3) - (62.0 / 3.0) * (f3 - f2) + (52.0 / 3.0) * (f2 - f1);

    // The 20.67 is upstream's empirical bound on the coefficient growth of the
    // 4-point rule; kept as GSL prints it rather than re-derived.
    let e4 = 2.0 * 20.67 * (f4.abs() + f3.abs() + f2.abs() + f1.abs()) * DBL_EPSILON;

    let dy = (r2 / h).abs().max((r4 / h).abs()) * (x / h).abs() * DBL_EPSILON;

    let result = r4 / h;
    let abserr_trunc = ((r4 - r2) / h).abs(); // O(h)
    let abserr_round = (e4 / h).abs() + dy;

    (result, abserr_round, abserr_trunc)
}

/// Central difference: the default choice.
///
/// Evaluates `f` at `x ± h` and `x ± h/2` — **never at `x` itself**, which is
/// occasionally useful when `f(x)` is the one point that is undefined.
///
/// `h` is a starting hint; the routine shrinks it when truncation dominates. See
/// the module docs.
///
/// # Accuracy
///
/// `O(h^2)` truncation, with a best achievable relative accuracy of about
/// `eps^(2/3)`, roughly `4e-11`. Read [`Derivative::abserr`].
///
/// # Example
///
/// ```
/// use petir::deriv::central;
/// // d/dx x^(3/2) at x = 2 is 1.5 * sqrt(2).
/// let d = central(|x: f64| x.powf(1.5), 2.0, 1e-4);
/// assert!((d.value - 1.5 * 2.0_f64.sqrt()).abs() < 1e-10);
/// assert!(d.abserr < 1e-9);
/// ```
pub fn central<F>(f: F, x: f64, h: f64) -> Derivative
where
    F: Fn(f64) -> f64,
{
    let (r_0, round, trunc) = central_at_h(&f, x, h);
    let mut value = r_0;
    let mut error = round + trunc;

    if round < trunc && round > 0.0 && trunc > 0.0 {
        // Compute an optimised step size to minimise the total error, using
        // the scaling of the truncation error (O(h^2)) and rounding error
        // (O(1/h)).
        let h_opt = h * (round / (2.0 * trunc)).powf(1.0 / 3.0);
        let (r_opt, round_opt, trunc_opt) = central_at_h(&f, x, h_opt);
        let error_opt = round_opt + trunc_opt;

        // Check that the new error is smaller, and that the new derivative is
        // consistent with the error bounds of the original estimate.
        if error_opt < error && (r_opt - r_0).abs() < 4.0 * error {
            value = r_opt;
            error = error_opt;
        }
    }

    Derivative {
        value,
        abserr: error,
    }
}

/// Forward difference: evaluates `f` only at points **above** `x`.
///
/// For a lower boundary of the domain, where `f(x - h)` does not exist.
///
/// # Accuracy
///
/// `O(h)` truncation — an order worse than [`central`], with a best achievable
/// relative accuracy of about `eps^(1/2)`, roughly `1.5e-8`. Prefer [`central`]
/// wherever `f` is defined on both sides.
///
/// # Example
///
/// ```
/// use petir::deriv::forward;
/// // d/dx sqrt(x) at x = 0.25 is 1.
/// let d = forward(|x: f64| x.sqrt(), 0.25, 1e-4);
/// assert!((d.value - 1.0).abs() < 1e-7);
/// ```
pub fn forward<F>(f: F, x: f64, h: f64) -> Derivative
where
    F: Fn(f64) -> f64,
{
    let (r_0, round, trunc) = forward_at_h(&f, x, h);
    let mut value = r_0;
    let mut error = round + trunc;

    if round < trunc && round > 0.0 && trunc > 0.0 {
        // Truncation is O(h) here rather than O(h^2), so the balancing
        // exponent is 1/2 rather than 1/3.
        let h_opt = h * (round / trunc).powf(1.0 / 2.0);
        let (r_opt, round_opt, trunc_opt) = forward_at_h(&f, x, h_opt);
        let error_opt = round_opt + trunc_opt;

        if error_opt < error && (r_opt - r_0).abs() < 4.0 * error {
            value = r_opt;
            error = error_opt;
        }
    }

    Derivative {
        value,
        abserr: error,
    }
}

/// Backward difference: evaluates `f` only at points **below** `x`.
///
/// For an upper boundary of the domain. Implemented as [`forward`] with `-h`,
/// which is precisely what GSL does.
///
/// # Accuracy
///
/// As [`forward`].
///
/// # Example
///
/// ```
/// use petir::deriv::backward;
/// // d/dx ln(x) at x = 2 is 0.5.
/// let d = backward(|x: f64| x.ln(), 2.0, 1e-4);
/// assert!((d.value - 0.5).abs() < 1e-7);
/// ```
pub fn backward<F>(f: F, x: f64, h: f64) -> Derivative
where
    F: Fn(f64) -> f64,
{
    forward(f, x, -h)
}
