//! CQUAD: doubly-adaptive integration with Clenshaw-Curtis rules and
//! interpolation-based error estimates, robust to non-finite integrand values,
//! ported from GSL 2.8 `integration/cquad.c` (lines 30-683) and
//! `cquad_const.c`, commit `cf180cd7`, read 2026-11-14. Copyright (C) 2010
//! Pedro Gonnet (GPL-3.0-or-later). gh:#1340. Research, education and V&V
//! only.
//!
//! Each interval carries the values of the integrand at the 33 nodes of the
//! finest rule and the coefficients of its Chebyshev interpolants of degree 4,
//! 8, 16 and 32. An interval first raises its degree (re-using the evaluations
//! it has), then is bisected; intervals live in a max-heap on the error
//! estimate, and the lowest-error intervals are dropped (their contribution
//! kept) when the workspace fills. Non-finite integrand values are replaced by
//! 0, the coefficients are *downdated* to remove the node, and the value is
//! marked NaN.
//!
//! The constants (`cquad_tables.rs`) were copied from `cquad_const.c` by
//! `reference-data/gsl/gen_cquad_tables_rs.py`, never typed.
//!
//! # Differences from upstream
//!
//! - The workspace is allocated inside the call (`size` intervals);
//!   `size < 3` is `GSL_EDOM` as `gsl_integration_cquad_workspace_alloc`
//!   (upstream's `NULL` workspace would then be `GSL_EINVAL` in `cquad`).
//! - Upstream's `int nans[32]` overflows when all 33 nodes of an interval are
//!   non-finite (undefined behaviour); the port keeps them in a `Vec`.
//! - `downdate` with `n == 0` (reachable only if every node of a degree-4
//!   interval but none were non-finite) would read `Lalpha[-1]` upstream; the
//!   port stops.
//! - `a` and `b` are not checked, as upstream.
//! - On `GSL_EDIVERGE` upstream sets only `*result` (and `*nevals`);
//!   `abserr` stays whatever the caller held, here `0.0`. On `GSL_EBADTOL`
//!   nothing is written.
//! - Every array access is a checked `get`.

use alloc::vec;
use alloc::vec::Vec;

#[allow(unused_imports)]
use crate::real::Real;
use crate::scalar::DBL_EPSILON;
use crate::{PetirError, Result};

use super::cquad_tables::{
    BEE, LALPHA, LGAMMA, TLEFT, TRIGHT, V1INV, V2INV, V3INV, V4INV, XI,
};

/// `gsl_integration_cquad_ival`.
#[derive(Clone)]
struct Ival {
    a: f64,
    b: f64,
    c: [f64; 64],
    fx: [f64; 33],
    igral: f64,
    err: f64,
    depth: i32,
    rdepth: i32,
    ndiv: i32,
}

impl Ival {
    fn empty() -> Self {
        Self {
            a: 0.0,
            b: 0.0,
            c: [0.0; 64],
            fx: [0.0; 33],
            igral: 0.0,
            err: 0.0,
            depth: 0,
            rdepth: 0,
            ndiv: 0,
        }
    }
}

const N: [usize; 4] = [4, 8, 16, 32];
const SKIP: [usize; 4] = [8, 4, 2, 1];
const IDX: [usize; 4] = [0, 5, 14, 31];
const NDIV_MAX: i32 = 20;

fn at(v: &[f64], i: usize) -> f64 {
    v.get(i).copied().unwrap_or(0.0)
}

fn put(v: &mut [f64], i: usize, x: f64) {
    if let Some(p) = v.get_mut(i) {
        *p = x;
    }
}

fn n_of(d: usize) -> usize {
    N.get(d).copied().unwrap_or(0)
}

fn skip_of(d: usize) -> usize {
    SKIP.get(d).copied().unwrap_or(1)
}

fn idx_of(d: usize) -> usize {
    IDX.get(d).copied().unwrap_or(0)
}

/// `Vinvfx`: the product of the node values with one of the inverse
/// Vandermonde-like matrices, written to `c[off..]`.
fn vinvfx(fx: &[f64; 33], c: &mut [f64; 64], off: usize, d: usize) {
    let (v, m, stride): (&[f64], usize, usize) = match d {
        0 => (&V1INV, 4, 8),
        1 => (&V2INV, 8, 4),
        2 => (&V3INV, 16, 2),
        _ => (&V4INV, 32, 1),
    };
    for i in 0..=m {
        put(c, off + i, 0.0);
        for j in 0..=m {
            let t = at(c, off + i) + at(v, i * (m + 1) + j) * at(fx, j * stride);
            put(c, off + i, t);
        }
    }
}

/// `downdate`: remove the nodes in `nans` from the interpolation given by
/// the coefficients `c[off..]` of degree `n`.
fn downdate(c: &mut [f64; 64], off: usize, mut n: usize, d: usize, nans: &[usize]) {
    const BIDX: [usize; 4] = [0, 6, 16, 34];
    let mut b_new = [0.0_f64; 34];
    let b0 = BIDX.get(d).copied().unwrap_or(0);
    for i in 0..=n + 1 {
        put(&mut b_new, i, at(&BEE, b0 + i));
    }
    for &nan in nans {
        let Some(nm1) = n.checked_sub(1) else {
            return;
        };
        let xn = at(&XI, nan);
        let t = at(&b_new, n + 1) / at(&LALPHA, n);
        put(&mut b_new, n + 1, t);
        let t = (at(&b_new, n) + xn * at(&b_new, n + 1)) / at(&LALPHA, nm1);
        put(&mut b_new, n, t);
        let mut j = n - 1;
        while j > 0 {
            let t = (at(&b_new, j) + xn * at(&b_new, j + 1)
                - at(&LGAMMA, j + 1) * at(&b_new, j + 2))
                / at(&LALPHA, j - 1);
            put(&mut b_new, j, t);
            j -= 1;
        }
        for j in 0..=n {
            let t = at(&b_new, j + 1);
            put(&mut b_new, j, t);
        }
        let alpha = at(c, off + n) / at(&b_new, n);
        for j in 0..n {
            let t = at(c, off + j) - alpha * at(&b_new, j);
            put(c, off + j, t);
        }
        put(c, off + n, 0.0);
        n -= 1;
    }
}

/// The result of [`cquad_with_status`]: GSL's `*result`, `*abserr`,
/// `*nevals` and status.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CquadOutcome {
    /// `*result` (`+-inf` for `GSL_EDIVERGE`; `0` for a refused call).
    pub value: f64,
    /// `*abserr` (`0` when upstream leaves it unwritten).
    pub abserr: f64,
    /// `*nevals`, the number of integrand evaluations (`0` when unwritten).
    pub nevals: usize,
    /// `Ok(())` for `GSL_SUCCESS`.
    pub status: Result<()>,
    /// GSL's integer status: 0 success, 1 `EDOM` (workspace size below 3),
    /// 13 `EBADTOL`, 22 `EDIVERGE`.
    pub gsl_status: i32,
}

fn refused(gsl_status: i32, status: PetirError) -> CquadOutcome {
    CquadOutcome {
        value: 0.0,
        abserr: 0.0,
        nevals: 0,
        status: Err(status),
        gsl_status,
    }
}

/// Integrate `f` over `[a, b]` with CQUAD — GSL's `gsl_integration_cquad`.
/// `size` is the number of intervals the workspace holds (at least 3; when it
/// fills, the intervals with the smallest error estimates are dropped, so it
/// is a buffer size, not a limit on the work).
///
/// # Errors
///
/// [`PetirError::Domain`] for `size < 3`, [`PetirError::Tolerance`]
/// (`GSL_EBADTOL`) for a negative tolerance or `epsabs <= 0` with
/// `epsrel < DBL_EPSILON`, [`PetirError::Diverged`] when the integral
/// diverges (use [`cquad_with_status`] for the `+-inf` GSL reports).
///
/// # Example
///
/// ```
/// use petir::integration::cquad;
/// // int_0^1 1/sqrt(x) dx = 2, with a non-finite value at x = 0.
/// let r = cquad(|x: f64| 1.0 / x.sqrt(), 0.0, 1.0, 0.0, 1e-9, 100).unwrap();
/// assert!((r.value - 2.0).abs() < 1e-8);
/// ```
pub fn cquad<F>(
    f: F,
    a: f64,
    b: f64,
    epsabs: f64,
    epsrel: f64,
    size: usize,
) -> Result<CquadOutcome>
where
    F: Fn(f64) -> f64,
{
    let o = cquad_with_status(f, a, b, epsabs, epsrel, size);
    o.status.map(|()| o)
}

/// [`cquad`], keeping what GSL writes to its output arguments on failure.
pub fn cquad_with_status<F>(
    f: F,
    a: f64,
    b: f64,
    epsabs: f64,
    epsrel: f64,
    size: usize,
) -> CquadOutcome
where
    F: Fn(f64) -> f64,
{
    if size < 3 {
        return refused(1, PetirError::Domain);
    }
    match cquad_core(&f, a, b, epsabs, epsrel, size) {
        Some(o) => o,
        None => refused(4, PetirError::Invalid),
    }
}

struct Cq {
    ivals: Vec<Ival>,
    heap: Vec<usize>,
}

impl Cq {
    fn iv(&self, k: usize) -> Option<&Ival> {
        self.ivals.get(*self.heap.get(k)?)
    }

    fn err_at(&self, k: usize) -> Option<f64> {
        Some(self.iv(k)?.err)
    }

    fn swap(&mut self, i: usize, j: usize) -> Option<()> {
        let (x, y) = (*self.heap.get(i)?, *self.heap.get(j)?);
        *self.heap.get_mut(i)? = y;
        *self.heap.get_mut(j)? = x;
        Some(())
    }

    /// The sift-down loop of upstream's three fix-ups, on a heap of `bound`
    /// entries starting at the root.
    fn sift_down(&mut self, bound: usize) -> Option<()> {
        let mut i = 0usize;
        while 2 * i + 1 < bound {
            let mut j = 2 * i + 1;
            if j + 1 < bound && self.err_at(j + 1)? >= self.err_at(j)? {
                j += 1;
            }
            if self.err_at(j)? <= self.err_at(i)? {
                break;
            }
            self.swap(i, j)?;
            i = j;
        }
        Some(())
    }
}

#[allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::float_cmp,
    clippy::cognitive_complexity
)]
fn cquad_core<F>(
    f: &F,
    a: f64,
    b: f64,
    epsabs: f64,
    epsrel: f64,
    size: usize,
) -> Option<CquadOutcome>
where
    F: Fn(f64) -> f64,
{
    let w = core::f64::consts::SQRT_2 / 2.0;
    let mut neval: usize = 0;

    // Check for unreasonable accuracy demands.
    if epsabs < 0.0 || epsrel < 0.0 {
        return Some(refused(13, PetirError::Tolerance));
    }
    if epsabs <= 0.0 && epsrel < DBL_EPSILON {
        return Some(refused(13, PetirError::Tolerance));
    }

    let mut ws = Cq {
        ivals: vec![Ival::empty(); size],
        heap: (0..size).collect(),
    };

    // Create the first interval.
    let mut nans: Vec<usize> = Vec::new();
    let (igral0, err0) = {
        let m = (a + b) / 2.0;
        let h = (b - a) / 2.0;
        let iv = ws.ivals.get_mut(0)?;
        for i in 0..=N[3] {
            put(&mut iv.fx, i, f(m + at(&XI, i) * h));
            neval += 1;
            if !at(&iv.fx, i).is_finite() {
                nans.push(i);
                put(&mut iv.fx, i, 0.0);
            }
        }
        let fx = iv.fx;
        vinvfx(&fx, &mut iv.c, idx_of(0), 0);
        vinvfx(&fx, &mut iv.c, idx_of(3), 3);
        vinvfx(&fx, &mut iv.c, idx_of(2), 2);
        for &k in &nans {
            put(&mut iv.fx, k, f64::NAN);
        }
        iv.a = a;
        iv.b = b;
        iv.depth = 3;
        iv.rdepth = 1;
        iv.ndiv = 0;
        iv.igral = 2.0 * h * at(&iv.c, idx_of(3)) * w;
        let mut nc = 0.0_f64;
        for i in N[2] + 1..=N[3] {
            let temp = at(&iv.c, idx_of(3) + i);
            nc += temp * temp;
        }
        let mut ncdiff = nc;
        for i in 0..=N[2] {
            let temp = at(&iv.c, idx_of(2) + i) - at(&iv.c, idx_of(3) + i);
            ncdiff += temp * temp;
            nc += at(&iv.c, idx_of(3) + i) * at(&iv.c, idx_of(3) + i);
        }
        ncdiff = Real::sqrt(ncdiff);
        nc = Real::sqrt(nc);
        iv.err = ncdiff * 2.0 * h;
        if ncdiff / nc > 0.1 && iv.err < 2.0 * h * nc {
            iv.err = 2.0 * h * nc;
        }
        (iv.igral, iv.err)
    };

    // Initialise some global values.
    let mut igral = igral0;
    let mut err = err0;
    let mut nivals: usize = 1;
    let mut igral_final = 0.0_f64;
    let mut err_final = 0.0_f64;

    // Main loop.
    while nivals > 0
        && err > 0.0
        && !(err <= igral.abs() * epsrel || err <= epsabs)
        && !(err_final > igral.abs() * epsrel && err - err_final < igral.abs() * epsrel)
        && !(err_final > epsabs && err - err_final < epsabs)
    {
        // Put our finger on the interval with the largest error.
        let k0 = *ws.heap.first()?;
        let (m, h) = {
            let iv = ws.ivals.get(k0)?;
            ((iv.a + iv.b) / 2.0, (iv.b - iv.a) / 2.0)
        };

        // Should we try to increase the degree?
        let split;
        if ws.ivals.get(k0)?.depth < 3 {
            let iv = ws.ivals.get_mut(k0)?;
            iv.depth += 1;
            let d = iv.depth as usize;

            // Get the new (missing) function values.
            let mut i = skip_of(d);
            while i <= 32 {
                put(&mut iv.fx, i, f(m + at(&XI, i) * h));
                neval += 1;
                i += 2 * skip_of(d);
            }
            nans.clear();
            let mut i = 0usize;
            while i <= 32 {
                if !at(&iv.fx, i).is_finite() {
                    nans.push(i);
                    put(&mut iv.fx, i, 0.0);
                }
                i += skip_of(d);
            }

            // Compute the new coefficients.
            let fx = iv.fx;
            vinvfx(&fx, &mut iv.c, idx_of(d), d);

            // Downdate any NaNs.
            if !nans.is_empty() {
                downdate(&mut iv.c, idx_of(d), n_of(d), d, &nans);
                for &k in &nans {
                    put(&mut iv.fx, k, f64::NAN);
                }
            }

            // Compute the error estimate.
            let mut nc = 0.0_f64;
            for i in n_of(d - 1) + 1..=n_of(d) {
                let temp = at(&iv.c, idx_of(d) + i);
                nc += temp * temp;
            }
            let mut ncdiff = nc;
            for i in 0..=n_of(d - 1) {
                let temp = at(&iv.c, idx_of(d - 1) + i) - at(&iv.c, idx_of(d) + i);
                ncdiff += temp * temp;
                nc += at(&iv.c, idx_of(d) + i) * at(&iv.c, idx_of(d) + i);
            }
            ncdiff = Real::sqrt(ncdiff);
            nc = Real::sqrt(nc);
            iv.err = ncdiff * 2.0 * h;

            // Compute the local integral.
            iv.igral = 2.0 * h * w * at(&iv.c, idx_of(d));

            // Split the interval prematurely?
            split = nc > 0.0 && ncdiff / nc > 0.1;
        } else {
            // Maximum degree reached, just split.
            split = true;
        }

        let ivc = ws.ivals.get(k0)?.clone();

        // Should we drop this interval?
        if (m + h * at(&XI, 0)) >= (m + h * at(&XI, 1))
            || (m + h * at(&XI, 31)) >= (m + h * at(&XI, 32))
            || ivc.err < ivc.igral.abs() * DBL_EPSILON * 10.0
        {
            // Keep this interval's contribution.
            err_final += ivc.err;
            igral_final += ivc.igral;

            // Swap with the last element on the heap.
            ws.swap(nivals - 1, 0)?;
            nivals -= 1;

            // Fix up the heap.
            ws.sift_down(nivals)?;
        } else if split {
            // Do we need to split this interval?
            let d = ivc.depth as usize;

            // Generate the interval on the left, then on the right.
            for side in 0..2usize {
                let slot = *ws.heap.get(nivals)?;
                nivals += 1;
                let mut nw = Ival::empty();
                if side == 0 {
                    nw.a = ivc.a;
                    nw.b = m;
                } else {
                    nw.a = m;
                    nw.b = ivc.b;
                }
                nw.depth = 0;
                nw.rdepth = ivc.rdepth + 1;
                if side == 0 {
                    put(&mut nw.fx, 0, at(&ivc.fx, 0));
                    put(&mut nw.fx, 32, at(&ivc.fx, 16));
                } else {
                    put(&mut nw.fx, 0, at(&ivc.fx, 16));
                    put(&mut nw.fx, 32, at(&ivc.fx, 32));
                }
                let mut i = skip_of(0);
                while i < 32 {
                    put(&mut nw.fx, i, f((nw.a + nw.b) / 2.0 + at(&XI, i) * h / 2.0));
                    neval += 1;
                    i += skip_of(0);
                }
                nans.clear();
                let mut i = 0usize;
                while i <= 32 {
                    if !at(&nw.fx, i).is_finite() {
                        nans.push(i);
                        put(&mut nw.fx, i, 0.0);
                    }
                    i += skip_of(0);
                }
                let fx = nw.fx;
                vinvfx(&fx, &mut nw.c, 0, 0);
                if !nans.is_empty() {
                    downdate(&mut nw.c, 0, n_of(0), 0, &nans);
                    for &k in &nans {
                        put(&mut nw.fx, k, f64::NAN);
                    }
                }
                let tmat: &[f64] = if side == 0 { &TLEFT } else { &TRIGHT };
                for i in 0..=n_of(d) {
                    put(&mut nw.c, idx_of(d) + i, 0.0);
                    for j in i..=n_of(d) {
                        let t = at(&nw.c, idx_of(d) + i)
                            + at(tmat, i * 33 + j) * at(&ivc.c, idx_of(d) + j);
                        put(&mut nw.c, idx_of(d) + i, t);
                    }
                }
                let mut ncdiff = 0.0_f64;
                for i in 0..=n_of(0) {
                    let temp = at(&nw.c, i) - at(&nw.c, idx_of(d) + i);
                    ncdiff += temp * temp;
                }
                for i in n_of(0) + 1..=n_of(d) {
                    let temp = at(&nw.c, idx_of(d) + i);
                    ncdiff += temp * temp;
                }
                ncdiff = Real::sqrt(ncdiff);
                nw.err = ncdiff * h;

                // Check for divergence.
                nw.ndiv = ivc.ndiv
                    + i32::from(at(&ivc.c, 0).abs() > 0.0 && at(&nw.c, 0) / at(&ivc.c, 0) > 2.0);
                if nw.ndiv > NDIV_MAX && 2 * nw.ndiv > nw.rdepth {
                    return Some(CquadOutcome {
                        value: if igral >= 0.0 {
                            f64::INFINITY
                        } else {
                            f64::NEG_INFINITY
                        },
                        abserr: 0.0,
                        nevals: neval,
                        status: Err(PetirError::Diverged),
                        gsl_status: 22,
                    });
                }

                // Compute the local integral.
                nw.igral = h * w * at(&nw.c, 0);

                *ws.ivals.get_mut(slot)? = nw;
            }

            // Fix-up the heap: flip the last interval to the top and sift
            // it down, then sift the last (new) interval up.
            ws.swap(nivals - 1, 0)?;
            nivals -= 1;

            let bound = nivals - 1;
            ws.sift_down(bound)?;

            let mut i = nivals - 1;
            while i > 0 {
                let j = (i - 1) / 2;
                if ws.err_at(j)? < ws.err_at(i)? {
                    ws.swap(j, i)?;
                    i = j;
                } else {
                    break;
                }
            }
        } else {
            // Otherwise, just fix-up the heap.
            ws.sift_down(nivals)?;
        }

        // If the heap is about to overflow, remove the last two intervals.
        while nivals + 2 > size {
            let iv = ws.iv(nivals - 1)?;
            err_final += iv.err;
            igral_final += iv.igral;
            nivals -= 1;
        }

        // Collect the value of the integral and error.
        igral = igral_final;
        err = err_final;
        for i in 0..nivals {
            let iv = ws.iv(i)?;
            igral += iv.igral;
            err += iv.err;
        }
    }

    Some(CquadOutcome {
        value: igral,
        abserr: err,
        nevals: neval,
        status: Ok(()),
        gsl_status: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smooth_singular_and_nonfinite_integrands() {
        let r = cquad(|x: f64| Real::exp(x), 0.0, 1.0, 0.0, 1e-12, 100).unwrap();
        assert!((r.value - (core::f64::consts::E - 1.0)).abs() < 1e-11);
        assert!(r.nevals >= 33);
        // 1/sqrt(x): non-finite at x = 0, downdated.
        let r = cquad(|x: f64| 1.0 / Real::sqrt(x), 0.0, 1.0, 0.0, 1e-9, 100).unwrap();
        assert!((r.value - 2.0).abs() < 1e-8);
        // A kink and a log singularity, with a small workspace that drops
        // intervals.
        let r = cquad(|x: f64| Real::ln(x), 0.0, 1.0, 0.0, 1e-8, 5).unwrap();
        assert!((r.value + 1.0).abs() < 1e-6);
        // 1/x on (0, 1] does not trip the divergence test in this port (compared
        // with GSL in the reference test, which also covers GSL_EDIVERGE).
        let o = cquad_with_status(|x: f64| 1.0 / x, 0.0, 1.0, 0.0, 1e-9, 100);
        assert_eq!(o.gsl_status, 0);
        // Refusals.
        assert_eq!(
            cquad_with_status(|x: f64| x, 0.0, 1.0, 0.0, 1e-9, 2).status,
            Err(PetirError::Domain)
        );
        assert_eq!(
            cquad_with_status(|x: f64| x, 0.0, 1.0, -1.0, 1e-9, 10).status,
            Err(PetirError::Tolerance)
        );
        assert_eq!(
            cquad_with_status(|x: f64| x, 0.0, 1.0, 0.0, 0.0, 10).status,
            Err(PetirError::Tolerance)
        );
    }

    #[test]
    fn downdate_removes_a_node() {
        // Interpolating x on 5 nodes with the middle value lost must still
        // give a finite interval value after the downdate.
        let o = cquad_with_status(
            |x: f64| if x == 0.0 { f64::NAN } else { x * x },
            -1.0,
            1.0,
            0.0,
            1e-10,
            50,
        );
        assert!((o.value - 2.0 / 3.0).abs() < 1e-9, "{}", o.value);
    }
}
