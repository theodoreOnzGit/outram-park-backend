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
// Copyright (C) 1996-2000 Reid Priedhorsky, Brian Gough
// GPL-3.0-or-later (verified from the per-file headers; see NOTICE).
//
//   roots/newton.c      newton_init     / newton_iterate
//   roots/secant.c      secant_init     / secant_iterate
//   roots/steffenson.c  steffenson_init / steffenson_iterate
//   roots/fdfsolver.c   gsl_root_fdfsolver_{iterate,root}
//
// folded into `PolishingSolver`, dispatched on `PolishingMethod`.

//! Derivative-based root finders — GSL's `roots/` `fdfsolver` family.
//!
//! # These do not bracket, and that is the whole trade
//!
//! Given a good starting guess these converge far faster than anything in
//! [`crate::roots::bracketing`] — Newton doubles its correct digits each step.
//! Given a bad one they can oscillate, run off to infinity, or land on a
//! different root entirely, and **nothing here will tell you that happened**.
//! There is no interval, so there is no guarantee.
//!
//! The practical advice is unglamorous: if you can bracket the root, use
//! [`BracketingMethod::Brent`] and stop reading. Reach for these when you
//! genuinely cannot bracket, when the derivative is cheap and exact, or when
//! you are polishing an answer you already almost have — which is where the
//! name comes from.
//!
//! [`BracketingMethod::Brent`]: crate::roots::BracketingMethod::Brent
//!
//! # Choosing a method
//!
//! | Method | Convergence near a simple root | Derivative | Cost per step |
//! |---|---|---|---|
//! | [`PolishingMethod::Newton`] | quadratic | exact, every step | one `f` and one `f'` |
//! | [`PolishingMethod::Secant`] | golden ratio, ~1.62 | exact once, then estimated | one `f` |
//! | [`PolishingMethod::Steffenson`] | quadratic, usually fastest | exact, every step | one `f` and one `f'` |
//!
//! [`PolishingMethod::Secant`] is the one to pick when `f'` is expensive: it
//! evaluates the true derivative only at the start and then updates it from
//! successive function values, so each later step costs a single `f`.
//!
//! [`PolishingMethod::Steffenson`] is Newton plus Aitken's delta-squared
//! acceleration. It is normally the fastest of the three and it is also the
//! least forgiving — the acceleration assumes the iterates are converging
//! geometrically, and amplifies the error when they are not.
//!
//! # A root of even multiplicity defeats all three
//!
//! At a double root `f` and `f'` vanish together, Newton's quadratic
//! convergence degrades to linear, and the step `f/f'` becomes `0/0`. You get
//! [`PetirError::ZeroDivide`] if the derivative reaches exactly zero and slow
//! creeping progress if it merely gets small. Bracketing methods have the same
//! trouble for a different reason — `f` does not change sign at a double root,
//! so there is nothing to bracket.

// Under a std-linked build (`cargo test`) f64's inherent abs shadows this
// trait, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::{PetirError, Result};

/// Which derivative-based algorithm a [`PolishingSolver`] runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolishingMethod {
    /// Newton-Raphson: `x <- x - f/f'`. GSL `gsl_root_fdfsolver_newton`.
    Newton,
    /// Secant: Newton's step with the derivative estimated from successive
    /// function values after the first. GSL `gsl_root_fdfsolver_secant`.
    Secant,
    /// Newton with Aitken delta-squared acceleration.
    /// GSL `gsl_root_fdfsolver_steffenson`.
    Steffenson,
}

/// A derivative-based root solver, stepped one iteration at a time.
///
/// Mirrors GSL's `gsl_root_fdfsolver`. Because there is no interval, check
/// convergence with [`crate::roots::test_delta`] (successive iterates stopped
/// moving) or [`crate::roots::test_residual`] — and read the caveats on both,
/// which matter more here than they do for a bracketing solver.
///
/// # Example
///
/// ```
/// use petir::roots::{PolishingMethod, PolishingSolver};
///
/// // sqrt(2) as the root of x^2 - 2, from a guess of 1.
/// let f = |x: f64| x * x - 2.0;
/// let df = |x: f64| 2.0 * x;
/// let mut s = PolishingSolver::new(PolishingMethod::Newton, f, df, 1.0).unwrap();
/// let r = s.solve(1e-14, 0.0, 100).unwrap();
/// assert!((r - 2.0_f64.sqrt()).abs() < 1e-14);
/// ```
#[derive(Debug, Clone)]
pub struct PolishingSolver<F, DF>
where
    F: Fn(f64) -> f64,
    DF: Fn(f64) -> f64,
{
    method: PolishingMethod,
    f: F,
    df: DF,
    /// The reported best estimate. For Steffenson this may be the Aitken-
    /// ACCELERATED value, which is NOT what drives the next iteration.
    root: f64,
    /// The unaccelerated iterate that drives the next step -- GSL's `state->x`,
    /// kept SEPARATE from `*root`.
    ///
    /// For Newton and Secant the two always coincide. For Steffenson they do
    /// not, and conflating them breaks the algorithm: feeding the accelerated
    /// value back into the Newton step makes the iteration diverge (observed
    /// as a NaN root on `x^20 - 1`). This field exists because of that bug.
    x: f64,
    /// Latest function value at `x`.
    fx: f64,
    /// Latest derivative estimate. For [`PolishingMethod::Secant`] this is an
    /// estimate after the first step, not the true `f'`.
    dfx: f64,
    /// Steffenson only: the two previous iterates, and how many steps have
    /// been taken (acceleration starts once three are available).
    x_1: f64,
    x_2: f64,
    count: u32,
}

impl<F, DF> PolishingSolver<F, DF>
where
    F: Fn(f64) -> f64,
    DF: Fn(f64) -> f64,
{
    /// Create a solver for `f` with derivative `df`, starting from `guess`.
    ///
    /// # Errors
    ///
    /// [`PetirError::Domain`] if `f` or `df` is not finite at `guess`.
    ///
    /// # On the derivative
    ///
    /// `df` must be the actual derivative of `f`. Nothing checks this, and a
    /// wrong derivative does not produce an error — it produces convergence to
    /// the wrong place, or no convergence, with no diagnostic. If the
    /// derivative is only available numerically, prefer a bracketing method:
    /// a finite-difference `f'` inside Newton tends to be both slower and less
    /// reliable than Brent.
    pub fn new(method: PolishingMethod, f: F, df: DF, guess: f64) -> Result<Self> {
        let fx = f(guess);
        let dfx = df(guess);
        if !fx.is_finite() || !dfx.is_finite() {
            return Err(PetirError::Domain);
        }
        Ok(Self {
            method,
            f,
            df,
            root: guess,
            x: guess,
            fx,
            dfx,
            x_1: 0.0,
            x_2: 0.0,
            count: 1,
        })
    }

    /// Advance by one iteration.
    ///
    /// # Errors
    ///
    /// - [`PetirError::ZeroDivide`] if the derivative is exactly zero, which
    ///   is GSL's `GSL_EZERODIV` for this case.
    /// - [`PetirError::Domain`] if `f` or `df` goes non-finite — GSL's
    ///   `GSL_EBADFUNC`. This is the usual symptom of a diverging iteration.
    pub fn iterate(&mut self) -> Result<()> {
        match self.method {
            PolishingMethod::Newton => self.iterate_newton(),
            PolishingMethod::Secant => self.iterate_secant(),
            PolishingMethod::Steffenson => self.iterate_steffenson(),
        }
    }

    fn iterate_newton(&mut self) -> Result<()> {
        if self.dfx == 0.0 {
            return Err(PetirError::ZeroDivide);
        }
        let root_new = self.x - (self.fx / self.dfx);

        let f_new = (self.f)(root_new);
        let df_new = (self.df)(root_new);

        self.x = root_new;
        self.root = root_new;
        self.fx = f_new;
        self.dfx = df_new;

        if !f_new.is_finite() || !df_new.is_finite() {
            return Err(PetirError::Domain);
        }
        Ok(())
    }

    fn iterate_secant(&mut self) -> Result<()> {
        let x = self.x;
        let f = self.fx;
        let df = self.dfx;

        // GSL returns success immediately on an exact root rather than
        // dividing; the iterate stops moving, which test_delta then sees.
        if f == 0.0 {
            return Ok(());
        }
        if df == 0.0 {
            return Err(PetirError::ZeroDivide);
        }

        let x_new = x - (f / df);
        let f_new = (self.f)(x_new);
        // The secant update: the derivative is re-estimated from the change in
        // f, so the true `df` is never called again after construction.
        let df_new = df * ((f - f_new) / f);

        self.x = x_new;
        self.root = x_new;
        self.fx = f_new;
        self.dfx = df_new;

        if !f_new.is_finite() || !df_new.is_finite() {
            return Err(PetirError::Domain);
        }
        Ok(())
    }

    fn iterate_steffenson(&mut self) -> Result<()> {
        if self.dfx == 0.0 {
            return Err(PetirError::ZeroDivide);
        }

        let x_1 = self.x_1;
        let x = self.x;

        let x_new = x - (self.fx / self.dfx);
        let f_new = (self.f)(x_new);
        let df_new = (self.df)(x_new);

        self.x_2 = x_1;
        self.x_1 = x;
        self.x = x_new;
        self.root = x_new;
        self.fx = f_new;
        self.dfx = df_new;

        if !f_new.is_finite() {
            return Err(PetirError::Domain);
        }

        if self.count < 3 {
            self.count += 1;
        } else {
            // Aitken's delta-squared, applied once three iterates exist.
            let u = x - x_1;
            let v = x_new - 2.0 * x + x_1;
            if v == 0.0 {
                self.root = x_new; // avoid division by zero
            } else {
                self.root = x_1 - u * u / v; // accelerated value
            }
        }

        if !df_new.is_finite() {
            return Err(PetirError::Domain);
        }
        Ok(())
    }

    /// The current best estimate of the root.
    pub fn root(&self) -> f64 {
        self.root
    }

    /// The most recent function value.
    pub fn f_value(&self) -> f64 {
        self.fx
    }

    /// The method this solver runs.
    pub fn method(&self) -> PolishingMethod {
        self.method
    }

    /// Iterate until successive iterates agree, or `max_iter` steps elapse.
    ///
    /// Uses [`crate::roots::test_delta`] on consecutive iterates.
    ///
    /// # Errors
    ///
    /// [`PetirError::MaxIterations`] if the tolerance is not met in time; also
    /// propagates any error from [`Self::iterate`]. Note that
    /// [`PetirError::MaxIterations`] here is a weaker signal than it is for a
    /// bracketing solver: without an interval there is no bound on how far the
    /// current estimate might be from a root.
    pub fn solve(&mut self, epsabs: f64, epsrel: f64, max_iter: usize) -> Result<f64> {
        for _ in 0..max_iter {
            let prev = self.root;
            self.iterate()?;
            if super::test_delta(self.root, prev, epsabs, epsrel)?.is_converged() {
                return Ok(self.root);
            }
        }
        Err(PetirError::MaxIterations)
    }
}
