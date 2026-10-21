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
//   min/golden.c       goldensection_iterate -> MinMethod::GoldenSection
//   min/brent.c        brent_init / brent_iterate -> MinMethod::Brent
//   min/convergence.c  gsl_min_test_interval  -> test_interval
//   min/fsolver.c      gsl_min_fminimizer_{iterate,x_minimum,f_minimum,...}

//! One-dimensional minimisation — GSL's `min/`.
//!
//! # The bracket is three points, not two
//!
//! A root bracket needs a sign change. A **minimum** bracket needs a middle
//! point that is *lower than both ends*: `a < m < b` with
//! `f(m) < f(a)` and `f(m) < f(b)`. That triple guarantees a continuous `f` has
//! an interior minimum, and every iteration shrinks the interval while
//! preserving the property. [`Minimizer::new`] checks it, and refuses
//! otherwise — a "bracket" without it is not a bracket.
//!
//! Finding that triple in the first place is the caller's problem, and often
//! the harder half.
//!
//! # Accuracy: you only get half your digits, and that is not a bug
//!
//! Near a minimum `f` is locally quadratic, so
//! `f(x* + d) - f(x*) ~ 0.5 f''(x*) d^2`. A change in `x` of `d` moves `f` by
//! order `d^2` — which means a `d` of `1e-8` moves `f` by `1e-16`, i.e. below
//! the rounding error of `f` itself. **The location of a minimum cannot be
//! determined to better than about `sqrt(eps)` relative, around `1.5e-8`**, no
//! matter how many iterations you spend.
//!
//! This is why GSL's Brent minimiser uses `GSL_SQRT_DBL_EPSILON` as its
//! internal tolerance, and why asking [`Minimizer::solve`] for `1e-14` is a
//! request that cannot be met. Ask for `1e-8` and be suspicious of any library
//! that claims better.
//!
//! # Choosing a method
//!
//! - [`MinMethod::Brent`] — parabolic interpolation with a golden-section
//!   fallback. Superlinear on a smooth `f`. **The default.**
//! - [`MinMethod::GoldenSection`] — shrinks the bracket by the golden ratio
//!   every step, unconditionally. Slower but utterly predictable, and immune to
//!   the parabolic step misbehaving on a jagged `f`.
//!
//! # Units
//!
//! Bare dimensionless `f64`.

// Under a std-linked build (`cargo test`) f64's inherent abs shadows this
// trait, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::scalar::SQRT_DBL_EPSILON;
use crate::{PetirError, Result};

/// `(3 - sqrt(5)) / 2`, the golden-section ratio — GSL's `golden` literal.
///
/// Kept at upstream's printed precision rather than recomputed, so the
/// arithmetic matches GSL's step for step.
const GOLDEN: f64 = 0.381_966_0;

/// Which minimisation algorithm a [`Minimizer`] runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinMethod {
    /// Golden-section search. GSL `gsl_min_fminimizer_goldensection`.
    GoldenSection,
    /// Brent: parabolic interpolation with a golden-section fallback.
    /// GSL `gsl_min_fminimizer_brent`.
    Brent,
}

/// A one-dimensional minimiser over a bracketing triple.
///
/// Mirrors GSL's `gsl_min_fminimizer`.
///
/// # Example
///
/// ```
/// use petir::min::{MinMethod, Minimizer};
///
/// // cos has a minimum at pi on [0, 2pi].
/// let f = |x: f64| x.cos();
/// let pi = core::f64::consts::PI;
/// let mut m = Minimizer::new(MinMethod::Brent, f, pi - 0.5, 0.0, 2.0 * pi).unwrap();
/// // 1e-6, not 1e-9: a minimum's LOCATION cannot be pinned below sqrt(eps).
/// let x = m.solve(1e-6, 0.0, 2000).unwrap();
/// assert!((x - pi).abs() < 1e-6);
/// ```
#[derive(Debug, Clone)]
pub struct Minimizer<F>
where
    F: Fn(f64) -> f64,
{
    method: MinMethod,
    f: F,
    x_minimum: f64,
    f_minimum: f64,
    x_lower: f64,
    f_lower: f64,
    x_upper: f64,
    f_upper: f64,
    /// Brent only: the two previous best points and their values, plus the
    /// last two step lengths.
    v: f64,
    w: f64,
    f_v: f64,
    f_w: f64,
    d: f64,
    e: f64,
}

impl<F> Minimizer<F>
where
    F: Fn(f64) -> f64,
{
    /// Create a minimiser over the bracketing triple `x_lower < x_minimum < x_upper`.
    ///
    /// # Errors
    ///
    /// - [`PetirError::Invalid`] if the points are not ordered, or if
    ///   `f(x_minimum)` is not strictly below **both** `f(x_lower)` and
    ///   `f(x_upper)` — GSL's "endpoints do not enclose a minimum".
    /// - [`PetirError::Domain`] if `f` is not finite at any of the three.
    pub fn new(
        method: MinMethod,
        f: F,
        x_minimum: f64,
        x_lower: f64,
        x_upper: f64,
    ) -> Result<Self> {
        if !(x_lower < x_minimum && x_minimum < x_upper) {
            return Err(PetirError::Invalid);
        }

        let f_minimum = f(x_minimum);
        let f_lower = f(x_lower);
        let f_upper = f(x_upper);

        if !f_minimum.is_finite() || !f_lower.is_finite() || !f_upper.is_finite() {
            return Err(PetirError::Domain);
        }

        // GSL: "endpoints do not enclose a minimum".
        if !(f_minimum < f_lower && f_minimum < f_upper) {
            return Err(PetirError::Invalid);
        }

        // Brent's initial second-best points, per brent_init.
        let v = x_lower + GOLDEN * (x_upper - x_lower);
        let f_vw = f(v);
        if !f_vw.is_finite() {
            return Err(PetirError::Domain);
        }

        Ok(Self {
            method,
            f,
            x_minimum,
            f_minimum,
            x_lower,
            f_lower,
            x_upper,
            f_upper,
            v,
            w: v,
            f_v: f_vw,
            f_w: f_vw,
            d: 0.0,
            e: 0.0,
        })
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

    /// Advance by one iteration.
    ///
    /// # Errors
    ///
    /// - [`PetirError::Domain`] if `f` goes non-finite at a probed point.
    /// - [`PetirError::NoProgress`] if a golden-section step fails to improve
    ///   the bracket, which is GSL's `GSL_FAILURE` for that routine.
    pub fn iterate(&mut self) -> Result<()> {
        match self.method {
            MinMethod::GoldenSection => self.iterate_golden(),
            MinMethod::Brent => self.iterate_brent(),
        }
    }

    fn iterate_golden(&mut self) -> Result<()> {
        let x_center = self.x_minimum;
        let x_left = self.x_lower;
        let x_right = self.x_upper;
        let f_min = self.f_minimum;

        let w_lower = x_center - x_left;
        let w_upper = x_right - x_center;

        // Step into the LARGER of the two sub-intervals, by the golden ratio.
        let x_new = x_center + GOLDEN * if w_upper > w_lower { w_upper } else { -w_lower };
        let f_new = self.eval(x_new)?;

        if f_new < f_min {
            self.x_minimum = x_new;
            self.f_minimum = f_new;
            Ok(())
        } else if x_new < x_center && f_new > f_min {
            self.x_lower = x_new;
            self.f_lower = f_new;
            Ok(())
        } else if x_new > x_center && f_new > f_min {
            self.x_upper = x_new;
            self.f_upper = f_new;
            Ok(())
        } else {
            // GSL returns GSL_FAILURE here: f_new == f_min, so neither the
            // bracket nor the best point can be improved. A flat region.
            Err(PetirError::NoProgress)
        }
    }

    fn iterate_brent(&mut self) -> Result<()> {
        let x_left = self.x_lower;
        let x_right = self.x_upper;

        let z = self.x_minimum;
        // NOTE the deliberate swap: upstream reads `d = state->e` and
        // `e = state->d`. It looks like a typo and is not -- it is how the
        // "step before last" is carried, and swapping it back changes the
        // sequence of steps. Kept exactly as GSL has it.
        let mut d = self.e;
        let mut e = self.d;
        let v = self.v;
        let w = self.w;
        let f_v = self.f_v;
        let f_w = self.f_w;
        let f_z = self.f_minimum;

        let w_lower = z - x_left;
        let w_upper = x_right - z;

        // The sqrt(eps) floor: see the accuracy note in the module docs.
        let tolerance = SQRT_DBL_EPSILON * z.abs();

        let mut p = 0.0_f64;
        let mut q = 0.0_f64;
        let mut r = 0.0_f64;

        let midpoint = 0.5 * (x_left + x_right);

        if e.abs() > tolerance {
            // Fit a parabola through (v, f_v), (w, f_w), (z, f_z).
            r = (z - w) * (f_z - f_v);
            q = (z - v) * (f_z - f_w);
            p = (z - v) * q - (z - w) * r;
            q = 2.0 * (q - r);

            if q > 0.0 {
                p = -p;
            } else {
                q = -q;
            }

            r = e;
            e = d;
        }

        if p.abs() < (0.5 * q * r).abs() && p < q * w_lower && p < q * w_upper {
            // The parabolic step is acceptable.
            let t2 = 2.0 * tolerance;
            d = p / q;
            // `u` here is provisional: upstream computes it only to check the
            // step does not land within 2*tolerance of either endpoint, and
            // then recomputes it below. Scoped to this branch so it cannot be
            // mistaken for the step actually taken.
            let u_trial = z + d;
            if (u_trial - x_left) < t2 || (x_right - u_trial) < t2 {
                d = if z < midpoint { tolerance } else { -tolerance };
            }
        } else {
            // Fall back to golden section on the larger sub-interval.
            e = if z < midpoint {
                x_right - z
            } else {
                -(z - x_left)
            };
            d = GOLDEN * e;
        }

        // The step actually taken, floored at `tolerance` in magnitude so the
        // iteration always moves.
        let u = if d.abs() >= tolerance {
            z + d
        } else {
            z + if d > 0.0 { tolerance } else { -tolerance }
        };

        self.e = e;
        self.d = d;

        let f_u = self.eval(u)?;

        if f_u <= f_z {
            if u < z {
                self.x_upper = z;
                self.f_upper = f_z;
            } else {
                self.x_lower = z;
                self.f_lower = f_z;
            }
            self.v = w;
            self.f_v = f_w;
            self.w = z;
            self.f_w = f_z;
            self.x_minimum = u;
            self.f_minimum = f_u;
            return Ok(());
        }

        if u < z {
            self.x_lower = u;
            self.f_lower = f_u;
        } else {
            self.x_upper = u;
            self.f_upper = f_u;
        }

        if f_u <= f_w || w == z {
            self.v = w;
            self.f_v = f_w;
            self.w = u;
            self.f_w = f_u;
        } else if f_u <= f_v || v == z || v == w {
            self.v = u;
            self.f_v = f_u;
        }
        Ok(())
    }

    /// The current best estimate of the minimiser's location.
    pub fn x_minimum(&self) -> f64 {
        self.x_minimum
    }

    /// `f` at [`Self::x_minimum`].
    pub fn f_minimum(&self) -> f64 {
        self.f_minimum
    }

    /// The current lower bound of the bracket.
    pub fn x_lower(&self) -> f64 {
        self.x_lower
    }

    /// The current upper bound of the bracket.
    pub fn x_upper(&self) -> f64 {
        self.x_upper
    }

    /// `f` at the lower bound.
    pub fn f_lower(&self) -> f64 {
        self.f_lower
    }

    /// `f` at the upper bound.
    pub fn f_upper(&self) -> f64 {
        self.f_upper
    }

    /// The method this minimiser runs.
    pub fn method(&self) -> MinMethod {
        self.method
    }

    /// Iterate until the bracket is narrow enough, or `max_iter` steps elapse.
    ///
    /// # Errors
    ///
    /// [`PetirError::MaxIterations`] if the tolerance is not met in time —
    /// **which is what you get for asking for better than `sqrt(eps)`**; see
    /// the accuracy note in the module documentation before loosening
    /// anything else to chase it. Also propagates errors from
    /// [`Self::iterate`].
    pub fn solve(&mut self, epsabs: f64, epsrel: f64, max_iter: usize) -> Result<f64> {
        for _ in 0..max_iter {
            self.iterate()?;
            if test_interval(self.x_lower, self.x_upper, epsabs, epsrel)?.is_converged() {
                return Ok(self.x_minimum);
            }
        }
        Err(PetirError::MaxIterations)
    }
}

pub use crate::roots::Convergence;

/// Test whether a minimisation bracket is narrow enough.
///
/// Identical in form to [`crate::roots::test_interval`] — GSL keeps two copies
/// (`gsl_min_test_interval` and `gsl_root_test_interval`) with the same body,
/// and this port keeps them separately named for the same reason: a reader
/// following `min/` should find `min`'s test where upstream puts it.
///
/// # Errors
///
/// [`PetirError::Tolerance`] for a negative tolerance; [`PetirError::Invalid`]
/// if `x_lower > x_upper`.
pub fn test_interval(
    x_lower: f64,
    x_upper: f64,
    epsabs: f64,
    epsrel: f64,
) -> Result<Convergence> {
    crate::roots::test_interval(x_lower, x_upper, epsabs, epsrel)
}
