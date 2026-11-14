// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine grist`, l.1559-1679 (`grist`);
//   - `subroutine stand`, l.2939-3009 (`stand`);
//   - the window tests of `covcal` for `ltyi > 0`, l.2101-2118 (`StandardWindow`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **ERRORR's ratio-to-standard tape, `nstan`** (gh:#1371).
//!
//! An NC-type sub-subsection with `LTY` 1-3 says that the reaction was measured
//! relative to a standard reaction `(MATS, MTS)`, whose own covariance must be
//! added. ERRORR reads that covariance from a second ENDF tape, `nstan`:
//! - [`grist`] (from `gridd`) merges the standard's energies into the union grid,
//!   inside the window `(E1, E2)` for `LTY = 1` and everywhere for `LTY = 2`
//!   (`gridd` does not call it for `LTY = 3`);
//! - [`stand`] (from `covcal`) appends the standard's NI-type records to the
//!   reaction's own, each with the window `(E1, E2)` and its `LTY`, which
//!   [`StandardWindow::admits`] applies.
//!
//! Card 11 can redefine a standard, `(matb, mtb) -> (matc, mtc)`
//! ([`Standards::redefine`]). A **negative** `matb(1)` (a third reaction
//! correlated through the standard: `covcal`'s `kmtb`, `ety1`/`ety2` and
//! `grist`'s further subsections) is refused by the deck reader and is not
//! translated here.

use std::sync::Arc;

use crate::endf::records::{Cont, SectionCursor};
use crate::endf::tape::Tape;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

use super::covcal::NiRecord;
use super::gridd::{merge, NDIG};

/// The `nstan` tape and card 11's redefinitions.
#[derive(Debug, Clone)]
pub struct Standards {
    /// The ENDF tape on unit `nstan`.
    pub tape: Arc<Tape>,
    /// Card 11: `(matb, mtb, matc, mtc)`, at most `nasmax = 5`, every `matb > 0`.
    pub redefine: Vec<[i32; 4]>,
}

impl Standards {
    /// The standard `(matstd, mtstd)` after card 11: the first entry whose
    /// `(matb, mtb)` matches is replaced by its `(matc, mtc)` (`:1580-1594`,
    /// `:2957-2966`).
    pub fn resolve(&self, matstd: i32, mtstd: i32) -> (i32, i32) {
        self.redefine
            .iter()
            .find(|r| r[0] == matstd && r[1] == mtstd)
            .map_or((matstd, mtstd), |r| (r[2], r[3]))
    }

    /// `findf(matstd, mfcov, mtstd, nstan)` and the section HEAD.
    fn section(&self, mfcov: i32, matstd: i32, mtstd: i32) -> Result<SectionCursor<'_>, NjoyError> {
        let sec = self.tape.section(matstd, mfcov, mtstd).ok_or_else(|| {
            NjoyError::InvalidInput(format!("errorr: standard mat {matstd} mf {mfcov} mt {mtstd} not on the nstan tape"))
        })?;
        Ok(SectionCursor::new(&sec.rows))
    }
}

/// `grist(matstd, mtstd, el, eh)` (`errorr.f90:1559-1679`) with card 11's
/// `matb(1) >= 0`: merge the energies of the standard's first subsection into
/// `eni`, inside `(el, eh)` (`merge`'s window; `(0, 0)` keeps all).
///
/// Upstream stops after the first subsection that has NI-type records, which
/// with `nas = 0` or `matb(1) >= 0` is the first one (`:1672-1675`); its NC-type
/// records are read and skipped (`il = 1`, `:1612-1628`).
///
/// # Errors
/// [`NjoyError::EndfParse`] for `NL <= 0` ("standards tape bad"), `NI = 0` in
/// the first subsection, `LB = 0`, or a malformed record;
/// [`NjoyError::InvalidInput`] for a standard not on the tape.
pub fn grist(std: &Standards, mfcov: i32, matstd: i32, mtstd: i32, el: f64, eh: f64, eni: &mut Vec<f64>) -> Result<(), NjoyError> {
    let (matstd, mtstd) = std.resolve(matstd, mtstd);
    let mut cur = std.section(mfcov, matstd, mtstd)?;
    let head = cur.read_cont()?;
    if head.n2 <= 0 {
        return Err(NjoyError::EndfParse("errorr grist: standards tape bad.".into()));
    }
    let sub = cur.read_cont()?;
    let (nc, ni) = (sub.n1, sub.n2);
    for _ in 0..nc {
        let _lty: Cont = cur.read_cont()?;
        let _ = cur.read_list()?;
    }
    if ni == 0 {
        return Err(NjoyError::EndfParse(format!("errorr grist: illegal ni=0 in the standard (matstd={matstd}, mtstd={mtstd})")));
    }
    for _ in 0..ni {
        let list = cur.read_list()?;
        let lb = list.head.l2;
        let nx = list.head.n2.max(0) as usize;
        if lb == 0 {
            return Err(NjoyError::EndfParse("errorr grist: illegal lb=0.".into()));
        }
        if lb >= 5 && lb != 8 {
            merge(&list.data[..nx], eni, el, eh)?;
            if lb != 5 {
                let nec = ((list.head.n1 - 1) / list.head.n2) as usize;
                merge(&list.data[nx..nx + nec], eni, el, eh)?;
            }
        } else {
            let x: Vec<f64> = (0..nx).map(|i| list.data[2 * i]).collect();
            let nl = list.head.l1.max(0) as usize;
            let nx1 = nx - nl;
            merge(&x[..nx1], eni, el, eh)?;
            merge(&x[nx1..], eni, el, eh)?;
        }
    }
    Ok(())
}

/// The energy window and `LTY` that `stand` stores in a standard's record
/// (`scr(loci)`, `scr(loci+1)`, `scr(loci+4)`, `:2992-2995`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StandardWindow {
    /// `elr = sigfig(E1, 6)` of the NC-type LIST.
    pub elr: f64,
    /// `ehr = sigfig(E2, 6)`.
    pub ehr: f64,
    /// `LTY`, 1-3.
    pub lty: i32,
}

impl StandardWindow {
    /// Upstream's check on a staged window (`:2102-2104`), made on the first
    /// group pair, before any window test.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] ("data in scr(loci) is illegal").
    pub fn validate(&self) -> Result<(), NjoyError> {
        if !(1..=3).contains(&self.lty) || self.elr <= 0.0 || self.ehr <= self.elr {
            return Err(NjoyError::EndfParse("errorr covcal: data in scr(loci) is illegal.".into()));
        }
        Ok(())
    }

    /// Whether the record contributes to union groups with lower bounds `eg`
    /// (row) and `ehr` (column) (`:2105-2112`): `LTY = 1` windows both, `2`
    /// the row only, `3` the column only.
    pub fn admits(&self, eg: f64, ehr: f64) -> bool {
        let inside = |e: f64| e >= self.elr && e < self.ehr;
        match self.lty {
            3 => inside(ehr),
            2 => inside(eg),
            _ => inside(eg) && inside(ehr),
        }
    }
}

/// `stand` (`errorr.f90:2939-3009`): the NC-type LIST head `nc_head`
/// (`E1, E2, MATS, MTS`) of a sub-subsection with `lty` 1-3 names the standard;
/// return its first subsection's NI-type records, each with its window.
///
/// # Errors
/// [`NjoyError::InvalidInput`] for a standard not on the tape; parse errors.
pub fn stand(std: &Standards, mfcov: i32, nc_head: &Cont, lty: i32) -> Result<Vec<(NiRecord, StandardWindow)>, NjoyError> {
    let w = StandardWindow { elr: sigfig(nc_head.c1, NDIG, 0), ehr: sigfig(nc_head.c2, NDIG, 0), lty };
    let (matstd, mtstd) = std.resolve(nc_head.l1, nc_head.l2);
    let mut cur = std.section(mfcov, matstd, mtstd)?;
    let _head = cur.read_cont()?;
    let sub = cur.read_cont()?;
    for _ in 0..sub.n1 {
        let _ = cur.read_cont()?;
        let _ = cur.read_list()?;
    }
    let mut out = Vec::with_capacity(sub.n2.max(0) as usize);
    for _ in 0..sub.n2 {
        let l = cur.read_list()?;
        out.push((NiRecord::from_list(l.head.l1, l.head.l2, l.head.n1, l.head.n2, &l.data), w));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_follow_lty() {
        let w = |lty| StandardWindow { elr: 1.0, ehr: 10.0, lty };
        assert!(w(1).admits(1.0, 5.0) && !w(1).admits(1.0, 10.0) && !w(1).admits(0.5, 5.0));
        assert!(w(2).admits(5.0, 50.0) && !w(2).admits(50.0, 5.0));
        assert!(w(3).admits(50.0, 5.0) && !w(3).admits(5.0, 50.0));
        assert!(w(1).validate().is_ok());
        assert!(StandardWindow { elr: 0.0, ehr: 1.0, lty: 1 }.validate().is_err());
    }

    #[test]
    fn card11_redefines_the_first_match() {
        let s = Standards { tape: Arc::new(Tape::from_sections(String::new(), Vec::new())), redefine: vec![[9228, 18, 9228, 102], [9228, 18, 1, 1]] };
        assert_eq!(s.resolve(9228, 18), (9228, 102));
        assert_eq!(s.resolve(9237, 18), (9237, 18));
    }
}
