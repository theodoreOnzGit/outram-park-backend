// Translated from NJOY2016.80 `src/main.f90` (the module loop) and the unit
// handling at the top of each module (`moder`, `reconr`, `broadr`, `heatr`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The deck driver: `program njoy`'s module loop, and the modules it can run.

use std::path::PathBuf;

use crate::broadr::broadn::BroadnTolerances;
use crate::broadr::joint::broadr_joint;
use crate::deck::cards::CardReader;
use crate::deck::modules::{
    read_broadr, read_gaspr, read_groupr, read_heatr, read_moder, read_reconr, read_thermr, BroadrCards, ModerCards,
    ReconrCards,
};
use crate::deck::tapes::TapeUnits;
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
            "unresr" | "purr" | "acer" | "gaminr" | "errorr" | "covr" | "dtfr" | "ccccr" | "matxsr" | "resxsr"
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
    Ok(DeckRun { units, modules, blocked, listing })
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
            units.store(*nout, u.tape, u.pointwise)
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
        let tape = pendf_for_heatr(&endf, m.mat, &res);
        sections.extend(tape.sections().iter().cloned());
        last = Some((m.mat, res));
    }
    let pointwise = if c.materials.len() == 1 { last } else { None };
    units.store(c.npend, Tape::from_sections(c.label.clone(), sections), pointwise)
}

/// A PENDF tape for `res` at `temp_k`: [`pendf_for_heatr`] with MF=1/MT=451's
/// `TEMP` (the `hdatio` record the downstream modules read) set.
fn pendf_at(endf: &Tape, mat: i32, res: &ReconrResult, temp_k: f64) -> Tape {
    let t = pendf_for_heatr(endf, mat, res);
    let secs: Vec<Section> = t
        .sections()
        .iter()
        .map(|s| {
            let mut s = s.clone();
            if s.key.mf == 1 && s.key.mt == 451 {
                if let Some(row) = s.rows.last_mut() {
                    row[0] = temp_k;
                }
            }
            s
        })
        .collect();
    Tape::from_sections(String::new(), secs)
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
    let endf = units.get(c.nendf)?.tape.clone();
    let input = units.get(c.nin)?.clone();
    let mut sections: Vec<Section> = Vec::new();
    let mut last = None;
    for m in &c.materials {
        if m.istart != 0 || m.istrap != 0 || m.temp1 != 0.0 {
            return Err(NjoyError::NotPorted("BROADR restart/bootstrap/starting temperature (cards 2)"));
        }
        if m.temps.len() != 1 {
            return Err(NjoyError::NotPorted("BROADR with more than one final temperature"));
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
        let temp = m.temps[0];
        let text_mode = c.nin > 0;
        let src = if text_mode { res.through_pendf_text() } else { res.clone() };
        let out = match broadr_joint(&src, temp, &BroadnTolerances::with_errthn(m.errthn), m.thnmx) {
            Some(b) if text_mode => b.through_pendf_text(),
            Some(b) => b,
            None => src,
        };
        sections.extend(pendf_at(&endf, m.mat, &out, temp).sections().iter().cloned());
        last = Some((m.mat, out));
    }
    let pointwise = if c.materials.len() == 1 { last } else { None };
    units.store(c.nout, Tape::from_sections(String::new(), sections), pointwise)
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
    units.store(u.nout, out.tape, pendf.pointwise)
}
