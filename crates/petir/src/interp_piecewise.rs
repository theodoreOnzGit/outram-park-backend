//! Steffen and Akima piecewise-cubic interpolation, ported from GSL 2.8
//! (gh:#1341).
//!
//! Upstream: GSL 2.8, `ampl/gsl` commit `cf180cd7`, `interpolation/steffen.c`
//! (Steffen 1990, "A simple method for monotonic interpolation in one
//! dimension", Astron. Astrophys. 239, 443; modelled by its GSL author on
//! Akima) and `interpolation/akima.c` (Akima 1970, J. ACM 17, 589), plus the
//! shared `interpolation/integ_eval.h`. Copyright the GSL authors
//! (GPL-3.0-or-later). Research, education and V&V only.
//!
//! # What each method is for
//!
//! - [`PiecewiseKind::Steffen`] is **monotone**: between two data points the
//!   interpolant is monotone, so it never overshoots; its first derivative is
//!   continuous, its second is not. This is the natural choice for property
//!   tables, where the natural cubic spline of [`crate::interp`] can ring.
//! - [`PiecewiseKind::Akima`] and [`PiecewiseKind::AkimaPeriodic`] are the
//!   local, low-overshoot Akima spline (not monotone), with end slopes
//!   extrapolated (non-periodic) or wrapped (periodic, which expects
//!   `y[0] == y[n-1]`; GSL does not check that and neither does this).
//!
//! # Relation to [`crate::interp`]
//!
//! Additive. [`crate::interp::Interp`] is untouched; these live in their own
//! type so no reviewed function changes. Out-of-range arguments are refused as
//! there ([`PetirError::Domain`]), `a > b` in [`PiecewiseInterp::eval_integ`]
//! is refused as GSL's `gsl_interp_eval_integ` does, and a repeated abscissa is
//! rejected at construction.
//!
//! # Bit-identity
//!
//! Every expression keeps upstream's grouping (`(sim1*hi + si*him1) /
//! (him1 + hi)`, `(y' + y'' - 2*si) / hi / hi`, Horner order, the
//! `1/4, 1/3, 1/2` integration weights). No libm function is involved, so the
//! comparison against compiled GSL
//! (`tests/gsl_interp_piecewise_code_to_code.rs`) is against stock GSL with no
//! shim.
//!
//! # Units
//!
//! Bare dimensionless `f64`.

use alloc::vec;
use alloc::vec::Vec;

#[allow(unused_imports)]
use crate::real::Real;

use crate::interp::integ_eval;
use crate::{PetirError, Result};

/// Which piecewise cubic a [`PiecewiseInterp`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiecewiseKind {
    /// Steffen's monotone interpolation, GSL `gsl_interp_steffen`.
    Steffen,
    /// Akima spline with extrapolated end slopes, GSL `gsl_interp_akima`.
    Akima,
    /// Akima spline with periodic end slopes, GSL `gsl_interp_akima_periodic`.
    AkimaPeriodic,
}

impl PiecewiseKind {
    /// The fewest points GSL accepts (`min_size` of the `gsl_interp_type`):
    /// 3 for Steffen, 5 for both Akima types.
    pub fn min_size(self) -> usize {
        match self {
            PiecewiseKind::Steffen => 3,
            PiecewiseKind::Akima | PiecewiseKind::AkimaPeriodic => 5,
        }
    }
}

/// A Steffen or Akima interpolant over `(x, y)` data.
///
/// # Example
///
/// ```
/// use petir::interp_piecewise::{PiecewiseInterp, PiecewiseKind};
/// // A step-like table: the natural cubic spline overshoots here, Steffen
/// // stays inside [0, 1].
/// let x = [0.0, 1.0, 2.0, 3.0, 4.0];
/// let y = [0.0, 0.0, 1.0, 1.0, 1.0];
/// let s = PiecewiseInterp::new(PiecewiseKind::Steffen, &x, &y).unwrap();
/// for k in 0..=40 {
///     let v = s.eval(k as f64 * 0.1).unwrap();
///     assert!((0.0..=1.0).contains(&v));
/// }
/// assert_eq!(s.eval(2.0).unwrap(), 1.0);
/// ```
#[derive(Debug, Clone)]
pub struct PiecewiseInterp {
    kind: PiecewiseKind,
    xa: Vec<f64>,
    ya: Vec<f64>,
    /// Steffen: `a, b, c, d` of eqs 2-5, `y = d + r(c + r(b + r a))`.
    /// Akima: `b, c, d` with `y = y_i + r(b + r(c + d r))`; `a` is empty.
    a: Vec<f64>,
    b: Vec<f64>,
    c: Vec<f64>,
    d: Vec<f64>,
}

/// `steffen_copysign` (steffen.c:317): flips `x` only when the signs differ
/// strictly, so a zero `y` leaves `x` alone (unlike `copysign`).
fn steffen_copysign(x: f64, y: f64) -> f64 {
    if (x < 0.0 && y > 0.0) || (x > 0.0 && y < 0.0) {
        -x
    } else {
        x
    }
}

/// `GSL_MIN_DBL` (`gsl_math.h`): `a < b ? a : b`.
fn gsl_min(a: f64, b: f64) -> f64 {
    if a < b {
        a
    } else {
        b
    }
}

impl PiecewiseInterp {
    /// Build the interpolant (`gsl_interp_init`, then the method's `init`).
    ///
    /// # Errors
    ///
    /// - [`PetirError::LengthMismatch`] if `xa` and `ya` differ in length.
    /// - [`PetirError::Invalid`] if there are fewer points than
    ///   [`PiecewiseKind::min_size`].
    /// - [`PetirError::Domain`] if `xa` is not strictly increasing (GSL:
    ///   `GSL_EINVAL`).
    pub fn new(kind: PiecewiseKind, xa: &[f64], ya: &[f64]) -> Result<Self> {
        if xa.len() != ya.len() {
            return Err(PetirError::LengthMismatch {
                expected: xa.len(),
                found: ya.len(),
            });
        }
        let n = xa.len();
        if n < kind.min_size() {
            return Err(PetirError::Invalid);
        }
        for i in 1..n {
            if !(xa[i - 1] < xa[i]) {
                return Err(PetirError::Domain);
            }
        }
        let mut s = Self {
            kind,
            xa: xa.to_vec(),
            ya: ya.to_vec(),
            a: Vec::new(),
            b: vec![0.0; n],
            c: vec![0.0; n],
            d: vec![0.0; n],
        };
        match kind {
            PiecewiseKind::Steffen => s.steffen_init(),
            PiecewiseKind::Akima => s.akima_init(false),
            PiecewiseKind::AkimaPeriodic => s.akima_init(true),
        }
        Ok(s)
    }

    /// `steffen_init` (steffen.c:101-166).
    fn steffen_init(&mut self) {
        let size = self.xa.len();
        let x = &self.xa;
        let y = &self.ya;
        let mut y_prime = vec![0.0_f64; size];
        let h0 = x[1] - x[0];
        let s0 = (y[1] - y[0]) / h0;
        y_prime[0] = s0;
        for i in 1..(size - 1) {
            let hi = x[i + 1] - x[i];
            let him1 = x[i] - x[i - 1];
            let si = (y[i + 1] - y[i]) / hi;
            let sim1 = (y[i] - y[i - 1]) / him1;
            let pi = (sim1 * hi + si * him1) / (him1 + hi);
            y_prime[i] = (steffen_copysign(1.0, sim1) + steffen_copysign(1.0, si))
                * gsl_min(sim1.abs(), gsl_min(si.abs(), 0.5 * pi.abs()));
        }
        y_prime[size - 1] = (y[size - 1] - y[size - 2]) / (x[size - 1] - x[size - 2]);

        let mut a = vec![0.0_f64; size];
        for i in 0..(size - 1) {
            let hi = x[i + 1] - x[i];
            let si = (y[i + 1] - y[i]) / hi;
            a[i] = (y_prime[i] + y_prime[i + 1] - 2.0 * si) / hi / hi;
            self.b[i] = (3.0 * si - 2.0 * y_prime[i] - y_prime[i + 1]) / hi;
            self.c[i] = y_prime[i];
            self.d[i] = y[i];
        }
        self.a = a;
    }

    /// `akima_init` / `akima_init_periodic` (akima.c:131-190) and
    /// `akima_calc` (akima.c:84-129). Upstream addresses `m[-2..size]` through
    /// a pointer offset by 2; here `m[k]` of upstream is `mm[k + 2]`.
    fn akima_init(&mut self, periodic: bool) {
        let size = self.xa.len();
        let x = &self.xa;
        let y = &self.ya;
        let mut mm = vec![0.0_f64; size + 4];
        for i in 0..=(size - 2) {
            mm[i + 2] = (y[i + 1] - y[i]) / (x[i + 1] - x[i]);
        }
        if periodic {
            mm[0] = mm[size - 1 - 2 + 2];
            mm[1] = mm[size - 1 - 1 + 2];
            mm[size - 1 + 2] = mm[2];
            mm[size + 2] = mm[3];
        } else {
            mm[0] = 3.0 * mm[2] - 2.0 * mm[3];
            mm[1] = 2.0 * mm[2] - mm[3];
            mm[size - 1 + 2] = 2.0 * mm[size - 2 + 2] - mm[size - 3 + 2];
            mm[size + 2] = 3.0 * mm[size - 2 + 2] - 2.0 * mm[size - 3 + 2];
        }
        // m(k) = upstream m[k], k in -2..=size.
        let m = |k: isize| mm[(k + 2) as usize];
        for i in 0..(size - 1) {
            let ii = i as isize;
            let ne = (m(ii + 1) - m(ii)).abs() + (m(ii - 1) - m(ii - 2)).abs();
            if ne == 0.0 {
                self.b[i] = m(ii);
                self.c[i] = 0.0;
                self.d[i] = 0.0;
            } else {
                let h_i = x[i + 1] - x[i];
                let ne_next = (m(ii + 2) - m(ii + 1)).abs() + (m(ii) - m(ii - 1)).abs();
                let alpha_i = (m(ii - 1) - m(ii - 2)).abs() / ne;
                let tl_ip1 = if ne_next == 0.0 {
                    m(ii)
                } else {
                    let alpha_ip1 = (m(ii) - m(ii - 1)).abs() / ne_next;
                    (1.0 - alpha_ip1) * m(ii) + alpha_ip1 * m(ii + 1)
                };
                self.b[i] = (1.0 - alpha_i) * m(ii - 1) + alpha_i * m(ii);
                self.c[i] = (3.0 * m(ii) - 2.0 * self.b[i] - tl_ip1) / h_i;
                self.d[i] = (self.b[i] + tl_ip1 - 2.0 * m(ii)) / (h_i * h_i);
            }
        }
    }

    /// `gsl_interp_bsearch` (as in [`crate::interp::Interp`]): `i` with
    /// `xa[i] <= x < xa[i+1]` and `i <= n - 2`.
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

    fn check_range(&self, x: f64) -> Result<()> {
        let lo = self.xa[0];
        let hi = self.xa[self.xa.len() - 1];
        if !(x >= lo && x <= hi) {
            return Err(PetirError::Domain);
        }
        Ok(())
    }

    /// Interpolate at `x`: `steffen_eval` (steffen.c:182-215) or `akima_eval`
    /// (akima.c:192-226).
    ///
    /// # Errors
    ///
    /// [`PetirError::Domain`] if `x` is outside `[x[0], x[n-1]]` or `NaN`.
    pub fn eval(&self, x: f64) -> Result<f64> {
        self.check_range(x)?;
        let i = self.bracket_index(x);
        let delx = x - self.xa[i];
        Ok(match self.kind {
            PiecewiseKind::Steffen => {
                self.d[i] + delx * (self.c[i] + delx * (self.b[i] + delx * self.a[i]))
            }
            _ => self.ya[i] + delx * (self.b[i] + delx * (self.c[i] + self.d[i] * delx)),
        })
    }

    /// First derivative at `x`: `steffen_eval_deriv` (`c + delx*(2*b +
    /// delx*3*a)`) or `akima_eval_deriv` (`b + delx*(2.0*c + 3.0*d*delx)`).
    ///
    /// # Errors
    ///
    /// As [`Self::eval`].
    pub fn eval_deriv(&self, x: f64) -> Result<f64> {
        self.check_range(x)?;
        let i = self.bracket_index(x);
        let delx = x - self.xa[i];
        Ok(match self.kind {
            PiecewiseKind::Steffen => self.c[i] + delx * (2.0 * self.b[i] + delx * 3.0 * self.a[i]),
            _ => self.b[i] + delx * (2.0 * self.c[i] + 3.0 * self.d[i] * delx),
        })
    }

    /// Second derivative at `x`: `6*a*delx + 2*b` (Steffen) or `2.0*c +
    /// 6.0*d*delx` (Akima). Discontinuous at the knots.
    ///
    /// # Errors
    ///
    /// As [`Self::eval`].
    pub fn eval_deriv2(&self, x: f64) -> Result<f64> {
        self.check_range(x)?;
        let i = self.bracket_index(x);
        let delx = x - self.xa[i];
        Ok(match self.kind {
            PiecewiseKind::Steffen => 6.0 * self.a[i] * delx + 2.0 * self.b[i],
            _ => 2.0 * self.c[i] + 6.0 * self.d[i] * delx,
        })
    }

    /// Integral of the interpolant from `a` to `b`: `steffen_eval_integ`
    /// (steffen.c:268-315) or `akima_eval_integ` (akima.c:290-346).
    ///
    /// # Errors
    ///
    /// [`PetirError::Domain`] if `a > b`, a limit is outside the table or
    /// `NaN` (GSL's `gsl_interp_eval_integ` guard; `NaN` is refused here where
    /// GSL lets it through, as in [`crate::interp::Interp::eval_integ`]).
    pub fn eval_integ(&self, a: f64, b: f64) -> Result<f64> {
        self.check_range(a)?;
        self.check_range(b)?;
        if a > b {
            return Err(PetirError::Domain);
        }
        if a == b {
            return Ok(0.0);
        }
        let index_a = self.bracket_index(a);
        let index_b = self.bracket_index(b);
        let mut result = 0.0_f64;
        for i in index_a..=index_b {
            let x_hi = self.xa[i + 1];
            let x_lo = self.xa[i];
            let dx = x_hi - x_lo;
            match self.kind {
                PiecewiseKind::Steffen => {
                    let x1 = if i == index_a { a - x_lo } else { 0.0 };
                    let x2 = if i == index_b { b - x_lo } else { x_hi - x_lo };
                    result += (1.0 / 4.0) * self.a[i] * (x2 * x2 * x2 * x2 - x1 * x1 * x1 * x1)
                        + (1.0 / 3.0) * self.b[i] * (x2 * x2 * x2 - x1 * x1 * x1)
                        + (1.0 / 2.0) * self.c[i] * (x2 * x2 - x1 * x1)
                        + self.d[i] * (x2 - x1);
                }
                _ => {
                    let y_lo = self.ya[i];
                    if i == index_a || i == index_b {
                        let x1 = if i == index_a { a } else { x_lo };
                        let x2 = if i == index_b { b } else { x_hi };
                        result += integ_eval(y_lo, self.b[i], self.c[i], self.d[i], x_lo, x1, x2);
                    } else {
                        result += dx
                            * (y_lo
                                + dx * (0.5 * self.b[i]
                                    + dx * (self.c[i] / 3.0 + 0.25 * self.d[i] * dx)));
                    }
                }
            }
        }
        Ok(result)
    }

    /// The method.
    pub fn kind(&self) -> PiecewiseKind {
        self.kind
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_bad_input() {
        let x = [0.0, 1.0, 2.0];
        let y = [0.0, 1.0, 4.0];
        assert!(PiecewiseInterp::new(PiecewiseKind::Akima, &x, &y).is_err());
        assert!(PiecewiseInterp::new(PiecewiseKind::Steffen, &x, &[1.0]).is_err());
        assert!(PiecewiseInterp::new(PiecewiseKind::Steffen, &[0.0, 0.0, 1.0], &y).is_err());
        let s = PiecewiseInterp::new(PiecewiseKind::Steffen, &x, &y).unwrap();
        assert!(s.eval(3.0).is_err() && s.eval(f64::NAN).is_err());
        assert!(s.eval_integ(1.5, 0.5).is_err());
        assert_eq!(s.eval_integ(1.0, 1.0).unwrap(), 0.0);
        assert_eq!(s.kind(), PiecewiseKind::Steffen);
    }

    /// Both methods reproduce a straight line exactly (all slopes equal), and
    /// the integral of `y = 2x + 1` over `[0.5, 3.5]` is `x^2 + x` there = 15.
    #[test]
    fn straight_line_is_exact_and_integrates() {
        let x = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
        let y: Vec<f64> = x.iter().map(|v| 2.0 * v + 1.0).collect();
        for k in [PiecewiseKind::Steffen, PiecewiseKind::Akima, PiecewiseKind::AkimaPeriodic] {
            let s = PiecewiseInterp::new(k, &x, &y).unwrap();
            assert!((s.eval(2.5).unwrap() - 6.0).abs() < 1e-14);
            assert!((s.eval_deriv(2.5).unwrap() - 2.0).abs() < 1e-14);
            assert!(s.eval_deriv2(2.5).unwrap().abs() < 1e-13);
            assert!((s.eval_integ(0.5, 3.5).unwrap() - 15.0).abs() < 1e-12);
        }
    }

    /// Steffen is monotone: on monotone data the interpolant never decreases.
    #[test]
    fn steffen_never_overshoots_a_step() {
        let x = [0.0, 1.0, 2.0, 3.0, 4.0];
        let y = [0.0, 0.0, 1.0, 1.0, 1.0];
        let s = PiecewiseInterp::new(PiecewiseKind::Steffen, &x, &y).unwrap();
        let mut prev = 0.0;
        for k in 0..=400 {
            let v = s.eval(k as f64 * 0.01).unwrap();
            assert!(v >= prev - 1e-15 && (0.0..=1.0).contains(&v));
            prev = v;
        }
    }
}
