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
    for iee in 1..=nee {
        let l = cur.read_list()?;
        let ee = l.head.c2;
        while eg < ee {
            new.push(eg);
            k += 1;
            eg = if k <= nold { old[k - 1] } else { ETOP };
        }
        new.push(ee);
        if eg == ee && iee != nee {
            k += 1;
            eg = if k <= nold { old[k - 1] } else { ETOP };
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
pub fn build_charged(tape: &Tape, mat: i32, kt_mev: f64, suffix: u32, text_pendf: bool) -> Result<AceTable, NjoyError> {
    let izai = incident_particle(tape, mat);
    if izai == 1 {
        return Err(NjoyError::InvalidInput("build_charged: the evaluation is an incident-neutron tape".into()));
    }
    let letter = class_letter(izai)?;
    let head = tape.section(mat, 1, 451).ok_or_else(|| NjoyError::EndfParse("acer: no MF=1/MT=451".into()))?;
    let za = head.rows[0][0].round() as i32;
    let awr = head.rows[0][1];
    let awi = head.rows[2][0];

    let grid = union_grid(tape, mat)?;
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
    let _ = (awi, mt5_here, kt_mev, suffix, letter, za, awr, nes, reactions.len(), jxs::ESZ, nxs::ZA);
    let _ = XssBuilder::new();
    Err(NjoyError::NotPorted("ACER of an incident charged particle: table assembly in progress (gh:#1485)"))
}
