// Ported from NJOY2016 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine rpxlc2`,  l.4634-4783 — LCOMP=2 compact covariance: uncertainties into the
//     diagonal, parameters repacked 6 per resonance, INTG correlations, variances.
//   - `subroutine rpxlc12`, l.4108-4632 — LCOMP=1/2 resolved ranges: scattering-radius
//     sensitivity, central-difference sensitivities per resonance parameter, propagation
//     into the `cff/cgg/cee/ctt` and `cef/ceg/cfg` accumulators.
//   - `subroutine rpendf`,  l.5015-5089 — pointwise cross sections on the ERRORJ energy
//     grid (`eskip` stepping, resonance-centred refinement).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Resolved resonance ranges (`LRU=1`) with the ERRORJ method: the group
//! sensitivities `∂σ_g/∂p` of total/elastic/fission/capture to every
//! resonance parameter `p` by central finite differences of the MLBW
//! cross sections (`ER` perturbed by ±0.01 %, widths by ±1 %), folded with
//! the parameter covariance matrix.
//!
//! **Scope.** `LRF=2` (MLBW) with `LCOMP=2` is what the Ar-37 oracle
//! exercises (`tests/errorr_mf32_ar37_golden.rs`); `LRF=3` (Reich-Moore,
//! [`super::rmatrix::ggrmat`]) with `LCOMP=1` is what the JENDL-3.3 U-238
//! oracle exercises (`tests/errorr_mf32_j33u238_lrf3_golden.rs`). Refused
//! with `NotPorted`: `LRF=1` (upstream's `rpendf` has no SLBW branch and
//! would return stale `sig1`), `LCOMP=0` (`rpxlc0`), `NLRS > 0`, and
//! `NM > 0` INTG correlation records in `rpxlc2` (the SAMM branch reads
//! them from the tape's raw text; this branch does not yet).
//!
//! **Upstream idioms kept deliberately** (the oracle depends on them):
//! - for `LRF=1/2` the scattering-radius perturbation writes `AP + DAP`
//!   into each L-block's `QX` slot (`b(inow+1)`, upstream treats it as
//!   `APL`) and "restores" it as `(AP+DAP)/(1+DAP)` — harmless for `LRX=0`
//!   but reproduced verbatim (`errorr.f90:4277-4285, 4338-4344`);
//! - `gwidth = ER*1e-4` keeps the sign of a negative `ER`, so the central
//!   difference is taken in the same direction the perturbation moved;
//! - `arat` is a module global that `ggmlbw` overwrites from the MF=2
//!   `AWRI` on every call; later `rho` evaluations read that value.
//!
//! **Deliberate divergence.** Upstream's resonance search
//! (`errorr.f90:4363-4380`) advances its MF=2 pointer only inside
//! `if (ipara.ne.0)`, so an L-block with zero resonances (Ar-37 `L=1`)
//! leaves it stale and every later L-block is never scanned — NJOY2016
//! aborts with `rpxlc12 ... problem` on the unmodified TENDL-2023 Ar-37
//! tape. This port advances the pointer for an empty block too; the
//! oracle test shows the port on the original tape equals NJOY on a tape
//! with the empty block moved last.

use crate::common::phys::AMASSN_AMU;
use crate::endf::records::SectionCursor;
use crate::mixr::mix::sigfig;
use crate::reconr::slbw::WAVE_K;
use crate::NjoyError;

use super::super::math::efacts;
use super::super::weight::ErrorrWeight;
use super::group::{rpxgrp, SigRow};
use super::mf2::{build_work_array, Mf2Range};
use super::mlbw::{ggmlbw, RC1, RC2, THIRD};
use super::{Eskip, RangeParams, ResonanceCovariance};

/// What `rpxlc2` leaves in the module globals and work arrays.
#[derive(Debug, Clone)]
pub struct Lcomp2Params {
    /// `AWRI` of the compact LIST.
    pub awri: f64,
    /// `nrb` — resonances (`NRSA`).
    pub nrb: usize,
    /// `mpar = NNN / nrb` — parameters per resonance.
    pub mpar: usize,
    /// `npar = nrb * mpar`.
    pub npar: usize,
    /// The resonance parameters repacked 6 per resonance `(ER, AJ, GT, GN,
    /// GG, GF)` (BW) — the leading `6*nrb` words of upstream's `a(lbg+6..)`.
    pub params: Vec<f64>,
    /// `cov(i, j)` as `[i][j]`, `npar × npar` (variances on the diagonal).
    pub cov: Vec<Vec<f64>>,
}

/// Read one `LCOMP=2` compact subsection (`rpxlc2`, `errorr.f90:4634-4783`).
///
/// # Errors
/// `NotPorted` for `LRF` other than 1–3 or `NM > 0`; `EndfParse` for an
/// illegal `NDIGIT`.
pub fn rpxlc2(cur: &mut SectionCursor, lrf: i32) -> Result<Lcomp2Params, NjoyError> {
    // mpid — which of the 12 words per resonance carry the uncertainties
    let mpid: [usize; 6] = match lrf {
        1 | 2 => [1, 4, 5, 6, 0, 0],
        3 => [1, 3, 4, 5, 6, 0],
        _ => {
            return Err(NjoyError::NotPorted(
                "errorr::rpxlc2: not ready for this lrf with lcomp=2",
            ))
        }
    };
    let list = cur.read_list()?;
    let awri = list.head.c1;
    let nrb = list.head.n2 as usize;
    let a = &list.data;
    if a.len() < 12 * nrb {
        return Err(NjoyError::EndfParse(
            "errorr::rpxlc2: compact LIST shorter than 12*NRSA".into(),
        ));
    }
    // uncertainties of the selected parameters into the diagonal (l.4683-4691):
    // cov(mm,mm) = a(l3 + mpid(m) - 1) with l3 = lbg + 12*n2, i.e. word
    // 12*(n2-1) + 6 + mpid(m) - 1 of the data (mpid = 0 reads the resonance's GF).
    let mut diag = Vec::with_capacity(5 * nrb);
    for n2 in 0..nrb {
        for &m in mpid.iter().take(5) {
            let idx = 12 * n2 + 6 + m;
            diag.push(a[idx - 1]);
        }
    }
    // repack the parameters without uncertainties (l.4695-4703)
    let mut params = Vec::with_capacity(6 * nrb);
    for n2 in 0..nrb {
        params.extend_from_slice(&a[12 * n2..12 * n2 + 6]);
    }
    // the compact correlation matrix header (l.4707-4722)
    let c = cur.read_cont()?;
    let mut ndigit = c.l1;
    if !(2..=6).contains(&ndigit) {
        if ndigit == 0 {
            ndigit = 2;
        } else {
            return Err(NjoyError::EndfParse(
                "errorr::rpxlc2: illegal value of ndigit".into(),
            ));
        }
    }
    let _ = ndigit;
    let nnn = c.l2 as usize;
    let nm = c.n1;
    let mpar = nnn / nrb;
    let mut cov_diag = vec![0.0f64; nrb * mpar];
    if mpar != 5 {
        for nr in 0..nrb {
            for m in 0..mpar {
                cov_diag[nr * mpar + m] = diag[5 * nr + m];
            }
        }
    } else {
        cov_diag.copy_from_slice(&diag);
    }
    if nm != 0 {
        return Err(NjoyError::NotPorted(
            "errorr::rpxlc2: INTG correlation records (NM > 0) — the tape reader stores 6 floats per line",
        ));
    }
    // square the diagonal terms to get the variance (l.4771-4775)
    let npar = nrb * mpar;
    let mut cov = vec![vec![0.0f64; npar]; npar];
    for (n1, d) in cov_diag.iter().enumerate() {
        cov[n1][n1] = d * d;
    }
    Ok(Lcomp2Params {
        awri,
        nrb,
        mpar,
        npar,
        params,
        cov,
    })
}

/// Pointwise cross sections on the ERRORJ grid (`rpendf`,
/// `errorr.f90:5015-5089`): from `elg` to `ehg`, stepping by `eskip`
/// according to the distance from `eres` (a negative `eres` — a bound
/// level or the scattering-radius pass — uses the fine uniform step),
/// every energy rounded to 8 figures, `elr`/`ehr` inserted, zero outside
/// `[elr, ehr]`.
///
/// `arat` is updated from the MF=2 `AWRI` whenever `ggmlbw` runs (the
/// upstream global side effect).
///
/// `npnls`/`valspi` select the `(L, J)` group `ggrmat` evaluates for an
/// `LRF=3` range (`npnls = 99`: every group); `ggmlbw` ignores them.
pub fn rpendf(
    b: &[f64],
    rp: &RangeParams,
    eskip: &Eskip,
    eres: f64,
    arat: &mut f64,
    npnls: usize,
    valspi: f64,
) -> Result<Vec<SigRow>, NjoyError> {
    let mut sig: Vec<SigRow> = Vec::new();
    let mut e1 = rp.elg;
    let elb1 = 0.9 * eres;
    let elu1 = 1.1 * eres;
    let elb2 = 0.8 * eres;
    let elu2 = 1.2 * eres;
    let elb3 = 0.7 * eres;
    let elu3 = 1.3 * eres;
    loop {
        e1 = sigfig(e1, 8, 0);
        if e1 > rp.ehg {
            e1 = rp.ehg;
        }
        let sig1 = if e1 >= rp.elr && e1 <= rp.ehr {
            match rp.lrf {
                2 => {
                    let (s, ar) = ggmlbw(e1, b);
                    *arat = ar;
                    s
                }
                3 => {
                    let (s, ar) = super::rmatrix::ggrmat(e1, b, npnls, valspi)?;
                    *arat = ar;
                    s
                }
                _ => {
                    return Err(NjoyError::NotPorted(
                        "errorr::rpendf: no cross-section branch for this lrf (upstream returns stale sig1)",
                    ))
                }
            }
        } else {
            [0.0; 4]
        };
        sig.push([sig1[0], sig1[1], sig1[2], sig1[3], e1]);
        if e1 < rp.ehg {
            let e2 = if eres < 0.0 {
                1.0 + 5.0 * (eskip.e1 - 1.0)
            } else if e1 < 0.1 {
                eskip.e4
            } else if e1 > elb1 && e1 < elu1 {
                eskip.e1
            } else if e1 > elb2 && e1 < elu2 {
                eskip.e2
            } else if e1 > elb3 && e1 < elu3 {
                eskip.e3
            } else {
                1.02
            };
            let ebc = e1;
            e1 *= e2;
            if ebc < rp.elr && e1 > rp.elr {
                e1 = rp.elr;
            }
            if ebc < rp.ehr && e1 > rp.ehr {
                e1 = rp.ehr;
            }
        } else {
            break;
        }
    }
    Ok(sig)
}

/// One covariance "section" of a resolved range: parameters and their
/// covariance (`LCOMP=1` LIST or the `LCOMP=2` compact block).
struct CovSection {
    mpar: usize,
    nrb: usize,
    npar: usize,
    params: Vec<f64>,
    cov: Vec<Vec<f64>>,
}

/// Resolved resonances, `LCOMP=1` or `2` (`rpxlc12`, `errorr.f90:4108-4632`).
///
/// `cur` stands at the record after the `(SPI, AP, 0, LCOMP, NLS, ISR)`
/// CONT (and the `DAP` record when `ISR=1`).
///
/// # Errors
/// See the module docs for the refused formats.
#[allow(clippy::too_many_arguments, clippy::needless_range_loop)]
pub fn rpxlc12(
    cur: &mut SectionCursor,
    rp: &RangeParams,
    mf2r: &Mf2Range,
    egn: &[f64],
    cflx: &[f64],
    weight: &ErrorrWeight,
    tempin: f64,
    eskip: &Eskip,
    rc: &mut ResonanceCovariance,
) -> Result<(), NjoyError> {
    let ngn = egn.len() - 1;
    let (iest, ieed) = (rp.iest, rp.ieed);
    let lrf = rp.lrf;
    if lrf <= 0 || lrf > 3 {
        return Err(NjoyError::EndfParse(format!(
            "errorr::rpxlc12: lrf={lrf} is not coded"
        )));
    }
    if lrf == 1 {
        return Err(NjoyError::NotPorted(
            "errorr::rpxlc12: lrf=1 — upstream rpendf has no SLBW branch (stale sig1)",
        ));
    }
    // general (lcomp=1) head, or the compact (lcomp=2) block (l.4157-4169)
    let mut arat;
    let ral0;
    let apl0;
    let mut nlrs = 0;
    let mut sections: Vec<CovSection> = Vec::new();
    let mut lcomp1_lists = Vec::new();
    if rp.lcomp == 1 {
        let c = cur.read_cont()?;
        let awri = c.c1;
        let nsrs = c.n1;
        nlrs = c.n2;
        if nsrs <= 0 {
            // label 600: nothing to do for this range
            if nlrs > 0 {
                return Err(NjoyError::NotPorted("errorr::rpxlc12: nlrs>0 not coded"));
            }
            rc.ifresr = true;
            return Ok(());
        }
        arat = awri / (awri + 1.0);
        let aw = AMASSN_AMU * awri;
        let ra = RC1 * aw.powf(THIRD) + RC2;
        ral0 = ra;
        apl0 = rp.ap;
        for _ in 0..nsrs {
            lcomp1_lists.push(cur.read_list()?);
        }
        for l in &lcomp1_lists {
            let mpar = l.head.l1 as usize;
            let nrb = l.head.n2 as usize;
            let npar = mpar * nrb;
            let params = l.data[..6 * nrb].to_vec();
            // cov(i,j) from the packed upper triangle after the parameters (l.4553-4562)
            let mut cov = vec![vec![0.0f64; npar]; npar];
            let mut l3 = 6 * nrb;
            for i in 0..npar {
                for j in i..npar {
                    let tmp = *l.data.get(l3).ok_or_else(|| {
                        NjoyError::EndfParse("errorr::rpxlc12: lcomp=1 LIST too short".into())
                    })?;
                    l3 += 1;
                    cov[i][j] = tmp;
                    cov[j][i] = tmp;
                }
            }
            sections.push(CovSection {
                mpar,
                nrb,
                npar,
                params,
                cov,
            });
        }
    } else {
        let lc2 = rpxlc2(cur, lrf)?;
        arat = lc2.awri / (lc2.awri + 1.0);
        let aw = AMASSN_AMU * lc2.awri;
        let ra = RC1 * aw.powf(THIRD) + RC2;
        ral0 = ra;
        apl0 = rp.ap;
        sections.push(CovSection {
            mpar: lc2.mpar,
            nrb: lc2.nrb,
            npar: lc2.npar,
            params: lc2.params,
            cov: lc2.cov,
        });
    }

    // write MF=2 data on the b array (l.4171-4222)
    let wa = build_work_array(mf2r, rp.lru, lrf, rp.naps, rp.ap, arat, ral0, apl0)?;
    let mut b = wa.b;
    let llmat = wa.llmat;
    let ral = wa.ral;
    let nls1 = llmat.len();
    let lb = 12usize; // first L-state head (Fortran lb = 13)

    let abn = rp.abn;
    let mut imess = false;
    for sec in &sections {
        let (mpar, nrb, npar) = (sec.mpar, sec.nrb, sec.npar);
        if mpar > 4 && lrf <= 2 {
            return Err(NjoyError::EndfParse(
                "errorr::rpxlc12: mpar.gt.4.and.lrf.le.2 not coded".into(),
            ));
        }
        if !imess {
            imess = true;
            rc.messages.push(if rp.isr == 0 {
                "rpxlc12: no scattering radius uncertainty".to_string()
            } else {
                "rpxlc12: include scattering radius uncertainty".to_string()
            });
        }

        // --- scattering radius uncertainty (l.4265-4346) ---
        if rp.isr == 1 {
            let dap = rp.dap;
            let sigr = rpendf(&b, rp, eskip, -1.0, &mut arat, 99, 0.0)?;
            // perturb ap and the penetration factors
            let ap_p = b[7] + dap;
            b[7] = ap_p;
            let nls = b[10].round() as usize;
            let mut inow = 12usize;
            let mut pneorg: Vec<f64> = Vec::new();
            for lll in 0..nls {
                let mut apl = b[inow + 1];
                if apl == 0.0 {
                    apl = ap_p;
                } else {
                    apl += rp.dap3[lll];
                }
                b[inow + 1] = apl;
                let ll = b[inow + 2].round() as i32;
                let nrs = b[inow + 5].round() as usize;
                inow += 6;
                for jj in 0..nrs {
                    let rho = WAVE_K * arat * b[inow + 6 * jj].abs().sqrt() * apl;
                    let (ser, per) = efacts(ll, rho);
                    pneorg.push(b[inow + 6 * nrs + 3 * jj]);
                    pneorg.push(b[inow + 6 * nrs + 3 * jj + 1]);
                    b[inow + 6 * nrs + 3 * jj] = ser;
                    b[inow + 6 * nrs + 3 * jj + 1] = per;
                }
                inow += 9 * nrs;
            }
            let mut sigp = rpendf(&b, rp, eskip, -1.0, &mut arat, 99, 0.0)?;
            // sensitivity to ap
            for (p, r) in sigp.iter_mut().zip(&sigr) {
                for j in 0..4 {
                    p[j] = (p[j] - r[j]) / dap;
                }
            }
            let mut gsig = rpxgrp(egn, &sigp, weight, tempin)?;
            for (g, &f) in gsig.iter_mut().zip(cflx) {
                let tmp = f * abn;
                for v in g.iter_mut() {
                    *v *= tmp;
                }
            }
            let dap2 = dap * dap;
            // error propagation (l.4310-4331)
            let mut igind = 0usize;
            for ig in 0..ngn {
                for ig2 in ig..ngn {
                    rc.cff[igind] += dap2 * gsig[ig][2] * gsig[ig2][2];
                    rc.cgg[igind] += dap2 * gsig[ig][3] * gsig[ig2][3];
                    rc.cee[igind] += dap2 * gsig[ig][1] * gsig[ig2][1];
                    rc.ctt[igind] += dap2 * gsig[ig][0] * gsig[ig2][0];
                    igind += 1;
                }
            }
            let mut igind = 0usize;
            for ig in 0..ngn {
                for ig2 in 0..ngn {
                    rc.cef[igind] += dap2 * gsig[ig][1] * gsig[ig2][2];
                    rc.ceg[igind] += dap2 * gsig[ig][1] * gsig[ig2][3];
                    rc.cfg[igind] += dap2 * gsig[ig][2] * gsig[ig2][3];
                    igind += 1;
                }
            }
            // restore reference data (l.4334-4346) — the (AP+DAP)/(1+DAP) idiom
            b[7] -= dap;
            let mut inow = 12usize;
            let mut itmp = 0usize;
            for lll in 0..nls {
                let apl = b[inow + 1] / (1.0 + rp.dap3[lll]);
                b[inow + 1] = apl;
                let nrs = b[inow + 5].round() as usize;
                inow += 6;
                for jj in 0..nrs {
                    b[inow + 6 * nrs + 3 * jj] = pneorg[itmp];
                    b[inow + 6 * nrs + 3 * jj + 1] = pneorg[itmp + 1];
                    itmp += 2;
                }
                inow += 9 * nrs;
            }
        }

        // --- sensitivities to every resonance parameter (l.4349-4552) ---
        // sens[loop][ig] (both 1-based) = (total, elastic, fission, capture)
        let mut sens = vec![[0.0f64; 4]; (npar + 1) * (ngn + 1)];
        let mut loop_ = 0usize;
        let mut il2 = lb;
        let mut ipos = 0usize;
        let mut ilnum = 0usize;
        // |ajres| of the resonance being perturbed (errorr.f90:4383), the
        // `valspi` ggrmat selects the (L, J) group with
        let mut ajres_abs = 0.0f64;
        for loopm in 1..=nrb {
            let eres = sec.params[6 * (loopm - 1)];
            for loopn in 1..=mpar {
                loop_ += 1;
                if loopn == 1 {
                    // search the MF=32 resonance in MF=2 (l.4356-4384)
                    let ajres = sec.params[6 * (loopm - 1) + 1];
                    il2 = lb;
                    let mut found = false;
                    'search: for il in 0..nls1 {
                        let itmp = il2;
                        let ipara = b[il2 + 5].round() as usize;
                        if ipara != 0 {
                            for ipp in 1..=ipara {
                                il2 += 6;
                                let eres2 = b[il2];
                                let ajres2 = b[il2 + 1];
                                if eres * eres2 > 0.0 {
                                    let rr = (eres / eres2 - 1.0).abs();
                                    let rr2 = (ajres - ajres2).abs();
                                    if rr < 1.0e-6 && rr2 < 1.0e-4 {
                                        ipos = itmp + 6 + ipara * 6 + (ipp - 1) * 3;
                                        ilnum = il;
                                        found = true;
                                        break 'search;
                                    }
                                }
                            }
                            il2 += 6;
                            il2 += ipara * 3;
                        } else {
                            // upstream leaves il2 stale here (errorr.f90:4363-4380)
                            il2 += 6;
                        }
                    }
                    if !found {
                        return Err(NjoyError::EndfParse(format!(
                            "errorr::rpxlc12: problem — MF=32 resonance E={eres:e} ajres={ajres:e} not found in MF=2"
                        )));
                    }
                    ajres_abs = ajres.abs();
                }

                // perturbed(-) (l.4389-4415)
                let il3;
                let backdt;
                let gwidth;
                let mut backdt2 = 0.0;
                let mut backdt3 = 0.0;
                if loopn == 1 {
                    il3 = il2;
                    backdt = b[il2];
                    b[il2] = backdt * 0.9999;
                    gwidth = backdt * 0.0001;
                    backdt2 = b[ipos];
                    backdt3 = b[ipos + 1];
                    let rho = WAVE_K * arat * b[il2].abs().sqrt() * ral;
                    let (ser, per) = efacts(llmat[ilnum], rho);
                    b[ipos] = ser;
                    b[ipos + 1] = per;
                } else {
                    il3 = if lrf <= 2 {
                        il2 + loopn + 1
                    } else {
                        il2 + loopn
                    };
                    backdt = b[il3];
                    gwidth = backdt * 0.01;
                    b[il3] = backdt * 0.99;
                }
                let sigr = if gwidth != 0.0 {
                    Some(rpendf(&b, rp, eskip, eres, &mut arat, ilnum + 1, ajres_abs)?)
                } else {
                    None
                };
                b[il3] = backdt;
                if loopn == 1 {
                    b[ipos] = backdt2;
                    b[ipos + 1] = backdt3;
                }

                // perturbed(+) (l.4424-4444)
                if loopn == 1 {
                    b[il2] = backdt * 1.0001;
                    let rho = WAVE_K * arat * b[il2].abs().sqrt() * ral;
                    let (ser, per) = efacts(llmat[ilnum], rho);
                    b[ipos] = ser;
                    b[ipos + 1] = per;
                } else {
                    b[il3] = backdt * 1.01;
                }
                let sigp = if gwidth != 0.0 {
                    Some(rpendf(&b, rp, eskip, eres, &mut arat, ilnum + 1, ajres_abs)?)
                } else {
                    None
                };
                b[il3] = backdt;
                if loopn == 1 {
                    b[ipos] = backdt2;
                    b[ipos + 1] = backdt3;
                }

                if let (Some(sigr), Some(mut sigp)) = (sigr, sigp) {
                    // differencing and integration (l.4447-4463)
                    for (p, r) in sigp.iter_mut().zip(&sigr) {
                        for j in 0..4 {
                            p[j] = (p[j] - r[j]) / (gwidth * 2.0);
                        }
                    }
                    let gsig = rpxgrp(egn, &sigp, weight, tempin)?;
                    for ig in iest..=ieed {
                        let tmp = cflx[ig - 1] * abn;
                        let s = &mut sens[loop_ * (ngn + 1) + ig];
                        for j in 0..4 {
                            s[j] = gsig[ig - 1][j] * tmp;
                        }
                    }
                } else {
                    for ig in iest..=ieed {
                        sens[loop_ * (ngn + 1) + ig] = [0.0; 4];
                    }
                }
            }
        }
        rc.messages
            .push("rpxlc12: resonance parameter loop done".to_string());

        // --- propagate into the accumulators (l.4567-4626) ---
        let cov = &sec.cov;
        let in_range = |g: usize| g >= iest && g <= ieed;
        let s = |i: usize, ig: usize| sens[i * (ngn + 1) + ig];
        let mut igind = 0usize;
        for ig in 1..=ieed {
            for ig2 in ig..=ngn {
                if in_range(ig) && in_range(ig2) {
                    for i in 1..=npar {
                        for j in i..=npar {
                            let tmp = cov[i - 1][j - 1];
                            if tmp != 0.0 {
                                rc.cff[igind] += tmp * s(i, ig)[2] * s(j, ig2)[2];
                                rc.cgg[igind] += tmp * s(i, ig)[3] * s(j, ig2)[3];
                                rc.cee[igind] += tmp * s(i, ig)[1] * s(j, ig2)[1];
                                rc.ctt[igind] += tmp * s(i, ig)[0] * s(j, ig2)[0];
                                if i != j {
                                    rc.cff[igind] += tmp * s(j, ig)[2] * s(i, ig2)[2];
                                    rc.cgg[igind] += tmp * s(j, ig)[3] * s(i, ig2)[3];
                                    rc.cee[igind] += tmp * s(j, ig)[1] * s(i, ig2)[1];
                                    rc.ctt[igind] += tmp * s(j, ig)[0] * s(i, ig2)[0];
                                }
                            }
                        }
                    }
                }
                igind += 1;
            }
            if ig > rc.nresg {
                rc.nresg = ig;
            }
        }
        let mut igind = 0usize;
        for ig in 1..=ieed {
            for ig2 in 1..=ngn {
                if in_range(ig) && in_range(ig2) {
                    for i in 1..=npar {
                        for j in i..=npar {
                            let tmp = cov[i - 1][j - 1];
                            if tmp != 0.0 {
                                rc.cef[igind] += tmp * s(i, ig)[1] * s(j, ig2)[2];
                                rc.ceg[igind] += tmp * s(i, ig)[1] * s(j, ig2)[3];
                                rc.cfg[igind] += tmp * s(i, ig)[2] * s(j, ig2)[3];
                                if i != j {
                                    rc.cef[igind] += tmp * s(j, ig)[1] * s(i, ig2)[2];
                                    rc.ceg[igind] += tmp * s(j, ig)[1] * s(i, ig2)[3];
                                    rc.cfg[igind] += tmp * s(j, ig)[2] * s(i, ig2)[3];
                                }
                            }
                        }
                    }
                }
                igind += 1;
            }
        }
        rc.messages
            .push("rpxlc12: sensitivity calculation completed".to_string());
    }

    // label 600 (l.4600-4620)
    if nlrs > 0 {
        return Err(NjoyError::NotPorted("errorr::rpxlc12: nlrs>0 not coded"));
    }
    rc.ifresr = true;
    Ok(())
}
