// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - the input section of `subroutine errorr`, l.420-640 and l.861-938;
//   - `gengpn`'s card 12 (`groupr.f90:4157-4161`), `egnwtf`'s card 13b.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **ERRORR from a deck** (gh:#1369): its cards, and the runs on an input GENDF or
//! a PENDF.
//!
//! The cards are read in upstream's order: card 1 (`nendf npend ngout nout nin
//! nstan`), card 2 (`matd ign iwt iprint irelco`), card 3 (`mprint tempin`), card 7
//! (`iread mfcov irespr legord ifissp efmean dap`, ENDF-5/6 only), card 12 for
//! `ign = +-1` (`egngpn`), card 13b for `iwt = 4` (`egnwtf`). `iverf` is taken from
//! the ENDF unit before card 7, as upstream reads the tape between cards 1 and 2.
//!
//! What runs (all 2026-11-13):
//! - `mfcov = 35` with `ngout /= 0` ([`crate::errorr::mf35`]);
//! - `mfcov = 33` from a PENDF with `ngout = 0` ([`crate::errorr::run_mf33`], its
//!   tape by [`mf33_text`]);
//! - `mfcov = 31`, and `33` with `ngout /= 0`, from a GENDF
//!   ([`crate::errorr::mf31::run_cov_gendf`]), with `nin /= 0` copied ahead
//!   ([`copy_nin_ahead`]); `mfcov = 40` the same way (2026-11-14, gh:#1371);
//! - `mfcov = 34` from a GENDF or a PENDF ([`crate::errorr::mf34`]);
//! - `nendf = 999` ([`covadd_text`]).
//!
//! Every other combination is read and then refused by name, so a deck stops here
//! with its cards consumed.

use crate::deck::cards::{CardReader, Slot};
use crate::deck::tapes::TapeUnits;
use crate::deck::groupr::GendfText;
use crate::errorr::gendf::GendfTape;
use crate::errorr::mf31::{run_cov_gendf, run_cov_gendf_card10};
use crate::errorr::mf34::{run_mf34_gendf, Mf34Config};
use crate::errorr::mf35::{run_mf35, Mf35Config};
use crate::errorr::weight::ErrorrWeight;
use crate::errorr::{run_mf33, Mf33Config};
use crate::NjoyError;

/// ERRORR's cards as read.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorrCards {
    /// Card 1: `nendf, npend, ngout, nout, nin, nstan`.
    pub units: [i32; 6],
    /// Card 2: `matd, ign, iwt, iprint, irelco`.
    pub card2: [i32; 5],
    /// Card 3: `mprint`, `tempin`.
    pub mprint: i32,
    /// Card 3's `tempin` (K).
    pub tempin: f64,
    /// Card 7: `iread, mfcov, irespr, legord, ifissp`.
    pub card7: [i32; 5],
    /// Card 7's `efmean` (eV).
    pub efmean: f64,
    /// Card 7's `dap`.
    pub dap: f64,
    /// Card 12 bounds (`ign = +-1`).
    pub egn: Option<Vec<f64>>,
    /// Card 13b (`iwt = 4`): `eb, tb, ec, tc`.
    pub wght: Option<(f64, f64, f64, f64)>,
    /// Card 13 (`iwt = 1`): the TAB1 weight as `egnwtf` keeps it, `win(1..iww)`
    /// (gh:#1371).
    pub tab1: Option<Vec<f64>>,
    /// Card 10 (`iread = 2`): the `mat1 mt1` pairs up to `mat1 = 0`
    /// (`errorr.f90:907-919`), gh:#1369. Empty for `iread = 0`.
    pub card10: Vec<(i32, i32)>,
}

/// Read ERRORR's cards (`iverf > 4`), up to the point a refusal would come.
///
/// # Errors
/// A malformed card; [`NjoyError::NotPorted`] for the `nendf = 999` (`covadd`)
/// option, ENDF/B-IV cards, `iread = 1` ~~`iread = 1/2`~~ (card 10, `iread = 2`,
/// is read since 2026-11-13, gh:#1369), `nstan /= 0` ~~and `iwt = 1`~~ (card 13 is
/// read since 2026-11-13, gh:#1371), whose further cards are not read here; and
/// for `iwt = 1` a card 13 that `egnwtf` would cut short, or (`mfcov = 34`) two
/// copies of card 13 or 13b that differ.
pub fn read_errorr(r: &mut CardReader, iverf: i32) -> Result<ErrorrCards, NjoyError> {
    let u = r.read_ints(&[0, 0, 0, 0, 0, 0])?;
    if u[0] == 999 {
        return Err(NjoyError::NotPorted("errorr nendf = 999 (covadd) from a deck"));
    }
    let c2 = r.read_ints(&[0, 1, 6, 1, 1])?;
    let mut s3 = [Slot::Int(0), Slot::Real(300.0)];
    r.read(&mut s3)?;
    if iverf <= 4 {
        return Err(NjoyError::NotPorted("errorr ENDF/B-IV cards 4-6 (iverf = 4)"));
    }
    let mut s7 = [
        Slot::Int(0),
        Slot::Int(33),
        Slot::Int(1),
        Slot::Int(1),
        Slot::Int(-1000),
        Slot::Real(2.0e6),
        Slot::Real(0.0),
    ];
    r.read(&mut s7)?;
    let card7 = [s7[0].int(), s7[1].int(), s7[2].int(), s7[3].int(), s7[4].int()];
    if card7[0] == 1 {
        return Err(NjoyError::NotPorted("errorr iread = 1 (cards 8-9)"));
    }
    // Card 10 (`iread = 2`, `errorr.f90:907-919`): `ii1 = 0` is preset, so a
    // card `0/` or `/` ends the list; `ii2` keeps its last value (gh:#1369).
    let mut card10 = Vec::new();
    if card7[0] == 2 {
        let mut ii2 = 0;
        loop {
            let p = r.read_ints(&[0, ii2])?;
            ii2 = p[1];
            if p[0] == 0 {
                break;
            }
            card10.push((p[0], p[1]));
        }
    }
    if u[5] != 0 {
        return Err(NjoyError::NotPorted("errorr nstan /= 0 (card 11)"));
    }
    let egn = if c2[1].abs() == 1 {
        let ngn = r.read_ints(&[0])?[0].max(0) as usize;
        Some(r.read_reals(&vec![0.0; ngn + 1])?)
    } else {
        None
    };
    // Card 13 (`iwt = 1`) and 13b (`iwt = 4`) are read by `egnwtf` itself
    // (`errorr.f90:9915-9962`), once per call: by `grpav` or `colaps` (`:1028-1032`;
    // `colaps` calls it for `iwt /= 0`, `:9122`), and again by `grpav4` when
    // `mfcov = 34` (`:1038-1040`, `:5467`). ~~One read~~ **CORRECTED 2026-11-13
    // (gh:#1371):** the card is read as often as upstream reads it, so a second copy
    // is not left for the next module's reader. Each call rebuilds `wght`; this port
    // runs one weight for both passes, so two copies that differ are refused by name.
    let reads = if card7[1] == 34 { 2 } else { 1 };
    let mut tab1: Option<Vec<f64>> = None;
    let mut wght: Option<(f64, f64, f64, f64)> = None;
    for _ in 0..reads {
        match c2[2].abs() {
            1 => {
                let t = egnwtf_card13(r)?;
                if tab1.as_ref().is_some_and(|p| *p != t) {
                    return Err(NjoyError::NotPorted("errorr iwt = 1 with mfcov = 34: two different card 13 TAB1s (grpav/colaps and grpav4 would weight differently)"));
                }
                tab1 = Some(t);
            }
            4 => {
                let w = r.read_reals(&[0.0; 4])?;
                let w = (w[0], w[1], w[2], w[3]);
                if wght.is_some_and(|p| p != w) {
                    return Err(NjoyError::NotPorted("errorr iwt = 4 with mfcov = 34: two different cards 13b (grpav/colaps and grpav4 would weight differently)"));
                }
                wght = Some(w);
            }
            _ => {}
        }
    }
    Ok(ErrorrCards {
        units: [u[0], u[1], u[2], u[3], u[4], u[5]],
        card2: [c2[0], c2[1], c2[2], c2[3], c2[4]],
        mprint: s3[0].int(),
        tempin: s3[1].real(),
        card7,
        efmean: s7[5].real(),
        dap: s7[6].real(),
        egn,
        wght,
        tab1,
        card10,
    })
}

/// `egnwtf`'s `iwt = 1` read (`errorr.f90:9916-9932`): 10 000 words preset to zero,
/// then `wght` is `win(1..iww)`, `iww` the last non-zero word.
///
/// # Errors
/// A malformed card; [`NjoyError::NotPorted`] when `iww` stops short of the TAB1's
/// own `6 + 2 nr + 2 np` words (a TAB1 ending in a zero weight): `terpa` would read
/// past `wght`, which is undefined upstream.
fn egnwtf_card13(r: &mut CardReader) -> Result<Vec<f64>, NjoyError> {
    let win = r.read_reals(&vec![0.0; 10000])?;
    let iww = win.iter().rposition(|&x| x != 0.0).map_or(0, |i| i + 1);
    let need = if iww >= 6 { 6 + 2 * (win[4].round() as i64) + 2 * (win[5].round() as i64) } else { i64::MAX };
    if (iww as i64) < need {
        return Err(NjoyError::NotPorted("errorr iwt = 1: card 13 TAB1 shorter than its nr/np once trailing zeros are dropped (egnwtf keeps win up to its last non-zero word)"));
    }
    Ok(win[..iww].to_vec())
}

/// The `iverf` of material `matd`'s ENDF on unit `nendf`, 6 when it cannot be told
/// (parse-only runs).
fn iverf_of(units: &mut TapeUnits, nendf: i32, matd_hint: Option<i32>) -> i32 {
    let Ok(u) = units.get(nendf) else { return 6 };
    let mat = matd_hint.or_else(|| u.tape.materials().first().copied());
    mat.and_then(|m| crate::errorr::mf33::endf_version(&u.tape, m).ok()).unwrap_or(6)
}

/// ERRORR from its cards: read them, then run.
///
/// # Errors
/// As [`read_errorr`], and [`NjoyError::NotPorted`] for anything but `mfcov = 35`
/// from an input GENDF.
pub fn run_errorr(r: &mut CardReader, units: &mut TapeUnits, parse_only: bool) -> Result<(), NjoyError> {
    // card 1 is peeked for the ENDF unit: upstream reads the tape between cards 1 and 2
    let mut probe = r.clone();
    let nendf = probe.read_ints(&[0])?[0];
    if nendf == 999 {
        return run_covadd(r, units, parse_only);
    }
    let iverf = if parse_only {
        6
    } else {
        let _ = units.openz(nendf, 0);
        iverf_of(units, nendf, None)
    };
    let c = read_errorr(r, iverf)?;
    if parse_only {
        return Ok(());
    }
    let [nendf, npend, ngout, nout, nin, _] = c.units;
    let mfcov = c.card7[1];
    if mfcov == 31 || mfcov == 40 || (mfcov == 33 && ngout != 0) {
        return run_errorr_cov_gendf(&c, units, mfcov);
    }
    if c.card7[0] == 2 {
        return Err(NjoyError::NotPorted("errorr iread = 2 other than mfcov = 31/33 from a GENDF"));
    }
    if mfcov == 33 {
        return run_errorr_mf33(&c, units);
    }
    if mfcov == 34 {
        return run_errorr_mf34(&c, units);
    }
    if mfcov != 35 {
        return Err(NjoyError::NotPorted("errorr from a deck: mfcov other than 33, 34 or 35"));
    }
    if ngout == 0 {
        return Err(NjoyError::InvalidInput("errorr: ngout must be nonzero when mfcov = 35".into()));
    }
    let _ = npend;
    if nin != 0 {
        return Err(NjoyError::NotPorted("errorr nin /= 0 (input covariance tape)"));
    }
    units.openz(nendf, 0)?;
    let endf = units.get(nendf)?.tape.clone();
    let gendf = units
        .gendf(ngout)?
        .ok_or_else(|| NjoyError::InvalidInput(format!("errorr: unit {ngout} holds no GENDF")))?;
    let cfg = Mf35Config {
        matd: c.card2[0],
        ign: c.card2[1],
        user_egn: c.egn.clone(),
        irelco: c.card2[4],
        ifissp: c.card7[4],
        efmean: c.efmean,
        tempin: c.tempin,
    };
    let res = run_mf35(&endf, &gendf, &cfg)?;
    if nout != 0 {
        units.openz(nout, 1)?;
        let text = res.tape.text();
        units.put_gendf(nout, text, crate::errorr::gendf::GendfTape { tpid: String::new(), records: res.tape.records.clone() });
    }
    Ok(())
}

/// ERRORR's `nendf = 999` option (`errorr.f90:424-443`): read `nitape notape` and
/// the MT list (ended by 0, at most 5), then copy `nitape` to `notape` with dummy
/// MF=33 sections ([`covadd_text`]).
///
/// # Errors
/// A malformed card; more than five MTs (upstream: "errorr in 999 option"); an
/// input tape [`covadd_text`] cannot read.
fn run_covadd(r: &mut CardReader, units: &mut TapeUnits, parse_only: bool) -> Result<(), NjoyError> {
    let _ = r.read_ints(&[0, 0, 0, 0, 0, 0])?;
    let io = r.read_ints(&[0, 0])?;
    let mut mts = Vec::new();
    loop {
        let ii = r.read_ints(&[0])?[0];
        if ii == 0 {
            break;
        }
        if mts.len() == 5 {
            return Err(NjoyError::InvalidInput("errorr: errorr in 999 option (more than 5 MTs)".into()));
        }
        mts.push(ii);
    }
    if parse_only {
        return Ok(());
    }
    let (nitape, notape) = (io[0], io[1]);
    units.openz(nitape, 0)?;
    let input = units
        .text_or_file(nitape)
        .ok_or_else(|| NjoyError::InvalidInput(format!("errorr covadd: unit {nitape} holds no tape text")))?;
    let text = covadd_text(&input, &mts)?;
    units.openz(notape, 1)?;
    units.put_endf_text(notape, text)
}

/// `covadd` (`errorr.f90:6907-6967`) as the text transform it is: copy the tape
/// line by line (`a66,i4,i2,i3,i5`, so every line is rewritten to 80 columns),
/// add `iadd` to `NXC` on the fifth line, append one dictionary line `33 mt 4 0` per
/// MT after the dictionary, and after MF=32's SEND write a `mat 0 0 99999` line and
/// a dummy MF=33 section per MT (HEAD, subsection CONT, an `LB = 5` LIST of three
/// words `1e-5 2e7 0`, SEND). Fields are written as upstream's formats print them
/// (`2f11.1` zeros as `        0.0`).
///
/// # Errors
/// [`NjoyError::EndfParse`] for a tape shorter than its MF=1 header and dictionary.
pub fn covadd_text(input: &str, mts: &[i32]) -> Result<String, NjoyError> {
    let short = || NjoyError::EndfParse("errorr covadd: tape ends inside MF=1".into());
    let fint = |l: &str, a: usize, b: usize| -> i32 { l.get(a..b.min(l.len())).and_then(|x| x.trim().parse().ok()).unwrap_or(0) };
    let a = |l: &str, n: usize| -> String { format!("{:<n$.n$}", l, n = n) };
    let tail = |l: &str| (fint(l, 66, 70), fint(l, 70, 72), fint(l, 72, 75), fint(l, 75, 80));
    let line = |d: &str, (mat, mf, mt, ii): (i32, i32, i32, i32)| format!("{d}{mat:4}{mf:2}{mt:3}{ii:5}");
    let mut it = input.lines();
    let mut out: Vec<String> = Vec::new();
    let mut czaawr = String::new();
    for i in 1..=4 {
        let l = it.next().ok_or_else(short)?;
        if i == 2 {
            czaawr = a(l, 22);
        }
        out.push(line(&a(l, 66), tail(l)));
    }
    let l = it.next().ok_or_else(short)?;
    let (i1, i2) = (fint(l, 44, 55), fint(l, 55, 66));
    out.push(line(&format!("{}{:11}{:11}", a(l, 44), i1, i2 + mts.len() as i32), tail(l)));
    let mut last = (0, 0, 0, 0);
    for _ in 0..(i1 + i2).max(0) {
        let l = it.next().ok_or_else(short)?;
        last = tail(l);
        out.push(line(&a(l, 66), last));
    }
    let (mat, mf, mt, ii) = last;
    for (k, m) in mts.iter().enumerate() {
        out.push(line(&format!("{:22}{:11}{:11}{:11}{:11}", "", 33, m, 4, 0), (mat, mf, mt, ii + k as i32 + 1)));
    }
    for l in it {
        let t = tail(l);
        out.push(line(&a(l, 66), t));
        if t.1 == 32 && t.2 == 0 {
            let mat = t.0;
            out.push(line(&" ".repeat(66), (mat, 0, 0, 99_999)));
            for &m in mts {
                out.push(line(&format!("{czaawr}{:11}{:11}{:11}{:11}", 0, 0, 0, 1), (mat, 33, m, 1)));
                out.push(line(&format!("{:11.1}{:11.1}{:11}{:11}{:11}{:11}", 0.0, 0.0, 0, m, 0, 1), (mat, 33, m, 2)));
                out.push(line(&format!("{:11.1}{:11.1}{:11}{:11}{:11}{:11}", 0.0, 0.0, 1, 5, 3, 2), (mat, 33, m, 3)));
                out.push(line(&format!("{:<66}", " 1.000000-5 2.000000+7 0.000000+0"), (mat, 33, m, 4)));
                out.push(line(&" ".repeat(66), (mat, 33, 0, 99_999)));
            }
        }
        if t.0 == -1 {
            break;
        }
    }
    let mut s = out.join("\n");
    s.push('\n');
    Ok(s)
}

/// ERRORR `mfcov = 34` ([`run_mf34_gendf`]), its tape on unit `nout`.
///
/// `errorr.f90:1038-1047`: a GENDF is required when MF=4/MT=2 has `LTT = 0/2`, or
/// `LTT = 3` with its Legendre range ending below the top group bound.
///
/// # Errors
/// [`NjoyError::NotPorted`] for `nin /= 0`, or neither a GENDF nor a PENDF;
/// [`NjoyError::InvalidInput`] when a GENDF is required and none is given; and
/// whatever [`run_mf34_gendf`] refuses.
fn run_errorr_mf34(c: &ErrorrCards, units: &mut TapeUnits) -> Result<(), NjoyError> {
    let [nendf, _, ngout, nout, nin, _] = c.units;
    if nin != 0 {
        return Err(NjoyError::NotPorted("errorr nin /= 0 (input covariance tape)"));
    }
    units.openz(nendf, 0)?;
    let endf = units.get(nendf)?.tape.clone();
    let matd = c.card2[0];
    let (ltt4, _, eltt4) = crate::errorr::mf34::mf4_flags(&endf, matd)?;
    let cfg = Mf34Config {
        matd,
        ign: c.card2[1],
        user_egn: c.egn.clone(),
        irelco: c.card2[4],
        legord: c.card7[3],
        ifissp: if c.card7[4] == -1000 { 1 } else { c.card7[4] },
        tempin: c.tempin,
    };
    if ngout == 0 {
        let top = c.egn.as_ref().and_then(|e| e.last().copied());
        let need2 = ltt4 == 0 || ltt4 == 2 || (ltt4 == 3 && top.is_none_or(|t| eltt4.unwrap_or(0.0) + 1.0e-5 < t));
        if need2 {
            return Err(NjoyError::InvalidInput("errorr: ngout input tape is required".into()));
        }
        let npend = c.units[1];
        if npend == 0 {
            return Err(NjoyError::NotPorted("errorr mfcov = 34 with npend = 0 and ngout = 0"));
        }
        units.openz(npend, 0)?;
        let pendf = units.get(npend)?.tape.clone();
        let weight = ErrorrWeight::from_iwt(c.card2[2], c.tab1.clone(), c.wght)?;
        let res = crate::errorr::mf34::run_mf34_pendf(&endf, &pendf, &cfg, &weight)?;
        return put_mf34(units, nout, res);
    }
    let gendf = units
        .gendf(ngout)?
        .ok_or_else(|| NjoyError::InvalidInput(format!("errorr: unit {ngout} holds no GENDF")))?;
    let res = run_mf34_gendf(&endf, &gendf, &cfg)?;
    put_mf34(units, nout, res)
}

/// An `mfcov = 34` tape on unit `nout` (text, and its records for a binary unit).
fn put_mf34(units: &mut TapeUnits, nout: i32, res: crate::errorr::mf34::Mf34Result) -> Result<(), NjoyError> {
    if nout != 0 {
        units.openz(nout, 1)?;
        let records = GendfTape { tpid: String::new(), records: res.tape.records.clone() };
        units.put_gendf(nout, res.tape.text(), records);
    }
    Ok(())
}

/// ERRORR `mfcov = 31` or `40`, or `33` with `ngout /= 0`, from an input GENDF
/// ([`run_cov_gendf`]); with `nin /= 0` the input covariance tape is copied ahead
/// of it on `nout` ([`copy_nin_ahead`]).
///
/// # Errors
/// [`NjoyError::NotPorted`] for `irespr = 0` with `mfcov = 33`, and whatever
/// [`run_cov_gendf`] refuses; [`NjoyError::InvalidInput`] for `ngout = 0`.
fn run_errorr_cov_gendf(c: &ErrorrCards, units: &mut TapeUnits, mfcov: i32) -> Result<(), NjoyError> {
    let [nendf, _, ngout, nout, nin, _] = c.units;
    if ngout == 0 {
        return Err(NjoyError::InvalidInput(format!("errorr: ngout must be nonzero when mfcov = {mfcov}")));
    }
    if mfcov == 33 && c.card7[2] != 1 {
        return Err(NjoyError::NotPorted("errorr irespr = 0 (MF=32 by finite differences)"));
    }
    units.openz(nendf, 0)?;
    let endf = units.get(nendf)?.tape.clone();
    let gendf = units
        .gendf(ngout)?
        .ok_or_else(|| NjoyError::InvalidInput(format!("errorr: unit {ngout} holds no GENDF")))?;
    let cfg = Mf33Config {
        matd: c.card2[0],
        ign: c.card2[1],
        user_egn: c.egn.clone(),
        weight: ErrorrWeight::from_iwt(c.card2[2], c.tab1.clone(), c.wght)?,
        tempin: c.tempin,
        irelco: c.card2[4],
        dap: c.dap,
    };
    let res = if c.card7[0] == 2 {
        run_cov_gendf_card10(&endf, &gendf, &cfg, mfcov, &c.card10)?
    } else {
        run_cov_gendf(&endf, &gendf, &cfg, mfcov)?
    };
    let nsh0 = covout_first_seq(&endf, cfg.matd, nendf, res.resonance.is_some());
    let t = mf33_text_labelled(&res, &gendf.tpid, nsh0)?;
    if nout == 0 {
        return Ok(());
    }
    let (text, records) = if nin != 0 {
        let prior = units.text_or_file(nin).ok_or_else(|| NjoyError::InvalidInput(format!("errorr: unit {nin} holds no covariance tape")))?;
        let mut recs = units.gendf(nin)?.map(|g| g.records).unwrap_or_default();
        recs.retain(|r| r.kind != crate::errorr::gendf::GendfKind::Tend);
        recs.extend(t.records.iter().cloned());
        (copy_nin_ahead(&prior, &t.text())?, recs)
    } else {
        (t.text(), t.records.clone())
    };
    units.openz(nout, 1)?;
    units.put_gendf(nout, text, GendfTape { tpid: String::new(), records });
    Ok(())
}

/// `covout`'s copy of the input covariance tape `nin` ahead of the new material
/// (`errorr.f90:7144-7158`): `nin`'s label and every line up to its last MEND,
/// renumbered `1, 2, ...` as NJOY's writer numbers copied records (SEND, FEND and
/// MEND included); then `ours` without its label, the sequence numbers of its first
/// section continuing from the copy's last.
///
/// # Errors
/// [`NjoyError::EndfParse`] when `nin` has no MEND.
pub fn copy_nin_ahead(nin: &str, ours: &str) -> Result<String, NjoyError> {
    let lines: Vec<&str> = nin.lines().collect();
    let is_mend = |l: &str| l.get(66..70).map(str::trim) == Some("0") && l.get(70..75).is_some_and(|x| x.trim() == "0  0" || x.trim().replace(' ', "") == "00");
    let last = lines.iter().rposition(|l| is_mend(l)).ok_or_else(|| NjoyError::EndfParse("errorr: nin holds no MEND record".into()))?;
    let mut out = Vec::with_capacity(lines.len());
    out.push(lines[0].to_string());
    let mut seq = 0usize;
    for l in &lines[1..=last] {
        seq += 1;
        out.push(format!("{:<75.75}{:5}", l, seq));
    }
    let mut first = true;
    for l in ours.lines().skip(1) {
        let s: usize = l.get(75..80).and_then(|x| x.trim().parse().ok()).unwrap_or(0);
        if first && s == 99_999 {
            first = false;
        }
        if first {
            out.push(format!("{:<75.75}{:5}", l, s + seq));
        } else {
            out.push(l.to_string());
        }
    }
    let mut text = out.join("\n");
    text.push('\n');
    Ok(text)
}

/// ERRORR `mfcov = 33` from a PENDF ([`run_mf33`]), its tape on unit `nout`.
///
/// # Errors
/// [`NjoyError::NotPorted`] for an input GENDF (`ngout /= 0`, `colaps` for MF=33),
/// `nin /= 0`, `irespr = 0`; and whatever [`run_mf33`] refuses.
fn run_errorr_mf33(c: &ErrorrCards, units: &mut TapeUnits) -> Result<(), NjoyError> {
    let [nendf, npend, ngout, nout, nin, _] = c.units;
    let _ = ngout; // ngout /= 0 goes to `run_errorr_cov_gendf`
    if nin != 0 {
        return Err(NjoyError::NotPorted("errorr nin /= 0 (input covariance tape)"));
    }
    if npend == 0 {
        return Err(NjoyError::NotPorted("errorr mfcov = 33 with npend = 0"));
    }
    if c.card7[2] != 1 {
        return Err(NjoyError::NotPorted("errorr irespr = 0 (MF=32 by finite differences)"));
    }
    units.openz(nendf, 0)?;
    units.openz(npend, 0)?;
    let endf = units.get(nendf)?.tape.clone();
    let pendf = units.get(npend)?.tape.clone();
    let cfg = Mf33Config {
        matd: c.card2[0],
        ign: c.card2[1],
        user_egn: c.egn.clone(),
        weight: ErrorrWeight::from_iwt(c.card2[2], c.tab1.clone(), c.wght)?,
        tempin: c.tempin,
        irelco: c.card2[4],
        dap: c.dap,
    };
    let res = run_mf33(&endf, &pendf, &cfg)?;
    if nout != 0 {
        units.openz(nout, 1)?;
        let t = mf33_text_labelled(&res, "", covout_first_seq(&endf, cfg.matd, nendf, res.resonance.is_some()))?;
        let records = GendfTape { tpid: String::new(), records: t.records.clone() };
        units.put_gendf(nout, t.text(), records);
    }
    Ok(())
}

/// The `nout` tape of an MF=33 run as `covout` writes it (`errorr.f90:7812-7838,
/// 7891-7898, 7280-7317, 7413-7431, 7488-7526`): the sections of
/// [`crate::errorr::ErrorrResult::to_tape`] as CONT and LIST records, with
/// SEND, FEND, MEND and TEND.
///
/// # Errors
/// A section whose rows end inside a record.
pub fn mf33_text(res: &crate::errorr::ErrorrResult) -> Result<GendfText, NjoyError> {
    mf33_text_labelled(res, "", 1)
}

/// The sequence number `covout`'s first MF=33 line carries (`nsh`): 1 after
/// `sigc`'s FEND, plus one per line `resprx`'s `rdumrd2` copied to its scratch unit
/// `nscr6` when that unit is coded (`nendf > 0`, `errorr.f90:3062-3066`,
/// `:5091-5225`): MF=2/MT=151's lines, the section end excluded. `nscr6` exists only
/// for `mfcov = 33` with MF=32 on file.
pub fn covout_first_seq(endf: &crate::endf::tape::Tape, matd: i32, nendf: i32, resprx_ran: bool) -> usize {
    if !resprx_ran || nendf <= 0 {
        return 1;
    }
    let n = endf.section(matd, 2, 151).map_or(0, |s| s.rows.len());
    let seq = 1 + n;
    if seq > 99_999 { 1 + (seq - 1) % 99_999 } else { seq }
}

/// [`mf33_text`] with the tape label `tpid` (`covout`'s `tpidio` copies the label
/// of the tape ERRORR's group constants came from: a GENDF's for `ngout /= 0`),
/// and `nsh0` as the first MF=33 line's sequence number ([`covout_first_seq`]).
///
/// # Errors
/// As [`mf33_text`].
pub fn mf33_text_labelled(res: &crate::errorr::ErrorrResult, tpid: &str, nsh0: usize) -> Result<GendfText, NjoyError> {
    let tape = res.to_tape();
    let mut t = GendfText::label(tpid);
    let secs = tape.sections();
    let mut first_cov = true;
    for (k, sec) in secs.iter().enumerate() {
        let key = sec.key;
        if key.mf == 33 && first_cov {
            first_cov = false;
            t.begin_at(key.mat, key.mf, key.mt, nsh0);
        } else {
            t.begin(key.mat, key.mf, key.mt);
        }
        let rows = &sec.rows;
        let mut i = 0;
        match key.mf {
            1 => {
                put_cont(&mut t, rows, &mut i)?;
                put_list(&mut t, rows, &mut i)?;
            }
            3 => {
                put_list(&mut t, rows, &mut i)?;
            }
            _ => {
                put_cont(&mut t, rows, &mut i)?;
                while i < rows.len() {
                    let kgp = put_cont(&mut t, rows, &mut i)?[5];
                    while put_list(&mut t, rows, &mut i)?[5] < kgp {}
                }
            }
        }
        t.send();
        if secs.get(k + 1).is_none_or(|n| n.key.mf != key.mf || n.key.mat != key.mat) {
            t.fend();
        }
    }
    t.end_tape();
    Ok(t)
}

fn short() -> NjoyError {
    NjoyError::EndfParse("errorr: covariance section ends inside a record".into())
}

/// `contio` of row `i`.
fn put_cont(t: &mut GendfText, rows: &[[f64; 6]], i: &mut usize) -> Result<[f64; 6], NjoyError> {
    let r = *rows.get(*i).ok_or_else(short)?;
    t.cont([r[0], r[1]], [r[2] as i32, r[3] as i32, r[4] as i32, r[5] as i32]);
    *i += 1;
    Ok(r)
}

/// `listio` of the LIST whose head is row `i`.
fn put_list(t: &mut GendfText, rows: &[[f64; 6]], i: &mut usize) -> Result<[f64; 6], NjoyError> {
    let h = *rows.get(*i).ok_or_else(short)?;
    let n = h[4].max(0.0) as usize;
    let nrow = n.div_ceil(6);
    let body = rows.get(*i + 1..*i + 1 + nrow).ok_or_else(short)?;
    let data: Vec<f64> = body.iter().flatten().copied().take(n).collect();
    t.list([h[0], h[1]], h[2] as i32, h[3] as i32, h[5] as i32, &data);
    *i += 1 + nrow;
    Ok(h)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deck 34's ERRORR cards, and deck 18's card 12.
    #[test]
    fn reads_the_mf35_cards() {
        let mut r = CardReader::new(" -30 0 -31 -21 0 0\n 9440 3 2 1 1 /\n 0 293.6\n 0 35 1 1 1 /\n");
        let c = read_errorr(&mut r, 6).unwrap();
        assert_eq!(c.units, [-30, 0, -31, -21, 0, 0]);
        assert_eq!(c.card7, [0, 35, 1, 1, 1]);
        assert_eq!((c.tempin, c.efmean), (293.6, 2.0e6));
        let mut r = CardReader::new(" 20 0 91 28 0 0 /\n 9999 1 2 1 1 /\n 1 300./\n 0 35 1 1 -1 1.23e6 /\n 2 /\n 1e-5 1e5 2e7 /\n");
        let c = read_errorr(&mut r, 6).unwrap();
        assert_eq!(c.egn, Some(vec![1e-5, 1e5, 2e7]));
        assert_eq!(c.efmean, 1.23e6);
        assert!(read_errorr(&mut CardReader::new(" 999 /\n"), 6).is_err());
    }

    /// Card 13 (`iwt = 1`, gh:#1371): kept to its last non-zero word; read twice for
    /// `mfcov = 34` (two `egnwtf` calls), and refused when the copies differ or a
    /// trailing zero would cut the TAB1 short.
    #[test]
    fn reads_card13_once_per_egnwtf_call() {
        let tab = " 0 0 0 0 1 2 2 2 1e-5 1. 2e7 3./\n";
        let c = read_errorr(&mut CardReader::new(&format!(" -21 -22 0 33/\n 1306 -1 1 1/\n 0 0/\n 0 33/\n 1/\n 1e-5 2e7/\n{tab}")), 6).unwrap();
        assert_eq!(c.tab1, Some(vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 1e-5, 1.0, 2e7, 3.0]));
        let mut r = CardReader::new(&format!(" -21 0 -22 34/\n 2631 3 1 0 1/\n 0 293.6/\n 0 34/\n{tab}{tab}next/\n"));
        assert!(read_errorr(&mut r, 6).unwrap().tab1.is_some());
        assert_eq!(r.read_string(4).unwrap(), "next");
        let other = " 0 0 0 0 1 2 2 2 1e-5 1. 2e7 4./\n";
        assert!(matches!(read_errorr(&mut CardReader::new(&format!(" -21 0 -22 34/\n 2631 3 1 0 1/\n 0 293.6/\n 0 34/\n{tab}{other}")), 6), Err(NjoyError::NotPorted(_))));
        let zero_end = " 0 0 0 0 1 2 2 2 1e-5 1. 2e7 0./\n";
        assert!(matches!(read_errorr(&mut CardReader::new(&format!(" -21 -22 0 33/\n 1306 3 1 1/\n 0 0/\n 0 33/\n{zero_end}")), 6), Err(NjoyError::NotPorted(_))));
    }
}
