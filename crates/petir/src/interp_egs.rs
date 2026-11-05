// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// ---------------------------------------------------------------------------
// PORTED from EGSnrc (NOT from GSL), at the maintainer's direction on
// GitHub issue #1289 ("port EGSnrc's general numerics ... fast interpolator").
//
//   Project : EGSnrc, National Research Council Canada (NRC)
//             https://github.com/nrc-cnrc/EGSnrc
//   Files   : HEN_HOUSE/egs++/egs_interpolator.h   (class EGS_Interpolator)
//             HEN_HOUSE/egs++/egs_interpolator.cpp (its implementation)
//             HEN_HOUSE/egs++/egs_functions.h:96   (`loopMax`)
//   Commit  : bd49a47aae3938f49f4100b8e018091861408b19 (2026-09-28),
//             read 2026-11-05
//   Upstream copyright, as written in both files:
//             Copyright (C) 2015 National Research Council Canada
//             Author:       Iwan Kawrakow, 2005
//             Contributors: Ernesto Mainegra-Hing, Reid Townson
//   Licence : GNU Affero General Public License, version 3 or (at your
//             option) any later version (AGPL-3.0-or-later). This port is
//             conveyed under AGPL-3.0-only, exercising the "or later" option
//             by choosing version 3. See crates/petir/NOTICE.
// ---------------------------------------------------------------------------

//! EGSnrc's fast run-time interpolator, `EGS_Interpolator`.
//!
//! # What it is, and how it differs from [`crate::interp`]
//!
//! [`crate::interp`] is GSL's interpolation: knots anywhere, a binary search
//! (`gsl_interp_bsearch`) to find the bracket, linear or natural-cubic
//! pieces, and extrapolation refused with [`crate::PetirError::Domain`].
//!
//! [`EgsInterpolator`] is a different tool for a different job. It is what
//! EGSnrc's transport kernels use for cross sections and similar tables:
//!
//! - **Uniform bins only.** The interval `xmin..xmax` is split into `n`
//!   equal bins, so the bin index is `(int)(ax + bx*x)`, one multiply-add and
//!   a truncation, no search. This is the `$SET INTERVAL` mortran macro.
//! - **Precomputed coefficients.** Each bin stores `a[i]` and `b[i]` so that
//!   the value is `a[i] + b[i]*x`, one more multiply-add (`$EVALUATE USING`).
//!   GSL's linear interpolant recomputes the slope from the two knots at
//!   every call.
//! - **Out of range is clamped, not refused.** [`EgsInterpolator::interpolate`]
//!   returns `f(xmin)` below the interval and `f(xmax)` above it (and, as
//!   upstream, for a NaN argument). There is no error path at evaluation.
//! - **Construction from a function, optionally adaptive.**
//!   [`EgsInterpolator::adaptive`] doubles the number of bins until linear
//!   interpolation at every bin midpoint is within a relative `accu` of the
//!   function, or `nmax` is reached.
//!
//! **Nothing from [`crate::interp`] was reused**: its knot storage, bracket
//! search and refusal to extrapolate are each the thing EGSnrc's class does
//! differently. What *is* shared is [`crate::PetirError`] for the
//! construction errors.
//!
//! # Interpolating in a log variable
//!
//! Upstream has no log mode in this class. EGSnrc's callers build the
//! interpolator over `ln E` and pass `ln E` at run time; do the same here.
//! That usage is one of the differential-test cases
//! (`verification_and_validation/egsnrc_interpolator/`).
//!
//! # Deviations from upstream, each flagged `DEVIATION` where it is made
//!
//! 1. A table shorter than `nbin` values, or coefficient arrays shorter than
//!    `nbin`, is refused with [`crate::PetirError::LengthMismatch`]. Upstream
//!    reads past the end of the array.
//! 2. The checks (`nbin >= 2`, `xmin < xmax`) run **before** the function is
//!    evaluated in [`EgsInterpolator::from_function`]. Upstream evaluates the
//!    function at `nbin` points first (at `NaN` when `nbin == 1`) and then
//!    calls `egsFatal`.
//! 3. `egsFatal` is an `Err`, not a process exit.
//! 4. Where upstream's in-range bin index would fall past the stored
//!    coefficients (only possible through round-off when `|xmin|` is huge
//!    against the bin width), [`EgsInterpolator::interpolate`] uses the last
//!    stored bin; upstream reads out of bounds. The no-check fast paths return
//!    `None` there instead.
//! 5. An interpolator is built by a constructor and never re-initialised;
//!    upstream's empty constructor, `initialize` on an existing object and
//!    `own_data` have no Rust counterpart. Coefficients given to
//!    [`EgsInterpolator::from_coefficients`] are moved in rather than borrowed.
//!
//! # An upstream defect, NOT reproduced
//!
//! Two of upstream's constructors pass `Xmax` for `Xmin`
//! (`egs_interpolator.cpp`, the function constructor and the `a`/`b`
//! constructor both call `initialize(nbin,Xmax,Xmax,...)`), so `check` sees
//! `Xmin >= Xmax` and `egsFatal` fires on every call. The `initialize`
//! methods they meant to call are correct, and they are what this module
//! ports. The defect was confirmed by running upstream (see the V&V record).
//!
//! # Units
//!
//! Bare `f64`, as everywhere in PETIR's numerics.

use alloc::vec::Vec;

use crate::{PetirError, Result};

/// Upstream's iteration guard, `loopMax` (`egs_functions.h:96`,
/// `const EGS_I64 loopMax = 1e10;`).
pub const LOOP_MAX: i64 = 10_000_000_000;

/// Upstream's default `nmax` for [`EgsInterpolator::adaptive`]
/// (`egs_interpolator.h`, `int nmax = 1024`).
pub const DEFAULT_NMAX: usize = 1024;

/// Upstream's default `accu` for [`EgsInterpolator::adaptive`]
/// (`egs_interpolator.h`, `EGS_Float accu = 1e-4`).
pub const DEFAULT_ACCU: f64 = 1e-4;

/// EGSnrc's `EGS_Interpolator`: piecewise-linear interpolation on uniform
/// bins with precomputed `a[i] + b[i]*x` coefficients.
///
/// ```
/// use petir::interp_egs::EgsInterpolator;
///
/// // Five values on 0..4, i.e. four bins of width 1.
/// let f = EgsInterpolator::from_values(5, 0.0, 4.0, &[0.0, 1.0, 4.0, 9.0, 16.0])?;
/// assert_eq!(f.interpolate(2.5), 6.5);   // halfway between 4 and 9
/// assert_eq!(f.interpolate(-3.0), 0.0);  // clamped to f(xmin)
/// assert_eq!(f.interpolate(99.0), 16.0); // clamped to f(xmax)
/// # Ok::<(), petir::PetirError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct EgsInterpolator {
    /// Number of bins, upstream's `n` (`nbin - 1`).
    n: i32,
    /// `x -> index` conversion: `index = (int)(ax + bx*x)`.
    ax: f64,
    /// See `ax`.
    bx: f64,
    /// Interpolation coefficients: `f(x) = a[i] + b[i]*x` in bin `i`.
    a: Vec<f64>,
    /// See `a`.
    b: Vec<f64>,
    /// Lower end of the interpolation interval.
    xmin: f64,
    /// Upper end of the interpolation interval.
    xmax: f64,
    /// Value returned for `x <= xmin`.
    fmin: f64,
    /// Value returned for `x >= xmax` (and for `NaN`).
    fmax: f64,
}

/// Upstream's `check` (`egs_interpolator.cpp`): `nbin < 2` and
/// `Xmin >= Xmax` are fatal. A `NaN` endpoint passes, as upstream.
///
/// Returns upstream's `n = nbin - 1` as `int`.
fn check(nbin: usize, xmin: f64, xmax: f64) -> Result<i32> {
    if nbin < 2 {
        return Err(PetirError::Invalid);
    }
    if xmin >= xmax {
        return Err(PetirError::Domain);
    }
    // Upstream's `n` is an `int`; a table longer than that cannot exist there.
    i32::try_from(nbin - 1).map_err(|_| PetirError::Invalid)
}

impl EgsInterpolator {
    /// Interpolate `values`, taken to sit at `nbin` uniformly spaced points
    /// from `xmin` to `xmax` inclusive (`nbin - 1` bins).
    ///
    /// Upstream: `EGS_Interpolator::initialize(int nbin, EGS_Float Xmin,
    /// EGS_Float Xmax, const EGS_Float *values)`, including the extra
    /// sub-interval at the top (`a[n] = a[n-1]`), which "takes care of
    /// round-off errors via extrapolation" and "mimics PEGS4 PWLF QFIT".
    ///
    /// # Errors
    ///
    /// - [`PetirError::Invalid`] if `nbin < 2` (upstream `egsFatal`).
    /// - [`PetirError::Domain`] if `xmin >= xmax` (upstream `egsFatal`).
    /// - [`PetirError::LengthMismatch`] if `values.len() < nbin`.
    ///   **DEVIATION:** upstream reads past the end of the array.
    pub fn from_values(nbin: usize, xmin: f64, xmax: f64, values: &[f64]) -> Result<Self> {
        let n = check(nbin, xmin, xmax)?;
        if values.len() < nbin {
            return Err(PetirError::LengthMismatch {
                expected: nbin,
                found: values.len(),
            });
        }
        let nu = nbin - 1;
        let fmin = values.first().copied().ok_or(PetirError::Invalid)?;
        let fmax = values.get(nu).copied().ok_or(PetirError::Invalid)?;
        // EGS_Float dx = (xmax-xmin)/n;  bx = 1/dx;  ax = -xmin*bx;
        let dx = (xmax - xmin) / f64::from(n);
        let bx = 1.0 / dx;
        let ax = -xmin * bx;
        let mut a = Vec::with_capacity(nbin);
        let mut b = Vec::with_capacity(nbin);
        // for (j=0; j<n; j++) {
        //     b[j] = (values[j+1]-values[j])*bx;
        //     a[j] = values[j] - b[j]*(xmin + dx*j);
        // }
        for (j, (&v0, &v1)) in values
            .iter()
            .zip(values.iter().skip(1))
            .take(nu)
            .enumerate()
        {
            let bj = (v1 - v0) * bx;
            let aj = v0 - bj * (xmin + dx * (j as f64));
            a.push(aj);
            b.push(bj);
        }
        // a[n] = a[n-1]; b[n] = b[n-1];
        let a_last = a.last().copied().ok_or(PetirError::Invalid)?;
        let b_last = b.last().copied().ok_or(PetirError::Invalid)?;
        a.push(a_last);
        b.push(b_last);
        Ok(Self {
            n,
            ax,
            bx,
            a,
            b,
            xmin,
            xmax,
            fmin,
            fmax,
        })
    }

    /// Tabulate `func` at `nbin` uniformly spaced points `xmin + dx*j`,
    /// `dx = (xmax - xmin)/(nbin - 1)`, and interpolate between them.
    ///
    /// Upstream: `EGS_Interpolator::initialize(int nbin, EGS_Float Xmin,
    /// EGS_Float Xmax, EGS_InterpolatorFuncion func, void *data)`. The
    /// `void *data` argument is whatever the closure captures.
    ///
    /// **Not upstream's constructor of the same signature**, which passes
    /// `Xmax` for `Xmin` and always fails; see the module docs.
    ///
    /// # Errors
    ///
    /// As [`EgsInterpolator::from_values`]. **DEVIATION:** the checks run
    /// before `func` is called; upstream calls it `nbin` times first.
    pub fn from_function<F: FnMut(f64) -> f64>(
        nbin: usize,
        xmin: f64,
        xmax: f64,
        mut func: F,
    ) -> Result<Self> {
        let n = check(nbin, xmin, xmax)?;
        // EGS_Float dx = (Xmax - Xmin)/(nbin-1);
        // for (j=0; j<nbin; j++) aux[j] = func(Xmin + dx*j, data);
        let dx = (xmax - xmin) / f64::from(n);
        let aux: Vec<f64> = (0..nbin).map(|j| func(xmin + dx * (j as f64))).collect();
        Self::from_values(nbin, xmin, xmax, &aux)
    }

    /// Wrap existing coefficients: `f(x) = a[i] + b[i]*x` in bin `i` of
    /// `nbin - 1` uniform bins on `xmin..xmax`.
    ///
    /// Upstream: `EGS_Interpolator::initialize(int nbin, EGS_Float Xmin,
    /// EGS_Float Xmax, EGS_Float *A, EGS_Float *B)`, used there to view the
    /// mortran EGSnrc interpolation arrays without copying them. Here the
    /// vectors are moved in (no lifetimes in PETIR). As upstream,
    /// `fmin = a[0] + b[0]*xmin` and `fmax = a[n-1] + b[n-1]*xmax`, with
    /// `n = nbin - 1`.
    ///
    /// **Not upstream's constructor of the same signature**, which passes
    /// `Xmax` for `Xmin` and always fails; see the module docs.
    ///
    /// # Errors
    ///
    /// - [`PetirError::Invalid`] if `nbin < 2`; [`PetirError::Domain`] if
    ///   `xmin >= xmax` (both upstream `egsFatal`).
    /// - [`PetirError::LengthMismatch`] if `a` or `b` holds fewer than `nbin`
    ///   entries. **DEVIATION:** upstream does not know the arrays' lengths;
    ///   `nbin` entries are what its own `values` constructor allocates, and
    ///   the in-range index can reach `n` through round-off at the top.
    pub fn from_coefficients(
        nbin: usize,
        xmin: f64,
        xmax: f64,
        a: Vec<f64>,
        b: Vec<f64>,
    ) -> Result<Self> {
        let n = check(nbin, xmin, xmax)?;
        for len in [a.len(), b.len()] {
            if len < nbin {
                return Err(PetirError::LengthMismatch {
                    expected: nbin,
                    found: len,
                });
            }
        }
        let nu = nbin - 1;
        let dx = (xmax - xmin) / f64::from(n);
        let bx = 1.0 / dx;
        let ax = -xmin * bx;
        let a0 = a.first().copied().ok_or(PetirError::Invalid)?;
        let b0 = b.first().copied().ok_or(PetirError::Invalid)?;
        let an = a.get(nu - 1).copied().ok_or(PetirError::Invalid)?;
        let bn = b.get(nu - 1).copied().ok_or(PetirError::Invalid)?;
        let fmin = a0 + b0 * xmin;
        let fmax = an + bn * xmax;
        Ok(Self {
            n,
            ax,
            bx,
            a,
            b,
            xmin,
            xmax,
            fmin,
            fmax,
        })
    }

    /// Choose the number of bins adaptively so that linear interpolation of
    /// `func` is within a relative `accu` at every bin midpoint, using at most
    /// `nmax` points ([`DEFAULT_NMAX`], [`DEFAULT_ACCU`] are upstream's
    /// defaults).
    ///
    /// Upstream: `EGS_Interpolator::initialize(EGS_Float Xmin, EGS_Float
    /// Xmax, EGS_InterpolatorFuncion func, void *data, int nmax, EGS_Float
    /// accu)`, step for step:
    ///
    /// 1. `nmax <= 9`: plain [`EgsInterpolator::from_function`] with `nmax`.
    /// 2. Start from 9 points. Evaluate `func` at the midpoints
    ///    `xmin + ddx*(0.5 + j)`; if every `|0.5*(f_j + f_{j+1}) / f_mid - 1|`
    ///    is `<= accu`, interpolate the current points.
    /// 3. Otherwise, if `2*nnow - 1 > nmax`, give up and use
    ///    [`EgsInterpolator::from_function`] with `nmax` points (re-evaluating
    ///    `func` there).
    /// 4. Otherwise interleave the midpoint values into the table, double,
    ///    and repeat. **The interleaved node values are the midpoint values
    ///    already computed at `xmin + ddx_old*(0.5 + j)`**, not re-evaluated
    ///    at the new grid's `xmin + ddx_new*(2j + 1)`; this port keeps that.
    ///
    /// The test is relative and has no guard: a midpoint value of `0` gives
    /// an infinite or `NaN` error, and `NaN > accu` is false, so a `NaN`
    /// error passes (as upstream).
    ///
    /// # Errors
    ///
    /// - [`PetirError::Invalid`] if `nmax < 2`; [`PetirError::Domain`] if
    ///   `xmin >= xmax` (both upstream `egsFatal`).
    /// - [`PetirError::MaxIterations`] after [`LOOP_MAX`] passes (upstream
    ///   `egsFatal`; unreachable in practice, since the bin count doubles
    ///   each pass).
    pub fn adaptive<F: FnMut(f64) -> f64>(
        xmin: f64,
        xmax: f64,
        mut func: F,
        nmax: usize,
        accu: f64,
    ) -> Result<Self> {
        check(nmax, xmin, xmax)?;
        if nmax <= 9 {
            return Self::from_function(nmax, xmin, xmax, func);
        }
        let mut nnow: usize = 9;
        let mut ddx = (xmax - xmin) / ((nnow - 1) as f64);
        let mut tmp: Vec<f64> = (0..nnow).map(|j| func(xmin + ddx * (j as f64))).collect();
        let mut loop_count: i64 = 0;
        while loop_count <= LOOP_MAX {
            if loop_count == LOOP_MAX {
                return Err(PetirError::MaxIterations);
            }
            // tmp1[j] = func(Xmin + ddx*(0.5+j), data);
            let tmp1: Vec<f64> = (0..nnow - 1)
                .map(|j| func(xmin + ddx * (0.5 + (j as f64))))
                .collect();
            // fint = 0.5*(tmp[j] + tmp[j+1]); err = fabs(fint/tmp1[j]-1);
            // if (err > accu) { ok = false; break; }
            let ok =
                !tmp.iter()
                    .zip(tmp.iter().skip(1))
                    .zip(tmp1.iter())
                    .any(|((&t0, &t1), &tm)| {
                        let fint = 0.5 * (t0 + t1);
                        let err = (fint / tm - 1.0).abs();
                        err > accu
                    });
            if ok {
                return Self::from_values(nnow, xmin, xmax, &tmp);
            }
            let next = nnow.saturating_mul(2) - 1;
            if next > nmax {
                break;
            }
            // aux[0] = tmp[0]; then alternate tmp1[j-1], tmp[j].
            let mut aux = Vec::with_capacity(next);
            if let Some(&t0) = tmp.first() {
                aux.push(t0);
            }
            for (&tm, &t) in tmp1.iter().zip(tmp.iter().skip(1)) {
                aux.push(tm);
                aux.push(t);
            }
            tmp = aux;
            nnow = next;
            ddx = (xmax - xmin) / ((nnow - 1) as f64);
            loop_count += 1;
        }
        Self::from_function(nmax, xmin, xmax, func)
    }

    /// Coefficients of bin `k`, `(a[k], b[k])`, if stored.
    fn coefficients(&self, k: usize) -> Option<(f64, f64)> {
        Some((self.a.get(k).copied()?, self.b.get(k).copied()?))
    }

    /// The value at `x`: `f(xmin)` if `x <= xmin`, `f(xmax)` if `x >= xmax`
    /// or `x` is `NaN`, and `a[i] + b[i]*x` with `i = (int)(ax + bx*x)`
    /// (clamped below at 0) in between.
    ///
    /// Upstream: `EGS_Interpolator::interpolate` (`egs_interpolator.h`).
    /// **DEVIATION** (only where upstream reads out of bounds): an index past
    /// the stored coefficients uses the last stored bin.
    #[inline]
    pub fn interpolate(&self, x: f64) -> f64 {
        if x > self.xmin && x < self.xmax {
            let i = (self.ax + self.bx * x) as i32;
            let i = if i < 0 { 0 } else { i };
            let last = self.a.len().min(self.b.len()).saturating_sub(1);
            let k = (i as usize).min(last);
            match self.coefficients(k) {
                Some((ak, bk)) => ak + bk * x,
                // Unreachable: construction stores at least two bins, and
                // `k <= last`.
                None => self.fmax,
            }
        } else if x <= self.xmin {
            self.fmin
        } else {
            self.fmax
        }
    }

    /// The value at `x` with no range check: `a[i] + b[i]*x`,
    /// `i = (int)(ax + bx*x)`.
    ///
    /// Upstream: `EGS_Interpolator::interpolateFast(EGS_Float)`, valid only
    /// for `x` in `xmin..=xmax`. Returns `Some` there, bit-identical to
    /// upstream. **DEVIATION:** where upstream would read outside its arrays
    /// (an `x` out of range), this returns `None` instead.
    #[inline]
    pub fn interpolate_fast(&self, x: f64) -> Option<f64> {
        self.interpolate_fast_at(self.get_index_fast(x), x)
    }

    /// Upstream's `getIndex`: `(int)(ax + bx*x)` inside the interval, `0`
    /// for `x <= xmin`, and `n - 1` for `x >= xmax` or `NaN`.
    ///
    /// Not clamped below inside the interval, as upstream; the result is
    /// `int`, hence `i32`.
    #[inline]
    pub fn get_index(&self, x: f64) -> i32 {
        if x > self.xmin && x < self.xmax {
            (self.ax + self.bx * x) as i32
        } else if x <= self.xmin {
            0
        } else {
            self.n - 1
        }
    }

    /// Upstream's `getIndexFast`: `(int)(ax + bx*x)`, no checks. Rust's
    /// float-to-int cast truncates toward zero exactly as C's does for every
    /// value an `int` can hold (and saturates, where C is undefined).
    #[inline]
    pub fn get_index_fast(&self, x: f64) -> i32 {
        (self.ax + self.bx * x) as i32
    }

    /// Upstream's `interpolateFast(int i, EGS_Float x)`: `a[i] + b[i]*x`
    /// for an index from [`EgsInterpolator::get_index`] or
    /// [`EgsInterpolator::get_index_fast`] (possibly of another interpolator
    /// on the same bins). `None` where upstream would read outside its arrays.
    #[inline]
    pub fn interpolate_fast_at(&self, i: i32, x: f64) -> Option<f64> {
        let k = usize::try_from(i).ok()?;
        let (ak, bk) = self.coefficients(k)?;
        Some(ak + bk * x)
    }

    /// Lower interpolation limit, upstream's `getXmin`.
    #[inline]
    pub fn xmin(&self) -> f64 {
        self.xmin
    }

    /// Upper interpolation limit, upstream's `getXmax`.
    #[inline]
    pub fn xmax(&self) -> f64 {
        self.xmax
    }

    /// `a[i]`, upstream's `get_a`; `None` outside the stored coefficients.
    #[inline]
    pub fn get_a(&self, i: i32) -> Option<f64> {
        self.a.get(usize::try_from(i).ok()?).copied()
    }

    /// `b[i]`, upstream's `get_b`; `None` outside the stored coefficients.
    #[inline]
    pub fn get_b(&self, i: i32) -> Option<f64> {
        self.b.get(usize::try_from(i).ok()?).copied()
    }

    /// Number of bins, upstream's private `n` (`nbin - 1`). Not an upstream
    /// accessor; exposed so a caller can see what [`EgsInterpolator::adaptive`]
    /// chose.
    #[inline]
    pub fn bins(&self) -> i32 {
        self.n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn guards_match_upstream_check() {
        assert_eq!(
            EgsInterpolator::from_values(1, 0.0, 1.0, &[1.0]),
            Err(PetirError::Invalid)
        );
        assert_eq!(
            EgsInterpolator::from_values(2, 1.0, 1.0, &[1.0, 2.0]),
            Err(PetirError::Domain)
        );
        assert_eq!(
            EgsInterpolator::from_values(2, 2.0, 1.0, &[1.0, 2.0]),
            Err(PetirError::Domain)
        );
        // A NaN endpoint passes upstream's check, and here.
        assert!(EgsInterpolator::from_values(2, f64::NAN, 1.0, &[1.0, 2.0]).is_ok());
        assert_eq!(
            EgsInterpolator::from_values(4, 0.0, 1.0, &[1.0, 2.0]),
            Err(PetirError::LengthMismatch {
                expected: 4,
                found: 2
            })
        );
        let mut calls = 0;
        assert_eq!(
            EgsInterpolator::from_function(1, 0.0, 1.0, |x| {
                calls += 1;
                x
            }),
            Err(PetirError::Invalid)
        );
        assert_eq!(calls, 0);
        assert_eq!(
            EgsInterpolator::from_coefficients(3, 0.0, 1.0, vec![0.0; 3], vec![0.0; 2]),
            Err(PetirError::LengthMismatch {
                expected: 3,
                found: 2
            })
        );
        assert_eq!(
            EgsInterpolator::adaptive(0.0, 1.0, |x| x, 1, DEFAULT_ACCU),
            Err(PetirError::Invalid)
        );
        assert_eq!(
            EgsInterpolator::adaptive(1.0, 0.0, |x| x, DEFAULT_NMAX, DEFAULT_ACCU),
            Err(PetirError::Domain)
        );
    }

    #[test]
    fn clamps_and_indices_follow_upstream() {
        let f = EgsInterpolator::from_values(5, 0.0, 4.0, &[0.0, 1.0, 4.0, 9.0, 16.0]).unwrap();
        assert_eq!(f.bins(), 4);
        assert_eq!(f.interpolate(2.5), 6.5);
        assert_eq!(f.interpolate(0.0), 0.0);
        assert_eq!(f.interpolate(4.0), 16.0);
        assert_eq!(f.interpolate(f64::NAN), 16.0);
        assert_eq!(f.get_index(-1.0), 0);
        assert_eq!(f.get_index(4.0), 3);
        assert_eq!(f.get_index(f64::NAN), 3);
        assert_eq!(f.get_index(2.5), 2);
        assert_eq!(f.get_index_fast(2.5), 2);
        assert_eq!(f.interpolate_fast(2.5), Some(6.5));
        assert_eq!(f.interpolate_fast_at(2, 2.5), Some(6.5));
        // The extra top bin copies the last one.
        assert_eq!(f.get_a(4), f.get_a(3));
        assert_eq!(f.get_b(4), f.get_b(3));
        assert_eq!(f.interpolate_fast(4.0), Some(16.0));
        // Out of range on the no-check path is None, not a read past the end.
        assert_eq!(f.interpolate_fast(-7.0), None);
        assert_eq!(f.interpolate_fast(70.0), None);
        assert_eq!(f.get_a(-1), None);
        assert_eq!(f.get_b(5), None);
        assert_eq!((f.xmin(), f.xmax()), (0.0, 4.0));
    }

    #[test]
    fn coefficient_mode_end_values() {
        let f = EgsInterpolator::from_coefficients(
            3,
            1.0,
            3.0,
            vec![1.0, 2.0, 9.0],
            vec![0.5, -1.0, 9.0],
        )
        .unwrap();
        assert_eq!(f.interpolate(0.0), 1.0 + 0.5 * 1.0);
        assert_eq!(f.interpolate(5.0), 2.0 - 3.0);
        assert_eq!(f.interpolate(2.5), 2.0 - 2.5);
    }

    #[test]
    fn adaptive_small_nmax_and_cap_branches() {
        // nmax <= 9: a plain tabulation with nmax points.
        let f = EgsInterpolator::adaptive(0.0, 1.0, |x| 1.0 + x * x, 7, DEFAULT_ACCU).unwrap();
        assert_eq!(f.bins(), 6);
        // A straight line converges on the first pass: 9 points, 8 bins.
        let g =
            EgsInterpolator::adaptive(0.0, 1.0, |x| 2.0 + x, DEFAULT_NMAX, DEFAULT_ACCU).unwrap();
        assert_eq!(g.bins(), 8);
        // Too tight a tolerance for nmax: falls back to nmax points.
        let h = EgsInterpolator::adaptive(0.0, 1.0, |x| 1.0 + x * x, 20, 1e-12).unwrap();
        assert_eq!(h.bins(), 19);
    }
}
