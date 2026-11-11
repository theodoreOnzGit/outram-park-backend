// Translated from NJOY2016.80 `src/groupr.f90`: `subroutine groupr` (the head
// record of a material on the output tape, `:540-604`) and the ENDF line layout
// of `contio`/`listio`/`afend`/`amend`/`atend`.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **GROUPR as a tape driver** (gh:#1369): the GENDF text NJOY writes.
//!
//! Ported so far: the tape label, the MF=1/MT=451 head record of a material
//! (`ZA, AWR, 0, nz, -1, ntw`; LIST `temp, 0, ngn, ngg, np, 0` with the title
//! word, `sigz`, `egn`, `egg`), and the line writers. The GENDF is kept as
//! coded text on its unit (a negative unit is NJOY's blocked binary; MODER to a
//! positive unit copies the text), because the GENDF has MF=6 matrix records
//! and per-group LISTs that [`crate::endf::tape::Tape`] has no layout for.

use crate::deck::modules::GrouprJob;
use crate::deck::tapes::TapeUnits;
use crate::endf::parse::format_endf_float;
use crate::errorr::groups::NeutronGroupStructure;
use crate::NjoyError;

/// A GENDF being written, one 80-column line per entry.
#[derive(Debug, Clone, Default)]
pub struct GendfText {
    /// The lines written so far (no newline).
    pub lines: Vec<String>,
    mat: i32,
    mf: i32,
    mt: i32,
    seq: i32,
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
        self.seq = 1;
    }

    fn push_body(&mut self, body: String) {
        self.lines.push(format!("{body:<66.66}{:4}{:2}{:3}{:5}", self.mat, self.mf, self.mt, self.seq));
        self.seq += 1;
    }

    /// `contio`: a CONT record `c1 c2 l1 l2 n1 n2`.
    pub fn cont(&mut self, c: [f64; 2], n: [i32; 4]) {
        let mut b = format_endf_float(c[0]);
        b.push_str(&format_endf_float(c[1]));
        for x in n {
            b.push_str(&i11(x));
        }
        self.push_body(b);
    }

    /// `listio`: a LIST record, head `c1 c2 l1 l2 npl n2`, then `data` six to a line.
    pub fn list(&mut self, c: [f64; 2], l1: i32, l2: i32, n2: i32, data: &[f64]) {
        self.cont(c, [l1, l2, data.len() as i32, n2]);
        for ch in data.chunks(6) {
            let mut b: String = ch.iter().map(|&x| format_endf_float(x)).collect();
            b.push_str(&" ".repeat(11 * (6 - ch.len())));
            self.push_body(b);
        }
    }

    /// `asend`: the section end, sequence 99999.
    pub fn send(&mut self) {
        self.lines.push(format!("{:66}{:4}{:2}{:3}{:5}", "", self.mat, self.mf, 0, 99_999));
    }

    /// `afend`: the file end.
    pub fn fend(&mut self) {
        self.lines.push(format!("{:66}{:4}{:2}{:3}{:5}", "", self.mat, 0, 0, 0));
    }

    /// `amend` then `atend`: the material and tape ends.
    pub fn end_tape(&mut self) {
        self.lines.push(format!("{:66}{:4}{:2}{:3}{:5}", "", 0, 0, 0, 0));
        self.lines.push(format!("{:66}{:4}{:2}{:3}{:5}", "", -1, 0, 0, 0));
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

/// GROUPR (`groupr.f90`) on one material job: `nendf npend` in, GENDF out.
///
/// # Errors
/// [`NjoyError::NotPorted`] for options and reactions not yet translated.
pub fn run_groupr(job: &GrouprJob, units: &mut TapeUnits, notes: &mut Vec<String>) -> Result<(), NjoyError> {
    let inp = &job.input;
    let u = &inp.units;
    if u.ngout2 == 0 {
        return Err(NjoyError::NotPorted("groupr with no output unit (ngout2 = 0)"));
    }
    if u.ngout1 != 0 || !job.more_materials.is_empty() || inp.temperatures.len() != 1 || inp.sigma_zeros.len() != 1 {
        return Err(NjoyError::NotPorted("groupr input GENDF, several materials, temperatures or sigma-zeros"));
    }
    units.openz(u.npend, 0)?;
    units.openz(u.nendf, 0)?;
    units.openz(u.ngout2, 1)?;
    let pend = units.get(u.npend)?.tape.clone();
    let mf1 = pend
        .section(inp.matb, 1, 451)
        .ok_or_else(|| NjoyError::InvalidInput(format!("groupr: unable to find mat={}", inp.matb)))?;
    let (za, awr) = (mf1.rows[0][0], mf1.rows[0][1]);
    let egn = match &inp.neutron_grid {
        Some(g) => g.clone(),
        None => inp.neutron_groups.boundaries()?,
    };
    let egg = match &inp.photon_grid {
        Some(g) => g.clone(),
        None => inp.photon_groups.boundaries()?,
    };
    let mut g = GendfText::label(&inp.title);
    write_head(&mut g, inp.matb, za, awr, inp.temperatures[0], &inp.sigma_zeros, &egn, &egg);
    // MF=3 vector reactions (card 9, `mfd = 3`), `groupr.f90:766-830` with `panel`.
    let iverf = crate::moder::layout::iverf_from_mf1(&mf1.rows);
    let emaxx = match iverf {
        ..=4 => 1.5e7,
        5 => 2.0e7,
        _ => {
            let c2 = mf1.rows[1][1];
            if c2 > 0.0 { c2 } else { 2.0e7 }
        }
    };
    let mut mtd_prev = 0;
    for req in &inp.reactions {
        let mts: Vec<i32> = if req.mtd < 0 { (mtd_prev + 1..=-req.mtd).collect() } else { vec![req.mtd] };
        mtd_prev = req.mtd.abs();
        for mtd in mts {
            if req.mfd != 3 {
                notes.push(format!("groupr: mf {} mt {} not written (matrix and photon-production sections are not ported)", req.mfd, mtd));
                continue;
            }
            if matches!(mtd, 257..=259 | 452 | 455 | 456) {
                notes.push(format!("groupr: mf 3 mt {mtd} not written (thermal, mubar/xi/gamma and nu sections are not ported)"));
                continue;
            }
            // `getsig` reads MT 2 for the ratio reactions (`:6705`).
            let ratio = (251..=253).contains(&mtd);
            let mt_read = if ratio { 2 } else { mtd };
            let Some(sec) = pend.section(inp.matb, 3, mt_read) else { continue };
            let mut cur = crate::endf::records::SectionCursor::new(&sec.rows);
            let head = cur.read_cont()?;
            let tab = cur.read_tab1()?;
            let lrflag = tab.head.l2;
            let (mut feed, first) = crate::groupr::njoy_panel::SigFeed::new(&tab, mtd);
            let iwt = inp.weight.iwt();
            let mut pf = if ratio {
                let ang = crate::groupr::file4::File4Angular::from_tape(&units.get(u.nendf)?.tape, inp.matb, 2, crate::groupr::file4::NLD)?;
                let tb = crate::groupr::two_body::TwoBodyFeed::new(ang, &egn, head.c2, tab.head.c2, lrflag)?;
                crate::groupr::njoy_panel::PanelFeed::TwoBody(tb.for_mf3_ratio(mtd, inp.lord.max(0) as usize))
            } else {
                crate::groupr::njoy_panel::PanelFeed::Vector
            };
            let rows = crate::groupr::njoy_panel::group_general(&mut feed, &mut pf, iwt, &egn, first, emaxx, ratio)?;
            g.begin(inp.matb, 3, mtd);
            g.cont([za, 0.0], [1, 1, lrflag, (egn.len() - 1) as i32]);
            for (ig, ng2, ig2lo, data) in rows {
                g.list([inp.temperatures[0], 0.0], ng2 as i32, ig2lo, ig as i32, &data);
            }
            g.send();
        }
    }
    g.end_tape();
    units.put_text(u.ngout2, g.text());
    Ok(())
}

#[allow(dead_code)]
fn _unused(_: NeutronGroupStructure) {}
