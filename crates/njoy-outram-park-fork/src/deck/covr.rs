// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/covr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - the input section of `subroutine covr`, l.168-260.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **COVR from a deck** (gh:#1369): its cards, and the plot option
//! ([`crate::covr::plot::run_plot`]) on the ERRORR tape a previous module left.
//!
//! The ERRORR tape is read as NJOY would read it: a negative (blocked-binary)
//! unit gives ERRORR's records at full precision ([`TapeUnits::binary_gendf`]),
//! a coded unit its text. The library option (`nout > 0`) is read and then
//! refused by name: [`crate::covr::run_library`] exists but no deck here runs it.

use crate::covr::input::{
    default_tlev, ColorStyle, CovarianceForm, CovrInput, CovrMode, LegendOption, LibraryOptions, MatrixOutputType,
    PlotOptions, ReactionSelector, NCASE_MAX,
};
use crate::deck::cards::{CardReader, Slot};
use crate::deck::tapes::TapeUnits;
use crate::endf::tape::{Section, Tape};
use crate::errorr::gendf::{GendfKind, GendfTape};
use crate::endf::EndfKey;
use crate::NjoyError;

fn bad(msg: &str) -> NjoyError {
    NjoyError::InvalidInput(format!("covr: {msg}"))
}

/// Read COVR's cards (`covr.f90:168-260`).
///
/// # Errors
/// A malformed card; `nlev > 9`; a `tlev` that does not increase; too many cases;
/// an `icolor`, `irelco` or `noleg` this crate's card model has no value for.
pub fn read_covr(r: &mut CardReader) -> Result<CovrInput, NjoyError> {
    let c1 = r.read_ints(&[0, 0, 0])?;
    let (nin, nout, nplot) = (c1[0], c1[1], c1[2]);
    let (mode, ncase) = if nout <= 0 {
        let icolor = r.read_ints(&[0])?[0];
        let color = ColorStyle::from_icolor(icolor).ok_or_else(|| bad("icolor other than 0, 1 or 2"))?;
        let mut tlev = default_tlev();
        if icolor == 2 {
            let mut s = vec![Slot::Int(6)];
            let defaults = [0.001, 0.1, 0.2, 0.3, 0.6, 1.0, 0.0, 0.0, 0.0];
            s.extend(defaults.iter().map(|&v| Slot::Real(v)));
            r.read(&mut s)?;
            let nlev = s[0].int();
            if nlev > 9 {
                return Err(bad(" nlev> 9 not allowed"));
            }
            tlev = s[1..=nlev.max(1) as usize].iter().map(Slot::real).collect();
            if tlev.windows(2).any(|w| w[1] <= w[0]) {
                return Err(bad("tlev array must sequentially increase"));
            }
            if let Some(last) = tlev.last_mut() {
                // `mess`: reset tlev(nlev) to 1.0
                *last = 1.0;
            }
        }
        let epmin = r.read_reals(&[0.0])?[0];
        let c3 = r.read_ints(&[1, 1, 0, 1, 1])?;
        if c3[0] != 0 && c3[0] != 1 {
            return Err(bad("irelco other than 0 or 1"));
        }
        let legend = LegendOption::from_noleg(c3[2]).ok_or_else(|| bad("noleg other than -1, 0 or 1"))?;
        let opts = PlotOptions {
            color,
            tlev,
            epmin,
            covariance_form: CovarianceForm::from_irelco(c3[0]),
            legend,
            nstart: c3[3],
            ndiv: if c3[4] == 0 { 1 } else { c3[4] },
        };
        (CovrMode::Plot(opts), c3[1])
    } else {
        let c2 = r.read_ints(&[3, 1])?;
        let lib_id = r.read_string(6)?;
        let description = r.read_string(21)?;
        let opts = LibraryOptions { matrix_type: MatrixOutputType::from_matype(c2[0]), lib_id, description };
        (CovrMode::Library(opts), if c2[1] <= 0 { 1 } else { c2[1] })
    };
    if ncase.max(0) as usize > NCASE_MAX {
        return Err(bad("requested too many cases."));
    }
    let mut cases = Vec::new();
    for _ in 0..ncase.max(0) {
        let c = r.read_ints(&[0, 0, 0, 0])?;
        cases.push(ReactionSelector { mat: c[0], mt: c[1], mat1: c[2], mt1: c[3] });
    }
    Ok(CovrInput { nin, nout, nplot, mode, cases })
}

/// An ERRORR tape kept as GENDF records (a binary unit), as rows of six words in
/// sections: what `contio`/`listio` read from NJOY's blocked-binary tape, at the
/// precision ERRORR computed.
pub fn tape_from_records(g: &GendfTape) -> Tape {
    let mut sections: Vec<Section> = Vec::new();
    let mut open: Option<Section> = None;
    for rec in &g.records {
        let key = EndfKey { mat: rec.mat, mf: rec.mf, mt: rec.mt };
        match rec.kind {
            GendfKind::Cont | GendfKind::List => {
                if open.as_ref().is_some_and(|s| s.key != key) {
                    sections.extend(open.take());
                }
                let sec = open.get_or_insert_with(|| Section { key, rows: Vec::new() });
                let n = rec.n.map(f64::from);
                sec.rows.push([rec.c[0], rec.c[1], n[0], n[1], n[2], n[3]]);
                for ch in rec.data.chunks(6) {
                    let mut row = [0.0; 6];
                    row[..ch.len()].copy_from_slice(ch);
                    sec.rows.push(row);
                }
            }
            _ => sections.extend(open.take()),
        }
    }
    sections.extend(open);
    Tape::from_sections(g.tpid.clone(), sections)
}

/// COVR from its cards: read them, then run the plot option.
///
/// # Errors
/// As [`read_covr`]; an empty or illegal input unit; whatever
/// [`crate::covr::plot::run_plot`] reports; [`NjoyError::NotPorted`] for the
/// library option.
pub fn run_covr(r: &mut CardReader, units: &mut TapeUnits, parse_only: bool) -> Result<(), NjoyError> {
    let c = read_covr(r)?;
    if parse_only {
        return Ok(());
    }
    let CovrMode::Plot(opts) = &c.mode else {
        return Err(NjoyError::NotPorted("covr library option (nout > 0) from a deck"));
    };
    let tape = match units.binary_gendf(c.nin) {
        Some(g) => tape_from_records(&g),
        None => {
            let text = units.text_or_file(c.nin).ok_or_else(|| bad(&format!("unit {} holds no errorr tape", c.nin)))?;
            Tape::read(text.as_bytes())?
        }
    };
    let plot = crate::covr::plot::run_plot(&tape, opts, &c.cases)?;
    if c.nplot != 0 {
        units.put_text(c.nplot, plot.text);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errorr::gendf::GendfRecord;

    /// Deck 34's and deck 18's COVR cards, and an `icolor = 2` card 2'.
    #[test]
    fn reads_the_plot_cards() {
        let mut r = CardReader::new(" -21 0 33 /\n  1/\n  /\n  /\n 9440 /\n");
        let c = read_covr(&mut r).unwrap();
        assert_eq!((c.nin, c.nout, c.nplot), (-21, 0, 33));
        assert_eq!(c.cases, vec![ReactionSelector { mat: 9440, mt: 0, mat1: 0, mt1: 0 }]);
        let CovrMode::Plot(p) = &c.mode else { panic!() };
        assert_eq!((p.color, p.epmin, p.ndiv, p.nstart), (ColorStyle::Color, 0.0, 1, 1));
        let mut r = CardReader::new(" 28 0 38 /\n 1 /\n 1.e3 /\n /\n 9999 /\n");
        let CovrMode::Plot(p) = read_covr(&mut r).unwrap().mode else { panic!() };
        assert_eq!(p.epmin, 1.0e3);
        let mut r = CardReader::new(" 28 0 38 /\n 2 /\n 3 0.1 0.5 0.9 /\n /\n 0 2 /\n 9999 /\n 9999 18 /\n");
        let c = read_covr(&mut r).unwrap();
        let CovrMode::Plot(p) = &c.mode else { panic!() };
        assert_eq!(p.tlev, vec![0.1, 0.5, 1.0]);
        assert_eq!(p.covariance_form, CovarianceForm::Absolute);
        assert_eq!(c.cases.len(), 2);
        let mut r = CardReader::new(" 28 0 38 /\n 2 /\n 2 0.5 0.1 /\n");
        assert!(read_covr(&mut r).is_err());
    }

    /// The library option's cards are read, then refused by name.
    #[test]
    fn library_option_is_refused_by_name() {
        let mut units = TapeUnits::new(None);
        let mut r = CardReader::new(" 21 22 /\n 4 1 /\n 'lib' /\n 'test' /\n 9228 /\n");
        assert!(matches!(run_covr(&mut r, &mut units, false), Err(NjoyError::NotPorted(_))));
        assert_eq!(r.records_left(), 0);
    }

    /// GENDF records become rows: a LIST head, its words six to a row, sections
    /// split at the markers.
    #[test]
    fn records_become_rows() {
        let list = GendfRecord {
            kind: GendfKind::List,
            mat: 9440,
            mf: 5,
            mt: 18,
            c: [0.0, 2.0e6],
            n: [0, 0, 7, 0],
            data: (1..=7).map(f64::from).collect(),
        };
        let g = GendfTape {
            tpid: "t".into(),
            records: vec![list, GendfRecord::marker(GendfKind::Send, 9440, 5)],
        };
        let t = tape_from_records(&g);
        let s = t.section(9440, 5, 18).unwrap();
        assert_eq!(s.rows.len(), 3);
        assert_eq!(s.rows[0], [0.0, 2.0e6, 0.0, 0.0, 7.0, 0.0]);
        assert_eq!(s.rows[2], [7.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    }
}
