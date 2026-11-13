//! QAGP: adaptive integration with known break points, ported from GSL 2.8
//! `integration/qagp.c` (lines 28-510, with `ptsort.c` 20-62 and
//! `append.c` 20-36), commit `cf180cd7`, read 2026-11-13. Copyright (C)
//! 1996-2000, 2007 Brian Gough (GPL-3.0-or-later); GSL's QAGP is QUADPACK's
//! `dqagpe` (public domain). gh:#1340. Research, education and V&V only.
//!
//! The integration range and the interior break points (singularities or
//! discontinuities) are given as one ascending list `pts`; each sub-range is
//! first integrated with the 21-point rule, then the interval with the
//! largest error is bisected with Wynn epsilon extrapolation, exactly as
//! [`super::qags`] (whose engine pieces this module re-uses).
//!
//! # Differences from upstream, all deliberate
//!
//! - The workspace is allocated inside the call with `limit` entries, so
//!   `npts > limit` is the only workspace check (upstream also compares
//!   `npts` with the caller's workspace size).
//! - Non-finite points are [`PetirError::Domain`]; fewer than two points are
//!   [`PetirError::Invalid`] (upstream's `npts - 1` would underflow).
//! - Every array access is a checked `get`.
//!
//! # Upstream quirks kept on purpose
//!
//! - The roundoff test `if (i > 10 && error12 > e_i)` (qagp.c line 297) uses
//!   `i`, the stale index of the earlier loops, which is `nint`, not the
//!   iteration count that `qags.c` and QUADPACK use. It is kept, so for more
//!   than ten sub-ranges `roundoff_type3` counts every error increase.
//! - `if (result != 0 && area != 0)` (line 441) tests the *pointer* `result`,
//!   which is never null, so only `area != 0` matters here, where `qags.c`
//!   tests `*result`.

#[allow(unused_imports)]
use crate::real::Real;
use crate::scalar::DBL_EPSILON;

/// `GSL_DBL_MAX`.
const DBL_MAX: f64 = f64::MAX;
use crate::{PetirError, Result};

use super::qags::{
    rd, rdu, subinterval_too_small, test_positivity, wr, wru, ExtrapolationTable, QagsOutcome,
    Workspace,
};
use super::{kronrod, Integral, QkRule};

/// `append.c`, `append_interval`.
fn append_interval(w: &mut Workspace, a1: f64, b1: f64, area1: f64, error1: f64) -> Option<()> {
    let i_new = w.size;
    wr(&mut w.alist, i_new, a1)?;
    wr(&mut w.blist, i_new, b1)?;
    wr(&mut w.rlist, i_new, area1)?;
    wr(&mut w.elist, i_new, error1)?;
    wru(&mut w.order, i_new, i_new)?;
    wru(&mut w.level, i_new, 0)?;
    w.size += 1;
    Some(())
}

/// `ptsort.c`, `sort_results`: order `order[]` by decreasing error (ties go
/// to the later interval, `e2 >= e1`) and point `i` at the largest.
fn sort_results(w: &mut Workspace) -> Option<()> {
    let nint = w.size;
    for i in 0..nint {
        let i1 = rdu(&w.order, i)?;
        let mut e1 = rd(&w.elist, i1)?;
        let mut i_max = i1;
        for j in (i + 1)..nint {
            let i2 = rdu(&w.order, j)?;
            let e2 = rd(&w.elist, i2)?;
            if e2 >= e1 {
                i_max = i2;
                e1 = e2;
            }
        }
        if i_max != i1 {
            let o = rdu(&w.order, i_max)?;
            wru(&mut w.order, i, o)?;
            wru(&mut w.order, i_max, i1)?;
        }
    }
    w.i = rdu(&w.order, 0)?;
    Some(())
}

/// `gsl_integration_qagp`: the integral over `[pts[0], pts[last]]` with the
/// break points `pts[1..last]`.
///
/// # Errors
///
/// As [`super::qags`], plus [`PetirError::Invalid`] for fewer than two
/// points, `pts.len() > limit`, or points not ascending.
///
/// # Example
///
/// ```
/// use petir::integration::qagp;
/// // int_0^3 |x - 1| dx, with the kink at 1 as a break point: 1/2 + 2 = 5/2.
/// let r = qagp(|x: f64| (x - 1.0).abs(), &[0.0, 1.0, 3.0], 0.0, 1e-10, 50).unwrap();
/// assert!((r.value - 2.5).abs() < 1e-10);
/// ```
pub fn qagp<F>(f: F, pts: &[f64], epsabs: f64, epsrel: f64, limit: usize) -> Result<Integral>
where
    F: Fn(f64) -> f64,
{
    let o = qagp_with_status(f, pts, epsabs, epsrel, limit);
    o.status.map(|()| o.integral)
}

/// [`qagp`], keeping GSL's estimate when the status is an error.
pub fn qagp_with_status<F>(
    f: F,
    pts: &[f64],
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
    let bad = |e: PetirError| QagsOutcome {
        integral: zero,
        status: Err(e),
    };
    if pts.len() < 2 || limit == 0 {
        return bad(PetirError::Invalid);
    }
    if !pts.iter().all(|p| p.is_finite()) {
        return bad(PetirError::Domain);
    }
    match qagp_core(&f, pts, epsabs, epsrel, limit) {
        Some(o) => o,
        None => bad(PetirError::Invalid),
    }
}

/// `qagp.c`, static `qagp` (lines 80-510) with the 21-point rule.
#[allow(clippy::too_many_lines)]
fn qagp_core<F>(
    f: &F,
    pts: &[f64],
    epsabs: f64,
    epsrel: f64,
    limit: usize,
) -> Option<QagsOutcome>
where
    F: Fn(f64) -> f64,
{
    let npts = pts.len();
    let nint = npts - 1;
    let q = QkRule::Qk21;

    let mut ertest;
    let mut error_over_large_intervals;
    let mut correc = 0.0;
    let mut ktmin: usize = 0;
    let (mut roundoff_type1, mut roundoff_type2, mut roundoff_type3) = (0u32, 0u32, 0u32);
    let mut error_type: u32 = 0;
    let mut error_type2 = false;
    let mut extrapolate = false;
    let mut disallow_extrapolation = false;

    let mut workspace = Workspace::new(limit, 0.0, 0.0)?;
    let zero_out = |status: Result<()>| QagsOutcome {
        integral: Integral {
            value: 0.0,
            abserr: 0.0,
            subintervals: 0,
        },
        status,
    };

    // Test on validity of parameters.
    if npts > limit {
        return Some(zero_out(Err(PetirError::Invalid)));
    }
    if epsabs <= 0.0 && (epsrel < 50.0 * DBL_EPSILON || epsrel < 0.5e-28) {
        return Some(zero_out(Err(PetirError::Tolerance)));
    }
    // The break points must ascend.
    for (p0, p1) in pts.iter().zip(pts.iter().skip(1)) {
        if *p1 < *p0 {
            return Some(zero_out(Err(PetirError::Invalid)));
        }
    }

    // Perform the first integration.
    let mut result0 = 0.0;
    let mut abserr0 = 0.0;
    let mut resabs0 = 0.0;

    for (i, (p0, p1)) in pts.iter().zip(pts.iter().skip(1)).enumerate() {
        let (a1, b1) = (*p0, *p1);
        let r = kronrod(q, f, a1, b1);
        let (area1, error1, resabs1, resasc1) = (r.result, r.abserr, r.resabs, r.resasc);

        result0 += area1;
        abserr0 += error1;
        resabs0 += resabs1;

        append_interval(&mut workspace, a1, b1, area1, error1)?;

        // ndin[i] aliases level[i].
        let flag = if error1 == resasc1 && error1 != 0.0 { 1 } else { 0 };
        wru(&mut workspace.level, i, flag)?;
    }

    // Compute the initial error estimate.
    let mut errsum = 0.0;
    for i in 0..nint {
        if rdu(&workspace.level, i)? != 0 {
            wr(&mut workspace.elist, i, abserr0)?;
        }
        errsum += rd(&workspace.elist, i)?;
    }
    for i in 0..nint {
        wru(&mut workspace.level, i, 0)?;
    }

    // Sort results into order of decreasing error.
    sort_results(&mut workspace)?;

    // Test on accuracy.
    let mut tolerance = epsabs.max(epsrel * result0.abs());

    let out = |value: f64, abserr: f64, w: &Workspace, status: Result<()>| QagsOutcome {
        integral: Integral {
            value,
            abserr,
            subintervals: w.size,
        },
        status,
    };

    if abserr0 <= 100.0 * DBL_EPSILON * resabs0 && abserr0 > tolerance {
        return Some(out(result0, abserr0, &workspace, Err(PetirError::RoundOff)));
    } else if abserr0 <= tolerance {
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
    let mut res_ext = result0;
    let mut err_ext = DBL_MAX;

    error_over_large_intervals = errsum;
    ertest = tolerance;

    let positive_integrand = test_positivity(result0, resabs0);

    let mut iteration: usize = nint - 1;
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

            // Upstream: `i > 10` with the stale loop index `i == nint`.
            if nint > 10 && error12 > e_i {
                roundoff_type3 += 1;
            }
        }

        if roundoff_type1 + roundoff_type2 >= 10 || roundoff_type3 >= 20 {
            error_type = 2; // round off error
        }

        if roundoff_type2 >= 5 {
            error_type2 = true;
        }

        if subinterval_too_small(a1, a2, b2) {
            error_type = 4;
        }

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
            if iteration < limit {
                continue;
            }
            break;
        }

        // Perform extrapolation.
        table.append(area)?;

        // `if (table.n < 3) goto skip_extrapolation;`
        if table.n >= 3 {
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
        }

        // skip_extrapolation:
        workspace.reset_nrmax()?;
        extrapolate = false;
        error_over_large_intervals = errsum;

        if iteration >= limit {
            break;
        }
    }

    let mut result = res_ext;
    let mut abserr = err_ext;
    let mut return_error = false;

    if !compute_result {
        if err_ext == DBL_MAX {
            compute_result = true;
        } else {
            if error_type != 0 || error_type2 {
                if error_type2 {
                    err_ext += correc;
                }
                if error_type == 0 {
                    error_type = 3;
                }
                // `result != 0` is a pointer test, always true.
                if area != 0.0 {
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

    #[test]
    fn break_points_and_refusals() {
        let r = qagp(|x: f64| (x - 1.0).abs(), &[0.0, 1.0, 3.0], 0.0, 1e-10, 50).unwrap();
        assert!((r.value - 2.5).abs() < 1e-10);
        // A log singularity at an interior break point.
        let l = qagp(|x: f64| Real::ln((x - 1.0).abs()), &[0.0, 1.0, 2.0], 0.0, 1e-8, 200);
        assert!(l.is_ok());
        assert_eq!(
            qagp_with_status(|x: f64| x, &[1.0, 0.0], 0.0, 1e-8, 10).status,
            Err(PetirError::Invalid)
        );
        assert_eq!(
            qagp_with_status(|x: f64| x, &[0.0], 0.0, 1e-8, 10).status,
            Err(PetirError::Invalid)
        );
        assert_eq!(
            qagp_with_status(|x: f64| x, &[0.0, 1.0, 2.0], 0.0, 1e-8, 2).status,
            Err(PetirError::Invalid)
        );
        assert_eq!(
            qagp_with_status(|x: f64| x, &[0.0, 1.0], 0.0, 0.0, 10).status,
            Err(PetirError::Tolerance)
        );
        assert_eq!(
            qagp_with_status(|x: f64| x, &[0.0, f64::NAN], 0.0, 1e-8, 10).status,
            Err(PetirError::Domain)
        );
    }
}
