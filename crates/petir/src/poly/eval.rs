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
// Translated from the GNU Scientific Library (GSL)
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996-2024 The GSL Team (Brian Gough, Gerard Jungman, et al.)
// GSL is free software under the GNU General Public License, version 3 or
// later. Upstream files: poly/eval.c, poly/dd.c
//   - `gsl_poly_eval`             -> `eval`
//   - `gsl_poly_eval_derivs`      -> `eval_derivs`
//   - `gsl_poly_dd_init`          -> `DividedDifference::new`
//   - `gsl_poly_dd_eval`          -> `DividedDifference::eval`
//   - `gsl_poly_dd_taylor`        -> `DividedDifference::to_taylor`

//! General-degree polynomial evaluation — GSL's `poly/` layer.
//!
//! # Contents
//!
//! - [`eval`] — Horner evaluation of `c[0] + c[1] x + ... + c[n-1] x^(n-1)`.
//! - [`eval_derivs`] — the value *and* the first `k` derivatives in one sweep,
//!   at the cost of one Horner pass each.
//! - [`DividedDifference`] — Newton's divided-difference interpolating
//!   polynomial through a set of points, with conversion back to an ordinary
//!   Taylor (monomial) coefficient list.
//!
//! # Coefficient order
//!
//! **Ascending**: `c[i]` multiplies `x^i`, matching GSL. This is the opposite
//! of GNU Octave's convention, where `polyval` takes coefficients in
//! *descending* powers. Both appear in this crate — the Octave-facing layer in
//! [`crate::octave`] keeps Octave's order deliberately, so that a translated
//! `.m` script reads the same way it did upstream. Mixing the two silently
//! reverses the polynomial, so each function says which it takes.
//!
//! # Units
//!
//! Bare dimensionless `f64` throughout; see [`crate::poly`] for why a
//! coefficient vector cannot carry a single `uom` type.

use crate::zip::zip_flat;
use alloc::vec;
use alloc::vec::Vec;

/// Evaluate a polynomial by Horner's rule.
///
/// `c` holds the coefficients in **ascending** powers: `c[0]` is the constant
/// term and `c[i]` multiplies `x^i`. An empty `c` evaluates to `0.0`, which is
/// the correct value of the empty sum.
///
/// # Why Horner
///
/// It is both the cheapest evaluation (`n` multiplications and `n` additions)
/// and the most accurate of the elementary schemes: forming `x^i` separately
/// and summing costs more and loses more, because the large intermediate powers
/// are rounded before they are scaled back down.
///
/// Translates GSL's `gsl_poly_eval`.
///
/// # Example
///
/// ```
/// use petir::poly::eval;
/// // 1 + 2x + 3x^2 at x = 2  ->  1 + 4 + 12 = 17
/// assert_eq!(eval(&[1.0, 2.0, 3.0], 2.0), 17.0);
/// assert_eq!(eval(&[], 5.0), 0.0);
/// ```
pub fn eval(c: &[f64], x: f64) -> f64 {
    let mut acc = 0.0_f64;
    for &ci in c.iter().rev() {
        acc = acc * x + ci;
    }
    acc
}

/// Evaluate a polynomial and its first `n_derivs` derivatives at `x`.
///
/// Returns a vector of length `n_derivs + 1`: element `0` is the value, element
/// `k` is the `k`-th derivative. `c` is in **ascending** powers.
///
/// # Why this rather than differentiating and re-evaluating
///
/// Repeated Horner on the *same* coefficient array yields all the derivatives
/// as a by-product, with no separate derivative coefficient arrays to build and
/// no extra rounding: the synthetic-division remainders of successive passes
/// are exactly the Taylor coefficients at `x`, which are the derivatives up to
/// the factorials restored at the end.
///
/// Requesting more derivatives than the polynomial's degree is not an error —
/// the extra entries are zero, which is what they mathematically are.
///
/// Translates GSL's `gsl_poly_eval_derivs`.
///
/// # Example
///
/// ```
/// use petir::poly::eval_derivs;
/// // p(x) = 1 + 2x + 3x^2 ;  p(2) = 17, p'(2) = 2 + 12 = 14, p''(2) = 6
/// let d = eval_derivs(&[1.0, 2.0, 3.0], 2.0, 3);
/// assert_eq!(d[0], 17.0);
/// assert_eq!(d[1], 14.0);
/// assert_eq!(d[2], 6.0);
/// assert_eq!(d[3], 0.0);
/// ```
pub fn eval_derivs(c: &[f64], x: f64, n_derivs: usize) -> Vec<f64> {
    let mut res = vec![0.0_f64; n_derivs + 1];
    let lenc = c.len();

    for (n, slot) in res.iter_mut().enumerate() {
        if n >= lenc {
            // Every derivative past the degree is identically zero.
            break;
        }
        // Horner on the n-th derivative's coefficient list, formed on the fly:
        // the coefficient of x^(i-n) in p^(n) is c[i] * i!/(i-n)!.
        let mut acc = 0.0_f64;
        // `n < lenc` was just established, so the tail always exists; pairing
        // it with its own index range keeps `i` available for the falling
        // factorial without a subscript.
        let Some(tail) = c.get(n..) else {
            break;
        };
        for (i, &ci) in (n..lenc).zip(tail.iter()).rev() {
            // Falling factorial i (i-1) ... (i-n+1).
            let mut fact = 1.0_f64;
            for j in 0..n {
                fact *= (i - j) as f64;
            }
            acc = acc * x + ci * fact;
        }
        *slot = acc;
    }
    res
}

/// Newton's divided-difference form of the interpolating polynomial.
///
/// # What it is
///
/// Given `n` distinct abscissae `xa[i]` and values `ya[i]`, there is exactly
/// one polynomial of degree `< n` through them. This type holds that polynomial
/// in Newton form,
///
/// `p(x) = d[0] + d[1](x - xa[0]) + d[2](x - xa[0])(x - xa[1]) + ...`
///
/// where the `d[k]` are the divided differences. Storing it this way rather
/// than as monomial coefficients is what makes it numerically usable: the
/// monomial form of a high-degree interpolant is catastrophically
/// ill-conditioned, while the Newton form evaluates stably by Horner.
///
/// # Cost
///
/// `O(n^2)` to build, `O(n)` per evaluation.
///
/// # A warning that belongs on every interpolating polynomial
///
/// Interpolating many points by a single polynomial is usually the wrong idea
/// — equally spaced nodes produce Runge's phenomenon, with the error near the
/// interval ends growing without bound as `n` increases. For more than a
/// handful of points reach for [`crate::interp`]'s splines instead. This type
/// exists because GSL has it, because it is exactly right for a small number of
/// points, and because it is the engine behind polynomial interpolation there.
///
/// Translates GSL's `gsl_poly_dd_init` / `gsl_poly_dd_eval` / `gsl_poly_dd_taylor`.
///
/// # Example
///
/// ```
/// use petir::poly::DividedDifference;
/// // Through (0,1), (1,3), (2,7): the quadratic 1 + x + x^2.
/// let dd = DividedDifference::new(&[0.0, 1.0, 2.0], &[1.0, 3.0, 7.0]).unwrap();
/// assert!((dd.eval(3.0) - 13.0).abs() < 1e-12);
/// let taylor = dd.to_taylor(0.0);
/// assert!((taylor[0] - 1.0).abs() < 1e-12);
/// assert!((taylor[1] - 1.0).abs() < 1e-12);
/// assert!((taylor[2] - 1.0).abs() < 1e-12);
/// ```
#[derive(Debug, Clone)]
pub struct DividedDifference {
    /// Divided-difference coefficients `d[0..n]`.
    d: Vec<f64>,
    /// The abscissae the coefficients were built against, needed to evaluate.
    xa: Vec<f64>,
}

impl DividedDifference {
    /// Build the divided-difference table for the points `(xa[i], ya[i])`.
    ///
    /// # Errors
    ///
    /// - [`crate::PetirError::LengthMismatch`] if `xa` and `ya` differ in length.
    /// - [`crate::PetirError::Invalid`] if fewer than one point is supplied.
    /// - [`crate::PetirError::ZeroDivide`] if two abscissae coincide — the
    ///   interpolating polynomial through a repeated `x` is not defined, and the
    ///   table construction would divide by zero. Reported rather than allowed
    ///   to produce a silent `inf`.
    pub fn new(xa: &[f64], ya: &[f64]) -> crate::Result<Self> {
        if xa.len() != ya.len() {
            return Err(crate::PetirError::LengthMismatch {
                expected: xa.len(),
                found: ya.len(),
            });
        }
        let n = xa.len();
        if n == 0 {
            return Err(crate::PetirError::Invalid);
        }
        let mut d: Vec<f64> = ya.to_vec();
        // In-place Neville-style accumulation, as GSL's gsl_poly_dd_init does.
        //
        // Upstream's inner loop is `for (i = n-1; i >= k; i--)`, reading
        // `d[i-1]` -- the value the descending order has not yet overwritten.
        // `split_last_mut` walks exactly that: `last` is `d[i]` and
        // `head.last()` is `d[i-1]`, still untouched. The `xa[i] - xa[i-k]`
        // differences ride alongside as a reversed zip of `xa[k..]` with
        // `xa[..n-k]`, which is the same pairing written without subscripts.
        for k in 1..n {
            let (Some(xa_hi), Some(xa_lo)) = (xa.get(k..), xa.get(..n - k)) else {
                return Err(crate::PetirError::Invalid);
            };
            let mut dx_pairs = zip_flat!(xa_hi.iter(), xa_lo.iter()).rev();
            let Some(mut rest) = d.get_mut(k - 1..) else {
                return Err(crate::PetirError::Invalid);
            };
            while let Some((last, head)) = rest.split_last_mut() {
                // An empty head means `last` is `d[k-1]`, which this round
                // does not touch -- upstream's `i >= k` bound.
                let Some(&prev) = head.last() else {
                    break;
                };
                let Some((&x_hi, &x_lo)) = dx_pairs.next() else {
                    break;
                };
                let dx = x_hi - x_lo;
                if dx == 0.0 {
                    return Err(crate::PetirError::ZeroDivide);
                }
                *last = (*last - prev) / dx;
                rest = head;
            }
        }
        Ok(Self { d, xa: xa.to_vec() })
    }

    /// Evaluate the interpolating polynomial at `x`, by Horner in Newton form.
    ///
    /// Translates GSL's `gsl_poly_dd_eval`.
    pub fn eval(&self, x: f64) -> f64 {
        // `new` rejects an empty point set, so `d` is never empty here; an
        // empty table would be the zero polynomial, which is what is returned.
        let Some((&d_last, d_head)) = self.d.split_last() else {
            return 0.0;
        };
        let mut y = d_last;
        // Upstream's `for (i = n-2; i >= 0; i--)` over `d[i]` and `xa[i]`.
        for (&d_i, &xa_i) in zip_flat!(d_head.iter(), self.xa.iter()).rev() {
            y = d_i + (x - xa_i) * y;
        }
        y
    }

    /// Convert to ordinary monomial coefficients about the point `xp`.
    ///
    /// The returned vector `c` is in **ascending** powers of `(x - xp)`, so
    /// `to_taylor(0.0)` gives the plain coefficients of `x`.
    ///
    /// # When not to use this
    ///
    /// The monomial form is what the Newton form exists to avoid. For a
    /// low-degree polynomial it is convenient and harmless; past degree 10 or
    /// so on a wide interval, the conversion itself loses accuracy that
    /// [`Self::eval`] would have kept. Convert for display and for handing to
    /// code that demands coefficients, not to evaluate.
    ///
    /// # Relation to GSL
    ///
    /// Computes the same thing as `gsl_poly_dd_taylor`, but written as a
    /// direct accumulation of `d[k] * prod(u + (xp - xa[j]))` rather than as a
    /// transcription of upstream's reversed-index in-place loop. The result is
    /// identical; the reason for departing from a line-by-line translation here
    /// is that upstream's index arithmetic is write-only code, and a
    /// mis-transcribed index would produce a plausible-looking wrong polynomial
    /// rather than an obvious failure. The form below can be read against the
    /// definition of the Newton form directly, and the tests check it against
    /// both the interpolated nodes and `Self::eval`.
    pub fn to_taylor(&self, xp: f64) -> Vec<f64> {
        let n = self.d.len();
        let mut c = vec![0.0_f64; n];
        // `prod` holds the running product
        //     (u + (xp - xa[0])) ... (u + (xp - xa[k-1]))
        // in ascending powers of u = x - xp. Starting from the degree-zero
        // polynomial 1, each step accumulates d[k] * prod into c and then
        // advances prod by one more factor.
        let mut prod = vec![0.0_f64; n];
        if let Some(p0) = prod.first_mut() {
            *p0 = 1.0;
        }
        for (k, (&d_k, &xa_k)) in zip_flat!(self.d.iter(), self.xa.iter()).enumerate() {
            for (c_i, &p_i) in zip_flat!(c.iter_mut(), prod.iter()).take(k + 1) {
                *c_i += d_k * p_i;
            }
            if k + 1 < n {
                // Multiply prod by (u + s), descending so each slot still
                // holds its old value when it is read -- the same
                // `split_last_mut` walk as the divided-difference table above.
                let s = xp - xa_k;
                let Some(mut rest) = prod.get_mut(..=k + 1) else {
                    break;
                };
                while let Some((last, head)) = rest.split_last_mut() {
                    let Some(&prev) = head.last() else {
                        break;
                    };
                    *last = prev + s * *last;
                    rest = head;
                }
                if let Some(p0) = prod.first_mut() {
                    *p0 *= s;
                }
            }
        }
        c
    }

    /// The divided-difference coefficients `d[k]`.
    pub fn coefficients(&self) -> &[f64] {
        &self.d
    }

    /// The abscissae the table was built from.
    pub fn abscissae(&self) -> &[f64] {
        &self.xa
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horner_matches_the_direct_sum_on_well_scaled_input() {
        let c = [1.5, -2.0, 0.25, 3.0];
        for &x in &[-2.0_f64, -0.5, 0.0, 0.5, 2.0] {
            let direct = c[0] + c[1] * x + c[2] * x * x + c[3] * x * x * x;
            assert!((eval(&c, x) - direct).abs() < 1e-14);
        }
    }

    #[test]
    fn empty_and_constant_polynomials() {
        assert_eq!(eval(&[], 3.0), 0.0);
        assert_eq!(eval(&[7.0], 3.0), 7.0);
        assert_eq!(eval(&[7.0], -1e30), 7.0);
    }

    #[test]
    fn eval_derivs_reproduces_hand_differentiated_values() {
        // p(x) = 2 - 3x + 4x^2 - 5x^3
        let c = [2.0, -3.0, 4.0, -5.0];
        let x = 1.5_f64;
        let d = eval_derivs(&c, x, 4);
        let p = 2.0 - 3.0 * x + 4.0 * x * x - 5.0 * x * x * x;
        let p1 = -3.0 + 8.0 * x - 15.0 * x * x;
        let p2 = 8.0 - 30.0 * x;
        let p3 = -30.0;
        assert!((d[0] - p).abs() < 1e-13);
        assert!((d[1] - p1).abs() < 1e-13);
        assert!((d[2] - p2).abs() < 1e-13);
        assert!((d[3] - p3).abs() < 1e-13);
        assert_eq!(d[4], 0.0, "derivative past the degree must vanish");
    }

    #[test]
    fn eval_derivs_zeroth_entry_is_just_eval() {
        let c = [0.3, 1.1, -2.2, 0.7, 5.0];
        for &x in &[-3.0_f64, 0.0, 1.0, 4.0] {
            assert!((eval_derivs(&c, x, 0)[0] - eval(&c, x)).abs() < 1e-14);
        }
    }

    #[test]
    fn divided_difference_passes_through_every_node() {
        let xa = [0.0, 1.0, 2.0, 4.0, 7.0];
        let ya = [1.0, 3.0, 7.0, 21.0, 57.0];
        let dd = DividedDifference::new(&xa, &ya).unwrap();
        for (&x, &y) in xa.iter().zip(ya.iter()) {
            assert!(
                (dd.eval(x) - y).abs() < 1e-11,
                "node ({x}, {y}) not interpolated"
            );
        }
    }

    #[test]
    fn divided_difference_reproduces_a_known_quadratic() {
        // 1 + x + x^2 sampled at three points must come back exactly.
        let xa = [0.0, 1.0, 2.0];
        let ya = [1.0, 3.0, 7.0];
        let dd = DividedDifference::new(&xa, &ya).unwrap();
        for &x in &[-3.0_f64, 0.5, 3.0, 10.0] {
            let exact = 1.0 + x + x * x;
            assert!((dd.eval(x) - exact).abs() < 1e-11, "x={x}");
        }
    }

    #[test]
    fn to_taylor_recovers_the_monomial_coefficients() {
        let xa = [0.0, 1.0, 2.0];
        let ya = [1.0, 3.0, 7.0];
        let dd = DividedDifference::new(&xa, &ya).unwrap();
        let c = dd.to_taylor(0.0);
        assert_eq!(c.len(), 3);
        for (i, &want) in [1.0, 1.0, 1.0].iter().enumerate() {
            assert!((c[i] - want).abs() < 1e-12, "coefficient {i} = {}", c[i]);
        }
        // And the recovered coefficients must evaluate to the same polynomial.
        for &x in &[-2.0_f64, 0.7, 5.0] {
            assert!((eval(&c, x) - dd.eval(x)).abs() < 1e-11);
        }
    }

    #[test]
    fn to_taylor_about_a_shifted_point_still_reproduces_the_polynomial() {
        let xa = [-1.0, 0.5, 2.0, 3.5];
        let ya = [2.0, -1.0, 4.0, 11.0];
        let dd = DividedDifference::new(&xa, &ya).unwrap();
        let xp = 1.25_f64;
        let c = dd.to_taylor(xp);
        for &x in &[-1.0_f64, 0.0, 1.25, 3.0] {
            assert!((eval(&c, x - xp) - dd.eval(x)).abs() < 1e-10, "x={x}");
        }
    }

    #[test]
    fn divided_difference_rejects_malformed_input() {
        assert!(matches!(
            DividedDifference::new(&[0.0, 1.0], &[1.0]),
            Err(crate::PetirError::LengthMismatch { .. })
        ));
        assert!(matches!(
            DividedDifference::new(&[], &[]),
            Err(crate::PetirError::Invalid)
        ));
        assert!(
            matches!(
                DividedDifference::new(&[1.0, 1.0], &[2.0, 3.0]),
                Err(crate::PetirError::ZeroDivide)
            ),
            "a repeated abscissa must be reported, not turned into inf"
        );
    }

    #[test]
    fn single_point_interpolation_is_the_constant() {
        let dd = DividedDifference::new(&[3.0], &[5.0]).unwrap();
        assert_eq!(dd.eval(0.0), 5.0);
        assert_eq!(dd.eval(1e6), 5.0);
    }
}
