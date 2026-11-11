// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine ptlegc`, l.7994-8115;
//   - `subroutine coul`, l.8117-8215;
//   - `mathm.f90` `legndr` (reused: `groupr::kinematics::shared::legndr`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Charged-particle elastic scattering for ACER: the Coulomb-nuclear
//! interference representations of ENDF-6 MF=6 `LAW = 5` with `LTP = 1` (nuclear
//! amplitude expansion) or `LTP = 2` (residual cross section expansion), as
//! `ptlegc` reconstructs them adaptively and `coul` evaluates them (gh:#1485).

use crate::common::phys::{AMASSN_AMU as AMASSN, AMU_G as AMU, CLIGHT_CM_S as CLIGHT, EV_ERG as EV, HBAR_ERG_S as HBAR};
use crate::groupr::kinematics::shared::legndr;
use crate::mixr::mix::sigfig;
use crate::NjoyError;
use petir::mathf::RealMath;

/// `coul` (`acefc.f90:8117-8215`): the elastic cross section `y` at cosine `x`
/// from the LIST record `c` (`c[0]` is Fortran's `c(1)`: the record's six head
/// words, then the data), for a projectile `izap`/`awp` on a target
/// `izat`/`awt` with spin `spi`.
pub fn coul(x: f64, c: &[f64], awp: f64, izap: i32, awt: f64, izat: i32, spi: f64) -> f64 {
    let cf = |i: usize| c[i - 1];
    let fm = 1.0e-12;
    let e = cf(2);
    let ltp = cf(3).round() as i32;
    let lidp = cf(4).round() as i32;
    let ai = awp * AMASSN;
    let at = awt * AMASSN;
    let zt = f64::from(izat / 1000);
    let zi = f64::from(izap / 1000);
    let i2s = (2.0 * spi).round() as i32;
    let cc1 = 2.0 * AMU * EV * fm.r_powi(2) / HBAR.r_powi(2);
    let ee = (EV / 10_000_000.0) * (CLIGHT / 10.0);
    let cc2 = ee.r_powi(4) * AMU / (2.0 * HBAR.r_powi(2) * EV);
    let eta = zt * zi * (cc2 * ai / e).sqrt();
    let wn = at * (cc1 * ai * e).sqrt() / (ai + at);
    let mut sigc = 0.0;
    if lidp == 0 {
        sigc = (eta.r_powi(2) / wn.r_powi(2)) / (1.0 - x).r_powi(2);
    }
    if lidp == 1 {
        let sgn = if i2s % 2 == 0 { 1.0 } else { -1.0 };
        sigc = ((2.0 * eta.r_powi(2) / wn.r_powi(2)) / (1.0 - x.r_powi(2)))
            * ((1.0 + x.r_powi(2)) / (1.0 - x.r_powi(2)) + sgn * (eta * ((1.0 + x) / (1.0 - x)).r_ln()).r_cos() / (2.0 * spi + 1.0));
    }
    let nt = cf(6).round() as usize;
    let np = 2 * nt;
    let p = legndr(x, np); // p[l] = Fortran p(l+1)
    // `cs = cs + k * p * dcmplx(re, im) / 2`, left to right: (k*p) scales the
    // complex number, then it is halved.
    let cadd = |cs: (f64, f64), k: f64, re: f64, im: f64| (cs.0 + k * re / 2.0, cs.1 + k * im / 2.0);
    if ltp == 1 {
        if lidp != 1 {
            let mut sigr = cf(7) / 2.0;
            for ip in 1..=np {
                sigr += f64::from(2 * ip as i32 + 1) * p[ip] * cf(ip + 7) / 2.0;
            }
            let mut cs1 = (cf(8 + np) / 2.0, cf(9 + np) / 2.0);
            for it in 1..=nt {
                let k = f64::from(2 * it as i32 + 1) * p[it];
                cs1 = cadd(cs1, k, cf(8 + np + 2 * it), cf(9 + np + 2 * it));
            }
            let th1 = eta * ((1.0 - x) / 2.0).r_ln();
            let (c1, s1) = (th1.r_cos(), th1.r_sin());
            let re = cs1.0 * c1 - cs1.1 * s1;
            let sigi = (-2.0 * eta / (1.0 - x)) * re;
            sigc + sigr + sigi
        } else {
            let mut sigr = cf(7) / 2.0;
            for it in 1..=nt {
                sigr += f64::from(4 * it as i32 + 1) * p[2 * it] * cf(it + 7) / 2.0;
            }
            let mut cs1 = (cf(8 + nt) / 2.0, cf(9 + nt) / 2.0);
            let mut cs2 = cs1;
            let mut sgn = -1.0;
            for it in 1..=nt {
                let k = f64::from(2 * it as i32 + 1) * p[it];
                cs1 = cadd(cs1, k, cf(8 + nt + 2 * it), cf(9 + nt + 2 * it));
                cs2 = cadd(cs2, sgn * k, cf(8 + nt + 2 * it), cf(9 + nt + 2 * it));
                sgn = -sgn;
            }
            let th1 = eta * ((1.0 - x) / 2.0).r_ln();
            let th2 = eta * ((1.0 + x) / 2.0).r_ln();
            let (c1, s1, c2, s2) = (th1.r_cos(), th1.r_sin(), th2.r_cos(), th2.r_sin());
            let a = (cs1.0 * (1.0 + x), cs1.1 * (1.0 + x));
            let b = (cs2.0 * (1.0 - x), cs2.1 * (1.0 - x));
            let re = (a.0 * c1 - a.1 * s1) + (b.0 * c2 - b.1 * s2);
            let sigi = (-2.0 * eta / (1.0 - x * x)) * re;
            sigc + sigr + sigi
        }
    } else if lidp != 1 {
        let mut sigr = cf(7) / 2.0;
        for ip in 1..=np {
            sigr += f64::from(2 * ip as i32 + 1) * p[ip] * cf(ip + 7) / 2.0;
        }
        sigc + sigr
    } else {
        let mut sigr = cf(7) / 2.0;
        for it in 1..=nt {
            sigr += f64::from(4 * it as i32 + 1) * p[2 * it] * cf(it + 7) / 2.0;
        }
        sigc + sigr
    }
}

/// `ptlegc` (`acefc.f90:7994-8115`): the adaptive reconstruction of the
/// angular distribution of one incident energy, thinned to a coarser
/// tolerance. Returns the `(mu, cross section)` pairs that replace the LIST's
/// data (`c(5) = 2*ii`, `c(6) = ii`).
///
/// # Errors
/// [`NjoyError::EndfParse`] for more than `maxang = 4000` angles.
pub fn ptlegc(c: &[f64], awp: f64, izai: i32, awt: f64, iza: i32, spi: f64) -> Result<Vec<(f64, f64)>, NjoyError> {
    const IMAX: usize = 20;
    const MAXANG: usize = 4000;
    let (tol1, tol2, umin) = (0.001, 0.01, 0.96);
    let hund = 0.01;
    let mut x = [0.0f64; IMAX + 1];
    let mut y = [0.0f64; IMAX + 1];
    let mut test = 0.0f64;
    let (mut xm, mut yt) = (0.0f64, 0.0f64);
    let mut aco = vec![0.0f64; MAXANG + 2];
    let mut cprob = vec![0.0f64; MAXANG + 2];
    let mut ii = 0usize;
    let mut i = 2usize;
    x[2] = -1.0;
    if iza == izai {
        x[2] = -umin;
    }
    y[2] = coul(x[2], c, awp, izai, awt, iza, spi);
    x[1] = umin;
    y[1] = coul(x[1], c, awp, izai, awt, iza, spi);
    while i > 0 {
        let mut dy = 0.0f64;
        if i > 1 && i < IMAX {
            let dm = x[i - 1] - x[i];
            xm = 0.5 * (x[i - 1] + x[i]);
            xm = sigfig(xm, 3, 0);
            if xm > x[i] && xm < x[i - 1] {
                let ym = 0.5 * (y[i - 1] + y[i]);
                yt = coul(xm, c, awp, izai, awt, iza, spi);
                test = tol1 * yt.abs();
                dy = (yt - ym).abs();
                if dm > hund {
                    dy = 2.0 * test;
                }
                if ym != 0.0 && yt / ym > 5.0 {
                    dy = 2.0 * test;
                }
                if ym != 0.0 && yt / ym < 1.0 / 5.0 {
                    dy = 2.0 * test;
                }
            }
        }
        if dy > test {
            i += 1;
            x[i] = x[i - 1];
            y[i] = y[i - 1];
            x[i - 1] = xm;
            y[i - 1] = yt;
        } else {
            ii += 1;
            if ii > MAXANG {
                return Err(NjoyError::EndfParse("ptlegc: too many coulomb angles, increase maxang".into()));
            }
            aco[ii] = x[i];
            cprob[ii] = y[i];
            i -= 1;
        }
    }
    let nn = ii - 1;

    // thin to a coarser tolerance
    let mut i = 1usize;
    let mut ii = 1usize;
    let mut idone = false;
    let mut dco = 0.0f64;
    let mut jj = 0usize;
    let mut check;
    while (i as i64) < nn as i64 - 1 && !idone {
        let mut j = i + 1;
        check = 0.0f64;
        while j < nn + 1 && check <= 0.0 && dco <= 0.5 {
            j += 1;
            jj = j - 1;
            dco = aco[j] - aco[i];
            if dco <= 0.5 {
                let mut k = i;
                while k < j - 1 && check <= 0.0 {
                    k += 1;
                    let f = (aco[j] - aco[k]) / dco;
                    let t = f * cprob[i] + (1.0 - f) * cprob[j];
                    let diff = tol2 * cprob[k].abs();
                    check = (t - cprob[k]).abs() - diff;
                }
            }
        }
        if dco > 0.5 || check > 0.0 {
            i = jj;
            ii += 1;
            aco[ii] = aco[i];
            cprob[ii] = cprob[i];
            dco = 0.0;
        } else {
            idone = true;
        }
    }
    let i = nn + 1;
    ii += 1;
    aco[ii] = aco[i];
    cprob[ii] = cprob[i];
    Ok((1..=ii).map(|k| (aco[k], cprob[k])).collect())
}
