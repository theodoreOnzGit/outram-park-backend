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
use crate::zip::zip2;
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

/// `v[i]`, or `NaN` if `i` is out of range.
///
/// Every call is in range by construction (lengths are validated in
/// [`PiecewiseInterp::new`] and the loops stay inside them). The fallback
/// exists so the crate's no-subscript gate (`tests/no_panic_gate.rs`) holds: a
/// violated invariant would surface as `NaN`, not a panic.
fn at(v: &[f64], i: usize) -> f64 {
    v.get(i).copied().unwrap_or(f64::NAN)
}

/// `v[i] = x`, ignored if `i` is out of range (same invariant as [`at`]).
fn put(v: &mut [f64], i: usize, x: f64) {
    if let Some(slot) = v.get_mut(i) {
        *slot = x;
    }
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
        for (&lo, &hi) in zip2(xa.iter(), xa.iter().skip(1)) {
            if !(lo < hi) {
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
        let h0 = at(x, 1) - at(x, 0);
        let s0 = (at(y, 1) - at(y, 0)) / h0;
        put(&mut y_prime, 0, s0);
        for i in 1..(size - 1) {
            let hi = at(x, i + 1) - at(x, i);
            let him1 = at(x, i) - at(x, i - 1);
            let si = (at(y, i + 1) - at(y, i)) / hi;
            let sim1 = (at(y, i) - at(y, i - 1)) / him1;
            let pi = (sim1 * hi + si * him1) / (him1 + hi);
            put(
                &mut y_prime,
                i,
                (steffen_copysign(1.0, sim1) + steffen_copysign(1.0, si))
                    * gsl_min(sim1.abs(), gsl_min(si.abs(), 0.5 * pi.abs())),
            );
        }
        put(
            &mut y_prime,
            size - 1,
            (at(y, size - 1) - at(y, size - 2)) / (at(x, size - 1) - at(x, size - 2)),
        );

        let mut a = vec![0.0_f64; size];
        for i in 0..(size - 1) {
            let hi = at(x, i + 1) - at(x, i);
            let si = (at(y, i + 1) - at(y, i)) / hi;
            let (yp0, yp1) = (at(&y_prime, i), at(&y_prime, i + 1));
            put(&mut a, i, (yp0 + yp1 - 2.0 * si) / hi / hi);
            put(&mut self.b, i, (3.0 * si - 2.0 * yp0 - yp1) / hi);
            put(&mut self.c, i, yp0);
            put(&mut self.d, i, at(y, i));
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
            put(
                &mut mm,
                i + 2,
                (at(y, i + 1) - at(y, i)) / (at(x, i + 1) - at(x, i)),
            );
        }
        // upstream m[0], m[1], m[size-3], m[size-2] (= mm[2], mm[3],
        // mm[size-1], mm[size]).
        let (m0, m1) = (at(&mm, 2), at(&mm, 3));
        let (mn3, mn2) = (at(&mm, size - 1), at(&mm, size));
        if periodic {
            put(&mut mm, 0, mn3);
            put(&mut mm, 1, mn2);
            put(&mut mm, size + 1, m0);
            put(&mut mm, size + 2, m1);
        } else {
            put(&mut mm, 0, 3.0 * m0 - 2.0 * m1);
            put(&mut mm, 1, 2.0 * m0 - m1);
            put(&mut mm, size + 1, 2.0 * mn2 - mn3);
            put(&mut mm, size + 2, 3.0 * mn2 - 2.0 * mn3);
        }
        // m(k) = upstream m[k] for k in -2..=size.
        let m = |k: isize| at(&mm, (k + 2) as usize);
        for i in 0..(size - 1) {
            let ii = i as isize;
            let ne = (m(ii + 1) - m(ii)).abs() + (m(ii - 1) - m(ii - 2)).abs();
            if ne == 0.0 {
                put(&mut self.b, i, m(ii));
                put(&mut self.c, i, 0.0);
                put(&mut self.d, i, 0.0);
            } else {
                let h_i = at(x, i + 1) - at(x, i);
                let ne_next = (m(ii + 2) - m(ii + 1)).abs() + (m(ii) - m(ii - 1)).abs();
                let alpha_i = (m(ii - 1) - m(ii - 2)).abs() / ne;
                let tl_ip1 = if ne_next == 0.0 {
                    m(ii)
                } else {
                    let alpha_ip1 = (m(ii) - m(ii - 1)).abs() / ne_next;
                    (1.0 - alpha_ip1) * m(ii) + alpha_ip1 * m(ii + 1)
                };
                let b_i = (1.0 - alpha_i) * m(ii - 1) + alpha_i * m(ii);
                put(&mut self.b, i, b_i);
                put(&mut self.c, i, (3.0 * m(ii) - 2.0 * b_i - tl_ip1) / h_i);
                put(&mut self.d, i, (b_i + tl_ip1 - 2.0 * m(ii)) / (h_i * h_i));
            }
        }
    }

    /// `gsl_interp_bsearch` (as in [`crate::interp::Interp`]): `i` with
    /// `xa[i] <= x < xa[i+1]` and `i <= n - 2`.
    fn bracket_index(&self, x: f64) -> usize {
        let mut ilo = 0usize;
        let mut ihi = self.xa.len().saturating_sub(1);
        while ihi > ilo + 1 {
            let i = (ihi + ilo) / 2;
            if at(&self.xa, i) > x {
                ihi = i;
            } else {
                ilo = i;
            }
        }
        ilo
    }

    fn check_range(&self, x: f64) -> Result<()> {
        let (Some(&lo), Some(&hi)) = (self.xa.first(), self.xa.last()) else {
            return Err(PetirError::Invalid);
        };
        if !(x >= lo && x <= hi) {
            return Err(PetirError::Domain);
        }
        Ok(())
    }

    /// The segment index of `x` and its coefficients `(i, delx, a, b, c, d)`.
    fn segment(&self, x: f64) -> Result<(usize, f64, f64, f64, f64, f64)> {
        self.check_range(x)?;
        let i = self.bracket_index(x);
        let delx = x - at(&self.xa, i);
        Ok((
            i,
            delx,
            at(&self.a, i),
            at(&self.b, i),
            at(&self.c, i),
            at(&self.d, i),
        ))
    }

    /// Interpolate at `x`: `steffen_eval` (steffen.c:182-215) or `akima_eval`
    /// (akima.c:192-226).
    ///
    /// # Errors
    ///
    /// [`PetirError::Domain`] if `x` is outside `[x[0], x[n-1]]` or `NaN`.
    pub fn eval(&self, x: f64) -> Result<f64> {
        let (i, delx, a, b, c, d) = self.segment(x)?;
        Ok(match self.kind {
            PiecewiseKind::Steffen => d + delx * (c + delx * (b + delx * a)),
            _ => at(&self.ya, i) + delx * (b + delx * (c + d * delx)),
        })
    }

    /// First derivative at `x`: `steffen_eval_deriv` (`c + delx*(2*b +
    /// delx*3*a)`) or `akima_eval_deriv` (`b + delx*(2.0*c + 3.0*d*delx)`).
    ///
    /// # Errors
    ///
    /// As [`Self::eval`].
    pub fn eval_deriv(&self, x: f64) -> Result<f64> {
        let (_, delx, a, b, c, d) = self.segment(x)?;
        Ok(match self.kind {
            PiecewiseKind::Steffen => c + delx * (2.0 * b + delx * 3.0 * a),
            _ => b + delx * (2.0 * c + 3.0 * d * delx),
        })
    }

    /// Second derivative at `x`: `6*a*delx + 2*b` (Steffen) or `2.0*c +
    /// 6.0*d*delx` (Akima). Discontinuous at the knots.
    ///
    /// # Errors
    ///
    /// As [`Self::eval`].
    pub fn eval_deriv2(&self, x: f64) -> Result<f64> {
        let (_, delx, a, b, c, d) = self.segment(x)?;
        Ok(match self.kind {
            PiecewiseKind::Steffen => 6.0 * a * delx + 2.0 * b,
            _ => 2.0 * c + 6.0 * d * delx,
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
            let x_hi = at(&self.xa, i + 1);
            let x_lo = at(&self.xa, i);
            let dx = x_hi - x_lo;
            let (ca, cb, cc, cd) = (
                at(&self.a, i),
                at(&self.b, i),
                at(&self.c, i),
                at(&self.d, i),
            );
            match self.kind {
                PiecewiseKind::Steffen => {
                    let x1 = if i == index_a { a - x_lo } else { 0.0 };
                    let x2 = if i == index_b { b - x_lo } else { x_hi - x_lo };
                    result += (1.0 / 4.0) * ca * (x2 * x2 * x2 * x2 - x1 * x1 * x1 * x1)
                        + (1.0 / 3.0) * cb * (x2 * x2 * x2 - x1 * x1 * x1)
                        + (1.0 / 2.0) * cc * (x2 * x2 - x1 * x1)
                        + cd * (x2 - x1);
                }
                _ => {
                    let y_lo = at(&self.ya, i);
                    if i == index_a || i == index_b {
                        let x1 = if i == index_a { a } else { x_lo };
                        let x2 = if i == index_b { b } else { x_hi };
                        result += integ_eval(y_lo, cb, cc, cd, x_lo, x1, x2);
                    } else {
                        result += dx * (y_lo + dx * (0.5 * cb + dx * (cc / 3.0 + 0.25 * cd * dx)));
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
