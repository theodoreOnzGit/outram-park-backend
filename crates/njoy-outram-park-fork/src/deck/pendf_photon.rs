// Translated from NJOY2016.80 (commit ac5adf5) `src/reconr.f90`: the photo-atomic
// (MF=23) path of `reconr` (`:197-260`), `anlyzd` (`:599-660`), `lunion`
// (`:1866-1914`), `emerge` (`:4720-4990`) and `recout` (`:5027-5440`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **RECONR on a photo-atomic evaluation** (MF=23 only; gh:#1369, decks 03 and
//! 30): the union grid of the MF=23 sections, each section on it, the redundant
//! sums MT=501 (and MT=522 when photo-ionisation subshells are given), and the
//! PENDF material as `recout` writes it.
//!
//! What upstream does that a neutron material does not:
//! - `anlyzd` (`:599-617`) makes MT=501 the first redundant reaction, and MT=522
//!   a second one when the dictionary lists an MT in 534-572; for ENDF-4/5 it
//!   sets `zain = awin = 0`. It returns before seeding MF=2 (`:654`), so no
//!   MF=2 is written (`no2 = 1`).
//! - `lunion` skips MT=501, MT=522 (when redundant) and MT=460 (`:1875-1877`),
//!   and an MF=23 TAB1 skips the threshold raise (`:1914`).
//! - `emerge` adds every MF=23 section but MT=515 and 517 into MT=501, and
//!   MT=534-572 into MT=522 (`:4896-4915`).
//! - `recout` names the first redundant reaction 501 (`:5264`).
//!
//! Refused by name: a material with both MF=3 and MF=23, and MF=10/12/13 on a
//! photo-atomic material (no evaluation does this; upstream would merge them).

use crate::endf::gety1::Gety1;
use crate::endf::records::SectionCursor;
use crate::endf::tape::{Section, Tape};
use crate::endf::EndfKey;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

/// `true` when material `mat` on `endf` is photo-atomic (has MF=23).
pub fn is_photoatomic(endf: &Tape, mat: i32) -> bool {
    endf.sections().iter().any(|s| s.key.mat == mat && s.key.mf == 23)
}

/// One material's PENDF, as [`photon_material`] builds it.
#[derive(Debug, Clone)]
pub struct PhotonPendf {
    /// MF=1/MT=451 then the MF=23 sections, in `recout`'s order.
    pub sections: Vec<Section>,
    /// MF=1/MT=451's raw text (the description cards), for `Tape::set_raw_text`.
    pub raw: Vec<String>,
    /// The closers `recout` copies (running sequence numbers).
    pub copied: Vec<EndfKey>,
}

fn nint(x: f64) -> i32 {
    x.round() as i32
}

/// The MF=23 TAB1 of section `s` with its HEAD, as `lunion` copies it to
/// `nscr1` and `emerge` reads it back (coded when `npend > 0`).
fn read_mf23(s: &Section, coded: bool) -> Result<([f64; 6], crate::endf::records::Tab1), NjoyError> {
    let mut cur = SectionCursor::new(&s.rows);
    let h = cur.read_cont()?;
    let mut t = cur.read_tab1()?;
    crate::reconr::as_emerge_reads_scratch(&mut t, coded);
    Ok(([h.c1, h.c2, f64::from(h.l1), f64::from(h.l2), f64::from(h.n1), f64::from(h.n2)], t))
}

/// RECONR's PENDF material for the photo-atomic material `mat` (one pass of
/// `reconr`'s material loop, `:197-388`).
///
/// # Errors
/// A malformed section, `lunion`'s errors, or a refusal named above.
pub fn photon_material(endf: &Tape, mat: i32, cards: &[String], err: f64, tempr: f64, npend: i32) -> Result<PhotonPendf, NjoyError> {
    const TEST: f64 = 1.0e-10;
    let mf1 = endf.section(mat, 1, 451).ok_or(NjoyError::SectionNotFound { mat, mf: 1, mt: 451 })?;
    if endf.sections().iter().any(|s| s.key.mat == mat && matches!(s.key.mf, 3 | 10 | 12 | 13)) {
        return Err(NjoyError::NotPorted("RECONR: a photo-atomic (MF=23) material with MF=3/10/12/13 sections"));
    }
    let iverf = crate::moder::layout::iverf_from_mf1(&mf1.rows);
    let r0 = mf1.rows[0];
    let (za, awr) = (r0[0], r0[1]);
    let coded = npend > 0;

    // `lunion`'s sections (`:1866-1877`), in tape order.
    let all: Vec<&Section> = endf.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 23).collect();
    let mtr522 = all.iter().any(|s| (534..=572).contains(&s.key.mt));
    let kept: Vec<&Section> = all
        .into_iter()
        .filter(|s| !(s.key.mt == 501 || (s.key.mt == 522 && mtr522) || s.key.mt == 460))
        .collect();
    let mut tabs = Vec::with_capacity(kept.len());
    for s in &kept {
        tabs.push(read_mf23(s, coded)?);
    }
    let inputs: Vec<crate::reconr::lunion::UnionInput> = tabs
        .iter()
        .map(|(_, t)| crate::reconr::lunion::UnionInput { interp: t.interp.clone(), pairs: t.pairs.clone(), keep_leading_zeros: false })
        .collect();
    // `lrp != 1`: no resonance nodes and the fixed bounds (`:303-311`).
    let bounds = crate::reconr::lunion::RangeBounds { eres: [1.0e-5, 20.0e6, 9.0e9, 9.0e9, 20.0e6], elim: 0.99e6 };
    let grid = crate::reconr::lunion::lunion(&[], &inputs, err, bounds)?;
    let ngo = grid.len();

    // `anlyzd`'s redundant list, sorted: 501, then 522.
    let mtr: Vec<i32> = if mtr522 { vec![501, 522] } else { vec![501] };
    let mut mtrt: Vec<usize> = mtr.iter().map(|&m| if m == 501 { 1 } else { 1_000_000 }).collect();
    let mut tot = vec![vec![0.0f64; mtr.len()]; ngo];

    // `emerge` (`:4740-4990`), one section at a time.
    struct Out {
        mt: i32,
        head: [f64; 6],
        tab_head: [f64; 4],
        pts: Vec<(f64, f64)>,
    }
    let mut outs: Vec<Out> = Vec::new();
    for (s, (head, t)) in kept.iter().zip(&tabs) {
        let mth = s.key.mt;
        let mut g = Gety1::new(t);
        let thresh = sigfig(g.get(0.0).xnext, 7, 0);
        let mut pts: Vec<(f64, f64)> = Vec::new();
        let mut ith = 0usize;
        for (ig, &eg) in grid.iter().enumerate() {
            let it = ig + 1;
            if thresh - eg > TEST * thresh {
                continue;
            }
            let mut sn = g.get(eg).y;
            if thresh > 1.0 && (thresh - eg).abs() < TEST * thresh {
                sn = 0.0;
            }
            let sn = sigfig(sn, 7, 0);
            pts.push((eg, sn));
            if ith == 0 && sn > 0.0 {
                ith = pts.len();
            }
            // `:4896-4915`.
            for (imtr, &m) in mtr.iter().enumerate() {
                let adds = if m == 501 { mth != 515 && mth != 517 } else { (534..=572).contains(&mth) };
                if !adds {
                    continue;
                }
                if it < mtrt[imtr] {
                    mtrt[imtr] = it;
                    if mtrt[imtr] > 1 {
                        mtrt[imtr] -= 1;
                    }
                }
                tot[ig][imtr] += sn;
            }
        }
        if ith > 1 {
            ith -= 1;
        }
        if ith == 0 {
            // `istart = 0` reads point 0 of the grid buffer (`:4934-4944`), which
            // upstream never defines for an all-zero section.
            return Err(NjoyError::NotPorted("RECONR: an MF=23 section with no positive point (upstream reads an undefined buffer word)"));
        }
        outs.push(Out { mt: mth, head: *head, tab_head: [t.head.c1, t.head.c2, f64::from(t.head.l1), f64::from(t.head.l2)], pts: pts[ith - 1..].to_vec() });
    }

    let ntab_lines = |np: usize| 3 + np.div_ceil(3);
    let redundant_np: Vec<usize> = mtrt
        .iter()
        .map(|&s| if s <= ngo { ngo - s + 1 } else { 0 })
        .collect();
    if redundant_np.contains(&0) {
        return Err(NjoyError::InvalidInput("recout: a redundant MF=23 reaction has no points".into()));
    }

    // MF=1/MT=451 (`recout` :5027-5175).
    let ncards = cards.len();
    let nxc = 1 + outs.len();
    let ndict = nxc + mtr.len();
    let lrp = if iverf == 6 { 2.0 } else { r0[2] };
    let mut rows: Vec<[f64; 6]> = Vec::new();
    rows.push([za, awr, lrp, r0[3], 0.0, if iverf == 4 { ndict as f64 } else { 0.0 }]);
    if iverf != 4 {
        let r = mf1.rows[1];
        rows.push([r[0], r[1], r[2], r[3], 0.0, if iverf == 6 { 6.0 } else { r[5] }]);
    }
    let mut tempi = 0.0;
    if iverf == 6 {
        let r = mf1.rows[2];
        // `zain = int(nsub/10)`, `nsub = 3` for photo-atomic data.
        let zain = (nint(r[4]) / 10) as f64;
        rows.push([r[0], r[1], r[2], 0.0, (10.0 * zain).trunc(), r[5]]);
        tempi = mf1.rows[3][0];
    }
    rows.push([if tempr == 0.0 { tempi } else { tempr }, err, if iverf == 6 { 1.0 } else { 0.0 }, 0.0, ncards as f64, ndict as f64]);
    let nhead = rows.len();
    let mut raw = vec![String::new(); nhead];
    for t in cards {
        rows.push([0.0; 6]);
        raw.push(t.clone());
    }
    let dict_row = |mt: i32, nc: usize| [0.0, 0.0, 23.0, f64::from(mt), nc as f64, 0.0];
    rows.push([0.0, 0.0, 1.0, 451.0, (ncards + 2 + ndict) as f64, 0.0]);
    raw.push(String::new());
    let mut imtr = 0usize;
    for o in &outs {
        while imtr < mtr.len() && o.mt >= mtr[imtr] {
            rows.push(dict_row(mtr[imtr], ntab_lines(redundant_np[imtr])));
            raw.push(String::new());
            imtr += 1;
        }
        rows.push(dict_row(o.mt, ntab_lines(o.pts.len())));
        raw.push(String::new());
    }
    let mut sections = vec![Section { key: EndfKey { mat, mf: 1, mt: 451 }, rows }];

    // MF=23 (`recout` :5222-5345).
    let tab1_rows = |head: [f64; 4], pts: &[(f64, f64)]| -> Vec<[f64; 6]> {
        let np = pts.len() as f64;
        let mut r = vec![[head[0], head[1], head[2], head[3], 1.0, np], [np, 2.0, 0.0, 0.0, 0.0, 0.0]];
        for chunk in pts.chunks(3) {
            let mut row = [0.0f64; 6];
            for (i, &(x, y)) in chunk.iter().enumerate() {
                row[2 * i] = x;
                row[2 * i + 1] = y;
            }
            r.push(row);
        }
        r
    };
    let redundant_section = |imtr: usize, head: [f64; 6]| -> Section {
        let start = mtrt[imtr];
        let pts: Vec<(f64, f64)> = (start..=ngo).map(|i| (grid[i - 1], sigfig(tot[i - 1][imtr], 7, 0))).collect();
        let mut r = vec![head];
        r.extend(tab1_rows([tempr, 0.0, 0.0, 0.0], &pts));
        Section { key: EndfKey { mat, mf: 23, mt: mtr[imtr] }, rows: r }
    };
    let mut copied: Vec<EndfKey> = Vec::new();
    let first = outs.first().ok_or_else(|| NjoyError::InvalidInput("reconr: photo-atomic material with no MF=23 section".into()))?;
    let (za3, awr3) = (first.head[0], first.head[1]);
    let (n1l, n2l) = (first.head[4], first.head[5]);
    // The first redundant head keeps `scr(5:6)` of the first scratch head.
    sections.push(redundant_section(0, [za3, awr3, 0.0, 99.0, n1l, n2l]));
    let mut imtr = 1usize;
    let mut k = 0usize;
    let mut at_270 = imtr >= mtr.len();
    while !at_270 {
        let mtd = mtr[imtr];
        let o = outs.get(k).ok_or(NjoyError::NotPorted("recout: a redundant MF=23 reaction above every section"))?;
        if o.mt > mtd {
            sections.push(redundant_section(imtr, [za3, awr3, 0.0, 99.0, 0.0, 0.0]));
            imtr += 1;
            at_270 = imtr >= mtr.len();
            continue;
        }
        let mut r = vec![[za3, awr3, o.head[2], o.head[3], 0.0, 0.0]];
        r.extend(tab1_rows(o.tab_head, &o.pts));
        sections.push(Section { key: EndfKey { mat, mf: 23, mt: o.mt }, rows: r });
        copied.push(EndfKey { mat, mf: 23, mt: o.mt });
        k += 1;
    }
    // Label 270: one more head, then `tofend` copies the rest verbatim.
    for (j, o) in outs.iter().enumerate().skip(k) {
        let head = if j == k { [za3, awr3, o.head[2], o.head[3], n1l, n2l] } else { o.head };
        let mut r = vec![head];
        r.extend(tab1_rows(o.tab_head, &o.pts));
        sections.push(Section { key: EndfKey { mat, mf: 23, mt: o.mt }, rows: r });
        copied.push(EndfKey { mat, mf: 23, mt: o.mt });
    }
    copied.push(EndfKey { mat, mf: 23, mt: 0 });
    copied.push(EndfKey { mat, mf: 0, mt: 0 });
    Ok(PhotonPendf { sections, raw, copied })
}

/// RECONR's output tape for a run whose materials are all photo-atomic.
///
/// # Errors
/// Those of [`photon_material`].
pub fn photon_tape(endf: &Tape, mats: &[(i32, Vec<String>, f64, f64)], label: &str, npend: i32) -> Result<Tape, NjoyError> {
    let mut secs = Vec::new();
    let mut raws = Vec::new();
    let mut copied = Vec::new();
    for (mat, cards, err, tempr) in mats {
        let p = photon_material(endf, *mat, cards, *err, *tempr, npend)?;
        secs.extend(p.sections);
        raws.push((*mat, p.raw));
        copied.extend(p.copied);
    }
    let mut tape = Tape::from_sections(format!("{:<66.66}{:4}{:2}{:3}{:5}", label, 1, 0, 0, 0), secs);
    for (mat, raw) in raws {
        tape.set_raw_text(EndfKey { mat, mf: 1, mt: 451 }, raw);
    }
    tape.set_copied_closers(copied);
    Ok(tape)
}
