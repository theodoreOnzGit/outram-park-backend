//! QAWS: adaptive integration of `(x-a)^alpha (b-x)^beta log^mu(x-a)
//! log^nu(b-x) f(x)` with algebraic-logarithmic end-point singularities,
//! ported from GSL 2.8 `integration/qaws.c` (lines 30-220), `qc25s.c`
//! (lines 20-238) and `qmomo.c` (lines 20-194), commit `cf180cd7`, read
//! 2026-11-13. Copyright (C) 1996-2000, 2007 Brian Gough
//! (GPL-3.0-or-later); GSL's QAWS is QUADPACK's `dqawse` (public domain).
//! gh:#1340. Research, education and V&V only.
//!
//! The weight is carried by a [`QawsTable`] (its Chebyshev moments are
//! computed once, `qmomo.c`). Sub-intervals touching `a` or `b` use the
//! 25-point Clenshaw-Curtis rule with the exact weighted moments
//! (`qc25s.c`); others use the 15-point Gauss-Kronrod rule on the weighted
//! integrand. There is no extrapolation.
//!
//! # Differences from upstream
//!
//! - The workspace is allocated inside the call with `limit` entries; the
//!   first step appends two intervals, so `limit < 2` is
//!   [`PetirError::Invalid`] (upstream would write past a workspace of size
//!   one).
//! - Non-finite limits are [`PetirError::Domain`]; `b <= a` is `Invalid`
//!   (`GSL_EINVAL`).
//! - Every array access is a checked `get`.

#[allow(unused_imports)]
use crate::real::Real;
use crate::scalar::DBL_EPSILON;
use crate::{PetirError, Result};

use super::qags::{subinterval_too_small, QagsOutcome, Workspace};
use super::qagp::append_interval;
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

/// `gsl_integration_qaws_table`: the weight parameters and the Chebyshev
/// moments `ri, rj, rg, rh`.
#[derive(Debug, Clone, PartialEq)]
pub struct QawsTable {
    alpha: f64,
    beta: f64,
    mu: i32,
    nu: i32,
    ri: [f64; 25],
    rj: [f64; 25],
    rg: [f64; 25],
    rh: [f64; 25],
}

impl QawsTable {
    /// `gsl_integration_qaws_table_alloc` (qmomo.c:30-70) with `initialise`
    /// (qmomo.c:118-194).
    ///
    /// # Errors
    ///
    /// [`PetirError::Invalid`] unless `alpha > -1` (as upstream:
    /// `alpha < -1.0` is refused and `-1` itself divides by zero), `beta`
    /// likewise, `mu` and `nu` each 0 or 1. Here `alpha = -1` and
    /// `beta = -1` are refused too, being a division by `alpha + 1`.
    ///
    /// # Example
    ///
    /// ```
    /// use petir::integration::{qaws, QawsTable};
    /// // int_0^1 log(x) f(x) with f = 1 is -1 (alpha = beta = 0, mu = 1).
    /// let t = QawsTable::new(0.0, 0.0, 1, 0).unwrap();
    /// let r = qaws(|_x: f64| 1.0, 0.0, 1.0, &t, 0.0, 1e-10, 100).unwrap();
    /// assert!((r.value + 1.0).abs() < 1e-9);
    /// ```
    pub fn new(alpha: f64, beta: f64, mu: i32, nu: i32) -> Result<Self> {
        if !(alpha > -1.0) || !(beta > -1.0) || (mu != 0 && mu != 1) || (nu != 0 && nu != 1) {
            return Err(PetirError::Invalid);
        }
        let mut t = Self {
            alpha,
            beta,
            mu,
            nu,
            ri: [0.0; 25],
            rj: [0.0; 25],
            rg: [0.0; 25],
            rh: [0.0; 25],
        };
        t.initialise();
        Ok(t)
    }

    /// `qmomo.c`, `initialise`.
    fn initialise(&mut self) {
        let alpha = self.alpha;
        let beta = self.beta;
        let alpha_p1 = alpha + 1.0;
        let beta_p1 = beta + 1.0;
        let alpha_p2 = alpha + 2.0;
        let beta_p2 = beta + 2.0;
        let r_alpha = Real::powf(2.0, alpha_p1);
        let r_beta = Real::powf(2.0, beta_p1);

        let (ri, rj, rg, rh) = (&mut self.ri, &mut self.rj, &mut self.rg, &mut self.rh);

        s(ri, 0, r_alpha / alpha_p1);
        let v1 = g(ri, 0) * alpha / alpha_p2;
        s(ri, 1, v1);

        let mut an = 2.0;
        let mut anm1 = 1.0;
        for i in 2..25 {
            let v = -(r_alpha + an * (an - alpha_p2) * g(ri, i - 1)) / (anm1 * (an + alpha_p1));
            s(ri, i, v);
            anm1 = an;
            an += 1.0;
        }

        s(rj, 0, r_beta / beta_p1);
        let v1 = g(rj, 0) * beta / beta_p2;
        s(rj, 1, v1);

        an = 2.0;
        anm1 = 1.0;
        for i in 2..25 {
            let v = -(r_beta + an * (an - beta_p2) * g(rj, i - 1)) / (anm1 * (an + beta_p1));
            s(rj, i, v);
            anm1 = an;
            an += 1.0;
        }

        s(rg, 0, -g(ri, 0) / alpha_p1);
        let v1 = -g(rg, 0) - 2.0 * r_alpha / (alpha_p2 * alpha_p2);
        s(rg, 1, v1);

        an = 2.0;
        anm1 = 1.0;
        for i in 2..25 {
            let v = -(an * (an - alpha_p2) * g(rg, i - 1) - an * g(ri, i - 1) + anm1 * g(ri, i))
                / (anm1 * (an + alpha_p1));
            s(rg, i, v);
            anm1 = an;
            an += 1.0;
        }

        s(rh, 0, -g(rj, 0) / beta_p1);
        let v1 = -g(rh, 0) - 2.0 * r_beta / (beta_p2 * beta_p2);
        s(rh, 1, v1);

        an = 2.0;
        anm1 = 1.0;
        for i in 2..25 {
            let v = -(an * (an - beta_p2) * g(rh, i - 1) - an * g(rj, i - 1) + anm1 * g(rj, i))
                / (anm1 * (an + beta_p1));
            s(rh, i, v);
            anm1 = an;
            an += 1.0;
        }

        let mut i = 1;
        while i < 25 {
            let a = g(rj, i) * -1.0;
            s(rj, i, a);
            let b = g(rh, i) * -1.0;
            s(rh, i, b);
            i += 2;
        }
    }

    /// `alpha`.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// `beta`.
    pub fn beta(&self) -> f64 {
        self.beta
    }

    /// `mu` (0 or 1).
    pub fn mu(&self) -> i32 {
        self.mu
    }

    /// `nu` (0 or 1).
    pub fn nu(&self) -> i32 {
        self.nu
    }

    /// The four moment arrays `(ri, rj, rg, rh)`.
    pub fn moments(&self) -> (&[f64; 25], &[f64; 25], &[f64; 25], &[f64; 25]) {
        (&self.ri, &self.rj, &self.rg, &self.rh)
    }
}

/// `qc25s.c`, `compute_result`.
fn compute_result(r: &[f64], cheb12: &[f64], cheb24: &[f64]) -> (f64, f64) {
    let mut res12 = 0.0;
    let mut res24 = 0.0;
    for i in 0..13 {
        res12 += g(r, i) * g(cheb12, i);
    }
    for i in 0..25 {
        res24 += g(r, i) * g(cheb24, i);
    }
    (res12, res24)
}

/// `qc25s.c`, `qc25s`: `(result, abserr, err_reliable)` on `[a1, b1]` of
/// `[a, b]`.
fn qc25s<F>(f: &F, a: f64, b: f64, a1: f64, b1: f64, t: &QawsTable) -> (f64, f64, bool)
where
    F: Fn(f64) -> f64,
{
    if a1 == a && (t.alpha != 0.0 || t.mu != 0) {
        // fn_qaws_R
        let weighted = |x: f64| {
            let mut factor = 1.0;
            if t.beta != 0.0 {
                factor *= Real::powf(b - x, t.beta);
            }
            if t.nu == 1 {
                factor *= Real::ln(b - x);
            }
            factor * f(x)
        };
        let factor = Real::powf(0.5 * (b1 - a1), t.alpha + 1.0);
        let (cheb12, cheb24) = qcheb(&weighted, a1, b1);

        if t.mu == 0 {
            let u = factor;
            let (res12, res24) = compute_result(&t.ri, &cheb12, &cheb24);
            (u * res24, (u * (res24 - res12)).abs(), false)
        } else {
            let u = factor * Real::ln(b1 - a1);
            let v = factor;
            let (res12a, res24a) = compute_result(&t.ri, &cheb12, &cheb24);
            let (res12b, res24b) = compute_result(&t.rg, &cheb12, &cheb24);
            (
                u * res24a + v * res24b,
                (u * (res24a - res12a)).abs() + (v * (res24b - res12b)).abs(),
                false,
            )
        }
    } else if b1 == b && (t.beta != 0.0 || t.nu != 0) {
        // fn_qaws_L
        let weighted = |x: f64| {
            let mut factor = 1.0;
            if t.alpha != 0.0 {
                factor *= Real::powf(x - a, t.alpha);
            }
            if t.mu == 1 {
                factor *= Real::ln(x - a);
            }
            factor * f(x)
        };
        let factor = Real::powf(0.5 * (b1 - a1), t.beta + 1.0);
        let (cheb12, cheb24) = qcheb(&weighted, a1, b1);

        if t.nu == 0 {
            let u = factor;
            let (res12, res24) = compute_result(&t.rj, &cheb12, &cheb24);
            (u * res24, (u * (res24 - res12)).abs(), false)
        } else {
            let u = factor * Real::ln(b1 - a1);
            let v = factor;
            let (res12a, res24a) = compute_result(&t.rj, &cheb12, &cheb24);
            let (res12b, res24b) = compute_result(&t.rh, &cheb12, &cheb24);
            (
                u * res24a + v * res24b,
                (u * (res24a - res12a)).abs() + (v * (res24b - res12b)).abs(),
                false,
            )
        }
    } else {
        // fn_qaws
        let weighted = |x: f64| {
            let mut factor = 1.0;
            if t.alpha != 0.0 {
                factor *= Real::powf(x - a, t.alpha);
            }
            if t.beta != 0.0 {
                factor *= Real::powf(b - x, t.beta);
            }
            if t.mu == 1 {
                factor *= Real::ln(x - a);
            }
            if t.nu == 1 {
                factor *= Real::ln(b - x);
            }
            factor * f(x)
        };
        let r = kronrod(QkRule::Qk15, weighted, a1, b1);
        (r.result, r.abserr, r.abserr != r.resasc)
    }
}

/// `gsl_integration_qaws`: integrate `f` times the weight of `table` over
/// `[a, b]`.
///
/// # Errors
///
/// As [`super::qags`]; [`PetirError::Invalid`] for `b <= a` or `limit < 2`.
pub fn qaws<F>(
    f: F,
    a: f64,
    b: f64,
    table: &QawsTable,
    epsabs: f64,
    epsrel: f64,
    limit: usize,
) -> Result<Integral>
where
    F: Fn(f64) -> f64,
{
    let o = qaws_with_status(f, a, b, table, epsabs, epsrel, limit);
    o.status.map(|()| o.integral)
}

/// [`qaws`], keeping GSL's estimate when the status is an error.
pub fn qaws_with_status<F>(
    f: F,
    a: f64,
    b: f64,
    table: &QawsTable,
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
    if !(a.is_finite() && b.is_finite()) {
        return bad(PetirError::Domain);
    }
    if limit < 2 {
        return bad(PetirError::Invalid);
    }
    match qaws_core(&f, a, b, table, epsabs, epsrel, limit) {
        Some(o) => o,
        None => bad(PetirError::Invalid),
    }
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn qaws_core<F>(
    f: &F,
    a: f64,
    b: f64,
    t: &QawsTable,
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

    let mut workspace = Workspace::new(limit, a, b)?;
    let zero = |e: PetirError| QagsOutcome {
        integral: Integral {
            value: 0.0,
            abserr: 0.0,
            subintervals: 0,
        },
        status: Err(e),
    };

    if b <= a {
        return Some(zero(PetirError::Invalid));
    }
    if epsabs <= 0.0 && (epsrel < 50.0 * DBL_EPSILON || epsrel < 0.5e-28) {
        return Some(zero(PetirError::Tolerance));
    }

    // Perform the first integration.
    let (result0, abserr0) = {
        let a1 = a;
        let b1 = 0.5 * (a + b);
        let a2 = b1;
        let b2 = b;
        let (area1, error1, _) = qc25s(f, a, b, a1, b1, t);
        let (area2, error2, _) = qc25s(f, a, b, a2, b2, t);

        if error1 > error2 {
            append_interval(&mut workspace, a1, b1, area1, error1)?;
            append_interval(&mut workspace, a2, b2, area2, error2)?;
        } else {
            append_interval(&mut workspace, a2, b2, area2, error2)?;
            append_interval(&mut workspace, a1, b1, area1, error1)?;
        }
        (area1 + area2, error1 + error2)
    };

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
        return Some(out(result0, abserr0, &workspace, Ok(())));
    } else if limit == 1 {
        return Some(out(
            result0,
            abserr0,
            &workspace,
            Err(PetirError::MaxIterations),
        ));
    }

    let mut area = result0;
    let mut errsum = abserr0;
    let mut iteration: usize = 2;

    loop {
        let (a_i, b_i, r_i, e_i) = workspace.retrieve()?;

        let a1 = a_i;
        let b1 = 0.5 * (a_i + b_i);
        let a2 = b1;
        let b2 = b_i;

        let (area1, error1, reliable1) = qc25s(f, a, b, a1, b1, t);
        let (area2, error2, reliable2) = qc25s(f, a, b, a2, b2, t);

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

    let result = workspace.sum_results();
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
    fn weights_and_refusals() {
        let t = QawsTable::new(0.0, 0.0, 1, 0).unwrap();
        let r = qaws(|_x: f64| 1.0, 0.0, 1.0, &t, 0.0, 1e-10, 100).unwrap();
        assert!((r.value + 1.0).abs() < 1e-9);
        // int_0^1 x^-1/2 (1-x)^-1/2 dx = pi.
        let t = QawsTable::new(-0.5, -0.5, 0, 0).unwrap();
        let r = qaws(|_x: f64| 1.0, 0.0, 1.0, &t, 0.0, 1e-9, 100).unwrap();
        assert!((r.value - core::f64::consts::PI).abs() < 1e-8);
        assert_eq!((t.alpha(), t.beta(), t.mu(), t.nu()), (-0.5, -0.5, 0, 0));
        assert!(t.moments().0.len() == 25);
        let tl = QawsTable::new(0.0, 0.0, 0, 1).unwrap();
        let r = qaws(|_x: f64| 1.0, 0.0, 1.0, &tl, 0.0, 1e-10, 100).unwrap();
        assert!((r.value + 1.0).abs() < 1e-9);
        assert!(QawsTable::new(-1.5, 0.0, 0, 0).is_err());
        assert!(QawsTable::new(0.0, -1.5, 0, 0).is_err());
        assert!(QawsTable::new(0.0, 0.0, 2, 0).is_err());
        assert!(QawsTable::new(0.0, 0.0, 0, -1).is_err());
        assert_eq!(
            qaws_with_status(|x: f64| x, 1.0, 0.0, &t, 0.0, 1e-8, 10).status,
            Err(PetirError::Invalid)
        );
        assert_eq!(
            qaws_with_status(|x: f64| x, 0.0, 1.0, &t, 0.0, 0.0, 10).status,
            Err(PetirError::Tolerance)
        );
        assert_eq!(
            qaws_with_status(|x: f64| x, 0.0, f64::NAN, &t, 0.0, 1e-8, 10).status,
            Err(PetirError::Domain)
        );
        assert_eq!(
            qaws_with_status(|x: f64| x, 0.0, 1.0, &t, 0.0, 1e-8, 1).status,
            Err(PetirError::Invalid)
        );
    }
}
