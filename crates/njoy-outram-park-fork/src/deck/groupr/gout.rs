// Translated from NJOY2016.80 `src/endf.f90` (`contio`, `tpidio`, `tosend`,
// `tofend`, `tomend`, `totend`) and `src/util.f90` (`skiprz`), as GROUPR uses them
// on its input GENDF `ngout1` (`groupr.f90:433-470`, `:560-604`, `:405-432`,
// `:599-630`, `:1004-1006`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! GROUPR's **input GENDF** (`ngout1`): a read position on that unit, and the
//! copies GROUPR makes from it onto the output GENDF (gh:#1370).
//!
//! A coded unit is read line by line, as `contio` and the `to*end` routines read
//! it, and a copied line keeps its 66 columns and `MAT, MF, MT` but takes the
//! running `nsh` as its sequence number; an END line copied this way does not
//! reset `nsh` (only `asend`, `afend`, `amend`, `atend` do), so the count runs
//! on across a copied material. A blocked-binary unit is read record by record
//! and copied record for record; its text is what MODER would write from it.

use super::GendfText;
use crate::endf::parse::parse_endf_float;
use crate::errorr::gendf::{GendfKind, GendfRecord};
use crate::NjoyError;

/// What `ngout1` holds: the lines of a coded GENDF, or the records of a
/// blocked-binary one.
#[derive(Debug, Clone)]
enum GoutSource {
    Text(Vec<String>),
    Binary(Vec<GendfRecord>),
}

/// One record as `contio` reads it: `MAT, MF, MT`, `c1, c2, l1, l2, n1, n2`, and
/// what a copy writes (the line, or the whole record).
#[derive(Debug, Clone)]
pub(super) struct GoutRec {
    pub(super) mat: i32,
    pub(super) mf: i32,
    pub(super) mt: i32,
    c: [f64; 2],
    n: [i32; 4],
}

/// The input GENDF and the read position on it (`ngout1`).
#[derive(Debug, Clone)]
pub(super) struct GoutIn {
    src: GoutSource,
    /// The next line (coded) or record (binary) to read; the label is item 0.
    pos: usize,
}

fn eof() -> NjoyError {
    NjoyError::InvalidInput("groupr: end of file on the input GENDF ngout1 (upstream stops with a Fortran runtime error)".into())
}

fn field_int(line: &str, a: usize, b: usize) -> i32 {
    line.get(a..b.min(line.len())).and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

impl GoutIn {
    /// A coded input GENDF (its lines, label first).
    pub(super) fn from_text(text: &str) -> Self {
        Self { src: GoutSource::Text(text.lines().map(str::to_string).collect()), pos: 0 }
    }

    /// A blocked-binary input GENDF (its records; the label is not a record here).
    pub(super) fn from_records(records: Vec<GendfRecord>) -> Self {
        Self { src: GoutSource::Binary(records), pos: 0 }
    }

    /// `tpidio(ngout1, ngout2)` (`:451`): copy the label. A coded label keeps its
    /// 66 columns and `MAT, MF, MT`; the binary record carries `tpid`.
    pub(super) fn copy_label(&mut self, tpid: &str) -> Result<GendfText, NjoyError> {
        match &self.src {
            GoutSource::Text(lines) => {
                let l = lines.first().ok_or_else(eof)?;
                let head: String = l.chars().take(66).collect();
                let mut g = GendfText::label(&head);
                g.lines[0] = format!("{head:<66}{:4}{:2}{:3}{:5}", field_int(l, 66, 70), field_int(l, 70, 72), field_int(l, 72, 75), 0);
                self.pos = 1;
                Ok(g)
            }
            GoutSource::Binary(_) => {
                self.pos = 0;
                Ok(GendfText::label(tpid))
            }
        }
    }

    /// `contio(ngout1, 0, ...)`: read one record (its first six words).
    pub(super) fn contio(&mut self) -> Result<GoutRec, NjoyError> {
        let r = match &self.src {
            GoutSource::Text(lines) => {
                let l = lines.get(self.pos).ok_or_else(eof)?;
                let mut f = [0.0f64; 6];
                for (i, v) in f.iter_mut().enumerate() {
                    *v = parse_endf_float(l.get(11 * i..(11 * i + 11).min(l.len())).unwrap_or(""))?;
                }
                GoutRec {
                    mat: field_int(l, 66, 70),
                    mf: field_int(l, 70, 72),
                    mt: field_int(l, 72, 75),
                    c: [f[0], f[1]],
                    n: [f[2].round() as i32, f[3].round() as i32, f[4].round() as i32, f[5].round() as i32],
                }
            }
            GoutSource::Binary(recs) => {
                let r = recs.get(self.pos).ok_or_else(eof)?;
                GoutRec { mat: r.mat, mf: r.mf, mt: r.mt, c: r.c, n: r.n }
            }
        };
        self.pos += 1;
        Ok(r)
    }

    /// `skiprz(ngout1, -k)`: back up `k` lines (coded) or records (binary).
    pub(super) fn back(&mut self, k: usize) {
        self.pos = self.pos.saturating_sub(k);
    }

    /// Read (and with `out`, copy) items until `stop` holds for one, that one
    /// included: the loop of `tosend`, `tofend`, `tomend` and `totend`.
    fn copy_until(&mut self, mut out: Option<&mut GendfText>, stop: fn(i32, i32, i32) -> bool) -> Result<(), NjoyError> {
        loop {
            let (mat, mf, mt) = match &self.src {
                GoutSource::Text(lines) => {
                    let l = lines.get(self.pos).ok_or_else(eof)?;
                    let (mat, mf, mt) = (field_int(l, 66, 70), field_int(l, 70, 72), field_int(l, 72, 75));
                    if let Some(g) = out.as_deref_mut() {
                        g.copy_line(l, mat, mf, mt);
                    }
                    (mat, mf, mt)
                }
                GoutSource::Binary(recs) => {
                    let r = recs.get(self.pos).ok_or_else(eof)?;
                    if let Some(g) = out.as_deref_mut() {
                        g.copy_record(r);
                    }
                    (r.mat, r.mf, r.mt)
                }
            };
            self.pos += 1;
            if stop(mat, mf, mt) {
                return Ok(());
            }
        }
    }

    /// `tosend`: through the SEND (`mt = 0`).
    pub(super) fn tosend(&mut self, out: Option<&mut GendfText>) -> Result<(), NjoyError> {
        self.copy_until(out, |_, _, mt| mt <= 0)
    }

    /// `tofend`: through the FEND (`mf = 0`).
    pub(super) fn tofend(&mut self, out: Option<&mut GendfText>) -> Result<(), NjoyError> {
        self.copy_until(out, |_, mf, _| mf <= 0)
    }

    /// `tomend`: through the MEND (`mat = 0`; a TEND, `mat = -1`, stops it too).
    pub(super) fn tomend(&mut self, out: Option<&mut GendfText>) -> Result<(), NjoyError> {
        self.copy_until(out, |mat, _, _| mat <= 0)
    }

    /// `totend`: through the TEND (`mat = -1`).
    pub(super) fn totend(&mut self, out: Option<&mut GendfText>) -> Result<(), NjoyError> {
        self.copy_until(out, |mat, _, _| mat <= -1)
    }
}

impl GoutIn {
    /// Label 120 (`groupr.f90:453-469`): copy each material of `ngout1` below
    /// `matb` (`contio` + `tomend`), skip one MEND-block of `matb` itself, or back
    /// up one record at a later MAT or the TEND. A record with `MAT = 1` is passed over.
    pub(super) fn label120(&mut self, g: &mut GendfText, matb: i32) -> Result<(), NjoyError> {
        loop {
            let r = self.contio()?;
            if r.mat == -1 {
                self.back(1);
                return Ok(());
            }
            if r.mat == 1 {
                continue;
            }
            if r.mat == matb {
                return self.tomend(None);
            }
            if r.mat > matb {
                self.back(1);
                return Ok(());
            }
            g.contio_copy(&r);
            self.tomend(Some(g))?;
        }
    }

    /// The MF=1 head for `iaddmt /= 0` (`:600-602`): `contio` and `tofend`, copied.
    pub(super) fn copy_head(&mut self, g: &mut GendfText) -> Result<(), NjoyError> {
        let r = self.contio()?;
        g.contio_copy(&r);
        self.tofend(Some(g))
    }

    /// Label 410 (`:405-432`, `iaddmt > 0`), before reaction `(mfd, mtd)`: copy the
    /// old sections up to the one with `(MF, MT) = (mfd, mtd)`, which is skipped;
    /// stop (backing up) at the MEND. Old SEND and FEND records are passed over.
    pub(super) fn label410(&mut self, g: &mut GendfText, mfd: i32, mtd: i32) -> Result<(), NjoyError> {
        loop {
            let r = self.contio()?;
            if r.mat == 0 && r.mf == 0 && r.mt == 0 {
                self.back(1);
                return Ok(());
            }
            if r.mf == 0 || r.mt == 0 {
                continue;
            }
            if r.mf != mfd || r.mt != mtd {
                g.contio_copy(&r);
                self.tosend(Some(g))?;
                continue;
            }
            return self.tosend(None);
        }
    }

    /// Label 600 (`:599-630`, `iaddmt > 0`) at the end of a temperature: copy a
    /// MEND or TEND (returning `true` for the TEND, `itend = 1`), or the rest of a
    /// block of `matd`; at another MAT back up two records.
    pub(super) fn label600(&mut self, g: &mut GendfText, matd: i32) -> Result<bool, NjoyError> {
        let r = self.contio()?;
        if r.mat <= 0 {
            g.contio_copy(&r);
            return Ok(r.mat < 0);
        }
        if r.mat != matd {
            self.back(2);
            return Ok(false);
        }
        g.contio_copy(&r);
        self.tomend(Some(g))?;
        Ok(false)
    }
}

impl GendfText {
    /// Set the running `nsh` to `n`, as a routine that assigns it does (`conver`).
    pub(super) fn set_nsh(&mut self, n: i32) {
        self.seq = n;
        self.carry = true;
    }

    /// A copied coded line (`tomend` and the rest): its 66 columns, `MAT, MF, MT`,
    /// and the running `nsh`, which wraps from 99999 to 1 and is not reset.
    fn copy_line(&mut self, line: &str, mat: i32, mf: i32, mt: i32) {
        let head: String = line.chars().take(66).collect();
        self.lines.push(format!("{head:<66}{mat:4}{mf:2}{mt:3}{:5}", self.seq));
        self.seq += 1;
        if self.seq > 99_999 {
            self.seq = 1;
        }
        self.carry = true;
        self.fresh = false;
    }

    /// A copied binary record, written as MODER would write it out: the END
    /// records through their writers, a CONT or LIST in its section.
    fn copy_record(&mut self, r: &GendfRecord) {
        match r.kind {
            GendfKind::Send => {
                (self.mat, self.mf) = (r.mat, r.mf);
                self.send();
            }
            GendfKind::Fend => {
                self.mat = r.mat;
                self.fend();
            }
            GendfKind::Mend => self.mend(),
            GendfKind::Tend => self.tend(),
            GendfKind::Cont | GendfKind::List => {
                if self.fresh || (self.mat, self.mf, self.mt) != (r.mat, r.mf, r.mt) {
                    self.begin(r.mat, r.mf, r.mt);
                }
                if r.kind == GendfKind::Cont {
                    self.cont(r.c, r.n);
                } else {
                    self.list(r.c, r.n[0], r.n[1], r.n[3], &r.data);
                }
            }
        }
    }

    /// `contio(0, ngout2, ...)` of a record `contio` read from `ngout1`: an END
    /// record goes through its writer (which resets `nsh`), any other is written
    /// as a CONT with the running `nsh`.
    pub(super) fn contio_copy(&mut self, r: &GoutRec) {
        if r.mat == -1 {
            self.tend();
        } else if r.mat == 0 {
            self.mend();
        } else if r.mf == 0 {
            self.mat = r.mat;
            self.fend();
        } else if r.mt == 0 {
            (self.mat, self.mf) = (r.mat, r.mf);
            self.send();
        } else {
            self.begin(r.mat, r.mf, r.mt);
            self.cont(r.c, r.n);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A copied coded material keeps its text and takes the running `nsh`, END
    /// lines included; a head written next continues it (`tomend`, gh:#1370).
    #[test]
    fn text_copy_runs_nsh_on_through_end_lines() {
        let mut src = GendfText::label("in");
        super::super::write_head(&mut src, 128, 1002.0, 1.9968, 0.0, &[1e10], &[1.0, 2.0], &[]);
        src.mend();
        src.tend();
        let mut gin = GoutIn::from_text(&src.text());
        let mut out = gin.copy_label("in").unwrap();
        let head = gin.contio().unwrap();
        assert_eq!((head.mat, head.mf, head.mt), (128, 1, 451));
        out.contio_copy(&head);
        gin.tomend(Some(&mut out)).unwrap();
        // label, head (1), LIST head (2) and data (3), FEND copied (4), MEND copied (5)
        assert_eq!(&out.lines[4][66..], " 128 0  0    4");
        assert_eq!(&out.lines[5][66..], "   0 0  0    5");
        out.begin(128, 3, 1);
        out.cont([0.0, 0.0], [0; 4]);
        assert!(out.lines[6].ends_with("    6"));
        gin.totend(Some(&mut out)).unwrap();
        assert_eq!(&out.lines[7][66..], "  -1 0  0    7");
        assert!(gin.contio().is_err(), "past the TEND is the end of the file");
    }

    /// A binary copy is record for record, numbered as MODER would write it.
    #[test]
    fn binary_copy_is_record_for_record() {
        let mut src = GendfText::label("in");
        super::super::write_head(&mut src, 128, 1002.0, 1.9968, 0.0, &[1e10], &[1.0, 2.0], &[]);
        src.mend();
        let mut gin = GoutIn::from_records(src.records.clone());
        let mut out = gin.copy_label("in").unwrap();
        gin.tomend(Some(&mut out)).unwrap();
        assert_eq!(out.records, src.records);
        assert_eq!(out.lines, src.lines);
    }
}
