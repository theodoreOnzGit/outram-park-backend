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
