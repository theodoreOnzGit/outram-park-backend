// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/powr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - the module variables of `powm` (l.8-26) that `lib = 1` and `lib = 2` use.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! POWR's read state: `powm`'s module variables, the `scr(5000)` work array
//! and the GENDF read through `endf.f90`'s record routines.
//!
//! The GENDF is read card by card ([`EndfStream::from_text`]), because
//! `gamxs` positions it with `skiprz(ngendf,-2)` and `findf`, which count
//! cards. Arrays keep Fortran's 1-based indices: `scr(i)` is [`Reader::sc`]`(i)`.

use crate::plotr::stream::{EndfStream, Head, NPAGE};
use crate::NjoyError;

/// `nwscr` (`powr.f90:16`).
pub const NWSCR: usize = 5000;

/// `powm`'s variables and the GENDF being read.
#[derive(Debug, Clone)]
pub struct Reader {
    /// The GENDF, card by card.
    pub s: EndfStream,
    /// `endf.f90`'s `math`, `mfh`, `mth`, `c1h` ... `n2h`.
    pub h: Head,
    /// `scr(5000)`, with room past `nwscr` for one more page (upstream checks
    /// the bound after some reads and not others; see [`Reader::listio`]).
    pub scr: Vec<f64>,
    /// `lib`.
    pub lib: i32,
    /// `ngnf = 185`.
    pub ngnf: i32,
    /// `ngn`.
    pub ngn: i32,
    /// `ngnd`.
    pub ngnd: i32,
    /// `cflux(140)`, 1-based (`cflux[0]` unused).
    pub cflux: [f64; 141],
    /// `tmpr(10)`, 1-based.
    pub tmpr: [f64; 11],
    /// `sigz(10)`, 1-based.
    pub sigz: [f64; 11],
    /// `ntp`.
    pub ntp: i32,
    /// `nsigz`.
    pub nsigz: i32,
    /// `jwf`.
    pub jwf: i32,
    /// `iwr`.
    pub iwr: i32,
    /// `izref`.
    pub izref: i32,
    /// `rtemp`.
    pub rtemp: f64,
    /// `matd`.
    pub matd: i32,
    /// `nid`.
    pub nid: i32,
    /// `nff`.
    pub nff: i32,
    /// `iwa`.
    pub iwa: i32,
    /// `iwf`.
    pub iwf: i32,
    /// `nfs`.
    pub nfs: i32,
    /// `xla(3)`, 1-based.
    pub xla: [f64; 4],
    /// `xld(3)`, 1-based.
    pub xld: [f64; 4],
    /// `xlol(3)`, 1-based.
    pub xlol: [f64; 4],
    /// The GENDF's first line (its label).
    pub label: String,
}

impl Reader {
    /// The state at the start of `powr` (module variables zero, as gfortran
    /// leaves static storage; `ngnf = 185`), reading the coded GENDF `gendf`.
    ///
    /// # Errors
    /// A GENDF card that does not parse.
    pub fn new(gendf: &str, lib: i32) -> Result<Self, NjoyError> {
        Ok(Self {
            s: EndfStream::from_text(gendf)?,
            h: Head::default(),
            scr: vec![0.0; NWSCR + NPAGE + 16],
            lib,
            ngnf: 185,
            ngn: 0,
            ngnd: 0,
            cflux: [0.0; 141],
            tmpr: [0.0; 11],
            sigz: [0.0; 11],
            ntp: 0,
            nsigz: 0,
            jwf: 0,
            iwr: 0,
            izref: 0,
            rtemp: 0.0,
            matd: 0,
            nid: 0,
            nff: 0,
            iwa: 0,
            iwf: 0,
            nfs: 0,
            xla: [0.0; 4],
            xld: [0.0; 4],
            xlol: [0.0; 4],
            label: gendf.lines().next().unwrap_or("").to_string(),
        })
    }

    /// `true` when the label's first 66 columns read as `6e11.0`, as `therm`
    /// reads them with `contio` (gfortran stops with "Bad value during
    /// floating point read" otherwise).
    pub fn label_reads_as_cont(&self) -> bool {
        let t = crate::acer::fortran_fmt::fixed(&self.label, 66);
        (0..6).all(|k| e_field_reads(&t[11 * k..11 * k + 11]))
    }

    /// `scr(i)`.
    pub fn sc(&self, i: usize) -> f64 {
        self.scr[i - 1]
    }

    /// `icgrp(ig)` with this run's `ngn`, `ngnd` and `lib`.
    pub fn icgrp(&self, ig: i32) -> i32 {
        super::icgrp(ig, self.ngn, self.ngnd, self.lib)
    }

    fn room(&self, at: usize, n: usize) -> Result<(), NjoyError> {
        if at == 0 || at - 1 + n > self.scr.len() {
            return Err(NjoyError::NotPorted(
                "POWR: a GENDF record past the end of scr(5000) (out of bounds upstream)",
            ));
        }
        Ok(())
    }

    /// `tpidio(ngendf,0,0,scr,nb,nw)`.
    pub fn tpidio(&mut self) -> Result<(), NjoyError> {
        self.s.tpidio(&mut self.h)
    }

    /// `contio(ngendf,0,0,scr(at),nb,nw)`.
    pub fn contio(&mut self, at: usize) -> Result<(), NjoyError> {
        self.room(at, 6)?;
        self.s.contio(&mut self.h, &mut self.scr, at)
    }

    /// `listio(ngendf,0,0,scr(at),nb,nw)`: returns `(nb, nw)`.
    ///
    /// # Errors
    /// A page that would land past the end of `scr`, which upstream writes
    /// out of bounds; it is refused here.
    pub fn listio(&mut self, at: usize) -> Result<(usize, usize), NjoyError> {
        self.room(at, 6 + NPAGE)?;
        self.s.listio(&mut self.h, &mut self.scr, at)
    }

    /// `moreio(ngendf,0,0,scr(at),nb,nw)`: returns `(nb, nw)`.
    pub fn moreio(&mut self, at: usize, nb: usize) -> Result<(usize, usize), NjoyError> {
        self.room(at, nb.min(NPAGE))?;
        self.s.moreio(&mut self.scr, at, nb)
    }

    /// `tosend(ngendf,0,0,scr)`.
    pub fn tosend(&mut self) -> Result<(), NjoyError> {
        self.s.tosend()
    }

    /// `tomend(ngendf,0,0,scr)`.
    pub fn tomend(&mut self) -> Result<(), NjoyError> {
        self.s.tomend()
    }

    /// `repoz(ngendf)`.
    pub fn repoz(&mut self) {
        self.s.rewind();
    }
}

/// Whether gfortran's `e11.0` input (blanks ignored) accepts `field`: blank,
/// or `[+-]` digits with at most one point and at least one digit, then an
/// optional exponent (`E`, `D` or `Q` with an optional sign, or a bare sign,
/// then digits). The crate's ENDF parser reads text as 0 and cannot tell.
pub fn e_field_reads(field: &str) -> bool {
    let b: Vec<u8> = field.bytes().filter(|&c| c != b' ').collect();
    if b.is_empty() {
        return true;
    }
    let mut k = 0;
    if b[k] == b'+' || b[k] == b'-' {
        k += 1;
    }
    let (mut digits, mut point) = (0, false);
    while k < b.len() && (b[k].is_ascii_digit() || (b[k] == b'.' && !point)) {
        if b[k] == b'.' {
            point = true;
        } else {
            digits += 1;
        }
        k += 1;
    }
    if digits == 0 {
        return false;
    }
    if k == b.len() {
        return true;
    }
    if matches!(b[k], b'E' | b'e' | b'D' | b'd' | b'Q' | b'q') {
        k += 1;
        if k < b.len() && (b[k] == b'+' || b[k] == b'-') {
            k += 1;
        }
    } else if b[k] == b'+' || b[k] == b'-' {
        k += 1;
    } else {
        return false;
    }
    k < b.len() && b[k..].iter().all(u8::is_ascii_digit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e_fields_read_as_gfortran_e11_0() {
        for ok in ["", "   ", "1", "-1.5", "1.0+5", "2.5e-3", ".5D+2", "1 .0- 5"] {
            assert!(e_field_reads(ok), "{ok}");
        }
        for bad in ["94-pu-238", "pu", ".", "1.0e", "1-", "1.2.3"] {
            assert!(!e_field_reads(bad), "{bad}");
        }
        let r = Reader::new("94-pu-238\n", 2).unwrap();
        assert!(!r.label_reads_as_cont());
        let r = Reader::new(" \n", 2).unwrap();
        assert!(r.label_reads_as_cont());
        assert_eq!(r.sc(1), 0.0);
    }
}
