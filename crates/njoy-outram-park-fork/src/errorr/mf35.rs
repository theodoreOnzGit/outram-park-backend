// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine errorr`, l.551-640 (the MF=35 subsection scan: `ifissp`, `efmean`);
//   - `subroutine gridd`, l.1091-1483, `mfcov = 35` branch (covariance energies);
//   - `subroutine covcal`, l.1770-2417, LB=7 branch; `sumchk` l.2419-2513;
//     `spcint` l.2515-2719; `covbin` l.2721-2753;
//   - `subroutine fssigc`, l.6073-6193; `subroutine covout`, l.7018-7788, `mfcov = 35`.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **ERRORR `mfcov = 35`**: secondary-energy (fission spectrum) covariances from
//! an input GENDF (gh:#1369).
//!
//! ```text
//! ngchk                    -> GENDF at tempin, first sigma-zero, P0
//! MF=35 scan               -> ifissp, efmean, eclo, echi
//! gridd (mfcov = 35)       -> eni: every subsection's energies
//! egngpn = groups + uniong -> egn, un
//! colaps                   -> union-group flux (MT=1) and chi (MF=5/MT=18)
//! covcal                   -> per subsection: sumchk (zero-sum rule, spcint),
//!                             covbin, sigfig; LB=7 rows times flx(jg) flx(jh)
//! fssigc                   -> coarse flux and chi; MF=1 and MF=5 on nout
//! covout                   -> subsection ifissp collapsed; relative form
//!                             / (chi chi) * dE dE; MF=35 on nout
//! ```
//!
//! **Scope**: `iread = 0`, `nstan = nin = 0`, one MF=35 MT (18), an ENDF MF=5/MT=18
//! with one `LF = 1` subsection (what `spcint` itself supports apart from MF=6
//! `LAW = 1`), a GENDF whose fission spectrum is MF=5 or a GROUPR MF=6 matrix
//! with its `ig = 0` record (`mffis = 5, 6`). The rest is
//! refused by name.

use crate::deck::groupr::GendfText;
use crate::endf::interp::{intega, terpa};
use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

use super::colaps::{colaps, FissionWindow};
use super::gendf::{ngchk, GendfTape};
use super::gridd::{merge, uniong};
use super::groups::neutron_group_structure;
use super::mf33::endf_version;

const NDIG: i32 = 6;

/// The cards of an `mfcov = 35` run that this path reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Mf35Config {
    /// `matd`.
    pub matd: i32,
    /// `ign` (`1`/`-1` take `user_egn`).
    pub ign: i32,
    /// Card 12b bounds for `ign = +-1`.
    pub user_egn: Option<Vec<f64>>,
    /// `irelco`: 1 relative, 0 absolute.
    pub irelco: i32,
    /// Card 7's `ifissp` (`-1000` when not given: `-1`).
    pub ifissp: i32,
    /// Card 7's `efmean` (eV).
    pub efmean: f64,
    /// Card 3's `tempin` (K).
    pub tempin: f64,
}

/// One MF=35 subsection: the LIST head words and data, Fortran `covl(1:6+nt)`.
fn covl_of(head: &crate::endf::records::Cont, data: &[f64]) -> Vec<f64> {
    let mut v = vec![head.c1, head.c2, f64::from(head.l1), f64::from(head.l2), f64::from(head.n1), f64::from(head.n2)];
    v.extend_from_slice(data);
    v
}

/// The MF=35/MT=18 subsections of `matd` (`findf(matd,35,18)`).
fn mf35_subsections(endf: &Tape, matd: i32) -> Result<(f64, f64, Vec<Vec<f64>>), NjoyError> {
    let sec = endf.section(matd, 35, 18).ok_or(NjoyError::SectionNotFound { mat: matd, mf: 35, mt: 18 })?;
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    let mut subs = Vec::new();
    for _ in 0..head.n1.max(0) {
        let l = cur.read_list()?;
        subs.push(covl_of(&l.head, &l.data));
    }
    Ok((head.c1, head.c2, subs))
}

/// The subsection scan of `errorr` (`:571-640`): `(ifissp, efmean, eclo, echi)`.
fn select_subsection(subs: &[Vec<f64>], ifissp_in: i32, efmean_in: f64) -> Result<(usize, f64, f64, f64, i32), NjoyError> {
    let nfissp = subs.len() as i32;
    let mut ifissp = if ifissp_in == -1000 { -1 } else { ifissp_in };
    if ifissp > nfissp {
        return Err(NjoyError::InvalidInput(format!("errorr: User ifissp ({ifissp}) not found. Max on file is {nfissp}.")));
    }
    let mut efmean = efmean_in;
    let (mut eclo, mut echi) = (1.0e-5, 2.0e7);
    let mut idone = false;
    let mut igflag = 0;
    for (i0, s) in subs.iter().enumerate() {
        let i = i0 as i32 + 1;
        if !idone {
            eclo = s[0];
            echi = s[1];
        }
        if i == ifissp {
            efmean = (eclo + echi) / 2.0;
            idone = true;
        } else if ifissp <= 0 {
            if eclo <= efmean && echi >= efmean {
                ifissp = i;
                igflag = 1;
                idone = true;
            }
            if nfissp == 1 {
                if efmean < eclo || efmean > echi {
                    efmean = (eclo + echi) / 2.0;
                } else {
                    igflag = 1;
                }
                ifissp = 1;
            }
        }
    }
    if ifissp <= 0 {
        return Err(NjoyError::InvalidInput("errorr: no covariance data found for user ifissp/efmean".into()));
    }
    Ok((ifissp as usize, efmean, eclo, echi, igflag))
}

/// `spcint` (`:2515-2719`): the ENDF fission spectrum integrated over the
/// covariance bins of `covm`, MF=5 `LF = 1` only.
fn spcint(endf: &Tape, matd: i32, covm: &[f64]) -> Result<Vec<f64>, NjoyError> {
    let ne = covm[5].round() as usize;
    let ne1 = ne - 1;
    let mut spc = vec![0.0; ne1];
    if endf.section(matd, 6, 18).is_some() && endf.section(matd, 5, 18).is_none() {
        return Err(NjoyError::NotPorted("errorr spcint: fission spectrum from MF=6 (law 1)"));
    }
    let sec = endf.section(matd, 5, 18).ok_or(NjoyError::SectionNotFound { mat: matd, mf: 5, mt: 18 })?;
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    if head.n1 != 1 {
        return Err(NjoyError::NotPorted("errorr spcint: MF=5/MT=18 with more than one subsection"));
    }
    let p = cur.read_tab1()?;
    if p.head.l2 != 1 {
        return Err(NjoyError::NotPorted("errorr spcint: MF=5 LF other than 1 (upstream: not ready for lf)"));
    }
    let t2 = cur.read_tab2()?;
    let np2 = t2.head.n2.max(0);
    for _ in 0..np2 {
        let g = cur.read_tab1()?;
        let esp = g.head.c2;
        if esp >= covm[0] {
            let (pe, _, _) = terpa(&p.interp, &p.pairs, esp);
            for (i, s) in spc.iter_mut().enumerate() {
                let aaa = intega(&g.interp, &g.pairs, covm[6 + i], covm[7 + i])?;
                *s += pe * aaa;
            }
            break;
        }
    }
    Ok(spc)
}

/// `sumchk` (`:2419-2513`): the zero-sum rule, corrected in place when it fails.
/// Returns `true` when the correction was applied.
fn sumchk(covl: &mut [f64], spc: &[f64]) -> Result<bool, NjoyError> {
    if covl[2].round() as i32 != 1 || covl[3].round() as i32 != 7 {
        return Err(NjoyError::EndfParse("errorr sumchk: mf35 matrix must be symmetric with lb = 7".into()));
    }
    let ne = covl[5].round() as usize;
    let ne1 = ne - 1;
    let mut rcs = vec![0.0f64; ne1];
    let mut sumt = 0.0f64;
    let mut ib = 6 + ne;
    for i in 0..ne1 {
        for j in i..ne1 {
            let v = covl[ib];
            ib += 1;
            rcs[i] += v;
            sumt += v;
            if j > i {
                rcs[j] += v;
                sumt += v;
            }
        }
    }
    let mut failed = false;
    let mut i = 0;
    while !failed && i + 1 < ne1 {
        if rcs[i] / spc[i] > 1.0e-5 {
            failed = true;
        }
        i += 1;
    }
    if failed {
        let mut ib = 6 + ne;
        for i in 0..ne1 {
            for j in i..ne1 {
                let v = covl[ib] - spc[i] * rcs[j] - spc[j] * rcs[i] + spc[i] * spc[j] * sumt;
                covl[ib] = if v.abs() < 1.0e-30 { 0.0 } else { v };
                ib += 1;
            }
        }
    }
    Ok(failed)
}

/// `covbin` (`:2721-2753`): bin covariances to density covariances.
fn covbin(covl: &mut [f64]) {
    let ng = covl[5].round() as usize - 1;
    let mut midx = 6 + ng + 1;
    for i in 1..=ng {
        let dei = covl[6 + i] - covl[5 + i];
        for j in i..=ng {
            let dej = covl[6 + j] - covl[5 + j];
            covl[midx] /= dei * dej;
            midx += 1;
        }
    }
}

/// `covcal`'s LB=7 rows for one subsection on the union grid (`:2189-2213`,
/// `:2340-2378`): `(jg, b(1:jgend))`, absolute covariance times `flx(jg) flx(jh)`.
fn lb7_rows(covl: &[f64], un: &[f64], flx: &[f64]) -> Vec<(usize, Vec<f64>)> {
    let nunion = un.len() - 1;
    let np = covl[5].round() as usize;
    let nk1 = np - 1;
    let e = |k: usize| covl[5 + k]; // data word k (1-based)
    let bin = |x: f64| (1..=nk1).find(|&k| x >= e(k) && x < e(k + 1));
    let bins: Vec<Option<usize>> = un[..nunion].iter().map(|&x| bin(x)).collect();
    let tri = |k: usize, l: usize| {
        let (k, l) = if l >= k { (k, l) } else { (l, k) };
        let off = nk1 * np / 2 - (np - k + 1) * (np - k) / 2 + l - k;
        covl[6 + np + off]
    };
    let mut rows = Vec::new();
    let mut cov = vec![0.0f64; nunion];
    for jg in 1..=nunion {
        for jh in 1..=nunion {
            cov[jh - 1] = match (bins[jg - 1], bins[jh - 1]) {
                (Some(k), Some(l)) => 0.0 + tri(k, l),
                _ => 0.0,
            };
        }
        let mut jgend = cov.iter().rposition(|&c| c != 0.0).map_or(0, |p| p + 1);
        if jgend == 0 {
            if jg < nunion {
                continue;
            }
            jgend = 1;
        }
        let row: Vec<f64> = (1..=jgend).map(|ij| cov[ij - 1] * flx[jg - 1] * flx[ij - 1]).collect();
        rows.push((jg, row));
    }
    rows
}

/// `npage` (`endf.f90:19`): the words a binary LIST record carries after its head.
const NPAGE: usize = 306;

/// gfortran's `nint` of a `real(8)` into a default integer: `lround`, then the low
/// 32 bits (measured with gfortran 13: `nint(3e9) = -1294967296`, `nint(1e20) = 0`).
fn fortran_nint(x: f64) -> i32 {
    if x.is_nan() || x.abs() >= 9.223_372_036_854_775_8e18 {
        0
    } else {
        (x.round() as i64) as i32
    }
}

/// One record of covcal's binary scratch tape `nscr1`: `(nb, words)`.
type ScratchRecord = (i64, Vec<f64>);

/// covcal's rows as the binary records `listio`/`moreio` write them
/// (`:2208-2221`): the first record holds the six-word head and up to `npage`
/// words, each continuation up to `npage` more; `nb` is the count still to come.
fn scratch_records(rows_by_sub: &[Vec<(usize, Vec<f64>)>]) -> Vec<ScratchRecord> {
    let mut out = Vec::new();
    for rows in rows_by_sub {
        for (jg, row) in rows {
            let n = row.len();
            let first = n.min(NPAGE);
            let mut w = vec![0.0, 0.0, 0.0, 18.0, n as f64, *jg as f64];
            w.extend_from_slice(&row[..first]);
            out.push(((n - first) as i64, w));
            let mut done = first;
            while done < n {
                let take = (n - done).min(NPAGE);
                out.push(((n - done - take) as i64, row[done..done + take].to_vec()));
                done += take;
            }
        }
    }
    out
}

/// `covout`'s reading of `nscr1` for `mfcov = 35` with `isd = 1`
/// (`:7247-7320`, `:7375-7380`, `:7421-7428`), record by record: the rows it
/// accumulates, `(jg, cov(1:nw))`, in order.
///
/// Kept as upstream does it: a row of another subsection is skipped by **one**
/// `listio`, without reading its continuation records, so the next `listio`
/// takes a continuation record as a row head and its sixth word, `nint`'d, as
/// `jg`. Words beyond a short record's length keep the previous record's values
/// (`scr` is reused). That decides where upstream thinks a subsection ends.
fn covout_rows(stream: &[ScratchRecord], nmt1h: i32, ifissp: i32, nunion: usize) -> Result<Vec<(usize, Vec<f64>)>, NjoyError> {
    let mut a = [0.0f64; 6];
    let mut pos = 0usize;
    let short = || NjoyError::EndfParse("errorr covout: scratch tape ended inside the MF=35 section".into());
    // `listio`/`moreio` into scr(1:nw): returns (nb, words)
    let read = |pos: &mut usize, a: &mut [f64; 6], is_list: bool| -> Result<(i64, Vec<f64>), NjoyError> {
        let (nb, w) = stream.get(*pos).ok_or_else(short)?;
        *pos += 1;
        for (i, v) in w.iter().take(6).enumerate() {
            a[i] = *v;
        }
        if is_list {
            for v in a.iter_mut().skip(2) {
                *v = f64::from(fortran_nint(*v));
            }
            // `listio` recomputes `nb = 6 + n1h - nw` from what it read (`endf.f90:174`),
            // whatever the record stored: a continuation record read as a head gives
            // `6 + nint(word 5) - nw`.
            return Ok((6 + a[4] as i64 - w.len() as i64, w.clone()));
        }
        Ok((*nb, w.clone()))
    };
    let (nmt1d, mut nmd, mut mt1lst, mut nm) = (nmt1h, 0i32, 1000i32, 0i32);
    let mut out = Vec::new();
    if nmd >= nmt1d {
        return Ok(out);
    }
    loop {
        // label 220
        let (mut nb, w) = read(&mut pos, &mut a, true)?;
        let jg: i32;
        if ifissp - 1 != nm {
            jg = a[5] as i32;
        } else {
            let (mat1, mt1) = (a[2] as i32, a[3] as i32);
            let mta = mt1.wrapping_add(mat1.wrapping_mul(1000));
            if mta != mt1lst {
                nmd += 1;
            }
            if !(mt1 == 18 && mat1 == 0) {
                while nb != 0 {
                    nb = read(&mut pos, &mut a, false)?.0;
                }
                if nmd < nmt1d {
                    continue;
                }
                break;
            }
            mt1lst = mta;
            let nw = a[4] as i32;
            jg = a[5] as i32;
            let first = w.get(6).copied().unwrap_or(0.0);
            if !(nw == 1 && first == 0.0) {
                let nw = nw.max(0) as usize;
                let mut data: Vec<f64> = w.iter().skip(6).copied().collect();
                while data.len() < nw && nb > 0 {
                    let (b, more) = read(&mut pos, &mut a, false)?;
                    nb = b;
                    data.extend(more);
                }
                data.truncate(nw);
                if jg < 1 || jg as usize > nunion {
                    return Err(NjoyError::EndfParse(format!("errorr covout: row jg = {jg} off the union grid")));
                }
                out.push((jg as usize, data));
            }
        }
        // label 380
        if (jg as i64) < nunion as i64 {
            continue;
        }
        nm += 1;
        if nm == ifissp {
            break;
        }
        if nmt1h > 1 && nm < nmt1h {
            continue;
        }
        break;
    }
    Ok(out)
}

/// What [`run_mf35`] computed, and the tape `nout` holds.
#[derive(Debug, Clone)]
pub struct Mf35Result {
    /// The output covariance tape as NJOY writes it (`tpidio` label first).
    pub tape: GendfText,
    /// The union grid `un`.
    pub un: Vec<f64>,
    /// The user groups `egn`.
    pub egn: Vec<f64>,
    /// `ifissp` and `efmean` after the subsection scan.
    pub ifissp: usize,
    /// `efmean` (eV).
    pub efmean: f64,
    /// `sumchk`'s outcome per subsection (`true`: the zero-sum correction applied).
    pub zero_sum_corrected: Vec<bool>,
}

/// ERRORR with `mfcov = 35` on an ENDF tape and an input GENDF.
///
/// # Errors
/// [`NjoyError::NotPorted`] outside the scope in the module doc; parse errors for
/// malformed records; [`NjoyError::InvalidInput`] for an `ifissp`/`efmean` that
/// selects nothing.
pub fn run_mf35(endf: &Tape, gendf: &GendfTape, cfg: &Mf35Config) -> Result<Mf35Result, NjoyError> {
    let matd = cfg.matd;
    let iverf = endf_version(endf, matd)?;
    if iverf <= 4 {
        return Err(NjoyError::NotPorted("errorr: ENDF/B-IV covariance layout (iverf=4)"));
    }
    let cg = ngchk(gendf, matd, cfg.tempin)?;
    let mffis = if cg.section(6, 18).is_some() {
        6
    } else if cg.section(5, 18).is_some() {
        5
    } else {
        return Err(NjoyError::InvalidInput("errorr: mfcov=35 requested but no spectra present".into()));
    };
    let others = endf.sections().iter().filter(|s| s.key.mat == matd && s.key.mf == 35 && s.key.mt != 18).count();
    if others > 0 {
        return Err(NjoyError::NotPorted("errorr mfcov=35: an MF=35 MT other than 18"));
    }
    let (za, awr, subs) = mf35_subsections(endf, matd)?;
    let (ifissp, efmean, eclo, echi, igflag) = select_subsection(&subs, cfg.ifissp, cfg.efmean)?;

    // gridd, mfcov = 35: each subsection's NE energies into eni
    let mut eni: Vec<f64> = Vec::new();
    for s in &subs {
        let ne = s[5].round() as usize;
        merge(&s[6..6 + ne], &mut eni, 0.0, 0.0)?;
    }
    // egngpn
    let raw = if cfg.ign.abs() == 1 {
        cfg.user_egn.clone().ok_or_else(|| NjoyError::InvalidInput("errorr: ign=+-1 needs card 12".into()))?
    } else {
        neutron_group_structure(cfg.ign)?
    };
    let mut egn: Vec<f64> = raw.iter().map(|&x| sigfig(x, NDIG, 0)).collect();
    let un = uniong(&egn, &eni)?;
    if cfg.ign == -1 {
        egn = un.clone();
    }
    let nunion = un.len() - 1;
    let u = colaps(&cg, &un, mffis, &FissionWindow { eclo, echi, efmean, igflag })?;
    if (egn[0] - 1.0e-5).abs() <= 1.0e-9 {
        egn[0] = 1.0e-5;
    }

    // covcal
    let flx = u.flux(nunion)?;
    let chi = u.chi_vector()?;
    let mut corrected = Vec::new();
    let mut rows_by_sub = Vec::new();
    for s in &subs {
        let mut covl = s.clone();
        let nt = covl[4].round() as usize;
        covl[4] = 0.0;
        let spc = spcint(endf, matd, &covl)?;
        corrected.push(sumchk(&mut covl, &spc)?);
        covbin(&mut covl);
        for w in covl.iter_mut().skip(6).take(nt) {
            *w = sigfig(*w, NDIG, 0);
        }
        rows_by_sub.push(lb7_rows(&covl, &un, &flx));
    }

    // fssigc
    let egt: Vec<f64> = un.iter().map(|&x| sigfig(x, NDIG, 0)).collect();
    let egn: Vec<f64> = egn.iter().map(|&x| sigfig(x, NDIG, 0)).collect();
    let ngn = egn.len() - 1;
    let inside = |x: f64, ig: usize| !(x < egn[ig] || x >= egn[ig + 1]);
    let mut cflx = vec![0.0f64; ngn];
    let mut csig = vec![0.0f64; ngn];
    for ig in 0..ngn {
        for jg in 0..nunion {
            if inside(egt[jg], ig) {
                cflx[ig] += flx[jg];
            }
        }
        for jg in 0..nunion {
            if inside(egt[jg], ig) {
                csig[ig] += chi[jg] * flx[jg];
            }
        }
        csig[ig] /= cflx[ig];
    }

    // covout, the ifissp-th subsection
    let mut cova = vec![0.0f64; ngn * ngn]; // cova(igp, ig) at [ig * ngn + igp]
    let (ek1, ek2) = (1.0e-10, 1.0e10);
    let stream = scratch_records(&rows_by_sub);
    let picked = covout_rows(&stream, subs.len() as i32, ifissp as i32, nunion)?;
    for (jg, cov) in &picked {
        let nw = cov.len();
        if nw == 1 && cov[0] == 0.0 {
            continue;
        }
        let egtjg = egt[jg - 1];
        let ig = (0..ngn).find(|&i| inside(egtjg, i)).unwrap_or(ngn - 1);
        if egtjg >= egn[ngn] {
            continue;
        }
        if !(egtjg >= ek1 && egtjg < ek2) {
            continue;
        }
        let mut igp = 0usize;
        for (jgp0, &c) in cov.iter().enumerate() {
            if c == 0.0 {
                continue;
            }
            let x = egt[jgp0];
            if !(x >= ek1 && x < ek2) {
                continue;
            }
            let mut found = true;
            while !inside(x, igp) {
                if igp + 1 == ngn {
                    found = false;
                    break;
                }
                igp += 1;
            }
            if found {
                cova[ig * ngn + igp] += 1.0 * 1.0 * c;
            }
        }
    }

    // the tape: tpid, MF=1, MF=5 (fssigc), MF=35 (covout)
    let mut t = GendfText::label(&cg.tpid);
    t.begin(matd, 1, 451);
    t.cont([za, awr], [iverf, 0, -12, 0]);
    t.list([u.tempin, 0.0], ngn as i32, 0, 0, &egn);
    t.send();
    t.fend();
    t.begin(matd, 5, 18);
    t.list([0.0, efmean], 0, 0, 0, &csig);
    t.send();
    t.fend();
    t.begin(matd, 35, 18);
    t.cont([za, awr], [0, 0, 0, 1]);
    t.cont([0.0, 0.0], [0, 18, 0, ngn as i32]);
    const EPS: f64 = 1.0e-20;
    for ig in 0..ngn {
        let mut vals: Vec<f64> = Vec::with_capacity(ngn);
        let (mut ig2lo, mut ng2) = (0usize, 0usize);
        for igp in 0..ngn {
            let mut v = cova[ig * ngn + igp] / (cflx[ig] * cflx[igp]);
            if v != 0.0 && cfg.irelco != 0 {
                let denom = csig[ig] * csig[igp];
                v = if denom > 0.0 { v / denom.max(EPS) * (egn[ig + 1] - egn[ig]) * (egn[igp + 1] - egn[igp]) } else { 0.0 };
            }
            if v.abs() <= EPS {
                if ig2lo == 0 {
                    vals.clear();
                    vals.push(v);
                } else {
                    vals.push(v);
                }
            } else {
                if ig2lo == 0 {
                    ig2lo = igp + 1;
                    vals.clear();
                }
                vals.push(v);
                ng2 = igp + 1;
            }
        }
        if ng2 != 0 || ig + 1 >= ngn {
            if ng2 == 0 {
                ig2lo = ig + 1;
                ng2 = ig + 1;
            }
            let n = ng2 - ig2lo + 1;
            vals.truncate(n);
            t.list([0.0, 0.0], n as i32, ig2lo as i32, (ig + 1) as i32, &vals);
        }
    }
    t.send();
    t.fend();
    t.mend();
    t.tend();
    Ok(Mf35Result { tape: t, un, egn, ifissp, efmean, zero_sum_corrected: corrected })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `covbin` divides by both bin widths; `sumchk` leaves a zero-sum matrix
    /// alone and corrects one that is not; the subsection scan takes `ifissp`'s
    /// mid-energy; LB=7 rows carry `flx(jg) flx(jh)`.
    #[test]
    fn mf35_kernels_follow_upstream() {
        // NE = 3: energies 1, 2, 4; triangle (1,1) (1,2) (2,2)
        let mut covl = vec![1.0, 4.0, 1.0, 7.0, 6.0, 3.0, 1.0, 2.0, 4.0, 0.5, -0.5, 0.5];
        covbin(&mut covl);
        assert_eq!(&covl[9..], &[0.5, -0.25, 0.125]);
        let mut zs = vec![1.0, 4.0, 1.0, 7.0, 6.0, 3.0, 1.0, 2.0, 4.0, 0.5, -0.5, 0.5];
        assert!(!sumchk(&mut zs, &[0.5, 0.5]).unwrap());
        let mut nz = vec![1.0, 4.0, 1.0, 7.0, 6.0, 4.0, 1.0, 2.0, 3.0, 4.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0];
        assert!(sumchk(&mut nz, &[0.5, 0.25, 0.25]).unwrap());
        let (i, ef, _, _, _) = select_subsection(&[vec![0.0, 4.0]], 1, 2.0e6).unwrap();
        assert_eq!((i, ef), (1, 2.0));
        let (i, ef, _, _, g) = select_subsection(&[vec![0.0, 4.0]], -1000, 2.0e6).unwrap();
        assert_eq!(g, 0);
        assert_eq!((i, ef), (1, 2.0));
        let rows = lb7_rows(&covl, &[1.0, 2.0, 4.0], &[2.0, 3.0]);
        assert_eq!(rows[0], (1, vec![0.5 * 4.0, -0.25 * 6.0]));
        assert_eq!(rows[1], (2, vec![-0.25 * 6.0, 0.125 * 9.0]));
    }
}
