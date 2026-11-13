// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine ngchk`, l.10865-11152 (condense the input GENDF tape).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **A GENDF tape as records** (gh:#1369), and ERRORR's `ngchk`.
//!
//! GROUPR's output (`deck::groupr::GendfText`) is a sequence of CONT and LIST
//! records with SEND/FEND/MEND/TEND markers. ERRORR reads it back record by
//! record (`contio`/`listio`). On a **binary** unit NJOY's records carry GROUPR's
//! full-precision words, so the deck driver keeps them ([`GendfRecord`]) beside
//! the text; on a **coded** unit NJOY reads the printed text, which
//! [`parse_gendf_text`] does too.
//!
//! [`ngchk`] is upstream's condensation (`errorr.f90:10865-11152`): the
//! material `matd` at temperature `tempin`, the first sigma-zero only, P0 only
//! for MF=3 and at most P0/P1 for MF=5/6/8, sections MF=3, 5, 6, 8 in that order.

use crate::endf::parse::parse_endf_float;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

/// What one GENDF line group is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GendfKind {
    /// A CONT record (`contio`).
    Cont,
    /// A LIST record (`listio` plus `moreio`): head and `n1` words.
    List,
    /// `asend`: section end.
    Send,
    /// `afend`: file end.
    Fend,
    /// `amend`: material end.
    Mend,
    /// `atend`: tape end.
    Tend,
}

/// One record of a GENDF tape at the precision it was written with.
#[derive(Debug, Clone, PartialEq)]
pub struct GendfRecord {
    /// Record kind.
    pub kind: GendfKind,
    /// `MAT`, `MF`, `MT` of the record.
    pub mat: i32,
    /// `MF`.
    pub mf: i32,
    /// `MT`.
    pub mt: i32,
    /// `c1, c2`.
    pub c: [f64; 2],
    /// `l1, l2, n1, n2` (for a LIST, `n1` is the word count).
    pub n: [i32; 4],
    /// The LIST words (empty for the other kinds).
    pub data: Vec<f64>,
}

impl GendfRecord {
    /// A marker record (`Send`, `Fend`, `Mend`, `Tend`).
    pub fn marker(kind: GendfKind, mat: i32, mf: i32) -> Self {
        Self { kind, mat, mf, mt: 0, c: [0.0; 2], n: [0; 4], data: Vec::new() }
    }
}

/// A GENDF tape: its label text and records.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GendfTape {
    /// The tape identification text (`tpidio`, 66 columns, right-trimmed).
    pub tpid: String,
    /// The records after the label, in tape order.
    pub records: Vec<GendfRecord>,
}

fn field_int(line: &str, a: usize, b: usize) -> i32 {
    line.get(a..b.min(line.len())).and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

fn six(line: &str) -> Result<[f64; 6], NjoyError> {
    let mut f = [0.0; 6];
    for (i, v) in f.iter_mut().enumerate() {
        let s = line.get(11 * i..(11 * i + 11).min(line.len())).unwrap_or("");
        *v = parse_endf_float(s)?;
    }
    Ok(f)
}

/// Read a coded GENDF tape the way `contio`/`listio` read it: within a section the
/// first record is a CONT head and every later one a LIST, up to the SEND.
///
/// # Errors
/// [`NjoyError::EndfParse`] on an unparsable field or a LIST cut short.
pub fn parse_gendf_text(text: &str) -> Result<GendfTape, NjoyError> {
    let lines: Vec<&str> = text.lines().collect();
    let Some(first) = lines.first() else {
        return Ok(GendfTape::default());
    };
    let tpid = first.get(..66.min(first.len())).unwrap_or("").trim_end().to_string();
    let mut records = Vec::new();
    let mut i = 1;
    let mut in_section = false;
    while i < lines.len() {
        let l = lines[i];
        let (mat, mf, mt) = (field_int(l, 66, 70), field_int(l, 70, 72), field_int(l, 72, 75));
        let marker = if mat == -1 {
            Some(GendfKind::Tend)
        } else if mat == 0 {
            Some(GendfKind::Mend)
        } else if mf == 0 {
            Some(GendfKind::Fend)
        } else if mt == 0 {
            Some(GendfKind::Send)
        } else {
            None
        };
        if let Some(k) = marker {
            records.push(GendfRecord::marker(k, mat, mf));
            in_section = false;
            i += 1;
            continue;
        }
        let f = six(l)?;
        let n = [f[2].round() as i32, f[3].round() as i32, f[4].round() as i32, f[5].round() as i32];
        if !in_section {
            records.push(GendfRecord { kind: GendfKind::Cont, mat, mf, mt, c: [f[0], f[1]], n, data: Vec::new() });
            in_section = true;
            i += 1;
            continue;
        }
        let npl = n[2].max(0) as usize;
        let mut data = Vec::with_capacity(npl);
        let mut j = i + 1;
        while data.len() < npl {
            let row = lines.get(j).ok_or_else(|| NjoyError::EndfParse("gendf: LIST cut short".into()))?;
            let g = six(row)?;
            let take = (npl - data.len()).min(6);
            data.extend_from_slice(&g[..take]);
            j += 1;
        }
        records.push(GendfRecord { kind: GendfKind::List, mat, mf, mt, c: [f[0], f[1]], n, data });
        i = j;
    }
    Ok(GendfTape { tpid, records })
}

/// One group record of a condensed section: `(ng2, ig2lo, ig, words)`.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupRecord {
    /// `ng2` (LIST `l1`).
    pub ng2: i32,
    /// `ig2lo` (LIST `l2`).
    pub ig2lo: i32,
    /// The group `ig` (LIST `n2`).
    pub ig: i32,
    /// The kept words, `nlnew * ng2` of them.
    pub words: Vec<f64>,
}

/// One section of the condensed GENDF (`ngchk`'s `nscr5`).
#[derive(Debug, Clone, PartialEq)]
pub struct CondensedSection {
    /// `MF`.
    pub mf: i32,
    /// `MT`.
    pub mt: i32,
    /// The head record (`nl` and `nz` already rewritten).
    pub head: GendfRecord,
    /// The group records in tape order.
    pub groups: Vec<GroupRecord>,
}

/// `ngchk`'s condensed tape for one material.
#[derive(Debug, Clone, PartialEq)]
pub struct CondensedGendf {
    /// The tape label of the input GENDF.
    pub tpid: String,
    /// `ZA`, `AWR` of the MF=1 head.
    pub za_awr: [f64; 2],
    /// The temperature of the block selected.
    pub temp: f64,
    /// Neutron group bounds `egn(1:ngn+1)` as on the tape (not `sigfig`'d).
    pub egn: Vec<f64>,
    /// MF=3, 5, 6, 8 sections in tape order.
    pub sections: Vec<CondensedSection>,
}

impl CondensedGendf {
    /// The section `(mf, mt)`, if present.
    pub fn section(&self, mf: i32, mt: i32) -> Option<&CondensedSection> {
        self.sections.iter().find(|s| s.mf == mf && s.mt == mt)
    }
}

/// `ngchk` (`errorr.f90:10865-11152`) for `matd` at `tempin` (`nmt1 = 0`).
///
/// The block is the first of `matd` whose MF=1 temperature is within 0.1 K of
/// `tempin` (below 1e4 K; a relative 1e-4 above), as upstream's search.
///
/// # Errors
/// [`NjoyError::EndfParse`] when the tape is not a GENDF (`n1 /= -1`), the material
/// or temperature is absent, or the records are out of order.
pub fn ngchk(tape: &GendfTape, matd: i32, tempin: f64) -> Result<CondensedGendf, NjoyError> {
    let r = &tape.records;
    let mut i = 0;
    let mut first = true;
    loop {
        let Some(h) = r.get(i) else {
            return Err(NjoyError::EndfParse(format!("ngchk: can't find matd {matd} on the gendf tape")));
        };
        if h.kind == GendfKind::Tend {
            return Err(NjoyError::EndfParse(format!("ngchk: can't find matd {matd} on the gendf tape")));
        }
        if h.kind != GendfKind::Cont {
            i += 1;
            continue;
        }
        if first && h.n[2] != -1 {
            return Err(NjoyError::EndfParse("ngchk: ngout is not a gendf tape".into()));
        }
        first = false;
        let list = r.get(i + 1).filter(|x| x.kind == GendfKind::List && x.mf == 1);
        let ok_mat = h.mat == matd && h.mf == 1 && h.mt == 451;
        let ok_temp = list.is_some_and(|l| {
            let t = l.c[0];
            if tempin == 0.0 && t <= 0.1 {
                true
            } else if tempin < 1.0e4 {
                (tempin - t).abs() <= 0.1
            } else {
                !((t / tempin) < 0.9999 || (t / tempin) > 1.0001)
            }
        });
        if ok_mat && ok_temp {
            break;
        }
        // tomend: skip to the material end
        while i < r.len() && r[i].kind != GendfKind::Mend {
            i += 1;
        }
        i += 1;
    }
    let head = &r[i];
    let list = &r[i + 1];
    let ntw = head.n[3].max(0) as usize;
    let ns0 = head.n[1].max(0) as usize;
    let nng = list.n[0].max(0) as usize;
    let egn: Vec<f64> = list.data.get(ntw + ns0..ntw + ns0 + nng + 1).map(<[f64]>::to_vec).ok_or_else(|| {
        NjoyError::EndfParse("ngchk: MF=1 LIST too short for the group bounds".into())
    })?;
    let temp = list.c[0];
    let mut j = i + 2;
    // the MF=1 FEND (`afend` after MF=1/451's SEND-less end)
    while j < r.len() && r[j].kind != GendfKind::Fend {
        j += 1;
    }
    j += 1;
    let mut sections = Vec::new();
    for mftest in [3, 5, 6, 8] {
        while let Some(h) = r.get(j).filter(|x| x.kind == GendfKind::Cont && x.mf == mftest) {
            let nl = h.n[0].max(1) as usize;
            let nz = h.n[1].max(1) as usize;
            let nlnew = if nl > 1 { if h.mf != 3 { 2 } else { 1 } } else { nl };
            let mut head = h.clone();
            head.n[0] = nlnew as i32;
            head.n[1] = 1;
            let ng = h.n[3];
            let mut groups = Vec::new();
            j += 1;
            let mut ig = 1;
            while ig < ng {
                let l = r.get(j).filter(|x| x.kind == GendfKind::List).ok_or_else(|| {
                    NjoyError::EndfParse("ngchk: ngout records out of order".into())
                })?;
                ig = l.n[3];
                let mut words = Vec::with_capacity(nlnew * l.n[0].max(0) as usize);
                for blk in l.data.chunks(nl * nz) {
                    for kk in 0..nlnew {
                        words.push(blk.get(kk).copied().unwrap_or(0.0));
                    }
                }
                groups.push(GroupRecord { ng2: l.n[0], ig2lo: l.n[1], ig, words });
                j += 1;
            }
            if r.get(j).map(|x| x.kind) != Some(GendfKind::Send) {
                return Err(NjoyError::EndfParse("ngchk: ngout records out of order".into()));
            }
            j += 1;
            sections.push(CondensedSection { mf: h.mf, mt: h.mt, head, groups });
            // the FEND between files
            if r.get(j).map(|x| x.kind) == Some(GendfKind::Fend) {
                j += 1;
            }
        }
    }
    Ok(CondensedGendf { tpid: tape.tpid.clone(), za_awr: head.c, temp, egn, sections })
}

/// The `sigfig`'d group bounds `ela` (`colaps`, `errorr.f90:9175-9178`).
pub fn sigfig_bounds(egn: &[f64]) -> Vec<f64> {
    egn.iter().map(|&e| sigfig(e, 6, 0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deck::groupr::GendfText;

    /// A two-group GENDF written by [`GendfText`] reads back to the same records
    /// from text, and `ngchk` keeps the first sigma-zero and P0 of MF=3.
    #[test]
    fn text_round_trip_and_ngchk_condense() {
        let mut g = GendfText::label("T");
        g.begin(100, 1, 451);
        g.cont([1.0, 2.0], [0, 2, -1, 1]);
        g.list([300.0, 0.0], 2, 0, 0, &[0.0, 1e10, 1.0, 1.0e-5, 1.0, 2.0e7]);
        g.fend();
        g.begin(100, 3, 1);
        g.cont([1.0, 0.0], [2, 2, 0, 2]);
        g.list([300.0, 0.0], 2, 1, 1, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        g.list([300.0, 0.0], 2, 1, 2, &[9.0, 9.5, 9.6, 9.7, 9.8, 9.9, 9.1, 9.2]);
        g.send();
        g.fend();
        g.end_tape();
        let parsed = parse_gendf_text(&g.text()).unwrap();
        assert_eq!(parsed.records, g.records);
        let c = ngchk(&parsed, 100, 300.0).unwrap();
        assert_eq!(c.egn, vec![1.0e-5, 1.0, 2.0e7]);
        let s = c.section(3, 1).unwrap();
        assert_eq!(s.head.n[0], 1);
        // blocks of nl*nz = 4 words: first word of each
        assert_eq!(s.groups[0].words, vec![1.0, 5.0]);
        assert_eq!(s.groups[1].words, vec![9.0, 9.8]);
        assert_eq!(sigfig_bounds(&[1.0000004e-5]), vec![sigfig(1.0e-5, 6, 0)]);
        assert!(ngchk(&parsed, 100, 600.0).is_err());
    }
}
