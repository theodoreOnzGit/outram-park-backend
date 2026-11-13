// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/ccccr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - module `ccccm`'s equivalenced container and module variables, l.1-71;
//   - `stow`, l.3509-3524;
// and from `src/endf.f90` (same commit): the reading side of `contio`, `listio`,
// `moreio`, `tosend`, `tofend`, `tomend`, `atend`, as CCCCR uses them.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The machinery CCCCR's routines share (gh:#1369).
//!
//! - [`Core`]: module `ccccm`'s `equivalence(a(1),ia(1),ha(1),ta(1))`, 100 000
//!   four-byte words. `a(i)` is the word as an `f32`, `ia(i)` as an `i32`, and
//!   `ha(i)`/`ta(i)` the 8 bytes of words `2i-1` and `2i` (`mult = 2`).
//!   Indices are upstream's, 1-based. An index outside the container is
//!   recorded, and [`Core::check`] turns it into an error after the routine,
//!   where gfortran would have read or written outside the array.
//! - [`Scratch`]: an unformatted scratch tape of `a`-words (`nscrt2`, `nscrt3`).
//! - [`Cursor`]: a GENDF tape read and written record by record (`nin`,
//!   `nscrt1`), with the ENDF header variables (`math`, `mfh`, `mth`, `c1h`,
//!   `l1h`, `l2h`, `n1h`, `n2h`) and the `e` array the reads fill.
//! - [`Records`]: the output file, gfortran unformatted sequential records
//!   (a 4-byte little-endian length before and after each record).

use crate::errorr::gendf::{GendfKind, GendfRecord, GendfTape};
use crate::NjoyError;

/// `isiza4` (`ccccr.f90:9`): the size of the equivalenced container in 4-byte words.
pub const ISIZA4: i64 = 100_000;
/// `mult` (`ccccr.f90:14`): 4-byte words per Hollerith word.
pub const MULT: i64 = 2;
/// `maxe` (`ccccr.f90:29`): the size of the GENDF read array `e`.
pub const MAXE: usize = 8000;

/// The equivalenced container `a`/`ia`/`ha`/`ta`.
#[derive(Debug, Clone)]
pub struct Core {
    w: Vec<u32>,
    bad: Option<i64>,
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}

impl Core {
    /// A container of zero words (a module variable is zero on the first call).
    pub fn new() -> Self {
        Self { w: vec![0; ISIZA4 as usize], bad: None }
    }

    fn slot(&mut self, i: i64) -> Option<usize> {
        if (1..=ISIZA4).contains(&i) {
            Some((i - 1) as usize)
        } else {
            self.bad.get_or_insert(i);
            None
        }
    }

    /// The raw word `i`.
    pub fn word(&mut self, i: i64) -> u32 {
        self.slot(i).map(|k| self.w[k]).unwrap_or(0)
    }

    /// Set the raw word `i`.
    pub fn set_word(&mut self, i: i64, v: u32) {
        if let Some(k) = self.slot(i) {
            self.w[k] = v;
        }
    }

    /// `a(i)`.
    pub fn a(&mut self, i: i64) -> f32 {
        f32::from_bits(self.word(i))
    }

    /// `a(i) = v`.
    pub fn set_a(&mut self, i: i64, v: f32) {
        self.set_word(i, v.to_bits());
    }

    /// `a(i) = v` for a `real(8)` right-hand side: gfortran rounds to nearest.
    pub fn set_a64(&mut self, i: i64, v: f64) {
        self.set_a(i, v as f32);
    }

    /// `ia(i)`.
    pub fn ia(&mut self, i: i64) -> i32 {
        self.word(i) as i32
    }

    /// `ia(i) = v`.
    pub fn set_ia(&mut self, i: i64, v: i32) {
        self.set_word(i, v as u32);
    }

    /// The 8 bytes of `ha(i)` (words `2i-1`, `2i`).
    pub fn ha(&mut self, i: i64) -> [u8; 8] {
        let lo = self.word(MULT * i - 1).to_le_bytes();
        let hi = self.word(MULT * i).to_le_bytes();
        [lo[0], lo[1], lo[2], lo[3], hi[0], hi[1], hi[2], hi[3]]
    }

    /// `ha(i) = v` (8 bytes).
    pub fn set_ha(&mut self, i: i64, v: [u8; 8]) {
        self.set_word(MULT * i - 1, u32::from_le_bytes([v[0], v[1], v[2], v[3]]));
        self.set_word(MULT * i, u32::from_le_bytes([v[4], v[5], v[6], v[7]]));
    }

    /// `read(h,'(a6)') ta(i)`: the text `h` (6 characters) left-justified in 8
    /// blank-padded bytes. Also `ta(i) = h` for a `character(8)` `h`.
    pub fn set_ta(&mut self, i: i64, h: &str) {
        self.set_ha(i, text8(h));
    }

    /// An index outside the container, if a routine used one.
    ///
    /// # Errors
    /// [`NjoyError::InvalidInput`] naming the first such index.
    pub fn check(&self, routine: &str) -> Result<(), NjoyError> {
        match self.bad {
            Some(i) => Err(NjoyError::InvalidInput(format!(
                "ccccr: {routine} indexed the container at {i}, outside 1..{ISIZA4} (upstream would overrun it)"
            ))),
            None => Ok(()),
        }
    }

    /// `stow(a(from), a(to), nwds)` (`ccccr.f90:3509-3524`): copy `nwds` words up.
    pub fn stow(&mut self, from: i64, to: i64, nwds: i64) {
        for n in 1..=nwds {
            let v = self.word(from + n - 1);
            self.set_word(to + n - 1, v);
        }
    }
}

/// `h` as 8 bytes: the first 8 characters, blank-padded (`a6` into `character(8)`
/// when `h` has 6 characters).
pub fn text8(h: &str) -> [u8; 8] {
    let mut out = [b' '; 8];
    for (o, b) in out.iter_mut().zip(h.bytes()) {
        *o = b;
    }
    out
}

/// `h` as exactly 6 characters (`character(6)`), blank-padded or truncated.
pub fn text6(h: &str) -> String {
    let mut s: String = h.chars().take(6).collect();
    while s.len() < 6 {
        s.push(' ');
    }
    s
}

/// An unformatted scratch tape of 4-byte words, one `Vec` per record.
#[derive(Debug, Clone, Default)]
pub struct Scratch {
    recs: Vec<Vec<u32>>,
    pos: usize,
}

impl Scratch {
    /// `repoz`: rewind.
    pub fn rewind(&mut self) {
        self.pos = 0;
    }

    /// `write(nscrt) ...`: a sequential write ends the file after this record.
    pub fn write(&mut self, rec: Vec<u32>) {
        self.recs.truncate(self.pos);
        self.recs.push(rec);
        self.pos += 1;
    }
}

/// The word indices of one I/O list, in order: `(ha(i),i=..)` contributes the
/// two words of each Hollerith item, `(a(i),i=..)` and `(ia(i),i=..)` one each.
#[derive(Debug, Clone, Default)]
pub struct Items(pub Vec<i64>);

impl Items {
    /// An empty list.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// `(ha(start+k), k=0,n-1)`.
    pub fn ha(mut self, start: i64, n: i64) -> Self {
        for k in 0..n.max(0) {
            self.0.push(MULT * (start + k) - 1);
            self.0.push(MULT * (start + k));
        }
        self
    }

    /// `(a(start+k), k=0,n-1)` or the same of `ia`.
    pub fn w(mut self, start: i64, n: i64) -> Self {
        for k in 0..n.max(0) {
            self.0.push(start + k);
        }
        self
    }
}

impl Scratch {
    /// `write(nscrt) <items>`.
    pub fn put(&mut self, core: &mut Core, it: &Items) {
        let rec = it.0.iter().map(|&i| core.word(i)).collect();
        self.write(rec);
    }

    /// `read(nscrt) <items>`.
    ///
    /// # Errors
    /// End of file, or a record shorter than the list.
    pub fn get(&mut self, core: &mut Core, it: &Items) -> Result<(), NjoyError> {
        let rec = self.recs.get(self.pos).ok_or_else(|| NjoyError::InvalidInput("ccccr: read past the end of a scratch tape".into()))?;
        if rec.len() < it.0.len() {
            return Err(NjoyError::InvalidInput("ccccr: scratch record shorter than the read".into()));
        }
        let rec = rec.clone();
        for (k, &i) in it.0.iter().enumerate() {
            core.set_word(i, rec[k]);
        }
        self.pos += 1;
        Ok(())
    }
}

/// The ENDF header variables a read sets (`endf.f90` module `endf`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Hdr {
    /// `math`.
    pub math: i32,
    /// `mfh`.
    pub mfh: i32,
    /// `mth`.
    pub mth: i32,
    /// `c1h`.
    pub c1h: f64,
    /// `c2h`.
    pub c2h: f64,
    /// `l1h`.
    pub l1h: i32,
    /// `l2h`.
    pub l2h: i32,
    /// `n1h`.
    pub n1h: i32,
    /// `n2h`.
    pub n2h: i32,
}

/// A GENDF tape read (and, for `nscrt1`, written) record by record.
///
/// On a coded unit `tosend`, `tofend` and `tomend` copy lines verbatim
/// (`endf.f90:1064-1240`) and on a binary unit records; either way a record is
/// copied unchanged, which is what [`Cursor::copy_to`] does. They read with
/// local `mat, mf, mt` and leave the header variables as the last
/// `contio`/`listio` set them.
#[derive(Debug, Clone, Default)]
pub struct Cursor {
    /// The records after the tape label.
    pub recs: Vec<GendfRecord>,
    /// The next record to read.
    pub pos: usize,
    /// At the label (just rewound): `tpidio` reads it.
    label: bool,
}

impl Cursor {
    /// A cursor at the start of `tape`.
    pub fn new(tape: &GendfTape) -> Self {
        Self { recs: tape.records.clone(), pos: 0, label: true }
    }

    /// `repoz`: rewind to the label (a scratch tape has none; see [`Cursor::tpidio`]).
    pub fn rewind(&mut self) {
        self.pos = 0;
        self.label = true;
    }

    /// `tpidio`: read the label if just rewound, else the next record (upstream
    /// reads whatever record comes, `endf.f90:528-536`). Sets `math, mfh, mth`
    /// from it (the label's `mf` and `mt` are 0).
    ///
    /// # Errors
    /// End of tape.
    pub fn tpidio(&mut self, h: &mut Hdr) -> Result<(), NjoyError> {
        if self.label {
            self.label = false;
            h.mfh = 0;
            h.mth = 0;
            return Ok(());
        }
        let r = self.next()?;
        h.math = r.mat;
        h.mfh = r.mf;
        h.mth = r.mt;
        Ok(())
    }

    fn next(&mut self) -> Result<GendfRecord, NjoyError> {
        self.label = false;
        let r = self.recs.get(self.pos).cloned().ok_or_else(|| NjoyError::InvalidInput("ccccr: read past the end of a GENDF tape".into()))?;
        self.pos += 1;
        Ok(r)
    }

    /// `contio`: one record as a CONT; `e(1..6)` = `c1, c2, l1, l2, n1, n2`.
    ///
    /// # Errors
    /// End of tape.
    pub fn contio(&mut self, h: &mut Hdr, e: &mut [f64]) -> Result<(), NjoyError> {
        let r = self.next()?;
        set_head(h, e, &r);
        Ok(())
    }

    /// `listio` with its `moreio` continuation: the head in `e(1..6)` and the
    /// words in `e(7..)`.
    ///
    /// # Errors
    /// End of tape; a list longer than `maxe` (upstream's "endf input size
    /// exceeded").
    pub fn listio(&mut self, h: &mut Hdr, e: &mut Vec<f64>) -> Result<(), NjoyError> {
        let r = self.next()?;
        set_head(h, e, &r);
        if 6 + r.data.len() > MAXE {
            return Err(NjoyError::InvalidInput("ccccr: endf input size exceeded".into()));
        }
        for (k, v) in r.data.iter().enumerate() {
            e[6 + k] = *v;
        }
        Ok(())
    }

    fn skip_until(&mut self, mut out: Option<&mut Cursor>, stop: fn(&GendfRecord) -> bool) -> Result<(), NjoyError> {
        loop {
            let r = self.next()?;
            let done = stop(&r);
            if let Some(o) = out.as_deref_mut() {
                o.write(r);
            }
            if done {
                return Ok(());
            }
        }
    }

    /// `tosend(nin, nout, 0, e)`: through the next record with `mt <= 0`.
    ///
    /// # Errors
    /// End of tape.
    pub fn tosend(&mut self, out: Option<&mut Cursor>) -> Result<(), NjoyError> {
        self.skip_until(out, |r| r.mt <= 0)
    }

    /// `tofend`: through the next record with `mf <= 0`.
    ///
    /// # Errors
    /// End of tape.
    pub fn tofend(&mut self, out: Option<&mut Cursor>) -> Result<(), NjoyError> {
        self.skip_until(out, |r| r.mf <= 0)
    }

    /// `tomend`: through the next record with `mat <= 0`.
    ///
    /// # Errors
    /// End of tape.
    pub fn tomend(&mut self, out: Option<&mut Cursor>) -> Result<(), NjoyError> {
        self.skip_until(out, |r| r.mat <= 0)
    }

    /// A sequential write: the tape ends after this record.
    pub fn write(&mut self, r: GendfRecord) {
        self.label = false;
        self.recs.truncate(self.pos);
        self.recs.push(r);
        self.pos += 1;
    }

    /// `atend(nscrt1, 0)`: the TEND record.
    pub fn atend(&mut self) {
        self.write(GendfRecord::marker(GendfKind::Tend, -1, 0));
    }
}

fn set_head(h: &mut Hdr, e: &mut [f64], r: &GendfRecord) {
    h.math = r.mat;
    h.mfh = r.mf;
    h.mth = r.mt;
    h.c1h = r.c[0];
    h.c2h = r.c[1];
    h.l1h = r.n[0];
    h.l2h = r.n[1];
    h.n1h = r.n[2];
    h.n2h = r.n[3];
    e[0] = r.c[0];
    e[1] = r.c[1];
    for k in 0..4 {
        e[2 + k] = f64::from(r.n[k]);
    }
}

/// An output file of gfortran unformatted sequential records.
#[derive(Debug, Clone, Default)]
pub struct Records {
    /// The file's bytes.
    pub bytes: Vec<u8>,
    cur: Vec<u8>,
}

impl Records {
    /// Append `ha(i)` to the record being built.
    pub fn ha(&mut self, core: &mut Core, i: i64) {
        let b = core.ha(i);
        self.cur.extend_from_slice(&b);
    }

    /// Append the 4-byte word `i` (an `a` or `ia` item: the same bytes).
    pub fn w(&mut self, core: &mut Core, i: i64) {
        let b = core.word(i).to_le_bytes();
        self.cur.extend_from_slice(&b);
    }

    /// `write(nout) <items>`: one whole record.
    pub fn put(&mut self, core: &mut Core, it: &Items) {
        for &i in &it.0 {
            self.w(core, i);
        }
        self.end();
    }

    /// End the record: length, bytes, length.
    pub fn end(&mut self) {
        let n = (self.cur.len() as u32).to_le_bytes();
        self.bytes.extend_from_slice(&n);
        self.bytes.append(&mut self.cur);
        self.bytes.extend_from_slice(&n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The equivalence: `ha(1)` is the bytes of words 1 and 2, `ta` blank-pads
    /// `a6` text, and a record carries gfortran's 4-byte markers.
    #[test]
    fn equivalence_and_record_markers() {
        let mut c = Core::new();
        c.set_ta(1, "isotxs");
        c.set_ia(3, 7);
        let mut r = Records::default();
        r.ha(&mut c, 1);
        r.w(&mut c, 3);
        r.end();
        let mut want = vec![12, 0, 0, 0];
        want.extend_from_slice(b"isotxs  ");
        want.extend_from_slice(&[7, 0, 0, 0, 12, 0, 0, 0]);
        assert_eq!(r.bytes, want);
        assert_eq!(c.word(1), u32::from_le_bytes(*b"isot"));
        assert!(c.check("t").is_ok());
        let _ = c.a(0);
        assert!(c.check("t").is_err(), "index 0 is outside the container");
    }
}
