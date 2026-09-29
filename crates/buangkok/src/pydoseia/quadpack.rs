// SPDX-License-Identifier: GPL-3.0-only
//! The adaptive quadrature pyDOSEIA's plume-shine integral runs on:
//! QUADPACK's `dqagse` (21-point Gauss-Kronrod, bisection, Wynn epsilon
//! extrapolation), and SciPy's `tplquad` nesting of it.
//!
//! # Provenance
//!
//! pyDOSEIA (`dosefunc.py`, `plumeshine_dose`) integrates the finite-cloud
//! kernel with `scipy.integrate.tplquad`. In SciPy 1.18.1 that is `nquad`
//! (`scipy/integrate/_quadpack_py.py`, class `_NQuad`), which calls
//! `quad` -> `_quadpack._qagse` at each of the three levels. `_qagse` is
//! SciPy's C translation of QUADPACK, `scipy/integrate/__quadpack.c`
//! (functions `dqagse`, `dqk21`, `dqelg`, `dqpsrt`), Copyright (C) 2024 SciPy
//! developers, BSD 3-clause; itself a translation of the public-domain Fortran
//! QUADPACK by R. Piessens, E. de Doncker-Kapenga, C. Ueberhuber and
//! D. Kahaner (1983), <https://www.netlib.org/quadpack/> (`dqagse.f`,
//! `dqk21.f`, `dqelg.f`, `dqpsrt.f`). Both were read for this port
//! (SciPy tag `v1.18.1`, netlib files fetched 2026-09-28). The port follows
//! the **C** text, including its 0-based indexing, because that is what
//! pyDOSEIA executes. BSD 3-clause is compatible with this crate's GPL-3.0;
//! the notice is reproduced in `crates/buangkok/NOTICE`.
//!
//! # Role: the regression reference, not the default
//!
//! ~~Why not `petir::integration::qag`~~ **CHANGED 2026-09-28** (maintainer:
//! "include petir as a dependency to buangkok. quadpack should be used as
//! regression test, but petir is the main one"). Plume shine now integrates
//! by default with `petir::integration::qags`, petir's port of GSL
//! `gsl_integration_qags` (itself QUADPACK `dqagse`), ported into petir for
//! this purpose. This module stays because it is what pyDOSEIA executes: the
//! code-to-code fixture selects it
//! ([`PlumeShineIntegrator::ScipyQuadpackReference`](crate::pydoseia::plume_shine::PlumeShineIntegrator))
//! so the comparison against upstream stays bit-exact, and
//! `tests/plume_shine_petir_vs_quadpack.rs` measures petir against it.
//!
//! What was true on 2026-09-28 before the change, and still is: petir's
//! plain `qag` is `dqage`, without the epsilon extrapolation `dqagse` adds, and
//! differs from SciPy's routine on singular integrands (the `quadpack`-group
//! mutation test). Petir's `qags` does not have that gap.

/// Machine epsilon, `np.finfo(np.float64).eps` (SciPy's `epmach`).
const EPMACH: f64 = f64::EPSILON;
/// Smallest normal positive double (SciPy's `uflow`).
const UFLOW: f64 = f64::MIN_POSITIVE;
/// Largest finite double (SciPy's `oflow`).
const OFLOW: f64 = f64::MAX;

/// Outcome of one [`qagse`] call.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuadResult {
    /// The integral estimate (what `scipy.integrate.quad` returns first).
    pub value: f64,
    /// The error estimate.
    pub abserr: f64,
    /// QUADPACK's `ier` (0 = requested accuracy reached). SciPy only warns on
    /// a non-zero value and still returns `value`, so callers here do the same.
    pub ier: i32,
    /// Integrand evaluations.
    pub neval: usize,
}

const WG: [f64; 5] = [
    0.066671344308688137593568809893332,
    0.149451349150580593145776339657697,
    0.219086362515982043995534934228163,
    0.269266719309996355091226921569469,
    0.295524224714752870173892994651338,
];
const XGK: [f64; 11] = [
    0.995657163025808080735527280689003,
    0.973906528517171720077964012084452,
    0.930157491355708226001207180059508,
    0.865063366688984510732096688423493,
    0.780817726586416897063717578345042,
    0.679409568299024406234327365114874,
    0.562757134668604683339000099272694,
    0.433395394129247190799265943165784,
    0.294392862701460198131126603103866,
    0.148874338981631210884826001129720,
    0.000000000000000000000000000000000,
];
const WGK: [f64; 11] = [
    0.011694638867371874278064396062192,
    0.032558162307964727478818972459390,
    0.054755896574351996031381300244580,
    0.075039674810919952767043140916190,
    0.093125454583697605535065465083366,
    0.109387158802297641899210590325805,
    0.123491976262065851077958109831074,
    0.134709217311473325928054001771707,
    0.142775938577060080797094273138717,
    0.147739104901338491374841515972068,
    0.149445554002916905664936468389821,
];

/// `dqk21`: one 21-point Gauss-Kronrod rule on `[a, b]`.
/// Returns `(result, abserr, resabs, resasc)`.
fn dqk21<F: FnMut(f64) -> f64>(f: &mut F, a: f64, b: f64) -> (f64, f64, f64, f64) {
    let mut fv1 = [0.0; 10];
    let mut fv2 = [0.0; 10];
    let centr = 0.5 * (a + b);
    let hlgth = 0.5 * (b - a);
    let dhlgth = hlgth.abs();
    let fc = f(centr);
    let mut resg = 0.0;
    let mut resk = WGK[10] * fc;
    let mut resabs = resk.abs();
    for j in 0..5 {
        let jtw = 2 * j + 1;
        let absc = hlgth * XGK[jtw];
        let fval1 = f(centr - absc);
        let fval2 = f(centr + absc);
        fv1[jtw] = fval1;
        fv2[jtw] = fval2;
        let fsum = fval1 + fval2;
        resg = resg + WG[j] * fsum;
        resk = resk + WGK[jtw] * fsum;
        resabs = resabs + WGK[jtw] * (fval1.abs() + fval2.abs());
    }
    for j in 0..5 {
        let jtwm1 = 2 * j;
        let absc = hlgth * XGK[jtwm1];
        let fval1 = f(centr - absc);
        let fval2 = f(centr + absc);
        fv1[jtwm1] = fval1;
        fv2[jtwm1] = fval2;
        let fsum = fval1 + fval2;
        resk = resk + WGK[jtwm1] * fsum;
        resabs = resabs + WGK[jtwm1] * (fval1.abs() + fval2.abs());
    }
    let reskh = resk * 0.5;
    let mut resasc = WGK[10] * (fc - reskh).abs();
    for j in 0..10 {
        resasc = resasc + WGK[j] * ((fv1[j] - reskh).abs() + (fv2[j] - reskh).abs());
    }
    let result = resk * hlgth;
    resabs = resabs * dhlgth;
    resasc = resasc * dhlgth;
    let mut abserr = ((resk - resg) * hlgth).abs();
    if resasc != 0.0 && abserr != 0.0 {
        abserr = resasc * f64::min(1.0, (200.0 * abserr / resasc).powf(1.5));
    }
    if resabs > UFLOW / (50.0 * EPMACH) {
        abserr = f64::max(EPMACH * 50.0 * resabs, abserr);
    }
    (result, abserr, resabs, resasc)
}

/// `dqelg`: Wynn's epsilon algorithm on the table `epstab` (0-based `n` is the
/// index of the newest element; SciPy's C convention).
fn dqelg(
    n: &mut usize,
    epstab: &mut [f64; 52],
    res3la: &mut [f64; 3],
    nres: &mut usize,
) -> (f64, f64) {
    let starting_n = *n;
    *nres += 1;
    let mut abserr = OFLOW;
    let mut result = epstab[*n];
    if *n < 2 {
        abserr = f64::max(abserr, 5.0 * EPMACH * result.abs());
        return (result, abserr);
    }
    let limexp = 49;
    epstab[*n + 2] = epstab[*n];
    let newelm = *n / 2;
    epstab[*n] = OFLOW;
    let mut k1 = *n;
    for i in 0..newelm {
        let k2 = k1 - 1;
        let k3 = k1 - 2;
        let mut res = epstab[k1 + 2];
        let e0 = epstab[k3];
        let e1 = epstab[k2];
        let e2 = res;
        let e1abs = e1.abs();
        let delta2 = e2 - e1;
        let err2 = delta2.abs();
        let tol2 = f64::max(e2.abs(), e1abs) * EPMACH;
        let delta3 = e1 - e0;
        let err3 = delta3.abs();
        let tol3 = f64::max(e1abs, e0.abs()) * EPMACH;
        if !((err2 > tol2) || (err3 > tol3)) {
            result = res;
            abserr = err2 + err3;
            abserr = f64::max(abserr, 5.0 * EPMACH * result.abs());
            return (result, abserr);
        }
        let e3 = epstab[k1];
        epstab[k1] = e1;
        let delta1 = e1 - e3;
        let err1 = delta1.abs();
        let tol1 = f64::max(e1abs, e3.abs()) * EPMACH;
        if (err1 <= tol1) || (err2 <= tol2) || (err3 <= tol3) {
            *n = i + i;
            break;
        }
        let ss = 1.0 / delta1 + 1.0 / delta2 - 1.0 / delta3;
        let epsinf = (ss * e1).abs();
        if !(epsinf > 1e-4) {
            *n = i + i;
            break;
        }
        res = e1 + 1.0 / ss;
        epstab[k1] = res;
        k1 -= 2;
        let error = err2 + (res - e2).abs() + err3;
        if !(error > abserr) {
            abserr = error;
            result = res;
        }
    }
    if *n == limexp {
        *n = 2 * (limexp / 2);
    }
    let n2_rem = starting_n % 2;
    for i in 0..=newelm {
        epstab[2 * i + n2_rem] = epstab[2 * i + 2 + n2_rem];
    }
    if *n != starting_n {
        let mut indx = starting_n - *n;
        for i in 0..=*n {
            epstab[i] = epstab[indx];
            indx += 1;
        }
    }
    if *nres < 4 {
        res3la[*nres - 1] = result;
        abserr = OFLOW;
    } else {
        abserr =
            (result - res3la[2]).abs() + (result - res3la[1]).abs() + (result - res3la[0]).abs();
        res3la[0] = res3la[1];
        res3la[1] = res3la[2];
        res3la[2] = result;
    }
    abserr = f64::max(abserr, 5.0 * EPMACH * result.abs());
    (result, abserr)
}

/// `dqpsrt`: maintain the descending ordering of error estimates.
fn dqpsrt(
    limit: usize,
    last: usize,
    maxerr: &mut usize,
    ermax: &mut f64,
    elist: &[f64],
    iord: &mut [usize],
    nrmax: &mut usize,
) {
    if last <= 2 {
        iord[0] = 0;
        iord[1] = 1;
        *maxerr = iord[*nrmax];
        *ermax = elist[*maxerr];
        return;
    }
    let errmax = elist[*maxerr];
    while *nrmax > 0 && errmax > elist[iord[*nrmax - 1]] {
        iord[*nrmax] = iord[*nrmax - 1];
        *nrmax -= 1;
    }
    let jupbn = if last > limit / 2 + 2 {
        limit - last + 2
    } else {
        last - 1
    };
    let errmin = elist[last - 1];
    let jbnd = jupbn - 1;
    let ibeg = *nrmax + 1;
    let mut found: Option<usize> = None;
    if ibeg <= jbnd {
        for i in ibeg..=jbnd {
            if errmax >= elist[iord[i]] {
                found = Some(i);
                break;
            }
            iord[i - 1] = iord[i];
        }
    }
    let Some(i) = found else {
        iord[jbnd] = *maxerr;
        iord[jupbn] = last - 1;
        *maxerr = iord[*nrmax];
        *ermax = elist[*maxerr];
        return;
    };
    // LINE60
    iord[i - 1] = *maxerr;
    let mut k = jbnd as isize;
    for _j in i..=jbnd {
        if errmin < elist[iord[k as usize]] {
            break;
        }
        iord[(k + 1) as usize] = iord[k as usize];
        k -= 1;
    }
    if errmin < elist[iord[k as usize]] {
        iord[(k + 1) as usize] = last - 1;
    } else {
        iord[i] = last - 1;
    }
    *maxerr = iord[*nrmax];
    *ermax = elist[*maxerr];
}

/// QUADPACK `dqagse` as SciPy's `scipy.integrate.quad` calls it for finite
/// limits (`limit` = 50 there). Returns the estimate even when `ier != 0`, as
/// SciPy does.
#[allow(clippy::too_many_lines)]
pub fn qagse<F: FnMut(f64) -> f64>(
    mut f: F,
    a: f64,
    b: f64,
    epsabs: f64,
    epsrel: f64,
    limit: usize,
) -> QuadResult {
    let limit = limit.max(1);
    let mut alist = vec![0.0; limit];
    let mut blist = vec![0.0; limit];
    let mut rlist = vec![0.0; limit];
    let mut elist = vec![0.0; limit];
    let mut iord = vec![0usize; limit];
    let mut res3la = [0.0; 3];
    let mut rlist2 = [0.0; 52];

    let mut small = 0.0;
    let mut ier: i32 = 0;
    let mut last: usize;
    alist[0] = a;
    blist[0] = b;
    let mut ierror = 0;
    let mut erlarg = 0.0;
    let mut ertest = 0.0;
    let mut correc = 0.0;
    if epsabs <= 0.0 && epsrel < f64::max(50.0 * EPMACH, 0.5e-28) {
        return QuadResult {
            value: 0.0,
            abserr: 0.0,
            ier: 6,
            neval: 0,
        };
    }
    let (mut result, mut abserr, defabs, resabs0) = dqk21(&mut f, a, b);
    let dres = result.abs();
    let mut errbnd = f64::max(epsabs, epsrel * dres);
    last = 1;
    rlist[0] = result;
    elist[0] = abserr;
    iord[0] = 0;
    if abserr <= 100.0 * EPMACH * defabs && abserr > errbnd {
        ier = 2;
    }
    if limit == 1 {
        ier = 1;
    }
    if ier != 0 || (abserr <= errbnd && abserr != resabs0) || abserr == 0.0 {
        return QuadResult {
            value: result,
            abserr,
            ier,
            neval: 42 * last - 21,
        };
    }
    rlist2[0] = result;
    let mut errmax = abserr;
    let mut maxerr: usize = 0;
    let mut area = result;
    let mut errsum = abserr;
    abserr = OFLOW;
    let mut nrmax: usize = 0;
    let mut nres: usize = 0;
    let mut numrl2: usize = 1;
    let mut ktmin = 0;
    let mut extrap = false;
    let mut noext = false;
    let mut iroff1 = 0;
    let mut iroff2 = 0;
    let mut iroff3 = 0;
    let ksgn: i32 = if dres >= (1.0 - 50.0 * EPMACH) * defabs {
        1
    } else {
        -1
    };

    // Where control goes after the loop.
    enum Exit {
        Line100,
        Line115,
    }
    let mut exit = Exit::Line100;
    let mut l_final: usize = 1;
    let mut l = 1;
    while l < limit {
        l_final = l;
        last = l + 1;
        let a1 = alist[maxerr];
        let b1 = 0.5 * (alist[maxerr] + blist[maxerr]);
        let a2 = b1;
        let b2 = blist[maxerr];
        let erlast = errmax;
        let (area1, error1, _r1, defab1) = dqk21(&mut f, a1, b1);
        let (area2, error2, _r2, defab2) = dqk21(&mut f, a2, b2);
        let area12 = area1 + area2;
        let error12 = error1 + error2;
        errsum = errsum + error12 - errmax;
        area = area + area12 - rlist[maxerr];
        if defab1 != error1 && defab2 != error2 {
            if !(((rlist[maxerr] - area12).abs() > 1.0e-5 * area12.abs())
                || (error12 < 0.99 * errmax))
            {
                if extrap {
                    iroff2 += 1;
                } else {
                    iroff1 += 1;
                }
            }
            if l > 9 && error12 > errmax {
                iroff3 += 1;
            }
        }
        rlist[maxerr] = area1;
        rlist[l] = area2;
        errbnd = f64::max(epsabs, epsrel * area.abs());
        if (iroff1 + iroff2) >= 10 || iroff3 >= 20 {
            ier = 2;
        }
        if iroff2 >= 5 {
            ierror = 3;
        }
        if last == limit {
            ier = 1;
        }
        if f64::max(a1.abs(), b2.abs()) <= (1.0 + 100.0 * EPMACH) * (a2.abs() + 1000.0 * UFLOW) {
            ier = 4;
        }
        if !(error2 > error1) {
            alist[l] = a2;
            blist[maxerr] = b1;
            blist[l] = b2;
            elist[maxerr] = error1;
            elist[l] = error2;
        } else {
            alist[maxerr] = a2;
            alist[l] = a1;
            blist[l] = b1;
            rlist[maxerr] = area2;
            rlist[l] = area1;
            elist[maxerr] = error2;
            elist[l] = error1;
        }
        dqpsrt(
            limit,
            last,
            &mut maxerr,
            &mut errmax,
            &elist,
            &mut iord,
            &mut nrmax,
        );
        if errsum <= errbnd {
            exit = Exit::Line115;
            break;
        }
        if ier != 0 {
            break;
        }
        if l == 1 {
            // LINE80
            small = (b - a).abs() * 0.375;
            erlarg = errsum;
            ertest = errbnd;
            rlist2[1] = area;
            l += 1;
            continue;
        }
        if noext {
            l += 1;
            continue;
        }
        erlarg = erlarg - erlast;
        if (b1 - a1).abs() > small {
            erlarg = erlarg + error12;
        }
        if !extrap {
            if (blist[maxerr] - alist[maxerr]).abs() > small {
                l += 1;
                continue;
            }
            extrap = true;
            nrmax = 1;
        }
        let mut goto_line90 = false;
        if !(ierror == 3 || erlarg <= ertest) {
            let jupbnd = if last > 2 + (limit / 2) {
                limit + 3 - last
            } else {
                last
            };
            let mut k = nrmax;
            while k < jupbnd {
                maxerr = iord[nrmax];
                errmax = elist[maxerr];
                if (blist[maxerr] - alist[maxerr]).abs() > small {
                    goto_line90 = true;
                    break;
                }
                nrmax += 1;
                k += 1;
            }
        }
        if goto_line90 {
            l += 1;
            continue;
        }
        // LINE60
        numrl2 += 1;
        rlist2[numrl2] = area;
        let (reseps, abseps) = dqelg(&mut numrl2, &mut rlist2, &mut res3la, &mut nres);
        ktmin += 1;
        if ktmin > 5 && abserr < 1.0e-3 * errsum {
            ier = 5;
        }
        if !(abseps >= abserr) {
            ktmin = 0;
            abserr = abseps;
            result = reseps;
            correc = erlarg;
            ertest = f64::max(epsabs, epsrel * reseps.abs());
            if abserr <= ertest {
                break;
            }
        }
        // LINE70
        if numrl2 == 0 {
            noext = true;
        }
        if ier == 5 {
            break;
        }
        maxerr = iord[0];
        errmax = elist[maxerr];
        nrmax = 0;
        extrap = false;
        small = small * 0.5;
        erlarg = errsum;
        l += 1;
    }

    let mut goto_115 = matches!(exit, Exit::Line115);
    let mut goto_130 = false;
    if !goto_115 {
        // LINE100
        if abserr == OFLOW {
            goto_115 = true;
        } else if ier + ierror == 0 {
            // -> LINE110
        } else {
            if ierror == 3 {
                abserr = abserr + correc;
            }
            if ier == 0 {
                ier = 3;
            }
            if result != 0.0 && area != 0.0 {
                // LINE105
                if abserr / result.abs() > errsum / area.abs() {
                    goto_115 = true;
                }
            } else if abserr > errsum {
                goto_115 = true;
            } else if area == 0.0 {
                goto_130 = true;
            }
        }
        if !goto_115 && !goto_130 {
            // LINE110
            if !(ksgn == -1 && f64::max(result.abs(), area.abs()) <= defabs * 0.01)
                && ((0.01 > (result / area)) || ((result / area) > 100.0) || (errsum > area.abs()))
            {
                ier = 6;
            }
        }
    }
    if goto_115 {
        result = 0.0;
        for k in 0..=l_final {
            result = result + rlist[k];
        }
        abserr = errsum;
    }
    // LINE130
    if ier > 2 {
        ier -= 1;
    }
    QuadResult {
        value: result,
        abserr,
        ier,
        neval: 42 * last - 21,
    }
}

/// SciPy's default `limit` for `quad` (and so for every level of `tplquad`).
pub const SCIPY_QUAD_LIMIT: usize = 50;

/// `scipy.integrate.tplquad(func, a, b, gfun, hfun, qfun, rfun, epsabs=,
/// epsrel=)` with constant limits, as `nquad` evaluates it: the **outer**
/// integral over `z` in `[z_lo, z_hi]`, the middle over `y` in `[y_lo, y_hi]`,
/// the inner over `x` in `[x_lo, x_hi]`; every level is [`qagse`] with the same
/// tolerances and [`SCIPY_QUAD_LIMIT`].
///
/// `f(x, y, z)` receives the innermost variable first, which is how SciPy
/// calls `func` (pyDOSEIA's integrand is written `lambda x, y, z`). The
/// limits array is in pyDOSEIA's order `[z_lo, z_hi, y_lo, y_hi, x_lo, x_hi]`.
pub fn tplquad<F: FnMut(f64, f64, f64) -> f64>(
    mut f: F,
    limits: [f64; 6],
    epsabs: f64,
    epsrel: f64,
) -> f64 {
    let [z_lo, z_hi, y_lo, y_hi, x_lo, x_hi] = limits;
    qagse(
        |z| {
            qagse(
                |y| qagse(|x| f(x, y, z), x_lo, x_hi, epsabs, epsrel, SCIPY_QUAD_LIMIT).value,
                y_lo,
                y_hi,
                epsabs,
                epsrel,
                SCIPY_QUAD_LIMIT,
            )
            .value
        },
        z_lo,
        z_hi,
        epsabs,
        epsrel,
        SCIPY_QUAD_LIMIT,
    )
    .value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polynomial_is_exact() {
        let r = qagse(|x| 3.0 * x * x, 0.0, 2.0, 1.49e-8, 1.49e-8, 50);
        assert!((r.value - 8.0).abs() < 1e-13, "{r:?}");
        assert_eq!(r.ier, 0);
    }

    #[test]
    fn endpoint_singularity_uses_extrapolation() {
        // int_0^1 x^-1/2 dx = 2; needs the epsilon algorithm to converge.
        let r = qagse(|x: f64| 1.0 / x.sqrt(), 0.0, 1.0, 1.49e-8, 1.49e-8, 50);
        assert!((r.value - 2.0).abs() < 1e-10, "{r:?}");
    }

    #[test]
    fn triple_integral_of_a_product() {
        let v = tplquad(
            |x, y, z| x * y * z,
            [0.0, 1.0, 0.0, 1.0, 0.0, 1.0],
            1.49e-8,
            1.49e-8,
        );
        assert!((v - 0.125).abs() < 1e-14, "{v}");
    }
}
