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
//! several temperatures, sigma-zeros ~~or materials, an input GENDF (`ngout1`)~~
//! (**CORRECTED 2026-11-14, gh:#1370:** several materials (card 10) and the input GENDF
//! run: labels 120, 410, 600 and `totend` in `groupr/gout.rs`; deck-driver README §35b), MF=3
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
//! **CORRECTED 2026-11-13 (gh:#1369, round 2):** of the "Still `NotPorted`" list above,
//! `mfd = 5` (`groupr::spectrum::mf5_spectrum`, `getsed` `LF = 1` and `LF = 5`), MF=3
//! `MT = 452-456` (`groupr::spectrum::NuYield`, `getyld` on MF=1) and `getdis` with
//! `mft = 8` for MF=6 LAW=3 are written now; ~~LAW = 2, 4, 5 are still refused.~~ **CORRECTED
//! 2026-11-14 (gh:#1370):** LAW = 2 neutrons are written too
//! (`File4Angular::from_mf6_law2`); ~~LAW = 4 and 5 are still refused.~~ **CORRECTED
//! 2026-11-14 (gh:#1370):** LAW = 4 (`File4Angular::from_mf6_law4` with
//! `TwoBodyFeed::with_law4_cosine_flip`) and LAW = 5 (`File4Angular::from_mf6_law5`) for
//! neutrons are written too; their named refusals are on those functions. `emaxx`
//! is read from `nendf`'s third MF=1 record, as `groupr.f90:413-431` does. Gated:
//! decks 29 (whole), 15, 18, 27, 34 (GENDF), 76 and 08 (GENDF without one automatic
//! card each).
//! **CORRECTED 2026-11-13 (gh:#1369 round 3, gh:#1517):** fission through `getff`
//! label 200 with `econst` (`groupr::continuum::ContinuumFeed`) and `conver`'s MF=12
//! `LO = 2` conversion (`groupr::conver::lo2_converted`) are written too. Decks 76
//! (GENDF as written), 47 (first GROUPR) and 08 (whole deck) are identical. Every
//! temperature's card-9 list is read, ~~but the run still refuses `nsigz > 1` and the
//! temperature loop~~.
//! **CORRECTED 2026-11-13 (gh:#1369 round 4):** the temperature loop and `nsigz > 1`
//! run: one MF=1 head, card-9 list and MEND per temperature on that temperature's
//! PENDF block; `stounr` and `genflx` per temperature (`groupr::flux_table`);
//! `init`'s `nz` and `nl` (MT=1 takes `nl = 2`); `getsig` with `getunr`; `panel`
//! and `displa` over `(il, iz)` (`njoy_panel::group_general_nz`). Deck 47's second
//! GROUPR and deck 11's GENDF without `6 18` are identical.
//!
//! Ported so far: the tape label, the MF=1/MT=451 head record of a material
//! (`ZA, AWR, 0, nz, -1, ntw`; LIST `temp, 0, ngn, ngg, np, 0` with the title
//! word, `sigz`, `egn`, `egg`), and the line writers. The GENDF is kept as
//! coded text on its unit (a negative unit is NJOY's blocked binary; MODER to a
//! positive unit copies the text), because the GENDF has MF=6 matrix records
//! and per-group LISTs that [`crate::endf::tape::Tape`] has no layout for.
//! **Since 2026-11-13 (gh:#1369)** the same records are also kept at full
//! precision for a negative unit (`GendfText::records`, `TapeUnits::put_gendf`),
//! which is what NJOY's ERRORR reads back from a blocked-binary GENDF.

use crate::deck::modules::GrouprJob;
use crate::deck::tapes::TapeUnits;
use crate::endf::parse::format_endf_float;
use crate::errorr::gendf::{GendfKind, GendfRecord};
use crate::errorr::groups::NeutronGroupStructure;
use crate::NjoyError;

mod gout;

/// A GENDF being written, one 80-column line per entry.
#[derive(Debug, Clone, Default)]
pub struct GendfText {
    /// The lines written so far (no newline).
    pub lines: Vec<String>,
    /// The same records at the precision they were written with (what NJOY's
    /// blocked-binary GENDF carries; ERRORR reads it, gh:#1369).
    pub records: Vec<crate::errorr::gendf::GendfRecord>,
    mat: i32,
    mf: i32,
    mt: i32,
    seq: i32,
    /// `nsh` carries on from a text copy (`tomend` and the rest write each copied
    /// line with the running `nsh` and never reset it), so a section begun after
    /// one continues the count (gh:#1370). Cleared by the END writers.
    carry: bool,
    /// The last line written was an END record or the label.
    fresh: bool,
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
    fn ended(&mut self) {
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

/// One temperature's self-shielding state: `getflx`'s source, `stounr`'s table,
/// the run's `sigz`, and the current reaction's `init` values `nz` and `nl`.
struct Shield {
    flux: crate::groupr::njoy_panel::FluxSource,
    unr: Option<std::sync::Arc<crate::groupr::unresolved::UnresolvedTable>>,
    sigz: Vec<f64>,
    nz: usize,
    nl: usize,
    mtd: i32,
}

impl Shield {
    /// `init`'s `nz` (`groupr.f90:5825-5838`) for `(mfd, mtd)`.
    fn set(&mut self, mfd: i32, mtd: i32) {
        let shielded = matches!((mfd, mtd), (3, 1 | 2 | 18 | 19 | 51 | 102 | 301 | 302 | 318 | 402 | 443 | 444) | (6, 2 | 18));
        self.nz = if shielded { self.sigz.len() } else { 1 };
        self.mtd = mtd;
    }

    /// The group loop for the current reaction: `getsig` with `getunr` when
    /// `nz > 1`, `getflx` from this temperature's flux.
    #[allow(clippy::too_many_arguments)]
    fn group(
        &mut self,
        sig: &mut crate::groupr::njoy_panel::SigFeed,
        pf: &mut crate::groupr::njoy_panel::PanelFeed,
        egn: &[f64],
        first: f64,
        emaxx: f64,
        irat: bool,
        nl: usize,
    ) -> Result<Vec<crate::groupr::njoy_panel::GroupRow>, NjoyError> {
        self.nl = nl;
        if self.nz > 1 {
            *sig = sig.clone().with_unr(self.unr.clone(), &self.sigz, self.mtd);
        }
        crate::groupr::njoy_panel::group_general_nz(sig, pf, &mut self.flux, self.nz, egn, first, emaxx, irat, nl)
    }
}

/// The blocks of material `mat` on a PENDF, one per temperature (an MF=1/MT=451
/// and the sections that follow it), each as a tape of its own.
fn temperature_blocks(pend: &crate::endf::tape::Tape, mat: i32) -> Vec<crate::endf::tape::Tape> {
    let secs = pend.sections();
    let mut out = Vec::new();
    let mut i = 0;
    while i < secs.len() {
        if secs[i].key.mat == mat && secs[i].key.mf == 1 && secs[i].key.mt == 451 {
            let mut j = i + 1;
            while j < secs.len() && secs[j].key.mat == mat && !(secs[j].key.mf == 1 && secs[j].key.mt == 451) {
                j += 1;
            }
            out.push(crate::endf::tape::Tape::from_sections(pend.tpid.clone(), secs[i..j].to_vec()));
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// `groupr`'s search for `tempin` on the PENDF (`:521-536`), from block `*k` on:
/// a block within `1 + tempin/1e4` K is taken; a hotter one first is an error.
fn find_temperature_block(blocks: &[crate::endf::tape::Tape], k: &mut usize, mat: i32, tempin: f64) -> Result<crate::endf::tape::Tape, NjoyError> {
    let eps = tempin / 10_000.0;
    while *k < blocks.len() {
        let b = &blocks[*k];
        let rows = &b.section(mat, 1, 451).ok_or_else(|| NjoyError::InvalidInput("groupr: block without MF=1".into()))?.rows;
        let iverf = crate::moder::layout::iverf_from_mf1(rows);
        let thead = match iverf {
            ..=4 => 1,
            5 => 2,
            _ => 3,
        };
        let t = rows.get(thead).map_or(0.0, |r| r[0]);
        let diff = (t - tempin).abs();
        if diff <= 1.0 + eps {
            *k += 1;
            return Ok(b.clone());
        }
        if t > tempin {
            break;
        }
        *k += 1;
    }
    Err(NjoyError::InvalidInput(format!("groupr: unable to find mat={mat} t={tempin:.4e}")))
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
    // ~~input GENDF (`ngout1`) or several materials: `NotPorted`~~ **CORRECTED
    // 2026-11-14 (gh:#1370):** both are translated (card 10, labels 120, 410, 600
    // and `totend`; `gout`).
    // `ruinb` keeps a negative first `matb` (`:1070-1073`): with `ngout1 /= 0`,
    // `findf` stops on it; with `ngout1 = 0` (`iaddmt = -1`), label 170 reads
    // `nendf`'s label as the next head and NJOY2016 never returns (run, gh:#1370).
    if inp.matb < 0 {
        if u.ngout1 != 0 {
            return Err(NjoyError::InvalidInput(format!("groupr findf: mat{} mf 1 mt451 not on tape (a negative first matb)", inp.matb)));
        }
        return Err(NjoyError::NotPorted("groupr matb < 0 with ngout1 = 0: upstream reads nendf's label as a material head and does not terminate"));
    }
    // `tosend`/`tofend`/`tomend`/`totend` refuse a coded unit copied to a binary one.
    if u.ngout1 != 0 && (u.ngout1 < 0) != (u.ngout2 < 0) {
        return Err(NjoyError::InvalidInput("groupr: mode conversion not allowed (ngout1 and ngout2 of different modes)".into()));
    }
    // ~~The tape driver runs one temperature and one sigma-zero~~ **CORRECTED
    // 2026-11-13 (gh:#1369 round 4):** the temperature loop (`:520-630`) and
    // `nsigz > 1` (`genflx`, `getflx`, `getsig`'s `getunr`, `nz > 1` in `panel` and
    // `displa`) are translated; see `groupr::flux_table`.
    // `ruinb` (`:1111`): `sigz(1) = 1e10` whatever card 6 says.
    // `genwtf` (`:4865-5113`): only `iwt = 4` stores parameters `getwtf` reads (card 8c).
    if inp.weight.iwt().abs() == 4 {
        let p = inp.thermal_fission_params.ok_or_else(|| NjoyError::InvalidInput("groupr: iwt = 4 without card 8c".into()))?;
        crate::groupr::njoy_panel::genwtf_iwt4(&p);
    }
    // `iwt = 1` stores card 8b's TAB1 (`:5011-5025`, gh:#1370); it is the last card
    // `read_groupr` keeps (after card 8a for `iwt = -1`).
    if inp.weight.iwt().abs() == 1 {
        let tab = job.weight_cards.last().ok_or_else(|| NjoyError::InvalidInput("groupr: iwt = 1 without card 8b".into()))?;
        crate::groupr::genwtf::genwtf_iwt1(tab)?;
    }
    let mut sigz: Vec<f64> = inp.sigma_zeros.clone();
    if let Some(s) = sigz.first_mut() {
        *s = 1.0e10;
    }
    units.openz(u.npend, 0)?;
    units.openz(u.nendf, 0)?;
    units.openz(u.ngout2, 1)?;
    let pend_all = units.get(u.npend)?.tape.clone();
    let egn = match &inp.neutron_grid {
        Some(g) => g.clone(),
        None => inp.neutron_groups.boundaries()?,
    };
    let egg = match &inp.photon_grid {
        Some(g) => g.clone(),
        None => inp.photon_groups.boundaries()?,
    };
    // `:413-431`: `iverf` and `emaxx` come from the first material of `nendf`: `tpidio`,
    // the head, the second record (`n1h`, `n2h` give `iverf`), and for ENDF-6 the
    // third record, whose C2 is `EMAX`.
    // ~~`emaxx` was the PENDF's `rows[1][1]`~~ **CORRECTED 2026-11-13 (gh:#1369):**
    // that is the second record's C2, `STA`; on Cf-252 decay data (deck 18, `STA = 1`)
    // it made `emaxx = 1` eV and skipped every group but the last.
    // `conver` adds MF=1/MT=456 = MT=452 when neither 455 nor 456 is present (`:11473-11550`).
    let nendf_raw = units.get(u.nendf)?.tape.clone();
    let is_mf1 = |s: &&crate::endf::tape::Section| s.key.mf == 1 && s.key.mt == 451;
    let mf1e = nendf_raw
        .sections()
        .iter()
        .find(is_mf1)
        .or_else(|| pend_all.sections().iter().find(is_mf1))
        .ok_or_else(|| NjoyError::InvalidInput(format!("groupr: unable to find mat={}", inp.matb)))?;
    let iverf = crate::moder::layout::iverf_from_mf1(&mf1e.rows);
    let emaxx = match iverf {
        ..=4 => 1.5e7,
        5 => 2.0e7,
        _ => {
            let c2 = mf1e.rows.get(2).map_or(0.0, |r| r[1]);
            if c2 > 0.0 { c2 } else { 2.0e7 }
        }
    };
    // Module state that `mfd = 5` reads without setting: `ndelg` (`conver`),
    // `dntc` (only `getyld` on MF=1/MT=455 sets it) and `lrflag` (the last `getsig`).
    // `lrflag_run` follows the `mfd = 3` branches only; an `mfd = 5` after a matrix
    // reaction (`mfd = 6/8/16/17/18` or production) would see that reaction's
    // `lrflag` upstream, which no gated deck exercises. Both carry over from one
    // temperature to the next, as module variables do.
    let mut dntc = [0.0f64; 8];
    let mut lrflag_run = 0i32;
    // Card 8a (`iwt < 0`): `fehi` is a module variable `genflx` rewrites (`:5452`),
    // and `ninwt` the binary unit it writes (`:4989`, `openz(-ninwt,1)`), gh:#1370.
    let card8a = match &inp.weight {
        crate::groupr::WeightSelection::ComputeFlux { params, .. } => *params,
        _ => None,
    };
    let mut fehi = card8a.map_or(0.0, |p| p.fehi_ev);
    let mut ninwt_file = crate::groupr::flux_table::NinwtFile::default();
    // `iwt = 0` (card 8d): `getfwt` reads the `ninwt` file a flux calculation wrote.
    let mut fwt = match &inp.weight {
        crate::groupr::WeightSelection::ReadResonanceFlux { ninwt } if sigz.len() > 1 => {
            let b = units
                .bytes_or_file(*ninwt)
                .ok_or_else(|| NjoyError::InvalidInput(format!("groupr: ninwt unit {ninwt} holds no file")))?;
            Some(crate::groupr::flux_table::FwtReader::from_bytes(&b)?)
        }
        _ => None,
    };
    // The output label (`:443`), or `ngout1`'s copied (`:451`).
    let mut gin = match u.ngout1 {
        0 => None,
        n if n > 0 => {
            let t = units.text_or_file(n).ok_or_else(|| NjoyError::InvalidInput(format!("groupr: ngout1 unit {n} holds no GENDF")))?;
            Some(gout::GoutIn::from_text(&t))
        }
        n => {
            let t = units
                .binary_gendf(n)
                .ok_or(NjoyError::NotPorted("groupr: a blocked-binary input GENDF no module of this run wrote"))?;
            Some(gout::GoutIn::from_records(t.records))
        }
    };
    let (mut g, tpid) = match gin.as_mut() {
        None => (GendfText::label(&inp.title), inp.title.clone()),
        Some(gi) => {
            let tpid = match u.ngout1 {
                n if n < 0 => units.binary_gendf(n).map(|t| t.tpid).unwrap_or_default(),
                n => units
                    .text_or_file(n)
                    .and_then(|t| t.lines().next().map(|l| l.chars().take(66).collect::<String>().trim_end().to_string()))
                    .unwrap_or_default(),
            };
            (gi.copy_label(&tpid)?, tpid)
        }
    };
    // The first material's card-9 lists, then card 10's (`:990-1001`).
    let mut first_lists = vec![(inp.reactions.clone(), job.extended.clone())];
    first_lists.extend(job.later_temperatures.iter().cloned());
    let mut materials = vec![(inp.matb, first_lists)];
    materials.extend(job.material_lists.iter().cloned());
    let mut itend = false;
    for (mat_signed, mat_lists) in materials {
    // `iaddmt` (`:994-997`); `matb = iabs(matb)`.
    let iaddmt = match (mat_signed < 0, u.ngout1 != 0) {
        (false, _) => 0,
        (true, true) => 1,
        (true, false) => {
            return Err(NjoyError::NotPorted(
                "groupr card 10 matb < 0 with ngout1 = 0: upstream reads nendf's label as a material head and does not terminate",
            ));
        }
    };
    let matb = mat_signed.abs();
    // Label 120: place `ngout1` (`:453-469`).
    if let Some(gi) = gin.as_mut() {
        gi.label120(&mut g, matb)?;
    }
    // Label 170: `conver` sets `nsh = 0` (`:10965`), and the `atend(nend2, nend3)`
    // after it (`:487`) leaves `nsh = 1` when `nend2` is coded. Only a copied head
    // (`iaddmt /= 0`) shows it; `write_head` sets `nsh = 1` (`:554`).
    g.set_nsh(i32::from(u.nendf > 0));
    if iaddmt > 0 && inp.weight.iwt() < 0 && sigz.len() > 1 {
        // `genflx` sets `nsh = 1` and copies MF=3 to a scratch unit (`:5423-5424`).
        return Err(NjoyError::NotPorted("groupr iaddmt = 1 with the flux calculator (genflx's nsh is not followed)"));
    }
    let inp_m = crate::groupr::input::GrouprInput { matb, ..inp.clone() };
    let inp = &inp_m;
    let blocks = temperature_blocks(&pend_all, inp.matb);
    if blocks.is_empty() {
        return Err(NjoyError::InvalidInput(format!("groupr: unable to find mat={}", inp.matb)));
    }
    let nend_tape = crate::groupr::conver::with_mt456(&nendf_raw, inp.matb).unwrap_or_else(|| nendf_raw.clone());
    let lists = crate::deck::groupr_auto::conver_lists(&nend_tape, inp.matb)?;
    // `nend2`, the tape `conver` writes (`:480-488`): its MF=12 `LO = 2` sections are
    // `LO = 1` yields there, and the level MF=14 sections isotropic (gh:#1517). Text
    // when `nendf` is (`:402`).
    let nend2 = crate::groupr::conver::lo2_converted(&nend_tape, inp.matb, iverf, emaxx, u.nendf > 0)?;
    let ndelg = crate::groupr::spectrum::ndelg(&nend_tape, inp.matb)?;
    let iblock = &mut 0usize;
    for itemp in 0..inp.temperatures.len() {
    let tempin = inp.temperatures[itemp];
    // `jtemp = itemp` (`:497`): `getwtf` reads `temp(jtemp)` for `iwt = 7, 10, 12`.
    crate::groupr::genwtf::getwtf_set_temperature(tempin);
    let pend = find_temperature_block(&blocks, iblock, inp.matb, tempin)?;
    let mf1 = pend
        .section(inp.matb, 1, 451)
        .ok_or_else(|| NjoyError::InvalidInput(format!("groupr: unable to find mat={}", inp.matb)))?;
    let (za, awr) = (mf1.rows[0][0], mf1.rows[0][1]);
    // `stounr` (`:533`), then `genflx` when `nsigz > 1` (`:539-546`).
    let unr = crate::groupr::flux_table::stounr(&pend, inp.matb, tempin)?.map(std::sync::Arc::new);
    let flux = if sigz.len() > 1 {
        let t = match &card8a {
            Some(p) => crate::groupr::flux_table::genflx_card8a(
                &pend,
                inp.matb,
                inp.weight.iwt(),
                inp.lord,
                &sigz,
                egn[0],
                unr.as_deref(),
                p,
                &mut fehi,
                tempin,
                &mut ninwt_file,
            )?,
            None => match fwt.as_mut() {
                Some(f) => crate::groupr::flux_table::genflx_fwt(&pend, inp.matb, inp.lord, &sigz, egn[0], unr.as_deref(), f, tempin, itemp + 1)?,
                None => crate::groupr::flux_table::genflx(&pend, inp.matb, inp.weight.iwt(), inp.lord, &sigz, egn[0], unr.as_deref())?,
            },
        };
        crate::groupr::njoy_panel::FluxSource::Table(crate::groupr::flux_table::FluxCursor::new(std::sync::Arc::new(t)))
    } else {
        crate::groupr::njoy_panel::FluxSource::Wtf(inp.weight.iwt())
    };
    let mut sh = Shield { flux, unr, sigz: sigz.clone(), nz: 1, nl: 1, mtd: 0 };
    match gin.as_mut() {
        Some(gi) if iaddmt != 0 => gi.copy_head(&mut g)?,
        _ => write_head(&mut g, inp.matb, za, awr, tempin, &sigz, &egn, &egg),
    }
    // This temperature's card-9 list (`:586-630`), and `mtd = 1` again (`:619`).
    let (reactions, extended) = mat_lists
        .get(itemp)
        .cloned()
        .ok_or_else(|| NjoyError::InvalidInput(format!("groupr: no card-9 list for mat {matb} temperature {}", itemp + 1)))?;
    let inp_t = crate::groupr::input::GrouprInput { temperatures: vec![tempin], reactions, ..inp.clone() };
    let inp = &inp_t;
    // The main loop's running `mtd` (`:619`, `mtd = 1`): a negative card-9 or
    // list entry `-m` visits `mtd + 1 ..= m` (`:640-646`, `:731-732`).
    let mut mtd_run = 1;
    for card in card9_sequence(&inp.reactions, &extended) {
        let req0 = match card {
            Card9Item::Extended(e) => {
                // Card 9a: one request, processed where it stands in the list.
                let item = card9a_reaction(e, &mut mtd_run)?;
                if let Some(gi) = gin.as_mut().filter(|_| iaddmt > 0) {
                    gi.label410(&mut g, item.mfd, item.mtd)?;
                }
                sh.set(item.mfd, item.mtd);
                write_production(&mut g, &mut sh, inp, &nend_tape, &pend, za, &egn, emaxx, &item)?;
                continue;
            }
            Card9Item::Plain(r) => r,
        };
        let req0 = &req0;
        // `mtd = -1000` (no `mtd` on card 9): `iauto = 1`, the lists of `nextr`.
        let items: Vec<crate::deck::groupr_auto::AutoReaction> = if req0.mtd == crate::groupr::input::MTD_AUTO {
            crate::deck::groupr_auto::nextr_all(req0.mfd, &lists, &pend, inp.matb, &mut mtd_run)?
        } else if req0.mtd < 0 {
            // `mfchk` (`:716`) rewrites `mfd` in place, so once a range switches
            // (16 to 17, 13 to 12) its later MTs keep the new file (gh:#1370).
            let mut mfd_run = req0.mfd;
            let mut v = Vec::new();
            loop {
                mtd_run += 1;
                mfd_run = crate::deck::groupr_auto::mfchk(mfd_run, mtd_run, &lists);
                v.push(crate::deck::groupr_auto::AutoReaction::plain(mfd_run, mtd_run));
                if mtd_run + req0.mtd >= 0 {
                    break;
                }
            }
            v
        } else {
            mtd_run = req0.mtd;
            let mfd = crate::deck::groupr_auto::mfchk(req0.mfd, req0.mtd, &lists);
            vec![crate::deck::groupr_auto::AutoReaction::plain(mfd, req0.mtd)]
        };
        for item in items {
            let mtd = item.mtd;
            let req = &crate::groupr::input::ReactionRequest { mfd: item.mfd, mtd, name: String::new() };
            // Label 410 (`iaddmt > 0`): the old sections before this one.
            if let Some(gi) = gin.as_mut().filter(|_| iaddmt > 0) {
                gi.label410(&mut g, item.mfd, mtd)?;
            }
            sh.set(req.mfd, mtd);
            if req.mfd >= 10_000_000 {
                write_production(&mut g, &mut sh, inp, &nend_tape, &pend, za, &egn, emaxx, &item)?;
                continue;
            }
            // `getff`'s dispatch (`:7061-7073`).
            if req.mfd == 6 && (mtd == 2 || (51..=90).contains(&mtd)) {
                write_mf6_two_body(&mut g, &mut sh, inp, units, &pend, za, &egn, emaxx, mtd)?;
                continue;
            }
            if req.mfd == 6 && (6..=9).contains(&mtd) {
                return Err(NjoyError::NotPorted("groupr getff label 400 for mfd = 6, MT = 6-9"));
            }
            if req.mfd == 6 && (221..=250).contains(&mtd) {
                write_mf6_thermal(&mut g, &mut sh, inp, &pend, za, &egn, emaxx, mtd)?;
                continue;
            }
            if req.mfd == 6 {
                // label 200: `getfle` x `getsed` (`ContinuumFeed` refuses what it lacks by name).
                write_mf6_continuum(&mut g, &mut sh, inp, &nend_tape, &pend, za, &egn, emaxx, mtd)?;
                continue;
            }
            if req.mfd == 17 || req.mfd == 16 {
                let nend2_tape = match &nend2 {
                    Some(t) => t,
                    None => &nend_tape,
                };
                write_mf16_photon(&mut g, &mut sh, inp, nend2_tape, &pend, za, &egn, &egg, emaxx, mtd, req.mfd)?;
                continue;
            }
            if req.mfd == 8 || req.mfd == 18 {
                write_getmf6(&mut g, &mut sh, inp, &nend_tape, &pend, za, &egn, &egg, emaxx, mtd, req.mfd)?;
                continue;
            }
            if req.mfd == 5 {
                let ismooth = inp.ismooth == crate::groupr::input::SmoothingOption::On;
                let rec = crate::groupr::spectrum::mf5_spectrum(&nend_tape, inp.matb, mtd, &egn, ismooth, ndelg, &dntc)?;
                write_mf5_spectrum(&mut g, inp, za, &egn, mtd, lrflag_run, &rec);
                continue;
            }
            if req.mfd != 3 {
                notes.push(format!("groupr: mf {} mt {} not written (matrix and photon-production sections are not ported)", req.mfd, mtd));
                return Err(NjoyError::NotPorted("groupr: this mfd in the tape driver"));
            }
            if matches!(mtd, 452 | 455 | 456) {
                // `getsig` reads MT=18 (`:6738`); `getff` label 100 with `getyld` on MF=1.
                let Some(sec) = pend.section(inp.matb, 3, 18) else { continue };
                let mut cur = crate::endf::records::SectionCursor::new(&sec.rows);
                let _head = cur.read_cont()?;
                let tab = cur.read_tab1()?;
                lrflag_run = tab.head.l2;
                let (nu, d) = crate::groupr::spectrum::NuYield::from_tape(&nend_tape, inp.matb, mtd)?;
                if let Some(d) = d {
                    dntc = d;
                }
                let (mut feed, first) = crate::groupr::njoy_panel::SigFeed::new(&tab, mtd);
                let mut pf = crate::groupr::njoy_panel::PanelFeed::Nu(nu);
                let rows = sh.group(&mut feed, &mut pf, &egn, first, emaxx, true, 1)?;
                write_vector_section(&mut g, &mut sh, inp, za, &egn, mtd, lrflag_run, rows);
                continue;
            }
            if (257..=259).contains(&mtd) {
                lrflag_run = 0;
                // `getsig` init (`:6684-6690`): no PENDF section, `lrflag = 0`.
                let (mut feed, first) = crate::groupr::njoy_panel::SigFeed::non_cross_section(mtd);
                let mut pf = crate::groupr::njoy_panel::PanelFeed::Vector;
                let rows = sh.group(&mut feed, &mut pf, &egn, first, emaxx, false, 1)?;
                write_vector_section(&mut g, &mut sh, inp, za, &egn, mtd, 0, rows);
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
            lrflag_run = lrflag;
            let (mut feed, first) = crate::groupr::njoy_panel::SigFeed::new(&tab, mtd);
            let nl1 = if mtd == 1 && sh.sigz.len() > 1 { 2 } else { 1 };
            let mut pf = if ratio {
                let nend = &units.get(u.nendf)?.tape;
                let ang = crate::groupr::file4::File4Angular::from_tape(nend, inp.matb, 2, crate::groupr::file4::NLD)?;
                let awr = getfle_mf4_awr(nend, inp.matb, 2).unwrap_or(head.c2);
                let tb = crate::groupr::two_body::TwoBodyFeed::new(ang, &egn, awr, tab.head.c2, lrflag)?;
                crate::groupr::njoy_panel::PanelFeed::TwoBody(tb.for_mf3_ratio(mtd, inp.lord.max(0) as usize))
            } else {
                crate::groupr::njoy_panel::PanelFeed::Vector
            };
            let rows = sh.group(&mut feed, &mut pf, &egn, first, emaxx, ratio, nl1)?;
            write_vector_section(&mut g, &mut sh, inp, za, &egn, mtd, lrflag, rows);
        }
    }
    // Label 590: `amend`, or for `iaddmt > 0` label 600's copy.
    match gin.as_mut() {
        Some(gi) if iaddmt > 0 => itend |= gi.label600(&mut g, inp.matb)?,
        _ => g.mend(),
    }
    }
    }
    // Label 650: `atend`, or `totend` from `ngout1` unless its TEND was copied.
    match gin.as_mut() {
        None => g.tend(),
        Some(gi) if !itend => gi.totend(Some(&mut g))?,
        Some(_) => {}
    }
    units.put_gendf(u.ngout2, g.text(), crate::errorr::gendf::GendfTape { tpid, records: g.records.clone() });
    if let Some(p) = card8a.filter(|p| p.ninwt != 0) {
        units.put_bytes(-p.ninwt.abs(), ninwt_file.bytes);
    }
    Ok(())
}

/// The AWR `getdis` uses with `mft = 4`: `getfle` sets the module `awr` to the
/// MF=4 head's C2 (`groupr.f90:9711-9713`), which can differ from the MF=3 head's
/// (Be-9 ENDF/B-VIII.0: 8.934760 against 8.934780; gh:#1370). `None` when the
/// section is absent (then `from_tape` has already said so, or it is isotropic).
fn getfle_mf4_awr(nend: &crate::endf::tape::Tape, mat: i32, mt: i32) -> Option<f64> {
    nend.section(mat, 4, mt).and_then(|s| s.rows.first()).map(|r| r[1])
}

/// One entry of a GROUPR card-9 list, in card order: a plain card 9
/// request, or a card 9a extended one `(mfd, mtd, izar, lfs)`.
#[derive(Debug, Clone, PartialEq)]
enum Card9Item {
    Plain(crate::groupr::input::ReactionRequest),
    Extended([i32; 4]),
}

/// The card-9 list in the order it was read: each card 9a request goes before
/// the plain request whose index it carries ([`crate::deck::modules::GrouprJob::extended`]).
fn card9_sequence(plain: &[crate::groupr::input::ReactionRequest], extended: &[(usize, [i32; 4])]) -> Vec<Card9Item> {
    let mut out = Vec::with_capacity(plain.len() + extended.len());
    let mut ext = extended.iter().peekable();
    for (i, r) in plain.iter().enumerate() {
        while let Some((_, e)) = ext.next_if(|(at, _)| *at <= i) {
            out.push(Card9Item::Extended(*e));
        }
        out.push(Card9Item::Plain(r.clone()));
    }
    out.extend(ext.map(|(_, e)| Card9Item::Extended(*e)));
    out
}

/// Card 9a, down-converted as `groupr.f90:655-676` does: `mfd` 3/6/9/10 becomes
/// `10/20/30/40 000 000 + 10*izar`, `lfs` and `isom` are the card's level, and
/// `mtd = mtdp` (also the running `mtd`). A negative `mtdp` (a range through
/// card 9a, `:731`) is refused by name, as is any other `mfd` (`illegal file`).
fn card9a_reaction(e: [i32; 4], mtd_run: &mut i32) -> Result<crate::deck::groupr_auto::AutoReaction, NjoyError> {
    let [mfd, mtdp, izar, lfs] = e;
    let base = match mfd {
        3 => 10_000_000,
        6 => 20_000_000,
        9 => 30_000_000,
        10 => 40_000_000,
        _ => return Err(NjoyError::InvalidInput(format!("groupr: illegal file ={mfd:3}"))),
    };
    if mtdp < 0 {
        return Err(NjoyError::NotPorted("groupr card 9a with a negative mtd (a range)"));
    }
    *mtd_run = mtdp;
    Ok(crate::deck::groupr_auto::AutoReaction { mfd: base + izar * 10, mtd: mtdp, lfs, isom: lfs, card9a: true })
}

/// One MF=3 vector section (`groupr.f90:840-905`): the head `za, 0, 1, 1, lrflag,
/// ngn`, one LIST per written group, `asend`. Nothing is written when no group was
/// (threshold above the top group: `ig1 = 0`, `asend` needs `ig1 /= 0`).
fn write_vector_section(
    g: &mut GendfText,
    sh: &mut Shield,
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
    g.cont([za, 0.0], [sh.nl as i32, sh.nz as i32, lrflag, (egn.len() - 1) as i32]);
    for (ig, ng2, ig2lo, data) in rows {
        g.list([inp.temperatures[0], 0.0], ng2 as i32, ig2lo, ig as i32, &data);
    }
    g.send();
}

/// An `mfd = 5` section (`groupr.f90:755-779`, `:840-930`): the head `za, 0, nl, 1,
/// lrflag, ngn` (with `lrflag` the module variable the previous `getsig` left),
/// one LIST `temp, 0, ng2, ig2lo = 1, nl*ng2, ig = ngn`, `asend`.
fn write_mf5_spectrum(
    g: &mut GendfText,
    inp: &crate::groupr::input::GrouprInput,
    za: f64,
    egn: &[f64],
    mtd: i32,
    lrflag: i32,
    rec: &crate::groupr::spectrum::SpectrumRecord,
) {
    let ngn = (egn.len() - 1) as i32;
    g.begin(inp.matb, 5, mtd);
    g.cont([za, 0.0], [rec.nl as i32, 1, lrflag, ngn]);
    g.list([inp.temperatures[0], 0.0], rec.ng2 as i32, 1, ngn, &rec.data);
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
    sh: &mut Shield,
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
    let rows = sh.group(&mut feed, &mut pf, egn, first, emaxx, false, 1)?;
    if rows.is_empty() {
        return Ok(());
    }
    g.begin(inp.matb, 3, r.mtd);
    g.cont([za, r.head_c2()], [1, sh.nz as i32, lrflag, (egn.len() - 1) as i32]);
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
    sh: &mut Shield,
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
    // `getmf6` sends LAW 2-5 to `getdis` (`:7956`, `:8096-8099`), with `mft = 8`;
    // `getfle` makes LAW=3 isotropic in the CM (`iso = 1`, `:9733-9737`).
    // LAW=2 reads its Legendre records through `getfle` with `mft = 8` (`:9733-9760`,
    // gh:#1370, `File4Angular::from_mf6_law2`).
    let neutron_law = if mfd == 8 { mf6_subsection_law(nend, inp.matb, mtd, 1)? } else { None };
    // LAW=4 backs up to the first subsection and flips the CM cosine (`:7636-7653`,
    // `:9519`); LAW=5 for a neutron reads `getco`'s `lang = LTP` branches with no
    // Coulomb term (`:9529`, `izap = 1`), gh:#1370.
    let mut pf = if matches!(neutron_law, Some(2..=5)) {
        let ang = match neutron_law {
            Some(2) => crate::groupr::file4::File4Angular::from_mf6_law2(nend, inp.matb, mtd, 1, crate::groupr::file4::NLD)?,
            Some(4) => crate::groupr::file4::File4Angular::from_mf6_law4(nend, inp.matb, mtd, 1, crate::groupr::file4::NLD)?,
            Some(5) => crate::groupr::file4::File4Angular::from_mf6_law5(nend, inp.matb, mtd, 1, crate::groupr::file4::NLD)?,
            _ => crate::groupr::file4::File4Angular::isotropic_cm(),
        };
        let mut tb = crate::groupr::two_body::TwoBodyFeed::new(ang, egn, head.c2, tab.head.c2, lrflag)?;
        if neutron_law == Some(4) {
            tb = tb.with_law4_cosine_flip();
        }
        crate::groupr::njoy_panel::PanelFeed::TwoBodyNl(tb, nl)
    } else if let Some(d) = disc102 {
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
    let rows = sh.group(&mut feed, &mut pf, egn, first, emaxx, false, nl)?;
    if rows.is_empty() {
        return Ok(());
    }
    g.begin(inp.matb, if mfd == 18 { 16 } else { 6 }, mtd);
    g.cont([za, 0.0], [nl as i32, sh.nz as i32, lrflag, (egn.len() - 1) as i32]);
    for (ig, ng2, ig2lo, data) in rows {
        g.list([inp.temperatures[0], 0.0], ng2 as i32, ig2lo, ig as i32, &data);
    }
    g.send();
    Ok(())
}

/// The LAW of the first MF=6/`mtd` subsection whose `ZAP` is `zap` (`getmf6`'s
/// search, `:7588-7615`); `None` without the section or such a subsection.
fn mf6_subsection_law(nend: &crate::endf::tape::Tape, mat: i32, mtd: i32, zap: i32) -> Result<Option<i32>, NjoyError> {
    let Some(sec) = nend.section(mat, 6, mtd) else { return Ok(None) };
    let mut cur = crate::endf::records::SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    for _ in 0..head.n1.max(0) {
        let t = cur.read_tab1()?;
        if t.head.c1.round() as i32 == zap {
            return Ok(Some(t.head.l2));
        }
        crate::acer::energy::mf6::skip_mf6_subsection(&mut cur, t.head.l2)?;
    }
    Ok(None)
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
    sh: &mut Shield,
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
    let nend = &units.get(inp.units.nendf)?.tape;
    let ang = crate::groupr::file4::File4Angular::from_tape(nend, inp.matb, mtd, crate::groupr::file4::NLD)?;
    let awr = getfle_mf4_awr(nend, inp.matb, mtd).unwrap_or(head.c2);
    let tb = crate::groupr::two_body::TwoBodyFeed::new(ang, egn, awr, tab.head.c2, lrflag)?;
    let mut pf = crate::groupr::njoy_panel::PanelFeed::TwoBodyNl(tb, nl);
    let rows = sh.group(&mut feed, &mut pf, egn, first, emaxx, false, nl)?;
    if rows.is_empty() {
        return Ok(());
    }
    g.begin(inp.matb, 6, mtd);
    g.cont([za, 0.0], [nl as i32, sh.nz as i32, lrflag, (egn.len() - 1) as i32]);
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
    sh: &mut Shield,
    inp: &crate::groupr::input::GrouprInput,
    nend: &crate::endf::tape::Tape,
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
    let ismooth = inp.ismooth == crate::groupr::input::SmoothingOption::On;
    let cf = crate::groupr::continuum::ContinuumFeed::new(nend, inp.matb, mtd, egn, inp.lord.max(0) as usize + 1, ismooth)?;
    let nl = cf.nl;
    let mut pf = crate::groupr::njoy_panel::PanelFeed::Continuum(cf);
    let rows = sh.group(&mut feed, &mut pf, egn, first, emaxx, false, nl)?;
    if rows.is_empty() {
        return Ok(());
    }
    g.begin(inp.matb, 6, mtd);
    g.cont([za, 0.0], [nl as i32, sh.nz as i32, lrflag, (egn.len() - 1) as i32]);
    for (ig, ng2, ig2lo, data) in rows {
        g.list([inp.temperatures[0], 0.0], ng2 as i32, ig2lo, ig as i32, &data);
    }
    g.send();
    Ok(())
}

/// `mfd = 6` thermal (`getff` label 700 = `getaed`, `MT = 221-250`).
fn write_mf6_thermal(
    g: &mut GendfText,
    sh: &mut Shield,
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
    let rows = sh.group(&mut feed, &mut pf, egn, first, emaxx, false, nl)?;
    if rows.is_empty() {
        return Ok(());
    }
    g.begin(inp.matb, 6, mtd);
    g.cont([za, 0.0], [nl as i32, sh.nz as i32, lrflag, (egn.len() - 1) as i32]);
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
    sh: &mut Shield,
    inp: &crate::groupr::input::GrouprInput,
    nend2: &crate::endf::tape::Tape,
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
    let pf = crate::groupr::photon_prod::PhotonFeed::new(nend2, inp.matb, mtd, mfd, egn, egg, inp.lord.max(0) as usize + 1)?;
    let nl = pf.nl;
    let mut pf = crate::groupr::njoy_panel::PanelFeed::Photon(pf);
    let rows = sh.group(&mut feed, &mut pf, egn, first, emaxx, false, nl)?;
    if rows.is_empty() {
        return Ok(());
    }
    g.begin(inp.matb, 16, mtd);
    g.cont([za, 0.0], [nl as i32, sh.nz as i32, lrflag, (egn.len() - 1) as i32]);
    for (ig, ng2, ig2lo, data) in rows {
        g.list([inp.temperatures[0], 0.0], ng2 as i32, ig2lo, ig as i32, &data);
    }
    g.send();
    Ok(())
}

#[allow(dead_code)]
fn _unused(_: NeutronGroupStructure) {}
