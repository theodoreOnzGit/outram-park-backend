// Translated from NJOY2016.80 `src/main.f90` (the module loop) and the unit
// handling at the top of each module (`moder`, `reconr`, `broadr`, `heatr`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The deck driver: `program njoy`'s module loop, and the modules it can run.

use std::path::PathBuf;

use crate::broadr::broadn::BroadnTolerances;
use crate::broadr::joint::{broadr_joint, broadr_joint_rounding};
use crate::deck::cards::CardReader;
use crate::acer::{build_deck, AceDeck};
use crate::deck::modules::{
    read_acer, read_broadr, read_gaspr, read_groupr, read_heatr, read_moder, read_purr, read_reconr, read_thermr,
    read_unresr, read_viewr, AcerCards, BroadrCards, ModerCards, PurrCards, ReconrCards, UnresrCards,
};
use crate::deck::tapes::{PendfMeta, TapeUnits};
use crate::endf::tape::{Section, Tape};
use crate::heatr::{heatr, pendf_for_heatr, HeatrInput};
use crate::moder::{select_materials, MaterialSelection};
use crate::reconr::{ReconrConfig, ReconrResult};
use crate::NjoyError;

/// How to run a deck.
#[derive(Debug, Clone, Default)]
pub struct DeckOptions {
    /// Directory holding `tape<n>` input files (e.g. `tape20`) and receiving
    /// the coded output units. `None`: memory only.
    pub workdir: Option<PathBuf>,
    /// Read the cards of every module but run none (checks the card stream).
    pub parse_only: bool,
}

/// What a deck run produced.
#[derive(Debug)]
pub struct DeckRun {
    /// The tape units at the end of the run.
    pub units: TapeUnits,
    /// Modules run (or, with `parse_only`, parsed), in order.
    pub modules: Vec<String>,
    /// The first module that could not run, with the reason, if the run
    /// stopped there. Its cards were read; the rest of the deck was not.
    pub blocked: Option<(String, String)>,
    /// HEATR listings, in the order run.
    pub listing: String,
    /// What a module did not do that its NJOY counterpart does (a tape section
    /// not written, a file not produced), in the order met.
    pub notes: Vec<String>,
}

/// Run an NJOY input deck: `program njoy` (`main.f90`).
///
/// Reads module names until `stop`; a module name of `--` is a comment card.
/// Each module's cards are read by its reader in [`crate::deck::modules`] and
/// the module is run on the tape units. A module this crate has no tape-level
/// driver for ([`NjoyError::NotPorted`]) stops the run **with its cards read**
/// and is reported in [`DeckRun::blocked`]; any other failure is an `Err`.
///
/// # Errors
/// A malformed card, an unknown module name (`illegal module name`), a tape
/// unit error, or a module's own failure.
pub fn run_deck(text: &str, opts: &DeckOptions) -> Result<DeckRun, NjoyError> {
    let mut r = CardReader::new(text);
    let mut units = TapeUnits::new(opts.workdir.clone());
    let mut modules = Vec::new();
    let mut listing = String::new();
    let mut notes: Vec<String> = Vec::new();
    let mut blocked = None;
    // VIEWR's module variables live as long as the process (`drawh` remembers its last line width).
    let mut viewr_state = crate::viewr::ViewrState::default();
    loop {
        let name = r.read_string(6)?;
        if name == "stop" {
            break;
        }
        if name == "--" {
            continue;
        }
        let outcome = match name.as_str() {
            "moder" => {
                let c = read_moder(&mut r)?;
                if opts.parse_only { Ok(()) } else { run_moder(&c, &mut units) }
            }
            "reconr" => {
                let c = read_reconr(&mut r)?;
                if opts.parse_only { Ok(()) } else { run_reconr(&c, &mut units) }
            }
            "broadr" => {
                let c = read_broadr(&mut r)?;
                if opts.parse_only { Ok(()) } else { run_broadr(&c, &mut units) }
            }
            "heatr" => {
                let c = read_heatr(&mut r)?;
                if opts.parse_only { Ok(()) } else { run_heatr(&c, &mut units, &mut listing) }
            }
            "thermr" => {
                let c = read_thermr(&mut r)?;
                if opts.parse_only { Ok(()) } else { crate::deck::thermr::run_thermr(&c, &mut units) }
            }
            "groupr" => {
                let job = read_groupr(&mut r)?;
                if opts.parse_only { Ok(()) } else { crate::deck::groupr::run_groupr(&job, &mut units, &mut notes) }
            }
            "gaspr" => {
                let c = read_gaspr(&mut r)?;
                if opts.parse_only { Ok(()) } else { run_gaspr(c, &mut units) }
            }
            "unresr" => {
                let c = read_unresr(&mut r)?;
                if opts.parse_only { Ok(()) } else { run_unresr(&c, &mut units, &mut notes) }
            }
            "purr" => {
                let c = read_purr(&mut r)?;
                if opts.parse_only { Ok(()) } else { run_purr(&c, &mut units, &mut notes) }
            }
            "acer" => {
                let c = read_acer(&mut r)?;
                if opts.parse_only { Ok(()) } else { run_acer(&c, &mut units, &mut notes) }
            }
            "leapr" => {
                let c = crate::deck::leapr::read_leapr(&mut r)?;
                if opts.parse_only { Ok(()) } else { crate::deck::leapr::run_leapr(&c, &mut units) }
            }
            "viewr" => {
                let c = read_viewr(&mut r)?;
                if opts.parse_only { Ok(()) } else { run_viewr(c, &mut units, &mut viewr_state) }
            }
            "gaminr" | "errorr" | "covr" | "dtfr" | "ccccr" | "matxsr" | "resxsr"
            | "powr" | "wimsr" | "plotr" | "mixr" => {
                // The card readers of these modules are a later part of gh:#1369.
                blocked = Some((name.clone(), "card reader not yet translated".to_string()));
                break;
            }
            other => return Err(NjoyError::InvalidInput(format!("njoy: illegal module name---{other}"))),
        };
        modules.push(name.clone());
        if let Err(e) = outcome {
            if matches!(e, NjoyError::NotPorted(_)) {
                blocked = Some((name, e.to_string()));
                if !opts.parse_only {
                    break;
                }
            } else {
                return Err(e);
            }
        }
    }
    if opts.workdir.is_some() && !opts.parse_only {
        units.write_coded_files()?;
    }
    Ok(DeckRun { units, modules, blocked, listing, notes })
}

/// VIEWR (`viewr.f90`): the plot-command text on unit `infile` becomes the
/// PostScript file on unit `nps` ([`crate::viewr`]).
fn run_viewr(c: [i32; 2], units: &mut TapeUnits, state: &mut crate::viewr::ViewrState) -> Result<(), NjoyError> {
    let [infile, nps] = c;
    let plot = units
        .text_or_file(infile)
        .ok_or_else(|| NjoyError::InvalidInput(format!("viewr: unit {infile} holds no plot-command file")))?;
    let ps = state.viewr(&plot)?;
    units.put_text(nps, ps);
    Ok(())
}

/// MODER (`moder.f90`): mode change or material selection.
fn run_moder(c: &ModerCards, units: &mut TapeUnits) -> Result<(), NjoyError> {
    match c {
        ModerCards::Convert { nin, nout } => {
            // A GENDF unit is kept as text (`deck::groupr`); converting it is a copy.
            if let Some(t) = units.text(*nin).map(str::to_string) {
                units.put_text(*nout, t);
                return Ok(());
            }
            units.openz(*nout, 1)?;
            if nin.abs() >= 20 {
                units.openz(*nin, 0)?;
            }
            let mut u = units.get(*nin)?.clone();
            u.tape.clear_copied_closers();
            units.store(*nout, u.tape, u.pointwise)?;
            units.set_meta(*nout, u.meta);
            Ok(())
        }
        ModerCards::Select { nout, tpid, picks, .. } => {
            units.openz(*nout, 1)?;
            let mut order: Vec<i32> = Vec::new();
            let mut inputs: Vec<Tape> = Vec::new();
            let mut sels = Vec::new();
            for &(nin, matd) in picks {
                let idx = match order.iter().position(|&n| n == nin) {
                    Some(i) => i,
                    None => {
                        units.openz(nin, 0)?;
                        inputs.push(units.get(nin)?.tape.clone());
                        order.push(nin);
                        order.len() - 1
                    }
                };
                sels.push(MaterialSelection { tape_index: idx, mat: matd });
            }
            let out = select_materials(&inputs, tpid, &sels)?;
            units.store(*nout, out, None)
        }
    }
}

/// RECONR (`reconr.f90`): one pointwise PENDF per material.
fn run_reconr(c: &ReconrCards, units: &mut TapeUnits) -> Result<(), NjoyError> {
    units.openz(c.nendf, 0)?;
    units.openz(c.npend, 1)?;
    let endf = units.get(c.nendf)?.tape.clone();
    let mut sections: Vec<Section> = Vec::new();
    let mut last: Option<(i32, ReconrResult)> = None;
    for m in &c.materials {
        // ruina: errmax/errint default from err; the port's RECONR has only `err`.
        let errmax = if m.errmax <= 0.0 { 10.0 * m.err } else { m.errmax }.max(m.err);
        let errint = if m.errint <= 0.0 { m.err / 20000.0 } else { m.errint };
        if !m.enode.is_empty() {
            return Err(NjoyError::NotPorted("RECONR user grid energies enode (card 6)"));
        }
        let res = crate::reconr::reconr_for_deck(&endf, &ReconrConfig { mat: m.mat, tolerance: m.err, temperature: m.tempr }, errmax, errint, c.npend)?;
        if c.materials.len() == 1 {
            let tape = crate::deck::pendf::reconr_tape(&endf, m.mat, &res, &c.label, &m.cards, m.err, m.tempr, c.npend)?;
            let meta = PendfMeta { blocks: vec![(res.temperature_k, res.clone())], ..PendfMeta::default() };
            units.store(c.npend, tape, Some((m.mat, res)))?;
            units.set_meta(c.npend, meta);
            return Ok(());
        }
        let tape = pendf_for_heatr(&endf, m.mat, &res);
        sections.extend(tape.sections().iter().cloned());
        last = Some((m.mat, res));
    }
    let pointwise = if c.materials.len() == 1 { last } else { None };
    units.store(c.npend, Tape::from_sections(c.label.clone(), sections), pointwise)
}

/// BROADR (`broadr.f90`), single final temperature from a RECONR pointwise tape.
///
/// **Mode matters** (`broadr.f90`'s tapes): with coded units BROADR reads and
/// writes ENDF text, so its input is rounded to 11 columns and its output is
/// too; with blocked-binary units both stay at full precision.
fn run_broadr(c: &BroadrCards, units: &mut TapeUnits) -> Result<(), NjoyError> {
    if (c.nin < 0 && c.nout > 0) || (c.nin > 0 && c.nout < 0) {
        return Err(NjoyError::InvalidInput("broadr: nin and nout must be same mode.".into()));
    }
    units.openz(c.nin, 0)?;
    units.openz(c.nendf, 0)?;
    units.openz(c.nout, 1)?;
    let input = units.get(c.nin)?.clone();
    if c.materials.len() != 1 {
        return Err(NjoyError::NotPorted("BROADR with more than one material in one run"));
    }
    for m in &c.materials {
        if m.istart != 0 || m.temp1 != 0.0 {
            return Err(NjoyError::NotPorted("BROADR restart / starting temperature (cards 2)"));
        }
        let Some((mat, res)) = &input.pointwise else {
            return Err(NjoyError::NotPorted(
                "BROADR on a PENDF tape not made by RECONR in this run (no PENDF reader into pointwise form)",
            ));
        };
        if *mat != m.mat {
            return Err(NjoyError::InvalidInput(format!("broadr: material {} not on the input tape (has {mat})", m.mat)));
        }
        let text_mode = c.nin > 0;
        let mut src = if text_mode { res.through_pendf_text() } else { res.clone() };
        let mut all: Vec<Section> = Vec::new();
        let mut final_out = None;
        let mut meta = input.meta.clone();
        meta.blocks.clear();
        for &temp in &m.temps {
            // `broadr.f90:204-209`: a zero `errmax` is `10 errthn`, a zero
            // `errint` is `errthn / 20000`; `thnmx` goes to `broadr_joint`
            // (which `sigfig`s it and applies `eresh`, `emax` and the sign).
            let mut tol = BroadnTolerances::with_errthn(m.errthn);
            if m.errmax != 0.0 {
                tol.errmax = m.errmax;
            }
            if m.errint != 0.0 {
                tol.errint = m.errint;
            }
            // `tempef <= 0`: upstream only thins (`thinb`, `broadr.f90:1218-1222`),
            // which is not ported (gh:#619); refuse rather than pass RECONR's
            // grid through unthinned (deck 43, gh:#1369).
            if temp - src.temperature_k <= 0.0 {
                return Err(NjoyError::NotPorted("BROADR thinning only (tempef <= 0, thinb)"));
            }
            let out = match broadr_joint(&src, temp, &tol, m.thnmx) {
                Some(b) if text_mode => b.through_pendf_text(),
                Some(b) => b,
                None => src.clone(),
            };
            let t = crate::deck::pendf::broadr_tape(&input.tape, m.mat, &out, temp)?;
            all.extend(t.sections().iter().cloned());
            // `istrap`: the next temperature broadens from this one
            // (`broadr.f90:1004-1011`); otherwise from the input's.
            if m.istrap != 0 {
                src = broadr_joint_rounding(&src, temp, &tol, m.thnmx, false).unwrap_or_else(|| out.clone());
            }
            meta.blocks.push((temp, out.clone()));
            final_out = Some(out);
        }
        let mut tape = Tape::from_sections(input.tape.tpid.clone(), all);
        tape.copy_raw_mf32_from(&input.tape);
        let copied = crate::deck::pendf::broadr_copied_closers(&tape, m.mat);
        tape.set_copied_closers(copied);
        units.store(c.nout, tape, final_out.map(|o| (m.mat, o)))?;
        units.set_meta(c.nout, meta);
        return Ok(());
    }
    Err(NjoyError::InvalidInput("broadr: no material card".into()))
}

/// HEATR (`heatr.f90`): kerma and damage onto the PENDF.
fn run_heatr(input: &HeatrInput, units: &mut TapeUnits, listing: &mut String) -> Result<(), NjoyError> {
    let u = input.units;
    units.openz(u.nendf, 0)?;
    units.openz(u.nin, 0)?;
    units.openz(u.nout, 1)?;
    if (u.nin < 0 && u.nout > 0) || (u.nin > 0 && u.nout < 0) {
        return Err(NjoyError::InvalidInput("heatr: mode conversion not allowed between nin and nout.".into()));
    }
    let endf = units.get(u.nendf)?.tape.clone();
    let pendf = units.get(u.nin)?.clone();
    let out = heatr(&endf, &pendf.tape, input)?;
    listing.push_str(&out.listing);
    // `nplot != 0`: HEATR's `viewr` plot commands go on that unit
    // (`heatr.f90:157`, written by the plot block from `:6035`), which a later VIEWR reads (upstream test 24's
    // `viewr 26 36`, gh:#1369).
    if u.nplot != 0 {
        if let Some(p) = out.plot {
            units.put_text(u.nplot, p);
        }
    }
    // HEATR leaves the pointwise cross sections as they were; MF=3 additions
    // ride on the tape only.
    units.store(u.nout, out.tape, pendf.pointwise)?;
    let mut meta = pendf.meta;
    meta.heatr = true;
    meta.heatr_binary = u.nout < 0;
    meta.heatr_partials = [302, 318, 402].map(|mt| input.partial_kermas.contains(&mt));
    units.set_meta(u.nout, meta);
    Ok(())
}

/// GASPR (`gaspr.f90`): MT=203-207 onto the PENDF ([`crate::gaspr::gaspr_tape`]).
fn run_gaspr(c: [i32; 3], units: &mut TapeUnits) -> Result<(), NjoyError> {
    let [nendf, npend, noutp] = c;
    if npend * noutp < 0 {
        return Err(NjoyError::InvalidInput("gaspr: npend and noutp must both be binary or both be coded".into()));
    }
    units.openz(nendf, 0)?;
    units.openz(npend, 0)?;
    units.openz(noutp, 1)?;
    let endf = units.get(nendf)?.tape.clone();
    let pend = units.get(npend)?.clone();
    let mut tape = crate::gaspr::gaspr_tape(&endf, &pend.tape)?;
    // A coded GASPR output carries sequence number 1 on its TPID line (measured
    // on test 12 against NJOY2016.80: RECONR and BROADR write 0 there; a MODER
    // conversion rewrites it as 0 again, so a binary unit is unaffected).
    if noutp > 0 && tape.tpid.len() >= 75 && tape.tpid.is_char_boundary(tape.tpid.len() - 5) {
        let n = tape.tpid.len();
        tape.tpid.replace_range(n - 5..n, "    1");
    }
    units.store(noutp, tape, pend.pointwise)?;
    units.set_meta(noutp, pend.meta);
    Ok(())
}

/// Copy a PENDF unit to the output unit of a module whose tape writer is not
/// ported, recording what the module did in its [`PendfMeta`].
fn pass_through(nin: i32, nout: i32, units: &mut TapeUnits, edit: impl FnOnce(&mut PendfMeta)) -> Result<(), NjoyError> {
    if (nin < 0 && nout > 0) || (nin > 0 && nout < 0) {
        return Err(NjoyError::InvalidInput("mode conversion between nin and nout not allowed.".into()));
    }
    let u = units.get(nin)?.clone();
    units.store(nout, u.tape, u.pointwise)?;
    let mut meta = u.meta;
    edit(&mut meta);
    units.set_meta(nout, meta);
    Ok(())
}

/// UNRESR (`unresr.f90`). ~~**Cards read, tape passed through.** UNRESR's
/// self-shielded MF=2/MT=152 is not written onto the PENDF; ACER does not read
/// it.~~ **CORRECTED 2026-11-13 (gh:#1369 round 4):** each temperature's
/// MF=2/MT=152 is rewritten with UNRESR's table
/// ([`crate::unresr::tape::unresr_tape`]), which GROUPR's `stounr` reads for
/// `nsigz > 1`. Everything else on the tape passes through.
fn run_unresr(c: &UnresrCards, units: &mut TapeUnits, _notes: &mut Vec<String>) -> Result<(), NjoyError> {
    units.openz(c.nendf, 0)?;
    units.openz(c.nin, 0)?;
    units.openz(c.nout, 1)?;
    pass_through(c.nin, c.nout, units, |m| m.unresr = true)?;
    let endf = units.get(c.nendf)?.tape.clone();
    let mut u = units.get(c.nout)?.clone();
    for m in &c.materials {
        u.tape = crate::unresr::tape::unresr_tape(&endf, &u.tape, m.mat, &m.temps, &m.sigz)?;
    }
    units.store(c.nout, u.tape, u.pointwise)?;
    units.set_meta(c.nout, u.meta);
    Ok(())
}

/// PURR (`purr.f90`). The probability tables are made where ACER needs them
/// ([`build_deck`]), from the evaluation, with PURR's random sequence from its
/// start. ~~PURR's own MT=152/153 writer is not ported, so the tape carries no
/// MT=153.~~ **CORRECTED 2026-11-12 (gh:#1498):** for one temperature, one
/// `sigma_0` and no HEATR on the input, the tape gets PURR's MT=152 and MT=153
/// ([`crate::deck::pendf::purr_tape`]; unit 34 of upstream tests 35, 36 and
/// 38-42 is identical), on the `nunx` energies card 2 asks for (`purr.f90:283-292`;
/// ~~`nunx = 0` only~~ any `nunx` since 2026-11-12, test 63). Other decks pass
/// the tape through, as before.
fn run_purr(c: &PurrCards, units: &mut TapeUnits, notes: &mut Vec<String>) -> Result<(), NjoyError> {
    units.openz(c.nendf, 0)?;
    units.openz(c.nin, 0)?;
    units.openz(c.nout, 1)?;
    if c.materials.len() != 1 {
        return Err(NjoyError::NotPorted("PURR with more than one material in one run"));
    }
    let m = &c.materials[0];
    let prev = units.get(c.nin)?.clone();
    // PURR's MT=152/153 writer (gh:#1498) covers the common deck: one
    // temperature, one `sigma_0`, the `nunx` energies PURR keeps, and no
    // HEATR on the input (`ihave = 0`, a zero heating column). Others keep the
    // pass-through and say so.
    let writable = m.temps.len() == 1 && m.sigz.len() == 1 && !prev.meta.heatr && prev.meta.blocks.len() == 1;
    if writable {
        let endf = units.get(c.nendf)?.tape.clone();
        let heating = crate::purr::PurrHeating { binary: c.nout < 0, ..Default::default() };
        let tables = crate::purr::UrrProbabilityTables::from_endf_with_heating_nunx(&endf, m.mat, m.temps[0], m.nbin as usize, m.nladr as usize, 10_000, &heating, m.nunx.max(0) as usize)?;
        if let (Some(t), Some(mf1)) = (tables, endf.section(m.mat, 1, 451)) {
            let (za, awr) = (mf1.rows[0][0], mf1.rows[0][1]);
            let tape = crate::deck::pendf::purr_tape(&prev.tape, m.mat, &t, za, awr, m.temps[0], m.sigz[0])?;
            units.store(c.nout, tape, prev.pointwise.clone())?;
            let mut meta = prev.meta.clone();
            meta.purr = Some((m.nbin as usize, m.nladr as usize, m.temps.clone()));
            meta.purr_binary = c.nout < 0;
            meta.purr_nunx = m.nunx.max(0) as usize;
            units.set_meta(c.nout, meta);
            return Ok(());
        }
    }
    notes.push("purr: MF=2/MT=153 (probability tables) not written (tape writer not ported for this deck)".into());
    pass_through(c.nin, c.nout, units, |meta| {
        meta.purr = Some((m.nbin as usize, m.nladr as usize, m.temps.clone()));
        meta.purr_binary = c.nout < 0;
        meta.purr_nunx = m.nunx.max(0) as usize;
    })
}

/// `dater` (`util.f90:72-94`): `' mm/dd/yy'` of today (UTC).
fn dater() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("  {m:02}/{d:02}/{:02}", y % 100)
}

/// ACER `iopt = 7` (`acer.f90:485-536`, `acefix`): the ACE file on `npend` is
/// read, its ZAID, comment and `(iz, aw)` list edited by cards 2-4, and written
/// to `nace` with the directory line on `ndir` ([`crate::acer::acefix`]).
/// `ngend != 0` asks for the plot file (`aplots`), written to that unit.
fn run_acer_fix(c: &AcerCards, units: &mut TapeUnits) -> Result<(), NjoyError> {
    // `itype=iopt-6` (`acer.f90:486`): for `iopt = 7` the input and output
    // files are Type 1 whatever card 2's `itype` says (decks 13 and 14 give
    // `7 1 2`); card 2's value is only echoed in the listing (`:313`). The
    // caller sends only `iopt = 7` here; Type-2 input (`iopt = 8`) has no
    // reader in this driver.
    if c.mcnpx || c.iopt - 6 != 1 {
        return Err(NjoyError::NotPorted("ACER iopt = 8 (Type-2 input) or mcnpx formats"));
    }
    let text = units
        .text(c.npend)
        .ok_or_else(|| NjoyError::InvalidInput(format!("acer: unit {} holds no ACE file", c.npend)))?
        .to_string();
    let req = crate::acer::acefix::AcefixRequest { suff: c.suff, hk: c.hk.clone(), nxtra: c.nxtra, plot: c.ngend != 0 };
    // `acer.f90:508-535`: the class letter of the ZAID picks the fix routine.
    let thermal = text.lines().next().and_then(|l| l.chars().nth(9)) == Some('t');
    let out = if thermal { crate::acer::thrfix::thrfix_type1(&text, &req)? } else { crate::acer::acefix::acefix_type1(&text, &req)? };
    if c.nace > 0 {
        units.put_text(c.nace, out.ace);
        if c.ndir != 0 {
            units.put_text(c.ndir, out.xsdir);
        }
    }
    if c.ngend != 0 {
        if let Some(p) = out.plot {
            units.put_text(c.ngend, p);
        }
    }
    Ok(())
}

/// ACER (`acer.f90`, `acefc.f90`), `iopt = 1` (fast data) with the options this
/// crate has: `newfor = 1` (or 0: the old equally probable angular bins, MF=4
/// only, gh:#1449), ~~`iopp = 1`, `ismooth = 1`~~ `iopp` and `ismooth` 0 or 1
/// (gh:#1369, upstream test 28), no thinning, ZAID rule 0,
/// Type 1 output. The table is [`build_deck`] on the PENDF's pointwise block
/// for `tempd` (`first`: the block within 1 K), HEATR and PURR as the chain ran
/// them. The ACE text goes to unit `nace`; ~~the directory unit `ndir` is not
/// written~~ **CORRECTED 2026-11-12:** the directory line goes to `ndir`
/// ([`write_directory`]).
fn run_acer(c: &AcerCards, units: &mut TapeUnits, notes: &mut Vec<String>) -> Result<(), NjoyError> {
    if c.iopt == 7 {
        return run_acer_fix(c, units);
    }
    if c.iopt == 2 {
        return crate::deck::acer_thermal::run_acer_thermal(c, units, dater());
    }
    if c.iopt == 4 || c.iopt == 5 {
        return crate::deck::acer_photon::run_acer_photon(c, units, dater());
    }
    if c.iopt != 1 {
        return Err(NjoyError::NotPorted("ACER iopt 3 or 8 from a deck (typed API exists: acer::dosimetry)"));
    }
    if c.newfor != 1 && c.newfor != 0 {
        return Err(NjoyError::InvalidInput(format!("acer: newfor = {} is not 0 or 1", c.newfor)));
    }
    if c.izaopt != 0 && c.izaopt != 1 {
        return Err(NjoyError::InvalidInput("acer: illegal izaopt.".into()));
    }
    if c.thin != [0.0; 3] || c.mcnpx || c.itype != 1 || c.nxtra != 0 {
        return Err(NjoyError::NotPorted("ACER thin/mcnpx/itype/nxtra other than the defaults"));
    }
    units.openz(c.nendf, 0)?;
    units.openz(c.npend, 0)?;
    units.openz(c.nace, 1)?;
    let endf = units.get(c.nendf)?.tape.clone();
    let pend = units.get(c.npend)?.clone();
    // A PENDF unit that no RECONR/BROADR in this run made (decks 14, 50-54, 62,
    // 71 hand ACER the raw evaluation `epn14`/..., an incident-charged-particle
    // ENDF tape with MF=6 charged-particle data, as both ENDF and PENDF) has no
    // pointwise block. Upstream's `first` reads on (`acefc.f90:288-299`) and
    // builds the charged-particle table (`izai != 1`): `acer::charged`
    // (gh:#1485, parent #1369).
    if pend.meta.blocks.is_empty() && pend.pointwise.is_none() {
        if crate::acer::charged::incident_particle(&endf, c.matd) == 1 {
            return Err(NjoyError::NotPorted("ACER from a PENDF unit no module in this run wrote (incident neutron)"));
        }
        let kt_mev = 8.617_333_262e-11 * c.tempd;
        let suffix = (c.suff * 100.0).round() as u32;
        let mut table = crate::acer::charged::build_charged(&endf, c.matd, kt_mev, suffix, c.npend > 0, c.nendf > 0)?;
        table.comment = c.hk.clone();
        table.date = dater();
        apply_acelod_isomer(&mut table, c, &endf, &pend.tape);
        // `aceout` prints XSS through `change` (`acefc.f90:13024`) for a
        // charged-particle table too; `acelcp`'s rewrite before an HPD block
        // (`acefc.f90:9376-9384`) leaves integer words the builder flagged as
        // reals (upstream test 14, 5 lines of tape31).
        let ace = crate::acer::acefix_change::type1_text(&table)?;
        write_directory(c, &ace, units, notes);
        units.put_text(c.nace, ace);
        return Ok(());
    }
    // `first`: the temperature block within `ttol = 1` K of `tempd`.
    let Some((_, res)) = pend.meta.blocks.iter().find(|(t, _)| (t - c.tempd).abs() < 1.0) else {
        return Err(NjoyError::InvalidInput("acer: desired temperature not found.".into()));
    };
    let mut deck = AceDeck::default();
    if !pend.meta.heatr {
        deck = deck.without_heatr();
    }
    deck = deck.with_heatr_partials(pend.meta.heatr_partials).with_purr_binary(pend.meta.purr_binary).with_heatr_binary(pend.meta.heatr_binary).with_newfor(c.newfor).with_iopp(c.iopp != 0).with_ismooth(c.ismooth > 0).with_pendf_binary(c.npend < 0).with_endf_binary(c.nendf < 0);
    // rows[0] is the HEAD, rows[1] the TAB1 record (NR, NP), then the
    // interpolation table (three pairs a row), then the points.
    let tab1_pairs = |mt: i32| -> Option<Vec<(f64, f64)>> {
        let sec = pend.tape.section(c.matd, 3, mt)?;
        let nr = sec.rows.get(1).map_or(0, |r| r[4] as usize);
        let np = sec.rows.get(1).map_or(0, |r| r[5] as usize);
        let start = 2 + nr.div_ceil(3);
        let mut pairs = Vec::with_capacity(np);
        'rows: for r in sec.rows.iter().skip(start) {
            for k in 0..3 {
                if pairs.len() == np {
                    break 'rows;
                }
                pairs.push((r[2 * k], r[2 * k + 1]));
            }
        }
        (pairs.len() == np).then_some(pairs)
    };
    // GASPR's MT=203-207 are reactions of the table (`unionx`).
    let gas: Vec<(i32, Vec<(f64, f64)>)> = (203..=207).filter_map(|mt| tab1_pairs(mt).map(|p| (mt, p))).collect();
    if !gas.is_empty() {
        deck = deck.with_gas_pairs(gas);
    }
    if pend.meta.heatr {
        if let Some(pairs) = tab1_pairs(301) {
            deck = deck.with_esz_kerma_pairs(pairs);
        }
        // MT=444 (damage energy) is a stored reaction in ACER (`unionx`,
        // `acefc.f90:1767`); 443 is skipped.
        if let Some(pairs) = tab1_pairs(444) {
            deck = deck.with_mt444_pairs(pairs);
        }
    }
    if let Some((nbin, nladr, temps)) = &pend.meta.purr {
        // PURR draws one random sequence over all its temperatures in order;
        // the table for the first is what a run at that temperature alone gives.
        if temps.first().is_none_or(|t| (t - c.tempd).abs() >= 1.0) {
            return Err(NjoyError::NotPorted("ACER at a PURR temperature other than PURR's first"));
        }
        deck = deck.with_purr(*nbin, *nladr, 10_000).with_purr_nunx(pend.meta.purr_nunx).with_temperature(c.tempd);
    }
    // `ngend`: the GROUPR photon-production GENDF, `gamout` (gh:#1481).
    if c.ngend != 0 {
        let text = units.text(c.ngend).ok_or_else(|| NjoyError::InvalidInput(format!("acer: ngend unit {} holds no GENDF", c.ngend)))?;
        deck = deck.with_photon_matrix(Some(crate::acer::gamout::photon_spectra(text, c.matd)?));
    }
    let kt_mev = 8.617_333_262e-11 * c.tempd;
    let suffix = (c.suff * 100.0).round() as u32;
    let mut table = build_deck(&endf, c.matd, res, kt_mev, suffix, &deck)?;
    table.comment = c.hk.clone();
    table.date = dater();
    apply_acelod_isomer(&mut table, c, &endf, &pend.tape);
    // `aceout` prints XSS through `change` (`acefc.f90:13024`), gh:#1498.
    let ace = crate::acer::acefix_change::type1_text(&table)?;
    write_directory(c, &ace, units, notes);
    units.put_text(c.nace, ace);
    Ok(())
}

/// `acelod`'s ZAID under card 2's `izaoption` (`acefc.f90:4951-4961`):
/// `izaid + suff`, or with `izaopt = 1` and a metastable target (`is > 0`)
/// `izaid + 300 + 100 is + suff`; Am-242 (`95242`) swaps the two, its ground
/// state taking `+400` and its first isomer the plain ZA.
pub(crate) fn izaopt_zaid(izaid: i32, is: i32, suff: f64, izaopt: i32) -> f64 {
    let mut zaid = f64::from(izaid) + suff;
    if izaopt == 1 && is > 0 {
        zaid = f64::from(izaid + 300 + 100 * is) + suff;
    }
    if izaopt == 1 && izaid == 95242 {
        if is == 0 {
            zaid = f64::from(izaid + 400) + suff;
        } else if is == 1 {
            zaid = f64::from(izaid) + suff;
        }
    }
    zaid
}

/// `is` as `first` reads it (`acefc.f90:304-309`): LISO, the `L2` of the
/// second record of the PENDF's MF=1/MT=451, when the ENDF tape's format
/// (`iverf`, from its first material, `:155-167`) is 5 or 6; else 0.
fn liso(endf: &Tape, pendf: &Tape, matd: i32) -> i32 {
    let first = endf.sections().iter().find(|s| s.key.mf == 1 && s.key.mt == 451);
    let iverf = first.map_or(6, |s| crate::moder::layout::iverf_from_mf1(&s.rows));
    if iverf < 5 {
        return 0;
    }
    pendf.section(matd, 1, 451).and_then(|s| s.rows.get(1)).map_or(0, |r| r[3].round() as i32)
}

/// What `acelod` takes from `first`'s `is` (LISO): `NXS(9)`, which `aceout`
/// writes as `is` (`acefc.f90:13019`) and the builder leaves 0, and, under
/// card 2's `izaopt = 1`, the `f9.2` ZAID field (the class letter after it is
/// kept).
fn apply_acelod_isomer(table: &mut crate::acer::AceTable, c: &AcerCards, endf: &Tape, pendf: &Tape) {
    let is = liso(endf, pendf, c.matd);
    table.nxs[crate::acer::nxs::S] = is;
    if c.izaopt != 1 || table.zaid.len() < 9 {
        return;
    }
    let Ok(num) = table.zaid[..9].trim().parse::<f64>() else { return };
    let izaid = (num - c.suff).round() as i32;
    let z = izaopt_zaid(izaid, is, c.suff, c.izaopt);
    table.zaid = format!("{z:9.2}{}", &table.zaid[9..]);
}

/// `aceout`'s directory line on `ndir` for the Type 1 table just written
/// (`acefc.f90:13052-13086`); the line is derived by reading the table back
/// through [`crate::acer::acefix`] with nothing to edit.
fn write_directory(c: &AcerCards, ace: &str, units: &mut TapeUnits, notes: &mut Vec<String>) {
    if c.ndir == 0 {
        return;
    }
    let keep = crate::acer::acefix::AcefixRequest { suff: -1.0, hk: String::new(), nxtra: 0, plot: false };
    match crate::acer::acefix::acefix_type1(ace, &keep) {
        Ok(out) if c.itype == 1 && !c.mcnpx => units.put_text(c.ndir, out.xsdir),
        _ => notes.push("acer: directory unit not written".into()),
    }
}

#[cfg(test)]
mod izaopt_tests {
    use super::izaopt_zaid;

    /// `acelod`'s four cases (`acefc.f90:4951-4961`), the ZAIDs upstream test
    /// 82's comments name.
    #[test]
    fn izaopt_follows_acelod() {
        assert_eq!(format!("{:9.2}", izaopt_zaid(27058, 1, 0.01, 0)), " 27058.01");
        assert_eq!(format!("{:9.2}", izaopt_zaid(27058, 1, 0.01, 1)), " 27458.01");
        assert_eq!(format!("{:9.2}", izaopt_zaid(27058, 0, 0.01, 1)), " 27058.01");
        assert_eq!(format!("{:9.2}", izaopt_zaid(95242, 0, 0.01, 1)), " 95642.01");
        assert_eq!(format!("{:9.2}", izaopt_zaid(95242, 1, 0.01, 1)), " 95242.01");
        assert_eq!(format!("{:9.2}", izaopt_zaid(95242, 1, 0.01, 0)), " 95242.01");
    }
}
