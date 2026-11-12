//! Large-`eta` Bessel-function case and top-level evaluation-strategy
//! dispatcher — `bigeta`, `getps`, `coulx`, ported from
//! `samm.f90:4340-4399` (`coulx`) and `samm.f90:4852-4996`
//! (`bigeta`/`getps`). The K0/K1 harmonic sums follow NJOY2016 2016.80 (single-precision
//! `1./(k+1)`, see `harmonic_step`).

use petir::mathf::RealMath;
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

/// `1./(k+1)` exactly as NJOY2016 2016.80 `samm.f90` `bigeta` evaluates it in the K0 and K1
/// harmonic sums (`b=b+1./(k+1)`, fix #415): `1.` is a default-kind (single-precision) real, so
/// the quotient is formed in `f32` and then widened to `f64` when added to the double-precision
/// `b`. (2016.79 wrote `1/(k+1)`, integer division, which is 0 for `k >= 1`.) The `f32`
/// rounding is deliberate: it is what makes the port bit-identical to 2016.80
/// (`tests/samm_bigeta_vs_njoy2016_units.rs`, gh:#1366, #1368). The sums are
/// ill-conditioned at large `2*rho*eta` (K0 is a difference of terms ~ `exp(2z)` larger), so
/// the f32 rounding changes K0 and K1 by up to O(1) there, not by ~1e-8.
#[inline]
fn harmonic_step(k: i32) -> f64 {
    f64::from(1.0_f32 / (k + 1) as f32)
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
        a = a * q / ((k + 1) as f64).r_powi(2);
    }
    if !converged {
        log::error!("samm bigeta: I0 sum failed to converge (eta={eta}, rho={rho})");
        return None;
    }
    let ai0 = sum;

    // samm.f90:4888-4900 -- K_0(z), A&S 9.6.13.
    let mut sum = -(zhalf.r_ln() + EULER) * ai0;
    let mut a = q;
    let mut b = 1.0_f64;
    let mut converged = false;
    for k in 1..=100 {
        if sum + a * b == sum {
            converged = true;
            break;
        }
        sum += a * b;
        b += harmonic_step(k);
        a = a * q / ((k + 1) as f64).r_powi(2);
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
    let mut sum = 1.0 / z + (zhalf.r_ln() + EULER) * ai1 - zhalf * 0.5;
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
        b += harmonic_step(k);
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
        // Upstream's `bigeta` passes its `llmax` argument, which `pspcou` sets to `lll+2`
        // (`samm.f90:3935`, `api.rs`); `getfg` takes `limit = max(3, llmax+1)` from it, so
        // passing 0 here (as this port did until 2026-11-10, gh:#1366) started the
        // recursions from a different `limit` and changed F, G in the last bits for `lll >= 1`.
        let (_llmax_out, f2, fpr2, g2, gpr2) = getfg(eta, rho, lll + 2, lll, g0, g0pr);
        f = f2;
        fpr = fpr2;
        g = g2;
        gpr = gpr2;
    }

    let a = PI * eta;
    let b = (-a).r_exp();
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
    let asq = f[n].r_powi(2) + g[n].r_powi(2);
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
        // Upstream calls `getfg(eta,rho,llmax,lll,..)` with `llmax = lll+2` (`pspcou`,
        // `samm.f90:3935`); this port passed `lll` until 2026-11-10 (gh:#1368), which moved the
        // start of `getfg`'s backward recursion (`limit = max(3, llmax+1)`) for `lll >= 1`.
        let (_llmax, f, fpr, g, gpr) = getfg(eta, rho, lll + 2, lll, g0, g0pr);
        (f, fpr, g, gpr)
    };

    Some(getps(rho, lll, &f, &fpr, &g, &gpr, jdopha, jdoder, ishift))
}
