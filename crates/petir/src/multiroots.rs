// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PORTED from the GNU Scientific Library (GSL) 2.8, commit
// cf180cd7fbd06039a577f9c9ff0b428784765ac1, read 2026-11-01:
//
//   multiroots/fdjac.c    `gsl_multiroot_fdjacobian`
//                         Copyright (C) 1996, 1997, 1998, 1999, 2000, 2007
//                         Brian Gough
//   gsl_machine.h:18      `GSL_SQRT_DBL_EPSILON`
//
// GSL is GPL-3.0-or-later (verified from the per-file header); this port is
// distributed under GPL-3.0-only.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.

//! Multidimensional root finding — GSL's `multiroots/`, so far only its
//! forward-difference Jacobian.
//!
//! # What is here
//!
//! - [`fdjacobian`] — `gsl_multiroot_fdjacobian` (`multiroots/fdjac.c:23`):
//!   the dense forward-difference Jacobian `J_ij ~ (f_i(x + dx_j e_j) -
//!   f_i(x)) / dx_j` with `dx_j = epsrel * |x_j|` (or `epsrel` when
//!   `x_j == 0`).
//! - [`GSL_SQRT_DBL_EPSILON`] — the `epsrel` GSL's own solvers pass to it
//!   (`multiroots/hybrid.c`, `dnewton.c`).
//!
//! It is used by [`crate::ode::implicit`] (a finite-difference `df/dy` when
//! the caller has no analytic Jacobian) and by [`crate::newton_krylov`] (a
//! finite-difference approximate Jacobian for preconditioning).
//!
//! The GSL solvers themselves (`hybrid`, `hybridj`, `dnewton`, `gnewton`) are
//! **not** ported.
//!
//! # Units
//!
//! Bare `f64`, per the crate's units policy.

use alloc::vec::Vec;

use crate::linalg::Matrix;
use crate::{PetirError, Result};

/// `GSL_SQRT_DBL_EPSILON` (`gsl_machine.h:18`), the relative step GSL's
/// multiroot solvers hand to [`fdjacobian`].
pub const GSL_SQRT_DBL_EPSILON: f64 = 1.4901161193847656e-08;

/// What [`fdjacobian`] found besides the Jacobian itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FdJacobianReport {
    /// How many columns came out exactly zero.
    ///
    /// Upstream returns `GSL_ESING` when any column is null ("this may be due
    /// to dx being too small. Try increasing epsrel", `fdjac.c:91-95`), but
    /// only **after** filling every column. That is right for a root finder,
    /// where a null column makes the Newton matrix singular, and wrong for an
    /// ODE right-hand side, where a component that `f` does not depend on is
    /// ordinary (Robertson's `y3`, or every column at a zero state). So the
    /// count is reported instead of being turned into an error, and each
    /// caller decides: [`crate::ode::implicit`] ignores it; a root finder
    /// should treat `> 0` as GSL does.
    pub null_columns: usize,
}

/// Forward-difference Jacobian of `f` at `x` — a port of
/// `gsl_multiroot_fdjacobian` (`multiroots/fdjac.c:23-108`).
///
/// `f(x, out)` writes the `m` function values at the `n`-vector `x`; `fx` is
/// `f(x)`, already evaluated (as upstream takes it); `jac` is `m x n` and is
/// overwritten. Column `j` is `(f(x + dx e_j) - f(x)) / dx` with
/// `dx = epsrel * |x_j|`, or `dx = epsrel` when that is zero — upstream's rule
/// exactly, perturbing one component at a time and restoring it.
///
/// Pass [`GSL_SQRT_DBL_EPSILON`] for `epsrel` unless you know `f`'s noise
/// level; that is what GSL's solvers do.
///
/// # Errors
///
/// - [`PetirError::LengthMismatch`] if `fx.len() != jac.rows()` or
///   `x.len() != jac.cols()` (upstream's `GSL_EBADLEN`, "function and jacobian
///   are not conformant").
/// - [`PetirError::Domain`] if a perturbed evaluation is non-finite
///   (upstream's `GSL_EBADFUNC`, which it raises when the function reports a
///   failure; a PETIR closure cannot report one, so a non-finite value stands
///   in for it).
///
/// # Example
///
/// ```
/// use petir::linalg::Matrix;
/// use petir::multiroots::{fdjacobian, GSL_SQRT_DBL_EPSILON};
/// let f = |x: &[f64], out: &mut [f64]| {
///     out[0] = x[0] * x[0] + x[1];
///     out[1] = 3.0 * x[1];
/// };
/// let x = [2.0, 1.0];
/// let mut fx = [0.0; 2];
/// f(&x, &mut fx);
/// let mut j = Matrix::zeros(2, 2).unwrap();
/// let rep = fdjacobian(f, &x, &fx, GSL_SQRT_DBL_EPSILON, &mut j).unwrap();
/// assert_eq!(rep.null_columns, 0);
/// assert!((j.get(0, 0) - 4.0).abs() < 1e-6);
/// assert!((j.get(1, 1) - 3.0).abs() < 1e-6);
/// ```
pub fn fdjacobian<F>(
    f: F,
    x: &[f64],
    fx: &[f64],
    epsrel: f64,
    jac: &mut Matrix,
) -> Result<FdJacobianReport>
where
    F: Fn(&[f64], &mut [f64]),
{
    let n = x.len();
    let m = fx.len();
    if m != jac.rows() {
        return Err(PetirError::LengthMismatch {
            expected: jac.rows(),
            found: m,
        });
    }
    if n != jac.cols() {
        return Err(PetirError::LengthMismatch {
            expected: jac.cols(),
            found: n,
        });
    }

    let mut x1: Vec<f64> = x.to_vec();
    let mut f1: Vec<f64> = fx.to_vec();
    let mut null_columns = 0usize;

    for (j, &xj) in x.iter().enumerate() {
        let mut dx = epsrel * xj.abs();
        if dx == 0.0 {
            dx = epsrel;
        }

        if let Some(slot) = x1.get_mut(j) {
            *slot = xj + dx;
        }
        f(&x1, &mut f1);
        if f1.iter().any(|v| !v.is_finite()) {
            return Err(PetirError::Domain);
        }
        if let Some(slot) = x1.get_mut(j) {
            *slot = xj;
        }

        let mut null_col = true;
        for (i, (&g1, &g0)) in f1.iter().zip(fx.iter()).enumerate() {
            let d = (g1 - g0) / dx;
            if d != 0.0 {
                null_col = false;
            }
            jac.set(i, j, d);
        }
        if null_col {
            null_columns += 1;
        }
    }

    Ok(FdJacobianReport { null_columns })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_column_is_counted_not_fatal() {
        // f does not depend on x[1]: upstream would return GSL_ESING.
        let f = |x: &[f64], out: &mut [f64]| {
            out[0] = 2.0 * x[0];
            out[1] = -x[0];
        };
        let x = [1.0, 5.0];
        let mut fx = [0.0; 2];
        f(&x, &mut fx);
        let mut j = Matrix::zeros(2, 2).unwrap();
        let rep = fdjacobian(f, &x, &fx, GSL_SQRT_DBL_EPSILON, &mut j).unwrap();
        assert_eq!(rep.null_columns, 1);
        assert_eq!(j.get(0, 1), 0.0);
        assert!((j.get(0, 0) - 2.0).abs() < 1e-7);
    }

    #[test]
    fn zero_component_uses_epsrel_as_the_step() {
        // dx = epsrel at x_j = 0: the difference is exact for a linear f.
        let f = |x: &[f64], out: &mut [f64]| out[0] = 4.0 * x[0];
        let mut j = Matrix::zeros(1, 1).unwrap();
        fdjacobian(f, &[0.0], &[0.0], 0.5, &mut j).unwrap();
        assert_eq!(j.get(0, 0), 4.0);
    }

    #[test]
    fn non_conformant_shapes_are_rejected() {
        let f = |_x: &[f64], _out: &mut [f64]| {};
        let mut j = Matrix::zeros(2, 2).unwrap();
        assert!(matches!(
            fdjacobian(f, &[0.0; 3], &[0.0; 2], 1e-8, &mut j),
            Err(PetirError::LengthMismatch { .. })
        ));
    }
}
