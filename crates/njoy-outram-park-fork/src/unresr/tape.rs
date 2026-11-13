// Translated from NJOY2016.80 `src/unresr.f90`: the driver `unresr`
// (`:136-330`, the table and the MF=2/MT=152 record), `rdunf2`'s energy nodes
// (`:426-748`) and `rdunf3` (`:783-840`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **UNRESR's MF=2/MT=152 record** (gh:#1369 round 4): the self-shielded
//! unresolved table GROUPR's `stounr` reads.
//!
//! The physics per energy is [`super::unresolved_cross_sections`] (`unresl`).
//! This file adds what was missing for a tape: `rdunf2`'s energy grid
//! ([`unresr_grid`]), `rdunf3`'s File-3 background on it, the record layout
//! (`:178-200`, `sigfig(sigu, 7, 0)`), and the rewrite of each temperature's
//! MT=152 on the PENDF.
//!
//! UNRESR's grid is not PURR's (`purr::tables::urr_energy_grid`, `rdf2un`): the
//! primer and refinement limit are 1 MeV, nodes are `sigfig(., 7, 0)`, the
//! `egridu` step test is `enut > 1.01 et`, and a final pass drops nodes within
//! `sigfig(., 7, 2)` of the last kept one and signs those below the resolved
//! range's top negative (overlap).
//!
//! Not translated (refused by name): a PENDF with no MT=152 yet (`new = 1`,
//! which inserts a dictionary entry, `:239-270`).

use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::mixr::mix::sigfig;
use crate::unresr::mf2::{UnresolvedCase, UnresolvedRange};
use crate::NjoyError;

/// `egridu`, `unresr.f90:405-417`.
const EGRIDU: [f64; 78] = [
    1.0e1, 1.25e1, 1.5e1, 1.7e1, 2.0e1, 2.5e1, 3.0e1, 3.5e1, 4.0e1, 5.0e1, 6.0e1, 7.2e1, 8.5e1, 1.0e2, 1.25e2,
    1.5e2, 1.7e2, 2.0e2, 2.5e2, 3.0e2, 3.5e2, 4.0e2, 5.0e2, 6.0e2, 7.2e2, 8.5e2, 1.0e3, 1.25e3, 1.5e3, 1.7e3,
    2.0e3, 2.5e3, 3.0e3, 3.5e3, 4.0e3, 5.0e3, 6.0e3, 7.2e3, 8.5e3, 1.0e4, 1.25e4, 1.5e4, 1.7e4, 2.0e4, 2.5e4,
    3.0e4, 3.5e4, 4.0e4, 5.0e4, 6.0e4, 7.2e4, 8.5e4, 1.0e5, 1.25e5, 1.5e5, 1.7e5, 2.0e5, 2.5e5, 3.0e5, 3.5e5,
    4.0e5, 5.0e5, 6.0e5, 7.2e5, 8.5e5, 1.0e6, 1.25e6, 1.5e6, 1.7e6, 2.0e6, 2.5e6, 3.0e6, 3.5e6, 4.0e6, 5.0e6,
    6.0e6, 7.2e6, 8.5e6,
];

/// `rdunf2`'s energy nodes (`unresr.f90:426-748`) over every LRU=2 range of
/// the material, in file order, with `elr` the top of the resolved ranges.
/// Returns `(eunr, intunr, lssf)`: energies below `elr` are negative.
pub fn unresr_grid(ranges: &[UnresolvedRange], elr: f64) -> (Vec<f64>, i32, i32) {
    const ONEMEV: f64 = 1.0e6;
    const WIDE: f64 = 1.26;
    const STEP: f64 = 1.01;
    let mut eunr = vec![ONEMEV];
    let (mut intunr, mut lssf, mut indep) = (0, 0, false);
    for r in ranges {
        // `el`, `eh` are already `sigfig(., 7, 0)` (`parse_lru2_ranges`).
        for et in [sigfig(r.el, 7, -1), sigfig(r.el, 7, 1), sigfig(r.eh, 7, -1), sigfig(r.eh, 7, 1)] {
            crate::unresr::mf2::ilist(et, &mut eunr);
        }
        lssf = r.lssf;
        match &r.case_ {
            UnresolvedCase::CaseA { .. } => {
                intunr = 2;
                indep = true;
            }
            UnresolvedCase::CaseB { fission_energies, .. } => {
                intunr = 2;
                // `:585-594`: every energy but the first.
                for &e in fission_energies.iter().skip(1) {
                    crate::unresr::mf2::ilist(e, &mut eunr);
                }
            }
            UnresolvedCase::CaseC { l_states, .. } => {
                for (l, ls) in l_states.iter().enumerate() {
                    for (j, js) in ls.j_states.iter().enumerate() {
                        intunr = js.int_;
                        let ne = js.points.len();
                        for (n, p) in js.points.iter().enumerate() {
                            if n != 0 && n + 1 != ne && l == 0 && j == 0 {
                                crate::unresr::mf2::ilist(p.e, &mut eunr);
                            }
                        }
                    }
                }
            }
        }
    }
    if eunr.len() <= 1 {
        return (Vec::new(), intunr, lssf);
    }
    // `:698-722`, 1-based `eunr(k)` = `eunr[k - 1]`.
    let mut i = 1usize;
    let mut elast = eunr[1];
    loop {
        i += 1;
        let Some(&enext) = eunr.get(i) else { break };
        if enext >= ONEMEV {
            break;
        }
        if enext >= WIDE * elast || indep {
            let mut et = elast;
            loop {
                let enut = EGRIDU.iter().copied().find(|&g| g > STEP * et).unwrap_or(enext);
                et = enut;
                if et >= enext {
                    break;
                }
                crate::unresr::mf2::ilist(et, &mut eunr);
                i += 1;
            }
        }
        elast = eunr[i];
    }
    // `:723-737`: keep `eunr(2..lim)` that clear the last by `sigfig(., 7, 2)`,
    // sign the overlap, then drop the last kept.
    let lim = eunr.len() - 1;
    let mut out = Vec::new();
    let mut en = 0.0;
    for &e in &eunr[1..lim] {
        let mut et = e;
        if et >= en {
            if et < elr {
                et = -et;
            }
            out.push(et);
            en = sigfig(et.abs(), 7, 2);
        }
    }
    out.pop();
    (out, intunr, lssf)
}

/// `rdunf3` (`unresr.f90:783-840`): the ENDF File-3 total, elastic, fission
/// (MT=18 only) and capture at `|eunr|`, the first nudged up by `1.00001`
/// and the last down by `0.99999`; zero for an absent MT.
///
/// # Errors
/// A malformed MF=3 section.
pub fn background(endf: &Tape, mat: i32, eunr: &[f64]) -> Result<Vec<[f64; 4]>, NjoyError> {
    const UP: f64 = 1.00001;
    const DN: f64 = 0.99999;
    let n = eunr.len();
    let mut sb = vec![[0.0f64; 4]; n];
    for (kx, mt) in [(0usize, 1), (1, 2), (2, 18), (3, 102)] {
        let Some(sec) = endf.section(mat, 3, mt) else { continue };
        let mut cur = SectionCursor::new(&sec.rows);
        let _ = cur.read_cont()?;
        let tab = cur.read_tab1()?;
        let mut g = crate::endf::gety1::Gety1::new(&tab);
        let _ = g.get(0.0);
        for (ie, &eu) in eunr.iter().enumerate() {
            let mut e = eu.abs();
            if ie == 0 {
                e *= UP;
            }
            if ie + 1 == n {
                e *= DN;
            }
            sb[ie][kx] = g.get(e).y;
        }
    }
    Ok(sb)
}

/// The MF=2/MT=152 rows UNRESR writes for one temperature (`:178-200`,
/// `:302-317`): HEAD `za, awr, lssf, 0, 0, intunr`, then a LIST `temz, 0, 5,
/// nsigz, npl, nunr` of `sigz`, then per energy `e` and `sigfig(sigu, 7, 0)` for
/// total, elastic, fission, capture and transport at each `sigz`.
///
/// # Errors
/// From the `unresl` kernel.
#[allow(clippy::too_many_arguments)]
pub fn mt152_rows(
    ranges: &[UnresolvedRange],
    eunr: &[f64],
    sb: Option<&[[f64; 4]]>,
    za: f64,
    awr: f64,
    lssf: i32,
    intunr: i32,
    temz: f64,
    sigz: &[f64],
    table: &crate::unresr::wfun::WTable,
) -> Result<Vec<[f64; 6]>, NjoyError> {
    let nsigz = sigz.len();
    let nunr = eunr.len();
    let mut data: Vec<f64> = sigz.to_vec();
    for (ie, &ez) in eunr.iter().enumerate() {
        data.push(ez);
        let bkg = match sb {
            Some(b) if lssf <= 0 => b[ie],
            _ => [0.0; 4],
        };
        let sigu = crate::unresr::unresolved_cross_sections(ranges, ez.abs(), temz, sigz, bkg, table)?;
        for ix in 0..5 {
            for row in &sigu {
                data.push(sigfig(row[ix], 7, 0));
            }
        }
    }
    let mut rows = vec![[za, awr, f64::from(lssf), 0.0, 0.0, f64::from(intunr)]];
    rows.push([temz, 0.0, 5.0, nsigz as f64, data.len() as f64, nunr as f64]);
    for ch in data.chunks(6) {
        let mut r = [0.0f64; 6];
        r[..ch.len()].copy_from_slice(ch);
        rows.push(r);
    }
    Ok(rows)
}

/// UNRESR on a PENDF (`:136-330`): for each material card and each of its
/// temperatures, in tape order, MT=152 of that temperature's block is replaced by
/// UNRESR's table, and the block's MF=1 temperature word set to `temz` (`:224`).
/// A material with no resonance parameters (`LRP = 0`) or no unresolved range
/// is copied as it is.
///
/// # Errors
/// [`NjoyError::NotPorted`] for a block with no MT=152 yet (`new = 1`);
/// [`NjoyError::InvalidInput`] when a temperature block is missing.
pub fn unresr_tape(endf: &Tape, prev: &Tape, mat: i32, temps: &[f64], sigz: &[f64]) -> Result<Tape, NjoyError> {
    let Some(mf1e) = endf.section(mat, 1, 451) else { return Ok(prev.clone()) };
    if mf1e.rows[0][2].round() as i32 == 0 {
        return Ok(prev.clone()); // `lrp = 0` (`:153`)
    }
    let Some(mf2) = endf.section(mat, 2, 151) else { return Ok(prev.clone()) };
    let (za, awr) = (mf2.rows[0][0], mf2.rows[0][1]);
    let ranges = if mf2.rows.len() > 1 { crate::unresr::mf2::parse_lru2_ranges(&mf2.rows[1..])? } else { Vec::new() };
    let info = crate::reconr::mf2::parse_resonance_info(mf2)?;
    let elr = info.ranges.iter().filter(|r| r.lru == 1).map(|r| sigfig(r.eh, 7, 0)).fold(0.0, f64::max);
    let (eunr, intunr, lssf) = unresr_grid(&ranges, elr);
    if eunr.is_empty() {
        return Ok(prev.clone()); // `nunr = 0` (`:170`)
    }
    let sb = if lssf > 0 { None } else { Some(background(endf, mat, &eunr)?) };
    let table = crate::unresr::wfun::WTable::new();
    let mut out = prev.clone();
    let heads: Vec<usize> = prev
        .sections()
        .iter()
        .enumerate()
        .filter(|(_, s)| s.key.mat == mat && s.key.mf == 1 && s.key.mt == 451)
        .map(|(i, _)| i)
        .collect();
    for (it, &temz) in temps.iter().enumerate() {
        let &h = heads.get(it).ok_or_else(|| NjoyError::InvalidInput(format!("unresr: no block {} of mat {mat} on the pendf", it + 1)))?;
        let end = heads.get(it + 1).copied().unwrap_or(prev.sections().len());
        let i152 = (h..end)
            .find(|&i| {
                let k = prev.sections()[i].key;
                k.mat == mat && k.mf == 2 && k.mt == 152
            })
            .ok_or(NjoyError::NotPorted("unresr on a pendf with no MF=2/MT=152 (new dictionary entry)"))?;
        let rows = mt152_rows(&ranges, &eunr, sb.as_deref(), za, awr, lssf, intunr, temz, sigz, &table)?;
        out.set_section_rows(i152, rows);
        let mut mf1 = prev.sections()[h].rows.clone();
        let iverf = crate::moder::layout::iverf_from_mf1(&mf1);
        let thead = match iverf {
            ..=4 => 1,
            5 => 2,
            _ => 3,
        };
        if let Some(r) = mf1.get_mut(thead) {
            r[0] = temz;
        }
        out.set_section_rows(h, mf1);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn egridu_refines_wide_intervals_and_drops_shades() {
        // One Case-A range 2.5-30 keV: shades, refinement by `egridu`, the outer
        // shades and the last kept node dropped.
        let r = UnresolvedRange {
            el: 2500.0,
            eh: 30000.0,
            abn: 1.0,
            naps: 0,
            nro: 0,
            ap_table: None,
            lssf: 0,
            case_: UnresolvedCase::CaseA { spi: 0.5, ap: 0.9, awri: 236.0, l_states: Vec::new() },
        };
        let (e, intunr, lssf) = unresr_grid(&[r], 0.0);
        assert_eq!((intunr, lssf), (2, 0));
        assert!((e[0] - 2500.001).abs() < 1e-9, "{e:?}");
        assert!(e.contains(&3000.0) && e.contains(&25000.0));
        assert!(e.windows(2).all(|w| w[1] > w[0]));
        assert!(*e.last().unwrap() < 30000.0);
    }
}
