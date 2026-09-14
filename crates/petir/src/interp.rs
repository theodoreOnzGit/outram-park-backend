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
        for w in xa.windows(2) {
            if !(w[0] < w[1]) {
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

        let mut c = vec![0.0_f64; num_points];
        // Natural boundary conditions: zero curvature at both ends.
        c[0] = 0.0;
        c[max_index] = 0.0;

        let mut diag = vec![0.0_f64; sys_size];
        let mut offdiag = vec![0.0_f64; sys_size];
        let mut g = vec![0.0_f64; sys_size];

        for i in 0..sys_size {
            let h_i = xa[i + 1] - xa[i];
            let h_ip1 = xa[i + 2] - xa[i + 1];
            let ydiff_i = ya[i + 1] - ya[i];
            let ydiff_ip1 = ya[i + 2] - ya[i + 1];
            let g_i = if h_i != 0.0 { 1.0 / h_i } else { 0.0 };
            let g_ip1 = if h_ip1 != 0.0 { 1.0 / h_ip1 } else { 0.0 };
            offdiag[i] = h_ip1;
            diag[i] = 2.0 * (h_ip1 + h_i);
            g[i] = 3.0 * (ydiff_ip1 * g_ip1 - ydiff_i * g_i);
        }

        if sys_size == 1 {
            // Three points: one interior knot, so the "system" is a division.
            if diag[0] == 0.0 {
                return Err(PetirError::ZeroDivide);
            }
            c[1] = g[0] / diag[0];
            return Ok(c);
        }

        // offdiag carries sys_size entries above, but the solver wants the
        // sys_size - 1 that actually sit off the diagonal.
        let solution = solve_symm_tridiag(&diag, &offdiag[..sys_size - 1], &g)?;
        c[1..=sys_size].copy_from_slice(&solution);
        Ok(c)
    }

    /// Binary search for the interval containing `x`.
    ///
    /// Returns `i` with `xa[i] <= x <= xa[i+1]`, always strictly less than
    /// `n - 1` so that `i + 1` is in range. Translates GSL's
    /// `gsl_interp_bsearch`.
    fn bracket_index(&self, x: f64) -> usize {
        let mut ilo = 0usize;
        let mut ihi = self.xa.len() - 1;
        while ihi > ilo + 1 {
            let i = (ihi + ilo) / 2;
            if self.xa[i] > x {
                ihi = i;
            } else {
                ilo = i;
            }
        }
        ilo
    }

    /// Reject an out-of-range `x` before any indexing happens.
    fn check_range(&self, x: f64) -> Result<()> {
        let lo = self.xa[0];
        let hi = self.xa[self.xa.len() - 1];
        if !(x >= lo && x <= hi) {
            // Also catches NaN, which compares false against everything.
            return Err(PetirError::Domain);
        }
        Ok(())
    }

    /// The cubic-spline coefficients `(b, c, d)` on the interval starting at
    /// `index`. Translates GSL's `coeff_calc`.
    fn coeff_calc(&self, dy: f64, dx: f64, index: usize) -> (f64, f64, f64) {
        let c_i = self.c[index];
        let c_ip1 = self.c[index + 1];
        let b = (dy / dx) - dx * (c_ip1 + 2.0 * c_i) / 3.0;
        let c = c_i;
        let d = (c_ip1 - c_i) / (3.0 * dx);
        (b, c, d)
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
        self.check_range(x)?;
        let index = self.bracket_index(x);

        let x_lo = self.xa[index];
        let x_hi = self.xa[index + 1];
        let y_lo = self.ya[index];
        let y_hi = self.ya[index + 1];
        let dx = x_hi - x_lo;
        if !(dx > 0.0) {
            return Err(PetirError::Invalid);
        }

        match self.method {
            InterpMethod::Linear => Ok(y_lo + (x - x_lo) / dx * (y_hi - y_lo)),
            InterpMethod::CubicSpline => {
                let dy = y_hi - y_lo;
                let delx = x - x_lo;
                let (b_i, c_i, d_i) = self.coeff_calc(dy, dx, index);
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
        self.check_range(x)?;
        let index = self.bracket_index(x);

        let x_lo = self.xa[index];
        let x_hi = self.xa[index + 1];
        let y_lo = self.ya[index];
        let y_hi = self.ya[index + 1];
        let dx = x_hi - x_lo;
        if !(dx > 0.0) {
            return Err(PetirError::Invalid);
        }

        match self.method {
            InterpMethod::Linear => Ok((y_hi - y_lo) / dx),
            InterpMethod::CubicSpline => {
                let dy = y_hi - y_lo;
                let delx = x - x_lo;
                let (b_i, c_i, d_i) = self.coeff_calc(dy, dx, index);
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
    pub fn x_min(&self) -> f64 {
        self.xa[0]
    }

    /// The highest `x` the interpolant is defined at.
    pub fn x_max(&self) -> f64 {
        self.xa[self.xa.len() - 1]
    }
}
