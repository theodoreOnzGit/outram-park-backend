//! Periodic cubic spline and polynomial interpolation, ported from GSL 2.8
//! (gh:#1341).
//!
//! Upstream: GSL 2.8, `ampl/gsl` commit `cf180cd7`, `interpolation/cspline.c`
//! (`cspline_init_periodic`, Engeln-Muellges & Uhlig p. 256, and the shared
//! `cspline_eval*` kernels; Copyright (C) 1996-2000, 2001, 2002, 2004 Gerard
//! Jungman) and `interpolation/poly.c` (Copyright (C) 2001 DAN, HO-JIN), on top
//! of `poly/dd.c` (divided differences) and `linalg/tridiag.c` (the cyclic
//! tridiagonal solver, [`crate::linalg::solve_symm_cyc_tridiag`]).
//! GPL-3.0-or-later. Research, education and V&V only.
//!
//! # What each is for
//!
//! - [`PeriodicSpline`]: a cubic spline whose first and second derivatives
//!   match across the ends, for data with `y[0] == y[n-1]` (GSL does not check
//!   that and neither does this). `n >= 3` (`n == 2` fails in GSL, in the
//!   cyclic solver, and fails here the same way).
//! - [`PolynomialInterp`]: the single polynomial through every point, in
//!   Newton form. Fine for a handful of points; for more it rings (Runge).
//!
//! # `spline.c` is not ported, on purpose
//!
//! `gsl_spline` is `gsl_interp` plus an owned copy of `x` and `y`. PETIR's
//! [`crate::interp::Interp`] and these types already own their data, so a
//! wrapper would add nothing (checked 2026-11-13 against `spline.c`).
//!
//! # Differences from upstream
//!
//! - Out-of-range and `NaN` arguments are [`PetirError::Domain`], as in
//!   [`crate::interp`]; a repeated or decreasing abscissa is refused at
//!   construction.
//! - No accelerator (GSL's plain `gsl_interp_bsearch` path is what is ported).
//! - Every array access is a checked `get`.
//! - [`PolynomialInterp`] ports `gsl_poly_dd_taylor` line by line for the
//!   derivative and the integral. [`crate::poly::DividedDifference::to_taylor`]
//!   is a different accumulation (not bit-identical), so it is not used.
//!
//! # Units
//!
//! Bare dimensionless `f64`.

use alloc::vec;
use alloc::vec::Vec;

#[allow(unused_imports)]
use crate::real::Real;

use crate::interp::integ_eval;
use crate::linalg::solve_symm_cyc_tridiag;
use crate::poly::DividedDifference;
use crate::zip::zip2;
use crate::{PetirError, Result};

fn at(v: &[f64], i: usize) -> f64 {
    v.get(i).copied().unwrap_or(f64::NAN)
}

fn put(v: &mut [f64], i: usize, x: f64) {
    if let Some(s) = v.get_mut(i) {
        *s = x;
    }
}

/// Validate `(xa, ya)` as `gsl_interp_alloc` + `gsl_interp_init` do.
fn check_data(xa: &[f64], ya: &[f64], min_size: usize) -> Result<()> {
    if xa.len() != ya.len() {
        return Err(PetirError::LengthMismatch {
            expected: xa.len(),
            found: ya.len(),
        });
    }
    if xa.len() < min_size {
        return Err(PetirError::Invalid);
    }
    for (&lo, &hi) in zip2(xa.iter(), xa.iter().skip(1)) {
        if !(lo < hi) {
            return Err(PetirError::Domain);
        }
    }
    Ok(())
}

/// `gsl_interp_bsearch(x_array, x, 0, size - 1)`.
fn bsearch(xa: &[f64], x: f64) -> usize {
    let mut ilo = 0usize;
    let mut ihi = xa.len().saturating_sub(1);
    while ihi > ilo + 1 {
        let i = (ihi + ilo) / 2;
        if at(xa, i) > x {
            ihi = i;
        } else {
            ilo = i;
        }
    }
    ilo
}

fn check_range(xa: &[f64], x: f64) -> Result<()> {
    let (Some(&lo), Some(&hi)) = (xa.first(), xa.last()) else {
        return Err(PetirError::Invalid);
    };
    if !(x >= lo && x <= hi) {
        return Err(PetirError::Domain);
    }
    Ok(())
}

/// A periodic cubic spline, GSL `gsl_interp_cspline_periodic`.
///
/// # Example
///
/// ```
/// use petir::interp_cspline_poly::PeriodicSpline;
/// // One period of a sampled sine; the endpoints agree.
/// let n = 9;
/// let x: Vec<f64> = (0..n).map(|i| 2.0 * core::f64::consts::PI * i as f64 / (n - 1) as f64).collect();
/// let mut y: Vec<f64> = x.iter().map(|v| v.sin()).collect();
/// y[n - 1] = y[0];
/// let s = PeriodicSpline::new(&x, &y).unwrap();
/// assert!((s.eval(1.0).unwrap() - 1.0_f64.sin()).abs() < 5e-3);
/// // Slopes match across the seam.
/// assert!((s.eval_deriv(x[0]).unwrap() - s.eval_deriv(x[n - 1]).unwrap()).abs() < 1e-12);
/// ```
#[derive(Debug, Clone)]
pub struct PeriodicSpline {
    xa: Vec<f64>,
    ya: Vec<f64>,
    c: Vec<f64>,
}

impl PeriodicSpline {
    /// `gsl_interp_init` with `gsl_interp_cspline_periodic`
    /// (`cspline_init_periodic`, cspline.c:143-210).
    ///
    /// # Errors
    ///
    /// [`PetirError::LengthMismatch`], [`PetirError::Invalid`] (fewer than 2
    /// points, and exactly 2 points, where GSL's cyclic solver refuses),
    /// [`PetirError::Domain`] (abscissae not strictly increasing),
    /// [`PetirError::ZeroDivide`] (a zero pivot).
    pub fn new(xa: &[f64], ya: &[f64]) -> Result<Self> {
        check_data(xa, ya, 2)?;
        let size = xa.len();
        let max_index = size - 1;
        let sys_size = max_index;
        let mut c = vec![0.0_f64; size];

        if sys_size == 2 {
            // Solve the 2x2 system in closed form.
            let h0 = at(xa, 1) - at(xa, 0);
            let h1 = at(xa, 2) - at(xa, 1);
            let a = 2.0 * (h0 + h1);
            let b = h0 + h1;
            let g0 = 3.0 * ((at(ya, 2) - at(ya, 1)) / h1 - (at(ya, 1) - at(ya, 0)) / h0);
            let g1 = 3.0 * ((at(ya, 1) - at(ya, 2)) / h0 - (at(ya, 2) - at(ya, 1)) / h1);
            let det = 3.0 * (h0 + h1) * (h0 + h1);
            put(&mut c, 1, (a * g0 - b * g1) / det);
            put(&mut c, 2, (-b * g0 + a * g1) / det);
            let c2 = at(&c, 2);
            put(&mut c, 0, c2);
            return Ok(Self {
                xa: xa.to_vec(),
                ya: ya.to_vec(),
                c,
            });
        }

        let mut diag = vec![0.0_f64; sys_size];
        let mut offdiag = vec![0.0_f64; sys_size];
        let mut g = vec![0.0_f64; sys_size];

        for i in 0..(sys_size - 1) {
            let h_i = at(xa, i + 1) - at(xa, i);
            let h_ip1 = at(xa, i + 2) - at(xa, i + 1);
            let ydiff_i = at(ya, i + 1) - at(ya, i);
            let ydiff_ip1 = at(ya, i + 2) - at(ya, i + 1);
            let g_i = if h_i != 0.0 { 1.0 / h_i } else { 0.0 };
            let g_ip1 = if h_ip1 != 0.0 { 1.0 / h_ip1 } else { 0.0 };
            put(&mut offdiag, i, h_ip1);
            put(&mut diag, i, 2.0 * (h_ip1 + h_i));
            put(&mut g, i, 3.0 * (ydiff_ip1 * g_ip1 - ydiff_i * g_i));
        }

        let i = sys_size - 1;
        {
            let h_i = at(xa, i + 1) - at(xa, i);
            let h_ip1 = at(xa, 1) - at(xa, 0);
            let ydiff_i = at(ya, i + 1) - at(ya, i);
            let ydiff_ip1 = at(ya, 1) - at(ya, 0);
            let g_i = if h_i != 0.0 { 1.0 / h_i } else { 0.0 };
            let g_ip1 = if h_ip1 != 0.0 { 1.0 / h_ip1 } else { 0.0 };
            put(&mut offdiag, i, h_ip1);
            put(&mut diag, i, 2.0 * (h_ip1 + h_i));
            put(&mut g, i, 3.0 * (ydiff_ip1 * g_ip1 - ydiff_i * g_i));
        }

        let solution = solve_symm_cyc_tridiag(&diag, &offdiag, &g)?;
        for (slot, &v) in zip2(c.iter_mut().skip(1), solution.iter()) {
            *slot = v;
        }
        let last = at(&c, max_index);
        put(&mut c, 0, last);
        Ok(Self {
            xa: xa.to_vec(),
            ya: ya.to_vec(),
            c,
        })
    }

    /// `coeff_calc` (cspline.c:222-231): `(b, c, d)` on the interval at `index`.
    fn coeff(&self, dy: f64, dx: f64, index: usize) -> (f64, f64, f64) {
        let c_i = at(&self.c, index);
        let c_ip1 = at(&self.c, index + 1);
        let b = (dy / dx) - dx * (c_ip1 + 2.0 * c_i) / 3.0;
        let d = (c_ip1 - c_i) / (3.0 * dx);
        (b, c_i, d)
    }

    /// `(index, delx, b, c, d, y_lo)` of the interval holding `x`.
    fn segment(&self, x: f64) -> Result<(f64, f64, f64, f64, f64)> {
        check_range(&self.xa, x)?;
        let index = bsearch(&self.xa, x);
        let x_hi = at(&self.xa, index + 1);
        let x_lo = at(&self.xa, index);
        let dx = x_hi - x_lo;
        if !(dx > 0.0) {
            return Err(PetirError::Invalid);
        }
        let y_lo = at(&self.ya, index);
        let y_hi = at(&self.ya, index + 1);
        let dy = y_hi - y_lo;
        let delx = x - x_lo;
        let (b, c, d) = self.coeff(dy, dx, index);
        Ok((delx, b, c, d, y_lo))
    }

    /// Interpolated value (`cspline_eval`): `y_lo + delx (b + delx (c + delx d))`.
    ///
    /// # Errors
    ///
    /// [`PetirError::Domain`] outside `[x[0], x[n-1]]` or for `NaN`.
    pub fn eval(&self, x: f64) -> Result<f64> {
        let (delx, b, c, d, y_lo) = self.segment(x)?;
        Ok(y_lo + delx * (b + delx * (c + delx * d)))
    }

    /// First derivative (`cspline_eval_deriv`): `b + delx (2 c + 3 d delx)`.
    ///
    /// # Errors
    ///
    /// As [`Self::eval`].
    pub fn eval_deriv(&self, x: f64) -> Result<f64> {
        let (delx, b, c, d, _) = self.segment(x)?;
        Ok(b + delx * (2.0 * c + 3.0 * d * delx))
    }

    /// Second derivative (`cspline_eval_deriv2`): `2 c + 6 d delx`.
    ///
    /// # Errors
    ///
    /// As [`Self::eval`].
    pub fn eval_deriv2(&self, x: f64) -> Result<f64> {
        let (delx, _, c, d, _) = self.segment(x)?;
        Ok(2.0 * c + 6.0 * d * delx)
    }

    /// Integral from `a` to `b` (`cspline_eval_integ`).
    ///
    /// # Errors
    ///
    /// [`PetirError::Domain`] if `a > b`, a limit is outside the table or
    /// `NaN` (GSL lets `NaN` through; refused here as in
    /// [`crate::interp::Interp::eval_integ`]).
    pub fn eval_integ(&self, a: f64, b: f64) -> Result<f64> {
        check_range(&self.xa, a)?;
        check_range(&self.xa, b)?;
        if a > b {
            return Err(PetirError::Domain);
        }
        if a == b {
            return Ok(0.0);
        }
        let index_a = bsearch(&self.xa, a);
        let index_b = bsearch(&self.xa, b);
        let mut result = 0.0_f64;
        for i in index_a..=index_b {
            let x_hi = at(&self.xa, i + 1);
            let x_lo = at(&self.xa, i);
            let y_lo = at(&self.ya, i);
            let y_hi = at(&self.ya, i + 1);
            let dx = x_hi - x_lo;
            let dy = y_hi - y_lo;
            if dx != 0.0 {
                let (b_i, c_i, d_i) = self.coeff(dy, dx, i);
                if i == index_a || i == index_b {
                    let x1 = if i == index_a { a } else { x_lo };
                    let x2 = if i == index_b { b } else { x_hi };
                    result += integ_eval(y_lo, b_i, c_i, d_i, x_lo, x1, x2);
                } else {
                    result += dx * (y_lo + dx * (0.5 * b_i + dx * (c_i / 3.0 + 0.25 * d_i * dx)));
                }
            } else {
                return Err(PetirError::Invalid);
            }
        }
        Ok(result)
    }
}

/// `gsl_poly_dd_taylor` (poly/dd.c:53-86), line by line: the Taylor
/// coefficients about `xp` of the Newton-form polynomial `(dd, xa)`.
fn dd_taylor(xp: f64, dd: &[f64], xa: &[f64]) -> Vec<f64> {
    let size = dd.len();
    let mut c = vec![0.0_f64; size];
    let mut w = vec![0.0_f64; size];

    put(&mut w, size - 1, 1.0);
    put(&mut c, 0, at(dd, 0));

    // for (i = size - 1; i-- > 0;)
    for i in (0..(size - 1)).rev() {
        let shift = at(xa, size - 2 - i) - xp;
        let w_i = -at(&w, i + 1) * shift;
        put(&mut w, i, w_i);

        for j in (i + 1)..(size - 1) {
            let v = at(&w, j) - at(&w, j + 1) * shift;
            put(&mut w, j, v);
        }

        for j in i..size {
            let v = at(&c, j - i) + at(&w, j) * at(dd, size - i - 1);
            put(&mut c, j - i, v);
        }
    }
    c
}

/// The interpolating polynomial through all points, GSL
/// `gsl_interp_polynomial`.
///
/// # Example
///
/// ```
/// use petir::interp_cspline_poly::PolynomialInterp;
/// // Through (0,1), (1,3), (2,7), (3,13): the quadratic 1 + x + x^2.
/// let p = PolynomialInterp::new(&[0.0, 1.0, 2.0, 3.0], &[1.0, 3.0, 7.0, 13.0]).unwrap();
/// assert!((p.eval(1.5).unwrap() - 4.75).abs() < 1e-12);
/// assert!((p.eval_deriv(1.5).unwrap() - 4.0).abs() < 1e-12);
/// assert!((p.eval_deriv2(1.5).unwrap() - 2.0).abs() < 1e-12);
/// assert!((p.eval_integ(0.0, 3.0).unwrap() - 14.25).abs() < 1e-12);
/// ```
#[derive(Debug, Clone)]
pub struct PolynomialInterp {
    xa: Vec<f64>,
    dd: DividedDifference,
}

impl PolynomialInterp {
    /// `gsl_interp_init` with `gsl_interp_polynomial` (`min_size` 3).
    ///
    /// # Errors
    ///
    /// [`PetirError::LengthMismatch`], [`PetirError::Invalid`] (fewer than 3
    /// points), [`PetirError::Domain`] (abscissae not strictly increasing).
    pub fn new(xa: &[f64], ya: &[f64]) -> Result<Self> {
        check_data(xa, ya, 3)?;
        let dd = DividedDifference::new(xa, ya)?;
        Ok(Self {
            xa: xa.to_vec(),
            dd,
        })
    }

    /// `gsl_poly_dd_eval` (`polynomial_eval`).
    ///
    /// # Errors
    ///
    /// [`PetirError::Domain`] outside `[x[0], x[n-1]]` or for `NaN`.
    pub fn eval(&self, x: f64) -> Result<f64> {
        check_range(&self.xa, x)?;
        Ok(self.dd.eval(x))
    }

    /// `coeff[1]` of the Taylor expansion about `x` (`polynomial_deriv`).
    ///
    /// # Errors
    ///
    /// As [`Self::eval`].
    pub fn eval_deriv(&self, x: f64) -> Result<f64> {
        check_range(&self.xa, x)?;
        let c = dd_taylor(x, self.dd.coefficients(), &self.xa);
        Ok(at(&c, 1))
    }

    /// `2 coeff[2]` of the Taylor expansion about `x` (`polynomial_deriv2`).
    ///
    /// # Errors
    ///
    /// As [`Self::eval`].
    pub fn eval_deriv2(&self, x: f64) -> Result<f64> {
        check_range(&self.xa, x)?;
        let c = dd_taylor(x, self.dd.coefficients(), &self.xa);
        Ok(2.0 * at(&c, 2))
    }

    /// Integral of the monomial form about 0 (`polynomial_integ`):
    /// `c0 (b-a) + sum_i c_i (b^(i+1) - a^(i+1)) / (i + 1)`.
    ///
    /// # Errors
    ///
    /// [`PetirError::Domain`] if `a > b`, a limit is outside the table or `NaN`.
    pub fn eval_integ(&self, a: f64, b: f64) -> Result<f64> {
        check_range(&self.xa, a)?;
        check_range(&self.xa, b)?;
        if a > b {
            return Err(PetirError::Domain);
        }
        let size = self.xa.len();
        let c = dd_taylor(0.0, self.dd.coefficients(), &self.xa);
        let mut sum = at(&c, 0) * (b - a);
        for i in 1..size {
            let e = (i + 1) as f64;
            sum += at(&c, i) * (Real::powf(b, e) - Real::powf(a, e)) / (i as f64 + 1.0);
        }
        Ok(sum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periodic_spline_is_smooth_across_the_seam_and_refuses_bad_input() {
        for n in [3usize, 4, 5, 9] {
            let x: Vec<f64> = (0..n).map(|i| (i * i) as f64 + i as f64).collect();
            let mut y: Vec<f64> = x.iter().map(|v| (v * 0.3).sin() + 0.1 * v).collect();
            y[n - 1] = y[0];
            let s = PeriodicSpline::new(&x, &y).unwrap();
            let (d0, d1) = (s.eval_deriv(x[0]).unwrap(), s.eval_deriv(x[n - 1]).unwrap());
            let (e0, e1) = (
                s.eval_deriv2(x[0]).unwrap(),
                s.eval_deriv2(x[n - 1]).unwrap(),
            );
            assert!((d0 - d1).abs() < 1e-10 && (e0 - e1).abs() < 1e-10, "n={n}");
            assert!((s.eval(x[1]).unwrap() - y[1]).abs() < 1e-12);
            assert!(s.eval_integ(x[0], x[n - 1]).unwrap().is_finite());
        }
        assert!(PeriodicSpline::new(&[0.0, 1.0], &[0.0, 0.0]).is_err());
        assert!(PeriodicSpline::new(&[0.0], &[0.0]).is_err());
        assert!(PeriodicSpline::new(&[0.0, 1.0, 1.0], &[0.0, 1.0, 0.0]).is_err());
        let s = PeriodicSpline::new(&[0.0, 1.0, 2.0, 3.0], &[0.0, 1.0, 2.0, 0.0]).unwrap();
        assert!(s.eval(4.0).is_err() && s.eval(f64::NAN).is_err());
        assert!(s.eval_integ(2.0, 1.0).is_err());
        assert_eq!(s.eval_integ(1.0, 1.0).unwrap(), 0.0);
        assert!(s.eval_deriv2(5.0).is_err());
    }

    #[test]
    fn polynomial_reproduces_a_cubic_exactly_and_refuses_bad_input() {
        let x = [0.0, 1.0, 2.0, 3.0, 4.0];
        let y: Vec<f64> = x.iter().map(|v| 1.0 - 2.0 * v + 0.5 * v * v * v).collect();
        let p = PolynomialInterp::new(&x, &y).unwrap();
        for t in [0.3, 1.7, 3.9] {
            let want = 1.0 - 2.0 * t + 0.5 * t * t * t;
            assert!((p.eval(t).unwrap() - want).abs() < 1e-12);
            assert!((p.eval_deriv(t).unwrap() - (-2.0 + 1.5 * t * t)).abs() < 1e-11);
            assert!((p.eval_deriv2(t).unwrap() - 3.0 * t).abs() < 1e-11);
        }
        let exact = 4.0 - 4.0 * 2.0 * 2.0 / 2.0 * 1.0 + 0.5 * 4.0_f64.powi(4) / 4.0 - 0.0;
        let _ = exact;
        // int_0^4 (1 - 2x + x^3/2) dx = 4 - 16 + 32 = 20.
        assert!((p.eval_integ(0.0, 4.0).unwrap() - 20.0).abs() < 1e-10);
        assert!(PolynomialInterp::new(&x[..2], &y[..2]).is_err());
        assert!(PolynomialInterp::new(&[0.0, 1.0, 1.0], &[0.0, 1.0, 0.0]).is_err());
        assert!(p.eval(5.0).is_err() && p.eval_integ(2.0, 1.0).is_err());
        assert!(p.eval_deriv(f64::NAN).is_err() && p.eval_deriv2(-1.0).is_err());
    }
}
