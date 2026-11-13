// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - the input section of `subroutine errorr`, l.420-640 and l.861-938;
//   - `gengpn`'s card 12 (`groupr.f90:4157-4161`), `egnwtf`'s card 13b.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **ERRORR from a deck** (gh:#1369): its cards, and the `mfcov = 35` run on an
//! input GENDF ([`crate::errorr::mf35`]).
//!
//! The cards are read in upstream's order: card 1 (`nendf npend ngout nout nin
//! nstan`), card 2 (`matd ign iwt iprint irelco`), card 3 (`mprint tempin`), card 7
//! (`iread mfcov irespr legord ifissp efmean dap`, ENDF-5/6 only), card 12 for
//! `ign = +-1` (`egngpn`), card 13b for `iwt = 4` (`egnwtf`). `iverf` is taken from
//! the ENDF unit before card 7, as upstream reads the tape between cards 1 and 2.
//!
//! What runs: `mfcov = 35` with `ngout /= 0`, and (since 2026-11-13) `mfcov = 33`
//! from a PENDF with `ngout = 0` ([`crate::errorr::run_mf33`], its tape by
//! [`mf33_text`]). Every other combination is read and
//! then refused by name, so a deck stops here with its cards consumed.

use crate::deck::cards::{CardReader, Slot};
use crate::deck::tapes::TapeUnits;
use crate::deck::groupr::GendfText;
use crate::errorr::gendf::GendfTape;
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
}

/// Read ERRORR's cards (`iverf > 4`), up to the point a refusal would come.
///
/// # Errors
/// A malformed card; [`NjoyError::NotPorted`] for the `nendf = 999` (`covadd`)
/// option, ENDF/B-IV cards, `iread = 1/2`, `nstan /= 0` and `iwt = 1`, whose
/// further cards are not read here.
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
    if card7[0] != 0 {
        return Err(NjoyError::NotPorted("errorr iread = 1/2 (cards 8-10)"));
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
    let wght = match c2[2].abs() {
        1 => return Err(NjoyError::NotPorted("errorr iwt = 1 (card 13 TAB1)")),
        4 => {
            let w = r.read_reals(&[0.0; 4])?;
            Some((w[0], w[1], w[2], w[3]))
        }
        _ => None,
    };
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
    })
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
    let iverf = if parse_only || nendf == 999 {
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
    if mfcov == 33 {
        return run_errorr_mf33(&c, units);
    }
    if mfcov != 35 {
        return Err(NjoyError::NotPorted("errorr from a deck: mfcov other than 33 or 35"));
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

/// ERRORR `mfcov = 33` from a PENDF ([`run_mf33`]), its tape on unit `nout`.
///
/// # Errors
/// [`NjoyError::NotPorted`] for an input GENDF (`ngout /= 0`, `colaps` for MF=33),
/// `nin /= 0`, `irespr = 0`; and whatever [`run_mf33`] refuses.
fn run_errorr_mf33(c: &ErrorrCards, units: &mut TapeUnits) -> Result<(), NjoyError> {
    let [nendf, npend, ngout, nout, nin, _] = c.units;
    if ngout != 0 {
        return Err(NjoyError::NotPorted("errorr mfcov = 33 from an input GENDF (ngout /= 0)"));
    }
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
        weight: ErrorrWeight::from_iwt(c.card2[2], None, c.wght)?,
        tempin: c.tempin,
        irelco: c.card2[4],
        dap: c.dap,
    };
    let res = run_mf33(&endf, &pendf, &cfg)?;
    if nout != 0 {
        units.openz(nout, 1)?;
        let t = mf33_text(&res)?;
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
    let tape = res.to_tape();
    let mut t = GendfText::label("");
    let secs = tape.sections();
    for (k, sec) in secs.iter().enumerate() {
        let key = sec.key;
        t.begin(key.mat, key.mf, key.mt);
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
}
