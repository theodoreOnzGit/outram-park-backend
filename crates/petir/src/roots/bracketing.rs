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
//   roots/bisection.c  bisection_init / bisection_iterate
//   roots/falsepos.c   falsepos_init  / falsepos_iterate
//   roots/brent.c      brent_init     / brent_iterate
//   roots/fsolver.c    gsl_root_fsolver_{iterate,root,x_lower,x_upper}
//
// all folded into `BracketingSolver`, dispatched on `BracketingMethod`.

//! Bracketing root finders — GSL's `roots/` `fsolver` family.
//!
//! # What "bracketing" buys you
//!
//! These methods need an interval whose endpoints give `f` opposite signs. In
//! exchange they give a **guarantee**: by the intermediate value theorem a
//! continuous `f` has a root in that interval, and every iteration shrinks the
//! interval while preserving the sign change. The root cannot escape. Nothing
//! in [`crate::roots::polishing`] offers that — those methods converge faster
//! and can diverge.
//!
//! So: if you can bracket, bracket.
//!
//! # Choosing a method
//!
//! | Method | Convergence | Guarantee | Use when |
//! |---|---|---|---|
//! | [`BracketingMethod::Bisection`] | linear, exactly one bit per step | interval halves every step | you want predictability above all, or `f` is nasty |
//! | [`BracketingMethod::FalsePosition`] | superlinear on nice `f`, linear at worst | interval shrinks, but one endpoint can stick | `f` is close to linear near the root |
//! | [`BracketingMethod::Brent`] | superlinear | interval shrinks; falls back to bisection | **the default choice** |
//!
//! [`BracketingMethod::Brent`] is the right default: it attempts inverse
//! quadratic interpolation and falls back to bisection whenever that step looks
//! unreliable, so it is never much worse than bisection and is usually far
//! better.
//!
//! The classic false-position failure is worth knowing, because it looks like
//! success: for a convex `f`, one endpoint can stop moving entirely, so the
//! bracket stays wide while the iterate converges. [`test_interval`] then never
//! fires. GSL's implementation (ported here) mitigates it by interleaving a
//! bisection step whenever the retained sub-interval is more than half the
//! previous one — so the bracket does keep shrinking — but the underlying
//! asymmetry remains.
//!
//! [`test_interval`]: crate::roots::test_interval
//!
//! # No trait objects
//!
//! GSL dispatches through a `gsl_root_fsolver_type` vtable. Here the method is
//! an enum and the state is an enum, matched at each step — the workspace's
//! no-`dyn` rule, and it means adding a method is a compile error at every
//! `match` rather than a silent gap.

// Under a std-linked build (`cargo test`) f64's inherent abs shadows this
// trait, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::scalar::DBL_EPSILON;
use crate::{PetirError, Result};

/// Which bracketing algorithm a [`BracketingSolver`] runs.
///
/// See the module documentation for how to choose. [`Self::Brent`] is the
/// default unless you have a specific reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BracketingMethod {
    /// Interval halving. Linear convergence, maximally robust.
    /// GSL `gsl_root_fsolver_bisection`.
    Bisection,
    /// Linear interpolation between the endpoints, with GSL's bisection
    /// safeguard. GSL `gsl_root_fsolver_falsepos`.
    FalsePosition,
    /// Brent-Dekker: inverse quadratic interpolation with a bisection
    /// fallback. GSL `gsl_root_fsolver_brent`.
    Brent,
}

/// Per-method iteration state.
#[derive(Debug, Clone, Copy)]
enum State {
    /// Bisection and false position both need only the endpoint values.
    Endpoints { f_lower: f64, f_upper: f64 },
    /// Brent carries three points and two step lengths.
    Brent {
        a: f64,
        b: f64,
        c: f64,
        d: f64,
        e: f64,
        fa: f64,
        fb: f64,
        fc: f64,
    },
}

/// A bracketing root solver, stepped one iteration at a time.
///
/// Mirrors GSL's `gsl_root_fsolver`: construct with a bracket, call
/// [`Self::iterate`] repeatedly, and check convergence yourself with
/// [`crate::roots::test_interval`]. [`Self::solve`] wraps that loop when you do
/// not need the control.
///
/// # Example
///
/// ```
/// use petir::roots::{BracketingMethod, BracketingSolver, test_interval, Convergence};
///
/// // Find the root of x^2 - 2 in [0, 2]: sqrt(2).
/// let f = |x: f64| x * x - 2.0;
/// let mut s = BracketingSolver::new(BracketingMethod::Brent, f, 0.0, 2.0).unwrap();
/// for _ in 0..100 {
///     s.iterate().unwrap();
///     if test_interval(s.x_lower(), s.x_upper(), 0.0, 1e-12).unwrap()
///         == Convergence::Converged
///     {
///         break;
///     }
/// }
/// assert!((s.root() - 2.0_f64.sqrt()).abs() < 1e-12);
/// ```
#[derive(Debug, Clone)]
pub struct BracketingSolver<F>
where
    F: Fn(f64) -> f64,
{
    method: BracketingMethod,
    f: F,
    root: f64,
    x_lower: f64,
    x_upper: f64,
    state: State,
}

impl<F> BracketingSolver<F>
where
    F: Fn(f64) -> f64,
{
    /// Create a solver for `f` on the bracket `[x_lower, x_upper]`.
    ///
    /// # Errors
    ///
    /// - [`PetirError::Invalid`] if `x_lower > x_upper`, or if the endpoints do
    ///   not straddle zero — GSL's "endpoints do not straddle y=0". This is the
    ///   check that makes the guarantee real, so it is not optional and not
    ///   merely advisory.
    /// - [`PetirError::Domain`] if `f` is not finite at either endpoint.
    ///
    /// # A root exactly at an endpoint is accepted
    ///
    /// `f(x_lower) == 0.0` or `f(x_upper) == 0.0` counts as straddling, and the
    /// first [`Self::iterate`] returns that endpoint. GSL behaves the same way;
    /// rejecting it would be surprising.
    pub fn new(method: BracketingMethod, f: F, x_lower: f64, x_upper: f64) -> Result<Self> {
        if x_lower > x_upper {
            return Err(PetirError::Invalid);
        }

        let f_lower = f(x_lower);
        let f_upper = f(x_upper);

        if !f_lower.is_finite() || !f_upper.is_finite() {
            return Err(PetirError::Domain);
        }

        // GSL: "endpoints do not straddle y=0", GSL_EINVAL.
        if (f_lower < 0.0 && f_upper < 0.0) || (f_lower > 0.0 && f_upper > 0.0) {
            return Err(PetirError::Invalid);
        }

        let root = 0.5 * (x_lower + x_upper);

        let state = match method {
            BracketingMethod::Bisection | BracketingMethod::FalsePosition => {
                State::Endpoints { f_lower, f_upper }
            }
            BracketingMethod::Brent => State::Brent {
                a: x_lower,
                fa: f_lower,
                b: x_upper,
                fb: f_upper,
                c: x_upper,
                fc: f_upper,
                d: x_upper - x_lower,
                e: x_upper - x_lower,
            },
        };

        Ok(Self {
            method,
            f,
            root,
            x_lower,
            x_upper,
            state,
        })
    }

    /// Advance the solver by one iteration.
    ///
    /// # Errors
    ///
    /// [`PetirError::Domain`] if `f` returns a non-finite value at a newly
    /// probed point.
    pub fn iterate(&mut self) -> Result<()> {
        match self.method {
            BracketingMethod::Bisection => self.iterate_bisection(),
            BracketingMethod::FalsePosition => self.iterate_falsepos(),
            BracketingMethod::Brent => self.iterate_brent(),
        }
    }

    /// Evaluate `f`, rejecting non-finite results as GSL's `SAFE_FUNC_CALL` does.
    fn eval(&self, x: f64) -> Result<f64> {
        let y = (self.f)(x);
        if y.is_finite() {
            Ok(y)
        } else {
            Err(PetirError::Domain)
        }
    }

    fn iterate_bisection(&mut self) -> Result<()> {
        let State::Endpoints { f_lower, f_upper } = self.state else {
            return Err(PetirError::Invalid);
        };
        let x_left = self.x_lower;
        let x_right = self.x_upper;

        if f_lower == 0.0 {
            self.root = x_left;
            self.x_upper = x_left;
            return Ok(());
        }
        if f_upper == 0.0 {
            self.root = x_right;
            self.x_lower = x_right;
            return Ok(());
        }

        let x_bisect = (x_left + x_right) / 2.0;
        let f_bisect = self.eval(x_bisect)?;

        if f_bisect == 0.0 {
            self.root = x_bisect;
            self.x_lower = x_bisect;
            self.x_upper = x_bisect;
            return Ok(());
        }

        // Discard the half of the interval which doesn't contain the root.
        if (f_lower > 0.0 && f_bisect < 0.0) || (f_lower < 0.0 && f_bisect > 0.0) {
            self.root = 0.5 * (x_left + x_bisect);
            self.x_upper = x_bisect;
            self.state = State::Endpoints {
                f_lower,
                f_upper: f_bisect,
            };
        } else {
            self.root = 0.5 * (x_bisect + x_right);
            self.x_lower = x_bisect;
            self.state = State::Endpoints {
                f_lower: f_bisect,
                f_upper,
            };
        }
        Ok(())
    }

    fn iterate_falsepos(&mut self) -> Result<()> {
        let State::Endpoints { f_lower, f_upper } = self.state else {
            return Err(PetirError::Invalid);
        };
        let x_left = self.x_lower;
        let x_right = self.x_upper;

        if f_lower == 0.0 {
            self.root = x_left;
            self.x_upper = x_left;
            return Ok(());
        }
        if f_upper == 0.0 {
            self.root = x_right;
            self.x_lower = x_right;
            return Ok(());
        }

        // Linear interpolation between the endpoints.
        let x_linear = x_right - (f_upper * (x_left - x_right) / (f_lower - f_upper));
        let f_linear = self.eval(x_linear)?;

        if f_linear == 0.0 {
            self.root = x_linear;
            self.x_lower = x_linear;
            self.x_upper = x_linear;
            return Ok(());
        }

        let w;
        if (f_lower > 0.0 && f_linear < 0.0) || (f_lower < 0.0 && f_linear > 0.0) {
            self.root = x_linear;
            self.x_upper = x_linear;
            self.state = State::Endpoints {
                f_lower,
                f_upper: f_linear,
            };
            w = x_linear - x_left;
        } else {
            self.root = x_linear;
            self.x_lower = x_linear;
            self.state = State::Endpoints {
                f_lower: f_linear,
                f_upper,
            };
            w = x_right - x_linear;
        }

        // GSL's safeguard: if the interpolation step already more than halved
        // the interval, accept it. Otherwise force a bisection so the bracket
        // cannot stagnate -- this is what stops the classic false-position
        // failure where one endpoint never moves.
        if w < 0.5 * (x_right - x_left) {
            return Ok(());
        }

        let x_bisect = 0.5 * (x_left + x_right);
        let f_bisect = self.eval(x_bisect)?;

        let State::Endpoints {
            f_lower: fl,
            f_upper: fu,
        } = self.state
        else {
            return Err(PetirError::Invalid);
        };

        if (f_lower > 0.0 && f_bisect < 0.0) || (f_lower < 0.0 && f_bisect > 0.0) {
            self.x_upper = x_bisect;
            self.state = State::Endpoints {
                f_lower: fl,
                f_upper: f_bisect,
            };
            if self.root > x_bisect {
                self.root = 0.5 * (x_left + x_bisect);
            }
        } else {
            self.x_lower = x_bisect;
            self.state = State::Endpoints {
                f_lower: f_bisect,
                f_upper: fu,
            };
            if self.root < x_bisect {
                self.root = 0.5 * (x_bisect + x_right);
            }
        }
        Ok(())
    }

    fn iterate_brent(&mut self) -> Result<()> {
        let State::Brent {
            mut a,
            mut b,
            mut c,
            mut d,
            mut e,
            mut fa,
            mut fb,
            mut fc,
        } = self.state
        else {
            return Err(PetirError::Invalid);
        };

        let mut ac_equal = false;

        if (fb < 0.0 && fc < 0.0) || (fb > 0.0 && fc > 0.0) {
            ac_equal = true;
            c = a;
            fc = fa;
            d = b - a;
            e = b - a;
        }

        if fc.abs() < fb.abs() {
            ac_equal = true;
            a = b;
            b = c;
            c = a;
            fa = fb;
            fb = fc;
            fc = fa;
        }

        let tol = 0.5 * DBL_EPSILON * b.abs();
        let m = 0.5 * (c - b);

        if fb == 0.0 {
            self.root = b;
            self.x_lower = b;
            self.x_upper = b;
            self.state = State::Brent {
                a,
                b,
                c,
                d,
                e,
                fa,
                fb,
                fc,
            };
            return Ok(());
        }

        if m.abs() <= tol {
            self.root = b;
            if b < c {
                self.x_lower = b;
                self.x_upper = c;
            } else {
                self.x_lower = c;
                self.x_upper = b;
            }
            self.state = State::Brent {
                a,
                b,
                c,
                d,
                e,
                fa,
                fb,
                fc,
            };
            return Ok(());
        }

        if e.abs() < tol || fa.abs() <= fb.abs() {
            // Use bisection.
            d = m;
            e = m;
        } else {
            // Use inverse quadratic (or linear, when a and c coincide)
            // interpolation.
            let mut p;
            let mut q;
            let s = fb / fa;

            if ac_equal {
                p = 2.0 * m * s;
                q = 1.0 - s;
            } else {
                let qq = fa / fc;
                let r = fb / fc;
                p = s * (2.0 * m * qq * (qq - r) - (b - a) * (r - 1.0));
                q = (qq - 1.0) * (r - 1.0) * (s - 1.0);
            }

            if p > 0.0 {
                q = -q;
            } else {
                p = -p;
            }

            let bound = {
                let lhs = 3.0 * m * q - (tol * q).abs();
                let rhs = (e * q).abs();
                if lhs < rhs {
                    lhs
                } else {
                    rhs
                }
            };

            if 2.0 * p < bound {
                e = d;
                d = p / q;
            } else {
                // Interpolation failed; fall back to bisection.
                d = m;
                e = m;
            }
        }

        a = b;
        fa = fb;

        if d.abs() > tol {
            b += d;
        } else {
            b += if m > 0.0 { tol } else { -tol };
        }

        fb = self.eval(b)?;

        self.root = b;

        // Update the reported bracket. Note GSL recomputes `c` here purely for
        // the bounds report and does NOT store that change back into the state.
        let c_report = if (fb < 0.0 && fc < 0.0) || (fb > 0.0 && fc > 0.0) {
            a
        } else {
            c
        };

        self.state = State::Brent {
            a,
            b,
            c,
            d,
            e,
            fa,
            fb,
            fc,
        };

        if b < c_report {
            self.x_lower = b;
            self.x_upper = c_report;
        } else {
            self.x_lower = c_report;
            self.x_upper = b;
        }
        Ok(())
    }

    /// The current best estimate of the root.
    pub fn root(&self) -> f64 {
        self.root
    }

    /// The current lower bound of the bracket.
    pub fn x_lower(&self) -> f64 {
        self.x_lower
    }

    /// The current upper bound of the bracket.
    pub fn x_upper(&self) -> f64 {
        self.x_upper
    }

    /// The method this solver runs.
    pub fn method(&self) -> BracketingMethod {
        self.method
    }

    /// Iterate to convergence on the bracket width, or until `max_iter` steps.
    ///
    /// Convenience over the explicit loop, using
    /// [`crate::roots::test_interval`] with the given tolerances.
    ///
    /// # Errors
    ///
    /// [`PetirError::MaxIterations`] if the tolerance is not met within
    /// `max_iter` iterations — the current estimate may still be perfectly
    /// good, so call [`Self::root`] if you want it anyway. Also propagates any
    /// error from [`Self::iterate`].
    ///
    /// # Example
    ///
    /// ```
    /// use petir::roots::{BracketingMethod, BracketingSolver};
    /// let mut s = BracketingSolver::new(BracketingMethod::Brent, |x: f64| x * x - 2.0, 0.0, 2.0)
    ///     .unwrap();
    /// let r = s.solve(0.0, 1e-12, 100).unwrap();
    /// assert!((r - 2.0_f64.sqrt()).abs() < 1e-12);
    /// ```
    pub fn solve(&mut self, epsabs: f64, epsrel: f64, max_iter: usize) -> Result<f64> {
        for _ in 0..max_iter {
            self.iterate()?;
            if super::test_interval(self.x_lower, self.x_upper, epsabs, epsrel)?
                .is_converged()
            {
                return Ok(self.root);
            }
        }
        Err(PetirError::MaxIterations)
    }
}
