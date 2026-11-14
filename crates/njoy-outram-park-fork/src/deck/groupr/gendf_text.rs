// Translated from NJOY2016.80 `src/groupr.f90`: the head record of a material on
// the output tape (`:540-604`), and the ENDF line layout of
// `contio`/`listio`/`afend`/`amend`/`atend` (`src/endf.f90`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The GENDF text GROUPR writes ([`GendfText`]) and a material's MF=1 head
//! ([`write_head`]); moved out of `deck/groupr.rs` unchanged (2026-11-14, gh:#1370,
//! the file-size cap).

use crate::endf::parse::format_endf_float;
use crate::errorr::gendf::{GendfKind, GendfRecord};

/// A GENDF being written, one 80-column line per entry.
#[derive(Debug, Clone, Default)]
pub struct GendfText {
    /// The lines written so far (no newline).
    pub lines: Vec<String>,
    /// The same records at the precision they were written with (what NJOY's
    /// blocked-binary GENDF carries; ERRORR reads it, gh:#1369).
    pub records: Vec<crate::errorr::gendf::GendfRecord>,
    pub(super) mat: i32,
    pub(super) mf: i32,
    pub(super) mt: i32,
    pub(super) seq: i32,
    /// `nsh` carries on from a text copy (`tomend` and the rest write each copied
    /// line with the running `nsh` and never reset it), so a section begun after
    /// one continues the count (gh:#1370). Cleared by the END writers.
    pub(super) carry: bool,
    /// The last line written was an END record or the label.
    pub(super) fresh: bool,
}

fn i11(x: i32) -> String {
    format!("{x:>11}")
}

impl GendfText {
    /// The tape label (`tpidio`): `title` in 66 columns, `MAT = MF = MT = 0`.
    pub fn label(title: &str) -> Self {
        let mut g = Self::default();
        g.lines.push(format!("{:<66.66}{:4}{:2}{:3}{:5}", title, 0, 0, 0, 0));
        g
    }

    /// Start section `(mat, mf, mt)`: sequence numbers restart at 1.
    pub fn begin(&mut self, mat: i32, mf: i32, mt: i32) {
        self.mat = mat;
        self.mf = mf;
        self.mt = mt;
        if !self.carry {
            self.seq = 1;
        }
    }

    /// Start section `(mat, mf, mt)` with sequence number `seq` on its first line:
    /// NJOY's `nsh` is one counter for every coded unit, so text written to a
    /// scratch unit since the last SEND/FEND carries over (ERRORR's `rdumrd2`,
    /// gh:#1369).
    pub fn begin_at(&mut self, mat: i32, mf: i32, mt: i32, seq: usize) {
        self.begin(mat, mf, mt);
        self.seq = i32::try_from(seq).unwrap_or(1);
    }

    fn push_body(&mut self, body: String) {
        self.lines.push(format!("{body:<66.66}{:4}{:2}{:3}{:5}", self.mat, self.mf, self.mt, self.seq));
        self.seq += 1;
        self.fresh = false;
    }

    /// An END record was written: `asend`/`afend`/`amend`/`atend` leave `nsh = 1`.
    pub(super) fn ended(&mut self) {
        self.seq = 1;
        self.carry = false;
        self.fresh = true;
    }

    /// `contio`: a CONT record `c1 c2 l1 l2 n1 n2`.
    pub fn cont(&mut self, c: [f64; 2], n: [i32; 4]) {
        self.records.push(GendfRecord { kind: GendfKind::Cont, mat: self.mat, mf: self.mf, mt: self.mt, c, n, data: Vec::new() });
        self.cont_line(c, n);
    }

    fn cont_line(&mut self, c: [f64; 2], n: [i32; 4]) {
        let mut b = format_endf_float(c[0]);
        b.push_str(&format_endf_float(c[1]));
        for x in n {
            b.push_str(&i11(x));
        }
        self.push_body(b);
    }

    /// `listio`: a LIST record, head `c1 c2 l1 l2 npl n2`, then `data` six to a line.
    pub fn list(&mut self, c: [f64; 2], l1: i32, l2: i32, n2: i32, data: &[f64]) {
        let n = [l1, l2, data.len() as i32, n2];
        self.records.push(GendfRecord { kind: GendfKind::List, mat: self.mat, mf: self.mf, mt: self.mt, c, n, data: data.to_vec() });
        self.cont_line(c, n);
        for ch in data.chunks(6) {
            let mut b: String = ch.iter().map(|&x| format_endf_float(x)).collect();
            b.push_str(&" ".repeat(11 * (6 - ch.len())));
            self.push_body(b);
        }
    }

    /// `asend`: the section end, sequence 99999.
    pub fn send(&mut self) {
        self.records.push(GendfRecord::marker(GendfKind::Send, self.mat, self.mf));
        self.lines.push(format!("{:66}{:4}{:2}{:3}{:5}", "", self.mat, self.mf, 0, 99_999));
        self.ended();
    }

    /// `afend`: the file end.
    pub fn fend(&mut self) {
        self.records.push(GendfRecord::marker(GendfKind::Fend, self.mat, 0));
        self.lines.push(format!("{:66}{:4}{:2}{:3}{:5}", "", self.mat, 0, 0, 0));
        self.ended();
    }

    /// `amend` then `atend`: the material and tape ends.
    pub fn end_tape(&mut self) {
        self.mend();
        self.tend();
    }

    /// `amend`: the material end, written after each temperature (`:590-592`).
    pub fn mend(&mut self) {
        self.records.push(GendfRecord::marker(GendfKind::Mend, 0, 0));
        self.lines.push(format!("{:66}{:4}{:2}{:3}{:5}", "", 0, 0, 0, 0));
        self.ended();
    }

    /// `atend`: the tape end.
    pub fn tend(&mut self) {
        self.records.push(GendfRecord::marker(GendfKind::Tend, -1, 0));
        self.lines.push(format!("{:66}{:4}{:2}{:3}{:5}", "", -1, 0, 0, 0));
        self.ended();
    }

    /// The text, newline-terminated.
    pub fn text(&self) -> String {
        let mut s = self.lines.join("\n");
        s.push('\n');
        s
    }
}

/// The MF=1/MT=451 head of material `mat` (`groupr.f90:540-604`) followed by `afend`.
///
/// `za`/`awr` come from the PENDF's MF=1; `tempin` is the temperature; `egn`
/// and `egg` are the neutron and photon group boundaries (`egg` empty when
/// `ngg = 0`, then one word 0 is written as `:588` does).
pub fn write_head(g: &mut GendfText, mat: i32, za: f64, awr: f64, tempin: f64, sigz: &[f64], egn: &[f64], egg: &[f64]) {
    // `nsh = 1` (`:556`): the head starts at 1 even after a text copy.
    g.carry = false;
    g.begin(mat, 1, 451);
    let nz = sigz.len() as i32;
    g.cont([za, awr], [0, nz, -1, 1]);
    // `rtitle(1) = 0`, `sigz`, `egn(1..ngn+1)`, `egg(1..ngg+1)`.
    let mut data = vec![0.0];
    data.extend_from_slice(sigz);
    data.extend_from_slice(egn);
    data.extend_from_slice(egg);
    let ngn = (egn.len() - 1) as i32;
    let ngg = egg.len().saturating_sub(1) as i32;
    if egg.is_empty() {
        data.push(0.0);
    }
    g.list([tempin, 0.0], ngn, ngg, 0, &data);
    g.fend();
}
