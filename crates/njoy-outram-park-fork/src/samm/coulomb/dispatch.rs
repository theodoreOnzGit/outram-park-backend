//! Large-`eta` Bessel-function case and top-level evaluation-strategy
//! dispatcher — `bigeta`, `getps`, `coulx`, ported from
//! `samm.f90:4340-4399` (`coulx`) and `samm.f90:4852-4996`
//! (`bigeta`/`getps`).

use super::asymptotic::{asymp1, asymp2, end1, getfg, taylor, xsigll};
use crate::common::phys::{EULER, PI};

/// Result of [`bigeta`]/[`coulx`]/[`super::api::pspcou`]: penetrability,
/// shift factor, phase shift (and derivatives) at a single channel `L`.
pub struct CoulombPsp {
    pub p: f64,
    pub s: f64,
    pub dp: f64,
    pub sinphi: f64,
    pub cosphi: f64,
    pub dphi: f64,
    /// `dS/d(rho)`, finite-differenced by [`super::api::pspcou`] at
    /// `rho*1.01` when `ishift>0 && jdoder` (upstream's separate
    /// `dshiftcoul` output, `samm.f90:3948-3955`/`3964-3977`/`3990-3995` —
    /// distinct from `dp`, which is `dP/d(rho)`). `0.0` when not requested
    /// or not applicable (matching upstream, which leaves `dshiftcoul=0`
    /// at `pspcou`'s top).
    pub dshift: f64,
}

/// `F,G` at `L=0` (and, if `lll>0`, all `L=0..=llmax` via [`getfg`]) plus
/// `P,S,phi` at `L=lll`, for `eta >> rho` — ported from `bigeta`
/// (`samm.f90:4852-4967`, Abramowitz & Stegun Eqs. 14.6.7-8), using the
/// modified Bessel functions `I_0,I_1,K_0,K_1` of `z=2*sqrt(2*rho*eta)`.
pub fn bigeta(
    eta: f64,
    rho: f64,
    lll: i32,
    jdopha: bool,
    jdoder: bool,
    ishift: i32,
) -> Option<CoulombPsp> {
    let q = 2.0 * rho * eta;
    let zhalf = q.sqrt();
    let z = zhalf * 2.0;

    // samm.f90:4876-4886 -- I_0(z), A&S 9.6.12.
    let mut sum = 1.0_f64;
    let mut a = q;
    let mut converged = false;
    for k in 1..=100 {
        if sum + a == sum {
            converged = true;
            break;
        }
        sum += a;
        a = a * q / ((k + 1) as f64).powi(2);
    }
    if !converged {
        log::error!("samm bigeta: I0 sum failed to converge (eta={eta}, rho={rho})");
        return None;
    }
    let ai0 = sum;

    // samm.f90:4888-4900 -- K_0(z), A&S 9.6.13.
    let mut sum = -(zhalf.ln() + EULER) * ai0;
    let mut a = q;
    let mut b = 1.0_f64;
    let mut converged = false;
    for k in 1..=100 {
        if sum + a * b == sum {
            converged = true;
            break;
        }
        sum += a * b;
        b += 1.0 / (k + 1) as f64;
        a = a * q / ((k + 1) as f64).powi(2);
    }
    if !converged {
        log::error!("samm bigeta: K0 sum failed to converge (eta={eta}, rho={rho})");
        return None;
    }
    let ak0 = sum;

    // samm.f90:4902-4912 -- I_1(z), A&S 9.6.10.
    let mut sum = 1.0_f64;
    let mut a = q / 2.0;
    let mut converged = false;
    for k in 1..=100 {
        if sum + a == sum {
            converged = true;
            break;
        }
        sum += a;
        a = a * q / (((k + 1) * (k + 2)) as f64);
    }
    if !converged {
        log::error!("samm bigeta: I1 sum failed to converge (eta={eta}, rho={rho})");
        return None;
    }
    let ai1 = sum * zhalf;

    // samm.f90:4914-4928 -- K_1(z), A&S 9.6.11.
    let mut sum = 1.0 / z + (zhalf.ln() + EULER) * ai1 - zhalf * 0.5;
    let mut a = zhalf * q / 2.0;
    let mut b = 1.0_f64;
    let mut c = 0.25_f64;
    let mut converged = false;
    for k in 1..=100 {
        if sum - a * (b + c) == sum {
            converged = true;
            break;
        }
        sum -= a * (b + c);
        c = 0.5 / (k + 2) as f64;
        b += 1.0 / (k + 1) as f64;
        a = a * q / (((k + 1) * (k + 2)) as f64);
    }
    if !converged {
        log::error!("samm bigeta: K1 sum failed to converge (eta={eta}, rho={rho})");
        return None;
    }
    let ak1 = sum;

    // samm.f90:4930-4944 -- F,G at L=0 from A&S Eq. 14.6.8.
    let c = (PI * rho).sqrt();
    let d = (2.0 * PI * eta).sqrt();
    let mut f = vec![c * ai1];
    let mut fpr = vec![d * ai0];
    let c2 = 2.0 * c / PI;
    let d2 = 2.0 * d / PI;
    let mut g = vec![c2 * ak1];
    let mut gpr = vec![-d2 * ak0];

    let g0 = g[0];
    let g0pr = gpr[0];

    if lll > 0 {
        let (_llmax_out, f2, fpr2, g2, gpr2) = getfg(eta, rho, 0, lll, g0, g0pr);
        f = f2;
        fpr = fpr2;
        g = g2;
        gpr = gpr2;
    }

    let a = PI * eta;
    let b = (-a).exp();
    let n = lll as usize;
    if f[n] * b * b + g[n] == g[n] {
        let p = (rho * b / g[n]) / g[n] * b;
        let s = if ishift > 0 { rho * gpr[n] / g[n] } else { 0.0 };
        let sinphi = if jdopha { f[n] / g[n] * b * b } else { 0.0 };
        Some(CoulombPsp {
            p,
            s,
            dp: 0.0,
            sinphi,
            cosphi: 1.0,
            dphi: 0.0,
            dshift: 0.0,
        })
    } else {
        f[n] *= b;
        g[n] /= b;
        fpr[n] *= b;
        gpr[n] /= b;
        Some(getps(rho, lll, &f, &fpr, &g, &gpr, jdopha, jdoder, ishift))
    }
}

/// `P,S,phi` (and derivatives) at `L=lll` from already-computed `F,G,F',G'`
/// arrays — ported from `getps` (`samm.f90:4969-4996`).
pub fn getps(
    rho: f64,
    lll: i32,
    f: &[f64],
    fpr: &[f64],
    g: &[f64],
    gpr: &[f64],
    jdopha: bool,
    jdoder: bool,
    ishift: i32,
) -> CoulombPsp {
    let n = lll as usize;
    let asq = f[n].powi(2) + g[n].powi(2);
    let a = asq.sqrt();
    let p = rho / asq;
    let mut ss = 0.0_f64;
    if jdoder || ishift > 0 {
        ss = rho * (f[n] * fpr[n] + g[n] * gpr[n]) / asq;
    }
    let dp = if jdoder { (1.0 - 2.0 * ss) / asq } else { 0.0 };
    let s = if ishift > 0 { ss } else { 0.0 };
    let (sinphi, cosphi, dphi) = if jdopha {
        let sinphi = f[n] / a;
        let cosphi = g[n] / a;
        let dphi = if jdoder {
            (gpr[n] * f[n] - g[n] * fpr[n]) / asq
        } else {
            0.0
        };
        (sinphi, cosphi, dphi)
    } else {
        (0.0, 0.0, 0.0)
    };
    CoulombPsp {
        p,
        s,
        dp,
        sinphi,
        cosphi,
        dphi,
        dshift: 0.0,
    }
}

/// Dispatcher between the three Coulomb evaluation strategies (large `eta`,
/// finite `eta` with large `rhoi`, finite `eta` with moderate `rhoi`) —
/// ported from `coulx` (`samm.f90:4340-4399`).
pub fn coulx(
    eta: f64,
    rho: f64,
    lll: i32,
    jdopha: bool,
    jdoder: bool,
    ishift: i32,
) -> Option<CoulombPsp> {
    if eta > 10.0 * rho && eta > 5.0 {
        return bigeta(eta, rho, lll, jdopha, jdoder, ishift);
    }

    let (u, upr, rhoi) = if eta >= 5.0 {
        let (u, upr, rhoi) = asymp1(eta);
        taylor(eta, rho, u, upr, rhoi)
    } else {
        let sigma = xsigll(eta, lll.max(0));
        let sigma0 = sigma[0];
        let (u, upr, rhoi) = asymp2(eta, rho, sigma0);
        if u.abs() <= 1.0e25 {
            taylor(eta, rho, u, upr, rhoi)
        } else {
            (u, upr, rhoi)
        }
    };
    let _ = rhoi;

    let g0 = u;
    let g0pr = upr;
    let (f, fpr, g, gpr) = if g0.abs() > 1.0e25 {
        let (_llmax, f, fpr, g, gpr) = end1(g0, g0pr);
        (f, fpr, g, gpr)
    } else {
        let (_llmax, f, fpr, g, gpr) = getfg(eta, rho, lll, lll, g0, g0pr);
        (f, fpr, g, gpr)
    };

    Some(getps(rho, lll, &f, &fpr, &g, &gpr, jdopha, jdoder, ishift))
}
