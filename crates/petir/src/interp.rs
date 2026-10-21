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
//   interpolation/gsl_interp.h  gsl_interp_bsearch -> bracket_index
//   interpolation/linear.c      linear_eval / linear_eval_deriv
//   interpolation/cspline.c     cspline_init / cspline_eval / cspline_eval_deriv
//                               and its `coeff_calc` helper
//
// Added 2026-10-20 (gh:#989), same commit, read that day:
//   interpolation/interp.c      gsl_interp_eval_integ -> Interp::eval_integ
//                               (Copyright (C) 2007 Brian Gough;
//                                1996-2000 Gerard Jungman)
//   interpolation/linear.c      linear_eval_integ
//   interpolation/cspline.c     cspline_eval_integ
//   interpolation/integ_eval.h  integ_eval -> integ_eval
//                               (Copyright (C) 1996-2000 Gerard Jungman)

//! Interpolation of tabulated data — GSL's `interpolation/`.
//!
//! # What interpolation is for, and what it is not for
//!
//! You have `f` at a set of `x` values and want it in between. That is
//! interpolation. If instead your data are *noisy measurements* and you want a
//! smooth trend through them, you want **fitting**, not interpolation — an
//! interpolant is required to pass through every point exactly, so it faithfully
//! reproduces your noise as wiggles. Nothing here will warn you about that.
//!
//! # Choosing a method
//!
//! | Method | Continuity | Overshoots? | Cost to build |
//! |---|---|---|---|
//! | [`InterpMethod::Linear`] | value only (`C^0`) | never | none |
//! | [`InterpMethod::CubicSpline`] | value, slope, curvature (`C^2`) | **yes** | `O(n)` tridiagonal solve |
//!
//! [`InterpMethod::Linear`] is the safe default for data where *monotonicity
//! matters more than smoothness* — a lookup table of a physical property that
//! must never go negative, for instance. It cannot overshoot, because every
//! interpolated value lies between its two neighbours.
//!
//! [`InterpMethod::CubicSpline`] is smooth, which is what you want when the
//! derivative is meaningful (a rate from a tabulated quantity). The price is
//! real and worth stating plainly: **a cubic spline can overshoot its data.**
//! Through points that step sharply it will dip below the local minimum or rise
//! above the local maximum, so a spline through a strictly positive table can
//! return a negative value. If that would be a physical impossibility rather
//! than a small error, use [`InterpMethod::Linear`], or a
//! monotonicity-preserving scheme — GSL has Steffen's method for exactly this,
//! and PETIR has not ported it yet.
//!
//! # Boundary conditions
//!
//! The cubic spline here is the **natural** spline: second derivative zero at
//! both ends. That is GSL's `gsl_interp_cspline`. It is the right default when
//! nothing is known about the behaviour beyond the data, and the wrong one when
//! something is — a periodic signal wants the periodic variant, which is not
//! ported.
//!
//! **It costs an order of accuracy near the ends, and that is worth knowing.**
//! A cubic spline converges as `O(h^4)` in the interior, but the natural
//! boundary condition is simply wrong for any function whose curvature is
//! non-zero at the endpoints — `sin` at `x = 4` has curvature `-0.757`, not
//! zero — so inside a boundary layer the error falls only as `O(h^2)`, the same
//! rate as linear interpolation. Refining the knots does not fix it. Measured
//! in this module's tests: halving `h` cuts the interior error by about 16 and
//! the whole-interval error by only about 4.
//!
//! If the ends matter, either extend the data past the region you care about so
//! the boundary layer falls outside it, or use a spline whose end condition you
//! can set. PETIR has only the natural variant today.
//!
//! # Integrals
//!
//! [`Interp::eval_integ`] integrates the interpolant itself in closed form,
//! GSL's `gsl_interp_eval_integ`. Like GSL it refuses `a > b` and limits
//! outside the table. Verified bit-identical against compiled GSL on 566
//! values (`tests/gsl_interp_integ_code_to_code.rs`, 2026-10-20).
//!
//! # Extrapolation is refused, not guessed
//!
//! Evaluating outside `[x[0], x[n-1]]` returns [`PetirError::Domain`]. GSL does
//! the same. A polynomial fitted inside an interval says nothing about the
//! outside, and silently returning the nearest edge value — or the runaway
//! cubic — would be worse than an error the caller can see.
//!
//! # Units
//!
//! Bare dimensionless `f64`.

use alloc::vec;
use alloc::vec::Vec;

// Under a std-linked build (`cargo test`) f64's inherent abs shadows this
// trait, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::linalg::solve_symm_tridiag;
use crate::zip::zip_flat;
use crate::{PetirError, Result};

/// Which interpolation scheme an [`Interp`] uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterpMethod {
    /// Straight lines between neighbouring points. `C^0`, never overshoots.
    /// GSL `gsl_interp_linear`.
    Linear,
    /// Natural cubic spline: `C^2`, second derivative zero at both ends.
    /// GSL `gsl_interp_cspline`. **Can overshoot** — see the module docs.
    CubicSpline,
}

impl InterpMethod {
    /// The fewest points this method needs.
    ///
    /// Linear needs two; the cubic spline needs three, since with two there is
    /// no interior knot and no system to solve.
    pub fn min_size(self) -> usize {
        match self {
            InterpMethod::Linear => 2,
            InterpMethod::CubicSpline => 3,
        }
    }
}

/// An interpolant over tabulated `(x, y)` data.
///
/// Built once, evaluated many times. Construction does the expensive work —
/// for a cubic spline, the tridiagonal solve for the second derivatives — so
/// evaluating is `O(log n)` for the bracket search plus a constant.
///
/// # Example
///
/// ```
/// use petir::interp::{Interp, InterpMethod};
///
/// let x = [0.0, 1.0, 2.0, 3.0];
/// let y = [0.0, 1.0, 4.0, 9.0]; // x^2 at the knots
/// let s = Interp::new(InterpMethod::CubicSpline, &x, &y).unwrap();
///
/// // Through every knot exactly.
/// assert!((s.eval(2.0).unwrap() - 4.0).abs() < 1e-12);
/// // ...and close to x^2 between them.
/// assert!((s.eval(1.5).unwrap() - 2.25).abs() < 0.05);
/// ```
#[derive(Debug, Clone)]
pub struct Interp {
    method: InterpMethod,
    xa: Vec<f64>,
    ya: Vec<f64>,
    /// Cubic spline only: the `c` coefficients (half the second derivative) at
    /// each knot. Empty for linear.
    c: Vec<f64>,
}

impl Interp {
    /// Build an interpolant over `xa` (strictly increasing) and `ya`.
    ///
    /// # Errors
    ///
    /// - [`PetirError::LengthMismatch`] if `xa` and `ya` differ in length.
    /// - [`PetirError::Invalid`] if there are fewer points than the method
    ///   needs ([`InterpMethod::min_size`]).
    /// - [`PetirError::Domain`] if `xa` is not **strictly** increasing. Equal
    ///   or descending abscissae are rejected rather than sorted: a repeated
    ///   `x` has no single `y`, and silently reordering the caller's data would
    ///   hide a mistake in how it was assembled.
    pub fn new(method: InterpMethod, xa: &[f64], ya: &[f64]) -> Result<Self> {
        if xa.len() != ya.len() {
            return Err(PetirError::LengthMismatch {
                expected: xa.len(),
                found: ya.len(),
            });
        }
        if xa.len() < method.min_size() {
            return Err(PetirError::Invalid);
        }
        for (&lo, &hi) in zip_flat!(xa.iter(), xa.iter().skip(1)) {
            if !(lo < hi) {
                return Err(PetirError::Domain);
            }
        }

        let c = match method {
            InterpMethod::Linear => Vec::new(),
            InterpMethod::CubicSpline => Self::cspline_coefficients(xa, ya)?,
        };

        Ok(Self {
            method,
            xa: xa.to_vec(),
            ya: ya.to_vec(),
            c,
        })
    }

    /// Solve for the natural cubic spline's `c` coefficients.
    ///
    /// Translates GSL's `cspline_init`. The system is tridiagonal and
    /// symmetric, with `c[0] = c[n-1] = 0` imposed by the natural boundary
    /// condition and the interior values solved for.
    fn cspline_coefficients(xa: &[f64], ya: &[f64]) -> Result<Vec<f64>> {
        let num_points = xa.len();
        let max_index = num_points - 1; // Engeln-Mullges + Uhlig "n"
        let sys_size = max_index - 1; // the linear system is sys_size x sys_size

        // Natural boundary conditions: zero curvature at both ends. Upstream
        // assigns `c[0] = c[n-1] = 0` explicitly; here the vector is created
        // zeroed, so the two ends are already correct and the assignments --
        // and their subscripts -- are gone rather than rewritten.
        let mut c = vec![0.0_f64; num_points];

        let mut diag: Vec<f64> = Vec::with_capacity(sys_size);
        let mut offdiag: Vec<f64> = Vec::with_capacity(sys_size);
        let mut g: Vec<f64> = Vec::with_capacity(sys_size);

        // Upstream's `for (i = 0; i < sys_size; i++)` over `xa[i]`, `xa[i+1]`,
        // `xa[i+2]` and the matching `ya`. The shortest input is the
        // `skip(2)` pair, so the zip runs exactly `sys_size` times.
        for (&x_i, &x_ip1, &x_ip2, &y_i, &y_ip1, &y_ip2) in zip_flat!(
            xa.iter(),
            xa.iter().skip(1),
            xa.iter().skip(2),
            ya.iter(),
            ya.iter().skip(1),
            ya.iter().skip(2)
        ) {
            let h_i = x_ip1 - x_i;
            let h_ip1 = x_ip2 - x_ip1;
            let ydiff_i = y_ip1 - y_i;
            let ydiff_ip1 = y_ip2 - y_ip1;
            let g_i = if h_i != 0.0 { 1.0 / h_i } else { 0.0 };
            let g_ip1 = if h_ip1 != 0.0 { 1.0 / h_ip1 } else { 0.0 };
            offdiag.push(h_ip1);
            diag.push(2.0 * (h_ip1 + h_i));
            g.push(3.0 * (ydiff_ip1 * g_ip1 - ydiff_i * g_i));
        }

        if sys_size == 1 {
            // Three points: one interior knot, so the "system" is a division.
            let (Some(&d0), Some(&g0), Some(c1)) = (diag.first(), g.first(), c.get_mut(1)) else {
                return Err(PetirError::Invalid);
            };
            if d0 == 0.0 {
                return Err(PetirError::ZeroDivide);
            }
            *c1 = g0 / d0;
            return Ok(c);
        }

        // `offdiag` carries `sys_size` entries above, but the solver wants the
        // `sys_size - 1` that actually sit off the diagonal -- so drop the
        // last, which `split_last` does without a range subscript.
        let Some((_, offdiag_head)) = offdiag.split_last() else {
            return Err(PetirError::Invalid);
        };
        let solution = solve_symm_tridiag(&diag, offdiag_head, &g)?;

        // `c[1..=sys_size] = solution`. Written as a zip rather than
        // `copy_from_slice`, which panics on a length mismatch -- the lengths
        // do agree, but "they agree" is a fact about the solver, not something
        // the compiler checks here.
        for (slot, &v) in zip_flat!(c.iter_mut().skip(1), solution.iter()) {
            *slot = v;
        }
        Ok(c)
    }

    /// Binary search for the interval containing `x`.
    ///
    /// Returns `i` with `xa[i] <= x <= xa[i+1]`, always strictly less than
    /// `n - 1` so that `i + 1` is in range. Translates GSL's
    /// `gsl_interp_bsearch`.
    fn bracket_index(&self, x: f64) -> usize {
        let mut ilo = 0usize;
        let mut ihi = self.xa.len().saturating_sub(1);
        while ihi > ilo + 1 {
            let i = (ihi + ilo) / 2;
            // `i` is strictly between `ilo` and `ihi`, so it is in range; the
            // `None` arm exists so the compiler need not be convinced of that.
            let Some(&x_i) = self.xa.get(i) else {
                break;
            };
            if x_i > x {
                ihi = i;
            } else {
                ilo = i;
            }
        }
        ilo
    }

    /// Reject an out-of-range `x` before any indexing happens.
    fn check_range(&self, x: f64) -> Result<()> {
        let (Some(&lo), Some(&hi)) = (self.xa.first(), self.xa.last()) else {
            // An interpolant with no knots cannot be built (`min_size` is at
            // least 2), so this is unreachable -- but reporting beats indexing.
            return Err(PetirError::Invalid);
        };
        if !(x >= lo && x <= hi) {
            // Also catches NaN, which compares false against everything.
            return Err(PetirError::Domain);
        }
        Ok(())
    }

    /// The cubic-spline coefficients `(b, c, d)` on the interval starting at
    /// `index`. Translates GSL's `coeff_calc`.
    fn coeff_calc(&self, dy: f64, dx: f64, index: usize) -> Result<(f64, f64, f64)> {
        let (Some(&c_i), Some(&c_ip1)) = (self.c.get(index), self.c.get(index + 1)) else {
            // `bracket_index` guarantees `index + 1 < n` and `c` has `n`
            // entries whenever the method is CubicSpline, so this cannot fire.
            return Err(PetirError::Invalid);
        };
        let b = (dy / dx) - dx * (c_ip1 + 2.0 * c_i) / 3.0;
        let c = c_i;
        let d = (c_ip1 - c_i) / (3.0 * dx);
        Ok((b, c, d))
    }

    /// The knot pair bracketing `x`, with the index of its left end.
    ///
    /// Factored out of [`Self::eval`] and [`Self::eval_deriv`], which differ
    /// only in what they do with the segment once they have it.
    fn segment(&self, x: f64) -> Result<(usize, f64, f64, f64, f64)> {
        self.check_range(x)?;
        let index = self.bracket_index(x);
        let (Some(&x_lo), Some(&x_hi), Some(&y_lo), Some(&y_hi)) = (
            self.xa.get(index),
            self.xa.get(index + 1),
            self.ya.get(index),
            self.ya.get(index + 1),
        ) else {
            return Err(PetirError::Invalid);
        };
        Ok((index, x_lo, x_hi, y_lo, y_hi))
    }

    /// Interpolate at `x`.
    ///
    /// # Errors
    ///
    /// [`PetirError::Domain`] if `x` lies outside `[x[0], x[n-1]]`, or is
    /// `NaN`. Extrapolation is refused — see the module docs.
    ///
    /// # Example
    ///
    /// ```
    /// use petir::interp::{Interp, InterpMethod};
    /// let s = Interp::new(InterpMethod::Linear, &[0.0, 1.0], &[0.0, 10.0]).unwrap();
    /// assert!((s.eval(0.25).unwrap() - 2.5).abs() < 1e-12);
    /// assert!(s.eval(2.0).is_err(), "extrapolation is refused");
    /// ```
    pub fn eval(&self, x: f64) -> Result<f64> {
        let (index, x_lo, x_hi, y_lo, y_hi) = self.segment(x)?;
        let dx = x_hi - x_lo;
        if !(dx > 0.0) {
            return Err(PetirError::Invalid);
        }

        match self.method {
            InterpMethod::Linear => Ok(y_lo + (x - x_lo) / dx * (y_hi - y_lo)),
            InterpMethod::CubicSpline => {
                let dy = y_hi - y_lo;
                let delx = x - x_lo;
                let (b_i, c_i, d_i) = self.coeff_calc(dy, dx, index)?;
                Ok(y_lo + delx * (b_i + delx * (c_i + delx * d_i)))
            }
        }
    }

    /// The derivative of the interpolant at `x`.
    ///
    /// # A warning about the linear derivative
    ///
    /// For [`InterpMethod::Linear`] this is the slope of the containing
    /// segment, which is **piecewise constant and discontinuous at every
    /// knot**. It is the derivative of the interpolant, not an estimate of the
    /// derivative of the underlying function, and it is a poor one. If the
    /// derivative is what you actually want, use [`InterpMethod::CubicSpline`]
    /// (whose derivative is continuous by construction) or differentiate the
    /// underlying function directly with [`crate::deriv`].
    ///
    /// # Errors
    ///
    /// As [`Self::eval`].
    pub fn eval_deriv(&self, x: f64) -> Result<f64> {
        let (index, x_lo, x_hi, y_lo, y_hi) = self.segment(x)?;
        let dx = x_hi - x_lo;
        if !(dx > 0.0) {
            return Err(PetirError::Invalid);
        }

        match self.method {
            InterpMethod::Linear => Ok((y_hi - y_lo) / dx),
            InterpMethod::CubicSpline => {
                let dy = y_hi - y_lo;
                let delx = x - x_lo;
                let (b_i, c_i, d_i) = self.coeff_calc(dy, dx, index)?;
                Ok(b_i + delx * (2.0 * c_i + 3.0 * d_i * delx))
            }
        }
    }

    /// The definite integral of the interpolant from `a` to `b`.
    ///
    /// Translates GSL 2.8's `gsl_interp_eval_integ` (`interpolation/interp.c`
    /// lines 255-278) and the two per-method kernels it dispatches to:
    /// `linear_eval_integ` (`interpolation/linear.c` lines 135-183) and
    /// `cspline_eval_integ` (`interpolation/cspline.c` lines 389-443), the
    /// latter through `integ_eval` (`interpolation/integ_eval.h` lines
    /// 24-36). Read 2026-10-20 at the commit named in the file header.
    ///
    /// The integral is **exact for the interpolant** — it integrates the
    /// piecewise line or piecewise cubic in closed form, segment by segment.
    /// It is not a quadrature of the underlying function, so it carries the
    /// interpolant's own error (including the cubic spline's overshoot) and
    /// nothing more.
    ///
    /// # Order of the limits
    ///
    /// GSL refuses `a > b` with `GSL_EDOM` rather than returning the negated
    /// integral, and so does this. `a == b` returns exactly `0.0`, as GSL
    /// does, without touching the knots. A caller that wants the signed
    /// integral swaps the limits and negates.
    ///
    /// # Errors
    ///
    /// - [`PetirError::Domain`] if `a > b`, if either limit lies outside
    ///   `[x[0], x[n-1]]`, or if either is `NaN`. Extrapolation is refused —
    ///   see the module docs. **Deviation, NaN only:** GSL's guard
    ///   (`a > b || a < xmin || b > xmax`) is false for a `NaN` limit, so GSL
    ///   lets it through and returns a `NaN` integral; here it is a `Domain`
    ///   error, the same choice [`Self::eval`] already makes.
    /// - [`PetirError::Invalid`] where `cspline_eval_integ` returns
    ///   `GSL_EINVAL` for a zero-width segment. [`Self::new`] rejects repeated
    ///   abscissae, so this cannot be reached through the public API.
    ///
    /// # Example
    ///
    /// ```
    /// use petir::interp::{Interp, InterpMethod};
    /// // A linear interpolant of y = 2x is exact, so the integral is x^2.
    /// let s = Interp::new(InterpMethod::Linear, &[0.0, 1.0, 3.0], &[0.0, 2.0, 6.0]).unwrap();
    /// assert!((s.eval_integ(0.5, 2.0).unwrap() - 3.75).abs() < 1e-14);
    /// assert_eq!(s.eval_integ(1.0, 1.0).unwrap(), 0.0);
    /// assert!(s.eval_integ(2.0, 0.5).is_err(), "a > b is refused, as in GSL");
    /// assert!(s.eval_integ(0.0, 4.0).is_err(), "extrapolation is refused");
    /// ```
    pub fn eval_integ(&self, a: f64, b: f64) -> Result<f64> {
        // gsl_interp_eval_integ's guard. check_range additionally rejects NaN.
        self.check_range(a)?;
        self.check_range(b)?;
        if a > b {
            return Err(PetirError::Domain);
        }
        if a == b {
            return Ok(0.0);
        }

        // Both kernels: `index_a = gsl_interp_bsearch(x_array, a, 0, size-1)`
        // and likewise for `b` (the accelerator, when present, finds the same
        // index, so it does not change the answer).
        let index_a = self.bracket_index(a);
        let index_b = self.bracket_index(b);

        let mut result = 0.0_f64;
        // `for (i = index_a; i <= index_b; i++)`, reading x[i], x[i+1],
        // y[i], y[i+1]. bracket_index returns at most n-2, so `i + 1` is in
        // range; `get` keeps that a checked fact rather than a subscript.
        for i in index_a..=index_b {
            let (Some(&x_lo), Some(&x_hi), Some(&y_lo), Some(&y_hi)) = (
                self.xa.get(i),
                self.xa.get(i + 1),
                self.ya.get(i),
                self.ya.get(i + 1),
            ) else {
                return Err(PetirError::Invalid);
            };
            let dx = x_hi - x_lo;

            match self.method {
                InterpMethod::Linear => {
                    // linear.c skips a zero-width segment silently.
                    if dx != 0.0 {
                        if i == index_a || i == index_b {
                            let x1 = if i == index_a { a } else { x_lo };
                            let x2 = if i == index_b { b } else { x_hi };
                            let d = (y_hi - y_lo) / dx;
                            result += (x2 - x1) * (y_lo + 0.5 * d * ((x2 - x_lo) + (x1 - x_lo)));
                        } else {
                            result += 0.5 * dx * (y_lo + y_hi);
                        }
                    }
                }
                InterpMethod::CubicSpline => {
                    let dy = y_hi - y_lo;
                    if dx != 0.0 {
                        let (b_i, c_i, d_i) = self.coeff_calc(dy, dx, i)?;
                        if i == index_a || i == index_b {
                            let x1 = if i == index_a { a } else { x_lo };
                            let x2 = if i == index_b { b } else { x_hi };
                            result += integ_eval(y_lo, b_i, c_i, d_i, x_lo, x1, x2);
                        } else {
                            result +=
                                dx * (y_lo + dx * (0.5 * b_i + dx * (c_i / 3.0 + 0.25 * d_i * dx)));
                        }
                    } else {
                        // cspline.c: `*result = 0.0; return GSL_EINVAL;`
                        return Err(PetirError::Invalid);
                    }
                }
            }
        }
        Ok(result)
    }

    /// The method this interpolant uses.
    pub fn method(&self) -> InterpMethod {
        self.method
    }

    /// The abscissae the interpolant was built on.
    pub fn xa(&self) -> &[f64] {
        &self.xa
    }

    /// The ordinates the interpolant was built on.
    pub fn ya(&self) -> &[f64] {
        &self.ya
    }

    /// The lowest `x` the interpolant is defined at.
    ///
    /// `NaN` only for a knot-less interpolant, which [`Self::new`] refuses to
    /// build -- so in practice this is always a real abscissa.
    pub fn x_min(&self) -> f64 {
        self.xa.first().copied().unwrap_or(f64::NAN)
    }

    /// The highest `x` the interpolant is defined at. `NaN` under the same
    /// unreachable condition as [`Self::x_min`].
    pub fn x_max(&self) -> f64 {
        self.xa.last().copied().unwrap_or(f64::NAN)
    }
}

/// The integral of one cubic piece `a + b r + c r^2 + d r^3`, `r = x - xi`,
/// from `x = a` to `x = b`.
///
/// Translates GSL's `integ_eval` (`interpolation/integ_eval.h` lines 24-36)
/// term for term; the grouping is upstream's and is kept so the rounding
/// matches compiled GSL.
fn integ_eval(ai: f64, bi: f64, ci: f64, di: f64, xi: f64, a: f64, b: f64) -> f64 {
    let r1 = a - xi;
    let r2 = b - xi;
    let r12 = r1 + r2;
    let bterm = 0.5 * bi * r12;
    let cterm = (1.0 / 3.0) * ci * (r1 * r1 + r2 * r2 + r1 * r2);
    let dterm = 0.25 * di * r12 * (r1 * r1 + r2 * r2);
    (b - a) * (ai + bterm + cterm + dterm)
}
