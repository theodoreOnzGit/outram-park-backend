// Ported from NJOY2016 `src/wimsr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine p1scat`, l.1680-1917 — the P1 scattering matrix per temperature.
//   - `subroutine p1sout`, l.1919-1969 — its packed `(ig, l1, l2, values)` records.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `p1scat` — the WIMS P1 scattering matrix (`ip1opt = 0`): the
//! temperature-independent `MF=6` matrices (every section but elastic,
//! fission, the thermal `MT`s and 221–250) collected once from the first
//! temperature block, plus per temperature the elastic (above the thermal
//! boundary), `mti` and `mtc` matrices, all at the most-shielded sigma-zero
//! (or the reference one when `sgref` selects it); then upstream's
//! upscatter suppression into the first thermal group.
//!
//! Upstream idioms kept: the thermal boundary `nth` starts at
//! `ngnd - nfg - nrg` and is moved to the highest `mti`/`mtc` group seen
//! in the first pass; `l1`/`l2` are widened from `ig2lo`/`ng2` with the
//! `l2t .ne. l1t` guard; a temperature block without an `MT=2` section
//! would add an uninitialised `elas` upstream — here zeros.

use crate::NjoyError;

use super::gendf::GendfMaterial;
use super::input::WimsrInput;
use super::xsecs::{Counts, XsecsResult};

/// One temperature's P1 matrix as `p1sout` packs it: per WIMS group
/// `(ig, l1, l2, sloc(ig, l1..=l2))`.
#[derive(Debug, Clone, Default)]
pub struct P1Temp {
    pub temp: f64,
    /// `(ig, l1, l2, values)` for `ig = 1..=ngnd`.
    pub rows: Vec<(usize, usize, usize, Vec<f64>)>,
}

/// `p1scat` (`wimsr.f90:1680-1917`). Returns `None` when `ip1opt == 1`.
///
/// # Errors
/// "no p1 matrices found" / "no temperature-dependent reactions".
#[allow(clippy::needless_range_loop)] // Fortran index loops kept verbatim
pub fn p1scat(
    inp: &WimsrInput,
    mat: &GendfMaterial,
    cnt: &Counts,
    xs: &XsecsResult,
) -> Result<Option<Vec<P1Temp>>, NjoyError> {
    if inp.ip1opt == 1 {
        return Ok(None);
    }
    let ngnd = inp.ngnd;
    let nfg = inp.nfg;
    let nrg = inp.nrg;
    let nther = ngnd - nfg - nrg;
    let il = 2i64;
    let isg = xs.isg;
    let ntemp = cnt.ntemp;
    let mti = inp.mti;
    let mtc = inp.mtc;

    let d = |data: &[f64], off: i64| -> f64 {
        if off < 0 {
            0.0
        } else {
            data.get(off as usize).copied().unwrap_or(0.0)
        }
    };

    // --- first pass (jtemp = 0): the temperature-independent matrix
    let mut sloc0 = vec![0.0f64; ngnd * ngnd];
    let mut l1_0 = vec![ngnd; ngnd + 1];
    let mut l2_0 = vec![1usize; ngnd + 1];
    let mut nth1 = 0usize;
    let mut nth = nther;
    let mut ip1 = false;
    let Some(first) = mat.blocks.first() else {
        return Err(NjoyError::EndfParse("wimsr::p1scat: no material".into()));
    };
    for sec in &first.sections {
        let nl = sec.nl as i64;
        let nz = sec.nz as i64;
        if nl < il && sec.mf != 1 {
            continue; // 155
        }
        if sec.mf == 3 {
            // 230: only MT=1 walks its records; nothing is stored
            continue;
        }
        if sec.mf != 6 {
            continue;
        }
        let mth = sec.mt;
        if (18..=21).contains(&mth) || mth == 38 || mth == 2 {
            continue;
        }
        if mth == mti || mth == mtc {
            // 180: locate the thermal boundary
            for rec in &sec.records {
                let ig = rec.ig.max(0) as usize;
                if ig > nth1 && ig <= nth {
                    nth1 = ig;
                }
                if ig >= nth {
                    break;
                }
            }
            continue;
        }
        if (221..=250).contains(&mth) {
            continue;
        }
        for rec in &sec.records {
            let ig = rec.ig as i64;
            if ig == 0 || ig > ngnd as i64 {
                continue; // 225
            }
            let jg = (ngnd as i64 - ig + 1) as usize;
            let ng2 = rec.ng2 as i64;
            let ig2lo = rec.ig2lo as i64;
            for i in 2..=ng2 {
                let ig2 = ig2lo + i - 2;
                if ig2 != 0 && ig2 <= ngnd as i64 {
                    let jg2 = (ngnd as i64 - ig2 + 1) as usize;
                    let mut jz = nz;
                    if isg > 0 && (isg as i64) < nz {
                        jz = isg as i64;
                    }
                    let loca = (il - 1) + nl * nz * (i - 1) + (jz - 1) * nl;
                    sloc0[(jg - 1) + ngnd * (jg2 - 1)] += d(&rec.data, loca);
                }
            }
            ip1 = true;
            let l1t = ngnd as i64 - ig2lo - ng2 + 3;
            let l1tc = l1t;
            if l1tc < l1_0[jg] as i64 && l1tc != 0 && l1tc <= ngnd as i64 {
                l1_0[jg] = l1tc as usize;
            }
            let l2t = l1t + ng2 - 2;
            let l2tc = l2t;
            if l2tc > l2_0[jg] as i64 && l2tc != l1t && l2tc > 0 && l2tc <= ngnd as i64 {
                l2_0[jg] = l2tc as usize;
            }
        }
    }
    // label 200 with jtemp = 0: nth = nth1
    nth = nth1;

    // --- temperature passes
    let mut out = Vec::new();
    for (jt, block) in mat.blocks.iter().enumerate() {
        let jtemp = jt + 1;
        if jtemp > ntemp {
            break;
        }
        let mut sloc = sloc0.clone();
        let mut l1 = l1_0.clone();
        let mut l2 = l2_0.clone();
        let mut l1e = vec![ngnd; ngnd + 1];
        let mut l2e = vec![1usize; ngnd + 1];
        let mut elas = vec![0.0f64; ngnd * ngnd];
        let mut mtelas = false;
        let mut itd = false;
        for sec in &block.sections {
            let nl = sec.nl as i64;
            let nz = sec.nz as i64;
            if nl < il && sec.mf != 1 {
                continue;
            }
            if sec.mf != 6 {
                continue;
            }
            let mth = sec.mt;
            if mth == 2 && !mtelas {
                mtelas = true;
            }
            // 210: only mt=2, mti, mtc are added at a temperature
            if !(mth == 2 || mth == mti || mth == mtc) {
                continue;
            }
            for rec in &sec.records {
                let ig = rec.ig as i64;
                if ig == 0 || ig > ngnd as i64 {
                    continue;
                }
                // 215
                if mth == 2 && ig as usize <= nth {
                    continue; // next record (140)
                }
                if mth == mti && ig as usize > nth {
                    break; // 155
                }
                if mth == mtc && ig as usize > nth && mtc != 0 {
                    break;
                }
                itd = true;
                let jg = (ngnd as i64 - ig + 1) as usize;
                let ng2 = rec.ng2 as i64;
                let ig2lo = rec.ig2lo as i64;
                for i in 2..=ng2 {
                    let ig2 = ig2lo + i - 2;
                    if ig2 != 0 && ig2 <= ngnd as i64 {
                        let jg2 = (ngnd as i64 - ig2 + 1) as usize;
                        let mut jz = nz;
                        if isg > 0 && (isg as i64) < nz {
                            jz = isg as i64;
                        }
                        let loca = (il - 1) + nl * nz * (i - 1) + (jz - 1) * nl;
                        let v = d(&rec.data, loca);
                        let loc = (jg - 1) + ngnd * (jg2 - 1);
                        sloc[loc] += v;
                        if mth == 2 {
                            elas[loc] += v;
                        }
                    }
                }
                ip1 = true;
                let l1t = ngnd as i64 - ig2lo - ng2 + 3;
                let l1tc = l1t;
                if l1tc < l1[jg] as i64 && l1tc != 0 {
                    l1[jg] = l1tc as usize;
                }
                if mth == 2 {
                    l1e[jg] = l1tc.max(0) as usize;
                }
                let l2t = l1t + ng2 - 2;
                let l2tc = l2t;
                if l2tc > l2[jg] as i64 && l2tc != l1t && l2tc > 0 && l2tc <= ngnd as i64 {
                    l2[jg] = l2tc as usize;
                }
                if mth == 2 && l2tc != l1t {
                    l2e[jg] = l2tc.max(0) as usize;
                }
            }
        }
        // 250
        if !ip1 {
            return Err(NjoyError::EndfParse(format!(
                "wimsr::p1scat: no p1 matrices found for mat {}",
                inp.mat
            )));
        }
        if mti > 0 && !itd {
            return Err(NjoyError::EndfParse(format!(
                "wimsr::p1scat: no temperature-dependent reactions for mat {}",
                inp.mat
            )));
        }
        if !mtelas {
            for i in 1..=ngnd {
                if l1e[i] < l1[i] {
                    l1[i] = l1e[i];
                }
                if l2e[i] > l2[i] {
                    l2[i] = l2e[i];
                }
            }
            for (s, e) in sloc.iter_mut().zip(&elas) {
                *s += e;
            }
        }
        // suppress upscattering from thermal into resonance groups (l.1895-1907)
        let nthr = nfg + nrg + 1;
        for jg in nthr..=ngnd {
            let mut jg2 = l1[jg];
            while jg2 < nthr {
                let loc1 = (jg - 1) + ngnd * (jg2 - 1);
                let loc2 = (jg - 1) + ngnd * (nthr - 1);
                let v = sloc[loc1];
                sloc[loc2] += v;
                sloc[loc1] = 0.0;
                jg2 += 1;
                l1[jg] = jg2;
            }
        }
        // p1sout (l.1919-1969)
        let mut rows = Vec::with_capacity(ngnd);
        for ig in 1..=ngnd {
            let lone = l1[ig];
            let ltwo = l2[ig];
            let mut vals = Vec::new();
            if lone <= ltwo {
                for i in lone..=ltwo {
                    vals.push(sloc[(ig - 1) + ngnd * (i - 1)]);
                }
            }
            rows.push((ig, lone, ltwo, vals));
        }
        out.push(P1Temp {
            temp: block.temp,
            rows,
        });
    }
    Ok(Some(out))
}
