//! Steed's method Coulomb wave-function core — `jwkb` (semiclassical
//! approximation) and `coulfg` (the CPC "COULFG" algorithm), ported from
//! `samm.f90:4297-4338` and `samm.f90:4003-4295`.

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
/// matching [`crate::samm::mf2::ParticlePair::shift_flag`]).
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
