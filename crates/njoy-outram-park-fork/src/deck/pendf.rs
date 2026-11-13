// Translated from NJOY2016.80 `src/reconr.f90`: `anlyzd` (the redundant-reaction
// list and the dictionary seed), `emerge` (MF=10/12/13/23 on the union grid) and
// `recout` (MF=1/MT=451 with its dictionary, the dummy MF=2/MT=151, MF=2/MT=152,
// the MF=3 heads). NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence,
// GPL-compatible; this derivative file is distributed under AGPL-3.0-only. This
// is a modified, non-LANL version, not endorsed by LANL/DOE. See crate root
// LICENSE.njoy + NOTICE.

//! **The PENDF tape as NJOY writes it** (gh:#1369, part 2): the text, the
//! dictionary, MF=2 and the non-MF=3 files that RECONR carries, so the tape a
//! deck passes between modules compares **byte for byte**, not section for
//! section.
//!
//! [`crate::heatr::pendf_for_heatr`] builds the pointwise content only (MF=3
//! and a bare MF=1). [`reconr_tape`] adds what `recout` writes around it, and
//! [`broadr_tape`] is BROADR's rewrite of it at one temperature.
//!
//! Where upstream's rules are reproduced here:
//!
//! - **MF=1/MT=451** (`recout` 5028-5176): the head records from the ENDF
//!   tape's own, `TEMP`/`ERR`/`NWD`/`NXC` record, the description cards the
//!   deck supplied, and the dictionary; `NC` of MT=451 is `ncards + 2 + NXC`
//!   (`anlyzd`, `ncs(1)=ncards+2`).
//! - **MF=2/MT=151** (`:5179-5202`): four records, always the same shape,
//!   with `EH` the top of the resonance range and `SPI`/`AP` of the last range.
//! - **MF=3 heads** (`:5270-5337`): a redundant reaction (the sums RECONR
//!   rebuilds: MT=1, 4, 18, 103-107) carries `L1=0, L2=99`; every other
//!   section carries its ENDF head's `L1`/`L2`.
//! - **MF=10/12/13/23** (`emerge` 4786-4925 with `lunion`'s selection,
//!   `:1866-1890`): evaluated on the union grid with `gety1`, cut at the
//!   first non-zero point (less one), `sigfig`'d to 7 figures.

use crate::endf::gety1::Gety1;
use crate::endf::records::SectionCursor;
use crate::endf::tape::{Section, Tape};
use crate::endf::EndfKey;
use crate::mixr::mix::sigfig;
use crate::reconr::mf2::parse_resonance_info;
use crate::reconr::ReconrResult;
use crate::NjoyError;

/// `anlyzd`'s redundant-reaction list for an ENDF tape (`reconr.f90:536-598`),
/// in the sorted order `recout` walks it.
fn redundant_list(endf: &Tape, mat: i32, iverf: i32) -> Vec<i32> {
    let (mp, md, mt_, m3, m4) = if iverf >= 6 {
        ((600, 649), (650, 699), (700, 749), (750, 799), (800, 849))
    } else {
        ((700, 718), (720, 738), (740, 758), (760, 768), (780, 798))
    };
    let mts: Vec<i32> = endf.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 3).map(|s| s.key.mt).collect();
    let mut r = vec![1];
    let any = |lo: i32, hi: i32| mts.iter().any(|&m| m >= lo && m <= hi);
    if mts.contains(&19) {
        r.push(18);
    }
    if any(51, 91) {
        r.push(4);
    }
    for (code, (lo, hi)) in [(103, mp), (104, md), (105, mt_), (106, m3), (107, m4)] {
        if any(lo, hi) {
            r.push(code);
        }
    }
    r.sort_unstable();
    r
}

/// `true` when `lunion` skips this MF=3 section because RECONR rebuilds it
/// (`reconr.f90:1890-1910`), so `emerge` regenerates it as redundant.
fn skipped_in_mf3(mt: i32, iverf: i32, redundant: &[i32]) -> bool {
    if mt == 1 || mt == 3 || mt == 101 || mt == 120 || mt == 151 {
        return true;
    }
    if redundant.contains(&mt) && matches!(mt, 4 | 103 | 104 | 105 | 106 | 107 | 18) {
        return true;
    }
    if (251..=300).contains(&mt) && mt != 261 {
        return true;
    }
    if iverf < 6 {
        mt >= 451 && mt < 700 || mt >= 800
    } else {
        mt >= 451 && mt < 600 || mt > 850 && mt <= 870 || mt > 891
    }
}

fn nint(x: f64) -> i32 {
    x.round() as i32
}

/// MF=10/12/13/23 sections as `emerge` writes them: `gety1` on each union-grid
/// energy, `sigfig(.,7,0)`.
///
/// `emerge` reads `lunion`'s scratch copy `nscr1`, not the ENDF tape
/// (`reconr.f90:372`, `:4752`), so each MF=10 subsection is read with its
/// first energy raised to the kinematic threshold of its own `QI`
/// (`lunion`, `:1913-1938`, once per subsection through `:2187`) and in the
/// scratch mode `coded_scratch` (`npend > 0`, `:193`):
/// [`crate::reconr::lunion_tab1_for_emerge`]. ~~The raw subsection~~
/// **CORRECTED 2026-11-12 (gh:#1496)**: read raw, Zr-90's second subsections
/// of MT=16/28/112 (excited residual states, `LFS = 1`) kept their tabulated thresholds
/// (1.269870e7 eV against the raised 1.269890e7), one point too many and
/// values off in the 5th figure on NJOY2016 decks 32, 49, 69. MF=12 (`LO=1`)
/// and MF=13 TAB1s have `C2 = 0` by ENDF-6 format, so `lunion`'s raise is a
/// no-op there; MF=23 skips it (`:1914`).
fn other_files(endf: &Tape, mat: i32, grid: &[f64], mtr522: bool, coded_scratch: bool) -> Result<Vec<Section>, NjoyError> {
    const TEST: f64 = 1.0e-10;
    let awin = endf.section(mat, 1, 451).map_or(1.0, crate::reconr::lunion_awin);
    let mut out = Vec::new();
    for s in endf.sections().iter().filter(|s| s.key.mat == mat && matches!(s.key.mf, 10 | 12 | 13 | 23)) {
        let (mf, mt) = (s.key.mf, s.key.mt);
        if mt == 501 || (mt == 522 && mtr522) || mt == 460 {
            continue;
        }
        let mut cur = SectionCursor::new(&s.rows);
        let head = cur.read_cont()?;
        if mf == 12 && head.l1 != 1 {
            continue;
        }
        // `emerge` loops `iss = nss..1` over the TAB1 subsections of an MF=10
        // section (`nss = N1` of the head); every other file has one.
        let nss = if mf == 10 { head.n1.max(1) as usize } else { 1 };
        let mut rows: Vec<[f64; 6]> = Vec::new();
        for iss in 0..nss {
            let mut t = cur.read_tab1()?;
            if mf == 10 {
                crate::reconr::lunion_tab1_for_emerge(&mut t, head.c2, awin, coded_scratch)?;
            }
            let mut g = Gety1::new(&t);
            let thresh = sigfig(g.get(0.0).xnext, 7, 0);
            let mut pts: Vec<(f64, f64)> = Vec::new();
            let mut ith = 0usize;
            for &eg in grid {
                if thresh - eg > TEST * thresh {
                    continue;
                }
                let r = g.get(eg);
                let mut sn = r.y;
                if thresh > 1.0 && (thresh - eg).abs() < TEST * thresh {
                    sn = 0.0;
                }
                let sn = sigfig(sn, 7, 0);
                pts.push((eg, sn));
                if ith == 0 && sn > 0.0 {
                    ith = pts.len();
                }
            }
            if ith > 1 {
                ith -= 1;
            }
            if ith == 0 {
                return Err(NjoyError::NotPorted("deck PENDF: MF=10-23 section with no positive point"));
            }
            let kept = &pts[ith - 1..];
            let np = kept.len() as f64;
            if iss == 0 {
                rows.push([head.c1, head.c2, f64::from(head.l1), f64::from(head.l2), if mf == 3 || mf == 10 { f64::from(head.n1) } else { 1.0 }, f64::from(head.n2)]);
            }
            rows.push([t.head.c1, t.head.c2, f64::from(t.head.l1), f64::from(t.head.l2), 1.0, np]);
            rows.push([np, 2.0, 0.0, 0.0, 0.0, 0.0]);
            for chunk in kept.chunks(3) {
                let mut row = [0.0f64; 6];
                for (i, &(x, y)) in chunk.iter().enumerate() {
                    row[2 * i] = x;
                    row[2 * i + 1] = y;
                }
                rows.push(row);
            }
        }
        out.push(Section { key: EndfKey { mat, mf, mt }, rows });
    }
    Ok(out)
}

/// The tape label line `tlabel` as `Tape::write` prints a first line:
/// 66 columns then `MAT=1, MF=0, MT=0, seq 0` (`   1 0  0    0`).
fn tpid_line(label: &str) -> String {
    format!("{:<66.66}{:4}{:2}{:3}{:5}", label, 1, 0, 0, 0)
}

/// Rebuild MF=1/MT=451's `N2` and dictionary for the sections now on the tape
/// (`recout`'s `dict`, `:5096-5175`), keeping its head records, `TEMP`/`ERR`
/// record and text. `ncards` text lines.
fn with_directory(mat: i32, iverf: i32, head_rows: Vec<[f64; 6]>, text: &[String], body: Vec<Section>) -> Result<(Vec<Section>, Vec<String>), NjoyError> {
    let ncards = text.len();
    let mut entries: Vec<[i32; 3]> = Vec::new();
    for s in &body {
        entries.push([s.key.mf, s.key.mt, s.rows.len() as i32]);
    }
    let nentries = entries.len() + 1; // MT=451 itself
    let mut rows = head_rows;
    let n = rows.len();
    // ENDF-4 keeps the entry count in the head record (`recout` :5036) and
    // zero in the TEMP record; later formats the other way round (:5071).
    if iverf == 4 {
        rows[0][5] = nentries as f64;
        rows[n - 1][5] = 0.0;
    } else {
        rows[n - 1][5] = nentries as f64;
    }
    rows[n - 1][4] = ncards as f64;
    let mut raw = vec![String::new(); n];
    for t in text {
        rows.push([0.0; 6]);
        raw.push(t.clone());
    }
    rows.push([0.0, 0.0, 1.0, 451.0, (ncards + 2 + nentries) as f64, 0.0]);
    raw.push(String::new());
    for e in &entries {
        rows.push([0.0, 0.0, f64::from(e[0]), f64::from(e[1]), f64::from(e[2]), 0.0]);
        raw.push(String::new());
    }
    let mut secs = vec![Section { key: EndfKey { mat, mf: 1, mt: 451 }, rows }];
    secs.extend(body);
    Ok((secs, raw))
}

/// The PENDF RECONR writes for one material, as `recout` lays it out.
///
/// `cards` are the description cards (`ncards` text lines), `label` the tape
/// label, `err` card 3's tolerance, `tempr` card 4's temperature (RECONR's
/// own, normally 0). `npend` is the deck's output unit: its sign is the
/// mode of `lunion`'s scratch tape (see [`crate::reconr::reconr_for_deck`]).
///
/// # Errors
/// An ENDF tape lacking the material's MF=1, or a section kind not ported
/// (MF=10).
pub fn reconr_tape(
    endf: &Tape,
    mat: i32,
    res: &ReconrResult,
    label: &str,
    cards: &[String],
    err: f64,
    tempr: f64,
    npend: i32,
) -> Result<Tape, NjoyError> {
    let mf1 = endf.section(mat, 1, 451).ok_or(NjoyError::SectionNotFound { mat, mf: 1, mt: 451 })?;
    let iverf = crate::moder::layout::iverf_from_mf1(&mf1.rows);
    let (za, awr) = (res.material.za, res.material.awr);
    // MF=1 head records.
    let r0 = mf1.rows[0];
    let mut head: Vec<[f64; 6]> = Vec::new();
    // `lrp`: the ENDF head's, 0 without MF=2 (`rdfil2` :713), 3 once an unresolved
    // range is read (:793), and 2 for ENDF-6 (`recout` :5031).
    let mut lrp = if endf.section(mat, 2, 151).is_some() { r0[2] } else { 0.0 };
    if let Some(m2) = endf.section(mat, 2, 151) {
        if parse_resonance_info(m2)?.ranges.iter().any(|r| r.lru == 2) {
            lrp = 3.0;
        }
    }
    if iverf == 6 {
        lrp = 2.0;
    }
    head.push([za, awr, lrp, r0[3], 0.0, 0.0]);
    if iverf != 4 {
        let r = mf1.rows[1];
        head.push([r[0], r[1], r[2], r[3], 0.0, if iverf == 6 { 6.0 } else { r[5] }]);
    }
    let mut zain = 1.0;
    if iverf == 6 {
        let r = mf1.rows[2];
        zain = f64::from(nint(r[4])) / 10.0;
        head.push([r[0], r[1], r[2], 0.0, f64::from(nint(10.0 * zain)), r[5]]);
    }
    let tempi = mf1.rows[head.len()][0];
    head.push([if tempr == 0.0 { tempi } else { tempr }, err, if iverf == 6 { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0]);

    let redundant = redundant_list(endf, mat, iverf);
    // MF=3: RECONR's sections, with `recout`'s heads.
    let mut mf3: Vec<Section> = Vec::new();
    for sec in &res.sections {
        let mt = sec.mt.number();
        let (l1, l2) = if redundant.contains(&mt) {
            (0.0, 99.0)
        } else {
            let h = endf.section(mat, 3, mt).map(|s| s.rows[0]);
            h.map_or((0.0, 0.0), |h| (h[2], h[3]))
        };
        let qm = endf.section(mat, 3, mt).and_then(|s| s.rows.get(1).map(|r| r[0])).unwrap_or(0.0);
        let np = sec.pairs.len() as f64;
        let mut rows = vec![[za, awr, l1, l2, 0.0, 0.0]];
        let (qm, lr) = if redundant.contains(&mt) { (tempr, 0.0) } else { (qm, f64::from(sec.lr)) };
        let qi = if mt == 18 && redundant.contains(&18) { sigfig(sec.qi, 7, 0) } else { sec.qi };
        rows.push([qm, qi, 0.0, lr, 1.0, np]);
        rows.push([np, 2.0, 0.0, 0.0, 0.0, 0.0]);
        for chunk in sec.pairs.chunks(3) {
            let mut row = [0.0f64; 6];
            for (i, &(x, y)) in chunk.iter().enumerate() {
                row[2 * i] = x;
                row[2 * i + 1] = y;
            }
            rows.push(row);
        }
        mf3.push(Section { key: EndfKey { mat, mf: 3, mt }, rows });
    }
    mf3.sort_by_key(|s| s.key.mt);
    recout_heads(&mut mf3, endf, mat, &redundant);

    // MF=2.
    let mut body: Vec<Section> = Vec::new();
    if let Some(m2) = endf.section(mat, 2, 151) {
        let info = parse_resonance_info(m2)?;
        let (spin, mut ascat) = info.ranges.last().map_or((0.0, 0.0), |r| (r.spi, r.ap));
        if let Some(rml) = info.ranges.last().and_then(|r| r.rml.as_ref()) {
            ascat = rml_ascat(&rml.section, ascat);
        }
        let eh = res.resonance_upper_limit.unwrap_or(2.0e7);
        body.push(Section {
            key: EndfKey { mat, mf: 2, mt: 151 },
            rows: vec![
                [za, awr, 0.0, 0.0, 1.0, 0.0],
                [za, 1.0, 0.0, 0.0, 1.0, 0.0],
                [1.0e-5, eh, 0.0, 0.0, 0.0, 0.0],
                [spin, ascat, 0.0, 0.0, 0.0, 0.0],
            ],
        });
        if let Some(u) = &res.unresolved_table {
            body.push(Section { key: EndfKey { mat, mf: 2, mt: 152 }, rows: u.clone() });
        }
    }
    let grid: Vec<f64> = res
        .sections
        .iter()
        .find(|s| s.mt.number() == 1)
        .map(|s| s.pairs.iter().map(|p| p.0).collect())
        .unwrap_or_default();
    body.extend(mf3);
    let _ = skipped_in_mf3;
    body.extend(other_files(endf, mat, &grid, false, npend > 0)?);
    let text: Vec<String> = cards.to_vec();
    let (secs, raw) = with_directory(mat, iverf, head, &text, body)?;
    let mut tape = Tape::from_sections(tpid_line(label), secs);
    tape.set_raw_text(EndfKey { mat, mf: 1, mt: 451 }, raw);
    // `recout` copies (tosend/tofend/tomend) every non-redundant MF=3 section
    // and all later files from its scratch tape, so their SEND, FEND and the
    // MEND carry the running count; the redundant sums, MF=1 and MF=2 are
    // written with asend/afend (`reconr.f90:5270-5435`). gh:#1484.
    let mut copied: Vec<EndfKey> = Vec::new();
    for s in tape.sections() {
        let k = s.key;
        if k.mf >= 3 && !(k.mf == 3 && redundant.contains(&k.mt)) {
            copied.push(k);
        }
        if k.mf >= 3 {
            copied.push(EndfKey { mat, mf: k.mf, mt: 0 });
        }
    }
    copied.push(EndfKey { mat, mf: 0, mt: 0 });
    tape.set_copied_closers(copied);
    let _ = zain;
    Ok(tape)
}

/// `ascat` after an LRF=7 range (`rdsammy`, `samm.f90:1114-1132`): the
/// effective radius `APE` of the last channel, over every spin group in order,
/// whose particle pair is elastic (`mt(ipp) = 2`); `recout` writes it in
/// MF=2/MT=151's fourth record (`reconr.f90:5200`). An RML range's own record
/// carries no `AP`, so without this the field was 0 (decks 81, 88; gh:#1369).
fn rml_ascat(sec: &crate::samm::mf2::RmlSection, init: f64) -> f64 {
    let mut ascat = init;
    for g in &sec.spin_groups {
        for ch in &g.channels {
            let pair = ch.particle_pair.checked_sub(1).and_then(|i| sec.particle_pairs.get(i));
            if pair.is_some_and(|p| p.mt == 2) {
                ascat = ch.radius_eff;
            }
        }
    }
    ascat
}

/// The MF=3 HEAD records as `recout` writes them (`reconr.f90:5222-5365`):
/// every head it writes itself carries `za`/`awr` from the first section on
/// its scratch tape (the first non-redundant reaction's own ENDF head,
/// `:5223-5224`); once the last redundant reaction is out (label 270) it
/// writes one more head and `tofend` copies the rest of the scratch tape, so
/// every later non-redundant head is the evaluation's own, verbatim (deck 82,
/// Am-242m1 MT=102: AWR `2.399800+2` against MF=1's `2.399801+2`; gh:#1369).
fn recout_heads(mf3: &mut [Section], endf: &Tape, mat: i32, redundant: &[i32]) {
    let endf_head = |mt: i32| endf.section(mat, 3, mt).and_then(|s| s.rows.first().copied());
    let Some(first) = mf3.iter().find(|s| !redundant.contains(&s.key.mt)).and_then(|s| endf_head(s.key.mt)) else {
        return;
    };
    let (za, awr) = (first[0], first[1]);
    let last_red = mf3.iter().filter(|s| redundant.contains(&s.key.mt)).map(|s| s.key.mt).max();
    let mut after_270 = false;
    for s in mf3.iter_mut() {
        let mt = s.key.mt;
        let beyond = last_red.is_some_and(|l| mt > l) && !redundant.contains(&mt);
        if beyond && after_270 {
            if let Some(h) = endf_head(mt) {
                s.rows[0] = h;
            }
            continue;
        }
        s.rows[0][0] = za;
        s.rows[0][1] = awr;
        after_270 |= beyond;
    }
}

/// The closing records BROADR *copies* on a coded output unit, which carry
/// the running sequence number ([`Tape::set_copied_closers`]):
/// - MF=1: its SEND and FEND (`tofend` after `dictio`, `broadr.f90:681`);
/// - MF=2: each section's SEND (`tosend`, `:868`; MT=152 by `tofend`, `:876`),
///   and the FEND when MT=152 ends the file (that `tofend` copies it);
/// - every file after MF=3, closers and MEND included (`tomend`, `:999`).
///
/// MF=3's SENDs are `asend` (`:996`) and its FEND `contio`'s `afend`.
pub fn broadr_copied_closers(tape: &Tape, mat: i32) -> Vec<EndfKey> {
    let mut out = vec![EndfKey { mat, mf: 1, mt: 451 }, EndfKey { mat, mf: 1, mt: 0 }, EndfKey { mat, mf: 0, mt: 0 }];
    let mut last_mf2 = 0;
    for s in tape.sections().iter().filter(|s| s.key.mat == mat) {
        let k = s.key;
        if k.mf == 2 {
            out.push(k);
            last_mf2 = k.mt;
        }
        if k.mf > 3 {
            out.push(k);
            out.push(EndfKey { mat, mf: k.mf, mt: 0 });
        }
    }
    if last_mf2 == 152 {
        out.push(EndfKey { mat, mf: 2, mt: 0 });
    }
    out
}

/// BROADR's PENDF at one temperature from RECONR's tape `prev` and the
/// broadened result: MF=3 replaced, `TEMP` set, the dictionary's `NC`s
/// recounted; everything else copied (`broadr.f90`'s `tosend`/`tofend`).
///
/// # Errors
/// A tape without MF=1/MT=451 for `mat`.
pub fn broadr_tape(prev: &Tape, mat: i32, out: &ReconrResult, temp_k: f64) -> Result<Tape, NjoyError> {
    let mf1 = prev.section(mat, 1, 451).ok_or(NjoyError::SectionNotFound { mat, mf: 1, mt: 451 })?;
    let iverf = crate::moder::layout::iverf_from_mf1(&mf1.rows);
    let thead = match iverf {
        4 => 1,
        5 => 2,
        _ => 3,
    };
    let mut head: Vec<[f64; 6]> = mf1.rows[..=thead].to_vec();
    head[thead][0] = temp_k;
    let ncards = nint(head[thead][4]) as usize;
    let raw_mf1 = prev.raw_text(EndfKey { mat, mf: 1, mt: 451 }).map(<[String]>::to_vec).unwrap_or_default();
    let text: Vec<String> = (0..ncards).map(|i| raw_mf1.get(thead + 1 + i).cloned().unwrap_or_default()).collect();
    let mut body: Vec<Section> = Vec::new();
    for s in prev.sections().iter().filter(|s| s.key.mat == mat && !(s.key.mf == 1 && s.key.mt == 451)) {
        if s.key.mf == 3 {
            let Some(n) = out.sections.iter().find(|n| n.mt.number() == s.key.mt) else { continue };
            let mut rows = s.rows[..2].to_vec();
            let np = n.pairs.len() as f64;
            rows[1][4] = 1.0;
            rows[1][5] = np;
            rows.push([np, 2.0, 0.0, 0.0, 0.0, 0.0]);
            for chunk in n.pairs.chunks(3) {
                let mut row = [0.0f64; 6];
                for (i, &(x, y)) in chunk.iter().enumerate() {
                    row[2 * i] = x;
                    row[2 * i + 1] = y;
                }
                rows.push(row);
            }
            body.push(Section { key: s.key, rows });
        } else if s.key.mf == 2 && s.key.mt == 152 {
            // `broadr.f90:864-870`: "update unresolved temperature".
            let mut sec = s.clone();
            sec.rows[1][0] = temp_k;
            body.push(sec);
        } else {
            body.push(s.clone());
        }
    }
    let (mut secs, raw) = with_directory(mat, iverf, head, &text, body)?;
    // BROADR copies RECONR's TEMP record as it is (`hdatio`), so the entry
    // count stays in its `N2` for ENDF-4 too (test 10's tape).
    if iverf == 4 {
        let n = secs[0].rows.len();
        let nxc = secs[0].rows[0][5];
        if let Some(r) = secs[0].rows.get_mut(1) {
            r[5] = nxc;
        }
        let _ = n;
    }
    let mut tape = Tape::from_sections(prev.tpid.clone(), secs);
    tape.set_raw_text(EndfKey { mat, mf: 1, mt: 451 }, raw);
    Ok(tape)
}

/// PURR's PENDF from the PENDF `prev` it reads (`purr.f90:316-570`, gh:#1498):
/// the evaluation's MF=2/MT=151, then PURR's own MT=152 (Bondarenko table) and
/// MT=153 (probability tables) in place of any earlier ones, and the dictionary
/// rebuilt with them. Everything else is copied; the `TEMP` record carries
/// `temz`, the card's temperature (`a(iscr)=temz`, `:344`).
///
/// The dictionary's `NC` for the two new sections is upstream's
/// `2 + (npl - 1) / 6` (`:365-368`), which is one less than the lines the
/// sections occupy when `npl mod 6 = 1`; see [`crate::purr::pendf::directory_nc`].
///
/// # Errors
/// A tape without MF=1/MT=451 or MF=2/MT=151 for `mat`, or tables with no PURR
/// record.
pub fn purr_tape(prev: &Tape, mat: i32, tables: &crate::purr::UrrProbabilityTables, za: f64, awr: f64, temz: f64, sigz: f64) -> Result<Tape, NjoyError> {
    use crate::purr::pendf::{directory_nc, mt152_rows, mt153_rows};
    let mf1 = prev.section(mat, 1, 451).ok_or(NjoyError::SectionNotFound { mat, mf: 1, mt: 451 })?;
    let iverf = crate::moder::layout::iverf_from_mf1(&mf1.rows);
    let thead = match iverf {
        4 => 1,
        5 => 2,
        _ => 3,
    };
    let mut head: Vec<[f64; 6]> = mf1.rows[..=thead].to_vec();
    head[thead][0] = temz;
    let ncards = nint(head[thead][4]) as usize;
    let raw_mf1 = prev.raw_text(EndfKey { mat, mf: 1, mt: 451 }).map(<[String]>::to_vec).unwrap_or_default();
    let text: Vec<String> = (0..ncards).map(|i| raw_mf1.get(thead + 1 + i).cloned().unwrap_or_default()).collect();
    let bad = || NjoyError::InvalidInput("purr: the probability tables carry no PENDF record".into());
    let r152 = mt152_rows(za, awr, temz, sigz, tables).ok_or_else(bad)?;
    let r153 = mt153_rows(za, awr, temz, tables).ok_or_else(bad)?;
    let (npl152, npl153) = (nint(r152[1][4]) as usize, nint(r153[1][4]) as usize);
    let mut body: Vec<Section> = Vec::new();
    let mut inserted = false;
    for s in prev.sections().iter().filter(|s| s.key.mat == mat && !(s.key.mf == 1 && s.key.mt == 451)) {
        if s.key.mf == 2 && (s.key.mt == 152 || s.key.mt == 153) {
            continue;
        }
        body.push(s.clone());
        if s.key.mf == 2 && s.key.mt == 151 {
            body.push(Section { key: EndfKey { mat, mf: 2, mt: 152 }, rows: r152.clone() });
            body.push(Section { key: EndfKey { mat, mf: 2, mt: 153 }, rows: r153.clone() });
            inserted = true;
        }
    }
    if !inserted {
        return Err(NjoyError::SectionNotFound { mat, mf: 2, mt: 151 });
    }
    let (mut secs, raw) = with_directory(mat, iverf, head, &text, body)?;
    // MT=451's own entry is copied from the input's dictionary (`:372-399`
    // moves the entries as they are), so its `NC` keeps the old count.
    let nc451 = mf1.rows.iter().skip(thead + 1 + ncards).find(|r| nint(r[2]) == 1 && nint(r[3]) == 451).map(|r| r[4]);
    for r in secs[0].rows.iter_mut() {
        if let (Some(nc), 1, 451) = (nc451, nint(r[2]), nint(r[3])) {
            r[4] = nc;
        } else if nint(r[2]) == 2 && nint(r[3]) == 152 {
            r[4] = directory_nc(npl152) as f64;
        } else if nint(r[2]) == 2 && nint(r[3]) == 153 {
            r[4] = directory_nc(npl153) as f64;
        }
    }
    let mut tape = Tape::from_sections(prev.tpid.clone(), secs);
    tape.set_raw_text(EndfKey { mat, mf: 1, mt: 451 }, raw);
    Ok(tape)
}
