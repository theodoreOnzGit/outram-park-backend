// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PETIR is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along
// with PETIR.  If not, see <https://www.gnu.org/licenses/>.
//
// PORTED from the GNU Scientific Library (GSL) 2.8, commit
// cf180cd7fbd06039a577f9c9ff0b428784765ac1, read 2026-09-28.
// Copyright (C) 1996, 1997, 1998, 1999, 2000, 2007 Brian Gough
// (qags.c, qelg.c, qpsrt.c, qpsrt2.c, util.c; initialise.c adds 2001;
// positivity.c, set_initial.c and reset.c carry no header of their own and
// are #included into qags.c, whose notice covers them).
// GPL-3.0-or-later (verified from the per-file headers; see NOTICE).
//
// GSL's integration layer is itself a translation of QUADPACK (Piessens,
// de Doncker-Kapenga, Uberhuber and Kahaner, 1983), which is public domain.
//
//   integration/qags.c        qags (lines 194-570), gsl_integration_qags (39-51)
//   integration/qelg.c        extrapolation_table, initialise_table,
//                             append_table, qelg        -> ExtrapolationTable
//   integration/qpsrt.c       qpsrt                     -> Workspace::qpsrt
//   integration/qpsrt2.c      increase_nrmax, large_interval
//   integration/util.c        update, retrieve, sum_results,
//                             subinterval_too_small
//   integration/initialise.c  initialise
//   integration/set_initial.c set_initial_result
//   integration/reset.c       reset_nrmax
//   integration/positivity.c  test_positivity

//! QAGS — adaptive Gauss-Kronrod quadrature **with epsilon-algorithm
//! extrapolation**. GSL's `gsl_integration_qags`, which is QUADPACK's `dqagse`.
//!
//! # What extrapolation buys
//!
//! [`super::qag`] bisects the worst sub-interval until the summed error
//! estimate falls under the tolerance. Near an integrable singularity
//! (`1/sqrt(x)`, `ln x`) or a sharp interior peak, the sequence of partial
//! sums converges slowly — algebraically in the interval width — and `qag`
//! runs out of sub-intervals. QAGS notices when the interval being bisected
//! is the smallest one, and feeds the sequence of area estimates into Wynn's
//! epsilon algorithm (`qelg`), which accelerates exactly that kind of
//! sequence. The extrapolated value is accepted once *its* error estimate
//! meets the tolerance.
//!
//! # Differences from upstream, all deliberate
//!
//! - The workspace is allocated inside the call (`limit` entries) instead of
//!   being passed in, so GSL's "iteration limit exceeds available workspace"
//!   check has no counterpart.
//! - Non-finite limits return [`PetirError::Domain`], as [`super::qag`] does.
//!   Upstream does not check them.
//! - Every array access is a checked `get`; an index that upstream would read
//!   out of bounds (none is reachable for a well-formed workspace) returns
//!   [`PetirError::Invalid`] instead of undefined behaviour.
//!
//! Everything else — the order of the arithmetic, the round-off counters, the
//! `qpsrt` ordering and the table shifts in `qelg` — follows the C line for
//! line, so iterates agree with compiled GSL (see
//! `tests/gsl_qags_code_to_code.rs`).

use alloc::vec;
use alloc::vec::Vec;

// Under a std-linked build (`cargo test`) f64's inherent abs shadows this
// trait, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use super::{kronrod, Integral, QkRule};
use crate::scalar::{DBL_EPSILON, DBL_MIN};
use crate::{PetirError, Result};

/// `GSL_DBL_MAX`.
const DBL_MAX: f64 = f64::MAX;

/// Checked read of a `f64` slot.
fn rd(v: &[f64], i: usize) -> Option<f64> {
    v.get(i).copied()
}

/// Checked write of a `f64` slot.
fn wr(v: &mut [f64], i: usize, x: f64) -> Option<()> {
    *v.get_mut(i)? = x;
    Some(())
}

/// Checked read of a `usize` slot.
fn rdu(v: &[usize], i: usize) -> Option<usize> {
    v.get(i).copied()
}

/// Checked write of a `usize` slot.
fn wru(v: &mut [usize], i: usize, x: usize) -> Option<()> {
    *v.get_mut(i)? = x;
    Some(())
}

/// A signed index (upstream's `int`) as a `usize`, `None` if negative.
fn ix(i: isize) -> Option<usize> {
    usize::try_from(i).ok()
}

/// `gsl_integration_workspace`, allocated for one call.
struct Workspace {
    limit: usize,
    size: usize,
    nrmax: usize,
    i: usize,
    maximum_level: usize,
    alist: Vec<f64>,
    blist: Vec<f64>,
    rlist: Vec<f64>,
    elist: Vec<f64>,
    order: Vec<usize>,
    level: Vec<usize>,
}

impl Workspace {
    /// `gsl_integration_workspace_alloc` followed by `initialise.c`.
    fn new(limit: usize, a: f64, b: f64) -> Option<Self> {
        let mut w = Workspace {
            limit,
            size: 0,
            nrmax: 0,
            i: 0,
            maximum_level: 0,
            alist: vec![0.0; limit],
            blist: vec![0.0; limit],
            rlist: vec![0.0; limit],
            elist: vec![0.0; limit],
            order: vec![0; limit],
            level: vec![0; limit],
        };
        wr(&mut w.alist, 0, a)?;
        wr(&mut w.blist, 0, b)?;
        Some(w)
    }

    /// `set_initial.c`, `set_initial_result`.
    fn set_initial_result(&mut self, result: f64, error: f64) -> Option<()> {
        self.size = 1;
        wr(&mut self.rlist, 0, result)?;
        wr(&mut self.elist, 0, error)
    }

    /// `util.c`, `retrieve`: `(a, b, r, e)` of the interval `i`.
    fn retrieve(&self) -> Option<(f64, f64, f64, f64)> {
        let i = self.i;
        Some((
            rd(&self.alist, i)?,
            rd(&self.blist, i)?,
            rd(&self.rlist, i)?,
            rd(&self.elist, i)?,
        ))
    }

    /// `util.c`, `update`: store the two halves, then `qpsrt`.
    #[allow(clippy::too_many_arguments)]
    fn update(
        &mut self,
        a1: f64,
        b1: f64,
        area1: f64,
        error1: f64,
        a2: f64,
        b2: f64,
        area2: f64,
        error2: f64,
    ) -> Option<()> {
        let i_max = self.i;
        let i_new = self.size;
        let new_level = rdu(&self.level, i_max)? + 1;

        // Append the newly-created intervals to the list.
        if error2 > error1 {
            wr(&mut self.alist, i_max, a2)?; // blist[maxerr] is already == b2
            wr(&mut self.rlist, i_max, area2)?;
            wr(&mut self.elist, i_max, error2)?;
            wru(&mut self.level, i_max, new_level)?;

            wr(&mut self.alist, i_new, a1)?;
            wr(&mut self.blist, i_new, b1)?;
            wr(&mut self.rlist, i_new, area1)?;
            wr(&mut self.elist, i_new, error1)?;
            wru(&mut self.level, i_new, new_level)?;
        } else {
            wr(&mut self.blist, i_max, b1)?; // alist[maxerr] is already == a1
            wr(&mut self.rlist, i_max, area1)?;
            wr(&mut self.elist, i_max, error1)?;
            wru(&mut self.level, i_max, new_level)?;

            wr(&mut self.alist, i_new, a2)?;
            wr(&mut self.blist, i_new, b2)?;
            wr(&mut self.rlist, i_new, area2)?;
            wr(&mut self.elist, i_new, error2)?;
            wru(&mut self.level, i_new, new_level)?;
        }

        self.size += 1;

        if new_level > self.maximum_level {
            self.maximum_level = new_level;
        }

        self.qpsrt()
    }

    /// `qpsrt.c`: maintain the descending ordering of the error estimates and
    /// point `i` at the interval with the `nrmax`-th largest error.
    fn qpsrt(&mut self) -> Option<()> {
        let last = self.size.checked_sub(1)?;
        let limit = self.limit;

        let mut i_nrmax = self.nrmax;
        let mut i_maxerr = rdu(&self.order, i_nrmax)?;

        // Check whether the list contains more than two error estimates.
        if last < 2 {
            wru(&mut self.order, 0, 0)?;
            wru(&mut self.order, 1, 1)?;
            self.i = i_maxerr;
            return Some(());
        }

        let errmax = rd(&self.elist, i_maxerr)?;

        // Only executed if, due to a difficult integrand, subdivision
        // increased the error estimate. In the normal case the insert
        // procedure should start after the nrmax-th largest error estimate.
        while i_nrmax > 0 && errmax > rd(&self.elist, rdu(&self.order, i_nrmax - 1)?)? {
            let o = rdu(&self.order, i_nrmax - 1)?;
            wru(&mut self.order, i_nrmax, o)?;
            i_nrmax -= 1;
        }

        // The number of elements to maintain in descending order depends on
        // the number of subdivisions still allowed.
        let top: isize = if last < (limit / 2 + 2) {
            isize::try_from(last).ok()?
        } else {
            isize::try_from(limit - last + 1).ok()?
        };

        // Insert errmax by traversing the list top-down.
        let mut i: isize = isize::try_from(i_nrmax).ok()? + 1;

        // The order of the tests matters (upstream: "to prevent a
        // segmentation fault").
        while i < top && errmax < rd(&self.elist, rdu(&self.order, ix(i)?)?)? {
            let o = rdu(&self.order, ix(i)?)?;
            wru(&mut self.order, ix(i - 1)?, o)?;
            i += 1;
        }

        wru(&mut self.order, ix(i - 1)?, i_maxerr)?;

        // Insert errmin by traversing the list bottom-up.
        let errmin = rd(&self.elist, last)?;

        let mut k: isize = top - 1;

        while k > i - 2 && errmin >= rd(&self.elist, rdu(&self.order, ix(k)?)?)? {
            let o = rdu(&self.order, ix(k)?)?;
            wru(&mut self.order, ix(k + 1)?, o)?;
            k -= 1;
        }

        wru(&mut self.order, ix(k + 1)?, last)?;

        // Set i_max and e_max.
        i_maxerr = rdu(&self.order, i_nrmax)?;

        self.i = i_maxerr;
        self.nrmax = i_nrmax;
        Some(())
    }

    /// `qpsrt2.c`, `increase_nrmax`: `Some(true)` if a larger interval with a
    /// big error was found to bisect next.
    fn increase_nrmax(&mut self) -> Option<bool> {
        let id = self.nrmax;
        let limit = self.limit;
        let last = self.size.checked_sub(1)?;

        let jupbnd = if last > (1 + limit / 2) {
            limit + 1 - last
        } else {
            last
        };

        let mut k = id;
        while k <= jupbnd {
            let i_max = rdu(&self.order, self.nrmax)?;
            self.i = i_max;
            if rdu(&self.level, i_max)? < self.maximum_level {
                return Some(true);
            }
            self.nrmax += 1;
            k += 1;
        }
        Some(false)
    }

    /// `qpsrt2.c`, `large_interval`.
    fn large_interval(&self) -> Option<bool> {
        Some(rdu(&self.level, self.i)? < self.maximum_level)
    }

    /// `reset.c`, `reset_nrmax`.
    fn reset_nrmax(&mut self) -> Option<()> {
        self.nrmax = 0;
        self.i = rdu(&self.order, 0)?;
        Some(())
    }

    /// `util.c`, `sum_results`.
    fn sum_results(&self) -> f64 {
        let mut result_sum = 0.0;
        for r in self.rlist.iter().take(self.size) {
            result_sum += *r;
        }
        result_sum
    }
}

/// `util.c`, `subinterval_too_small`.
fn subinterval_too_small(a1: f64, a2: f64, b2: f64) -> bool {
    let e = DBL_EPSILON;
    let u = DBL_MIN;
    let tmp = (1.0 + 100.0 * e) * (a2.abs() + 1000.0 * u);
    a1.abs() <= tmp && b2.abs() <= tmp
}

/// `positivity.c`, `test_positivity`: does `f` keep one sign?
fn test_positivity(result: f64, resabs: f64) -> bool {
    result.abs() >= (1.0 - 50.0 * DBL_EPSILON) * resabs
}

/// `qelg.c`, `struct extrapolation_table`.
struct ExtrapolationTable {
    n: usize,
    rlist2: [f64; 52],
    nres: usize,
    res3la: [f64; 3],
}

impl ExtrapolationTable {
    /// `initialise_table`.
    fn new() -> Self {
        ExtrapolationTable {
            n: 0,
            rlist2: [0.0; 52],
            nres: 0,
            res3la: [0.0; 3],
        }
    }

    /// `append_table`.
    fn append(&mut self, y: f64) -> Option<()> {
        wr(&mut self.rlist2, self.n, y)?;
        self.n += 1;
        Some(())
    }

    /// `qelg`: Wynn's epsilon algorithm on the table, returning the
    /// extrapolated `(result, abserr)`.
    #[allow(clippy::too_many_lines)]
    fn qelg(&mut self) -> Option<(f64, f64)> {
        let epstab = &mut self.rlist2;
        let res3la = &mut self.res3la;
        let n = self.n.checked_sub(1)?;

        let current = rd(epstab, n)?;

        let mut absolute = DBL_MAX;
        let mut relative = 5.0 * DBL_EPSILON * current.abs();

        let newelm = n / 2;
        let n_orig = n;
        let mut n_final = n;

        let nres_orig = self.nres;

        let mut result = current;
        let mut abserr = DBL_MAX;

        if n < 2 {
            result = current;
            abserr = absolute.max(relative);
            return Some((result, abserr));
        }

        let en = rd(epstab, n)?;
        wr(epstab, n + 2, en)?;
        wr(epstab, n, DBL_MAX)?;

        for i in 0..newelm {
            let mut res = rd(epstab, n - 2 * i + 2)?;
            let e0 = rd(epstab, n - 2 * i - 2)?;
            let e1 = rd(epstab, n - 2 * i - 1)?;
            let e2 = res;

            let e1abs = e1.abs();
            let delta2 = e2 - e1;
            let err2 = delta2.abs();
            let tol2 = e2.abs().max(e1abs) * DBL_EPSILON;
            let delta3 = e1 - e0;
            let err3 = delta3.abs();
            let tol3 = e1abs.max(e0.abs()) * DBL_EPSILON;

            if err2 <= tol2 && err3 <= tol3 {
                // If e0, e1 and e2 are equal to within machine accuracy,
                // convergence is assumed.
                result = res;
                absolute = err2 + err3;
                relative = 5.0 * DBL_EPSILON * res.abs();
                abserr = absolute.max(relative);
                return Some((result, abserr));
            }

            let e3 = rd(epstab, n - 2 * i)?;
            wr(epstab, n - 2 * i, e1)?;
            let delta1 = e1 - e3;
            let err1 = delta1.abs();
            let tol1 = e1abs.max(e3.abs()) * DBL_EPSILON;

            // If two elements are very close to each other, omit a part of
            // the table by adjusting the value of n.
            if err1 <= tol1 || err2 <= tol2 || err3 <= tol3 {
                n_final = 2 * i;
                break;
            }

            let ss = (1.0 / delta1 + 1.0 / delta2) - 1.0 / delta3;

            // Test to detect irregular behaviour in the table, and eventually
            // omit a part of the table by adjusting the value of n.
            if (ss * e1).abs() <= 0.0001 {
                n_final = 2 * i;
                break;
            }

            // Compute a new element and eventually adjust the value of
            // result.
            res = e1 + 1.0 / ss;
            wr(epstab, n - 2 * i, res)?;

            {
                let error = err2 + (res - e2).abs() + err3;

                if error <= abserr {
                    abserr = error;
                    result = res;
                }
            }
        }

        // Shift the table.
        {
            let limexp = 50 - 1;

            if n_final == limexp {
                n_final = 2 * (limexp / 2);
            }
        }

        if n_orig % 2 == 1 {
            for i in 0..=newelm {
                let v = rd(epstab, i * 2 + 3)?;
                wr(epstab, 1 + i * 2, v)?;
            }
        } else {
            for i in 0..=newelm {
                let v = rd(epstab, i * 2 + 2)?;
                wr(epstab, i * 2, v)?;
            }
        }

        if n_orig != n_final {
            for i in 0..=n_final {
                let v = rd(epstab, n_orig - n_final + i)?;
                wr(epstab, i, v)?;
            }
        }

        self.n = n_final + 1;

        if nres_orig < 3 {
            wr(res3la, nres_orig, result)?;
            abserr = DBL_MAX;
        } else {
            // Compute error estimate.
            let [r0, r1, r2] = *res3la;
            abserr = (result - r2).abs() + (result - r1).abs() + (result - r0).abs();

            *res3la = [r1, r2, result];
        }

        // Upstream moved the nres update here from the top of QUADPACK's
        // qelg so that res3la is never read before it is set.
        self.nres = nres_orig + 1;

        abserr = abserr.max(5.0 * DBL_EPSILON * result.abs());

        Some((result, abserr))
    }
}

/// The outcome of [`qags_with_status`]: GSL's `(status, result, abserr)`
/// triple, which reports an estimate **even when the status is an error**.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QagsOutcome {
    /// The estimate GSL writes to `*result` and `*abserr`, and the workspace
    /// size at exit. Zero for the argument errors, where GSL writes zeros.
    pub integral: Integral,
    /// `Ok(())` for `GSL_SUCCESS`, otherwise the error GSL returned.
    pub status: Result<()>,
}

/// Adaptive quadrature with epsilon-algorithm extrapolation — GSL's
/// `gsl_integration_qags` (21-point Gauss-Kronrod rule), QUADPACK `dqagse`.
///
/// The routine to reach for when `f` may have an integrable singularity at an
/// endpoint, a logarithmic or algebraic kink, or a sharp peak: everywhere
/// [`super::qag`] would exhaust `limit`. On a smooth integrand it performs the
/// same bisections as `qag` and stops at the same point.
///
/// # Errors
///
/// As upstream's `return_error` switch (`qags.c:530-569`):
///
/// - [`PetirError::Domain`] if `a` or `b` is not finite (petir's check).
/// - [`PetirError::Invalid`] if `limit == 0`.
/// - [`PetirError::Tolerance`] if both tolerances are effectively zero
///   (`GSL_EBADTOL`).
/// - [`PetirError::MaxIterations`] — `limit` sub-intervals were not enough.
/// - [`PetirError::RoundOff`] — round-off prevents the tolerance, either in
///   the subdivision or in the extrapolation table.
/// - [`PetirError::BadIntegrand`] — `GSL_ESING`, bad integrand behaviour.
/// - [`PetirError::Diverged`] — the integral is divergent or converges too
///   slowly.
/// - [`PetirError::Failed`] — `GSL_EFAILED`, "could not integrate function".
///
/// Use [`qags_with_status`] to keep GSL's estimate when it fails.
///
/// # Example
///
/// ```
/// use petir::integration::qags;
/// // integral of 1/sqrt(x) over [0, 1] is 2 -- qag cannot do this one.
/// let r = qags(|x: f64| 1.0 / x.sqrt(), 0.0, 1.0, 0.0, 1e-10, 100).unwrap();
/// assert!((r.value - 2.0).abs() < 1e-10);
/// ```
pub fn qags<F>(f: F, a: f64, b: f64, epsabs: f64, epsrel: f64, limit: usize) -> Result<Integral>
where
    F: Fn(f64) -> f64,
{
    let o = qags_with_status(f, a, b, epsabs, epsrel, limit);
    o.status.map(|()| o.integral)
}

/// [`qags`], returning GSL's estimate alongside its status instead of
/// discarding it on failure — what `scipy.integrate.quad` does (it warns and
/// returns the value), and what a nested integral needs when an inner level
/// fails to certify its tolerance at one point.
pub fn qags_with_status<F>(
    f: F,
    a: f64,
    b: f64,
    epsabs: f64,
    epsrel: f64,
    limit: usize,
) -> QagsOutcome
where
    F: Fn(f64) -> f64,
{
    let zero = Integral {
        value: 0.0,
        abserr: 0.0,
        subintervals: 0,
    };
    if !a.is_finite() || !b.is_finite() {
        return QagsOutcome {
            integral: zero,
            status: Err(PetirError::Domain),
        };
    }
    if limit == 0 {
        return QagsOutcome {
            integral: zero,
            status: Err(PetirError::Invalid),
        };
    }
    match qags_core(QkRule::Qk21, &f, a, b, epsabs, epsrel, limit) {
        Some(o) => o,
        // An internal index out of range: unreachable for a workspace of
        // `limit` entries, reported rather than panicking.
        None => QagsOutcome {
            integral: zero,
            status: Err(PetirError::Invalid),
        },
    }
}

/// `qags.c`, static `qags` (lines 194-570). `None` only on an internal
/// indexing failure.
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn qags_core<F>(
    q: QkRule,
    f: &F,
    a: f64,
    b: f64,
    epsabs: f64,
    epsrel: f64,
    limit: usize,
) -> Option<QagsOutcome>
where
    F: Fn(f64) -> f64,
{
    let mut ertest = 0.0;
    let mut error_over_large_intervals = 0.0;
    let mut correc = 0.0;
    let mut ktmin: usize = 0;
    let (mut roundoff_type1, mut roundoff_type2, mut roundoff_type3) = (0u32, 0u32, 0u32);
    let mut error_type: u32 = 0;
    let mut error_type2 = false;

    let mut extrapolate = false;
    let mut disallow_extrapolation = false;

    // Initialise results.
    let mut workspace = Workspace::new(limit, a, b)?;

    let out = |value: f64, abserr: f64, w: &Workspace, status: Result<()>| QagsOutcome {
        integral: Integral {
            value,
            abserr,
            subintervals: w.size,
        },
        status,
    };

    // Test on accuracy.
    if epsabs <= 0.0 && (epsrel < 50.0 * DBL_EPSILON || epsrel < 0.5e-28) {
        return Some(QagsOutcome {
            integral: Integral {
                value: 0.0,
                abserr: 0.0,
                subintervals: 0,
            },
            status: Err(PetirError::Tolerance),
        });
    }

    // Perform the first integration.
    let first = kronrod(q, f, a, b);
    let (result0, abserr0, resabs0, resasc0) =
        (first.result, first.abserr, first.resabs, first.resasc);

    workspace.set_initial_result(result0, abserr0)?;

    let mut tolerance = epsabs.max(epsrel * result0.abs());

    if abserr0 <= 100.0 * DBL_EPSILON * resabs0 && abserr0 > tolerance {
        return Some(out(result0, abserr0, &workspace, Err(PetirError::RoundOff)));
    } else if (abserr0 <= tolerance && abserr0 != resasc0) || abserr0 == 0.0 {
        return Some(out(result0, abserr0, &workspace, Ok(())));
    } else if limit == 1 {
        return Some(out(
            result0,
            abserr0,
            &workspace,
            Err(PetirError::MaxIterations),
        ));
    }

    // Initialization.
    let mut table = ExtrapolationTable::new();
    table.append(result0)?;

    let mut area = result0;
    let mut errsum = abserr0;

    let mut res_ext = result0;
    let mut err_ext = DBL_MAX;

    let positive_integrand = test_positivity(result0, resabs0);

    let mut iteration: usize = 1;

    // `goto compute_result` in upstream: sum the workspace, errsum.
    let mut compute_result = false;

    loop {
        // Bisect the subinterval with the largest error estimate.
        let (a_i, b_i, r_i, e_i) = workspace.retrieve()?;

        let current_level = rdu(&workspace.level, workspace.i)? + 1;

        let a1 = a_i;
        let b1 = 0.5 * (a_i + b_i);
        let a2 = b1;
        let b2 = b_i;

        iteration += 1;

        let left = kronrod(q, f, a1, b1);
        let right = kronrod(q, f, a2, b2);
        let (area1, error1, resasc1) = (left.result, left.abserr, left.resasc);
        let (area2, error2, resasc2) = (right.result, right.abserr, right.resasc);

        let area12 = area1 + area2;
        let error12 = error1 + error2;
        let last_e_i = e_i;

        // Improve previous approximations to the integral and test for
        // accuracy. Written, as upstream, the same way as QUADPACK so the
        // rounding is the same.
        errsum = errsum + error12 - e_i;
        area = area + area12 - r_i;

        tolerance = epsabs.max(epsrel * area.abs());

        if resasc1 != error1 && resasc2 != error2 {
            let delta = r_i - area12;

            if delta.abs() <= 1.0e-5 * area12.abs() && error12 >= 0.99 * e_i {
                if !extrapolate {
                    roundoff_type1 += 1;
                } else {
                    roundoff_type2 += 1;
                }
            }
            if iteration > 10 && error12 > e_i {
                roundoff_type3 += 1;
            }
        }

        // Test for roundoff and eventually set error flag.
        if roundoff_type1 + roundoff_type2 >= 10 || roundoff_type3 >= 20 {
            error_type = 2; // round off error
        }

        if roundoff_type2 >= 5 {
            error_type2 = true;
        }

        // Set error flag in the case of bad integrand behaviour at a point
        // of the integration range.
        if subinterval_too_small(a1, a2, b2) {
            error_type = 4;
        }

        // Append the newly-created intervals to the list.
        workspace.update(a1, b1, area1, error1, a2, b2, area2, error2)?;

        if errsum <= tolerance {
            compute_result = true;
            break;
        }

        if error_type != 0 {
            break;
        }

        if iteration >= limit - 1 {
            error_type = 1;
            break;
        }

        if iteration == 2 {
            // Set up variables on first iteration.
            error_over_large_intervals = errsum;
            ertest = tolerance;
            table.append(area)?;
            if iteration < limit {
                continue;
            }
            break;
        }

        if disallow_extrapolation {
            if iteration < limit {
                continue;
            }
            break;
        }

        error_over_large_intervals += -last_e_i;

        if current_level < workspace.maximum_level {
            error_over_large_intervals += error12;
        }

        let mut skip = false;
        if !extrapolate {
            // Test whether the interval to be bisected next is the smallest
            // interval.
            if workspace.large_interval()? {
                skip = true;
            } else {
                extrapolate = true;
                workspace.nrmax = 1;
            }
        }

        if !skip
            && !error_type2
            && error_over_large_intervals > ertest
            && workspace.increase_nrmax()?
        {
            skip = true;
        }

        if skip {
            // `continue` in a do-while re-tests the condition.
            if iteration < limit {
                continue;
            }
            break;
        }

        // Perform extrapolation.
        table.append(area)?;

        let (reseps, abseps) = table.qelg()?;

        ktmin += 1;

        if ktmin > 5 && err_ext < 0.001 * errsum {
            error_type = 5;
        }

        if abseps < err_ext {
            ktmin = 0;
            err_ext = abseps;
            res_ext = reseps;
            correc = error_over_large_intervals;
            ertest = epsabs.max(epsrel * reseps.abs());
            if err_ext <= ertest {
                break;
            }
        }

        // Prepare bisection of the smallest interval.
        if table.n == 1 {
            disallow_extrapolation = true;
        }

        if error_type == 5 {
            break;
        }

        // Work on interval with largest error.
        workspace.reset_nrmax()?;
        extrapolate = false;
        error_over_large_intervals = errsum;

        if iteration >= limit {
            break;
        }
    }

    let mut result = res_ext;
    let mut abserr = err_ext;
    // `goto return_error` in upstream: skip the divergence test.
    let mut return_error = false;

    if !compute_result {
        if err_ext == DBL_MAX {
            compute_result = true;
        } else {
            if error_type != 0 || error_type2 {
                // Upstream adds `correc` after `*abserr = err_ext`, so it only
                // moves the comparison below, never the reported error.
                if error_type2 {
                    err_ext += correc;
                }

                if error_type == 0 {
                    error_type = 3;
                }

                if res_ext != 0.0 && area != 0.0 {
                    if err_ext / res_ext.abs() > errsum / area.abs() {
                        compute_result = true;
                    }
                } else if err_ext > errsum {
                    compute_result = true;
                } else if area == 0.0 {
                    return_error = true;
                }
            }

            if !compute_result && !return_error {
                // Test on divergence.
                let max_area = res_ext.abs().max(area.abs());

                if positive_integrand || max_area >= 0.01 * resabs0 {
                    let ratio = res_ext / area;

                    if ratio < 0.01 || ratio > 100.0 || errsum > area.abs() {
                        error_type = 6;
                    }
                }
            }
        }
    }

    if compute_result {
        result = workspace.sum_results();
        abserr = errsum;
    }

    if error_type > 2 {
        error_type -= 1;
    }

    let status = match error_type {
        0 => Ok(()),
        1 => Err(PetirError::MaxIterations),
        2 => Err(PetirError::RoundOff),
        3 => Err(PetirError::BadIntegrand),
        4 => Err(PetirError::RoundOff),
        5 => Err(PetirError::Diverged),
        _ => Err(PetirError::Failed),
    };

    Some(out(result, abserr, &workspace, status))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `subinterval_too_small` must flag an interval that has collapsed to a
    /// point and leave an ordinary one alone.
    #[test]
    fn subinterval_too_small_flags_only_degenerate_intervals() {
        assert!(subinterval_too_small(1.0, 1.0, 1.0));
        assert!(!subinterval_too_small(0.0, 0.5, 1.0));
    }

    /// `qelg` on a table of fewer than three entries returns the last entry
    /// with an unbounded error (upstream's `n < 2` branch).
    #[test]
    fn qelg_on_a_short_table_returns_the_last_entry() {
        let mut t = ExtrapolationTable::new();
        t.append(1.5).unwrap();
        t.append(1.75).unwrap();
        let (r, e) = t.qelg().unwrap();
        assert_eq!(r, 1.75);
        assert_eq!(e, DBL_MAX);
    }
}
