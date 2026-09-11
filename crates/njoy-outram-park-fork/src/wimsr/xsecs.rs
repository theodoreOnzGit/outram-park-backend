// Ported from NJOY2016 `src/wimsr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine xsecs`, l.1069-1447 — the per-temperature cross-section
//     accumulation from the GENDF.
//   - `subroutine xseco`, l.1449-1678 — the temperature-independent and
//     per-temperature records (the `nscr2`/`nscr3` scratch content).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `xsecs` — WIMS cross sections from the GENDF: absorption, transport-
//! corrected total, ν σ_f / σ_f, the P0 scattering matrix with its
//! `(l1, l2)` band, the slowing-down power, the potential cross section
//! and the fission spectrum; `xseco` — how they are laid out for the
//! library.
//!
//! Indices follow the Fortran: GENDF group `ig` counts up in energy,
//! WIMS group `jg = ngnd - ig + 1` counts down; a record's data word
//! `scr(l + lz + off)` is `data[off]` here. The upstream idioms are
//! kept: `abs1` (capture at the reference dilution) and `ab0` persist
//! across temperatures where every other array is re-zeroed; the
//! `MF=5/452` spectrum takes only the section's first record; a
//! fission matrix seen at `jtemp = 1` supplies ν and χ when `MF=3/452`
//! or `MF=5/452` are missing (with upstream's "only prompt contribution"
//! message); the thermal boundary `nth` is set by `MF=3/mti` (highest
//! group with a non-zero thermal cross section) and, failing that, by
//! `ngnd - nfg - nrg` when the first `MF=6` section is met; `mti`'s
//! matrix must be present at every temperature or the run stops there
//! ("mti missing from higher temps").

use crate::dtfr::gendf::GendfGroupRecord;
use crate::NjoyError;

use super::gendf::GendfMaterial;
use super::input::WimsrInput;

/// The `nscr2` record `xseco` writes once (`wimsr.f90:1478-1487`,
/// `1531-1533`, `1596-1597`), plus what the `iprint` listing shows.
#[derive(Debug, Clone, Default)]
pub struct TempIndependent {
    /// `spot(ngr0..=ngr1)`, `sdp(ngr0..=ngr1)`, `sn2n(1..=nfg)` (WIMS-E
    /// only), `xtr(1..=nnt)`, `ab0(1..=nnt)`, `nrg` zeros, `glam` — the
    /// flat record, `nw = 4 nrg + 2 nnt (+ nfg)` words.
    pub record: Vec<f64>,
    /// `snus(1..=nnt)`, `sf0(1..=nnt)` when `ifiss > 1`.
    pub fission: Option<Vec<f64>>,
    /// The packed non-thermal P0 scattering block (`ndat` words).
    pub scatter: Vec<f64>,
    /// The listing's `p1nrm * p1flx(1..=nnt)`.
    pub current_spectrum: Vec<f64>,
}

/// The `nscr3` records per temperature (`wimsr.f90:1601-1602`, `1617-1618`,
/// `1675-1676`).
#[derive(Debug, Clone, Default)]
pub struct TempDependent {
    /// `tempr(itemp)` \[K\].
    pub temp: f64,
    /// `xtr(nt0..=ngnd)`, `ab0(nt0..=ngnd)`.
    pub record: Vec<f64>,
    /// `snus(nt0..=ngnd)`, `sf0(nt0..=ngnd)` when `ifiss > 1`.
    pub fission: Option<Vec<f64>>,
    /// The packed thermal P0 scattering block.
    pub scatter: Vec<f64>,
}

/// Everything `xsecs` leaves in the module globals for `resint`,
/// `p1scat` and `wimout`, plus the `xseco` records.
#[derive(Debug, Clone, Default)]
pub struct XsecsResult {
    pub temp_independent: TempIndependent,
    pub per_temp: Vec<TempDependent>,
    /// `ifiss` (the `jfis` of the first temperature).
    pub ifiss: i32,
    /// `snu(1..=ngnd)`, `spot(1..=ngnd)`, `abs2(1..=ngnd)` (1-based
    /// storage: index 0 unused).
    pub snu: Vec<f64>,
    pub spot: Vec<f64>,
    pub abs2: Vec<f64>,
    /// `p1flx(1..=ngnd)` after the first temperature.
    pub p1flx: Vec<f64>,
    /// `uff(1..=nfiss)` — the normalised fission spectrum (`isof = 1`).
    pub uff: Vec<f64>,
    pub nfiss: usize,
    /// `isg` after the reference-dilution search (0, or the 1-based
    /// sigma-zero index `iz`).
    pub isg: usize,
    /// `tempr(1..=ntemp)` — the temperatures actually processed.
    pub tempr: Vec<f64>,
    /// `ntemp` after the "mti missing" cut-back.
    pub ntemp: usize,
    /// Upstream's `mess` lines.
    pub messages: Vec<String>,
}

/// `ntemp`/`nsigz`/`ires` after `wminit`'s clamps against the tape
/// (`wimsr.f90:328-345`).
pub struct Counts {
    pub ntemp: usize,
    pub nsigz: usize,
    pub ires: usize,
}

/// `wminit`'s counts: `ntemp = 0` means every temperature on the tape,
/// `nsigz = 0` every sigma-zero; `ires` is raised to the tape's
/// temperature count when it exceeds `ntemp`.
pub fn counts(inp: &WimsrInput, mat: &GendfMaterial) -> Counts {
    let jtemp = mat.blocks.len();
    let nz = mat.blocks[0].nz.max(0) as usize;
    let mut ntemp = inp.ntemp;
    if ntemp == 0 {
        ntemp = jtemp;
    }
    if ntemp > jtemp {
        ntemp = jtemp;
    }
    let mut ires = inp.ires;
    if ires > 0 && ntemp < ires {
        ires = jtemp;
    }
    let mut nsigz = inp.nsigz;
    if nsigz == 0 {
        nsigz = nz;
    }
    if nsigz > nz {
        nsigz = nz;
    }
    Counts { ntemp, nsigz, ires }
}

#[inline]
fn d(rec: &GendfGroupRecord, off: i64) -> f64 {
    if off < 0 {
        return 0.0;
    }
    rec.data.get(off as usize).copied().unwrap_or(0.0)
}

/// The packed scattering block for WIMS groups `ia` in `range`
/// (`xseco`, `wimsr.f90:1537-1573` and `1630-1667`): per group the pair
/// `(ia - l1 + 1, l2 - l1 + 1)` then `xs(ia, l1..=l2)` when `l1 != l2`;
/// a group with no transfers gives `(1, 0)`.
fn scatter_block(
    range: std::ops::RangeInclusive<usize>,
    ngnd: usize,
    l1: &[usize],
    l2: &[usize],
    xs: &[f64],
) -> Vec<f64> {
    let mut out = Vec::new();
    for ia in range {
        let lone = l1[ia];
        let ltwo = l2[ia];
        if lone != ngnd || ltwo != 1 {
            out.push((ia as f64) - (lone as f64) + 1.0);
            out.push((ltwo as f64) - (lone as f64) + 1.0);
            if lone != ltwo {
                for i in lone..=ltwo {
                    out.push(xs[(ia - 1) + ngnd * (i - 1)]);
                }
            }
        } else {
            out.push(1.0);
            out.push(0.0);
        }
    }
    out
}

/// `xsecs` (`wimsr.f90:1069-1447`) + `xseco` over every temperature
/// block.
///
/// # Errors
/// `EndfParse` when the reference sigma-zero is not on the tape is *not*
/// an error upstream (the first entry is used, with a message); an empty
/// tape is.
#[allow(clippy::needless_range_loop)] // Fortran index loops kept verbatim
pub fn xsecs(inp: &WimsrInput, mat: &GendfMaterial, cnt: &Counts) -> Result<XsecsResult, NjoyError> {
    const DILINF: f64 = 1.0e10;
    let ngnd = inp.ngnd;
    let nfg = inp.nfg;
    let nrg = inp.nrg;
    let ngr1 = nfg + nrg;
    let ntemp = cnt.ntemp;
    let egb = &mat.egb;
    let mti = inp.mti;
    let mtc = inp.mtc;
    let sigp = inp.sigp;

    let mut out = XsecsResult {
        snu: vec![0.0; ngnd + 1],
        spot: vec![0.0; ngnd + 1],
        abs2: vec![0.0; ngnd + 1],
        p1flx: vec![0.0; ngnd + 1],
        uff: Vec::new(),
        ..Default::default()
    };
    // card 8 current spectrum (wimsr.f90:210-216)
    for (j, &v) in inp.p1flx_user.iter().enumerate() {
        if j < ngnd {
            out.p1flx[j + 1] = v;
        }
    }
    let mut uff = vec![0.0f64; ngnd + 1];
    let mut snu = vec![0.0f64; ngnd + 1];
    // arrays that persist across temperatures (allocated once upstream)
    let mut abs1 = vec![0.0f64; ngnd + 1];
    let mut ab0 = vec![0.0f64; ngnd + 1];
    let mut xtr = vec![0.0f64; ngnd + 1];
    let mut sdp = vec![0.0f64; ngnd + 1];
    let mut nfiss = 0usize;
    let mut jfiss = 0i32;
    let mut jfisd = 0i32;
    let mut jfist = 0i32;
    let mut jfspd = 0i32;
    let mut jfspt = 0i32;
    let mut jfspp = 0i32;
    let mut jn2n = 0i32;
    let mut i318 = 0i32;
    let mut p1nrm = 1.0f64;
    let mut isg = if inp.sgref < DILINF { 1usize } else { 0 };
    let mut sgref = inp.sgref;
    let mut jfis = 0i32;
    let mut ifiss = 0i32;
    // aver. log. decrement per collision, isotropic scattering (l.1130-1132)
    let alf = ((mat.awr - 1.0) / (mat.awr + 1.0)).powi(2);
    let xxi = 1.0 + alf.ln() * alf / (1.0 - alf);

    let mut jtemp = 0usize;
    let mut blocks = mat.blocks.iter();
    loop {
        // label 140: per-temperature initialisation
        let Some(block) = blocks.next() else {
            break; // math.ne.mat -> 540
        };
        let mut csp1 = vec![0.0f64; ngnd + 1];
        let mut spot = vec![sigp; ngnd + 1];
        let mut xi = vec![xxi; ngnd + 1];
        let mut abs2 = vec![0.0f64; ngnd + 1];
        let mut sf0 = vec![0.0f64; ngnd + 1];
        let mut sfi = vec![0.0f64; ngnd + 1];
        let mut snus = vec![0.0f64; ngnd + 1];
        let mut cspc = vec![0.0f64; ngnd + 1];
        let mut chi = vec![0.0f64; ngnd + 1];
        let mut scat = vec![0.0f64; ngnd + 1];
        let mut sn2n = vec![0.0f64; ngnd + 1];
        let mut l1e = vec![ngnd; ngnd + 1];
        let mut l2e = vec![1usize; ngnd + 1];
        let mut l1 = vec![ngnd; ngnd + 1];
        let mut l2 = vec![1usize; ngnd + 1];
        let mut xs = vec![0.0f64; ngnd * ngnd];
        let mut cnorm;
        let mut dnorm = 0.0f64;
        let mut nth1 = 0usize;
        let mut nth = 0usize;
        let mut if6 = 0i32;
        // the module globals spot/abs2 are re-initialised here upstream, so a
        // run that stops at this temperature ("mti missing") leaves resint
        // reading these values
        out.spot = spot.clone();
        out.abs2 = abs2.clone();

        // reference sigma-zero (l.1149-1165)
        let nz = block.nz.max(0) as usize;
        let mut iz = 0usize;
        for i in 1..=nz {
            if (sgref - block.sigz[i - 1]).abs() <= sgref / 100.0 {
                iz = i;
                break;
            }
        }
        if iz == 0 {
            out.messages.push(format!(
                "xsecs: ref. sig0 {sgref:.3e} not on the list — first entry used as default"
            ));
            iz = 1;
            sgref = block.sigz[0];
        }
        if isg > 0 {
            isg = iz;
        }
        jtemp += 1;
        out.tempr.push(block.temp);
        let mut jic = 0i32;

        // --- loop over reactions (label 150)
        for sec in &block.sections {
            let mfh = sec.mf;
            let mth = sec.mt;
            let nl = sec.nl as i64;
            let nzs = sec.nz as i64;
            let nli = sec.nl.max(1) as i64;
            let _ = nli;
            // 'skip this section' == go to 365
            let mut skip_section = false;
            for rec in &sec.records {
                if skip_section {
                    break;
                }
                let ng2 = rec.ng2 as i64;
                let ig2lo = rec.ig2lo as i64;
                let ig = rec.ig as i64;
                let jg = (ngnd as i64 - ig + 1) as usize;
                let lz = 0i64; // data[off]: off = loca - (l + lz)
                let _ = lz;

                // branch according to type of data (l.1180-1195)
                enum B {
                    Nu452,
                    Del455,
                    Abs,
                    Fis,
                    Tot,
                    Ela,
                    N2n,
                    N3n,
                    Xi,
                    Thermal,
                    Mf6Temp,
                    Mf6Indep,
                    SkipSection,
                }
                let br = if mth == 452 {
                    B::Nu452
                } else if mth == 455 {
                    B::Del455
                } else if mfh == 3 {
                    if (102..=150).contains(&mth) {
                        B::Abs
                    } else if (18..=21).contains(&mth) || mth == 38 {
                        B::Fis
                    } else if mth == 1 {
                        B::Tot
                    } else if mth == 2 {
                        B::Ela
                    } else {
                        if mth == 16 {
                            jn2n = 16;
                        }
                        if mth == 16 || mth == 24 || ((875..=891).contains(&mth) && jn2n != 16) {
                            B::N2n
                        } else if mth == 17 || mth == 25 {
                            B::N3n
                        } else if mth == 252 {
                            B::Xi
                        } else if mth == mti {
                            B::Thermal
                        } else {
                            B::SkipSection
                        }
                    }
                } else if mfh != 6 {
                    B::SkipSection
                } else if mth == 2 || mth == mti || mth == mtc {
                    B::Mf6Temp
                } else {
                    B::Mf6Indep
                };

                // `true` = go to 365 (skip the rest of the section)
                let mut to_365 = false;
                match br {
                    B::SkipSection => to_365 = true,
                    B::Abs => {
                        // l.1198-1206
                        let mut loca = nl * nzs;
                        if mth == 102 {
                            if isg > 0 && nzs >= iz as i64 {
                                loca = nl * (iz as i64 - 1 + nzs);
                            }
                            abs1[jg] = d(rec, loca);
                        } else {
                            abs2[jg] += d(rec, loca);
                        }
                    }
                    B::Fis => {
                        // l.1209-1220
                        if mth == 18 {
                            i318 = 1;
                        }
                        if mth == 18 || i318 == 0 {
                            let mut loca = nl * nzs;
                            sfi[jg] += d(rec, loca);
                            if isg > 0 && nzs >= iz as i64 {
                                loca = nl * (iz as i64 - 1 + nzs);
                            }
                            sf0[jg] += d(rec, loca);
                            ab0[jg] += d(rec, loca);
                        } else {
                            to_365 = true;
                        }
                    }
                    B::Tot => {
                        // l.1224-1234
                        let mut loca = nl * (nzs - 1);
                        if isg > 0 && nzs >= iz as i64 {
                            loca = nl * (iz as i64 - 1);
                        }
                        if nl > 1 {
                            loca += 1;
                        }
                        if out.p1flx[jg] == 0.0 {
                            out.p1flx[jg] = d(rec, loca);
                            p1nrm = 1.0;
                        } else {
                            if p1nrm == 1.0 {
                                p1nrm = d(rec, loca) / out.p1flx[jg];
                            }
                            out.p1flx[jg] *= p1nrm;
                        }
                    }
                    B::N2n => {
                        sn2n[jg] += d(rec, nl * nzs);
                    }
                    B::N3n => {
                        sn2n[jg] += 2.0 * d(rec, nl * nzs);
                    }
                    B::Thermal => {
                        // l.1250-1252
                        if ig as usize > nth1 && d(rec, nzs * nl) != 0.0 {
                            nth1 = ig as usize;
                        }
                        nth = ngnd - nth1 + 1;
                    }
                    B::Ela => {}
                    B::Xi => {
                        xi[jg] = d(rec, nl * nzs);
                    }
                    B::Nu452 => {
                        // l.1264-1285
                        if jtemp > 1 {
                            to_365 = true;
                        } else {
                            if jfiss == 0 {
                                jfiss = 1;
                            }
                            if mfh != 5 {
                                jfist = 1;
                                snu[jg] = d(rec, 1);
                            } else {
                                if inp.isof != 0 {
                                    jfspt = 1;
                                    for i in 2..=ng2 {
                                        let jg2 = ngnd as i64 - ig2lo - i + 3;
                                        if jg2 >= 1 && jg2 <= ngnd as i64 {
                                            uff[jg2 as usize] = d(rec, nl * (i - 1));
                                        }
                                    }
                                }
                                to_365 = true;
                            }
                        }
                    }
                    B::Del455 => {
                        // l.1288-1305
                        if jtemp > 1 {
                            to_365 = true;
                        } else if mfh != 5 {
                            jfisd = 1;
                            snus[jg] += d(rec, 1) * d(rec, 2);
                            dnorm += d(rec, 0) * d(rec, 1) * d(rec, 2);
                        } else {
                            jfspd = 1;
                            for i in 2..=ng2 {
                                let locc = 1 + ngnd as i64 - ig2lo - i + 2;
                                for ll in 1..=nl {
                                    let loca = ll - 1 + nl * (i - 1);
                                    if locc >= 1 && locc <= ngnd as i64 {
                                        chi[locc as usize] += dnorm * d(rec, loca);
                                    }
                                }
                            }
                        }
                    }
                    B::Mf6Temp => {
                        // l.1308-1350 (temperature-dependent file 6 matrices)
                        if mth != 2 {
                            jic = 1;
                        }
                        if mth == 2 && jg >= nth {
                            // 295: next record
                        } else if mth == 2 && jg < nth {
                            // 301: treated as temperature-independent
                            to_365 = mf6_independent(
                                rec, mfh, mth, nl, nzs, ng2, ig2lo, jg, ngnd, nfg, nrg, mti, mtc,
                                isg, iz, jtemp, &mut if6, &mut nth1, &mut nth, &mut xs,
                                &mut scat, &mut csp1, &out.p1flx, &mut l1, &mut l2, &mut snus,
                                &mut chi, &mut cspc, &mut jfiss, &mut jfspp, true,
                            );
                        } else {
                            let mut max = 0usize;
                            let mut min = ngnd;
                            for i in 2..=ng2 {
                                let ig2 = ig2lo + i - 2;
                                if ig2 >= 1 && ig2 <= ngnd as i64 {
                                    let jg2 = (ngnd as i64 - ig2 + 1) as usize;
                                    let mut loca = nl * nzs * (i - 1);
                                    if isg > 0 && nzs >= iz as i64 {
                                        loca += nl * (iz as i64 - 1);
                                    }
                                    let v = d(rec, loca);
                                    if v != 0.0 && mth != 2 {
                                        let loc = (jg - 1) + ngnd * (jg2 - 1);
                                        if jg2 as i64 - jg as i64 >= -1 || v >= 0.0 {
                                            xs[loc] += v;
                                            scat[jg] += v;
                                            if jg > ngr1 {
                                                csp1[jg] += d(rec, loca + 1);
                                            }
                                            if jg2 > max {
                                                max = jg2;
                                            }
                                            if jg2 < min {
                                                min = jg2;
                                            }
                                        }
                                    }
                                }
                            }
                            if min < l1[jg] && mth != 2 {
                                l1[jg] = min;
                            }
                            if min < l1e[jg] && mth == 2 {
                                l1e[jg] = min;
                            }
                            if max > l2[jg] && mth != 2 {
                                l2[jg] = max;
                            }
                            if max > l2e[jg] && mth == 2 {
                                l2e[jg] = max;
                            }
                        }
                    }
                    B::Mf6Indep => {
                        // l.1352-1355: if jtemp > 1 -> 360 (skip record only)
                        if jtemp <= 1 {
                            to_365 = mf6_independent(
                                rec, mfh, mth, nl, nzs, ng2, ig2lo, jg, ngnd, nfg, nrg, mti, mtc,
                                isg, iz, jtemp, &mut if6, &mut nth1, &mut nth, &mut xs,
                                &mut scat, &mut csp1, &out.p1flx, &mut l1, &mut l2, &mut snus,
                                &mut chi, &mut cspc, &mut jfiss, &mut jfspp, false,
                            );
                        }
                    }
                }
                if to_365 {
                    skip_section = true;
                }
                // label 360/295: if ig < ngnd continue with the next record
            }
        }

        // --- label 370: data loaded for this temperature
        if jtemp <= 1 && jfiss >= 2 && (jfisd >= 1 || jfist <= 0) {
            // fix up nu if computed from matrix (l.1415-1428)
            for i in 1..=ngnd {
                if sfi[i] != 0.0 {
                    snu[i] = snus[i] / sfi[i];
                }
            }
            if jfisd != 1 {
                out.messages.push(
                    "xsecs: nu-bar calculated from fission matrix — only prompt contribution available"
                        .into(),
                );
            }
        }
        if jic == 0 {
            // l.1431-1437: mti missing at this temperature -> stop here
            jtemp -= 1;
            out.tempr.pop();
            out.messages.push(format!(
                "xsecs: use only {jtemp} temps for mat {} — mti missing from higher temps",
                inp.mat
            ));
            break;
        }
        if jfiss != 0 {
            for i in 1..=ngnd {
                snus[i] = snu[i] * sf0[i];
            }
        }
        // total absorption (l.1445-1449)
        for i in 1..=ngnd {
            ab0[i] = sf0[i] + abs1[i] + abs2[i] - sn2n[i];
        }
        // correct scattering matrix (l.1451-1455)
        let in_ = nth.saturating_sub(1);
        for i in 1..=in_.min(ngnd) {
            if l1e[i] < l1[i] {
                l1[i] = l1e[i];
            }
            if l2e[i] > l2[i] {
                l2[i] = l2e[i];
            }
        }
        // correct transport (l.1457-1461)
        for i in 1..=ngnd {
            if inp.ip1opt > 0 {
                xs[(i - 1) * (1 + ngnd)] -= csp1[i];
            }
            xtr[i] = scat[i] + ab0[i] - csp1[i];
        }
        // slowing-down power; suppress upscatter from thermal into
        // resonance groups (l.1463-1481, the suppression nested as upstream)
        let nthr = nfg + nrg + 1;
        for i in 1..=ngnd {
            for jg in nthr..=ngnd {
                let mut jg2 = l1[jg];
                while jg2 < nthr {
                    let loc1 = (jg - 1) + ngnd * (jg2 - 1);
                    let loc2 = (jg - 1) + ngnd * (nthr - 1);
                    let v = xs[loc1];
                    xs[loc2] += v;
                    xs[loc1] = 0.0;
                    jg2 += 1;
                    l1[jg] = jg2;
                }
            }
            sdp[i] = scat[i] * xi[i] / (egb[i - 1] / egb[i]).ln();
            if sigp == 0.0 {
                spot[i] = scat[i];
            }
        }
        if jtemp == 1 {
            jfis = 0;
            if inp.ires > 0 {
                jfis = 1;
            }
            if jfis >= 1 && jfiss > 0 {
                jfis = 3;
            }
            if jfis > 1 && inp.inorf > 0 {
                jfis = 2;
            }
            if jfis == 0 && jfiss > 0 {
                jfis = 4;
            }
        }

        // --- xseco (l.1449-1678)
        let ngr0 = nfg + 1;
        let nnt = ngr1;
        if jtemp == 1 {
            let mut record = Vec::new();
            record.extend((ngr0..=ngr1).map(|i| spot[i]));
            record.extend((ngr0..=ngr1).map(|i| sdp[i]));
            if inp.iverw == 5 {
                record.extend((1..=nfg).map(|i| sn2n[i]));
            }
            record.extend((1..=nnt).map(|i| xtr[i]));
            record.extend((1..=nnt).map(|i| ab0[i]));
            record.extend((ngr0..=ngr1).map(|_| 0.0));
            record.extend(inp.glam.iter().take(nrg).copied());
            let fission = if jfis > 1 {
                let mut f: Vec<f64> = (1..=nnt).map(|i| snus[i]).collect();
                f.extend((1..=nnt).map(|i| sf0[i]));
                Some(f)
            } else {
                None
            };
            let scatter = scatter_block(1..=nnt, ngnd, &l1, &l2, &xs);
            // `xseco` declares `p1nrm` as an *integer* (`wimsr.f90:1435`),
            // so `p1nrm=1/p1flx(igref)` (l.1467) truncates toward zero: the
            // listing prints the raw `p1flx` whenever `0.5 < p1flx(igref)
            // <= 1` and all zeros whenever `p1flx(igref) > 1`. Mirrored.
            let mut pn = 1.0;
            if inp.igref <= nnt && out.p1flx[inp.igref] != 0.0 {
                pn = (1.0 / out.p1flx[inp.igref]).trunc();
            }
            out.temp_independent = TempIndependent {
                record,
                fission,
                scatter,
                current_spectrum: (1..=nnt).map(|i| pn * out.p1flx[i]).collect(),
            };
        }
        // resint/wimout read the module globals as the last temperature left them
        out.spot = spot.clone();
        out.abs2 = abs2.clone();
        let nt0 = nnt + 1;
        let mut record: Vec<f64> = (nt0..=ngnd).map(|i| xtr[i]).collect();
        record.extend((nt0..=ngnd).map(|i| ab0[i]));
        let fission = if jfis > 1 {
            let mut f: Vec<f64> = (nt0..=ngnd).map(|i| snus[i]).collect();
            f.extend((nt0..=ngnd).map(|i| sf0[i]));
            Some(f)
        } else {
            None
        };
        let scatter = scatter_block(nt0..=ngnd, ngnd, &l1, &l2, &xs);
        out.per_temp.push(TempDependent {
            temp: block.temp,
            record,
            fission,
            scatter,
        });
        i318 = 0;
        ifiss = jfis;

        // fission spectrum (l.1490-1516)
        if inp.isof != 0 && jtemp <= 1 && jfiss != 0 {
            if jfspt == 0 {
                if jfspd != 1 || jfspp != 1 {
                    out.messages.push(
                        "xsecs: spectrum calculated from fission matrix — only prompt contribution available"
                            .into(),
                    );
                }
                uff[1..=ngnd].copy_from_slice(&chi[1..=ngnd]);
            }
            cnorm = 0.0;
            for i in 1..=ngr1 {
                if uff[i] > 0.0 {
                    cnorm += uff[i];
                    nfiss = i;
                }
            }
            if cnorm > 0.0 {
                cnorm = 1.0 / cnorm;
                for i in 1..=nfiss {
                    uff[i] *= cnorm;
                }
            }
        }

        // continue loop over temperatures (l.1519-1521)
        if jtemp < ntemp || ntemp == 0 {
            continue;
        }
        break;
    }

    out.ifiss = ifiss;
    out.snu = snu;
    out.uff = uff[1..=nfiss].to_vec();
    out.nfiss = nfiss;
    out.isg = isg;
    out.ntemp = jtemp;
    Ok(out)
}

/// Labels 301–347: the temperature-independent `MF=6` matrix path
/// (`wimsr.f90:1353-1412`). Returns `true` for "go to 365" (skip the
/// rest of the section). `via_elastic` marks the entry from the `mth = 2`
/// / `jg < nth` branch of label 270.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::needless_range_loop)] // Fortran index loops kept verbatim
fn mf6_independent(
    rec: &GendfGroupRecord,
    mfh: i32,
    mth: i32,
    nl: i64,
    nz: i64,
    ng2: i64,
    ig2lo: i64,
    jg: usize,
    ngnd: usize,
    nfg: usize,
    nrg: usize,
    _mti: i32,
    _mtc: i32,
    isg: usize,
    iz: usize,
    jtemp: usize,
    if6: &mut i32,
    nth1: &mut usize,
    nth: &mut usize,
    xs: &mut [f64],
    scat: &mut [f64],
    csp1: &mut [f64],
    p1flx: &[f64],
    l1: &mut [usize],
    l2: &mut [usize],
    snus: &mut [f64],
    chi: &mut [f64],
    cspc: &mut [f64],
    jfiss: &mut i32,
    jfspp: &mut i32,
    _via_elastic: bool,
) -> bool {
    if mfh != 6 {
        return true;
    }
    if *if6 == 0 {
        if *nth1 == 0 {
            *nth1 = ngnd - nfg - nrg;
        }
        *nth = ngnd - *nth1 + 1;
        *if6 = 1;
    }
    // label 305
    let is_fission = (18..=21).contains(&mth) || mth == 38;
    if !is_fission {
        if mth == 2 && jg >= *nth {
            return true;
        }
        if (221..=250).contains(&mth) {
            return true;
        }
        let mut max = 0usize;
        let mut min = ngnd;
        let ig = rec.ig as i64;
        let _ = ig;
        for i in 2..=ng2 {
            let ig2 = ig2lo + i - 2;
            if ig2 >= 1 && ig2 <= ngnd as i64 {
                let jg2 = (ngnd as i64 - ig2 + 1) as usize;
                let mut loca = nl * nz * (i - 1);
                if mth == 2 {
                    loca = nl * nz * (i - 1) + (nz - 1) * nl;
                    if isg > 0 && nz >= iz as i64 {
                        loca = nl * nz * (i - 1) + nl * (iz as i64 - 1);
                    }
                }
                let loc = (jg - 1) + ngnd * (jg2 - 1);
                let v = d(rec, loca);
                xs[loc] += v;
                scat[jg] += v;
                if jg2 > max {
                    max = jg2;
                }
                if jg2 < min {
                    min = jg2;
                }
                if nl != 1 && jg2 < *nth {
                    csp1[jg2] += d(rec, loca + 1) * p1flx[jg] / p1flx[jg2];
                }
            }
        }
        if min < l1[jg] {
            l1[jg] = min;
        }
        if max > l2[jg] {
            l2[jg] = max;
        }
        return false; // 360
    }
    // label 315: nu*sigf and chi from the fission matrix (l.1379-1410)
    if jtemp > 1 {
        return true;
    }
    let ig = rec.ig as i64;
    if ig != 0 {
        *jfiss = 2;
        *jfspp = 1;
        let locf = 0i64;
        for i in 2..=ng2 {
            let loca = nl * nz * (i - 1);
            if ig2lo != 0 {
                snus[jg] += d(rec, loca);
                let locc = 1 + ngnd as i64 - i - ig2lo + 2;
                if locc >= 1 && locc <= ngnd as i64 {
                    chi[locc as usize] += d(rec, locf) * d(rec, loca);
                }
            } else {
                for k in 1..=ngnd {
                    snus[jg] += cspc[k] * d(rec, loca);
                    let locc = 1 + ngnd - k;
                    chi[locc] += cspc[k] * d(rec, loca - 1) * d(rec, loca);
                }
            }
        }
    } else {
        for i in 1..=ng2 {
            let idx = ig2lo + i - 1;
            if idx >= 1 && idx <= ngnd as i64 {
                cspc[idx as usize] = d(rec, nl * nz * (i - 1));
            }
        }
    }
    false
}
