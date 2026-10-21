//! Faithful port of `acelf5` (`acefc.f90:6684-7137`): an ENDF MF=5 section to
//! its complete ACE **DLW** entry, law-applicability table included.
//!
//! # Why this replaced `parse_mf5_law4` in the ACE builder
//!
//! The builder wrote MF=5 through [`super::energy::parse_mf5_law4`] behind the
//! generic 9-word header (`E_lo..E_hi` of the whole grid, probability 1).
//! Upstream writes each subsection's own `p(E)` table as the header, rounds the
//! outgoing energies with `sigfig(E'/1e6, 7)`, and for fission (`ismooth`)
//! supplements the tail above 10 MeV. Measured 2026-09-26 against NJOY2016's
//! U-234 table: the fission spectra's outgoing energies were written unrounded
//! (`7.11416256903e-5` against NJOY's `7.114163e-5`), 35 747 of 113 139 DLW
//! words differing.
//!
//! # Scope
//!
//! Ported: `LF = 1` (with the MT=18 exponential supplement), `7`, `9`, `12`
//! (Madland-Nix, through the verified [`super::energy::madland_nix::fmn`]),
//! and `11` when both of its TAB1s are a single lin-lin region. Refused:
//! `LF = 5`, which upstream refuses too (`'sorry. acer cannot handle lf=5.'`),
//! and multi-region `LF = 11`, whose upstream interpolation-table writes
//! overlap the data they precede (`acefc.f90:7015-7022` index with `n` where
//! the table has `m` entries) -- reproducing that is not something to do
//! without a case that exercises it.

use super::acelf6::{Acelf6Tyr, DlwEntry, DlwWord};
use crate::endf::interp::{terp1, IntLaw};
use crate::endf::records::SectionCursor;
use crate::endf::tape::Section;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

const EMEV: f64 = 1.0e6;
const RMIN: f64 = 1.0e-30;

/// `acelf5` for reaction `mt`. `Ok(None)` for what is refused (see the module
/// doc); `tyr` is the TYR the caller has already decided (19 for fission).
///
/// # Errors
/// Malformed records, `LF = 5` (as upstream), or an illegal `LF`.
pub fn acelf5(
    section: &Section,
    mt: i32,
    q_mev: f64,
    tyr: i32,
    ismooth: bool,
) -> Result<Option<DlwEntry>, NjoyError> {
    let mut cur = SectionCursor::new(&section.rows);
    let head = cur.read_cont()?;
    let nk = head.n1;
    let mut w: Vec<DlwWord> = Vec::new();
    let mut last: Option<usize> = None;
    let mut first_law = 4;
    for k in 0..nk {
        let pt = cur.read_tab1()?;
        let u = pt.head.c1;
        let lf = pt.head.l2;
        if let Some(l) = last {
            w[l] = DlwWord::Loc(w.len());
        }
        let l0 = w.len();
        last = Some(l0);
        let law = if lf == 1 || lf == 12 { 4 } else { lf };
        if k == 0 {
            first_law = law;
        }
        w.push(DlwWord::Int(0));
        w.push(DlwWord::Int(i64::from(law)));
        w.push(DlwWord::Int(0)); // IDAT, set below
        push_interp_raw(&mut w, &pt.interp);
        w.push(DlwWord::Int(pt.pairs.len() as i64));
        for &(x, _) in &pt.pairs {
            w.push(DlwWord::Real(sigfig(x / EMEV, 7, 0)));
        }
        for &(_, p) in &pt.pairs {
            w.push(DlwWord::Real(p));
        }
        w[l0 + 2] = DlwWord::Loc(w.len());

        match lf {
            1 => {
                let t2 = cur.read_tab2()?;
                push_interp_capped(&mut w, &t2.interp);
                let ne = t2.head.n2 as usize;
                w.push(DlwWord::Int(ne as i64));
                let e_at = w.len();
                for _ in 0..ne {
                    w.push(DlwWord::Real(0.0));
                }
                let l_at = w.len();
                for _ in 0..ne {
                    w.push(DlwWord::Int(0));
                }
                for j in 0..ne {
                    let t1 = cur.read_tab1()?;
                    let e = t1.head.c2;
                    w[e_at + j] = DlwWord::Real(sigfig(e / EMEV, 7, 0));
                    w[l_at + j] = DlwWord::Loc(w.len());
                    let jnt = t1.interp.last().map_or(2, |&(_, i)| i as i32);
                    let mut pairs = t1.pairs.clone();
                    drop_leading_zeros(&mut pairs, t1.interp.first().map_or(2, |&(_, i)| i), mt)?;
                    if ismooth && mt == 18 && jnt == 2 {
                        supplement_tail(&mut pairs)?;
                    }
                    tabulated_block(&mut w, jnt, &mut pairs, e, q_mev);
                }
            }
            5 => {
                return Err(NjoyError::EndfParse(
                    "acelf5: sorry. acer cannot handle lf=5. \
                     you will have to patch the evaluation to use lf=1."
                        .into(),
                ))
            }
            7 | 9 => {
                let th = cur.read_tab1()?;
                push_interp_raw(&mut w, &th.interp);
                w.push(DlwWord::Int(th.pairs.len() as i64));
                for &(x, _) in &th.pairs {
                    w.push(DlwWord::Real(sigfig(x / EMEV, 7, 0)));
                }
                for &(_, y) in &th.pairs {
                    w.push(DlwWord::Real(sigfig(y / EMEV, 7, 0)));
                }
                // Unrounded, as upstream (`xss(next)=u/emeV`).
                w.push(DlwWord::Real(u / EMEV));
            }
            11 => {
                let a = cur.read_tab1()?;
                let b = cur.read_tab1()?;
                let simple = |t: &crate::endf::records::Tab1| {
                    t.interp.len() == 1 && t.interp[0].1 == 2
                };
                if !simple(&a) || !simple(&b) {
                    return Ok(None);
                }
                w.push(DlwWord::Int(0));
                w.push(DlwWord::Int(a.pairs.len() as i64));
                for &(x, _) in &a.pairs {
                    w.push(DlwWord::Real(sigfig(x / EMEV, 7, 0)));
                }
                for &(_, y) in &a.pairs {
                    w.push(DlwWord::Real(sigfig(y / EMEV, 7, 0)));
                }
                w.push(DlwWord::Int(0));
                w.push(DlwWord::Int(b.pairs.len() as i64));
                for &(x, _) in &b.pairs {
                    w.push(DlwWord::Real(sigfig(x / EMEV, 7, 0)));
                }
                for &(_, y) in &b.pairs {
                    w.push(DlwWord::Real(sigfig(y * EMEV, 7, 0)));
                }
                w.push(DlwWord::Real(sigfig(u / EMEV, 7, 0)));
            }
            12 => {
                let t = cur.read_tab1()?;
                madland_nix_blocks(&mut w, &t)?;
            }
            other => {
                return Err(NjoyError::EndfParse(format!("acelf5: illegal lf={other}")));
            }
        }
    }
    Ok(Some(DlwEntry {
        words: w,
        yield_len: 0,
        tyr: Acelf6Tyr::Fixed(tyr),
        law: first_law,
        correlated: false,
    }))
}

/// `topfil`'s MF=5 `LF = 1` clean-up (`acefc.f90:2168-2215`), applied to
/// every outgoing spectrum before `acelf5` reads it: a run of leading
/// `f(E') = 0` pairs is collapsed -- to one zero pair for a non-histogram
/// first region, to none for a histogram one. (The NBT shift upstream makes
/// alongside only matters for the region lookup, which reads only the last
/// region's law here.)
fn drop_leading_zeros(pairs: &mut Vec<(f64, f64)>, first_int: u32, mt: i32) -> Result<(), NjoyError> {
    if pairs.first().map_or(true, |p| p.1 != 0.0) {
        return Ok(());
    }
    let start = if first_int != 1 { 1 } else { 0 };
    let mut npe = 0usize;
    let mut k = start;
    while k < pairs.len() && pairs[k].1 == 0.0 {
        k += 1;
        npe += 1;
    }
    if k >= pairs.len() {
        return Err(NjoyError::EndfParse(format!(
            "topfil: mf=5,mt={mt}, entire tab1 function is zero."
        )));
    }
    pairs.drain(0..npe);
    Ok(())
}

/// `[m, NBT(m), INT(m)]` as stored, or `[0]` for one lin-lin region.
fn push_interp_raw(w: &mut Vec<DlwWord>, interp: &[(u32, u32)]) {
    let jnt = interp.first().map_or(2, |&(_, i)| i);
    if interp.len() != 1 || jnt != 2 {
        w.push(DlwWord::Int(interp.len() as i64));
        w.extend(interp.iter().map(|&(n, _)| DlwWord::Int(i64::from(n))));
        w.extend(interp.iter().map(|&(_, i)| DlwWord::Int(i64::from(i))));
    } else {
        w.push(DlwWord::Int(0));
    }
}

/// As [`push_interp_raw`] with each law taken `mod 10` and capped at 2.
fn push_interp_capped(w: &mut Vec<DlwWord>, interp: &[(u32, u32)]) {
    let cap = |i: u32| (i % 10).min(2);
    let jnt = interp.first().map_or(2, |&(_, i)| cap(i));
    if interp.len() != 1 || jnt != 2 {
        w.push(DlwWord::Int(interp.len() as i64));
        w.extend(interp.iter().map(|&(n, _)| DlwWord::Int(i64::from(n))));
        w.extend(interp.iter().map(|&(_, i)| DlwWord::Int(i64::from(cap(i)))));
    } else {
        w.push(DlwWord::Int(0));
    }
}

/// The MT=18 tail supplement (`acefc.f90:6816-6851`): above 9.99 MeV, any
/// panel wider than 200 keV gets four interior points on the log-linear
/// (exponential) shape between its ends.
///
/// Upstream's `ta11` save/restore never restores: it saves the value only when
/// it is zero, then tests `ta11 /= 0`. So a zero at the panel's upper end is
/// **permanently** replaced by `1e-6` times the lower end's density. Kept.
fn supplement_tail(pairs: &mut Vec<(f64, f64)>) -> Result<(), NjoyError> {
    let mut ix = 0usize; // 0-based upstream `ix - 1`
    while ix + 1 < pairs.len() {
        if pairs[ix].0 < 9.99e6 {
            ix += 1;
            continue;
        }
        let dele = pairs[ix + 1].0 - pairs[ix].0;
        if dele > 2.0e5 {
            let lo = pairs[ix];
            let mut hi = pairs[ix + 1];
            if hi.1 == 0.0 {
                hi.1 = 1.0e-6 * lo.1;
                pairs[ix + 1].1 = hi.1;
            }
            let mut add = Vec::with_capacity(4);
            for ixx in 1..=4 {
                let x = lo.0 + ixx as f64 * dele / 5.0;
                add.push((x, terp1(lo.0, lo.1, hi.0, hi.1, x, IntLaw::LogLin)?));
            }
            pairs.splice(ix + 1..ix + 1, add);
            ix += 5;
        } else {
            ix += 1;
        }
    }
    Ok(())
}

/// One incident energy's `[INTT, N, E', pdf, cdf]` (`acefc.f90:6852-6884`).
fn tabulated_block(w: &mut Vec<DlwWord>, jnt: i32, pairs: &mut [(f64, f64)], e: f64, q: f64) {
    let n = pairs.len();
    w.push(DlwWord::Int(i64::from(jnt)));
    w.push(DlwWord::Int(n as i64));
    for ki in 0..n {
        if pairs[ki].0 > e && q < 0.0 {
            pairs[ki].0 = e - (n - 1 - ki) as f64 * 1000.0;
        }
    }
    let eo: Vec<f64> = pairs.iter().map(|&(x, _)| sigfig(x / EMEV, 7, 0)).collect();
    let mut pdf: Vec<f64> = pairs
        .iter()
        .map(|&(_, g)| {
            let v = sigfig(g * EMEV, 7, 0);
            if v < RMIN {
                0.0
            } else {
                v
            }
        })
        .collect();
    let mut cdf = vec![0.0_f64; n];
    for ki in 1..n {
        let (x0, y0) = pairs[ki - 1];
        let (x1, y1) = pairs[ki];
        if jnt == 1 {
            cdf[ki] = cdf[ki - 1] + y0 * (x1 - x0);
        }
        if jnt == 2 {
            cdf[ki] = cdf[ki - 1] + ((y0 + y1) / 2.0) * (x1 - x0);
        }
    }
    let renorm = if cdf[n - 1] != 0.0 { 1.0 / cdf[n - 1] } else { 1.0 };
    for k in 0..n {
        pdf[k] = sigfig(pdf[k] * renorm, 7, 0);
        cdf[k] = sigfig(cdf[k] * renorm, 9, 0);
    }
    w.extend(eo.into_iter().map(DlwWord::Real));
    w.extend(pdf.into_iter().map(DlwWord::Real));
    w.extend(cdf.into_iter().map(DlwWord::Real));
}

/// `LF = 12` (`acefc.f90:7037-7126`): adaptive linearisation of the
/// Madland-Nix shape onto a law-4 table per incident energy.
fn madland_nix_blocks(
    w: &mut Vec<DlwWord>,
    t: &crate::endf::records::Tab1,
) -> Result<(), NjoyError> {
    use super::energy::madland_nix::fmn;
    const ISMAX: usize = 20;
    const JSMAX: usize = 500;
    const TOL: f64 = 0.02;
    const TMIN: f64 = 1.0e-12;
    let (efl, efh) = (t.head.c1, t.head.c2);
    let n = t.pairs.len();
    let emin = t.pairs[0].0;
    let emax = t.pairs[n - 1].0;
    w.push(DlwWord::Int(0));
    w.push(DlwWord::Int(n as i64));
    let e_at = w.len();
    for _ in 0..n {
        w.push(DlwWord::Real(0.0));
    }
    let l_at = w.len();
    for _ in 0..n {
        w.push(DlwWord::Int(0));
    }
    for ie in 0..n {
        let (tme, tmt) = t.pairs[ie];
        let mut xs = [0.0_f64; ISMAX + 1];
        let mut ys = [0.0_f64; ISMAX + 1];
        let mut xxs: Vec<f64> = Vec::new();
        let mut yys: Vec<f64> = Vec::new();
        let mut renorm = 0.0;
        let mut is = 3usize;
        xs[3] = emin;
        ys[3] = fmn(emin, efl, efh, tmt)?;
        xs[2] = EMEV;
        ys[2] = fmn(xs[2], efl, efh, tmt)?;
        xs[1] = emax;
        ys[1] = fmn(emax, efl, efh, tmt)?;
        while is > 0 {
            let mut dy = 0.0;
            let mut test = 1.0;
            let (mut xm, mut yt) = (0.0, 0.0);
            if is > 1 && is < ISMAX {
                xm = (xs[is - 1] + xs[is]) / 2.0;
                let ym = (ys[is - 1] + ys[is]) / 2.0;
                yt = fmn(xm, efl, efh, tmt)?;
                test = TOL * yt.abs() + TMIN;
                dy = (yt - ym).abs();
            }
            if dy > test {
                is += 1;
                xs[is] = xs[is - 1];
                ys[is] = ys[is - 1];
                xs[is - 1] = xm;
                ys[is - 1] = yt;
            } else {
                if xxs.len() < JSMAX {
                    xxs.push(xs[is]);
                    yys.push(ys[is]);
                } else {
                    // `if (js.gt.jsmax) js=jsmax`: the last slot is overwritten.
                    *xxs.last_mut().unwrap() = xs[is];
                    *yys.last_mut().unwrap() = ys[is];
                }
                let js = xxs.len();
                if js > 1 {
                    renorm += (xxs[js - 1] - xxs[js - 2]) * (yys[js - 1] + yys[js - 2]) / 2.0;
                }
                is -= 1;
            }
        }
        let renorm = 1.0 / renorm;
        w[e_at + ie] = DlwWord::Real(sigfig(tme / EMEV, 7, 0));
        w[l_at + ie] = DlwWord::Loc(w.len());
        let js = xxs.len();
        w.push(DlwWord::Int(2));
        w.push(DlwWord::Int(js as i64));
        let mut cdf = vec![0.0_f64; js];
        for ki in 1..js {
            cdf[ki] = cdf[ki - 1] + renorm * (yys[ki] + yys[ki - 1]) * (xxs[ki] - xxs[ki - 1]) / 2.0;
        }
        w.extend(xxs.iter().map(|&x| DlwWord::Real(sigfig(x / EMEV, 7, 0))));
        w.extend(yys.iter().map(|&y| DlwWord::Real(sigfig(y * renorm * EMEV, 7, 0))));
        w.extend(cdf.into_iter().map(DlwWord::Real));
    }
    Ok(())
}
