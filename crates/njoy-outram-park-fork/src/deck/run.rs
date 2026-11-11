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
    read_unresr, AcerCards, BroadrCards, ModerCards, PurrCards, ReconrCards, UnresrCards,
};
use crate::deck::tapes::{PendfMeta, TapeUnits};
use crate::endf::tape::{Section, Tape};
use crate::heatr::{heatr, pendf_for_heatr, HeatrInput};
use crate::moder::{select_materials, MaterialSelection};
use crate::reconr::{reconr, ReconrConfig, ReconrResult};
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
                read_thermr(&mut r)?;
                Err(NjoyError::NotPorted(
                    "thermr tape driver (thermr.f90 `thermr`: calcem/coh/iel written to a PENDF tape; \
                     only the ACER-facing tables exist, thermr::on_pendf)",
                ))
            }
            "groupr" => {
                read_groupr(&mut r)?;
                Err(NjoyError::NotPorted(
                    "groupr tape driver (groupr.f90 `groupr`: end-to-end GENDF from a PENDF tape)",
                ))
            }
            "gaspr" => {
                read_gaspr(&mut r)?;
                Err(NjoyError::NotPorted("gaspr tape driver (typed API: gaspr::GasProduction)"))
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
            "gaminr" | "errorr" | "covr" | "dtfr" | "ccccr" | "matxsr" | "resxsr"
            | "powr" | "wimsr" | "plotr" | "viewr" | "mixr" | "leapr" => {
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

/// MODER (`moder.f90`): mode change or material selection.
fn run_moder(c: &ModerCards, units: &mut TapeUnits) -> Result<(), NjoyError> {
    match c {
        ModerCards::Convert { nin, nout } => {
            units.openz(*nout, 1)?;
            if nin.abs() >= 20 {
                units.openz(*nin, 0)?;
            }
            let u = units.get(*nin)?.clone();
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
        if errmax != 10.0 * m.err || errint != m.err / 20000.0 {
            return Err(NjoyError::NotPorted("RECONR with non-default errmax/errint (card 3)"));
        }
        if !m.enode.is_empty() {
            return Err(NjoyError::NotPorted("RECONR user grid energies enode (card 6)"));
        }
        let res = reconr(&endf, &ReconrConfig { mat: m.mat, tolerance: m.err, temperature: m.tempr })?;
        if c.materials.len() == 1 {
            let tape = crate::deck::pendf::reconr_tape(&endf, m.mat, &res, &c.label, &m.cards, m.err, m.tempr)?;
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
        if m.thnmx != 6.5e6 || m.errmax != 0.0 || m.errint != 0.0 {
            return Err(NjoyError::NotPorted("BROADR non-default thnmx/errmax/errint (card 3)"));
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
            let tol = BroadnTolerances::with_errthn(m.errthn);
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

/// UNRESR (`unresr.f90`): **cards read, tape passed through.** UNRESR's
/// self-shielded MF=2/MT=152 (`unresr.f90`'s `unresr`, `unresl`, the table
/// writer) is not written onto the PENDF; ACER does not read it.
fn run_unresr(c: &UnresrCards, units: &mut TapeUnits, notes: &mut Vec<String>) -> Result<(), NjoyError> {
    units.openz(c.nendf, 0)?;
    units.openz(c.nin, 0)?;
    units.openz(c.nout, 1)?;
    notes.push("unresr: MF=2/MT=152 (self-shielded unresolved table) not written (tape writer not ported)".into());
    pass_through(c.nin, c.nout, units, |m| m.unresr = true)
}

/// PURR (`purr.f90`): **cards read, tape passed through.** The probability
/// tables are made where ACER needs them ([`build_deck`]), from the
/// evaluation, with PURR's random sequence from its start; PURR's own MT=153
/// writer is not ported, so the tape carries no MT=153.
fn run_purr(c: &PurrCards, units: &mut TapeUnits, notes: &mut Vec<String>) -> Result<(), NjoyError> {
    units.openz(c.nendf, 0)?;
    units.openz(c.nin, 0)?;
    units.openz(c.nout, 1)?;
    if c.materials.len() != 1 {
        return Err(NjoyError::NotPorted("PURR with more than one material in one run"));
    }
    let m = &c.materials[0];
    notes.push("purr: MF=2/MT=153 (probability tables) not written (tape writer not ported)".into());
    pass_through(c.nin, c.nout, units, |meta| {
        meta.purr = Some((m.nbin as usize, m.nladr as usize, m.temps.clone()));
        meta.purr_binary = c.nout < 0;
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

/// ACER (`acer.f90`, `acefc.f90`), `iopt = 1` (fast data) with the options this
/// crate has: `newfor = 1` (or 0: the old equally probable angular bins, MF=4
/// only, gh:#1449), `iopp = 1`, `ismooth = 1`, no thinning, ZAID rule 0,
/// Type 1 output. The table is [`build_deck`] on the PENDF's pointwise block
/// for `tempd` (`first`: the block within 1 K), HEATR and PURR as the chain ran
/// them. The ACE text goes to unit `nace`; the directory unit `ndir` is not
/// written.
fn run_acer(c: &AcerCards, units: &mut TapeUnits, notes: &mut Vec<String>) -> Result<(), NjoyError> {
    if c.iopt != 1 {
        return Err(NjoyError::NotPorted("ACER iopt other than 1 from a deck (typed APIs exist: acer::thermal, dosimetry, photoatomic, photonuclear)"));
    }
    if c.newfor != 1 && c.newfor != 0 {
        return Err(NjoyError::InvalidInput(format!("acer: newfor = {} is not 0 or 1", c.newfor)));
    }
    if c.iopp != 1 || c.ismooth != 1 || c.thin != [0.0; 3] || c.izaopt != 0 || c.mcnpx || c.itype != 1 || c.nxtra != 0 {
        return Err(NjoyError::NotPorted("ACER iopp/ismooth/thin/izaopt/mcnpx/itype/nxtra other than the defaults"));
    }
    units.openz(c.nendf, 0)?;
    units.openz(c.npend, 0)?;
    units.openz(c.nace, 1)?;
    let endf = units.get(c.nendf)?.tape.clone();
    let pend = units.get(c.npend)?.clone();
    // `first`: the temperature block within `ttol = 1` K of `tempd`.
    let Some((_, res)) = pend.meta.blocks.iter().find(|(t, _)| (t - c.tempd).abs() < 1.0) else {
        return Err(NjoyError::InvalidInput("acer: desired temperature not found.".into()));
    };
    let mut deck = AceDeck::default();
    if !pend.meta.heatr {
        deck = deck.without_heatr();
    }
    deck = deck.with_heatr_partials(pend.meta.heatr_partials).with_purr_binary(pend.meta.purr_binary).with_heatr_binary(pend.meta.heatr_binary).with_newfor(c.newfor);
    if let Some((nbin, nladr, temps)) = &pend.meta.purr {
        // PURR draws one random sequence over all its temperatures in order;
        // the table for the first is what a run at that temperature alone gives.
        if temps.first().is_none_or(|t| (t - c.tempd).abs() >= 1.0) {
            return Err(NjoyError::NotPorted("ACER at a PURR temperature other than PURR's first"));
        }
        deck = deck.with_purr(*nbin, *nladr, 10_000).with_temperature(c.tempd);
    }
    let kt_mev = 8.617_333_262e-11 * c.tempd;
    let suffix = (c.suff * 100.0).round() as u32;
    let mut table = build_deck(&endf, c.matd, res, kt_mev, suffix, &deck)?;
    table.comment = c.hk.clone();
    table.date = dater();
    units.put_text(c.nace, table.to_type1_text()?);
    if c.ndir != 0 {
        notes.push("acer: directory unit not written".into());
    }
    Ok(())
}
