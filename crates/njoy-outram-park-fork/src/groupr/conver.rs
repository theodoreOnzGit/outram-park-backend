// Translated from NJOY2016.80 `src/groupr.f90`, subroutine `conver`: the MF=3
// threshold table (:10994-11007), the MF=12 `LO = 2` conversion (:11188-11440) and
// the MF=14 rewrite under `l2flg` (:11185, :11442-11470).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **`conver`'s photon rewrite** (gh:#1517): the tape `nend2` that GROUPR's
//! `getgyl` and `getgfl` read, with every MF=12 `LO = 2` transition array turned
//! into `LO = 1` yields and, once any was, the discrete-level MF=14 sections made
//! isotropic with one photon per converted line.
//!
//! The transition-matrix algebra is ACER's [`crate::acer::photon_blocks::Lo2Cascade`]
//! (`acefc.f90`'s `convr`), which is `conver`'s line for line: the same `mt0`
//! series, the same `1e-4` level match and the same descending exchange sort. Only
//! the starting level energies differ: `conver` takes them from the ENDF tape's own
//! MF=3 thresholds (`gety1` at zero, `:10997`), times `awr/(awr+1)` (`:11303`).
//!
//! What `conver` writes for one converted section (`:11385-11437`): a HEAD with
//! `LO = 1`, `NK = l`; for `l > 1` a total-yield TAB1 that is constant at `ysum`
//! from `elow` to `etop`; then one TAB1 per line, `C1 = E_gamma`, `C2 = E_level`,
//! `LF = 2`, constant at its yield. `etop` is `emaxx`, `elow` the MF=3 threshold of
//! the section's MT (else `ebeg = 1e-9`). When `nendf` is a text unit, `nend2` is too
//! (`:402`), so every word GROUPR reads back went through `a11`.
//!
//! Not translated (refused by name): ENDF-5 and earlier tapes (`iverf <= 5`), whose
//! `mt0` series for MT >= 700 and MAT 1149/1150 patches `Lo2Cascade` does not carry.

use crate::endf::records::{SectionCursor, Tab1};
use crate::endf::tape::{Section, Tape};
use crate::NjoyError;

/// `ebeg = 10*small` (`groupr.f90:391`).
const EBEG: f64 = 1.0e-9;

/// `[c1, c2, l1, l2, n1, n2]` as a tape row.
fn cont_row(c1: f64, c2: f64, l1: i32, l2: i32, n1: i32, n2: i32) -> [f64; 6] {
    [c1, c2, f64::from(l1), f64::from(l2), f64::from(n1), f64::from(n2)]
}

/// `tab1io` of a TAB1 with one lin-lin region and two points, `y` constant, as
/// `conver` writes them (`:11410-11436`).
fn constant_tab1(rows: &mut Vec<[f64; 6]>, c1: f64, c2: f64, l2: i32, elow: f64, etop: f64, y: f64) {
    rows.push(cont_row(c1, c2, 0, l2, 1, 2));
    rows.push([2.0, 2.0, 0.0, 0.0, 0.0, 0.0]);
    rows.push([elow, y, etop, y, 0.0, 0.0]);
}

/// A value as GROUPR reads it back from `nend2`: through `a11` when the unit is
/// text, unchanged when it is binary.
fn through_unit(x: f64, text: bool) -> f64 {
    if text {
        crate::endf::parse::parse_endf_float(&crate::endf::parse::format_endf_float(x)).unwrap_or(x)
    } else {
        x
    }
}

/// The first MT of each discrete-level series `conver` makes isotropic (`:11443-11454`,
/// `iverf >= 6`), or `None` outside them.
fn level_series(mt: i32) -> Option<i32> {
    match mt {
        50..=90 => Some(50),
        600..=648 => Some(600),
        650..=698 => Some(650),
        700..=748 => Some(700),
        750..=798 => Some(750),
        800..=848 => Some(800),
        875..=890 => Some(875),
        _ => None,
    }
}

/// `nend2` for material `mat` of `nend`: `None` when no MF=12 section has `LO = 2`
/// (then `conver` changes nothing GROUPR's photon feed reads), else the tape with
/// the conversions applied. `text` is `nendf > 0`.
///
/// # Errors
/// [`NjoyError::NotPorted`] for `iverf <= 5`; `'too many lo=2 gammas'` past 500
/// lines; record errors.
pub fn lo2_converted(nend: &Tape, mat: i32, iverf: i32, emaxx: f64, text: bool) -> Result<Option<Tape>, NjoyError> {
    let has_lo2 = nend.sections().iter().any(|s| s.key.mat == mat && s.key.mf == 12 && s.rows.first().is_some_and(|r| r[2] == 2.0));
    if !has_lo2 {
        return Ok(None);
    }
    if iverf <= 5 {
        return Err(NjoyError::NotPorted("groupr conver: MF=12 LO = 2 on an ENDF-5 or older tape"));
    }
    // `:10994-11007`: `gety1(0)` of every MF=3 section, in tape order.
    let mut thresholds: Vec<(i32, f64)> = Vec::new();
    for s in nend.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 3 && s.key.mt != 0) {
        let mut cur = SectionCursor::new(&s.rows);
        let _ = cur.read_cont()?;
        let t: Tab1 = cur.read_tab1()?;
        let mut g = crate::endf::gety1::Gety1::new(&t);
        thresholds.push((s.key.mt, g.get(0.0).xnext));
    }
    // `do i=1,nnth: if (mtth(i).eq.mth) elow=eeth(i)`: the last match.
    let threshold = |mt: i32| thresholds.iter().rev().find(|(m, _)| *m == mt).map(|&(_, e)| e);
    let mut cascade = crate::acer::photon_blocks::Lo2Cascade::new();
    let mut ngam = [0i32; 440];
    let mut l2flg = false;
    let mut out = Vec::with_capacity(nend.len());
    for s in nend.sections() {
        let (mf, mt) = (s.key.mf, s.key.mt);
        if s.key.mat != mat {
            out.push(s.clone());
            continue;
        }
        if mf == 12 && s.rows.first().is_some_and(|r| r[2] == 2.0) {
            let mut cur = SectionCursor::new(&s.rows);
            let head = cur.read_cont()?;
            let (za, awr, lg) = (head.c1, head.c2, head.l2);
            l2flg = true;
            let list = cur.read_list()?;
            let level = |m: i32| threshold(m).map(|e| e * awr / (awr + 1.0));
            let res = cascade
                .section(mt, lg, &list, &level)
                .ok_or(NjoyError::NotPorted("groupr conver: an MF=12 LO = 2 MT outside the level series"))?;
            let photons = res.map(|o| (o.ysum, o.photons)).unwrap_or((0.0, Vec::new()));
            let l = photons.1.len();
            if l > 500 {
                return Err(NjoyError::InvalidInput("conver: too many lo=2 gammas".into()));
            }
            let mtl = if mt >= 600 { 549 } else { 49 };
            let slot = (mt - mtl) as usize;
            if slot >= ngam.len() {
                return Err(NjoyError::InvalidInput(format!("conver: MT {mt} outside ngam")));
            }
            ngam[slot] = l as i32;
            let etop = through_unit(emaxx, text);
            let elow = through_unit(threshold(mt).unwrap_or(EBEG), text);
            let mut rows = vec![cont_row(za, awr, 1, 0, l as i32, 0)];
            if l > 1 {
                constant_tab1(&mut rows, 0.0, 0.0, 0, elow, etop, through_unit(photons.0, text));
            }
            for &(eg, es, y) in &photons.1 {
                let y = through_unit(y, text);
                constant_tab1(&mut rows, through_unit(eg, text), through_unit(es, text), 2, elow, etop, y);
            }
            out.push(Section { key: s.key, rows });
            continue;
        }
        if mf == 14 && l2flg {
            if level_series(mt).is_some() {
                let head = s.rows[0];
                let mtl = if mt >= 600 { 549 } else { 49 };
                let nk = ngam.get((mt - mtl) as usize).copied().unwrap_or(0);
                out.push(Section { key: s.key, rows: vec![cont_row(head[0], head[1], 1, 0, nk, 0)] });
                continue;
            }
        }
        out.push(s.clone());
    }
    Ok(Some(Tape::from_sections(nend.tpid.clone(), out)))
}

/// `conver`'s File-1 pass (`groupr.f90:11473-11550`): when MF=1 has MT=452 but
/// neither MT=455 nor MT=456, a copy of MT=452 is added as MT=456 (`L2 = LNU`,
/// the rest of the HEAD zero), so that `getyld(.., 1, 456, ..)` finds a prompt
/// nu. `None` when nothing is added. The HEAD's `C1`, `C2` are upstream's stale
/// `scr` words (the next section's); GROUPR reads only `L2` of it, so they are
/// set from MT=452's here.
pub fn with_mt456(nend: &Tape, mat: i32) -> Option<Tape> {
    let s452 = nend.section(mat, 1, 452)?;
    if nend.section(mat, 1, 455).is_some() || nend.section(mat, 1, 456).is_some() {
        return None;
    }
    let h = s452.rows[0];
    let mut rows = vec![cont_row(h[0], h[1], 0, h[3].round() as i32, 0, 0)];
    rows.extend_from_slice(&s452.rows[1..]);
    let mut secs: Vec<Section> = Vec::new();
    for s in nend.sections() {
        secs.push(s.clone());
        if s.key.mat == mat && s.key.mf == 1 && s.key.mt == 452 {
            let mut key = s.key;
            key.mt = 456;
            secs.push(Section { key, rows: rows.clone() });
        }
    }
    let mut t = Tape::from_sections(nend.tpid.clone(), secs);
    t.copy_raw_mf32_from(nend);
    Some(t)
}

#[cfg(test)]
mod mt456_tests {
    use super::*;

    #[test]
    fn mt452_is_copied_to_456_only_without_455_and_456() {
        let k = |mt| crate::endf::EndfKey { mat: 1050, mf: 1, mt };
        let s452 = Section { key: k(452), rows: vec![[94238.0, 236.0, 0.0, 1.0, 0.0, 0.0], [2.0, 0.0, 0.0, 0.0, 1.0, 0.0]] };
        let t = Tape::from_sections(String::new(), vec![s452.clone()]);
        let t2 = with_mt456(&t, 1050).unwrap();
        let s456 = t2.section(1050, 1, 456).unwrap();
        assert_eq!(s456.rows[0][3], 1.0);
        assert_eq!(s456.rows[1], s452.rows[1]);
        let s455 = Section { key: k(455), rows: s452.rows.clone() };
        assert!(with_mt456(&Tape::from_sections(String::new(), vec![s452, s455]), 1050).is_none());
    }
}
