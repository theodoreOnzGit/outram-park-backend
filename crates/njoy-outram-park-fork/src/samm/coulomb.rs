//! Coulomb wave functions for charged-particle channels — ported from
//! `samm.f90`'s Coulomb library: `jwkb`, `coulfg` (Steed's method, the CPC
//! "COULFG" algorithm of Barnett), `xsigll`, `asymp1`, `asymp2`, `taylor`,
//! `end1`, `getfg`, `bigeta`, `getps`, `coulx`, `pspcou`, `pghcou`.
//!
//! This is the charged-particle counterpart of [`super::penetrability`]:
//! where that module's `pgh` gives the penetrability/shift/phase-shift for
//! uncharged (neutral) channels via closed-form hard-sphere formulas, this
//! module's [`pghcou`] gives the same three quantities for channels where
//! the Coulomb interaction between the two exit particles matters (charged
//! exit channels — e.g. `(n,p)`, `(n,alpha)` on light nuclides).
//!
//! # Indexing convention
//!
//! Every Fortran array here (`fc`/`gc`/`fcp`/`gcp` in `coulfg`; `f`/`fpr`/
//! `g`/`gpr` elsewhere) is 1-indexed with array position `p` holding the
//! quantity at orbital angular momentum `L = p-1` (`samm.f90`'s own
//! convention, since `Xlm`/`Llmin` is always `0` in this port — see
//! [`coulfg`]'s doc comment). This port uses **direct 0-indexed-by-`L`**
//! `Vec<f64>`s throughout instead (`vec[L]` rather than `array(L+1)`),
//! which is the natural Rust mapping and removes a constant `+1`/`-1` from
//! every access site — but every derivation below is still checked
//! position-by-position against the original Fortran, not re-derived from
//! scratch, precisely because an indexing slip here is easy to make and
//! hard to notice.

use crate::common::phys::{EULER, PI};

/// JWKB (semiclassical) approximation to the irregular Coulomb function
/// `G_l(eta,x)` and its reciprocal-product partner `F_l`, used by
/// [`coulfg`] deep in the classically-forbidden region — ported from
/// `jwkb` (`samm.f90:4297-4338`).
///
/// Returns `None` exactly when upstream's early `return` (before setting
/// any output) fires — `samm.f90:4318`, `gh2+xll1<=0`. Both call sites in
/// this module already have `fjwkb=0`/`gjwkb=0`/`iexp` at their prior
/// default when that happens (`coulfg` sets `gjwkb=0`,`iexp=1` before
/// calling), so a `None` here should be treated as "leave the caller's
/// current values untouched", not as an error.
pub struct Jwkb {
    pub fjwkb: f64,
    pub gjwkb: f64,
    pub iexp: i32,
}

pub fn jwkb(xx: f64, eta1: f64, xl: f64) -> Option<Jwkb> {
    const ALOGE: f64 = 0.434_294_481_903_251_816_667_932;
    const SIX35: f64 = 0.171_428_571_428_571_428_571_428_571_428_571_428_571_4285;

    let x = xx;
    let eta = eta1;
    let gh2 = x * (eta + eta - x);
    let xll1 = (xl * xl + xl).max(0.0);
    if gh2 + xll1 <= 0.0 {
        return None;
    }
    let hll = xll1 + SIX35;
    let hl = hll.sqrt();
    let sl = eta / hl + hl / x;
    let rl2 = 1.0 + eta * eta / hll;
    let gh = (gh2 + hll).sqrt() / x;
    let mut phi = x * gh - 0.5 * (hl * ((gh + sl).powi(2) / rl2).ln() - gh.ln());
    if eta != 0.0 {
        phi -= eta * (x * gh).atan2(x - eta);
    }

    let phi10 = -phi * ALOGE;
    let mut iexp = phi10 as i32; // Fortran `int()` truncates toward zero, matching `as i32`.
    let gjwkb;
    if iexp > 70 {
        gjwkb = 10f64.powf(phi10 - iexp as f64);
    } else {
        gjwkb = (-phi).exp();
        iexp = 0;
    }
    let fjwkb = 0.5 / (gh * gjwkb);

    Some(Jwkb { fjwkb, gjwkb, iexp })
}

/// Result of [`coulfg`]: regular/irregular Coulomb functions `F_L`, `G_L`
/// (and derivatives) for `L=0..=llmax`, plus the penetrability/shift/phase
/// shift at `L=lll`. `ifail != 0` on any of the four failure modes upstream
/// reports via `write` + early `return` (`samm.f90:4265-4293`); on failure
/// the array fields are whatever was computed up to the failure point (not
/// meaningful) — callers must check `ifail` first, matching upstream.
pub struct CoulfgResult {
    pub fc: Vec<f64>,
    pub gc: Vec<f64>,
    pub fcp: Vec<f64>,
    pub gcp: Vec<f64>,
    pub pcoul: f64,
    pub scoul: f64,
    pub dpcoul: f64,
    pub sinphi: f64,
    pub cosphi: f64,
    pub dphi: f64,
    pub ifail: i32,
    pub iexp: i32,
}

/// Revised Coulomb wave-function routine using Steed's method — ported from
/// `coulfg` (`samm.f90:4003-4295`, the CPC "COULFG" algorithm, A.R. Barnett
/// et al.). Returns `F,G,F',G'` for real `xx>0`, real `eta1` (including 0),
/// for integer angular momenta `L=0..=llmax` (`xlm`/`Llmin` hardcoded to
/// `0` — `samm.f90:4050`, matching upstream's own comment that this is
/// "automatic with NML re-write, which assumes Llmin=0").
///
/// `jdopha`/`jdoder`: whether to compute the phase shift and its
/// derivative (upstream's `jdopha`/`jdoder` flags, `>0` meaning "yes").
/// `ishift`: whether to compute the shift factor (`>0` meaning "yes",
/// matching [`super::mf2::ParticlePair::shift_flag`]).
pub fn coulfg(xx: f64, eta1: f64, lll: i32, llmax: i32, jdopha: bool, jdoder: bool, ishift: i32) -> CoulfgResult {
    const ACCUR: f64 = 1.0e-16;
    const TM30: f64 = 1.0e-30;
    const ABORT: f64 = 2.0e4;

    let n = (llmax + 1) as usize;
    let mut fc = vec![0.0f64; n];
    let mut gc = vec![0.0f64; n];
    let mut fcp = vec![0.0f64; n];
    let mut gcp = vec![0.0f64; n];
    let mut pcoul = 0.0_f64;
    let mut scoul = 0.0_f64;
    let mut dpcoul = 0.0_f64;
    let mut sinphi = 0.0_f64;
    let mut cosphi = 0.0_f64;
    let mut dphi = 0.0_f64;

    let acc = ACCUR;
    let acc4 = acc * 100.0 * 100.0 / 10.0;
    let acch = acc.sqrt();

    if xx <= acch {
        log::error!("samm coulfg: xx={xx} <= sqrt(accur)={acch}; try small-x solutions, or check for negative x");
        return CoulfgResult {
            fc, gc, fcp, gcp, pcoul, scoul, dpcoul, sinphi, cosphi, dphi, ifail: -1, iexp: 1,
        };
    }

    let x = xx;
    let eta = eta1;
    // xlm=0 always (see this function's doc comment), so e2mm1 = eta^2 and
    // the xlturn threshold simplifies to `x*(x-2*eta) < 0`.
    let e2mm1 = eta * eta;
    let mut xlturn = x * (x - 2.0 * eta) < 0.0;

    let xll = llmax as f64;
    let xi = 1.0 / x;
    let mut fcl = 1.0_f64;
    let mut pk = xll + 1.0;
    let px = pk + ABORT;

    // samm.f90:4064-4082 ("10 continue" -- CF1 initial value, with its own
    // possible B1-fixup re-entry loop).
    let mut f;
    let mut df;
    let mut d;
    let mut pk1;
    loop {
        let ek = eta / pk;
        f = (ek + pk * xi) * fcl + (fcl - 1.0) * xi;
        pk1 = pk + 1.0;
        if (eta * x + pk * pk1).abs() <= acc {
            fcl = (1.0 + ek * ek) / (1.0 + (eta / pk1).powi(2));
            pk = 2.0 + pk;
            continue;
        }
        d = 1.0 / ((pk + pk1) * (xi + ek / pk1));
        df = -fcl * (1.0 + ek * ek) * d;
        if fcl != 1.0 {
            fcl = -1.0;
        }
        if d < 0.0 {
            fcl = -fcl;
        }
        f += df;
        break;
    }

    // samm.f90:4084-4106 ("20 continue" -- CF1 loop on Pk=K=lambda+1).
    let mut p_retry = 0;
    loop {
        pk = pk1;
        pk1 += 1.0;
        let ek = eta / pk;
        let tk = (pk + pk1) * (xi + ek / pk1);
        d = tk - d * (1.0 + ek * ek);
        if d.abs() <= acch {
            log::warn!(
                "samm coulfg: CF1 accuracy loss (d={d}, df={df}, acch={acch}, k={pk}, eta/k={ek}, eta={eta}, x={x})"
            );
            p_retry += 1;
            if p_retry > 2 {
                log::error!("samm coulfg: CF1 failed to converge after {ABORT} iterations (f={f}, df={df}, pk={pk}, px={px}, acc={acc})");
                return CoulfgResult {
                    fc, gc, fcp, gcp, pcoul, scoul, dpcoul, sinphi, cosphi, dphi, ifail: 1, iexp: 1,
                };
            }
        }
        d = 1.0 / d;
        if d < 0.0 {
            fcl = -fcl;
        }
        df *= d * tk - 1.0;
        f += df;
        if pk > px {
            log::error!("samm coulfg: CF1 failed to converge after {ABORT} iterations (f={f}, df={df}, pk={pk}, px={px}, acc={acc})");
            return CoulfgResult {
                fc, gc, fcp, gcp, pcoul, scoul, dpcoul, sinphi, cosphi, dphi, ifail: 1, iexp: 1,
            };
        }
        if df.abs() < f.abs() * acc {
            break;
        }
    }

    // samm.f90:4108-4135 -- downward recurrence to L=0. Fortran's running
    // `xl` local is provably equal to the loop's descending Fortran
    // position `l_upper` at every step (both start at `llmax` and
    // decrement by 1 per iteration), so it is not carried as a separate
    // variable here -- `l_upper as f64` is used directly in its place.
    if llmax > 0 {
        fcl *= TM30;
        let mut fpl = fcl * f;
        fc[llmax as usize] = fcl;
        fcp[llmax as usize] = fpl;
        for l_upper in (1..=llmax).rev() {
            let xl = l_upper as f64;
            let el = eta / xl;
            let rl = (1.0 + el * el).sqrt();
            let sl = el + xl * xi;
            let fcl1 = (fcl * sl + fpl) / rl;
            let fpl_new = fcl1 * sl - fcl * rl;
            fcl = fcl1;
            fpl = fpl_new;
            fc[(l_upper - 1) as usize] = fcl;
            fcp[(l_upper - 1) as usize] = fpl;
            gc[l_upper as usize] = rl; // scratch: RL, reused as G(L) below
        }
        if fcl == 0.0 {
            fcl = acc;
        }
        f = fpl / fcl;
    }

    let mut fjwkb = 0.0_f64;
    let mut gjwkb = 0.0_f64;
    let mut iexp = 1_i32;
    if xlturn {
        if let Some(out) = jwkb(x, eta, 0.0) {
            fjwkb = out.fjwkb;
            gjwkb = out.gjwkb;
            iexp = out.iexp;
        }
    }

    let p_val;
    let q_val;
    let w;
    let gam;
    if iexp > 1 || gjwkb > 1.0 / (acch * 100.0) {
        w = fjwkb;
        gam = gjwkb * w;
        p_val = f;
        q_val = 1.0;
    } else {
        xlturn = false;
        let ta = 2.0 * ABORT;
        let mut pk2 = 0.0_f64;
        let wi = eta + eta;
        let mut p = 0.0_f64;
        let mut q = 1.0 - eta * xi;
        let mut ar = -e2mm1;
        let mut ai = eta;
        let br = 2.0 * (x - eta);
        let mut bi = 2.0_f64;
        let mut dr = br / (br * br + bi * bi);
        let mut di = -bi / (br * br + bi * bi);
        let mut dp = -xi * (ar * di + ai * dr);
        let mut dq = xi * (ar * dr - ai * di);
        loop {
            p += dp;
            q += dq;
            pk2 += 2.0;
            ar += pk2;
            ai += wi;
            bi += 2.0;
            let d2 = ar * dr - ai * di + br;
            let di2 = ai * dr + ar * di + bi;
            let c = 1.0 / (d2 * d2 + di2 * di2);
            dr = c * d2;
            di = -c * di2;
            let a = br * dr - bi * di - 1.0;
            let b = bi * dr + br * di;
            let c2 = dp * a - dq * b;
            dq = dp * b + dq * a;
            dp = c2;
            if pk2 > ta {
                log::error!("samm coulfg: CF2 failed to converge after {ABORT} iterations (p={p}, q={q}, dp={dp}, dq={dq}, acc={acc})");
                return CoulfgResult {
                    fc, gc, fcp, gcp, pcoul, scoul, dpcoul, sinphi, cosphi, dphi, ifail: 2, iexp: 1,
                };
            }
            if dp.abs() + dq.abs() < (p.abs() + q.abs()) * acc {
                break;
            }
        }
        // Upstream also computes `paccq` here (an accuracy estimate) but
        // never reads it again in this subroutine -- write-only dead local,
        // intentionally not ported.
        if q <= acc4 * p.abs() {
            log::error!("samm coulfg: final q <= |p|*acc*1e4 (q={q}, p={p}, acc={acc}, llmax={llmax})");
            return CoulfgResult {
                fc, gc, fcp, gcp, pcoul, scoul, dpcoul, sinphi, cosphi, dphi, ifail: 3, iexp: 1,
            };
        }
        gam = (f - p) / q;
        w = 1.0 / ((f - p) * gam + q).sqrt();
        p_val = p;
        q_val = q;
    }

    let fcm = w.copysign(fcl); // Fortran SIGN(w,fcl): magnitude of w, sign of fcl.
    fc[0] = fcm;
    let gcl_init = if !xlturn { fcm * gam } else { gjwkb };
    gc[0] = gcl_init;
    let gpl_init = gcl_init * (p_val - q_val / gam);
    gcp[0] = gpl_init;
    fcp[0] = fcm * f;

    // samm.f90:4206-4222 -- upward recurrence, renormalizing Fc/Fcp.
    // Fortran's running `xl` is again provably equal to the loop index `l`
    // here (both start at 0 and increment by 1 per iteration).
    if llmax > 0 {
        let mut gcl = gcl_init;
        let mut gpl = gpl_init;
        let wn = w / fcl.abs();
        for l in 1..=llmax {
            let xl = l as f64;
            let el = eta / xl;
            let rl = gc[l as usize]; // holds RL from the downward pass
            let sl = el + xl * xi;
            let gcl1 = (sl * gcl - gpl) / rl;
            gpl = rl * gcl - sl * gcl1;
            gcl = gcl1;
            gc[l as usize] = gcl1;
            gcp[l as usize] = gpl;
            fcp[l as usize] = wn * fcp[l as usize];
            fc[l as usize] = wn * fc[l as usize];
        }
    }

    // samm.f90:4224-4259 -- penetrability, shift factor, phase shift at L=lll.
    let l_idx = lll as usize; // Fortran `l=lll+1` -> myL = lll directly.
    if iexp > 1 {
        if iexp < 150 {
            let asq = gc[l_idx].powi(2);
            let aaa = 10f64.powi(-iexp * 2);
            pcoul = xx / asq * aaa;
            if jdopha {
                sinphi = fc[l_idx] / gc[l_idx] * aaa;
                cosphi = 1.0 - sinphi.powi(2);
                if jdoder {
                    dphi = gcp[l_idx] / gc[l_idx] * sinphi - fcp[l_idx] / gc[l_idx] * aaa;
                }
            }
        }
        if ishift > 0 {
            scoul = xx * gcp[l_idx] / gc[l_idx];
        }
    } else {
        let asq = fc[l_idx].powi(2) + gc[l_idx].powi(2);
        pcoul = xx / asq;
        let sss = xx * (fc[l_idx] * fcp[l_idx] + gc[l_idx] * gcp[l_idx]) / asq;
        if ishift > 0 {
            scoul = sss;
        }
        if jdoder {
            dpcoul = (1.0 - 2.0 * sss) / asq;
        }
        if jdopha {
            let a = asq.sqrt();
            sinphi = fc[l_idx] / a;
            cosphi = gc[l_idx] / a;
            if jdoder {
                dphi = (gcp[l_idx] * fc[l_idx] - gc[l_idx] * fcp[l_idx]) / asq;
            }
        }
    }

    CoulfgResult { fc, gc, fcp, gcp, pcoul, scoul, dpcoul, sinphi, cosphi, dphi, ifail: 0, iexp }
}

/// Coulomb phase shift `sigma_L = arg(Gamma(L+1+i*eta))` for `L=0..=lmax`
/// — ported from `xsigll` (`samm.f90:4429-4517`), needed by [`asymp2`]'s
/// asymptotic expansion of `G_0`. Returns `sigma[0..=lmax]` (`sigma[L]` =
/// upstream's `sigma(L+1)`).
pub fn xsigll(eta: f64, lmax: i32) -> Vec<f64> {
    const SMALL: f64 = 0.000_001;
    const BER: [f64; 5] = [
        0.166_666_666_666_666_666_666_666_666_666_666_667,
        -0.033_333_333_333_333_333_333_333_333_333_333_333,
        0.023_809_523_809_523_809_523_809_523_809_523_810,
        -0.033_333_333_333_333_333_333_333_333_333_333_333,
        0.075_757_575_757_575_757_575_757_575_757_575_758,
    ];
    const MMMXXX: i64 = 100_000;

    let peta = eta.abs();
    let mut sigma0;
    if peta >= 3.0 {
        let mut sum = 0.0_f64;
        for i in 1..=5 {
            let xi = i as f64;
            let m = 2 * i - 1;
            let xm = m as f64;
            sum += BER[i - 1] / (2.0 * xi * xm * peta.powi(m as i32));
        }
        sigma0 = PI / 4.0 + peta * (peta.ln() - 1.0) - sum;
    } else {
        let mut sumas = 0.0_f64;
        for is in 1..=MMMXXX {
            let s = is as f64;
            let temp1 = peta / s;
            let as_;
            if s <= 2.0 * peta {
                as_ = temp1 - temp1.atan();
            } else {
                let mut acc = 0.0_f64;
                let mut k = 0;
                for j in 1..=MMMXXX {
                    let m = j + j + 1;
                    let xm = m as f64;
                    let add = temp1.powi(m as i32) / xm;
                    if k == 0 {
                        acc += add;
                        k = 1;
                    } else {
                        acc -= add;
                        k = 0;
                    }
                    if (add / acc).abs() <= SMALL {
                        break;
                    }
                }
                as_ = acc;
            }
            sumas += as_;
            if (as_ / sumas).abs() <= SMALL {
                break;
            }
        }
        sigma0 = -EULER * peta + sumas;
    }
    if eta < 0.0 {
        sigma0 = -sigma0;
    }

    let mut sigma = vec![0.0_f64; (lmax.max(0) as usize) + 1];
    sigma[0] = sigma0;
    for ll in 1..=lmax {
        let xl = ll as f64;
        sigma[ll as usize] = sigma[(ll - 1) as usize] + (eta / xl).atan();
    }
    sigma
}

/// Asymptotic expansion of `G_0`, `G_0'` at `rhoi=2*eta`, valid for large
/// `eta` — ported from `asymp1` (`samm.f90:4401-4427`, Abramowitz & Stegun
/// Eqs. 14.5.12b/14.5.13b).
pub fn asymp1(eta: f64) -> (f64, f64, f64) {
    let ceta = eta.powf(1.0 / 3.0);
    let seta = ceta.sqrt();
    let temp = 1.0 / (ceta * ceta);
    let u = 1.223_404_016 * seta
        * (1.0
            + temp.powi(2)
                * (0.049_595_701_65
                    + temp
                        * (-0.008_888_888_889
                            + temp.powi(2) * (0.002_455_199_181 + temp * (-0.000_910_895_806_1 + temp.powi(2) * 0.000_253_468_411_5)))));
    let upr = -0.707_881_773_4
        * (1.0
            + temp
                * (-0.172_826_036_9
                    + temp.powi(2)
                        * (0.000_317_460_317_4 + temp * (-0.003_581_214_850 + temp.powi(2) * (0.000_311_782_468_0 - temp * 0.000_907_396_642_7)))))
        / seta;
    let rhoi = 2.0 * eta;
    (u, upr, rhoi)
}

/// Asymptotic expansion of `G_0`, `G_0'` at large `rhoi` for finite `eta`
/// — ported from `asymp2` (`samm.f90:4519-4601`, Abramowitz & Stegun Eqs.
/// 14.5.1-8), doubling `rhoi` and retrying until the continued expansion
/// converges.
pub fn asymp2(eta: f64, rho: f64, sigma0: f64) -> (f64, f64, f64) {
    const EPSLON: f64 = 0.000_001;
    const DEL: f64 = 100.0;

    let mut rhoi = (rho * 2.0).max(10.0).max(10.0 * eta);

    'retry: loop {
        let mut xn = 0.0_f64;
        let mut zold = [1.0_f64, 0.0, 0.0, 1.0 - eta / rhoi];
        let mut z = zold;
        let mut bigz = [z[0].abs(), z[1].abs(), z[2].abs(), z[3].abs()];

        let mut jcheck = 0;
        for _n in 1..=100 {
            let temp = 2.0 * (xn + 1.0) * rhoi;
            let an = (2.0 * xn + 1.0) * eta / temp;
            let bn = (eta * eta - xn * (xn + 1.0)) / temp;
            xn += 1.0;
            let znew = [
                an * zold[0] - bn * zold[1],
                an * zold[1] + bn * zold[0],
                0.0, // filled below (depends on znew[0])
                0.0, // filled below (depends on znew[1])
            ];
            let znew2 = an * zold[2] - bn * zold[3] - znew[0] / rhoi;
            let znew3 = an * zold[3] + bn * zold[2] - znew[1] / rhoi;
            let znew = [znew[0], znew[1], znew2, znew3];

            let mut icheck = 0;
            for i in 0..4 {
                z[i] += znew[i];
                zold[i] = znew[i];
                let temp2 = z[i].abs();
                bigz[i] = bigz[i].max(z[i].abs());
                if bigz[i] / temp2 > DEL {
                    rhoi *= 2.0;
                    continue 'retry;
                }
                if (znew[i] / z[i]).abs() <= EPSLON {
                    icheck += 1;
                }
            }
            let w = z[0] * z[3] - z[1] * z[2];
            if w.abs() > 10.0 {
                rhoi *= 2.0;
                continue 'retry;
            }
            if icheck == 4 {
                jcheck += 1;
                if jcheck >= 4 {
                    let phi = rhoi - eta * (2.0 * rhoi).ln() + sigma0;
                    let cosphi = phi.cos();
                    let sinphi = phi.sin();
                    let g0 = z[0] * cosphi - z[1] * sinphi;
                    let g0pr = z[2] * cosphi - z[3] * sinphi;
                    return (g0, g0pr, rhoi);
                }
            } else {
                jcheck = 0;
            }
        }
        // Fell through the `n=1..100` loop without converging: same
        // "double rhoi and retry" fallback as the `bigz`/`w` early exits.
        rhoi *= 2.0;
    }
}

/// Taylor-series integration of `u=G_0`, `u'=G_0'` from `arg=rhoi` (where
/// they're already known) to `arg=rho` — ported from `taylor`
/// (`samm.f90:4603-4735`), solving `u'' + (1-2*eta/rho)*u = 0` via a
/// power-series expansion in `delta=rho-rhoi`, halving `delta` and
/// stepping partway when the full-`delta` series doesn't converge.
pub fn taylor(eta: f64, rho: f64, u_in: f64, upr_in: f64, rhoi_in: f64) -> (f64, f64, f64) {
    const EPSLON: f64 = 1.0e-6;
    const BIGGER: f64 = 1.0e10;
    const BIGGST: f64 = 1.0e30;
    const DEL: f64 = 100.0;

    let mut u = u_in;
    let mut upr = upr_in;
    let mut rhoi = rhoi_in;
    let mut delta = rho - rhoi;
    if delta == 0.0 {
        return (u, upr, rhoi);
    }

    let mut a = vec![0.0_f64; 101]; // a[1..=100] used (Fortran 1-indexed); a[0] unused.

    'outer: loop {
        a[1] = u;
        a[2] = delta * upr;
        a[3] = -delta * delta / 2.0 * (1.0 - 2.0 * eta / rhoi) * a[1];
        let mut nstart = 4_i64;

        loop {
            let mut jcheck = 0;
            let mut sum = 0.0_f64;
            let mut sumpr = 0.0_f64;
            let mut big = 0.0_f64;
            let mut bigpr = 0.0_f64;
            let mut n_final = 100_i64;
            let mut converged = false;
            let mut failed_at = 0_i64;

            for n in 1..=100_i64 {
                let xn = (n - 1) as f64;
                if n >= nstart {
                    let a_n = -(delta * (xn - 1.0) * (xn - 2.0) * a[(n - 1) as usize]
                        + (rhoi - 2.0 * eta) * delta.powi(2) * a[(n - 2) as usize]
                        + delta.powi(3) * a[(n - 3) as usize])
                        / (rhoi * (xn - 1.0) * xn);
                    a[n as usize] = a_n;
                    if a_n > BIGGER {
                        failed_at = n;
                        break;
                    }
                }
                sum += a[n as usize];
                sumpr += xn * a[n as usize];
                if sum >= BIGGST || sumpr >= BIGGST {
                    failed_at = n;
                    break;
                }
                big = big.max(sum.abs());
                bigpr = bigpr.max(sumpr.abs());
                if sum == 0.0 || sumpr == 0.0 {
                    jcheck = 0;
                } else {
                    if (big / sum).abs() >= DEL || (bigpr / sumpr).abs() >= DEL {
                        failed_at = n;
                        break;
                    }
                    if (a[n as usize] / sum).abs() >= EPSLON || (xn * a[n as usize] / sumpr).abs() >= EPSLON {
                        jcheck = 0;
                    } else {
                        jcheck += 1;
                        if jcheck >= 4 {
                            converged = true;
                            n_final = n;
                            break;
                        }
                    }
                }
                n_final = n;
            }

            if converged {
                // samm.f90:4725-4732 ("60 continue")
                u = sum;
                upr = sumpr / delta;
                rhoi += delta;
                delta = rho - rhoi;
                if delta.abs() >= EPSLON {
                    continue 'outer;
                }
                return (u, upr, rhoi);
            }

            // samm.f90:4710-4723 ("40 continue") -- series didn't converge;
            // halve delta (find u(rhoi+delta/2) instead) and retry from the
            // current `nstart`/`a` state.
            let n = if failed_at != 0 { failed_at } else { n_final };
            nstart = nstart.max(n + 1);
            let m = nstart - 1;
            delta /= 2.0;
            let mut temp = 2.0_f64;
            for k in 1..=m {
                temp /= 2.0;
                a[k as usize] *= temp;
            }
        }
    }
}

/// Degenerate `L=0`-only result (`llmax` forced to `0`) — ported from
/// `end1` (`samm.f90:4737-4751`), the `abs(g0)>1e25` fallback in [`coulx`]
/// where `getfg`'s recursion would overflow.
pub fn end1(g0: f64, g0pr: f64) -> (i32, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    (0, vec![0.0], vec![0.0], vec![g0], vec![g0pr])
}

/// `F_L`, `G_L` (and derivatives) for `L=0..=llmax`, given `G_0`, `G_0'`
/// — ported from `getfg` (`samm.f90:4753-4850`). Returns the (possibly
/// reduced, see below) `llmax` actually filled, plus `f,fpr,g,gpr` sized to
/// it.
///
/// The forward recursion for `G_L` (Abramowitz & Stegun Eq. 14.2.3) can
/// overflow for large `L`; if `|G_L|` exceeds `1e12` past `L=lll`, upstream
/// stops early and reports the reduced `llmax` back to the caller
/// (`samm.f90:4780-4783`, `if (limit.le.lmax) llmax=limit-1`) — this is why
/// the return type carries `llmax` rather than assuming the input value.
pub fn getfg(eta: f64, rho: f64, lmax: i32, lll: i32, g0: f64, g0pr: f64) -> (i32, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    const BIG: f64 = 1.0e12;

    let limit0 = 3.max(lmax + 1);
    let mut g = vec![0.0_f64; (limit0 as usize) + 1];
    let mut gpr = vec![0.0_f64; (limit0 as usize) + 1];
    g[0] = g0;
    gpr[0] = g0pr;
    g[1] = ((eta + 1.0 / rho) * g[0] - gpr[0]) / (eta * eta + 1.0).sqrt();

    let mut limit = limit0;
    for l in 3..=limit0 {
        let xl = (l - 1) as f64;
        let temp1 = (xl * xl + eta * eta).sqrt();
        let gl = (2.0 * xl - 1.0) / temp1 * (eta / (xl - 1.0) + xl / rho) * g[(l - 2) as usize]
            - xl / temp1 * (1.0 + (eta / (xl - 1.0)).powi(2)).sqrt() * g[(l - 3) as usize];
        g[(l - 1) as usize] = gl;
        if gl.abs() > BIG && l > lll {
            limit = l;
            break;
        }
    }

    // samm.f90:4786-4803 -- find J such that G(J) is well-converged
    // relative to G(limit) three checks in a row.
    let mut gm2 = g[(limit - 2) as usize];
    let mut gm1 = g[(limit - 1) as usize];
    let mut il: i32 = -1;
    let mut j: i64 = limit as i64;
    let mut gm = gm1;
    for jj in (limit as i64)..=10_000 {
        j = jj;
        let xl = jj as f64;
        let temp1 = (xl * xl + eta * eta).sqrt();
        gm = (2.0 * xl - 1.0) / temp1 * (eta / (xl - 1.0) + xl / rho) * gm1
            - xl / temp1 * (1.0 + (eta / (xl - 1.0)).powi(2)).sqrt() * gm2;
        if (g[(limit - 1) as usize] / gm).abs() > 1.0e-4 {
            il = -2;
        }
        if il > 0 {
            break;
        }
        il += 1;
        gm2 = gm1;
        gm1 = gm;
    }

    // samm.f90:4805-4822 -- approximate F(limit+3..j-1) in reverse, not stored.
    let xl_j = j as f64;
    let mut fp1 = xl_j / gm / (xl_j * xl_j + eta * eta).sqrt();
    let mut fp2 = 0.0_f64;
    let mut l = j - 1;
    let n1 = (j - 3 - limit as i64).max(0);
    for _ in 0..n1 {
        l -= 1;
        let xl = l as f64;
        let temp2 = ((xl + 1.0).powi(2) + eta * eta).sqrt();
        let fp = ((2.0 * xl + 3.0) * (eta / (xl + 2.0) + (xl + 1.0) / rho) * fp1
            - (xl + 1.0) * (1.0 + (eta / (xl + 2.0)).powi(2)).sqrt() * fp2)
            / temp2;
        fp2 = fp1;
        fp1 = fp;
    }

    // samm.f90:4824-4835 -- F(1..limit+1), reverse recursion, stored this time.
    let f_size = (limit + 2) as usize;
    let mut f = vec![0.0_f64; f_size];
    for _ in 0..(limit + 2) {
        l -= 1;
        let xl = l as f64;
        let temp2 = ((xl + 1.0).powi(2) + eta * eta).sqrt();
        let fp = ((2.0 * xl + 3.0) * (eta / (xl + 2.0) + (xl + 1.0) / rho) * fp1
            - (xl + 1.0) * (1.0 + (eta / (xl + 2.0)).powi(2)).sqrt() * fp2)
            / temp2;
        f[l as usize] = fp;
        fp2 = fp1;
        fp1 = fp;
    }

    // samm.f90:4837-4845 -- Fpr, Gpr for L=1..limit-1 (Fpr(1) handled separately).
    let mut fpr = vec![0.0_f64; f_size];
    fpr[0] = (1.0 / rho + eta) * f[0] - (1.0 + eta * eta).sqrt() * f[1];
    for l in 2..=limit {
        let xl = l as f64;
        fpr[(l - 1) as usize] = (xl / rho + eta / xl) * f[(l - 1) as usize] - (1.0 + (eta / xl).powi(2)).sqrt() * f[l as usize];
        let temp1 = eta / (xl - 1.0);
        gpr[(l - 1) as usize] = (1.0 + temp1 * temp1).sqrt() * g[(l - 2) as usize] - ((xl - 1.0) / rho + temp1) * g[(l - 1) as usize];
    }

    let llmax_out = if limit <= lmax { limit - 1 } else { lmax };
    (llmax_out, f, fpr, g, gpr)
}

/// Result of [`bigeta`]/[`coulx`]/[`pspcou`]: penetrability, shift factor,
/// phase shift (and derivatives) at a single channel `L`.
pub struct CoulombPsp {
    pub p: f64,
    pub s: f64,
    pub dp: f64,
    pub sinphi: f64,
    pub cosphi: f64,
    pub dphi: f64,
    /// `dS/d(rho)`, finite-differenced by [`pspcou`] at `rho*1.01` when
    /// `ishift>0 && jdoder` (upstream's separate `dshiftcoul` output,
    /// `samm.f90:3948-3955`/`3964-3977`/`3990-3995` — distinct from `dp`,
    /// which is `dP/d(rho)`). `0.0` when not requested or not applicable
    /// (matching upstream, which leaves `dshiftcoul=0` at `pspcou`'s top).
    pub dshift: f64,
}

/// `F,G` at `L=0` (and, if `lll>0`, all `L=0..=llmax` via [`getfg`]) plus
/// `P,S,phi` at `L=lll`, for `eta >> rho` — ported from `bigeta`
/// (`samm.f90:4852-4967`, Abramowitz & Stegun Eqs. 14.6.7-8), using the
/// modified Bessel functions `I_0,I_1,K_0,K_1` of `z=2*sqrt(2*rho*eta)`.
pub fn bigeta(eta: f64, rho: f64, lll: i32, jdopha: bool, jdoder: bool, ishift: i32) -> Option<CoulombPsp> {
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
        Some(CoulombPsp { p, s, dp: 0.0, sinphi, cosphi: 1.0, dphi: 0.0, dshift: 0.0 })
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
pub fn getps(rho: f64, lll: i32, f: &[f64], fpr: &[f64], g: &[f64], gpr: &[f64], jdopha: bool, jdoder: bool, ishift: i32) -> CoulombPsp {
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
        let dphi = if jdoder { (gpr[n] * f[n] - g[n] * fpr[n]) / asq } else { 0.0 };
        (sinphi, cosphi, dphi)
    } else {
        (0.0, 0.0, 0.0)
    };
    CoulombPsp { p, s, dp, sinphi, cosphi, dphi, dshift: 0.0 }
}

/// Dispatcher between the three Coulomb evaluation strategies (large `eta`,
/// finite `eta` with large `rhoi`, finite `eta` with moderate `rhoi`) —
/// ported from `coulx` (`samm.f90:3340-4399`, sic — actually
/// `samm.f90:4340-4399`).
pub fn coulx(eta: f64, rho: f64, lll: i32, jdopha: bool, jdoder: bool, ishift: i32) -> Option<CoulombPsp> {
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

/// Compute Coulomb `P` (penetrability), `S` (shift factor), and `phi`
/// (hard-sphere-analogue phase shift), for channel angular momentum `lll`
/// — ported from `pspcou` (`samm.f90:3906-4001`). Dispatches to [`coulx`]
/// for `rho<1.02` (small-`rho` branch, matching upstream, using its own
/// separate approximation strategy) or [`coulfg`] otherwise, retrying at
/// `coulx` if `coulfg` fails (`ifail!=0`) exactly as upstream does. If a
/// shift-factor derivative is requested, both branches additionally
/// evaluate at `rho*1.01` to finite-difference `dshiftcoul`.
///
/// `ishift`: `>0` to compute the shift factor. Returns `None` only for the
/// upstream `lll<0` guard (`ifail=10` there); all other upstream failure
/// paths (`coulfg`/`bigeta` convergence failures) are logged and fall
/// through to a best-effort `coulx` retry, matching upstream's own
/// fallback behavior.
pub fn pspcou(rho: f64, lll: i32, eta: f64, ishift: i32, jdopha: bool, jdoder: bool) -> Option<CoulombPsp> {
    if lll < 0 {
        return None;
    }

    let llmax = lll + 2;

    let mut result = if rho < 1.02 {
        coulx(eta, rho, lll, jdopha, jdoder, ishift).unwrap_or(CoulombPsp { p: 0.0, s: 0.0, dp: 0.0, sinphi: 0.0, cosphi: 1.0, dphi: 0.0, dshift: 0.0 })
    } else {
        let cf = coulfg(rho, eta, lll, llmax, jdopha, jdoder, ishift);
        if cf.ifail != 0 {
            log::warn!("samm pspcou: coulfg failed (ifail={}), falling back to coulx (rho={rho}, eta={eta}, lll={lll})", cf.ifail);
            coulx(eta, rho, lll, jdopha, jdoder, ishift).unwrap_or(CoulombPsp { p: 0.0, s: 0.0, dp: 0.0, sinphi: 0.0, cosphi: 1.0, dphi: 0.0, dshift: 0.0 })
        } else {
            CoulombPsp { p: cf.pcoul, s: cf.scoul, dp: cf.dpcoul, sinphi: cf.sinphi, cosphi: cf.cosphi, dphi: cf.dphi, dshift: 0.0 }
        }
    };

    if ishift > 0 && jdoder {
        let rho2 = 1.01 * rho;
        let eta2 = eta * rho / rho2;
        let s2 = if rho2 < 1.02 {
            coulx(eta2, rho2, lll, jdopha, jdoder, ishift).map(|r| r.s)
        } else {
            let cf2 = coulfg(rho2, eta2, lll, llmax, jdopha, jdoder, ishift);
            if cf2.ifail == 0 {
                Some(cf2.scoul)
            } else {
                coulx(eta2, rho2, lll, jdopha, jdoder, ishift).map(|r| r.s)
            }
        };
        if let Some(s2) = s2 {
            result.dshift = (s2 - result.s) / (rho2 - rho);
        }
    }

    Some(result)
}

/// `G+iH` (the real/imaginary parts of `1/(S-B+iP)`), the penetrability
/// `P`, and their derivatives, for a **charged**-particle channel — the
/// Coulomb counterpart of [`super::penetrability::pgh`] — ported from
/// `pghcou` (`samm.f90:3823-3904`).
///
/// If `S-B+iP == 0` (the `iffy` condition upstream), `g=h=p=dp=ds=0` is
/// returned along with `iffy=true` rather than dividing by zero, matching
/// [`super::penetrability::Pgh`].
pub struct Pghcou {
    pub g: f64,
    pub h: f64,
    pub p: f64,
    pub dp: f64,
    pub ds: f64,
    pub iffy: bool,
}

pub fn pghcou(rho: f64, l: i32, bound: f64, ishift: i32, eta: f64, jdopha: bool) -> Option<Pghcou> {
    let jdoder = true;
    let psp = pspcou(rho, l, eta, ishift, jdopha, jdoder)?;

    let hh_raw = psp.p;
    let dp = psp.dp;
    let ds = if ishift > 0 { psp.dshift } else { 0.0 };
    let gg = if ishift > 0 { psp.s - bound } else { 0.0 };

    let hh = if hh_raw <= 1.0e-35 { 0.0 } else { hh_raw };

    if gg == 0.0 && hh == 0.0 {
        return Some(Pghcou { g: 0.0, h: 0.0, p: 0.0, dp: 0.0, ds: 0.0, iffy: true });
    }
    if hh == 0.0 {
        return Some(Pghcou { g: 1.0 / gg, h: 0.0, p: 0.0, dp, ds, iffy: false });
    }
    let p = hh;
    if gg == 0.0 {
        return Some(Pghcou { g: 0.0, h: -1.0 / hh, p, dp, ds, iffy: false });
    }
    if hh + gg == hh {
        return Some(Pghcou { g: (gg / hh) / hh, h: -1.0 / hh, p, dp, ds, iffy: false });
    }
    if hh + gg == gg {
        return Some(Pghcou { g: 1.0 / gg, h: -(hh / gg) / gg, p, dp, ds, iffy: false });
    }
    let d = hh * hh + gg * gg;
    Some(Pghcou { g: gg / d, h: -hh / d, p, dp, ds, iffy: false })
}
