// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Translation of the MOOSE framework interpolation classes that Moltres'
// `GenericMoltresMaterial` / `MoltresJsonMaterial` delegate to.
//   Upstream: https://github.com/idaholab/moose (INL, LGPL-2.1)
//   Upstream commit: d7c4bde4025537ac4574cc80e78c2407e23a42cc (the MOOSE
//     submodule pointer of Moltres 3dd2ce7a60f59424d0d8f46748ba4e243d76b671)
//   Upstream sources translated (framework/src/utils/ and framework/include/utils/):
//     SplineInterpolationBase.C   spline(), findInterval(), computeCoeffs(),
//                                 sample(), sampleDerivative()
//     SplineInterpolation.C       errorCheck(), solve(), sample(), sampleDerivative()
//     MonotoneCubicInterpolation.C errorCheck(), phi/psi, h1..h4, p, pPrime,
//                                 initialize_derivs(), modify_derivs(), solve(),
//                                 findInterval(), sample(), sampleDerivative()
//     BicubicSplineInterpolation.C errorCheck(), solve(), construct{Row,Column}Spline*,
//                                 sample(), sampleDerivative()
//   Upstream license: LGPL-2.1, incorporated into this GPL-3.0 crate under
//   the LGPL-2.1 section 3 conversion option. No MOOSE or libMesh is linked.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! MOOSE `SplineInterpolation`, `MonotoneCubicInterpolation` and
//! `BicubicSplineInterpolation` (gh:#1538), translated expression by
//! expression in MOOSE's own operation order so that they can be compared
//! bit for bit with the C++ (`verification_and_validation/
//! moltres_differential_gh1400/dump_moltres_kernels.cpp`).
//!
//! Behaviours kept from upstream on purpose:
//! * **no clamping outside the table**: the end cell's cubic is extrapolated
//!   (unlike `LinearInterpolation`, which holds the end values);
//! * the natural-spline end condition is selected by `yp >= 1e30`, and the
//!   default end slope is `f64::MAX`;
//! * `MonotoneCubicInterpolation` needs at least three points;
//! * `std::pow(x, 2)` is `x * x` (GCC folds it that way; glibc's `pow` also
//!   returns the correctly rounded square for these arguments).
//!
//! Not translated: the second-derivative samplers (`sample2ndDerivative`),
//! the `ADReal` overloads and the CSV dump helpers, none of which Moltres
//! calls.

use crate::error::MoltresError;

/// MOOSE `SplineInterpolationBase::_deriv_bound`
/// (`std::numeric_limits<Real>::max()`): an end slope at or above `1e30`
/// means "natural" end condition.
pub const DERIV_BOUND: f64 = f64::MAX;

fn bad(msg: &str) -> MoltresError {
    MoltresError::InvalidMaterial(msg.to_string())
}

/// `SplineInterpolationBase::spline`: second derivatives `y2` of the cubic
/// spline through `(x, y)` with end slopes `yp1`, `ypn` (`>= 1e30` selects the
/// natural condition). Numerical Recipes in C section 3.3.
///
/// # Errors
/// Fewer than two knots (upstream `mooseError`).
pub fn spline_second_derivatives(
    x: &[f64],
    y: &[f64],
    yp1: f64,
    ypn: f64,
) -> Result<Vec<f64>, MoltresError> {
    let n = x.len();
    if n < 2 {
        return Err(bad("You must have at least two knots to create a spline."));
    }
    let mut u = vec![0.0; n];
    let mut y2 = vec![0.0; n];
    if yp1 >= 1e30 {
        y2[0] = 0.0;
        u[0] = 0.0;
    } else {
        y2[0] = -0.5;
        u[0] = (3.0 / (x[1] - x[0])) * ((y[1] - y[0]) / (x[1] - x[0]) - yp1);
    }
    for i in 1..n - 1 {
        let sig = (x[i] - x[i - 1]) / (x[i + 1] - x[i - 1]);
        let p = sig * y2[i - 1] + 2.0;
        y2[i] = (sig - 1.0) / p;
        u[i] = (y[i + 1] - y[i]) / (x[i + 1] - x[i]) - (y[i] - y[i - 1]) / (x[i] - x[i - 1]);
        u[i] = (6.0 * u[i] / (x[i + 1] - x[i - 1]) - sig * u[i - 1]) / p;
    }
    let (qn, un);
    if ypn >= 1e30 {
        qn = 0.0;
        un = 0.0;
    } else {
        qn = 0.5;
        un = (3.0 / (x[n - 1] - x[n - 2])) * (ypn - (y[n - 1] - y[n - 2]) / (x[n - 1] - x[n - 2]));
    }
    y2[n - 1] = (un - qn * u[n - 2]) / (qn * y2[n - 2] + 1.0);
    for k in (1..n).rev() {
        y2[k - 1] = y2[k - 1] * y2[k] + u[k - 1];
    }
    Ok(y2)
}

/// `SplineInterpolationBase::findInterval`: bisection, `x[klo] <= x_int <
/// x[khi]` inside the table and the end cell outside it.
fn spline_find_interval(x: &[f64], x_int: f64) -> (usize, usize) {
    let mut klo = 0usize;
    let mut khi = x.len() - 1;
    while khi - klo > 1 {
        let k = (khi + klo) >> 1;
        if x[k] > x_int {
            khi = k;
        } else {
            klo = k;
        }
    }
    (klo, khi)
}

/// `SplineInterpolationBase::computeCoeffs` (returns `h, a, b`).
///
/// # Errors
/// `x[khi] == x[klo]` (upstream `mooseError`).
fn spline_coeffs(
    x: &[f64],
    klo: usize,
    khi: usize,
    x_int: f64,
) -> Result<(f64, f64, f64), MoltresError> {
    let h = x[khi] - x[klo];
    if h == 0.0 {
        return Err(bad("The values of x must be distinct"));
    }
    let a = (x[khi] - x_int) / h;
    let b = (x_int - x[klo]) / h;
    Ok((h, a, b))
}

/// `SplineInterpolationBase::sample(x, y, y2, x_int, klo, khi)`.
fn spline_sample_in(
    x: &[f64],
    y: &[f64],
    y2: &[f64],
    x_int: f64,
    klo: usize,
    khi: usize,
) -> Result<f64, MoltresError> {
    let (h, a, b) = spline_coeffs(x, klo, khi, x_int)?;
    Ok(a * y[klo]
        + b * y[khi]
        + ((a * a * a - a) * y2[klo] + (b * b * b - b) * y2[khi]) * (h * h) / 6.0)
}

/// `SplineInterpolationBase::sample(x, y, y2, x_int)`.
///
/// # Errors
/// Repeated abscissae.
pub fn spline_sample(x: &[f64], y: &[f64], y2: &[f64], x_int: f64) -> Result<f64, MoltresError> {
    let (klo, khi) = spline_find_interval(x, x_int);
    spline_sample_in(x, y, y2, x_int, klo, khi)
}

/// `SplineInterpolationBase::sampleDerivative`.
///
/// # Errors
/// Repeated abscissae.
pub fn spline_sample_derivative(
    x: &[f64],
    y: &[f64],
    y2: &[f64],
    x_int: f64,
) -> Result<f64, MoltresError> {
    let (klo, khi) = spline_find_interval(x, x_int);
    let (h, a, b) = spline_coeffs(x, klo, khi, x_int)?;
    Ok((y[khi] - y[klo]) / h
        - (((3.0 * a * a - 1.0) * y2[klo] + (3.0 * b * b - 1.0) * -y2[khi]) * h / 6.0))
}

/// MOOSE `SplineInterpolation`: natural (or clamped-slope) cubic spline of
/// one table column, as Moltres builds it for `interp_type = spline`.
#[derive(Debug, Clone, PartialEq)]
pub struct SplineInterpolation {
    x: Vec<f64>,
    y: Vec<f64>,
    y2: Vec<f64>,
}

impl SplineInterpolation {
    /// `SplineInterpolation::setData(x, y)` with the default natural ends
    /// (`yp1 = ypn = _deriv_bound`), which is what Moltres calls.
    ///
    /// # Errors
    /// Different lengths, `x` not strictly increasing, or fewer than two
    /// knots (`errorCheck()` and `spline()`).
    pub fn new(x: &[f64], y: &[f64]) -> Result<Self, MoltresError> {
        Self::with_end_slopes(x, y, DERIV_BOUND, DERIV_BOUND)
    }

    /// `SplineInterpolation::setData(x, y, yp1, ypn)`.
    ///
    /// # Errors
    /// As [`Self::new`].
    pub fn with_end_slopes(x: &[f64], y: &[f64], yp1: f64, ypn: f64) -> Result<Self, MoltresError> {
        if x.len() != y.len() {
            return Err(bad("SplineInterpolation: vectors are not the same length"));
        }
        for i in 0..x.len().saturating_sub(1) {
            if x[i] >= x[i + 1] {
                return Err(bad("x-values are not strictly increasing"));
            }
        }
        let y2 = spline_second_derivatives(x, y, yp1, ypn)?;
        Ok(Self {
            x: x.to_vec(),
            y: y.to_vec(),
            y2,
        })
    }

    /// `SplineInterpolation::sample`. The end cell's cubic is extrapolated
    /// outside the table.
    #[must_use]
    pub fn sample(&self, v: f64) -> f64 {
        spline_sample(&self.x, &self.y, &self.y2, v).unwrap_or(f64::NAN)
    }

    /// `SplineInterpolation::sampleDerivative`.
    #[must_use]
    pub fn sample_derivative(&self, v: f64) -> f64 {
        spline_sample_derivative(&self.x, &self.y, &self.y2, v).unwrap_or(f64::NAN)
    }
}

/// MOOSE `MonotoneCubicInterpolation`: Fritsch-Carlson-limited cubic Hermite
/// interpolation, as Moltres builds it for `interp_type = monotone_cubic`.
#[derive(Debug, Clone, PartialEq)]
pub struct MonotoneCubicInterpolation {
    x: Vec<f64>,
    y: Vec<f64>,
    yp: Vec<f64>,
}

fn mc_sign(x: f64) -> f64 {
    if x < 0.0 {
        -1.0
    } else if x > 0.0 {
        1.0
    } else {
        0.0
    }
}

fn mc_phi(t: f64) -> f64 {
    3. * t * t - 2. * t * t * t
}

fn mc_phi_prime(t: f64) -> f64 {
    6. * t - 6. * t * t
}

fn mc_psi(t: f64) -> f64 {
    t * t * t - t * t
}

fn mc_psi_prime(t: f64) -> f64 {
    3. * t * t - 2. * t
}

fn mc_h1(xhi: f64, xlo: f64, x: f64) -> f64 {
    let h = xhi - xlo;
    let t = (xhi - x) / h;
    mc_phi(t)
}

fn mc_h1_prime(xhi: f64, xlo: f64, x: f64) -> f64 {
    let h = xhi - xlo;
    let t = (xhi - x) / h;
    let t_prime = -1. / h;
    mc_phi_prime(t) * t_prime
}

fn mc_h2(xhi: f64, xlo: f64, x: f64) -> f64 {
    let h = xhi - xlo;
    let t = (x - xlo) / h;
    mc_phi(t)
}

fn mc_h2_prime(xhi: f64, xlo: f64, x: f64) -> f64 {
    let h = xhi - xlo;
    let t = (x - xlo) / h;
    let t_prime = 1. / h;
    mc_phi_prime(t) * t_prime
}

fn mc_h3(xhi: f64, xlo: f64, x: f64) -> f64 {
    let h = xhi - xlo;
    let t = (xhi - x) / h;
    -h * mc_psi(t)
}

fn mc_h3_prime(xhi: f64, xlo: f64, x: f64) -> f64 {
    let h = xhi - xlo;
    let t = (xhi - x) / h;
    let t_prime = -1. / h;
    -h * mc_psi_prime(t) * t_prime
}

fn mc_h4(xhi: f64, xlo: f64, x: f64) -> f64 {
    let h = xhi - xlo;
    let t = (x - xlo) / h;
    h * mc_psi(t)
}

fn mc_h4_prime(xhi: f64, xlo: f64, x: f64) -> f64 {
    let h = xhi - xlo;
    let t = (x - xlo) / h;
    let t_prime = 1. / h;
    h * mc_psi_prime(t) * t_prime
}

/// Values at one cell's two ends, `(xhi, xlo, fhi, flo, dhi, dlo)`.
#[derive(Clone, Copy)]
struct HermiteCell {
    xhi: f64,
    xlo: f64,
    fhi: f64,
    flo: f64,
    dhi: f64,
    dlo: f64,
}

/// `MonotoneCubicInterpolation::p`.
fn mc_p(c: HermiteCell, x: f64) -> f64 {
    c.flo * mc_h1(c.xhi, c.xlo, x)
        + c.fhi * mc_h2(c.xhi, c.xlo, x)
        + c.dlo * mc_h3(c.xhi, c.xlo, x)
        + c.dhi * mc_h4(c.xhi, c.xlo, x)
}

/// `MonotoneCubicInterpolation::pPrime`.
fn mc_p_prime(c: HermiteCell, x: f64) -> f64 {
    c.flo * mc_h1_prime(c.xhi, c.xlo, x)
        + c.fhi * mc_h2_prime(c.xhi, c.xlo, x)
        + c.dlo * mc_h3_prime(c.xhi, c.xlo, x)
        + c.dhi * mc_h4_prime(c.xhi, c.xlo, x)
}

impl MonotoneCubicInterpolation {
    /// `MonotoneCubicInterpolation::setData` = `errorCheck()` + `solve()`.
    ///
    /// # Errors
    /// Different lengths, fewer than three points, or `x` not strictly
    /// increasing (upstream throws `std::domain_error`).
    pub fn new(x: &[f64], y: &[f64]) -> Result<Self, MoltresError> {
        if x.len() != y.len() {
            return Err(bad(
                "MonotoneCubicInterpolation: x and y vectors are not the same length",
            ));
        }
        if x.len() < 3 {
            return Err(MoltresError::InvalidMaterial(format!(
                "MonotoneCubicInterpolation: {} points is not enough data for a cubic interpolation",
                x.len()
            )));
        }
        for i in 0..x.len() - 1 {
            if x[i] >= x[i + 1] {
                return Err(bad("x-values are not strictly increasing"));
            }
        }
        let y = y.to_vec();
        let n_knots = x.len();
        let n_intervals = x.len() - 1;
        let internal_knots = x.len() - 2;
        let mut h = vec![0.0; n_intervals];
        let mut yp = vec![0.0; n_knots];
        let mut delta = vec![0.0; n_intervals];
        let mut alpha = vec![0.0; n_intervals];
        let mut beta = vec![0.0; n_intervals];
        for i in 0..n_intervals {
            h[i] = x[i + 1] - x[i];
        }

        // initialize_derivs()
        for i in 1..n_knots - 1 {
            yp[i] = (h[i - 1] * h[i - 1] * y[i + 1]
                - h[i] * h[i] * y[i - 1]
                - y[i] * (h[i - 1] - h[i]) * (h[i - 1] + h[i]))
                / (h[i - 1] * h[i] * (h[i - 1] * h[i]));
        }
        yp[0] = (-(h[0] * h[0]) * y[2] - h[1] * y[0] * (2.0 * h[0] + h[1])
            + y[1] * ((h[0] + h[1]) * (h[0] + h[1])))
            / (h[0] * h[1] * (h[0] + h[1]));
        let hlast = h[n_intervals - 1];
        let hsecond = h[n_intervals - 2];
        let ylast = y[n_knots - 1];
        let ysecond = y[n_knots - 2];
        let ythird = y[n_knots - 3];
        yp[n_knots - 1] = (hsecond * ylast * (hsecond + 2.0 * hlast) + hlast * hlast * ythird
            - ysecond * ((hsecond + hlast) * (hsecond + hlast)))
            / (hsecond * hlast * (hsecond + hlast));

        for i in 0..n_intervals {
            delta[i] = (y[i + 1] - y[i]) / h[i];
        }
        if mc_sign(delta[0]) != mc_sign(yp[0]) {
            yp[0] = 0.0;
        }
        if mc_sign(delta[n_intervals - 1]) != mc_sign(yp[n_knots - 1]) {
            yp[n_knots - 1] = 0.0;
        }
        for i in 0..internal_knots {
            if mc_sign(delta[i + 1]) == 0.0
                || mc_sign(delta[i]) == 0.0
                || mc_sign(delta[i + 1]) != mc_sign(delta[i])
            {
                yp[1 + i] = 0.0;
            }
        }

        for i in 0..n_intervals {
            if yp[i] == 0.0 {
                alpha[i] = 0.0;
            } else if delta[i] == 0.0 {
                alpha[i] = 4.0;
            } else {
                alpha[i] = yp[i] / delta[i];
            }
            if yp[i + 1] == 0.0 {
                beta[i] = 0.0;
            } else if delta[i] == 0.0 {
                beta[i] = 4.0;
            } else {
                beta[i] = yp[i + 1] / delta[i];
            }
            // check if outside region of monotonicity
            if alpha[i] * alpha[i] + beta[i] * beta[i] > 9. {
                // modify_derivs(alpha, beta, delta, yp_lo, yp_hi)
                let tau = 3. / (alpha[i] * alpha[i] + beta[i] * beta[i]).sqrt();
                let alpha_star = alpha[i] * tau;
                let beta_star = beta[i] * tau;
                yp[i] = alpha_star * delta[i];
                yp[i + 1] = beta_star * delta[i];
            }
        }
        Ok(Self {
            x: x.to_vec(),
            y,
            yp,
        })
    }

    fn cell(&self, v: f64) -> HermiteCell {
        let mut klo = 0usize;
        let mut khi = self.x.len() - 1;
        while khi - klo > 1 {
            let k = (khi + klo) >> 1;
            if self.x[k] > v {
                khi = k;
            } else {
                klo = k;
            }
        }
        HermiteCell {
            xhi: self.x[khi],
            xlo: self.x[klo],
            fhi: self.y[khi],
            flo: self.y[klo],
            dhi: self.yp[khi],
            dlo: self.yp[klo],
        }
    }

    /// `MonotoneCubicInterpolation::sample`. The end cell's cubic is
    /// extrapolated outside the table.
    #[must_use]
    pub fn sample(&self, v: f64) -> f64 {
        mc_p(self.cell(v), v)
    }

    /// `MonotoneCubicInterpolation::sampleDerivative`.
    #[must_use]
    pub fn sample_derivative(&self, v: f64) -> f64 {
        mc_p_prime(self.cell(v), v)
    }

    /// The limited end and knot slopes `_yp` after `solve()` (for tests).
    #[must_use]
    pub fn knot_slopes(&self) -> &[f64] {
        &self.yp
    }
}

/// MOOSE `BicubicSplineInterpolation` with default (natural) end slopes, the
/// way Moltres' `bicubicSplineConstruct` calls `setData(x1, x2, y)`.
///
/// `y[i][j]` is the value at `(x1[i], x2[j])`. Each sample builds a column
/// spline from `m` row-spline evaluations (and, for the `x2` slope, a row
/// spline from `n` column-spline evaluations), exactly as upstream.
#[derive(Debug, Clone, PartialEq)]
pub struct BicubicSplineInterpolation {
    x1: Vec<f64>,
    x2: Vec<f64>,
    y: Vec<Vec<f64>>,
    y_trans: Vec<Vec<f64>>,
    y2_rows: Vec<Vec<f64>>,
    y2_columns: Vec<Vec<f64>>,
}

impl BicubicSplineInterpolation {
    /// `BicubicSplineInterpolation::setData(x1, x2, y)`.
    ///
    /// # Errors
    /// `y` not `x1.len() x x2.len()`, or a row/column with fewer than two
    /// knots.
    pub fn new(x1: &[f64], x2: &[f64], y: &[Vec<f64>]) -> Result<Self, MoltresError> {
        let m = x1.len();
        let n = x2.len();
        if y.len() != m {
            return Err(bad("y row dimension does not match the size of x1."));
        }
        for row in y {
            if row.len() != n {
                return Err(bad("y column dimension does not match the size of x2."));
            }
        }
        // constructRowSplineSecondDerivativeTable() (natural ends: the
        // defaulted `_yx21`, `_yx2n` hold `_deriv_bound`)
        let mut y2_rows = Vec::with_capacity(m);
        for row in y.iter() {
            y2_rows.push(spline_second_derivatives(x2, row, DERIV_BOUND, DERIV_BOUND)?);
        }
        // constructColumnSplineSecondDerivativeTable()
        let mut y_trans = vec![vec![0.0; m]; n];
        for (i, row) in y.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                y_trans[j][i] = *v;
            }
        }
        let mut y2_columns = Vec::with_capacity(n);
        for col in y_trans.iter() {
            y2_columns.push(spline_second_derivatives(x1, col, DERIV_BOUND, DERIV_BOUND)?);
        }
        Ok(Self {
            x1: x1.to_vec(),
            x2: x2.to_vec(),
            y: y.to_vec(),
            y_trans,
            y2_rows,
            y2_columns,
        })
    }

    /// `constructColumnSpline`: `m` row-spline evaluations at `x2` and the
    /// column spline's second derivatives through them.
    fn column_spline(&self, x2: f64) -> Result<(Vec<f64>, Vec<f64>), MoltresError> {
        let (klo, khi) = spline_find_interval(&self.x2, x2);
        let mut eval = vec![0.0; self.x1.len()];
        for (j, e) in eval.iter_mut().enumerate() {
            *e = spline_sample_in(&self.x2, &self.y[j], &self.y2_rows[j], x2, klo, khi)?;
        }
        let y2 = spline_second_derivatives(&self.x1, &eval, DERIV_BOUND, DERIV_BOUND)?;
        Ok((eval, y2))
    }

    /// `constructRowSpline`: `n` column-spline evaluations at `x1` and the
    /// row spline's second derivatives through them.
    fn row_spline(&self, x1: f64) -> Result<(Vec<f64>, Vec<f64>), MoltresError> {
        let (klo, khi) = spline_find_interval(&self.x1, x1);
        let mut eval = vec![0.0; self.x2.len()];
        for (j, e) in eval.iter_mut().enumerate() {
            *e = spline_sample_in(&self.x1, &self.y_trans[j], &self.y2_columns[j], x1, klo, khi)?;
        }
        let y2 = spline_second_derivatives(&self.x2, &eval, DERIV_BOUND, DERIV_BOUND)?;
        Ok((eval, y2))
    }

    /// `BicubicSplineInterpolation::sample(x1, x2)`.
    #[must_use]
    pub fn sample(&self, x1: f64, x2: f64) -> f64 {
        self.column_spline(x2)
            .and_then(|(eval, y2)| spline_sample(&self.x1, &eval, &y2, x1))
            .unwrap_or(f64::NAN)
    }

    /// `BicubicSplineInterpolation::sampleDerivative(x1, x2, deriv_var)`:
    /// `deriv_var = 1` is `d/dx1`, `2` is `d/dx2`. Any other value is
    /// upstream's `mooseError`, returned here as NaN.
    #[must_use]
    pub fn sample_derivative(&self, x1: f64, x2: f64, deriv_var: u32) -> f64 {
        match deriv_var {
            1 => self
                .column_spline(x2)
                .and_then(|(eval, y2)| spline_sample_derivative(&self.x1, &eval, &y2, x1))
                .unwrap_or(f64::NAN),
            2 => self
                .row_spline(x1)
                .and_then(|(eval, y2)| spline_sample_derivative(&self.x2, &eval, &y2, x2))
                .unwrap_or(f64::NAN),
            _ => f64::NAN,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_spline_reproduces_nodes_and_a_line() {
        let x = [800.0, 850.0, 900.0, 950.0];
        let y: Vec<f64> = x.iter().map(|t| 2.0 * t + 1.0).collect();
        let s = SplineInterpolation::new(&x, &y).unwrap();
        for (xi, yi) in x.iter().zip(&y) {
            assert!((s.sample(*xi) - yi).abs() < 1e-9);
        }
        // a straight line has zero second derivative: spline == line, and the
        // end cell extrapolates (no clamping)
        assert!((s.sample(1000.0) - 2001.0).abs() < 1e-8);
        assert!((s.sample_derivative(870.0) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn spline_rejects_bad_input() {
        assert!(SplineInterpolation::new(&[1.0, 2.0], &[1.0]).is_err());
        assert!(SplineInterpolation::new(&[1.0, 1.0], &[1.0, 2.0]).is_err());
        assert!(SplineInterpolation::new(&[1.0], &[1.0]).is_err());
        let s = SplineInterpolation::with_end_slopes(&[0.0, 1.0, 2.0], &[0.0, 1.0, 4.0], 0.0, 4.0)
            .unwrap();
        // clamped y = x^2 has slope 0 at 0: derivative honours the end slope
        assert!(s.sample_derivative(0.0).abs() < 1e-12);
        assert!((s.sample(1.0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn monotone_cubic_is_monotone_and_rejects_two_points() {
        assert!(MonotoneCubicInterpolation::new(&[1.0, 2.0], &[1.0, 2.0]).is_err());
        assert!(MonotoneCubicInterpolation::new(&[1.0, 2.0, 2.0], &[1.0, 2.0, 3.0]).is_err());
        assert!(MonotoneCubicInterpolation::new(&[1.0, 2.0, 3.0], &[1.0, 2.0]).is_err());
        // a step-like monotone data set that a natural spline would overshoot
        let x = [0.0, 1.0, 2.0, 3.0, 4.0];
        let y = [0.0, 0.0, 0.0, 1.0, 1.0];
        let m = MonotoneCubicInterpolation::new(&x, &y).unwrap();
        let mut prev = m.sample(0.0);
        for i in 1..=400 {
            let v = m.sample(f64::from(i) * 0.01);
            assert!(v >= prev - 1e-15 && (-1e-15..=1.0 + 1e-15).contains(&v));
            prev = v;
        }
        assert!(m.sample_derivative(2.5) > 0.0);
        assert_eq!(m.knot_slopes().len(), 5);
        assert_eq!(m.sample(1.0), 0.0);
    }

    /// Pins MOOSE's interior-knot slope formula AS WRITTEN. At d7c4bde,
    /// `MonotoneCubicInterpolation::initialize_derivs` divides the interior
    /// three-point numerator by `h[i-1] * h[i] * (h[i-1] * h[i])`, where the
    /// end-knot formulas (and the correct three-point derivative) use
    /// `(h[i-1] + h[i])`. For y = x^2 on unit spacing the true slopes are
    /// 2 and 4; upstream gives 4 and 8 before limiting and [0, 3, 8, 6]
    /// after. The translation reproduces upstream, because Moltres' gold
    /// `gmm_monotone_cubic` / `mjm_monotone_cubic` k_eff were produced by it
    /// (S5/S6 match them to ~1e-14). Reported on gh:#1538, not corrected.
    #[test]
    fn monotone_cubic_keeps_moose_interior_slope_formula() {
        let x = [0.0, 1.0, 2.0, 3.0];
        let y = [0.0, 1.0, 4.0, 9.0];
        let m = MonotoneCubicInterpolation::new(&x, &y).unwrap();
        assert_eq!(m.knot_slopes(), &[0.0, 3.0, 8.0, 6.0]);
        for (xi, yi) in x.iter().zip(&y) {
            assert!((m.sample(*xi) - yi).abs() < 1e-12);
        }
    }

    #[test]
    fn bicubic_reproduces_a_bilinear_surface() {
        let x1 = [0.0, 1.0, 2.0, 3.0];
        let x2 = [10.0, 20.0, 30.0];
        let y: Vec<Vec<f64>> = x1
            .iter()
            .map(|a| x2.iter().map(|b| 1.0 + 2.0 * a + 0.5 * b + 0.1 * a * b).collect())
            .collect();
        let s = BicubicSplineInterpolation::new(&x1, &x2, &y).unwrap();
        let (a, b) = (1.3, 17.0);
        let exact = 1.0 + 2.0 * a + 0.5 * b + 0.1 * a * b;
        assert!((s.sample(a, b) - exact).abs() < 1e-9);
        assert!((s.sample_derivative(a, b, 1) - (2.0 + 0.1 * b)).abs() < 1e-9);
        assert!((s.sample_derivative(a, b, 2) - (0.5 + 0.1 * a)).abs() < 1e-9);
        assert!(s.sample_derivative(a, b, 3).is_nan());
        assert!(BicubicSplineInterpolation::new(&x1, &x2, &y[..3]).is_err());
        let mut ragged = y.clone();
        ragged[1].pop();
        assert!(BicubicSplineInterpolation::new(&x1, &x2, &ragged).is_err());
    }
}
