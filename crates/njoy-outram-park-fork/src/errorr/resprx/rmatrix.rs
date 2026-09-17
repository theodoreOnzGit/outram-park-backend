// Ported from NJOY2016 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine ggrmat`, l.6227-6525 — Reich-Moore (`LRF=3`) cross sections at one
//     energy from the `b` work array `rpxlc12` builds, restricted to one `(L, J)`.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `ggrmat` — zero-temperature Reich-Moore cross sections for the ERRORJ
//! sensitivity loop, on the same work array [`super::mlbw::ggmlbw`] reads
//! (`(EL, EH, LRU, LRF, NRO, NAPS)`, `(SPI, AP, 0, 0, NLS, 0)`, then per L
//! the LIST head, `6·NRS` parameters `(ER, AJ, GN, GG, GFA, GFB)` and the
//! appended `(S_l, P_l, 0)` triples).
//!
//! Unlike `ggmlbw`, `ggrmat` evaluates **one** `(L, J)` at a time when the
//! caller passes `npnls = L` (1-based) and `valspi = |AJ|` — the ERRORJ
//! loop only needs the spin group the perturbed resonance belongs to; the
//! scattering-radius pass passes `npnls = 99` for every `(L, J)`.
//!
//! Upstream idioms kept: `gf` (a fission width was seen) is never reset,
//! so once set the 3×3 R-matrix path is used for every later `(L, J)` and
//! `L` with the fission rows at unity; the `L`-dependent `jjl` and the
//! `kkkkkk` channel-spin rule decide whether the hard-sphere term
//! `2 g_J (1 - cos 2φ)` is added for a `J` with no resonances of one
//! channel spin; `efrobns`'s singular case is an error here where
//! upstream degrades silently.

use crate::common::phys::{AMASSN_AMU, PI};
use crate::reconr::slbw::WAVE_K;
use crate::NjoyError;

use super::super::math::{efacphi, efacts, efrobns};
use super::mlbw::{RC1, RC2, THIRD};

/// `(σ_t, σ_el, σ_f, σ_γ)` \[b\] at `e` \[eV\] for the `(L = npnls, J =
/// valspi)` group of the work array `b` (`ggrmat`, `errorr.f90:6227-6525`),
/// or every group when `npnls == 99`. Also returns `arat = AWRI/(AWRI+1)`
/// (the upstream global side effect, as `ggmlbw`).
///
/// # Errors
/// `NotConvergent` from `efrobns` if the complex 3×3 R-matrix is singular
/// (upstream copies `A` through and continues).
pub fn ggrmat(e: f64, b: &[f64], npnls: usize, valspi: f64) -> Result<([f64; 4], f64), NjoyError> {
    const QUAR: f64 = 0.25;
    const HAF: f64 = 0.5;
    const SMALL: f64 = 3.0e-4;

    let mut sigp = [0.0f64; 4];
    // retrieve nuclide information (l.6259-6268)
    let naps = b[5].round() as i32;
    let awri = b[12];
    let ap = b[7];
    let aw = AMASSN_AMU * awri;
    let mut ra = RC1 * aw.powf(THIRD) + RC2;
    if naps == 1 {
        ra = ap;
    }
    let spi = b[6];
    let gjd = 2.0 * (2.0 * spi + 1.0);
    let nls = b[10].round() as usize;

    // wave number, rho and rho-cap at e (l.6271-6278)
    let arat = awri / (awri + 1.0);
    let k = WAVE_K * arat * e.abs().sqrt();
    let pifac = PI / (k * k);
    let mut gfa;
    let mut gfb;
    let mut gf = 0.0f64;
    let mut inow = 12usize;

    let mut nlsmax = nls;
    if npnls < nlsmax {
        nlsmax = npnls;
    }
    for l in 1..=nlsmax {
        let inowb = inow;
        let nrs = b[inow + 5].round() as usize;
        let ncyc = (b[inow + 4].round() as usize) / nrs.max(1);
        let ll = b[inow + 2].round() as i32;
        let apl = b[inow + 1];
        let mut rhoc = k * ap;
        let mut rho = k * ra;
        if apl != 0.0 {
            rhoc = k * apl;
        }
        if apl != 0.0 && naps == 1 {
            rho = k * apl;
        }

        // shift and penetration factors at the cross-section energy (l.6300-6306)
        let (_se, pe) = efacts(ll, rho);
        let phi = efacphi(ll, rhoc);
        let phid = phi;
        let p1 = (2.0 * phid).cos();
        let p2 = (2.0 * phid).sin();

        // possible j values (l.6309-6318)
        let fl = f64::from(ll);
        let ajmin = ((spi - fl).abs() - HAF).abs();
        let ajmax = spi + fl + HAF;
        let numj = (ajmax - ajmin + 1.0).round() as i64;
        let mut ajc = ajmin - 1.0;
        let jjl: i64 = if ll != 0 && (fl > spi - HAF && fl <= spi) {
            0
        } else {
            1
        };
        let mut in_ = inow;
        for jj in 1..=numj {
            inow = inowb;
            ajc += 1.0;
            if ((ajc - valspi).abs() > 0.01 || l != npnls) && npnls != 99 {
                in_ = (inow + 6) + nrs * 6 + nrs * 3;
                continue;
            }
            let gj = (2.0 * ajc + 1.0) / gjd;

            // loop over possible channel spins (l.6329-6333)
            let mut kchanl = 0;
            let idone = 0;
            while kchanl < 2 && idone == 0 {
                kchanl += 1;
                inow = inowb;
                let mut kpstv = 0;
                let mut kngtv = 0;
                let mut s = [[0.0f64; 3]; 3];
                let mut r = [[0.0f64; 3]; 3];

                // loop over resonances (l.6345-6398)
                inow += 6;
                in_ = inow + nrs * 6;
                for _i in 0..nrs {
                    let aj = b[inow + 1].abs();
                    if (aj - ajc).abs() <= QUAR {
                        if b[inow + 1] < 0.0 {
                            kngtv += 1;
                        }
                        if b[inow + 1] > 0.0 {
                            kpstv += 1;
                        }
                        let mut iskip = false;
                        if kchanl == 1 && b[inow + 1] < 0.0 {
                            iskip = true;
                        }
                        if kchanl == 2 && b[inow + 1] > 0.0 {
                            iskip = true;
                        }
                        if !iskip {
                            let er = b[inow];
                            let gn = b[inow + 2];
                            let gg = b[inow + 3];
                            gfa = b[inow + 4];
                            gfb = b[inow + 5];
                            let per = b[in_ + 1];
                            let a1 = (gn * pe / per).sqrt();
                            let mut a2 = 0.0;
                            if gfa != 0.0 {
                                a2 = gfa.abs().sqrt();
                            }
                            if gfa < 0.0 {
                                a2 = -a2;
                            }
                            let mut a3 = 0.0;
                            if gfb != 0.0 {
                                a3 = gfb.abs().sqrt();
                            }
                            if gfb < 0.0 {
                                a3 = -a3;
                            }
                            let diff = er - e;
                            let den = diff * diff + QUAR * gg * gg;
                            let de2 = HAF * diff / den;
                            let gg4 = QUAR * gg / den;
                            r[0][0] += gg4 * a1 * a1;
                            s[0][0] -= de2 * a1 * a1;
                            if gfa != 0.0 || gfb != 0.0 {
                                r[0][1] += gg4 * a1 * a2;
                                s[0][1] -= de2 * a1 * a2;
                                r[0][2] += gg4 * a1 * a3;
                                s[0][2] -= de2 * a1 * a3;
                                r[1][1] += gg4 * a2 * a2;
                                s[1][1] -= de2 * a2 * a2;
                                r[2][2] += gg4 * a3 * a3;
                                s[2][2] -= de2 * a3 * a3;
                                r[1][2] += gg4 * a2 * a3;
                                s[1][2] -= de2 * a2 * a3;
                                gf = 1.0;
                            }
                        }
                    }
                    inow += ncyc;
                    in_ += 3;
                }

                // channel spin per the sign of aj (l.6400-6443)
                let mut kkkkkk = 0;
                let interior = jj > jjl && jj < numj;
                if kchanl == 1 {
                    if kpstv > 0 {
                        if kngtv == 0 {
                            kkkkkk = if interior { 2 } else { 1 };
                        } else {
                            kkkkkk = 1;
                        }
                    } else if kngtv == 0 {
                        kkkkkk = if interior { 2 } else { 1 };
                    } else {
                        kkkkkk = 0;
                    }
                } else if kpstv > 0 {
                    if kngtv > 0 {
                        kkkkkk = 1;
                    }
                } else if kngtv > 0 {
                    kkkkkk = if interior { 2 } else { 1 };
                }
                if kkkkkk != 0 {
                    let (termf, termt, termn);
                    if gf != 0.0 {
                        // r-matrix path -- make symmetric matrix (l.6447-6470)
                        r[0][0] += 1.0;
                        r[1][1] += 1.0;
                        r[2][2] += 1.0;
                        r[1][0] = r[0][1];
                        s[1][0] = s[0][1];
                        r[2][0] = r[0][2];
                        s[2][0] = s[0][2];
                        r[2][1] = r[1][2];
                        s[2][1] = s[1][2];
                        let (ri, si) = efrobns(r, s)?;
                        let t1 = ri[0][1];
                        let t2 = si[0][1];
                        let t3 = ri[0][2];
                        let t4 = si[0][2];
                        termf = 4.0 * gj * (t1 * t1 + t2 * t2 + t3 * t3 + t4 * t4);
                        let u11r = p1 * (2.0 * ri[0][0] - 1.0) + 2.0 * p2 * si[0][0];
                        let u11i = p2 * (1.0 - 2.0 * ri[0][0]) + 2.0 * p1 * si[0][0];
                        termt = 2.0 * gj * (1.0 - u11r);
                        termn = gj * ((1.0 - u11r).powi(2) + u11i * u11i);
                    } else {
                        // r-function path (l.6473-6495)
                        let dd = r[0][0];
                        let rr = 1.0 + dd;
                        let ss = s[0][0];
                        let amag = rr * rr + ss * ss;
                        let rri = rr / amag;
                        let ssi = -ss / amag;
                        let uur = p1 * (2.0 * rri - 1.0) + 2.0 * p2 * ssi;
                        let uui = p2 * (1.0 - 2.0 * rri) + 2.0 * p1 * ssi;
                        if dd.abs() < SMALL && phid.abs() < SMALL {
                            let mut xx = 2.0 * dd;
                            xx += 2.0 * (dd * dd + ss * ss + phid * phid + p2 * ss);
                            xx -= 2.0 * phid * phid * (dd * dd + ss * ss);
                            xx /= amag;
                            termt = 2.0 * gj * xx;
                            termn = gj * (xx * xx + uui * uui);
                        } else {
                            termt = 2.0 * gj * (1.0 - uur);
                            termn = gj * ((1.0 - uur).powi(2) + uui * uui);
                        }
                        termf = 0.0;
                    }
                    let (mut termn, mut termt) = (termn, termt);
                    if kkkkkk == 2 {
                        termn += 2.0 * gj * (1.0 - p1);
                        termt += 2.0 * gj * (1.0 - p1);
                    }
                    let termg = termt - termf - termn;
                    sigp[1] += termn;
                    sigp[3] += termg;
                    sigp[2] += termf;
                    sigp[0] += termt;
                }
            }
        }
        inow = in_;
    }

    for v in sigp.iter_mut() {
        *v *= pifac;
    }
    Ok((sigp, arat))
}
