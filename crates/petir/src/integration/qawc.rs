//! QAWC: adaptive integration for Cauchy principal values, ported from GSL
//! 2.8 `integration/qawc.c` (lines 30-220) and `qc25c.c` (lines 20-132),
//! commit `cf180cd7`, read 2026-11-13. Copyright (C) 1996-2000, 2007 Brian
//! Gough (GPL-3.0-or-later); GSL's QAWC is QUADPACK's `dqawce` (public
//! domain). gh:#1340. Research, education and V&V only.
//!
//! Computes the principal value `PV int_a^b f(x) / (x - c) dx` for `c`
//! strictly inside `(a, b)` (or outside, where the kernel is regular). Each
//! sub-interval is integrated by `qc25c`: the 25-point Clenshaw-Curtis rule
//! with the exact Cauchy moments when `|cc| <= 1.1` (the singularity is on or
//! near the interval), otherwise a 15-point Gauss-Kronrod rule on
//! `f(x) / (x - c)`. There is no epsilon extrapolation in QAWC.
//!
//! # Differences from upstream
//!
//! - The workspace is allocated inside the call with `limit` entries.
//! - Non-finite `a`, `b`, `c` are [`PetirError::Domain`] (upstream does not
//!   check); `limit == 0` is [`PetirError::Invalid`].
//! - Every array access is a checked `get`.

#[allow(unused_imports)]
use crate::real::Real;
use crate::scalar::DBL_EPSILON;
use crate::{PetirError, Result};

use super::qags::{subinterval_too_small, QagsOutcome, Workspace};
use super::qcheb::qcheb;
use super::{kronrod, Integral, QkRule};

fn g(v: &[f64], i: usize) -> f64 {
    v.get(i).copied().unwrap_or(0.0)
}

fn s(v: &mut [f64], i: usize, x: f64) {
    if let Some(p) = v.get_mut(i) {
        *p = x;
    }
}

/// `qc25c.c`, `compute_moments`: the Chebyshev moments of `1 / (x - cc)`.
fn compute_moments(cc: f64) -> [f64; 25] {
    let mut moment = [0.0_f64; 25];
    let mut a0 = Real::ln(((1.0 - cc) / (1.0 + cc)).abs());
    let mut a1 = 2.0 + a0 * cc;

    s(&mut moment, 0, a0);
    s(&mut moment, 1, a1);

    for k in 2..25usize {
        let a2 = if k % 2 == 0 {
            2.0 * cc * a1 - a0
        } else {
            let km1 = (k as f64) - 1.0;
            2.0 * cc * a1 - a0 - 4.0 / (km1 * km1 - 1.0)
        };
        s(&mut moment, k, a2);
        a0 = a1;
        a1 = a2;
    }
    moment
}

/// `qc25c.c`, `qc25c`: `(result, abserr, err_reliable)` on `[a, b]`.
pub(super) fn qc25c<F>(f: &F, a: f64, b: f64, c: f64) -> (f64, f64, bool)
where
    F: Fn(f64) -> f64,
{
    let cc = (2.0 * c - b - a) / (b - a);

    if cc.abs() > 1.1 {
        let weighted = |x: f64| f(x) / (x - c);
        let r = kronrod(QkRule::Qk15, weighted, a, b);
        let reliable = r.abserr != r.resasc;
        (r.result, r.abserr, reliable)
    } else {
        let (cheb12, cheb24) = qcheb(f, a, b);
        let moment = compute_moments(cc);
        let mut res12 = 0.0;
        let mut res24 = 0.0;
        for i in 0..13 {
            res12 += g(&cheb12, i) * g(&moment, i);
        }
        for i in 0..25 {
            res24 += g(&cheb24, i) * g(&moment, i);
        }
        (res24, (res24 - res12).abs(), false)
    }
}

/// `gsl_integration_qawc`: the Cauchy principal value
/// `PV int_a^b f(x) / (x - c) dx`.
///
/// # Errors
///
/// As [`super::qags`]; [`PetirError::Invalid`] when `c == a` or `c == b`
/// (`GSL_EINVAL`).
///
/// # Example
///
/// ```
/// use petir::integration::qawc;
/// // PV int_{-1}^{2} 1/(x - 0) dx = ln 2.
/// let r = qawc(|_x: f64| 1.0, -1.0, 2.0, 0.0, 0.0, 1e-10, 100).unwrap();
/// assert!((r.value - core::f64::consts::LN_2).abs() < 1e-9);
/// ```
pub fn qawc<F>(
    f: F,
    a: f64,
    b: f64,
    c: f64,
    epsabs: f64,
    epsrel: f64,
    limit: usize,
) -> Result<Integral>
where
    F: Fn(f64) -> f64,
{
    let o = qawc_with_status(f, a, b, c, epsabs, epsrel, limit);
    o.status.map(|()| o.integral)
}

/// [`qawc`], keeping GSL's estimate when the status is an error.
pub fn qawc_with_status<F>(
    f: F,
    a: f64,
    b: f64,
    c: f64,
    epsabs: f64,
    epsrel: f64,
    limit: usize,
) -> QagsOutcome
where
    F: Fn(f64) -> f64,
{
    let bad = |e: PetirError| QagsOutcome {
        integral: Integral {
            value: 0.0,
            abserr: 0.0,
            subintervals: 0,
        },
        status: Err(e),
    };
    if !(a.is_finite() && b.is_finite() && c.is_finite()) {
        return bad(PetirError::Domain);
    }
    if limit == 0 {
        return bad(PetirError::Invalid);
    }
    match qawc_core(&f, a, b, c, epsabs, epsrel, limit) {
        Some(o) => o,
        None => bad(PetirError::Invalid),
    }
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn qawc_core<F>(
    f: &F,
    a: f64,
    b: f64,
    c: f64,
    epsabs: f64,
    epsrel: f64,
    limit: usize,
) -> Option<QagsOutcome>
where
    F: Fn(f64) -> f64,
{
    let mut roundoff_type1: u32 = 0;
    let mut roundoff_type2: u32 = 0;
    let mut error_type: u32 = 0;
    let mut sign = 1.0_f64;

    let (lower, higher) = if b < a {
        sign = -1.0;
        (b, a)
    } else {
        (a, b)
    };

    let mut workspace = Workspace::new(limit, lower, higher)?;
    let zero = |e: PetirError| QagsOutcome {
        integral: Integral {
            value: 0.0,
            abserr: 0.0,
            subintervals: 0,
        },
        status: Err(e),
    };

    if epsabs <= 0.0 && (epsrel < 50.0 * DBL_EPSILON || epsrel < 0.5e-28) {
        return Some(zero(PetirError::Tolerance));
    }

    if c == a || c == b {
        return Some(zero(PetirError::Invalid));
    }

    // Perform the first integration.
    let (result0, abserr0, _reliable0) = qc25c(f, lower, higher, c);

    workspace.set_initial_result(result0, abserr0)?;

    // Test on accuracy, with 0.01 relative error as an extra safety margin
    // on the first iteration.
    let mut tolerance = epsabs.max(epsrel * result0.abs());

    let out = |value: f64, abserr: f64, w: &Workspace, status: Result<()>| QagsOutcome {
        integral: Integral {
            value,
            abserr,
            subintervals: w.size,
        },
        status,
    };

    if abserr0 < tolerance && abserr0 < 0.01 * result0.abs() {
        return Some(out(sign * result0, abserr0, &workspace, Ok(())));
    } else if limit == 1 {
        return Some(out(
            sign * result0,
            abserr0,
            &workspace,
            Err(PetirError::MaxIterations),
        ));
    }

    let mut area = result0;
    let mut errsum = abserr0;
    let mut iteration: usize = 1;

    loop {
        // Bisect the subinterval with the largest error estimate.
        let (a_i, b_i, r_i, e_i) = workspace.retrieve()?;

        let a1 = a_i;
        let mut b1 = 0.5 * (a_i + b_i);
        let mut a2 = b1;
        let b2 = b_i;

        if c > a1 && c <= b1 {
            b1 = 0.5 * (c + b2);
            a2 = b1;
        } else if c > b1 && c < b2 {
            b1 = 0.5 * (a1 + c);
            a2 = b1;
        }

        let (area1, error1, reliable1) = qc25c(f, a1, b1, c);
        let (area2, error2, reliable2) = qc25c(f, a2, b2, c);

        let area12 = area1 + area2;
        let error12 = error1 + error2;

        errsum += error12 - e_i;
        area += area12 - r_i;

        if reliable1 && reliable2 {
            let delta = r_i - area12;

            if delta.abs() <= 1.0e-5 * area12.abs() && error12 >= 0.99 * e_i {
                roundoff_type1 += 1;
            }
            if iteration >= 10 && error12 > e_i {
                roundoff_type2 += 1;
            }
        }

        tolerance = epsabs.max(epsrel * area.abs());

        if errsum > tolerance {
            if roundoff_type1 >= 6 || roundoff_type2 >= 20 {
                error_type = 2; // round off error
            }
            if subinterval_too_small(a1, a2, b2) {
                error_type = 3;
            }
        }

        workspace.update(a1, b1, area1, error1, a2, b2, area2, error2)?;

        iteration += 1;

        if !(iteration < limit && error_type == 0 && errsum > tolerance) {
            break;
        }
    }

    let result = sign * workspace.sum_results();
    let abserr = errsum;

    let status = if errsum <= tolerance {
        Ok(())
    } else if error_type == 2 {
        Err(PetirError::RoundOff)
    } else if error_type == 3 {
        Err(PetirError::BadIntegrand)
    } else if iteration == limit {
        Err(PetirError::MaxIterations)
    } else {
        Err(PetirError::Failed)
    };

    Some(out(result, abserr, &workspace, status))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn principal_values_and_refusals() {
        // PV int_{-1}^{2} dx/x = ln 2; reversed limits flip the sign.
        let r = qawc(|_x: f64| 1.0, -1.0, 2.0, 0.0, 0.0, 1e-10, 100).unwrap();
        assert!((r.value - core::f64::consts::LN_2).abs() < 1e-9);
        let r = qawc(|_x: f64| 1.0, 2.0, -1.0, 0.0, 0.0, 1e-10, 100).unwrap();
        assert!((r.value + core::f64::consts::LN_2).abs() < 1e-9);
        // The singularity outside the range is a plain integral of 1/(x-5).
        let r = qawc(|_x: f64| 1.0, 0.0, 1.0, 5.0, 0.0, 1e-10, 100).unwrap();
        assert!((r.value - Real::ln(4.0 / 5.0)).abs() < 1e-9);
        assert_eq!(
            qawc_with_status(|x: f64| x, 0.0, 1.0, 1.0, 0.0, 1e-8, 10).status,
            Err(PetirError::Invalid)
        );
        assert_eq!(
            qawc_with_status(|x: f64| x, 0.0, 1.0, 0.5, 0.0, 0.0, 10).status,
            Err(PetirError::Tolerance)
        );
        assert_eq!(
            qawc_with_status(|x: f64| x, 0.0, 1.0, f64::NAN, 0.0, 1e-8, 10).status,
            Err(PetirError::Domain)
        );
        // Low limit: iteration limit status.
        let o = qawc_with_status(|x: f64| Real::sqrt(x.abs()), -1.0, 1.0, 0.3, 0.0, 1e-14, 2);
        assert!(o.status.is_err());
    }
}
