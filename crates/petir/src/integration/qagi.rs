//! QAGI, QAGIU and QAGIL: adaptive integration over an infinite range
//! (gh:#1340), ported from GSL 2.8 `integration/qags.c` lines 52-192
//! (`gsl_integration_qagi`, `_qagil`, `_qagiu` and their `*_transform`
//! helpers), commit `cf180cd7`. Copyright the GSL authors
//! (GPL-3.0-or-later). Research, education and V&V only.
//!
//! Each maps the range onto `t` in `(0, 1]` and calls the **same** `qags`
//! engine as [`super::qags`] with the 15-point Kronrod rule
//! (`gsl_integration_qk15`):
//!
//! - [`qagi`]: `int_{-inf}^{inf} f = int_0^1 (f((1-t)/t) + f(-(1-t)/t)) / t^2 dt`;
//! - [`qagiu`]: `int_a^{inf} f = int_0^1 f(a + (1-t)/t) / t^2 dt`;
//! - [`qagil`]: `int_{-inf}^b f = int_0^1 f(b - (1-t)/t) / t^2 dt`.
//!
//! The transforms keep upstream's grouping, `(y / t) / t` and `x = (1 - t) / t`
//! (`a + (1 - t) / t`, `b - (1 - t) / t`), because the rounding is part of the
//! bit-identity claim.
//!
//! # Not QUADPACK's `dqagi`
//!
//! GSL's QAGI is the QAGS engine on a transformed integrand with the plain
//! `qk15` rule. QUADPACK's own `dqagi` uses the specialised `dqk15i` rule that
//! transforms inside the rule. They agree in the integral, not in the
//! evaluation sequence; this port follows GSL, as the rest of PETIR does.
//!
//! # Errors and the returned estimate
//!
//! As [`super::qags`]; the `*_with_status` variants keep GSL's estimate when
//! the status is an error. `t = 0` is never evaluated by the rule (the nodes
//! are interior), so `(1 - t) / t` is finite.

use super::qags::{qags_core, QagsOutcome};
use super::{Integral, QkRule};
use crate::PetirError;
use crate::Result;

/// The transformation family, so one function runs all three.
#[derive(Clone, Copy)]
enum Range {
    /// `(-inf, inf)`.
    Whole,
    /// `[a, inf)`.
    Upper(f64),
    /// `(-inf, b]`.
    Lower(f64),
}

fn run<F>(f: &F, range: Range, epsabs: f64, epsrel: f64, limit: usize) -> QagsOutcome
where
    F: Fn(f64) -> f64,
{
    let zero = Integral {
        value: 0.0,
        abserr: 0.0,
        subintervals: 0,
    };
    let bad = |e: PetirError| QagsOutcome {
        integral: zero,
        status: Err(e),
    };
    let finite = match range {
        Range::Whole => true,
        Range::Upper(v) | Range::Lower(v) => v.is_finite(),
    };
    if !finite {
        return bad(PetirError::Domain);
    }
    if limit == 0 {
        return bad(PetirError::Invalid);
    }
    let transform = |t: f64| -> f64 {
        let x = match range {
            Range::Whole => (1.0 - t) / t,
            Range::Upper(a) => a + (1.0 - t) / t,
            Range::Lower(b) => b - (1.0 - t) / t,
        };
        let y = match range {
            Range::Whole => f(x) + f(-x),
            _ => f(x),
        };
        (y / t) / t
    };
    match qags_core(QkRule::Qk15, &transform, 0.0, 1.0, epsabs, epsrel, limit) {
        Some(o) => o,
        None => bad(PetirError::Invalid),
    }
}

/// `int_{-inf}^{inf} f(x) dx` — GSL `gsl_integration_qagi`.
///
/// # Errors
///
/// As [`super::qags`] (`Invalid` for `limit == 0`).
///
/// # Example
///
/// ```
/// use petir::integration::qagi;
/// // The Gaussian integral: sqrt(pi).
/// let r = qagi(|x: f64| (-x * x).exp(), 0.0, 1e-10, 100).unwrap();
/// assert!((r.value - core::f64::consts::PI.sqrt()).abs() < 1e-9);
/// ```
pub fn qagi<F>(f: F, epsabs: f64, epsrel: f64, limit: usize) -> Result<Integral>
where
    F: Fn(f64) -> f64,
{
    let o = qagi_with_status(f, epsabs, epsrel, limit);
    o.status.map(|()| o.integral)
}

/// [`qagi`], keeping GSL's estimate when the status is an error.
pub fn qagi_with_status<F>(f: F, epsabs: f64, epsrel: f64, limit: usize) -> QagsOutcome
where
    F: Fn(f64) -> f64,
{
    run(&f, Range::Whole, epsabs, epsrel, limit)
}

/// `int_a^{inf} f(x) dx` — GSL `gsl_integration_qagiu`.
///
/// # Errors
///
/// As [`qagi`], plus [`PetirError::Domain`] for a non-finite `a`.
///
/// # Example
///
/// ```
/// use petir::integration::qagiu;
/// // int_1^inf x^-2 dx = 1.
/// let r = qagiu(|x: f64| 1.0 / (x * x), 1.0, 0.0, 1e-10, 100).unwrap();
/// assert!((r.value - 1.0).abs() < 1e-10);
/// ```
pub fn qagiu<F>(f: F, a: f64, epsabs: f64, epsrel: f64, limit: usize) -> Result<Integral>
where
    F: Fn(f64) -> f64,
{
    let o = qagiu_with_status(f, a, epsabs, epsrel, limit);
    o.status.map(|()| o.integral)
}

/// [`qagiu`], keeping GSL's estimate when the status is an error.
pub fn qagiu_with_status<F>(f: F, a: f64, epsabs: f64, epsrel: f64, limit: usize) -> QagsOutcome
where
    F: Fn(f64) -> f64,
{
    run(&f, Range::Upper(a), epsabs, epsrel, limit)
}

/// `int_{-inf}^b f(x) dx` — GSL `gsl_integration_qagil`.
///
/// # Errors
///
/// As [`qagiu`], for `b`.
///
/// # Example
///
/// ```
/// use petir::integration::qagil;
/// // int_{-inf}^0 e^x dx = 1.
/// let r = qagil(|x: f64| x.exp(), 0.0, 0.0, 1e-10, 100).unwrap();
/// assert!((r.value - 1.0).abs() < 1e-10);
/// ```
pub fn qagil<F>(f: F, b: f64, epsabs: f64, epsrel: f64, limit: usize) -> Result<Integral>
where
    F: Fn(f64) -> f64,
{
    let o = qagil_with_status(f, b, epsabs, epsrel, limit);
    o.status.map(|()| o.integral)
}

/// [`qagil`], keeping GSL's estimate when the status is an error.
pub fn qagil_with_status<F>(f: F, b: f64, epsabs: f64, epsrel: f64, limit: usize) -> QagsOutcome
where
    F: Fn(f64) -> f64,
{
    run(&f, Range::Lower(b), epsabs, epsrel, limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::real::Real;

    #[test]
    fn closed_forms_and_refusals() {
        let g = qagi(|x: f64| Real::exp(-x * x), 0.0, 1e-10, 200).unwrap();
        assert!((g.value - core::f64::consts::PI.sqrt()).abs() < 1e-9);
        let u = qagiu(|x: f64| Real::exp(-x), 2.0, 0.0, 1e-10, 200).unwrap();
        assert!((u.value - Real::exp(-2.0)).abs() < 1e-10);
        let l = qagil(|x: f64| Real::exp(x), -1.0, 0.0, 1e-10, 200).unwrap();
        assert!((l.value - Real::exp(-1.0)).abs() < 1e-10);
        assert!(qagi(|x: f64| x, 0.0, 1e-8, 0).is_err());
        assert!(qagiu(|x: f64| x, f64::INFINITY, 0.0, 1e-8, 10).is_err());
        assert!(qagil(|x: f64| x, f64::NAN, 0.0, 1e-8, 10).is_err());
        // (An odd integrand like `x` would cancel to 0 and "converge".)
        let o = qagi_with_status(|x: f64| 1.0 / Real::sqrt(1.0 + x * x), 0.0, 1e-8, 10);
        assert!(o.status.is_err(), "int 1/sqrt(1+x^2) over R diverges");
    }
}
