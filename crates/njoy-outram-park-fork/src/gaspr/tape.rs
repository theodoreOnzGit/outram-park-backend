// Translated from NJOY2016.80 `src/gaspr.f90` (the `gaspr` subroutine, 1-1153)
// and `src/endf.f90` (`gety1`). NJOY2016 is under a modified BSD 3-Clause
// (LANL/DOE) licence, GPL-compatible; this derivative file is distributed under
// AGPL-3.0-only. This is a modified, non-LANL version, not endorsed by LANL/DOE.
// See crate root LICENSE.njoy + NOTICE.

//! **GASPR's tape driver** (gh:#1369, differential #1366): the PENDF-in,
//! PENDF-out half of `gaspr.f90`, where [`super::GasProduction`] is its typed
//! physics.
//!
//! What it does that the typed API does not, all of it upstream's behaviour and
//! all of it visible on the tape:
//!
//! - the **energy grid** is MT=1's own grid from the threshold `thrg` (the
//!   lowest start of any section that can make gas, computed by a first pass
//!   with its own skip ladder, `gaspr.f90:318-470`), not a union of the
//!   contributing sections' grids;
//! - every contributing section is read with `gety1` (ported here as
//!   [`Gety1`], its `0.999999` shading of a leading zero and all), in tape order;
//! - the old MT=203-207 are deleted, the new ones written after the last
//!   MT <= 200 with the first stored point one grid point **below** the first
//!   non-zero value, and the MF=1/MT=451 directory updated with upstream's
//!   record counts (`(n+2)/3`, which counts neither the HEAD nor the TAB1
//!   record, and `n` taken before the `i = 1` clamp, so one too many when the
//!   first point is non-zero) and its `N2 = 1` in the entry.
//!
//! Not ported: more than one material, a non-neutron incident particle (the
//! typed API's `zain = 1`), the legacy MT=600-849 fallback beyond skipping the
//! levels, and the listing GASPR writes. MF=6/MT=5 multiplicities use
//! [`eval_tab1`] where upstream uses `terpa`.

use crate::acer::energy::mf6::parse_mf6_product_yields;
use crate::endf::gety1::Gety1;
use crate::endf::interp::eval_tab1;
use crate::endf::records::{SectionCursor, Tab1};
use crate::endf::tape::{Section, Tape};
use crate::endf::EndfKey;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

use super::{credit_residual, gas_yield_for};

/// GASPR's `emax` (`gaspr.f90:63`).
const EMAX: f64 = 1.0e10;

/// A section's TAB1 in `gety1`'s reader ([`crate::endf::gety1::Gety1`], the
/// existing port): the reader and the `(xnext, y)` of its `x = 0` call.
fn open(sec: &Section) -> Result<(Gety1, f64, f64), NjoyError> {
    let mut cur = SectionCursor::new(&sec.rows);
    cur.read_cont()?;
    let t = cur.read_tab1()?;
    let mut g = Gety1::new(&t);
    let v = g.get(0.0);
    Ok((g, v.xnext, v.y))
}

/// The MT ranges of the detailed partial-breakup levels, by ENDF version
/// (`gaspr.f90:130-153`): `(mpmin, mpmax, mdmin, mdmax, mtmin, mtmax, m3min,
/// m3max, m4min, m4max)`.
fn level_ranges(iverf: i32) -> [i32; 10] {
    if iverf >= 6 {
        [600, 649, 650, 699, 700, 749, 750, 799, 800, 849]
    } else {
        [700, 718, 720, 738, 740, 758, 760, 768, 780, 798]
    }
}

/// Whether MT in one of the five level ranges (`gaspr.f90:431-435`).
fn in_level_ranges(mt: i32, r: &[i32; 10]) -> bool {
    (0..5).any(|k| mt >= r[2 * k] && mt <= r[2 * k + 1])
}

/// Pass 1 (`gaspr.f90:318-470`): whether this MF=3 section counts toward the
/// gas threshold `thrg`. A transcription of the `izg`/`izr` ladder, goto for
/// goto: `false` is `go to 245`.
fn counts_toward_threshold(mt: i32, lr: i32, za_plus_zain: i32, mf6mt5: bool, ranges: &[i32; 10]) -> bool {
    let mut izg = 0;
    let mut izr = za_plus_zain;
    let (mpmin, m4max) = (ranges[0], ranges[9]);
    if mt > 200 && mt < mpmin {
        return false;
    }
    if mt > m4max || mt == 0 {
        return false;
    }
    if (1..=4).contains(&mt) {
        return false;
    }
    if mt == 5 && mf6mt5 {
        izg = 1;
    }
    if (6..=10).contains(&mt) {
        return false;
    }
    if mt == 11 {
        izg = 1;
        izr -= 1004;
    }
    if (12..=15).contains(&mt) {
        return false;
    }
    if mt == 16 {
        izr -= 2;
    }
    if mt == 17 {
        izr -= 3;
    }
    if (18..=21).contains(&mt) {
        return false;
    }
    if (22..=25).contains(&mt) {
        izg = 1;
    }
    izr -= match mt {
        22 => 2005,
        23 => 6013,
        24 => 2006,
        25 => 2007,
        _ => 0,
    };
    if (26..=27).contains(&mt) {
        return false;
    }
    if (28..=32).contains(&mt) {
        izg = 1;
    }
    izr -= match mt {
        28 => 1002,
        29 => 4009,
        30 => 4010,
        _ => 0,
    };
    if mt == 31 {
        return false;
    }
    if (32..=36).contains(&mt) {
        izg = 1;
    }
    izr -= match mt {
        32 => 1003,
        33 => 1004,
        34 => 2004,
        35 => 5011,
        36 => 5012,
        37 => 4,
        _ => 0,
    };
    if (38..=40).contains(&mt) {
        return false;
    }
    if (41..=42).contains(&mt) {
        izg = 1;
    }
    izr -= match mt {
        41 => 1003,
        42 => 1004,
        _ => 0,
    };
    if mt == 43 {
        return false;
    }
    if (44..=45).contains(&mt) {
        izg = 1;
    }
    izr -= match mt {
        44 => 2003,
        45 => 3006,
        _ => 0,
    };
    if (46..=50).contains(&mt) {
        return false;
    }
    if (51..=91).contains(&mt) {
        izr -= 1;
        if (22..=25).contains(&lr) {
            izg = 1;
        }
        izr -= match lr {
            22 => 2004,
            23 => 6012,
            24 => 2005,
            25 => 2006,
            _ => 0,
        };
        if (28..=30).contains(&lr) {
            izg = 1;
        }
        izr -= match lr {
            28 => 1001,
            29 => 4008,
            30 => 4009,
            _ => 0,
        };
        if (32..=36).contains(&lr) {
            izg = 1;
        }
        izr -= match lr {
            32 => 1002,
            33 => 1003,
            34 => 2003,
            35 => 5010,
            36 => 5011,
            _ => 0,
        };
    }
    if (92..=101).contains(&mt) {
        return false;
    }
    if (103..=117).contains(&mt) {
        izg = 1;
    }
    izr -= match mt {
        103 => 1001,
        104 => 1002,
        105 => 1003,
        106 => 2003,
        107 => 2004,
        _ => 0,
    };
    if in_level_ranges(mt, ranges) {
        return false;
    }
    izr -= match mt {
        108 => 4008,
        109 => 6012,
        111 => 2002,
        112 => 3005,
        113 => 5011,
        114 => 5010,
        115 => 2003,
        116 => 2004,
        117 => 3006,
        _ => 0,
    };
    if (154..=159).contains(&mt) || (162..=200).contains(&mt) {
        izg = 1;
    }
    izr -= match mt {
        152 => 5,
        153 => 6,
        154 => 1005,
        155 => 3007,
        156 => 1005,
        157 => 1005,
        158 => 3007,
        159 => 3007,
        160 => 7,
        161 => 8,
        162 => 1006,
        163 => 1007,
        164 => 1008,
        165 => 2008,
        166 => 2009,
        167 => 2010,
        168 => 2011,
        169 => 1006,
        170 => 1007,
        171 => 1008,
        172 => 1006,
        173 => 1007,
        174 => 1008,
        175 => 1009,
        176 => 2005,
        177 => 2006,
        178 => 2007,
        179 => 2005,
        180 => 4011,
        181 => 3008,
        182 => 2005,
        183 => 2004,
        184 => 2005,
        185 => 2006,
        186 => 3005,
        187 => 3006,
        188 => 3007,
        189 => 3008,
        190 => 2004,
        191 => 3004,
        192 => 3005,
        193 => 4007,
        194 => 2006,
        195 => 4012,
        196 => 3009,
        197 => 3003,
        198 => 3004,
        199 => 4009,
        200 => 2007,
        _ => 0,
    };
    if izr == 4008 {
        izg = 1;
    }
    !(izg == 0 && (izr > 2004 || izr <= 0))
}

/// The yields `(y203, ..., y207)` of one section in pass 2 (`gaspr.f90:476-826`),
/// `None` where upstream skips it (or every yield is zero). MT=5 with MF=6
/// multiplicities carries the flag `111` for each tracked product present.
fn pass2_yields(target_za: i32, mt: i32, lr: i32, mf6mt5: bool, have: [bool; 5], ranges: &[i32; 10]) -> Option<[f64; 5]> {
    let (mpmin, m4max) = (ranges[0], ranges[9]);
    if (mt > 200 && mt < mpmin) || mt > m4max || mt == 0 || in_level_ranges(mt, ranges) {
        return None;
    }
    let g = if mt == 5 && mf6mt5 {
        let flag = |b: bool| if b { 111 } else { 0 };
        let emitted = super::GasYield { p: flag(have[0]), d: flag(have[1]), t: flag(have[2]), he3: flag(have[3]), alpha: flag(have[4]) };
        credit_residual(emitted, target_za + 1)
    } else {
        gas_yield_for(target_za, mt, lr)?
    };
    if g.is_zero() {
        return None;
    }
    Some([f64::from(g.p), f64::from(g.d), f64::from(g.t), f64::from(g.he3), f64::from(g.alpha)])
}

/// `NP` of a TAB1 section (`maxg = nint(a(6))`).
fn np_of(sec: &Section) -> usize {
    sec.rows.get(1).map_or(0, |r| nint(r[5]).max(0) as usize)
}

/// `nint`.
fn nint(x: f64) -> i32 {
    x.round() as i32
}

/// GASPR on a PENDF tape: `gaspr nendf npend noutp` (`gaspr.f90:64-1135`) for a
/// neutron evaluation, one material, any number of temperatures.
///
/// `endf` is the evaluation (unit `nendf`), `pendf` the PENDF after BROADR
/// (and HEATR), unit `npend`. Returns the tape of unit `noutp` (not yet
/// rounded through text: the caller's unit mode decides that).
///
/// # Errors
/// [`NjoyError::NotPorted`] for a PENDF holding more than one material, an
/// incident particle other than the neutron, or a PENDF without MF=2; any
/// other inconsistency in a tape is [`NjoyError::InvalidInput`].
pub fn gaspr_tape(endf: &Tape, pendf: &Tape) -> Result<Tape, NjoyError> {
    let bad = |m: &str| NjoyError::InvalidInput(format!("gaspr: {m}"));
    let first = endf.sections().first().ok_or_else(|| bad("empty endf tape"))?.key;
    let matd = first.mat;
    let mf1 = endf.section(matd, 1, 451).ok_or(NjoyError::SectionNotFound { mat: matd, mf: 1, mt: 451 })?;
    let iverf = crate::moder::layout::iverf_from_mf1(&mf1.rows);
    let (za, awr) = (mf1.rows[0][0], mf1.rows[0][1]);
    let nsub = if iverf >= 6 { nint(mf1.rows.get(2).map_or(10.0, |r| r[4])) } else { 10 };
    let zain = nsub / 10;
    if zain != 1 {
        return Err(NjoyError::NotPorted("gaspr for an incident particle other than the neutron"));
    }
    let target_za = nint(za);
    let za_plus_zain = nint(za + f64::from(zain));
    let ranges = level_ranges(iverf);
    let m4max = ranges[9];

    // MF=6/MT=5 multiplicities of the five tracked products (`:100-240`).
    let mf6mt5 = endf.section(matd, 6, 5);
    let mut mult: [Option<Tab1>; 5] = Default::default();
    if let Some(sec) = mf6mt5 {
        for (zap, tab) in parse_mf6_product_yields(sec)? {
            if let Some(k) = [1001, 1002, 1003, 2003, 2004].iter().position(|&z| z == zap) {
                mult[k] = Some(tab); // the last of a repeated ZAP wins (`lsix` overwritten)
            }
        }
    }
    let have = [0, 1, 2, 3, 4].map(|k| mult[k].is_some());
    let has_mf6mt5 = mf6mt5.is_some();

    // Split the PENDF into temperature blocks.
    let mut blocks: Vec<Vec<Section>> = Vec::new();
    for s in pendf.sections() {
        if s.key.mat != matd {
            return Err(NjoyError::NotPorted("gaspr on a PENDF holding another material than the evaluation's first"));
        }
        if s.key.mf == 1 && s.key.mt == 451 {
            blocks.push(Vec::new());
        }
        blocks.last_mut().ok_or_else(|| bad("PENDF does not begin with MF=1/MT=451"))?.push(s.clone());
    }

    let mut out: Vec<Section> = Vec::new();
    let mut raw_mf1: Option<Vec<String>> = None;
    for block in &blocks {
        if !block.iter().any(|s| s.key.mf == 2) {
            return Err(NjoyError::NotPorted("gaspr on a PENDF without MF=2 (its second tofend would eat file 3)"));
        }
        // The scratch tape's MF=3 (`:260-290`): every section up to the first MT > m4max.
        let mut scratch: Vec<&Section> = Vec::new();
        for s in block.iter().filter(|s| s.key.mf == 3) {
            if s.key.mt > m4max {
                break;
            }
            scratch.push(s);
        }
        let lr_of = |s: &Section| s.rows.get(1).map_or(0, |r| nint(r[3]));

        // Pass 1: the gas threshold.
        let mut thrg = EMAX;
        for s in &scratch {
            let (_, enext, _) = open(s)?;
            if counts_toward_threshold(s.key.mt, lr_of(s), za_plus_zain, has_mf6mt5, &ranges) && enext < thrg {
                thrg = enext;
            }
        }

        // The grid: MT=1's own, from `thrg` (`:472-496`).
        let mt1 = scratch.iter().find(|s| s.key.mt == 1).ok_or_else(|| bad("no MT=1 on the PENDF"))?;
        let (mut g1, _, _) = open(mt1)?;
        let maxg = np_of(mt1);
        let mut egas: Vec<f64> = Vec::new();
        let mut enext = thrg;
        while enext < EMAX {
            if egas.len() + 1 > maxg {
                return Err(bad("too many gas production energy points"));
            }
            let en = enext;
            enext = g1.get(en).xnext;
            egas.push(en);
        }
        let ngas = egas.len();
        let mut sgas = vec![[0.0f64; 5]; ngas];

        // Pass 2: sum the yields (`:501-841`).
        for s in &scratch {
            let Some(yl) = pass2_yields(target_za, s.key.mt, lr_of(s), has_mf6mt5, have, &ranges) else { continue };
            let (mut g, _, _) = open(s)?;
            for (i, &en) in egas.iter().enumerate() {
                let y = g.get(en).y;
                for j in 0..5 {
                    if yl[j] > 100.0 {
                        let yyy = match &mult[j] {
                            Some(t) => eval_tab1(en, &t.interp, &t.pairs)?,
                            None => 0.0,
                        };
                        sgas[i][j] += yyy * y;
                    } else {
                        sgas[i][j] += yl[j] * y;
                    }
                }
            }
        }

        // Directory counts (`:870-886`).
        let mut n_dir = [0usize; 5];
        let mut nsec = 0usize;
        for jg in 0..5 {
            if let Some(i0) = (0..ngas).find(|&i| sgas[i][jg] != 0.0) {
                // upstream's `ii` is the 1-based index minus one: `n = ngas - ii + 1`.
                nsec += 1;
                n_dir[jg] = ngas - i0 + 1;
            }
        }

        // MF=1/MT=451 with the new directory.
        let old = block.iter().find(|s| s.key.mf == 1 && s.key.mt == 451).ok_or_else(|| bad("block without MF=1/MT=451"))?;
        let thead = match iverf {
            4 => 1,
            5 => 2,
            _ => 3,
        };
        let ncards = nint(old.rows[thead][4]) as usize;
        let dstart = thead + 1 + ncards;
        let entries: Vec<[f64; 6]> = old.rows[dstart..].to_vec();
        let is_old_gas = |e: &[f64; 6]| nint(e[2]) == 3 && (203..=207).contains(&nint(e[3]));
        let nold = entries.iter().filter(|e| is_old_gas(e)).count();
        let ins = entries.iter().position(|e| nint(e[2]) > 3 || (nint(e[2]) == 3 && nint(e[3]) > 200)).unwrap_or(entries.len());
        let mut dict: Vec<[f64; 6]> = entries[..ins].to_vec();
        for jg in 0..5 {
            if n_dir[jg] != 0 {
                dict.push([0.0, 0.0, 3.0, f64::from(203 + jg as i32), ((n_dir[jg] + 2) / 3) as f64, 1.0]);
            }
        }
        dict.extend(entries[ins..].iter().filter(|e| !is_old_gas(e)));
        let nx_new = (entries.len() + nsec - nold) as f64;
        let mut rows = old.rows[..dstart].to_vec();
        if iverf == 4 {
            rows[0][5] = nx_new;
        } else {
            rows[thead][5] = nx_new;
        }
        rows.extend(dict);
        if raw_mf1.is_none() {
            raw_mf1 = pendf.raw_text(old.key).map(|r| {
                let mut v: Vec<String> = r.iter().take(dstart).cloned().collect();
                v.resize(rows.len(), String::new());
                v
            });
        }
        out.push(Section { key: old.key, rows });

        // MF=2 and MF=3 up to MT=200, then the new sections, then the rest
        // without the old gas sections (`:889-1006`).
        let mut wrote_gas = false;
        let mut in_file3_head = true;
        for s in block.iter().filter(|s| !(s.key.mf == 1 && s.key.mt == 451)) {
            if (s.key.mf == 3 && s.key.mt > 200) || s.key.mf > 3 {
                in_file3_head = false;
            }
            if !in_file3_head && !wrote_gas {
                write_gas(&mut out, matd, za, awr, &egas, &sgas);
                wrote_gas = true;
            }
            if s.key.mf == 3 && (203..=207).contains(&s.key.mt) {
                continue;
            }
            out.push(s.clone());
        }
        if !wrote_gas {
            write_gas(&mut out, matd, za, awr, &egas, &sgas);
        }
    }
    let mut tape = Tape::from_sections(pendf.tpid.clone(), out);
    tape.copy_raw_mf32_from(pendf);
    if let Some(r) = raw_mf1 {
        tape.set_raw_text(EndfKey { mat: matd, mf: 1, mt: 451 }, r);
    }
    Ok(tape)
}

/// The MF=3 sections MT=203-207 (`gaspr.f90:1037-1103`): from one grid point
/// below the first non-zero value (not below the first point), `sigfig(.,7,0)`.
fn write_gas(out: &mut Vec<Section>, mat: i32, za: f64, awr: f64, egas: &[f64], sgas: &[[f64; 5]]) {
    let ngas = egas.len();
    for jg in 0..5 {
        let Some(i0) = (0..ngas).find(|&i| sgas[i][jg] != 0.0) else { continue };
        let istart = i0.saturating_sub(1);
        let np = ngas - istart;
        let mut rows = vec![[za, awr, 0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0, 1.0, np as f64], [np as f64, 2.0, 0.0, 0.0, 0.0, 0.0]];
        for chunk in (istart..ngas).collect::<Vec<_>>().chunks(3) {
            let mut row = [0.0f64; 6];
            for (k, &i) in chunk.iter().enumerate() {
                row[2 * k] = egas[i];
                row[2 * k + 1] = sigfig(sgas[i][jg], 7, 0);
            }
            rows.push(row);
        }
        out.push(Section { key: EndfKey { mat, mf: 3, mt: 203 + jg as i32 }, rows });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pass1(mt: i32, lr: i32, za: i32, mf6: bool) -> bool {
        counts_toward_threshold(mt, lr, za + 1, mf6, &level_ranges(6))
    }

    /// Every branch of the pass-1 ladder (`gaspr.f90:318-470`), on the cases
    /// the Fortran decides by hand: the `go to 245` skips, the `izg` flag, and
    /// the `izr` window (0, 2004] that lets MT=102 count only on light targets.
    #[test]
    fn pass1_ladder_follows_the_fortran() {
        // Ni-61 (ZA 28061): only the explicit particle reactions count.
        assert!(!pass1(2, 0, 28061, false), "MT<=4 skipped");
        assert!(!pass1(102, 0, 28061, false), "capture on Ni-61 leaves a residual above He-4");
        assert!(pass1(103, 0, 28061, false), "(n,p) izg = 1");
        assert!(pass1(107, 0, 28061, false), "(n,a)");
        assert!(pass1(22, 0, 28061, false) && pass1(28, 0, 28061, false) && pass1(41, 0, 28061, false));
        assert!(!pass1(16, 0, 28061, false), "(n,2n) leaves Ni-60");
        assert!(!pass1(18, 0, 28061, false) && !pass1(91, 0, 28061, false) && !pass1(12, 0, 28061, false));
        assert!(pass1(11, 0, 28061, false), "(n,2nd) izg = 1");
        // MT=5 counts only with MF=6 multiplicities.
        assert!(!pass1(5, 0, 28061, false));
        assert!(pass1(5, 0, 28061, true));
        // Levels: LR breakup flags decide MT=51-91.
        assert!(!pass1(51, 0, 28061, false));
        assert!(pass1(51, 22, 28061, false) && pass1(60, 28, 28061, false) && pass1(91, 32, 28061, false));
        assert!(!pass1(51, 31, 28061, false), "LR=31 is not a breakup flag");
        // Light targets: the residual itself is a gas (izr in (0, 2004]).
        assert!(pass1(102, 0, 1002, false), "H-2(n,g)H-3: izr = 1003");
        assert!(pass1(16, 0, 4009, false), "Be-9(n,2n)Be-8: izr = 4008, izg = 1");
        assert!(!pass1(102, 0, 8016, false));
        // Other MF=3 numbers: sums above 200 and the level ranges are skipped.
        assert!(!pass1(301, 0, 1002, false) && !pass1(444, 0, 1002, false));
        assert!(!pass1(600, 0, 28061, false) && !pass1(849, 0, 28061, false));
        assert!(pass1(185, 0, 28061, false) && !pass1(152, 0, 28061, false) && !pass1(31, 0, 28061, false));
        // Pre-ENDF-6 level ranges differ: MT=700 is a level, 600 is not.
        let r5 = level_ranges(5);
        assert!(!counts_toward_threshold(700, 0, 28062, false, &r5));
        assert!(in_level_ranges(760, &r5) && !in_level_ranges(719, &r5));
        assert!(in_level_ranges(849, &level_ranges(6)) && !in_level_ranges(850, &level_ranges(6)));
    }

    /// Pass 2's yields: named ejectiles, the residual credit, MT=5's `111` flag.
    #[test]
    fn pass2_yields_follow_the_fortran() {
        let r = level_ranges(6);
        let none = [false; 5];
        assert_eq!(pass2_yields(28061, 103, 0, false, none, &r), Some([1.0, 0.0, 0.0, 0.0, 0.0]));
        assert_eq!(pass2_yields(28061, 107, 0, false, none, &r), Some([0.0, 0.0, 0.0, 0.0, 1.0]));
        assert_eq!(pass2_yields(28061, 24, 0, false, none, &r), Some([0.0, 0.0, 0.0, 0.0, 1.0]));
        assert_eq!(pass2_yields(28061, 102, 0, false, none, &r), None, "yields all zero");
        assert_eq!(pass2_yields(28061, 2, 0, false, none, &r), None);
        assert_eq!(pass2_yields(28061, 600, 0, false, none, &r), None, "levels");
        assert_eq!(pass2_yields(3006, 105, 0, false, none, &r), Some([0.0, 0.0, 1.0, 0.0, 1.0]), "Li-6(n,t)He-4: the residual alpha is credited");
        assert_eq!(pass2_yields(4009, 16, 0, false, none, &r), Some([0.0, 0.0, 0.0, 0.0, 2.0]), "Be-8 counts two alphas");
        let have = [true, true, false, false, false];
        assert_eq!(pass2_yields(28061, 5, 0, true, have, &r), Some([111.0, 111.0, 0.0, 0.0, 0.0]));
        assert_eq!(pass2_yields(28061, 5, 0, true, none, &r), None, "MT=5 with MF=6 but none of the five products");
    }

    fn tab1_section(mt: i32, pairs: &[(f64, f64)]) -> Section {
        let np = pairs.len() as f64;
        let mut rows = vec![[28061.0, 60.0, 0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0, 1.0, np], [np, 2.0, 0.0, 0.0, 0.0, 0.0]];
        for ch in pairs.chunks(3) {
            let mut r = [0.0; 6];
            for (k, &(x, y)) in ch.iter().enumerate() {
                r[2 * k] = x;
                r[2 * k + 1] = y;
            }
            rows.push(r);
        }
        Section { key: EndfKey { mat: 2834, mf: 3, mt }, rows }
    }

    /// `open` on a leading-zero table: `xnext` is the last zero before the
    /// first non-zero value, shaded by 0.999999 unless it is the first point
    /// (`gety1`'s "zero extension", `endf.f90:1497-1516`); a section starting
    /// non-zero gives its first point. The threshold comes from this.
    #[test]
    fn open_gives_the_gas_threshold_point() {
        let s = tab1_section(103, &[(1.0e-5, 0.0), (1.0e6, 0.0), (2.0e6, 0.0), (3.0e6, 1.0), (4.0e6, 2.0)]);
        let (_, xnext, y) = open(&s).unwrap();
        assert_eq!(y, 0.0);
        assert_eq!(xnext, 2.0e6 * 0.999_999);
        let s = tab1_section(103, &[(1.0e-5, 0.0), (1.0e6, 1.0)]);
        assert_eq!(open(&s).unwrap().1, 1.0e-5, "first point unshaded");
        let s = tab1_section(2, &[(1.0e-5, 5.0), (1.0e6, 1.0)]);
        assert_eq!(open(&s).unwrap().1, 1.0e-5);
        assert_eq!(np_of(&s), 2);
        assert!(open(&Section { key: s.key, rows: vec![s.rows[0]] }).is_err());
    }

    /// `write_gas`: the first stored point is one grid point below the first
    /// non-zero value, clamped to the first point; zero products are not written.
    #[test]
    fn write_gas_starts_one_point_below_the_first_nonzero() {
        let egas = [1.0, 2.0, 3.0, 4.0];
        let sgas = [[0.0, 0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0, 1.0], [5.0, 0.0, 0.0, 0.0, 2.0], [6.0, 0.0, 0.0, 0.0, 3.0]];
        let mut out = Vec::new();
        write_gas(&mut out, 2834, 28061.0, 60.0, &egas, &sgas);
        let mts: Vec<i32> = out.iter().map(|s| s.key.mt).collect();
        assert_eq!(mts, vec![203, 207]);
        // MT=203: first non-zero at index 2, so it starts at index 1 (E = 2): 3 points.
        assert_eq!(out[0].rows[1][5], 3.0);
        assert_eq!(out[0].rows[3][0], 2.0);
        // MT=207: first non-zero at index 1: starts at index 0, 4 points.
        assert_eq!(out[1].rows[1][5], 4.0);
        assert_eq!(out[1].rows[3][0], 1.0);
        assert_eq!(out[1].rows[3][1], 0.0);
    }
}
