//! Romberg integration, ported from GSL 2.8 `integration/romberg.c` (lines
//! 41-164), commit `cf180cd7`, read 2026-11-13. Copyright (C) 2019 Patrick
//! Alken (GPL-3.0-or-later). gh:#1340. Research, education and V&V only.
//!
//! Richardson extrapolation of the composite trapezoid rule with `2^i + 1`
//! points per row; the workspace holds two rows of `n <= 30` entries. It
//! needs a smooth integrand and an interval with finite endpoints (the
//! endpoints are evaluated).
//!
//! # Differences from upstream
//!
//! - The workspace is allocated inside the call (`n` capped at 30 as upstream
//!   caps it).
//! - `n == 0` and a negative tolerance are [`PetirError::Domain`] (`GSL_EDOM`
//!   both there); non-finite limits are `Domain` too (upstream does not check).
//! - Every array access is a checked `get`.

use alloc::vec;

use crate::{PetirError, Result};

/// The outcome of [`romberg`]: GSL writes `*result` and `*neval` before it
/// returns `GSL_EMAXITER`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RombergOutcome {
    /// `*result`: the converged `R(i, i)`, or the last row's best guess.
    pub value: f64,
    /// `*neval`: integrand evaluations.
    pub neval: usize,
    /// `Ok(())`, or `Err(MaxIterations)` when the table is exhausted.
    pub status: Result<()>,
}

fn at(v: &[f64], i: usize) -> f64 {
    v.get(i).copied().unwrap_or(f64::NAN)
}

fn put(v: &mut [f64], i: usize, x: f64) {
    if let Some(s) = v.get_mut(i) {
        *s = x;
    }
}

/// `gsl_integration_romberg`: Romberg integration of `f` over `[a, b]` with
/// at most `n` rows (`n <= 30`, so at most `2^29 + 1` points).
///
/// # Errors
///
/// [`PetirError::Domain`] for `n == 0`, a negative tolerance or a non-finite
/// limit; [`PetirError::MaxIterations`] when no row converged (the best
/// guess is in [`romberg_with_status`]).
///
/// # Example
///
/// ```
/// use petir::integration::romberg;
/// let r = romberg(|x: f64| x.exp(), 0.0, 1.0, 0.0, 1e-12, 20).unwrap();
/// assert!((r.value - (core::f64::consts::E - 1.0)).abs() < 1e-12);
/// ```
pub fn romberg<F>(f: F, a: f64, b: f64, epsabs: f64, epsrel: f64, n: usize) -> Result<RombergOutcome>
where
    F: Fn(f64) -> f64,
{
    let o = romberg_with_status(f, a, b, epsabs, epsrel, n);
    o.status.map(|()| o)
}

/// [`romberg`], keeping GSL's `*result` and `*neval` when the status is an
/// error.
pub fn romberg_with_status<F>(
    f: F,
    a: f64,
    b: f64,
    epsabs: f64,
    epsrel: f64,
    n: usize,
) -> RombergOutcome
where
    F: Fn(f64) -> f64,
{
    let bad = RombergOutcome {
        value: 0.0,
        neval: 0,
        status: Err(PetirError::Domain),
    };
    // `epsabs < 0.0` is false for NaN; upstream lets that through, so the
    // finiteness check here is separate and stricter for the limits only.
    if n < 1 || epsabs < 0.0 || epsrel < 0.0 || !(a.is_finite() && b.is_finite()) {
        return bad;
    }
    let n = n.min(30);
    let mut rp = vec![0.0_f64; n];
    let mut rc = vec![0.0_f64; n];
    let mut h = 0.5 * (b - a);

    // R(0,0)
    put(&mut rp, 0, h * (f(a) + f(b)));
    let mut neval: usize = 2;

    for i in 1..n {
        let mut sum = 0.0;
        let two_i: usize = 1 << i;

        let mut j = 1usize;
        while j < two_i {
            sum += f(a + (j as f64) * h);
            neval += 1;
            j += 2;
        }

        // R(i,0)
        put(&mut rc, 0, sum * h + 0.5 * at(&rp, 0));

        let mut four_j = 4.0;
        for j in 1..=i {
            let v = (four_j * at(&rc, j - 1) - at(&rp, j - 1)) / (four_j - 1.0);
            put(&mut rc, j, v);
            four_j *= 4.0;
        }

        // Compare with the previous row and test for convergence.
        let err = (at(&rc, i) - at(&rp, i - 1)).abs();
        if err < epsabs || err < epsrel * at(&rc, i).abs() {
            return RombergOutcome {
                value: at(&rc, i),
                neval,
                status: Ok(()),
            };
        }

        core::mem::swap(&mut rp, &mut rc);
        h *= 0.5;
    }

    // Did not converge: return the best guess.
    RombergOutcome {
        value: at(&rp, n - 1),
        neval,
        status: Err(PetirError::MaxIterations),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converges_and_refuses() {
        let r = romberg(|x: f64| x * x, 0.0, 3.0, 0.0, 1e-12, 10).unwrap();
        assert!((r.value - 9.0).abs() < 1e-12);
        // |x| has a kink: Romberg stalls and runs out of rows.
        let o = romberg_with_status(|x: f64| (x - 0.3).abs(), 0.0, 1.0, 0.0, 1e-14, 6);
        assert_eq!(o.status, Err(PetirError::MaxIterations));
        assert_eq!(o.neval, 33);
        // n = 1 never enters the loop.
        let o = romberg_with_status(|x: f64| x, 0.0, 1.0, 0.0, 1e-8, 1);
        assert_eq!((o.neval, o.status), (2, Err(PetirError::MaxIterations)));
        assert_eq!(
            romberg_with_status(|x: f64| x, 0.0, 1.0, -1.0, 0.0, 5).status,
            Err(PetirError::Domain)
        );
        assert_eq!(
            romberg_with_status(|x: f64| x, 0.0, 1.0, 0.0, 0.0, 0).status,
            Err(PetirError::Domain)
        );
    }
}
