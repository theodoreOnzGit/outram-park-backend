// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine first`, l.255-805 (the `izai`-dependent parts);
//   - `subroutine unionx`, l.1550-1727 (the incident-charged-particle union grid);
//   - `subroutine acelod`, l.4932-6335, the `izai /= 1` branches;
//   - `subroutine acecpe`, l.6523-6670 (charged-particle elastic).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! ACER for an **incident charged particle** (`izai != 1`), gh:#1485.
//!
//! NJOY2016's `acefc` builds a table for a proton, deuteron, triton, He-3 or
//! alpha projectile when the ENDF tape is an ENDF-6 incident-charged-particle
//! evaluation (`NSUB = 10 * izai`, MF=6 for the products). The neutron path in
//! [`super::build`] is not touched; this module adds the branches upstream
//! takes on `izai`, and reuses [`super::acelf6`], [`super::acelcp`] and
//! [`super::photon_blocks`] for the parts that are common.
//!
//! What differs from the neutron table (read from `acefc.f90`):
//!
//! - **ZAID class letter** from `izai` (`h`, `o`, `r`, `s`, `a` for p, d, t,
//!   He-3, alpha; `acelod`, `:4963-4973`), and the header's `hm`;
//! - **the union grid** is the union of every MF=3 section's own grid plus the
//!   incident energies of MF=6/MT=2, thinned nowhere, with an extra point
//!   whenever a panel is wider than a factor 1.2 (`unionx`, `:1550-1727`);
//! - **which MF=3 sections are stored, and which count in `NR`**: per-`izai`
//!   MT lists (`acelod`, `:5069-5290` and `:5390-5700`); the table's `DLW`
//!   describes the outgoing particle of the **same kind as the projectile**;
//! - **the charged-particle elastic** (MF=6/MT=2, `LAW = 5`) is turned into an
//!   `AND` entry and the ESZ elastic and total columns are replaced by the
//!   nuclear-plus-Coulomb integral (`acecpe`);
//! - **the production blocks** (`acelcp`) cover the neutron too, since the
//!   projectile's own kind is the one written to `DLW`.

use crate::endf::gety1::Gety1;
use crate::endf::records::{SectionCursor, Tab1};
use crate::endf::tape::{Section, Tape};
use crate::mixr::mix::sigfig;
use crate::NjoyError;

use super::build::XssBuilder;
use super::{jxs, nxs, AceTable};

const EMEV: f64 = 1.0e6;
const ETOP: f64 = 1.0e10;
const EPS: f64 = 1.0e-10;
const ELOW: f64 = 1.0e-5;
const STEP: f64 = 1.2;

/// `izai` of an ENDF-6 evaluation: `NSUB / 10` (`first`, `acefc.f90:322`).
/// `1` for a neutron tape and for any tape without the ENDF-6 sub-library.
pub fn incident_particle(tape: &Tape, mat: i32) -> i32 {
    let Some(s) = tape.section(mat, 1, 451) else { return 1 };
    // Only an ENDF-6 tape (`NFOR = 6`) carries NSUB (`iverf.eq.6`).
    let nfor = s.rows.get(1).map_or(0.0, |r| r[5]);
    if nfor.round() as i32 != 6 {
        return 1;
    }
    s.rows.get(2).map_or(1, |r| (r[4].round() as i32) / 10)
}

fn tab1_of(sec: &Section) -> Result<Tab1, NjoyError> {
    let mut cur = SectionCursor::new(&sec.rows);
    cur.read_cont()?;
    cur.read_tab1()
}

/// The ZAID class letter, `acelod` `:4963-4973` (`mcnpx = 0`).
fn class_letter(izai: i32) -> Result<char, NjoyError> {
    Ok(match izai {
        1 => 'c',
        1001 => 'h',
        1002 => 'o',
        1003 => 'r',
        2003 => 's',
        2004 => 'a',
        _ => return Err(NjoyError::NotPorted("acelod: not coded for this incident particle")),
    })
}

/// `unionx`'s union grid for `izai != 1` (`acefc.f90:1550-1727`): the union of
/// every MF=3 section's own energy points (each section starts at its
/// zero-extension-trimmed first point), then the incident energies of
/// MF=6/MT=2, then extra points wherever a panel is wider than a factor 1.2.
pub fn union_grid(tape: &Tape, mat: i32) -> Result<Vec<f64>, NjoyError> {
    let mf3: Vec<&Section> = tape.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 3).collect();
    let mut old: Vec<f64> = Vec::new();
    for sec in &mf3 {
        let t = tab1_of(sec)?;
        let mut g = Gety1::new(&t);
        let mut enext = g.get(0.0).xnext;
        let mut er = enext;
        let nold = old.len();
        let mut k = 0usize;
        let mut eg = ETOP;
        if nold > 0 {
            k = 1;
            eg = old[0];
        }
        let mut new: Vec<f64> = Vec::new();
        let test = ETOP - ETOP / 100.0;
        loop {
            if er > eg {
                new.push(eg);
                let last = enext > test && k == nold;
                if k < nold {
                    k += 1;
                    eg = old[k - 1];
                } else {
                    eg = ETOP;
                }
                if last {
                    break;
                }
            } else {
                enext = g.get(er).xnext;
                new.push(er);
                let last = enext > test && k == nold;
                if er == eg && k < nold {
                    k += 1;
                    eg = old[k - 1];
                }
                if last {
                    break;
                }
                er = enext;
            }
        }
        old = new;
    }

    // Add the incident energies of MF=6/MT=2.
    let s62 = tape.section(mat, 6, 2).ok_or_else(|| NjoyError::EndfParse("acer: charged-particle table needs MF=6/MT=2".into()))?;
    let mut cur = SectionCursor::new(&s62.rows);
    cur.read_cont()?;
    cur.read_tab1()?;
    let t2 = cur.read_tab2()?;
    let nee = t2.head.n2 as usize;
    let mut new: Vec<f64> = Vec::new();
    let nold = old.len();
    let mut k = 1usize;
    let mut eg = old[0];
    // Upstream's `c` is the last record `finda` fetched or `loada` wrote, and
    // the `eg < ee` loop writes `c`, not `eg`: after an `ee` that is not a grid
    // point `c` still holds `ee`, so the next pass repeats it and the grid
    // point that followed is lost (deck 50: 4.4416 MeV; `acefc.f90:1637-1662`).
    let mut c1 = old[0];
    let fetch = |k: usize| if k <= nold { old[k - 1] } else { ETOP };
    for iee in 1..=nee {
        let l = cur.read_list()?;
        let ee = l.head.c2;
        while eg < ee {
            new.push(c1);
            k += 1;
            c1 = fetch(k);
            eg = c1;
        }
        new.push(ee);
        c1 = ee;
        if eg == ee && iee != nee {
            k += 1;
            c1 = fetch(k);
            eg = c1;
        }
    }
    let old = new;
    let nold = old.len();

    // Extra points for panels with large steps.
    let mut out: Vec<f64> = Vec::new();
    let mut k = 1usize;
    let mut eg = old[0];
    let mut c1 = old[0];
    while k < nold {
        k += 1;
        c1 = old[k - 1];
        if k < nold {
            let er = c1;
            let mut egrid = 0.0;
            while egrid < er {
                out.push(eg);
                egrid = sigfig(STEP * eg, 2, 0);
                if egrid < er {
                    eg = egrid;
                }
            }
            eg = er;
        }
    }
    out.push(c1);
    Ok(out)
}

/// One MF=3 reaction carried onto the union grid, as `unionx` writes it to its
/// scratch tape: the TAB1 head's `QI`, and the `(E, sigma)` pairs from the
/// first grid point at or above the threshold (text-rounded when the PENDF is
/// text, i.e. `npend > 0`).
struct Reaction {
    mt: i32,
    qi: f64,
    tab: Tab1,
}

fn include_in_union(mt: i32) -> bool {
    // `unionx`, `acefc.f90:1710-1760` (iverf >= 6; MT=3/4 redundancy and the
    // MF=12/16 checks are done by the caller).
    !(matches!(mt, 26 | 27 | 101 | 120 | 151)
        || (208..=221).contains(&mt) && mt > 207
        || (221..=260).contains(&mt)
        || (251..=300).contains(&mt) && mt > 250
        || (302..444).contains(&mt)
        || (445..600).contains(&mt)
        || (851..=870).contains(&mt)
        || mt > 900)
}

fn carry_reactions(tape: &Tape, mat: i32, grid: &[f64], text: bool) -> Result<Vec<Reaction>, NjoyError> {
    use crate::endf::parse::{format_endf_float, parse_endf_float};
    let rt = |x: f64| if text { parse_endf_float(&format_endf_float(x)).unwrap_or(x) } else { x };
    let mut out = Vec::new();
    for sec in tape.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 3) {
        let mt = sec.key.mt;
        if !include_in_union(mt) {
            continue;
        }
        if mt == 3 || mt == 4 {
            return Err(NjoyError::NotPorted("ACER of an incident charged particle with MF=3 MT=3/4 (redundancy checks, acefc.f90:1713)"));
        }
        let t = tab1_of(sec)?;
        let mut g = Gety1::new(&t);
        let mut thresh = g.get(0.0).xnext;
        if matches!(mt, 2 | 3 | 102 | 301 | 444) {
            thresh = ELOW;
        }
        let mut pairs: Vec<(f64, f64)> = Vec::new();
        let mut ifrst = false;
        for (i, &c1) in grid.iter().enumerate() {
            if c1 >= (1.0 - EPS) * thresh {
                let e = if i == 0 { c1 * (1.0 + EPS) } else { c1 };
                let mut y = g.get(e).y;
                if i > 0 && !ifrst {
                    y = 0.0;
                }
                ifrst = true;
                pairs.push((rt(c1), rt(y)));
            }
        }
        let n = pairs.len() as i32;
        out.push(Reaction {
            mt,
            qi: rt(t.head.c2),
            tab: Tab1 {
                head: crate::endf::records::Cont { c1: rt(t.head.c1), c2: rt(t.head.c2), l1: t.head.l1, l2: t.head.l2, n1: 1, n2: n },
                interp: vec![(n as u32, 2)],
                pairs,
            },
        });
    }
    Ok(out)
}

/// `acelod`'s per-`izai` MT lists (`:5069-5290` counts, `:5390-5700` passes).
mod lists {
    pub fn in_nr(izai: i32, mt: i32, mt5_here: bool) -> bool {
        let hit = match izai {
            1 => {
                (5..=91).contains(&mt)
                    || (152..=154).contains(&mt)
                    || (156..=181).contains(&mt)
                    || (183..=190).contains(&mt)
                    || (194..=196).contains(&mt)
                    || (198..=200).contains(&mt)
                    || (875..=899).contains(&mt)
            }
            1001 => {
                matches!(mt, 5 | 28 | 41 | 42 | 44 | 45 | 103 | 111 | 112 | 115 | 116 | 156 | 159 | 162 | 163 | 164 | 179 | 181 | 183 | 184 | 186 | 190 | 191 | 194 | 196 | 197 | 198 | 199 | 200)
                    || (600..=649).contains(&mt)
            }
            1002 => {
                matches!(mt, 5 | 11 | 32 | 35 | 104 | 114 | 115 | 117 | 157 | 158 | 169 | 170 | 171 | 182 | 183 | 185 | 187 | 192)
                    || (650..=699).contains(&mt)
            }
            1003 => {
                matches!(mt, 5 | 33 | 36 | 105 | 113 | 116 | 154 | 155 | 172 | 173 | 174 | 175 | 182 | 184 | 185 | 188 | 189)
                    || (700..=749).contains(&mt)
            }
            2003 => {
                matches!(mt, 5 | 34 | 106 | 176 | 177 | 178 | 186 | 187 | 188 | 191 | 192 | 193) || (750..=799).contains(&mt)
            }
            2004 => {
                // `mt.eq.35.or.mt.eq.36.and.mt.eq.45` parses as `35 .or. (36 .and. 45)`.
                matches!(mt, 5 | 22 | 23 | 24 | 25 | 29 | 30 | 35 | 107 | 108 | 109 | 112 | 113 | 114 | 117 | 155 | 158 | 159 | 165 | 166 | 167 | 168 | 180 | 181 | 189 | 193 | 195 | 196 | 199)
                    || (800..=849).contains(&mt)
            }
            _ => false,
        };
        hit && (mt != 5 || mt5_here)
    }
}

/// ACER for an incident charged particle: the continuous-energy table of
/// `acefc` with `izai != 1` for the MF=3/MF=4/MF=6 blocks.
///
/// # Errors
/// [`NjoyError::NotPorted`] for the branches not yet ported (see the module
/// documentation), [`NjoyError::EndfParse`] for malformed records.
pub fn build_charged(tape: &Tape, mat: i32, kt_mev: f64, suffix: u32, text_pendf: bool, text_endf: bool) -> Result<AceTable, NjoyError> {
    let izai = incident_particle(tape, mat);
    if izai == 1 {
        return Err(NjoyError::InvalidInput("build_charged: the evaluation is an incident-neutron tape".into()));
    }
    let letter = class_letter(izai)?;
    let head = tape.section(mat, 1, 451).ok_or_else(|| NjoyError::EndfParse("acer: no MF=1/MT=451".into()))?;
    let za = head.rows[0][0].round() as i32;
    let awr = head.rows[0][1];
    let awi = head.rows[2][0];

    let mut grid = union_grid(tape, mat)?;
    // `acelod` reads the grid back from the total with `gety1`, which steps to the
    // next abscissa ABOVE the current one: a repeated energy appears once.
    grid.dedup();
    let nes = grid.len();
    let reactions = carry_reactions(tape, mat, &grid, text_pendf)?;

    // `mt5n..mt5a` (`first`): MT=5 carries a production of the species -> 0.
    let mut mt5 = [true; 6]; // n, p, d, t, he3, a: true = "no production in MT=5"
    if let Some(s5) = tape.section(mat, 6, 5) {
        let mut cur = SectionCursor::new(&s5.rows);
        let h = cur.read_cont()?;
        for _ in 0..h.n1 {
            let t = cur.read_tab1()?;
            let z = t.head.c1;
            for (slot, zz) in [(0, 1.0), (1, 1001.0), (2, 1002.0), (3, 1003.0), (4, 2003.0), (5, 2004.0)] {
                if (z - zz).abs() < 0.0001 {
                    mt5[slot] = false;
                }
            }
            super::energy::mf6::skip_mf6_subsection(&mut cur, t.head.l2)?;
        }
    }
    let mt5_here = match izai {
        1 => !mt5[0],
        1001 => !mt5[1],
        1002 => !mt5[2],
        1003 => !mt5[3],
        2003 => !mt5[4],
        _ => !mt5[5],
    };
    let kt = kt_mev;
    let es: Vec<f64> = grid.iter().map(|&e| if text_pendf { rt_text(e) } else { e }).collect();

    // ---- which reactions are stored, in `acelod`'s order -------------------
    let mut ntr = 0usize;
    let mut nr = 0usize;
    let mut mtnr = 0;
    let mut mtntr = 0;
    for r in &reactions {
        if r.mt == 1 {
            mtntr = r.mt;
            continue;
        }
        if r.mt != 2 && (r.mt != 5 || mt5_here) {
            ntr += 1;
            if lists::in_nr(izai, r.mt, mt5_here) {
                nr += 1;
                mtnr = r.mt;
            }
        }
        mtntr = r.mt;
    }
    if reactions.iter().any(|r| r.mt == 301) {
        return Err(NjoyError::NotPorted("ACER of an incident charged particle with HEATR's MT=301"));
    }
    let mut order: Vec<&Reaction> = Vec::new();
    let mut elastic: Option<&Reaction> = None;
    // pass 1: reactions that produce the projectile kind
    for r in &reactions {
        if r.mt == 2 {
            elastic = Some(r);
        }
    }
    for r in &reactions {
        if r.mt == 1 || r.mt == 2 {
            continue;
        }
        if r.mt <= mtnr && lists::in_nr(izai, r.mt, mt5_here) {
            order.push(r);
        }
    }
    // pass 2: the rest, from MT=2 up to the last MF=3 section
    for r in &reactions {
        if r.mt == 1 || r.mt == 2 || r.mt > mtntr {
            continue;
        }
        if !lists::in_nr(izai, r.mt, true) {
            order.push(r);
        }
    }
    if order.len() != ntr {
        return Err(NjoyError::NotPorted("ACER of an incident charged particle: reaction bookkeeping differs from acelod (MT=5 without production?)"));
    }

    // ---- ESZ and SIG --------------------------------------------------------
    let mut tot = vec![0.0f64; nes];
    let mut elas = vec![0.0f64; nes];
    if let Some(r) = elastic {
        let mut g = Gety1::new(&r.tab);
        g.get(0.0);
        for j in 0..nes {
            let s = sigfig(g.get(es[j]).y, 7, 0);
            elas[j] = s;
            tot[j] += s;
        }
    }
    let mut disap = vec![0.0f64; nes];
    let mut sig: Vec<(usize, Vec<f64>)> = Vec::new();
    for (ir, r) in order.iter().enumerate() {
        let mut g = Gety1::new(&r.tab);
        let enext = g.get(0.0).xnext;
        let mut j = nes;
        while j >= 1 && enext <= (1.0 + EPS) * es[j - 1] {
            j -= 1;
        }
        let ie = j + 1;
        let mut vals = Vec::new();
        let mut n = 0;
        let mut jj = ie;
        while jj <= nes {
            let v = g.get(es[jj - 1]);
            let mut s = v.y;
            if jj > 1 && n == 0 && v.xnext > 100.0 {
                s = 0.0;
            }
            s = sigfig(s, 7, 0);
            if (6..=9).contains(&r.mt) || (46..=49).contains(&r.mt) {
                s = sigfig(s / 2.0, 7, 0);
            }
            if ir < nr {
                // pass 1: reactions producing the projectile kind
                if r.mt >= 5 {
                    tot[jj - 1] += s;
                }
            } else if r.mt <= 200 || r.mt >= 600 {
                // pass 2: also the disappearance column (`:5663-5674`)
                disap[jj - 1] += s;
                tot[jj - 1] += s;
            }
            vals.push(s);
            n += 1;
            jj += 1;
        }
        sig.push((ie, vals));
    }

    // ---- elastic angular distribution (MF=6/MT=2, LAW=5) ----------------------
    let s62 = tape.section(mat, 6, 2).ok_or_else(|| NjoyError::EndfParse("acer: no MF=6/MT=2".into()))?;
    let (and_words, tot_new, elas_new) = acecpe(s62, izai, awi, za, &es, &tot, &elas)?;
    let tot = tot_new;
    let elas = elas_new;

    // ---- DLW entries (TYR needs them) ---------------------------------------
    let mut dlws = Vec::new();
    let mut off = 1i32;
    let mut tyrs = vec![0i32; ntr];
    for (i, r) in order.iter().take(nr).enumerate() {
        let sec = tape
            .section(mat, 6, r.mt)
            .ok_or(NjoyError::NotPorted("ACER of an incident charged particle: reaction without MF=6 (MF=4/MF=5 route)"))?;
        let law = super::acelf6::first_law_of_mf6(sec)?;
        if law != 1 && law != 6 {
            return Err(NjoyError::NotPorted("ACER of an incident charged particle: MF=6 LAW 2/3 reactions"));
        }
        let q = sigfig(r.qi / EMEV, 7, 0);
        let entry = super::acelf6::acelf6_incident(sec, r.mt, q, za, izai, true)?
            .ok_or(NjoyError::NotPorted("acelf6 returned nothing"))?;
        tyrs[i] = match entry.tyr {
            super::acelf6::Acelf6Tyr::Fixed(t) => t,
            super::acelf6::Acelf6Tyr::Generalized { sign } => sign * (100 + off),
        };
        let l = off + entry.yield_len as i32;
        let n = entry.words.len() as i32;
        dlws.push((off, l, entry));
        off += n;
    }

    // ---- assemble XSS -----------------------------------------------------------
    let mut b = XssBuilder::new();
    for &e in &es {
        b.real(sigfig(e / EMEV, 9, 0));
    }
    for &v in &tot {
        b.real(sigfig(v, 9, 0));
    }
    for &v in &disap {
        b.real(sigfig(v, 9, 0));
    }
    for &v in &elas {
        b.real(v);
    }
    for _ in 0..nes {
        b.real(0.0);
    }
    let mut jx = [0i32; 32];
    jx[jxs::ESZ] = 1;
    jx[jxs::MTR] = b.next_locator();
    for r in &order {
        b.int(r.mt);
    }
    jx[jxs::LQR] = b.next_locator();
    for r in &order {
        b.real(sigfig(r.qi / EMEV, 7, 0));
    }
    jx[jxs::TYR] = b.next_locator();
    for &t in &tyrs {
        b.int(t);
    }
    jx[jxs::LSIG] = b.next_locator();
    let mut loc = 1usize;
    for (_, v) in &sig {
        b.int(loc as i32);
        loc += 2 + v.len();
    }
    jx[jxs::SIG] = b.next_locator();
    for (ie, v) in &sig {
        b.int(*ie as i32);
        b.int(v.len() as i32);
        v.iter().for_each(|&x| b.real(x));
    }
    jx[jxs::LAND] = b.next_locator();
    b.int(1);
    for _ in 0..nr {
        b.int(-1);
    }
    jx[jxs::AND] = b.next_locator();
    for (v, is_int) in &and_words {
        b.word(*v, *is_int);
    }
    jx[jxs::LDLW] = b.next_locator();
    for (_, l, _) in &dlws {
        b.int(*l);
    }
    jx[jxs::DLW] = b.next_locator();
    for (start, _, e) in &dlws {
        for (v, is_int) in e.resolve(*start) {
            b.word(v, is_int);
        }
    }
    // ---- photon production: GPD, MTRP..DLWP, YP ----------------------------
    let pendf = raw_pendf(tape, mat, za, awr)?;
    let mut ntrp = 0i32;
    if let Some(entries) = super::photon_blocks::build_with_pendf_mode(tape, mat, &pendf, text_endf) {
        if !entries.is_empty() {
            if let Some(g) = super::photon_blocks::gpd_on_grid(tape, mat, &pendf, &es) {
                jx[11] = b.next_locator();
                g.iter().for_each(|&v| b.real(v));
            }
            let (mtrp, lsigp, sigp, landp, andp, ldlwp, dlwp) = super::build::append_photon_blocks(&mut b, &entries, &es);
            jx[jxs::MTRP] = mtrp;
            jx[jxs::LSIGP] = lsigp;
            jx[jxs::SIGP] = sigp;
            jx[jxs::LANDP] = landp;
            jx[jxs::ANDP] = andp;
            jx[jxs::LDLWP] = ldlwp;
            jx[jxs::DLWP] = dlwp;
            ntrp = entries.len() as i32;
            jx[jxs::YP] = b.next_locator();
            let mut yp: Vec<i32> = Vec::new();
            let mut mtl = 0;
            for e in entries.iter() {
                let mtd = e.mtrp / 1000;
                let mf13 = !matches!(e.sigp, super::photon_blocks::SigP::Yield { .. });
                if !mf13 && mtd != mtl {
                    mtl = mtd;
                    yp.push(mtd);
                }
            }
            b.int(yp.len() as i32);
            for mt in yp {
                b.int(mt);
            }
        }
    }
    let (xss, is_int) = b.finish();
    jx[jxs::END] = xss.len() as i32;
    let mut nx = [0i32; 16];
    nx[nxs::LEN_XSS] = xss.len() as i32;
    nx[nxs::ZA] = za;
    nx[nxs::NES] = nes as i32;
    nx[nxs::NTR] = ntr as i32;
    nx[nxs::NR] = nr as i32;
    nx[nxs::NTRP] = ntrp;
    nx[nxs::Z] = za / 1000;
    nx[nxs::A] = za % 1000;
    let zaid_num = za as f64 + suffix as f64 / 100.0;
    let mut table = AceTable {
        zaid: format!("{:9.2}{}", zaid_num, letter),
        awr,
        kt_mev: kt,
        iz: [0; 16],
        date: "  njoy-rust".to_string(),
        comment: String::new(),
        mat_id: format!("   mat{mat:4}"),
        nxs: nx,
        jxs: jx,
        xss,
        xss_is_int: is_int,
    };
    if !super::acelcp::recoil_heating_supported(tape, mat) {
        return Err(NjoyError::NotPorted("acelcp: LAW=4 recoil heating of an incident charged particle (acefc.f90:11060)"));
    }
    super::acelcp::append_incident(&mut table, tape, mat, &pendf, 1, izai, awi);
    Ok(table)
}

fn rt_text(x: f64) -> f64 {
    use crate::endf::parse::{format_endf_float, parse_endf_float};
    parse_endf_float(&format_endf_float(x)).unwrap_or(x)
}

/// `acecpe` (`acefc.f90:6523-6670`) for MF=6/MT=2 with `LAW = 5`. Returns the
/// AND words of the elastic reaction and the new total and elastic columns.
/// Only `LTP >= 12` (nuclear amplitude ratio given directly) is ported;
/// `LTP < 12` needs `ptlegc`/`coul` and is refused by name.
fn acecpe(
    sec: &Section,
    izai: i32,
    awi: f64,
    za: i32,
    es: &[f64],
    tot_in: &[f64],
    elas_in: &[f64],
) -> Result<(Vec<(f64, bool)>, Vec<f64>, Vec<f64>), NjoyError> {
    use crate::common::phys::{AMASSN_AMU as AMASSN, AMU_G as AMU, CLIGHT_CM_S as CLIGHT, EV_ERG as EV, HBAR_ERG_S as HBAR, PI};
    use crate::endf::interp::{terp1, IntLaw};
    use petir::mathf::RealMath;
    let fm = 1.0e-12;
    let nes = es.len();
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    let awr = head.c2;
    cur.read_tab1()?;
    let t2 = cur.read_tab2()?;
    let ne = t2.head.n2 as usize;
    let spi = t2.head.c1;
    let lidp = t2.head.l1;
    let mut ie_words: Vec<f64> = Vec::new();
    let mut blocks: Vec<Vec<(f64, bool)>> = Vec::new();
    let mut xxs = Vec::new();
    let mut yys = Vec::new();
    for _ in 0..ne {
        let l = cur.read_list()?;
        let e = l.head.c2;
        let ltp = l.head.l1;
        let mut nl = l.head.n2 as usize;
        let mut data = l.data.clone();
        if ltp < 12 {
            // `ptlegc` rebuilds the distribution as the actual elastic cross
            // section; its LIST gets `LIDP` in the L2 slot first (`:6556`).
            let mut c: Vec<f64> = vec![l.head.c1, l.head.c2, f64::from(l.head.l1), f64::from(lidp), f64::from(l.head.n1), f64::from(l.head.n2)];
            c.extend_from_slice(&l.data);
            let pairs = super::charged_coulomb::ptlegc(&c, awi, izai, awr, za, spi)?;
            nl = pairs.len();
            data = pairs.iter().flat_map(|&(a, b)| [a, b]).collect();
        }
        let mut ien = 1usize;
        while ien < nes && es[ien] < e {
            ien += 1;
        }
        let ien = ien.min(nes - 1).max(1);
        let f = (es[ien] - e) / (es[ien] - es[ien - 1]);
        let xelas = elas_in[ien - 1] * f + elas_in[ien] * (1.0 - f);
        let (mut cumm, mut amul, mut smul, mut sigcl, mut ratr, mut ratrl) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let mut iterp = 0;
        let mut trip: Vec<(f64, f64, f64)> = Vec::new();
        for jl in 1..=nl {
            let mut ione = 0;
            while ione == 0 {
                let mut amuu = data[2 * (jl - 1)];
                let mut pmu = data[2 * (jl - 1) + 1];
                let mut signi;
                let mut sigc;
                loop {
                    let i2s = (2.0 * spi).round() as i32;
                    let ai = awi * AMASSN;
                    let at = awr * AMASSN;
                    let zt = (za / 1000) as f64;
                    let zi = (izai / 1000) as f64;
                    let cc1 = 2.0 * AMU * EV * fm * fm / (HBAR * HBAR);
                    let ee = (EV / 1.0e7) * (CLIGHT / 10.0);
                    let cc2 = ee.r_powi(4) * AMU / (2.0 * HBAR * HBAR * EV);
                    let wn = at * (cc1 * e * ai).sqrt() / (ai + at);
                    let eta = zt * zi * (cc2 * ai / e).sqrt();
                    sigc = 0.0;
                    if lidp == 0 {
                        sigc = (eta * eta / (wn * wn)) / ((1.0 - amuu) * (1.0 - amuu));
                    }
                    if lidp == 1 {
                        let sgn = if i2s % 2 == 0 { 1.0 } else { -1.0 };
                        sigc = ((2.0 * eta * eta / (wn * wn)) / (1.0 - amuu * amuu))
                            * ((1.0 + amuu * amuu) / (1.0 - amuu * amuu)
                                + sgn * (eta * ((1.0 + amuu) / (1.0 - amuu)).r_ln()).r_cos() / (2.0 * spi + 1.0));
                    }
                    if ltp < 12 {
                        pmu -= sigc;
                    }
                    if iterp == 1 {
                        signi = (ratr - 1.0) * sigc;
                    } else {
                        signi = pmu * xelas;
                        if signi < -sigc {
                            signi = -sigc;
                        }
                    }
                    ratr = (sigc + signi) / sigc;
                    if jl > 1 && iterp == 0 && sigc > signi.abs() && sigc > 2.0 * sigcl {
                        iterp = 1;
                        amuu = (amul + amuu) / 2.0;
                        ratr = (ratrl + ratr) / 2.0;
                        continue;
                    }
                    break;
                }
                if jl > 1 {
                    cumm += (amuu - amul) * (signi + sigc + smul) / 2.0;
                }
                trip.push((amuu, signi + sigc, cumm));
                amul = amuu;
                sigcl = sigc;
                smul = signi + sigc;
                ratrl = ratr;
                ione = 1;
                if iterp == 1 {
                    iterp = 0;
                    ione = 0;
                }
            }
        }
        xxs.push(e);
        yys.push(cumm * 2.0 * PI);
        ie_words.push(sigfig(e / EMEV, 7, 0));
        let kk = trip.len();
        let mut blk: Vec<(f64, bool)> = vec![(2.0, true), (kk as f64, true)];
        blk.extend(trip.iter().map(|t| (sigfig(t.0, 7, 0), false)));
        blk.extend(trip.iter().map(|t| (sigfig(t.1 / cumm, 7, 0), false)));
        blk.extend(trip.iter().map(|t| (sigfig(t.2 / cumm, 9, 0), false)));
        blocks.push(blk);
    }
    let mut words: Vec<(f64, bool)> = vec![(ne as f64, true)];
    words.extend(ie_words.iter().map(|&e| (e, false)));
    let mut pos = 1 + 2 * ne + 1;
    for blk in &blocks {
        words.push((-(pos as f64), true));
        pos += blk.len();
    }
    for blk in &blocks {
        words.extend(blk.iter().copied());
    }
    let mut tot = vec![0.0; nes];
    let mut el = vec![0.0; nes];
    for j in 0..nes {
        let e = es[j];
        let signi = elas_in[j];
        let t0 = tot_in[j] - signi;
        let mut jj = 1usize;
        while jj < ne && xxs[jj] < e {
            jj += 1;
        }
        let jj = jj.min(ne - 1).max(1);
        let signow = terp1(xxs[jj - 1], yys[jj - 1], xxs[jj], yys[jj], e, IntLaw::LogLog)?;
        let signow = sigfig(signow, 7, 0);
        tot[j] = sigfig(t0 + signow, 9, 0);
        el[j] = signow;
    }
    Ok((words, tot, el))
}

/// The raw evaluation as the PENDF ACER is handed (decks 14, 50-54, 62, 71
/// pass the ENDF tape as `npend`): MF=3 sections on their own grids.
fn raw_pendf(tape: &Tape, mat: i32, za: i32, awr: f64) -> Result<crate::reconr::ReconrResult, NjoyError> {
    let mut sections = Vec::new();
    for sec in tape.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 3) {
        let t = tab1_of(sec)?;
        sections.push(crate::reconr::ReconrSection {
            lr: t.head.l2,
            mt: crate::endf::mt::MtReaction::from_any(sec.key.mt),
            qi: t.head.c2,
            pairs: t.pairs,
        });
    }
    Ok(crate::reconr::ReconrResult {
        material: crate::reconr::mf1::MaterialInfo { za: f64::from(za), awr, lrp: 0, lfi: 0, nlib: 0, elis: 0.0, nfor: 6, emax: 0.0 },
        sections,
        resonance_upper_limit: None,
        unresolved_table: None,
        temperature_k: 0.0,
    })
}
