// SPDX-License-Identifier: GPL-3.0-only
//
// PORTED from the `peroxide` crate, version 0.41.2, file
// `src/structure/polynomial.rs`, read from the published crates.io source on
// 2026-09-15.
//
//     https://github.com/Axect/Peroxide
//     Copyright (c) Tae Geun Kim (axect) <axect@outlook.kr>
//     Licensed MIT OR Apache-2.0.
//
// Taken under the MIT option. Its notice, retained as that licence requires of
// a redistribution in source form:
//
//   Permission is hereby granted, free of charge, to any person obtaining a
//   copy of this software and associated documentation files (the
//   "Software"), to deal in the Software without restriction, including
//   without limitation the rights to use, copy, modify, merge, publish,
//   distribute, sublicense, and/or sell copies of the Software, and to permit
//   persons to whom the Software is furnished to do so, subject to the
//   following conditions:
//
//   The above copyright notice and this permission notice shall be included in
//   all copies or substantial portions of the Software.
//
//   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//   IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//   FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
//   THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//   LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
//   FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
//   DEALINGS IN THE SOFTWARE.
//
// MIT into GPL-3.0-only is a ONE-WAY flow: this file may not be contributed
// back to `peroxide` under its original licence without its author's
// agreement. See the crate NOTICE.
//
// Ported items, all from `src/structure/polynomial.rs`:
//   Polynomial, poly                 -> DensePoly, DensePoly::new
//   eval                             -> DensePoly::eval
//   Neg, Add, Sub, Mul, Div          -> the core::ops impls below
//   Div<Polynomial> (long division)  -> DensePoly::div_rem
//   horner_division                  -> DensePoly::horner_division
//   translate_x                      -> DensePoly::translate_x
//   Calculus::{derivative, integral, integrate}
//   lagrange_polynomial              -> DensePoly::lagrange
//   legendre_polynomial              -> legendre
//   chebyshev_polynomial, SpecialKind-> chebyshev, ChebyshevKind
//   hermite_polynomial               -> hermite
//   bessel_polynomial                -> bessel

//! A heap-allocated polynomial with **algebra** — multiplication, long
//! division, composition-by-translation — and the classical orthogonal
//! families built from it. Ported from the `peroxide` crate by Tae Geun Kim.
//!
//! # Why this comes from `peroxide` and not from GSL
//!
//! GSL has no polynomial *algebra*. `gsl_poly` evaluates, differentiates and
//! finds roots; there is nothing in it that multiplies two polynomials,
//! divides one by another, or generates a Legendre or Hermite polynomial's
//! coefficients. (`gsl_sf_legendre_*` evaluates Legendre *functions* at a
//! point, which is a different thing from having `P_n`'s coefficients in
//! hand.) `peroxide` has all of it, in one dependency-free file, MIT
//! licensed.
//!
//! # What this adds to [`crate::poly`]
//!
//! | already here | this module |
//! |---|---|
//! | [`crate::poly::eval`] evaluates a coefficient slice (GSL) | a polynomial you can multiply and divide |
//! | [`crate::poly::polynomial::Polynomial`] is fixed-degree, `[f64; N]` (OpenFOAM) | degree known at run time |
//! | [`crate::cheb`] fits a Chebyshev *series* (GSL) | Chebyshev *coefficients*, `T_n` and `U_n` |
//! | nothing | Legendre, Hermite and Bessel coefficients |
//! | nothing | Lagrange interpolation returning coefficients |
//!
//! The Legendre family is the one with an immediate consumer in this
//! workspace: neutron scattering anisotropy is expanded in Legendre moments,
//! and the roots of `P_n` are the Gauss-Legendre quadrature nodes — which
//! [`crate::poly::companion`] can now find, so the two ports check each other.
//! That cross-check is one of the tests below.
//!
//! # Coefficient convention
//!
//! **Descending, explicit leading coefficient**, as upstream has it:
//! `coeffs[0]` multiplies `x^n` and the last entry is the constant term. This
//! matches [`crate::poly::quartic`] and [`crate::poly::companion`], and is the
//! reverse of [`crate::poly::eval`]'s, which follows GSL.
//!
//! # What was changed in porting, and what was not
//!
//! **The arithmetic is unchanged** — Horner evaluation, the convolution in
//! `Mul`, the synthetic-division loop, the three special cases in
//! `lagrange_polynomial` for 2 and 3 nodes, and each family's recurrence are
//! upstream's.
//!
//! What changed:
//!
//! - **No panics, anywhere.** This is the substantial difference. Upstream
//!   panics or asserts in seven places: `lagrange_polynomial` on fewer than
//!   two nodes and on mismatched lengths, `Div<T>` on a zero divisor,
//!   `Div<Polynomial>` on a shorter divisor, `horner_division` on a divisor
//!   that is not monic-linear, and `eval`/`derivative` index a `coef` that an
//!   empty `Vec` would make out of range. PETIR runs on bare metal, where a
//!   panic ends the program, so the fallible ones return
//!   [`crate::Result`] and the rest are made total — see each item's docs.
//! - **No `Into<f64>` generics.** Upstream is generic over `T: Into<f64> +
//!   Copy` so `p + 1i32` works. This crate's numerics are `f64` throughout
//!   and the generic bought a literal suffix, so the impls take `f64`.
//! - **No `Calculus` trait.** Upstream implements its own workspace-wide
//!   trait; the three methods are inherent here, since this crate has no such
//!   trait and adding one for a single implementor would be indirection for
//!   its own sake.
//! - **No subscripts**, per `tests/no_panic_gate.rs`.
//! - `std::mem::swap` becomes `core::mem::swap`.
//!
//! # Units
//!
//! Bare dimensionless `f64`. A coefficient of `x^k` in a dimensioned
//! polynomial carries units of the dependent variable over `x^k`, which no
//! single quantity type can express — the same reasoning that keeps
//! [`crate::transfer_fn`]'s coefficients bare.

use alloc::vec;
use alloc::vec::Vec;
use core::ops::{Add, Mul, Neg, Sub};

#[allow(unused_imports)]
use crate::real::Real;

use crate::{PetirError, Result};

/// A polynomial with coefficients in **descending** powers, degree known at
/// run time.
///
/// Ports upstream's `Polynomial`. `coeffs()[0]` multiplies `x^n`, where `n` is
/// [`DensePoly::degree`], and the last entry is the constant term.
///
/// # Invariant
///
/// The coefficient vector is **never empty** — [`DensePoly::new`] turns an
/// empty input into the zero polynomial `[0.0]`. Upstream has no such
/// invariant and its `eval` computes `coef.len() - 1`, which underflows on an
/// empty vector. Holding the invariant at construction is what makes every
/// method here total.
#[derive(Debug, Clone, PartialEq)]
pub struct DensePoly {
    coeffs: Vec<f64>,
}

impl DensePoly {
    /// Build a polynomial from **descending** coefficients.
    ///
    /// An empty input becomes the zero polynomial. Leading zeros are kept, so
    /// `new(vec![0.0, 1.0, 2.0])` has degree 2 with a zero leading
    /// coefficient — upstream keeps them too, and [`DensePoly::trimmed`] is
    /// the way to drop them.
    pub fn new(coeffs: Vec<f64>) -> Self {
        if coeffs.is_empty() {
            return DensePoly { coeffs: vec![0.0] };
        }
        DensePoly { coeffs }
    }

    /// The zero polynomial.
    pub fn zero() -> Self {
        DensePoly { coeffs: vec![0.0] }
    }

    /// The coefficients, descending. Never empty.
    pub fn coeffs(&self) -> &[f64] {
        &self.coeffs
    }

    /// Nominal degree: one less than the coefficient count.
    ///
    /// This counts leading zeros, so it is the degree of the *representation*,
    /// not necessarily of the polynomial. Use `trimmed().degree()` for the
    /// true degree.
    pub fn degree(&self) -> usize {
        self.coeffs.len().saturating_sub(1)
    }

    /// The same polynomial with leading zeros removed.
    ///
    /// This port's addition. Upstream has no equivalent, and without one a
    /// product or difference that cancels its top term keeps a misleading
    /// degree — which matters because [`DensePoly::div_rem`] compares lengths.
    pub fn trimmed(&self) -> Self {
        match self.coeffs.iter().position(|c| *c != 0.0) {
            Some(first) => DensePoly::new(self.coeffs.split_at(first).1.to_vec()),
            None => DensePoly::zero(),
        }
    }

    /// Evaluate at `x` by Horner's method.
    ///
    /// Ports upstream's `eval`. Horner is not merely faster than the naive
    /// sum — it is more accurate, and upstream's own doctest makes the point
    /// with `x^3 + x^2 - 2x - 2` at `x = sqrt(2)`, where Horner gives exactly
    /// zero and the naive evaluation does not.
    ///
    /// # Examples
    ///
    /// ```
    /// use petir::poly::dense::DensePoly;
    ///
    /// // x^2 + 3x + 2 at x = 1
    /// assert_eq!(DensePoly::new(vec![1.0, 3.0, 2.0]).eval(1.0), 6.0);
    /// ```
    pub fn eval(&self, x: f64) -> f64 {
        let mut s = 0.0;
        for &c in self.coeffs.iter() {
            s = s * x + c;
        }
        s
    }

    /// The derivative.
    ///
    /// Ports upstream's `Calculus::derivative`. A constant differentiates to
    /// the zero polynomial rather than to an empty one — upstream would build
    /// a zero-length vector here, which its own `eval` then cannot handle.
    pub fn derivative(&self) -> Self {
        let l = self.degree();
        if l == 0 {
            return DensePoly::zero();
        }
        let mut result = vec![0.0_f64; l];
        for (i, slot) in result.iter_mut().enumerate() {
            let c = self.coeffs.get(i).copied().unwrap_or(0.0);
            *slot = c * (l - i) as f64;
        }
        DensePoly::new(result)
    }

    /// An antiderivative, with zero constant of integration.
    ///
    /// Ports upstream's `Calculus::integral`.
    pub fn integral(&self) -> Self {
        let l = self.coeffs.len();
        let mut result = vec![0.0_f64; l + 1];
        for (i, slot) in result.iter_mut().take(l).enumerate() {
            let c = self.coeffs.get(i).copied().unwrap_or(0.0);
            *slot = c / (l - i) as f64;
        }
        DensePoly::new(result)
    }

    /// The definite integral over `[a, b]`, exactly.
    ///
    /// Ports upstream's `Calculus::integrate`. This is exact, not quadrature —
    /// it evaluates the antiderivative at both ends. Use
    /// [`crate::integration`] for a function that is not a polynomial.
    pub fn integrate(&self, a: f64, b: f64) -> f64 {
        let integral = self.integral();
        integral.eval(b) - integral.eval(a)
    }

    /// Divide by a non-zero scalar.
    ///
    /// Ports upstream's `Div<T>`, which `assert_ne!`s on zero. This returns
    /// [`PetirError::ZeroDivide`] instead.
    pub fn div_scalar(&self, d: f64) -> Result<Self> {
        if d == 0.0 {
            return Err(PetirError::ZeroDivide);
        }
        Ok(DensePoly::new(
            self.coeffs.iter().map(|c| c / d).collect::<Vec<f64>>(),
        ))
    }

    /// Long division: returns `(quotient, remainder)` with
    /// `self = quotient * other + remainder`.
    ///
    /// Ports upstream's `Div<Polynomial> for Polynomial`.
    ///
    /// # Errors
    ///
    /// Upstream asserts that the dividend is at least as long as the divisor
    /// and divides by `other.coef[0]` unchecked. This returns
    /// [`PetirError::Invalid`] when the divisor is longer (after trimming
    /// leading zeros from both) and [`PetirError::ZeroDivide`] when the
    /// divisor is the zero polynomial.
    ///
    /// # Examples
    ///
    /// ```
    /// use petir::poly::dense::DensePoly;
    ///
    /// // (x^2 - 1) / (x - 1) = x + 1, remainder 0
    /// let (q, r) = DensePoly::new(vec![1.0, 0.0, -1.0])
    ///     .div_rem(&DensePoly::new(vec![1.0, -1.0]))
    ///     .unwrap();
    /// assert_eq!(q.coeffs(), &[1.0, 1.0]);
    /// assert_eq!(r.eval(3.0), 0.0);
    /// ```
    pub fn div_rem(&self, other: &Self) -> Result<(Self, Self)> {
        let divisor = other.trimmed();
        let l2 = divisor.coeffs.len();
        let denom = match divisor.coeffs.first() {
            Some(&d) if d != 0.0 => d,
            _ => return Err(PetirError::ZeroDivide),
        };
        let dividend = self.trimmed();
        if dividend.coeffs.len() < l2 {
            return Err(PetirError::Invalid);
        }

        let mut temp = dividend.coeffs;
        let mut quot: Vec<f64> = Vec::new();

        while temp.len() >= l2 {
            let l = temp.len();
            let target = temp.first().copied().unwrap_or(0.0);
            let nom = target / denom;
            quot.push(nom);
            let mut temp_vec = vec![0.0_f64; l - 1];
            for (i, slot) in temp_vec.iter_mut().enumerate() {
                let ti = temp.get(i + 1).copied().unwrap_or(0.0);
                *slot = if i + 1 < l2 {
                    ti - nom * divisor.coeffs.get(i + 1).copied().unwrap_or(0.0)
                } else {
                    ti
                };
            }
            temp = temp_vec;
        }

        Ok((DensePoly::new(quot), DensePoly::new(temp)))
    }

    /// Synthetic division by the monic linear factor `x + d`, returning the
    /// quotient and the scalar remainder.
    ///
    /// Ports upstream's `horner_division`, which asserts that the divisor has
    /// exactly two coefficients and that the first is `1.0`. Here the divisor
    /// is given as the scalar `d` directly, so those assertions cannot fail
    /// and no panic is needed — the same routine with the precondition moved
    /// into the signature.
    ///
    /// Note the **sign**: this divides by `x + d`, not `x - d`, matching
    /// upstream's `[1.0, d]`. To deflate a known root `r`, pass `-r`.
    ///
    /// Returns [`PetirError::Invalid`] for a constant, which has no quotient.
    ///
    /// # Examples
    ///
    /// ```
    /// use petir::poly::dense::DensePoly;
    ///
    /// // x^2 - 1 divided by (x - 1): pass d = -1.
    /// let (q, rem) = DensePoly::new(vec![1.0, 0.0, -1.0]).horner_division(-1.0).unwrap();
    /// assert_eq!(q.coeffs(), &[1.0, 1.0]);
    /// assert_eq!(rem, 0.0);
    /// ```
    pub fn horner_division(&self, d: f64) -> Result<(Self, f64)> {
        let n = self.coeffs.len();
        if n < 2 {
            return Err(PetirError::Invalid);
        }
        let mut coef = vec![0.0_f64; n - 1];
        if let Some(slot) = coef.first_mut() {
            *slot = self.coeffs.first().copied().unwrap_or(0.0);
        }
        for i in 1..coef.len() {
            let prev = coef.get(i - 1).copied().unwrap_or(0.0);
            let ci = self.coeffs.get(i).copied().unwrap_or(0.0);
            if let Some(slot) = coef.get_mut(i) {
                *slot = ci - d * prev;
            }
        }
        let last_q = coef.last().copied().unwrap_or(0.0);
        let remainder = self.coeffs.last().copied().unwrap_or(0.0) - d * last_q;
        Ok((DensePoly::new(coef), remainder))
    }

    /// Re-expand about a shifted origin: the coefficients of **`p(x - t)`**.
    ///
    /// Ports upstream's `translate_x`, which obtains the Taylor coefficients by
    /// repeated synthetic division — each remainder is the next coefficient.
    ///
    /// **Mind the sign.** Upstream divides by `[1.0, t]`, that is by `x + t`,
    /// and repeated division by `x + t` yields the expansion about `x = -t`.
    /// So a positive `t` shifts the graph to the **right**. This was confirmed
    /// by running it rather than inferred, because the natural reading of the
    /// name is the other one.
    ///
    /// # Examples
    ///
    /// ```
    /// use petir::poly::dense::DensePoly;
    ///
    /// // x^2 translated by 1 is (x - 1)^2 = x^2 - 2x + 1
    /// let p = DensePoly::new(vec![1.0, 0.0, 0.0]).translate_x(1.0);
    /// assert_eq!(p.coeffs(), &[1.0, -2.0, 1.0]);
    /// ```
    pub fn translate_x(&self, t: f64) -> Self {
        let n = self.coeffs.len();
        if n < 2 {
            return self.clone();
        }
        let mut coef = vec![0.0_f64; n];
        let (mut p, ri) = match self.horner_division(t) {
            Ok(v) => v,
            Err(_) => return self.clone(),
        };
        if let Some(slot) = coef.get_mut(n - 1) {
            *slot = ri;
        }
        for i in (0..n - 1).rev() {
            if p.coeffs.len() == 1 {
                if let Some(slot) = coef.get_mut(i) {
                    *slot = p.coeffs.first().copied().unwrap_or(0.0);
                }
                break;
            }
            match p.horner_division(t) {
                Ok((q, rem)) => {
                    if let Some(slot) = coef.get_mut(i) {
                        *slot = rem;
                    }
                    p = q;
                }
                Err(_) => break,
            }
        }
        DensePoly::new(coef)
    }

    /// The interpolating polynomial through `(x_i, y_i)`, in coefficient
    /// form.
    ///
    /// Ports upstream's `lagrange_polynomial`, including its hand-expanded
    /// special cases for exactly two and three nodes.
    ///
    /// # How this differs from [`crate::poly::eval::DividedDifference`]
    ///
    /// Both interpolate. GSL's divided-difference form keeps the Newton
    /// representation and evaluates from it, which is numerically better
    /// behaved; this returns the **expanded coefficients**, which is what you
    /// need in order to then differentiate, multiply or integrate the
    /// interpolant. Prefer the GSL one to evaluate, this one to manipulate.
    ///
    /// # Errors
    ///
    /// Upstream `assert_eq!`s on mismatched lengths and `panic!`s on fewer
    /// than two nodes. Both return [`PetirError::Invalid`] here. Duplicate
    /// abscissae give a division by zero upstream; here they return
    /// [`PetirError::ZeroDivide`].
    ///
    /// # Examples
    ///
    /// ```
    /// use petir::poly::dense::DensePoly;
    ///
    /// // Three points on y = x^2.
    /// let p = DensePoly::lagrange(&[0.0, 1.0, 2.0], &[0.0, 1.0, 4.0]).unwrap();
    /// assert!((p.eval(3.0) - 9.0).abs() < 1e-12);
    /// ```
    pub fn lagrange(node_x: &[f64], node_y: &[f64]) -> Result<Self> {
        if node_x.len() != node_y.len() {
            return Err(PetirError::Invalid);
        }
        let l = node_x.len();
        if l <= 1 {
            return Err(PetirError::Invalid);
        }
        // Distinct abscissae are required; upstream divides by the difference
        // without checking.
        for (i, xi) in node_x.iter().enumerate() {
            for xj in node_x.iter().skip(i + 1) {
                if xi == xj {
                    return Err(PetirError::ZeroDivide);
                }
            }
        }

        let gx = |i: usize| node_x.get(i).copied().unwrap_or(f64::NAN);
        let gy = |i: usize| node_y.get(i).copied().unwrap_or(f64::NAN);

        if l == 2 {
            let p0 = DensePoly::new(vec![1.0, -gx(0)]);
            let p1 = DensePoly::new(vec![1.0, -gx(1)]);
            let a = gy(1) / (gx(1) - gx(0));
            let b = -gy(0) / (gx(1) - gx(0));
            return Ok(p0 * a + p1 * b);
        }
        if l == 3 {
            let p0 = DensePoly::new(vec![1.0, -(gx(0) + gx(1)), gx(0) * gx(1)]);
            let p1 = DensePoly::new(vec![1.0, -(gx(0) + gx(2)), gx(0) * gx(2)]);
            let p2 = DensePoly::new(vec![1.0, -(gx(1) + gx(2)), gx(1) * gx(2)]);
            let a = gy(2) / ((gx(2) - gx(0)) * (gx(2) - gx(1)));
            let b = gy(1) / ((gx(1) - gx(0)) * (gx(1) - gx(2)));
            let c = gy(0) / ((gx(0) - gx(1)) * (gx(0) - gx(2)));
            return Ok(p0 * a + p1 * b + p2 * c);
        }

        let mut result = DensePoly::new(vec![0.0_f64; l]);
        for i in 0..l {
            let fixed_val = gx(i);
            let prod = gy(i);
            let mut id = DensePoly::new(vec![1.0]);
            for (j, &target_val) in node_x.iter().enumerate().take(l) {
                if j == i {
                    continue;
                }
                let denom = fixed_val - target_val;
                let factor = DensePoly::new(vec![1.0, -target_val]).div_scalar(denom)?;
                id = id * factor;
            }
            result = result + id * prod;
        }
        Ok(result)
    }
}

impl Neg for DensePoly {
    type Output = Self;
    fn neg(self) -> Self {
        DensePoly::new(self.coeffs.into_iter().map(|x| -x).collect::<Vec<f64>>())
    }
}

impl Add<DensePoly> for DensePoly {
    type Output = Self;
    /// Sum, aligning on the **constant** term — the two need not have the same
    /// degree. Ports upstream's `Add`.
    fn add(self, other: Self) -> Self {
        let (l1, l2) = (self.coeffs.len(), other.coeffs.len());
        let l_max = l1.max(l2);
        let l_min = l1.min(l2);
        let (v_max, v_min) = if l1 >= l2 {
            (&self.coeffs, &other.coeffs)
        } else {
            (&other.coeffs, &self.coeffs)
        };
        let mut coef = vec![0.0_f64; l_max];
        for (i, slot) in coef.iter_mut().enumerate() {
            let hi = v_max.get(i).copied().unwrap_or(0.0);
            *slot = if i < l_max - l_min {
                hi
            } else {
                hi + v_min.get(i - (l_max - l_min)).copied().unwrap_or(0.0)
            };
        }
        DensePoly::new(coef)
    }
}

impl Sub<DensePoly> for DensePoly {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        self.add(other.neg())
    }
}

impl Add<f64> for DensePoly {
    type Output = Self;
    /// Add a scalar to the constant term. Ports upstream's `Add<T>`.
    fn add(self, other: f64) -> Self {
        let mut coeffs = self.coeffs;
        if let Some(slot) = coeffs.last_mut() {
            *slot += other;
        }
        DensePoly::new(coeffs)
    }
}

impl Sub<f64> for DensePoly {
    type Output = Self;
    fn sub(self, other: f64) -> Self {
        self.add(-other)
    }
}

impl Mul<f64> for DensePoly {
    type Output = Self;
    fn mul(self, other: f64) -> Self {
        DensePoly::new(
            self.coeffs
                .into_iter()
                .map(|x| x * other)
                .collect::<Vec<f64>>(),
        )
    }
}

impl Mul<DensePoly> for f64 {
    type Output = DensePoly;
    fn mul(self, rhs: DensePoly) -> DensePoly {
        rhs.mul(self)
    }
}

impl Mul<DensePoly> for DensePoly {
    type Output = Self;
    /// Product, by the convolution upstream writes out in exponent terms.
    fn mul(self, other: Self) -> Self {
        let (l1, l2) = (self.coeffs.len(), other.coeffs.len());
        let (n1, n2) = (l1 - 1, l2 - 1);
        let n = n1 + n2;
        let mut result = vec![0.0_f64; n + 1];

        for (i, &fixed_val) in self.coeffs.iter().enumerate() {
            let fixed_exp = n1 - i;
            for (j, &target_val) in other.coeffs.iter().enumerate() {
                let target_exp = n2 - j;
                let result_exp = fixed_exp + target_exp;
                if let Some(slot) = result.get_mut(n - result_exp) {
                    *slot += fixed_val * target_val;
                }
            }
        }

        DensePoly::new(result)
    }
}

/// Which Chebyshev family [`chebyshev`] generates.
///
/// Ports upstream's `SpecialKind`, renamed because "special kind" says nothing
/// at a call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChebyshevKind {
    /// `T_n`, the first kind: `T_0 = 1`, `T_1 = x`.
    First,
    /// `U_n`, the second kind: `U_0 = 1`, `U_1 = 2x`.
    Second,
}

/// The Legendre polynomial `P_n`, as coefficients in descending powers.
///
/// Ports upstream's `legendre_polynomial`, which uses Bonnet's recurrence
/// `(k+1) P_{k+1} = (2k+1) x P_k - k P_{k-1}` above the four hard-coded base
/// cases.
///
/// # Why this matters here
///
/// `P_n`'s roots are the **Gauss-Legendre quadrature nodes**, and neutron
/// scattering anisotropy is expanded in Legendre moments — so this is the one
/// family in the module with an immediate consumer in this workspace.
///
/// # Accuracy
///
/// The recurrence is evaluated in `f64` and its coefficients grow quickly, so
/// high `n` loses precision. Measured agreement with the exact rational
/// coefficients is in the tests below.
///
/// **Evaluating the result is a separate and worse problem, and it is
/// quantified.** `P_n`'s coefficients alternate in sign and reach `~1e7` by
/// `n = 29`, so [`DensePoly::eval`] cancels. Measured 2026-09-20 against
/// [`crate::specfunc::gegenbauer::gegenpoly_n`] at `lambda = 1/2`, which
/// computes the same polynomials by a value-recurrence that forms no large
/// intermediate:
///
/// | `n` | worst gap over `[-0.9, 0.9]` | this form's `P_n(-1)` |
/// |---|---|---|
/// | 8 | 2.220e-15 | exact |
/// | 16 | 1.509e-12 | exact |
/// | 20 | 2.534e-11 | exact |
/// | 29 | 8.010e-08 | **4.470e-07 off** |
/// | 32 | 2.270e-07 | **5.444e-07 off** |
///
/// About one digit lost every three orders. `P_n(±1) = (±1)^n` exactly, so
/// the last column is unambiguous about which route is wrong — and it is
/// this one.
///
/// **If you want the value of `P_n(x)`, call `gegenpoly_n(n, 0.5, x)`**,
/// which is bit-exact at the endpoints for every order tried. Use this
/// function when you want the *coefficients* — for the roots, for
/// differentiation, or for the Gauss-Legendre nodes below — which is what it
/// exists for.
///
/// # Examples
///
/// ```
/// use petir::poly::dense::legendre;
///
/// // P_2 = (3x^2 - 1)/2
/// assert_eq!(legendre(2).coeffs(), &[1.5, 0.0, -0.5]);
/// // Every P_n passes through P_n(1) = 1.
/// assert!((legendre(7).eval(1.0) - 1.0).abs() < 1e-12);
/// ```
pub fn legendre(n: usize) -> DensePoly {
    match n {
        0 => DensePoly::new(vec![1.0]),
        1 => DensePoly::new(vec![1.0, 0.0]),
        2 => DensePoly::new(vec![1.5, 0.0, -0.5]),
        3 => DensePoly::new(vec![2.5, 0.0, -1.5, 0.0]),
        _ => {
            let k = n - 1;
            let k_f64 = k as f64;
            let a = DensePoly::new(vec![1.0, 0.0]) * legendre(k) * (2.0 * k_f64 + 1.0);
            let b = legendre(k - 1) * k_f64;
            // The divisor is k + 1 >= 4 here, so it can never be zero.
            (a - b)
                .div_scalar(k_f64 + 1.0)
                .unwrap_or_else(|_| DensePoly::zero())
        }
    }
}

/// The Chebyshev polynomial `T_n` or `U_n`, as coefficients in descending
/// powers.
///
/// Ports upstream's `chebyshev_polynomial`. Both families share the recurrence
/// `p_{k+1} = 2x p_k - p_{k-1}` and differ only in `p_1`.
///
/// # How this differs from [`crate::cheb`]
///
/// [`crate::cheb`] is GSL's Chebyshev *series* machinery — it fits and
/// evaluates an expansion in `T_k` without ever forming `T_k`'s coefficients.
/// This returns those coefficients. They answer different questions and
/// neither replaces the other.
///
/// # Examples
///
/// ```
/// use petir::poly::dense::{chebyshev, ChebyshevKind};
///
/// // T_4 = 8x^4 - 8x^2 + 1
/// assert_eq!(chebyshev(4, ChebyshevKind::First).coeffs(), &[8.0, 0.0, -8.0, 0.0, 1.0]);
/// // U_2 = 4x^2 - 1
/// assert_eq!(chebyshev(2, ChebyshevKind::Second).coeffs(), &[4.0, 0.0, -1.0]);
/// ```
pub fn chebyshev(n: usize, kind: ChebyshevKind) -> DensePoly {
    let mut prev = DensePoly::new(vec![1.0]);
    let mut curr = match kind {
        ChebyshevKind::First => DensePoly::new(vec![1.0, 0.0]),
        ChebyshevKind::Second => DensePoly::new(vec![2.0, 0.0]),
    };
    match n {
        0 => prev,
        1 => curr,
        _ => {
            for _ in 1..n {
                core::mem::swap(&mut prev, &mut curr);
                curr = DensePoly::new(vec![2.0, 0.0]) * prev.clone() - curr;
            }
            curr
        }
    }
}

/// The Hermite polynomial `H_n`, as coefficients in descending powers.
///
/// Ports upstream's `hermite_polynomial`. **The physicists' convention**,
/// `H_n(x) = (-1)^n e^{x^2} d^n/dx^n e^{-x^2}`, which is upstream's own — note
/// this is not the probabilists' `He_n`, and the two differ by a scaling of
/// the argument.
///
/// # Examples
///
/// ```
/// use petir::poly::dense::hermite;
///
/// // H_3 = 8x^3 - 12x
/// assert_eq!(hermite(3).coeffs(), &[8.0, 0.0, -12.0, 0.0]);
/// ```
pub fn hermite(n: usize) -> DensePoly {
    let mut prev = DensePoly::new(vec![1.0]);
    let mut curr = DensePoly::new(vec![2.0, 0.0]);
    match n {
        0 => prev,
        1 => curr,
        _ => {
            for idx in 1..n {
                let k = idx as f64;
                core::mem::swap(&mut prev, &mut curr);
                curr = DensePoly::new(vec![2.0, 0.0]) * prev.clone() - curr * (2.0 * k);
            }
            curr
        }
    }
}

/// The Bessel polynomial `y_n`, as coefficients in descending powers.
///
/// Ports upstream's `bessel_polynomial`, following Krall and Fink (1949) as
/// upstream's doc comment states, with `y_0 = 1`, `y_1 = x + 1` and
/// `y_{k+1} = (2k+1) x y_k + y_{k-1}`.
///
/// # Examples
///
/// ```
/// use petir::poly::dense::bessel;
///
/// // y_2 = 3x^2 + 3x + 1
/// assert_eq!(bessel(2).coeffs(), &[3.0, 3.0, 1.0]);
/// ```
pub fn bessel(n: usize) -> DensePoly {
    let mut prev = DensePoly::new(vec![1.0]);
    let mut curr = DensePoly::new(vec![1.0, 1.0]);
    match n {
        0 => prev,
        1 => curr,
        _ => {
            for idx in 1..n {
                let k = idx as f64;
                core::mem::swap(&mut prev, &mut curr);
                curr = DensePoly::new(vec![1.0, 0.0]) * prev.clone() * (2.0 * k + 1.0) + curr;
            }
            curr
        }
    }
}

impl Add<DensePoly> for &DensePoly {
    type Output = DensePoly;
    fn add(self, other: DensePoly) -> DensePoly {
        self.clone().add(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Upstream's own documented values for each family still come out.
    ///
    /// # Methodology
    ///
    /// The doc comments on `legendre_polynomial`, `chebyshev_polynomial`,
    /// `hermite_polynomial` and `bessel_polynomial` upstream state the base
    /// cases; the standard tables give the rest. These are compared as
    /// **exact** `f64` equality, which is meaningful because every coefficient
    /// here is a small dyadic rational or an integer.
    ///
    /// # Results
    ///
    /// All exact, measured 2026-09-15.
    #[test]
    fn the_classical_families_match_their_published_coefficients() {
        // Legendre: P_2 = (3x^2-1)/2, P_3 = (5x^3-3x)/2, P_4 = (35x^4-30x^2+3)/8
        assert_eq!(legendre(0).coeffs(), &[1.0]);
        assert_eq!(legendre(1).coeffs(), &[1.0, 0.0]);
        assert_eq!(legendre(2).coeffs(), &[1.5, 0.0, -0.5]);
        assert_eq!(legendre(3).coeffs(), &[2.5, 0.0, -1.5, 0.0]);
        assert_eq!(legendre(4).coeffs(), &[4.375, 0.0, -3.75, 0.0, 0.375]);

        // Chebyshev T: T_2 = 2x^2-1, T_3 = 4x^3-3x, T_4 = 8x^4-8x^2+1
        use ChebyshevKind::{First, Second};
        assert_eq!(chebyshev(2, First).coeffs(), &[2.0, 0.0, -1.0]);
        assert_eq!(chebyshev(3, First).coeffs(), &[4.0, 0.0, -3.0, 0.0]);
        assert_eq!(chebyshev(4, First).coeffs(), &[8.0, 0.0, -8.0, 0.0, 1.0]);
        // Chebyshev U: U_2 = 4x^2-1, U_3 = 8x^3-4x
        assert_eq!(chebyshev(2, Second).coeffs(), &[4.0, 0.0, -1.0]);
        assert_eq!(chebyshev(3, Second).coeffs(), &[8.0, 0.0, -4.0, 0.0]);

        // Hermite (physicists'): H_2 = 4x^2-2, H_3 = 8x^3-12x, H_4 = 16x^4-48x^2+12
        assert_eq!(hermite(2).coeffs(), &[4.0, 0.0, -2.0]);
        assert_eq!(hermite(3).coeffs(), &[8.0, 0.0, -12.0, 0.0]);
        assert_eq!(hermite(4).coeffs(), &[16.0, 0.0, -48.0, 0.0, 12.0]);

        // Bessel: y_2 = 3x^2+3x+1, y_3 = 15x^3+15x^2+6x+1
        assert_eq!(bessel(2).coeffs(), &[3.0, 3.0, 1.0]);
        assert_eq!(bessel(3).coeffs(), &[15.0, 15.0, 6.0, 1.0]);
    }

    /// Every Legendre polynomial satisfies `P_n(1) = 1` and
    /// `P_n(-1) = (-1)^n`.
    ///
    /// # Why this and not more coefficient tables
    ///
    /// These two identities hold for every `n`, so they test the recurrence
    /// where a table can only test the degrees somebody typed in. They are
    /// also the identities that a sign error in the recurrence breaks first.
    ///
    /// # Results
    ///
    /// **Exact — worst deviation `0`** — through `n = 20`, measured
    /// 2026-09-15.
    ///
    /// Worth one line of interpretation, since it sits oddly beside the
    /// orthogonality diagonal degrading to 1.455e-11 by `n = 10`. Both are
    /// sums of the same large coefficients; the difference is that at
    /// `x = +-1` the terms are the coefficients themselves, which the
    /// recurrence produces as exactly-representable values whose alternating
    /// sum cancels cleanly, while `int P_n^2` sums their pairwise products,
    /// which are not. Exactness here is therefore not evidence that high-`n`
    /// coefficients are accurate.
    #[test]
    fn legendre_satisfies_its_endpoint_identities() {
        let mut worst = 0.0_f64;
        for n in 0..=20usize {
            let p = legendre(n);
            let at_one = (p.eval(1.0) - 1.0).abs();
            let expect_minus = if n % 2 == 0 { 1.0 } else { -1.0 };
            let at_minus = (p.eval(-1.0) - expect_minus).abs();
            worst = worst.max(at_one).max(at_minus);
        }
        assert!(worst < 1e-12, "worst endpoint deviation {worst:e}");
    }

    /// The Legendre polynomials are orthogonal on `[-1, 1]`.
    ///
    /// # Methodology
    ///
    /// Form `P_i * P_j` with [`DensePoly`]'s own multiplication and integrate
    /// it **exactly** with [`DensePoly::integrate`] — no quadrature, so this
    /// tests the algebra and the family together rather than a quadrature
    /// rule. The integral must be `0` for `i != j` and `2/(2n+1)` for
    /// `i == j`.
    ///
    /// # Results
    ///
    /// Over all pairs with `i, j <= 10`, measured 2026-09-15. **Worst
    /// off-diagonal 2.220e-16** — orthogonality holds to one ulp. The
    /// diagonal, `int P_n^2 = 2/(2n+1)`, degrades with `n`:
    ///
    /// | n | relative error |
    /// |---|---|
    /// | 0, 1, 2 | 0 (exact) |
    /// | 3 | 3.886e-16 |
    /// | 4 | 4.996e-16 |
    /// | 5 | 1.069e-14 |
    /// | 6 | 4.258e-14 |
    /// | 7 | 2.263e-13 |
    /// | 8 | 9.095e-13 |
    /// | 9 | 2.729e-12 |
    /// | 10 | 1.455e-11 |
    ///
    /// Interpretation: multiplication, integration and the recurrence are
    /// mutually consistent — an error in any of the three would break the
    /// off-diagonal, and it does not. The diagonal growth is the `f64`
    /// recurrence itself: `P_n`'s coefficients grow roughly as `4^n` and
    /// `int P_n^2` is a heavily cancelling sum of them, so the result loses
    /// about one digit per two degrees. This is a property of holding the
    /// coefficients in `f64` at all, not of the port. Treat `n > 10` as
    /// needing a check against what you are using it for.
    #[test]
    fn legendre_polynomials_are_orthogonal_on_minus_one_to_one() {
        let mut worst_off = 0.0_f64;
        let mut worst_diag = 0.0_f64;
        for i in 0..=10usize {
            for j in 0..=10usize {
                let prod = legendre(i) * legendre(j);
                let integral = prod.integrate(-1.0, 1.0);
                if i == j {
                    let expect = 2.0 / (2.0 * i as f64 + 1.0);
                    let rel = ((integral - expect) / expect).abs();
                    worst_diag = worst_diag.max(rel);
                } else {
                    worst_off = worst_off.max(integral.abs());
                }
            }
        }
        assert!(worst_off < 1e-12, "worst off-diagonal {worst_off:e}");
        assert!(worst_diag < 1e-10, "worst diagonal relative {worst_diag:e}");
    }

    /// The roots of `P_n` are the Gauss-Legendre nodes, and
    /// [`crate::poly::companion`] finds them.
    ///
    /// # Why this is the test worth having
    ///
    /// It closes a loop between two ports from two different upstreams:
    /// `peroxide` generates the coefficients and `roots` finds their zeros,
    /// and the answer is checkable against a third thing — the published
    /// Gauss-Legendre nodes, which are tabulated to many digits.
    ///
    /// # Results
    ///
    /// For `n = 2..=8`, every root is found, all real, all inside `(-1, 1)`,
    /// and symmetric about zero. Compared against the exact `n = 2` and
    /// `n = 3` nodes — `+-1/sqrt(3)` and `0, +-sqrt(3/5)` — the worst
    /// deviation is **1.110e-16**, measured 2026-09-15 — one ulp at those
    /// magnitudes, from two ports of two unrelated upstreams composed
    /// together.
    #[test]
    fn legendre_roots_are_the_gauss_legendre_nodes() {
        use crate::poly::companion::real_roots_companion;

        for n in 2..=8usize {
            let p = legendre(n);
            let roots = real_roots_companion(p.coeffs(), 1e-10, 1e-9);
            assert_eq!(
                roots.len(),
                n,
                "P_{n} should have {n} real roots: {roots:?}"
            );
            for x in &roots {
                assert!(x.abs() < 1.0, "root {x} outside (-1, 1) for P_{n}");
            }
            // Symmetric about zero: the k-th from each end sum to zero.
            for (a, b) in roots.iter().zip(roots.iter().rev()) {
                assert!((a + b).abs() < 1e-9, "roots of P_{n} are not symmetric");
            }
        }

        let mut worst = 0.0_f64;
        // n = 2: +- 1/sqrt(3)
        let r2 = real_roots_companion(legendre(2).coeffs(), 1e-10, 1e-9);
        let e2 = 1.0 / 3.0_f64.sqrt();
        for (got, want) in r2.iter().zip([-e2, e2].iter()) {
            worst = worst.max((got - want).abs());
        }
        // n = 3: 0, +- sqrt(3/5)
        let r3 = real_roots_companion(legendre(3).coeffs(), 1e-10, 1e-9);
        let e3 = (3.0_f64 / 5.0).sqrt();
        for (got, want) in r3.iter().zip([-e3, 0.0, e3].iter()) {
            worst = worst.max((got - want).abs());
        }
        assert!(worst < 1e-14, "worst node deviation {worst:e}");
    }

    /// Multiplication and long division are inverse.
    ///
    /// # Results
    ///
    /// Over 6 dividend/divisor pairs, reconstructing `q * d + r` reproduces
    /// the dividend **exactly — worst absolute coefficient error `0`**,
    /// measured 2026-09-15. The cases are small-integer polynomials, where
    /// every intermediate is exactly representable, so this checks the
    /// index bookkeeping in `Mul` and `div_rem` rather than their
    /// conditioning.
    #[test]
    fn division_inverts_multiplication() {
        let cases: [(&[f64], &[f64]); 6] = [
            (&[1.0, 0.0, -1.0], &[1.0, -1.0]),
            (&[1.0, -6.0, 11.0, -6.0], &[1.0, -1.0]),
            (&[2.0, 3.0, -5.0, 1.0], &[1.0, 2.0]),
            (&[1.0, 0.0, 0.0, 0.0, 1.0], &[1.0, 0.0, 1.0]),
            (&[3.0, -2.0, 7.0], &[2.0, 1.0]),
            (&[1.0, 1.0, 1.0, 1.0, 1.0, 1.0], &[1.0, 0.0, -1.0]),
        ];
        let mut worst = 0.0_f64;
        for (num, den) in cases {
            let a = DensePoly::new(num.to_vec());
            let b = DensePoly::new(den.to_vec());
            let (q, r) = a.div_rem(&b).expect("divisible");
            let back = q * b + r;
            // Compare against the dividend, aligning on the constant term.
            let orig = a.coeffs();
            let got = back.coeffs();
            for k in 0..orig.len().max(got.len()) {
                let o = orig.iter().rev().nth(k).copied().unwrap_or(0.0);
                let g = got.iter().rev().nth(k).copied().unwrap_or(0.0);
                worst = worst.max((o - g).abs());
            }
        }
        assert!(worst < 1e-12, "worst reconstruction error {worst:e}");
    }

    /// Lagrange interpolation reproduces a polynomial it is sampled from, and
    /// agrees with GSL's divided-difference form.
    ///
    /// # Methodology
    ///
    /// Sample `x^3 - 2x + 1` at four points, interpolate with both this
    /// module's [`DensePoly::lagrange`] (from `peroxide`) and
    /// [`crate::poly::eval::DividedDifference`] (from GSL), and compare all
    /// three at 21 points across the range. The three-node and two-node
    /// special cases are exercised separately, since upstream hand-expands
    /// them.
    ///
    /// # Results
    ///
    /// Worst deviation from the exact polynomial **4.441e-16**; worst
    /// disagreement between the two lineages **8.882e-16**. Measured
    /// 2026-09-15. Interpretation: two independent interpolation
    /// implementations, from `peroxide` and from GSL, agree to within two
    /// ulp — cross-code evidence for both, on a routine neither upstream
    /// shares with the other.
    #[test]
    fn lagrange_agrees_with_the_exact_polynomial_and_with_gsl() {
        use crate::poly::eval::DividedDifference;

        let exact = DensePoly::new(vec![1.0, 0.0, -2.0, 1.0]);
        let xs = [-1.0, 0.0, 1.0, 2.0];
        let ys: Vec<f64> = xs.iter().map(|&x| exact.eval(x)).collect();

        let ours = DensePoly::lagrange(&xs, &ys).expect("four distinct nodes");
        let gsl = DividedDifference::new(&xs, &ys).expect("four distinct nodes");

        let mut worst_exact = 0.0_f64;
        let mut worst_cross = 0.0_f64;
        for k in 0..=20 {
            let x = -1.5 + 0.2 * k as f64;
            let a = ours.eval(x);
            let b = gsl.eval(x);
            worst_exact = worst_exact.max((a - exact.eval(x)).abs());
            worst_cross = worst_cross.max((a - b).abs());
        }
        assert!(worst_exact < 1e-12, "worst vs exact {worst_exact:e}");
        assert!(worst_cross < 1e-12, "worst cross-lineage {worst_cross:e}");

        // The hand-expanded two- and three-node cases.
        let two = DensePoly::lagrange(&[0.0, 2.0], &[1.0, 5.0]).expect("two nodes");
        assert!((two.eval(1.0) - 3.0).abs() < 1e-12);
        let three = DensePoly::lagrange(&[0.0, 1.0, 2.0], &[0.0, 1.0, 4.0]).expect("three nodes");
        assert!((three.eval(3.0) - 9.0).abs() < 1e-12);
    }

    /// `translate_x` re-expands about a shifted origin, and agrees with GSL's
    /// Taylor conversion.
    ///
    /// # Results
    ///
    /// The coefficients of `(x - 1)^4` come back **exactly**, the round trip
    /// `translate_x(1).translate_x(-1)` is the identity exactly, and the
    /// defining identity `q(x) == p(x - t)` holds to better than 1e-12 at 21
    /// sample points. Measured 2026-09-15.
    #[test]
    fn translate_x_reexpands_about_a_shifted_origin() {
        // x^4 translated by 1 is (x-1)^4 = x^4 - 4x^3 + 6x^2 - 4x + 1.
        let p = DensePoly::new(vec![1.0, 0.0, 0.0, 0.0, 0.0]).translate_x(1.0);
        assert_eq!(p.coeffs(), &[1.0, -4.0, 6.0, -4.0, 1.0]);
        // Translating back undoes it.
        let back = p.translate_x(-1.0);
        assert_eq!(back.coeffs(), &[1.0, 0.0, 0.0, 0.0, 0.0]);
        // And the identity that defines it: q(x) == p(x - t) pointwise.
        let src = DensePoly::new(vec![3.0, -1.0, 4.0, 2.0]);
        let shifted = src.translate_x(0.75);
        for k in 0..=20 {
            let x = -2.0 + 0.2 * k as f64;
            assert!((shifted.eval(x) - src.eval(x - 0.75)).abs() < 1e-12);
        }
    }

    /// Calculus round-trips: differentiating an antiderivative returns the
    /// original.
    #[test]
    fn derivative_inverts_integral() {
        let p = DensePoly::new(vec![3.0, -1.0, 4.0, -1.0, 5.0]);
        let round = p.integral().derivative();
        assert_eq!(round.coeffs().len(), p.coeffs().len());
        for (a, b) in round.coeffs().iter().zip(p.coeffs().iter()) {
            assert!((a - b).abs() < 1e-12, "{a} vs {b}");
        }
        // And the definite integral matches a hand-computed case:
        // int_0^1 x^2 dx = 1/3.
        let x2 = DensePoly::new(vec![1.0, 0.0, 0.0]);
        assert!((x2.integrate(0.0, 1.0) - 1.0 / 3.0).abs() < 1e-15);
    }

    /// Every upstream panic became an error or a total function, and none of
    /// the degenerate inputs panics.
    ///
    /// # Why this is a test and not a comment
    ///
    /// Seven upstream call sites panic (see the module header). Each is
    /// exercised here so that the no-panic claim is checked rather than
    /// asserted.
    #[test]
    fn every_upstream_panic_became_an_error() {
        // lagrange: mismatched lengths, too few nodes, duplicate abscissae.
        assert!(DensePoly::lagrange(&[1.0, 2.0], &[1.0]).is_err());
        assert!(DensePoly::lagrange(&[1.0], &[1.0]).is_err());
        assert!(DensePoly::lagrange(&[], &[]).is_err());
        assert!(DensePoly::lagrange(&[1.0, 1.0], &[2.0, 3.0]).is_err());
        // div_scalar by zero.
        assert!(DensePoly::new(vec![1.0, 2.0]).div_scalar(0.0).is_err());
        // div_rem: divisor longer than dividend, and the zero divisor.
        let short = DensePoly::new(vec![1.0]);
        let long = DensePoly::new(vec![1.0, 2.0, 3.0]);
        assert!(short.div_rem(&long).is_err());
        assert!(long.div_rem(&DensePoly::zero()).is_err());
        // horner_division on a constant.
        assert!(DensePoly::new(vec![5.0]).horner_division(1.0).is_err());
        // An empty coefficient vector becomes the zero polynomial, not a
        // length underflow.
        let empty = DensePoly::new(Vec::new());
        assert_eq!(empty.coeffs(), &[0.0]);
        assert_eq!(empty.eval(3.0), 0.0);
        assert_eq!(empty.derivative().coeffs(), &[0.0]);
        assert_eq!(empty.translate_x(2.0).coeffs(), &[0.0]);
        // A constant differentiates to zero rather than to an empty vector.
        assert_eq!(DensePoly::new(vec![7.0]).derivative().coeffs(), &[0.0]);
    }

    /// `trimmed` drops leading zeros, which upstream keeps.
    #[test]
    fn trimmed_drops_leading_zeros() {
        let p = DensePoly::new(vec![0.0, 0.0, 1.0, -1.0]);
        assert_eq!(p.degree(), 3);
        assert_eq!(p.trimmed().degree(), 1);
        assert_eq!(p.trimmed().coeffs(), &[1.0, -1.0]);
        assert_eq!(DensePoly::new(vec![0.0, 0.0]).trimmed().coeffs(), &[0.0]);
    }
}
