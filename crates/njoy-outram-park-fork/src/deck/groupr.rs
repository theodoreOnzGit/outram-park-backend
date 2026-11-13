// Translated from NJOY2016.80 `src/groupr.f90`: `subroutine groupr` (the head
// record of a material on the output tape, `:540-604`) and the ENDF line layout
// of `contio`/`listio`/`afend`/`amend`/`atend`.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **GROUPR as a tape driver** (gh:#1369): the GENDF text NJOY writes.
//!
//! MF=3 vector sections, MF=6 `MT = 2, 51-90` two-body matrices (`nl = lord + 1`,
//! `getdis`), MF=6 `MT = 91` continuum (`getff` label 200, `groupr/continuum.rs`),
//! MF=6 thermal `MT = 221-250` (`getaed`, label 700, `groupr/thermal.rs`) and MF=16
//! photon production (`mfd = 17` MF=13/14, and `mfd = 16` MF=12 multiplicities with
//! the `econst` constant-spectrum branch, `groupr/photon_prod.rs`) are written, and
//! the **whole GENDF tape of NJOY2016 test 01 is identical** line for line (1330
//! lines, 2026-11-11, `test_01_groupr_gendf_whole_tape`; gh:#1369).
//! ~~Not yet: thermal MF=6, MF=16 photon production and the FEND/SEND/MEND layout, so
//! the whole tape is not yet compared.~~ **CORRECTED 2026-11-11** - all three are done
//! (verified by the whole-tape test). **Test 07 (U-235, card 9 `16 /` automatic
//! lists, `iwt = 9`, MF=15 continuum photons) is also identical, 467 of 467 lines
//! (2026-11-11, `test_07_groupr_gendf_whole_tape`; gh:#1481).** Still `NotPorted` here:
//! several temperatures, sigma-zeros or materials, an input GENDF (`ngout1`), MF=3
//! ~~`MT = 257-259, 452-456`~~ `MT = 452-456` (**CORRECTED 2026-11-13**: 257-259 are
//! written; 452-456 used to be skipped silently and are now refused by name), `mfd = 5`,
//! analytic `LF > 1` photon spectra, ~~and the automatic lists for `mfd` other than 3 and
//! 16/17 (and MF=18)~~ the charged-particle automatic lists (`mfd = 21-26`), `getdis`
//! with `mft = 8` (MF=6 LAW 2-5) and `getmf6` for fission.
//! ~~the automatic reaction lists (`mtd = 0`, `nextr`)~~ **CORRECTED 2026-11-11**: `auto_reactions`.
//! **CORRECTED 2026-11-13 (gh:#1369):** `auto_reactions` is replaced by
//! [`crate::deck::groupr_auto`], `conver`'s lists and `nextr` translated whole
//! (MF=4 then `mfd = 8`, MF=12/13 and the MF=6 photon list `mf18`, MF=8 production);
//! `mfd = 8`/`18` go to [`crate::groupr::mf6_feed::Mf6Feed`] (or `gam102`) and
//! production reactions (`mfd >= 10 000 000`) to `write_production`. Decks 30
//! (GENDF, 21 249 lines), 46 (GENDF, 8 281) and 73 (whole deck, 1 009) are
//! identical (`tests/deck_driver_tapes_vs_njoy2016.rs`).
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
    // The main loop's running `mtd` (`:619`, `mtd = 1`): a negative card-9 or
    // list entry `-m` visits `mtd + 1 ..= m` (`:640-646`, `:731-732`).
    let mut mtd_run = 1;
    let nend_tape = units.get(u.nendf)?.tape.clone();
    let lists = crate::deck::groupr_auto::conver_lists(&nend_tape, inp.matb)?;
    for req0 in &inp.reactions {
        // `mtd = -1000` (no `mtd` on card 9): `iauto = 1`, the lists of `nextr`.
        let items: Vec<crate::deck::groupr_auto::AutoReaction> = if req0.mtd == crate::groupr::input::MTD_AUTO {
            crate::deck::groupr_auto::nextr_all(req0.mfd, &lists, &pend, inp.matb, &mut mtd_run)?
        } else if req0.mtd < 0 {
            let mut v = Vec::new();
            loop {
                mtd_run += 1;
                v.push(crate::deck::groupr_auto::AutoReaction::plain(req0.mfd, mtd_run));
                if mtd_run + req0.mtd >= 0 {
                    break;
                }
            }
            v
        } else {
            mtd_run = req0.mtd;
            vec![crate::deck::groupr_auto::AutoReaction::plain(req0.mfd, req0.mtd)]
        };
        for item in items {
            let mtd = item.mtd;
            let req = &crate::groupr::input::ReactionRequest { mfd: item.mfd, mtd, name: String::new() };
            if req.mfd >= 10_000_000 {
                write_production(&mut g, inp, &nend_tape, &pend, za, &egn, emaxx, &item)?;
                continue;
            }
            // `getff`'s dispatch (`:7061-7073`).
            if req.mfd == 6 && (mtd == 2 || (51..=90).contains(&mtd)) {
                write_mf6_two_body(&mut g, inp, units, &pend, za, &egn, emaxx, mtd)?;
                continue;
            }
            if req.mfd == 6 && (6..=9).contains(&mtd) {
                return Err(NjoyError::NotPorted("groupr getff label 400 for mfd = 6, MT = 6-9"));
            }
            if req.mfd == 6 && (221..=250).contains(&mtd) {
                write_mf6_thermal(&mut g, inp, &pend, za, &egn, emaxx, mtd)?;
                continue;
            }
            if req.mfd == 6 {
                // label 200: `getfle` x `getsed` (`ContinuumFeed` refuses what it lacks by name).
                write_mf6_continuum(&mut g, inp, units, &pend, za, &egn, emaxx, mtd)?;
                continue;
            }
            if req.mfd == 17 || req.mfd == 16 {
                write_mf16_photon(&mut g, inp, units, &pend, za, &egn, &egg, emaxx, mtd, req.mfd)?;
                continue;
            }
            if req.mfd == 8 || req.mfd == 18 {
                write_getmf6(&mut g, inp, &nend_tape, &pend, za, &egn, &egg, emaxx, mtd, req.mfd)?;
                continue;
            }
            if req.mfd == 5 {
                return Err(NjoyError::NotPorted("groupr mfd = 5 (spectrum calculation) in the tape driver"));
            }
            if req.mfd != 3 {
                notes.push(format!("groupr: mf {} mt {} not written (matrix and photon-production sections are not ported)", req.mfd, mtd));
                return Err(NjoyError::NotPorted("groupr: this mfd in the tape driver"));
            }
            if matches!(mtd, 452 | 455 | 456) {
                return Err(NjoyError::NotPorted("groupr: mf 3 mt 452/455/456 (nu, getyld on MF=1)"));
            }
            if (257..=259).contains(&mtd) {
                // `getsig` init (`:6684-6690`): no PENDF section, `lrflag = 0`.
                let (mut feed, first) = crate::groupr::njoy_panel::SigFeed::non_cross_section(mtd);
                let mut pf = crate::groupr::njoy_panel::PanelFeed::Vector;
                let rows = crate::groupr::njoy_panel::group_general(&mut feed, &mut pf, inp.weight.iwt(), &egn, first, emaxx, false, 1)?;
                write_vector_section(&mut g, inp, za, &egn, mtd, 0, rows);
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
            let rows = crate::groupr::njoy_panel::group_general(&mut feed, &mut pf, iwt, &egn, first, emaxx, ratio, 1)?;
            write_vector_section(&mut g, inp, za, &egn, mtd, lrflag, rows);
        }
    }
    g.end_tape();
    units.put_text(u.ngout2, g.text());
    Ok(())
}

/// One MF=3 vector section (`groupr.f90:840-905`): the head `za, 0, 1, 1, lrflag,
/// ngn`, one LIST per written group, `asend`. Nothing is written when no group was
/// (threshold above the top group: `ig1 = 0`, `asend` needs `ig1 /= 0`).
fn write_vector_section(
    g: &mut GendfText,
    inp: &crate::groupr::input::GrouprInput,
    za: f64,
    egn: &[f64],
    mtd: i32,
    lrflag: i32,
    rows: Vec<crate::groupr::njoy_panel::GroupRow>,
) {
    if rows.is_empty() {
        return;
    }
    g.begin(inp.matb, 3, mtd);
    g.cont([za, 0.0], [1, 1, lrflag, (egn.len() - 1) as i32]);
    for (ig, ng2, ig2lo, data) in rows {
        g.list([inp.temperatures[0], 0.0], ng2 as i32, ig2lo, ig as i32, &data);
    }
    g.send();
}

/// A production reaction, `mfd >= 10 000 000` (`groupr.f90:688-711`, `getsig`
/// `:6676-6748`, `getff` label 100): the PENDF MF=3 cross section (LMF 3, 6, 9)
/// or the MF=10 subsection with `IZAP = izar`, `LFS = lfs` (LMF 10), times the
/// MF=6/MF=9 product yield (`getyld`) for LMF 6/9. The section is MF=3/MT=`mtd`
/// with `izam` in the head's C2 (`:843-855`).
#[allow(clippy::too_many_arguments)]
fn write_production(
    g: &mut GendfText,
    inp: &crate::groupr::input::GrouprInput,
    nend: &crate::endf::tape::Tape,
    pend: &crate::endf::tape::Tape,
    za: f64,
    egn: &[f64],
    emaxx: f64,
    r: &crate::deck::groupr_auto::AutoReaction,
) -> Result<(), NjoyError> {
    let mf = if r.mfd >= 40_000_000 { 10 } else { 3 };
    let sec = pend
        .section(inp.matb, mf, r.mtd)
        .ok_or(NjoyError::SectionNotFound { mat: inp.matb, mf, mt: r.mtd })?;
    let mut cur = crate::endf::records::SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    let tab = if mf == 10 {
        let (izar, lfs) = (r.izar(), r.lfs);
        let mut found = None;
        for _ in 0..head.n1.max(0) {
            let t = cur.read_tab1()?;
            if t.head.l1 == izar && t.head.l2 == lfs {
                found = Some(t);
            }
        }
        found.ok_or_else(|| NjoyError::InvalidInput(format!("getsig: can't find mf,mt,izar,lfs = {mf} {} {izar} {lfs}", r.mtd)))?
    } else {
        cur.read_tab1()?
    };
    let lrflag = if mf == 3 { tab.head.l2 } else { 0 };
    let (mut feed, first) = crate::groupr::njoy_panel::SigFeed::new(&tab, r.mtd);
    let mut pf = if (20_000_000..40_000_000).contains(&r.mfd) {
        // `getyld` gets `isom` for MF=6 and `lfs` for MF=9 (`:7085-7088`).
        let l = if r.mfd < 30_000_000 { r.isom } else { r.lfs };
        crate::groupr::njoy_panel::PanelFeed::Yield(crate::deck::groupr_auto::product_yield(nend, inp.matb, r.mfd, r.mtd, l)?, 2, 1)
    } else {
        crate::groupr::njoy_panel::PanelFeed::Vector
    };
    let rows = crate::groupr::njoy_panel::group_general(&mut feed, &mut pf, inp.weight.iwt(), egn, first, emaxx, false, 1)?;
    if rows.is_empty() {
        return Ok(());
    }
    g.begin(inp.matb, 3, r.mtd);
    g.cont([za, f64::from(r.izam())], [1, 1, lrflag, (egn.len() - 1) as i32]);
    for (ig, ng2, ig2lo, data) in rows {
        g.list([inp.temperatures[0], 0.0], ng2 as i32, ig2lo, ig as i32, &data);
    }
    g.send();
    Ok(())
}

/// `mfd = 8` (MF=6 neutrons, `getff` label 800) or `mfd = 18` (MF=6 photons,
/// label 810): `getmf6` times the PENDF MF=3 cross section, written as MF=6 or
/// MF=16 (`:843-848`). `nl = lord + 1` (`init`). `getsig` gives `awr` (the MF=3
/// head's C2) and `q` (the TAB1's C2), `:6716-6748`. Fission and its chances
/// (`MT = 18-21, 38`, label 800's `getyld` on MT=456 and `nl = 1`) are refused by
/// name. A photon request with no MF=6 photons ("no photons in mf6", `:7957-7961`)
/// writes nothing.
#[allow(clippy::too_many_arguments)]
fn write_getmf6(
    g: &mut GendfText,
    inp: &crate::groupr::input::GrouprInput,
    nend: &crate::endf::tape::Tape,
    pend: &crate::endf::tape::Tape,
    za: f64,
    egn: &[f64],
    egg: &[f64],
    emaxx: f64,
    mtd: i32,
    mfd: i32,
) -> Result<(), NjoyError> {
    if (18..=21).contains(&mtd) || mtd == 38 {
        return Err(NjoyError::NotPorted("groupr getff label 800 for fission (getyld on MT=456, nl = 1)"));
    }
    let Some(sec) = pend.section(inp.matb, 3, mtd) else { return Ok(()) };
    let mut cur = crate::endf::records::SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    let tab = cur.read_tab1()?;
    let lrflag = tab.head.l2;
    let nl = inp.lord.max(0) as usize + 1;
    let cfg = crate::groupr::mf6_feed::Mf6FeedConfig {
        awr: head.c2,
        awrp: 1.0,
        izap: 1,
        q: tab.head.c2,
        ismooth: inp.ismooth == crate::groupr::input::SmoothingOption::On,
        nl,
    };
    let disc102 = if mfd == 18 { photon_line_awp(nend, inp.matb, mtd)? } else { None };
    let mut pf = if let Some(d) = disc102 {
        crate::groupr::njoy_panel::PanelFeed::Gam102(crate::groupr::njoy_panel::Gam102 { disc102: d, awr: head.c2, egg: egg.to_vec(), nl, iglo: 0 })
    } else {
        let f = crate::groupr::mf6_feed::Mf6Feed::new(nend, inp.matb, mfd, mtd, cfg)?;
        if f.no_gammas() {
            return Ok(());
        }
        let eg = if mfd == 18 { egg.to_vec() } else { egn.to_vec() };
        crate::groupr::njoy_panel::PanelFeed::Mf6(f, eg, nl)
    };
    let (mut feed, first) = crate::groupr::njoy_panel::SigFeed::new(&tab, mtd);
    let rows = crate::groupr::njoy_panel::group_general(&mut feed, &mut pf, inp.weight.iwt(), egn, first, emaxx, false, nl)?;
    if rows.is_empty() {
        return Ok(());
    }
    g.begin(inp.matb, if mfd == 18 { 16 } else { 6 }, mtd);
    g.cont([za, 0.0], [nl as i32, 1, lrflag, (egn.len() - 1) as i32]);
    for (ig, ng2, ig2lo, data) in rows {
        g.list([inp.temperatures[0], 0.0], ng2 as i32, ig2lo, ig as i32, &data);
    }
    g.send();
    Ok(())
}

/// `getmf6`'s `disc102` test (`:7603-7623`): the `AWP` of the first MF=6
/// subsection with `ZAP = 0`, when it is a LAW=2 photon (a discrete line).
fn photon_line_awp(nend: &crate::endf::tape::Tape, mat: i32, mtd: i32) -> Result<Option<f64>, NjoyError> {
    let Some(sec) = nend.section(mat, 6, mtd) else { return Ok(None) };
    let mut cur = crate::endf::records::SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    for _ in 0..head.n1.max(0) {
        let t = cur.read_tab1()?;
        if t.head.c1.round() as i32 == 0 {
            return Ok((t.head.l2 == 2).then_some(t.head.c2));
        }
        crate::acer::energy::mf6::skip_mf6_subsection(&mut cur, t.head.l2)?;
    }
    Ok(None)
}

/// `mfd = 6` for `MT = 2` and `51-90` (`getff` label 400 = `getdis`, `nl = lord + 1`):
/// the infinite-dilution transfer matrix section (`groupr.f90:766-830`, `displa`).
#[allow(clippy::too_many_arguments)]
fn write_mf6_two_body(
    g: &mut GendfText,
    inp: &crate::groupr::input::GrouprInput,
    units: &mut TapeUnits,
    pend: &crate::endf::tape::Tape,
    za: f64,
    egn: &[f64],
    emaxx: f64,
    mtd: i32,
) -> Result<(), NjoyError> {
    let Some(sec) = pend.section(inp.matb, 3, mtd) else { return Ok(()) };
    let mut cur = crate::endf::records::SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    let tab = cur.read_tab1()?;
    let lrflag = tab.head.l2;
    let nl = inp.lord.max(0) as usize + 1;
    let (mut feed, first) = crate::groupr::njoy_panel::SigFeed::new(&tab, mtd);
    let ang = crate::groupr::file4::File4Angular::from_tape(&units.get(inp.units.nendf)?.tape, inp.matb, mtd, crate::groupr::file4::NLD)?;
    let tb = crate::groupr::two_body::TwoBodyFeed::new(ang, egn, head.c2, tab.head.c2, lrflag)?;
    let mut pf = crate::groupr::njoy_panel::PanelFeed::TwoBodyNl(tb, nl);
    let rows = crate::groupr::njoy_panel::group_general(&mut feed, &mut pf, inp.weight.iwt(), egn, first, emaxx, false, nl)?;
    if rows.is_empty() {
        return Ok(());
    }
    g.begin(inp.matb, 6, mtd);
    g.cont([za, 0.0], [nl as i32, 1, lrflag, (egn.len() - 1) as i32]);
    for (ig, ng2, ig2lo, data) in rows {
        g.list([inp.temperatures[0], 0.0], ng2 as i32, ig2lo, ig as i32, &data);
    }
    g.send();
    Ok(())
}

/// `mfd = 6` continuum (`getff` label 200, `MT = 91`): `getfle` x `getsed`.
#[allow(clippy::too_many_arguments)]
fn write_mf6_continuum(
    g: &mut GendfText,
    inp: &crate::groupr::input::GrouprInput,
    units: &mut TapeUnits,
    pend: &crate::endf::tape::Tape,
    za: f64,
    egn: &[f64],
    emaxx: f64,
    mtd: i32,
) -> Result<(), NjoyError> {
    let Some(sec) = pend.section(inp.matb, 3, mtd) else { return Ok(()) };
    let mut cur = crate::endf::records::SectionCursor::new(&sec.rows);
    let _head = cur.read_cont()?;
    let tab = cur.read_tab1()?;
    let lrflag = tab.head.l2;
    let (mut feed, first) = crate::groupr::njoy_panel::SigFeed::new(&tab, mtd);
    let cf = crate::groupr::continuum::ContinuumFeed::new(&units.get(inp.units.nendf)?.tape, inp.matb, mtd, egn, inp.lord.max(0) as usize + 1)?;
    let nl = cf.nl;
    let mut pf = crate::groupr::njoy_panel::PanelFeed::Continuum(cf);
    let rows = crate::groupr::njoy_panel::group_general(&mut feed, &mut pf, inp.weight.iwt(), egn, first, emaxx, false, nl)?;
    if rows.is_empty() {
        return Ok(());
    }
    g.begin(inp.matb, 6, mtd);
    g.cont([za, 0.0], [nl as i32, 1, lrflag, (egn.len() - 1) as i32]);
    for (ig, ng2, ig2lo, data) in rows {
        g.list([inp.temperatures[0], 0.0], ng2 as i32, ig2lo, ig as i32, &data);
    }
    g.send();
    Ok(())
}

/// `mfd = 6` thermal (`getff` label 700 = `getaed`, `MT = 221-250`).
fn write_mf6_thermal(
    g: &mut GendfText,
    inp: &crate::groupr::input::GrouprInput,
    pend: &crate::endf::tape::Tape,
    za: f64,
    egn: &[f64],
    emaxx: f64,
    mtd: i32,
) -> Result<(), NjoyError> {
    let Some(sec) = pend.section(inp.matb, 3, mtd) else { return Ok(()) };
    let mut cur = crate::endf::records::SectionCursor::new(&sec.rows);
    let _head = cur.read_cont()?;
    let tab = cur.read_tab1()?;
    let lrflag = tab.head.l2;
    let nl = inp.lord.max(0) as usize + 1;
    let (mut feed, first) = crate::groupr::njoy_panel::SigFeed::new(&tab, mtd);
    let tf = crate::groupr::thermal::ThermalFeed::new(pend, inp.matb, mtd, egn, nl)?;
    let mut pf = crate::groupr::njoy_panel::PanelFeed::Thermal(tf);
    let rows = crate::groupr::njoy_panel::group_general(&mut feed, &mut pf, inp.weight.iwt(), egn, first, emaxx, false, nl)?;
    if rows.is_empty() {
        return Ok(());
    }
    g.begin(inp.matb, 6, mtd);
    g.cont([za, 0.0], [nl as i32, 1, lrflag, (egn.len() - 1) as i32]);
    for (ig, ng2, ig2lo, data) in rows {
        g.list([inp.temperatures[0], 0.0], ng2 as i32, ig2lo, ig as i32, &data);
    }
    g.send();
    Ok(())
}

/// `mfd = 17` photon production from MF=13/14 (`getff` label 300), written as MF=16.
#[allow(clippy::too_many_arguments)]
fn write_mf16_photon(
    g: &mut GendfText,
    inp: &crate::groupr::input::GrouprInput,
    units: &mut TapeUnits,
    pend: &crate::endf::tape::Tape,
    za: f64,
    egn: &[f64],
    egg: &[f64],
    emaxx: f64,
    mtd: i32,
    mfd: i32,
) -> Result<(), NjoyError> {
    // `getsig` with `mfd = 17` reads MF=13 of the PENDF, `mfd = 16` reads MF=3 (`:6681`);
    // `lrflag = 0` for `mf /= 3`.
    let mf_sig = if mfd == 17 { 13 } else { 3 };
    let Some(sec) = pend.section(inp.matb, mf_sig, mtd) else { return Ok(()) };
    let mut cur = crate::endf::records::SectionCursor::new(&sec.rows);
    let _head = cur.read_cont()?;
    let tab = cur.read_tab1()?;
    let lrflag = if mfd == 16 { tab.head.l2 } else { 0 };
    let (mut feed, first) = crate::groupr::njoy_panel::SigFeed::new(&tab, mtd);
    let pf = crate::groupr::photon_prod::PhotonFeed::new(&units.get(inp.units.nendf)?.tape, inp.matb, mtd, mfd, egn, egg, inp.lord.max(0) as usize + 1)?;
    let nl = pf.nl;
    let mut pf = crate::groupr::njoy_panel::PanelFeed::Photon(pf);
    let rows = crate::groupr::njoy_panel::group_general(&mut feed, &mut pf, inp.weight.iwt(), egn, first, emaxx, false, nl)?;
    if rows.is_empty() {
        return Ok(());
    }
    g.begin(inp.matb, 16, mtd);
    g.cont([za, 0.0], [nl as i32, 1, lrflag, (egn.len() - 1) as i32]);
    for (ig, ng2, ig2lo, data) in rows {
        g.list([inp.temperatures[0], 0.0], ng2 as i32, ig2lo, ig as i32, &data);
    }
    g.send();
    Ok(())
}

#[allow(dead_code)]
fn _unused(_: NeutronGroupStructure) {}
