// Translated from NJOY2016.80 `src/util.f90`: `openz`, `repoz`, `closz`.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **Tape units**: what `openz`, `repoz` and `closz` give NJOY's modules.
//!
//! In NJOY a tape is a Fortran unit. A **positive** unit number is a coded
//! (ASCII, "formatted") file and a **negative** one a blocked-binary
//! ("unformatted") file; unit `n` is the file `tape<n>`; units 10-19 are
//! scratch files that vanish on close; units 5-7 and 0 are never opened; a
//! unit above 999 is an error (`openz`, `util.f90:167-215`, the limit raised
//! from 99 in 2016.78).
//!
//! Here each unit is an **in-memory** [`Tape`], keyed by its absolute number,
//! tagged with the mode it was written in. The model is that of
//! [`crate::moder`]: no blocked-binary file is read or written, because a
//! fully in-memory pipeline never needs one. Consequences, stated:
//!
//! - **A binary unit keeps full `f64` precision; a coded unit passes through
//!   ENDF text.** That is the one thing the sign of a unit does to values.
//!   [`TapeUnits::store`] reproduces it: storing to a positive unit writes the
//!   tape as ENDF text and reads it back, as the next module would.
//! - **A mode mismatch is an error**, as it is upstream (reading a coded unit
//!   with a negative number, or the reverse).
//! - **A binary tape file on disk cannot be loaded** (needs MODER's `glstio`
//!   reader, not ported): [`NjoyError::NotPorted`].
//! - **Only units a module produced are written back** (gh:#1508). Upstream a
//!   module opens its outputs with `openz(lun,1)` (`status='new'`) and its
//!   inputs with `openz(lun,0)` (`status='old'`), and never writes an input.
//!   Here a unit becomes an output by [`TapeUnits::openz`] with `new == 1` or
//!   by [`TapeUnits::store`]; a unit only loaded from `tape<n>` on disk is an
//!   input, and [`TapeUnits::write_coded_files`] leaves its file untouched.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::endf::tape::Tape;
use crate::reconr::ReconrResult;
use crate::NjoyError;

/// What the modules run so far have done to a PENDF unit, beyond its MF=3
/// content: what ACER needs to know to build the table NJOY's chain would.
///
/// UNRESR's MF=2/MT=152 and PURR's MF=2/MT=153 are **not** written onto the
/// tape here (their tape writers are not ported); PURR's probability tables
/// are computed by ACER from the evaluation, as [`crate::acer::build_deck`]
/// does, and these flags say that the deck asked for them.
#[derive(Debug, Clone, Default)]
pub struct PendfMeta {
    /// HEATR ran on this PENDF (MT=301 is present).
    pub heatr: bool,
    /// HEATR's card 3 asked for the partial kermas MT=302, 318, 402 (flags in
    /// that order), which PURR's `rdheat` reads.
    pub heatr_partials: [bool; 3],
    /// HEATR's output unit was negative (blocked binary).
    pub heatr_binary: bool,
    /// UNRESR ran (its MT=152 is not on the tape, see above).
    pub unresr: bool,
    /// PURR ran: `(nbin, nladr, temperatures)` of its cards.
    pub purr: Option<(usize, usize, Vec<f64>)>,
    /// PURR's output unit was negative (blocked binary).
    pub purr_binary: bool,
    /// PURR card 2's `nunx` (`0`: every unresolved energy), gh:#1498.
    pub purr_nunx: usize,
    /// The pointwise result of every temperature block on the tape, in tape
    /// order: `(temperature [K], result)`.
    pub blocks: Vec<(f64, ReconrResult)>,
    /// THERMR's runs on this PENDF, in order (gh:#1369): its cards and the
    /// thermal ENDF tape it read. ACER `iopt = 2` takes THERMR's result from the
    /// PENDF it is given; here that result is rebuilt from these by
    /// [`crate::acer::AceTable::thermal_from_mf7_on_pendf`], which is gated word
    /// for word against the NJOY2016 tape read-back.
    pub thermr: Vec<ThermrRun>,
}

/// One THERMR run recorded in a [`PendfMeta`].
#[derive(Debug, Clone)]
pub struct ThermrRun {
    /// THERMR's cards.
    pub cards: crate::deck::modules::ThermrCards,
    /// The thermal ENDF tape (`nendf`), when the run had one.
    pub endf: Option<Tape>,
}

/// What a unit holds besides its tape.
#[derive(Debug, Clone)]
pub struct Unit {
    /// What the modules run so far have done to it.
    pub meta: PendfMeta,
    /// The tape's sections.
    pub tape: Tape,
    /// `true` if written through a negative unit number (blocked binary).
    pub binary: bool,
    /// For a PENDF made by RECONR or BROADR in this run: the pointwise result
    /// it carries, at full precision (what BROADR broadens), with its MAT.
    pub pointwise: Option<(i32, ReconrResult)>,
}

/// The set of open tape units of one deck run.
#[derive(Debug, Clone, Default)]
pub struct TapeUnits {
    units: BTreeMap<i32, Unit>,
    texts: BTreeMap<i32, String>,
    /// Units a module opened new or stored a tape on (its outputs), gh:#1508.
    written: BTreeSet<i32>,
    workdir: Option<PathBuf>,
}

fn unit_err(msg: String) -> NjoyError {
    NjoyError::InvalidInput(format!("tape unit: {msg}"))
}

impl TapeUnits {
    /// Units backed by files `tape<n>` in `workdir` (`None`: memory only).
    pub fn new(workdir: Option<PathBuf>) -> Self {
        Self { units: BTreeMap::new(), texts: BTreeMap::new(), written: BTreeSet::new(), workdir }
    }

    /// `openz(lun, new)`: open unit `lun`; with `new == 1` destroy any previous
    /// content. Units 0 and 5-7 are no-ops; above 999 is an error.
    ///
    /// For an existing unit (`new != 1`) that holds nothing yet, `tape<n>` is
    /// read from the work directory if present (coded units only).
    ///
    /// # Errors
    /// [`NjoyError::InvalidInput`] for an illegal unit, a missing unit file,
    /// or a unit already holding a tape of the other mode;
    /// [`NjoyError::NotPorted`] for a binary file on disk.
    pub fn openz(&mut self, lun: i32, new: i32) -> Result<(), NjoyError> {
        let nun = lun.abs();
        if (5..=7).contains(&nun) || nun == 0 {
            return Ok(());
        }
        if nun > 999 {
            return Err(unit_err("openz: illegal unit number.".into()));
        }
        if new == 1 {
            self.units.insert(
                nun,
                Unit { meta: PendfMeta::default(), tape: Tape::from_sections(String::new(), Vec::new()), binary: lun < 0, pointwise: None },
            );
            self.written.insert(nun);
            return Ok(());
        }
        if !self.units.contains_key(&nun) {
            self.load_file(lun)?;
        }
        self.check_mode(lun)
    }

    fn load_file(&mut self, lun: i32) -> Result<(), NjoyError> {
        let nun = lun.abs();
        let Some(dir) = &self.workdir else {
            return Err(unit_err(format!("unit {lun}: no tape and no work directory to read tape{nun} from")));
        };
        let path = dir.join(format!("tape{nun}"));
        if lun < 0 {
            return Err(NjoyError::NotPorted("blocked-binary tape file on disk (MODER glstio reader)"));
        }
        let tape = Tape::read_file(&path)?;
        self.units.insert(nun, Unit { meta: PendfMeta::default(), tape, binary: false, pointwise: None });
        Ok(())
    }

    fn check_mode(&self, lun: i32) -> Result<(), NjoyError> {
        match self.units.get(&lun.abs()) {
            Some(u) if u.binary != (lun < 0) => Err(unit_err(format!(
                "unit {} was written {} but is used {}",
                lun.abs(),
                if u.binary { "binary" } else { "coded" },
                if lun < 0 { "binary" } else { "coded" },
            ))),
            _ => Ok(()),
        }
    }

    /// `repoz(lun)`: rewind. In memory this only checks the mode.
    pub fn repoz(&self, lun: i32) -> Result<(), NjoyError> {
        if lun == 0 {
            return Ok(());
        }
        self.check_mode(lun)
    }

    /// `closz(lun)`: no action below unit 10; above 999 an error. Scratch
    /// units (10-19) are destroyed.
    pub fn closz(&mut self, lun: i32) -> Result<(), NjoyError> {
        let nun = lun.abs();
        if nun < 10 {
            return Ok(());
        }
        if nun > 999 {
            return Err(unit_err("closz: illegal unit number.".into()));
        }
        if (10..=19).contains(&nun) {
            self.units.remove(&nun);
            self.written.remove(&nun);
        }
        Ok(())
    }

    /// The unit's content, opened for reading as `lun` (sign checked).
    ///
    /// # Errors
    /// [`NjoyError::InvalidInput`] if nothing has been written there or the
    /// mode differs.
    pub fn get(&mut self, lun: i32) -> Result<&Unit, NjoyError> {
        let nun = lun.abs();
        if !self.units.contains_key(&nun) {
            self.load_file(lun)?;
        }
        self.check_mode(lun)?;
        self.units.get(&nun).ok_or_else(|| unit_err(format!("unit {lun} is empty")))
    }

    /// Put `tape` on unit `lun` (a module's output). A coded (positive) unit
    /// passes the tape through ENDF text, as the next reader sees it; a
    /// binary one keeps full precision.
    ///
    /// # Errors
    /// Those of [`Tape::write`] / [`Tape::read`] for a coded unit.
    pub fn store(&mut self, lun: i32, tape: Tape, pointwise: Option<(i32, ReconrResult)>) -> Result<(), NjoyError> {
        let nun = lun.abs();
        if nun == 0 {
            return Ok(());
        }
        let (tape, pointwise) = if lun > 0 {
            let mut text = Vec::new();
            tape.write(&mut text)?;
            let mut back = Tape::read(&text[..])?;
            back.copy_closers_from(&tape);
            (back, pointwise.map(|(m, r)| (m, r.through_pendf_text())))
        } else {
            (tape, pointwise)
        };
        self.units.insert(nun, Unit { meta: PendfMeta::default(), tape, binary: lun < 0, pointwise });
        self.written.insert(nun);
        Ok(())
    }

    /// Replace unit `lun`'s [`PendfMeta`] (after a [`TapeUnits::store`]).
    pub fn set_meta(&mut self, lun: i32, meta: PendfMeta) {
        if let Some(u) = self.units.get_mut(&lun.abs()) {
            u.meta = meta;
        }
    }

    /// Put a non-ENDF text file on unit `lun` (an ACE file, a directory line).
    pub fn put_text(&mut self, lun: i32, text: String) {
        self.texts.insert(lun.abs(), text);
    }

    /// The text file on unit `lun`, if one was put there.
    pub fn text(&self, lun: i32) -> Option<&str> {
        self.texts.get(&lun.abs()).map(String::as_str)
    }

    /// The text file on unit `lun`: the one put there by a module, else the
    /// work directory's `tape<n>` (an input file such as an ACER plot file).
    pub fn text_or_file(&self, lun: i32) -> Option<String> {
        if let Some(t) = self.text(lun) {
            return Some(t.to_string());
        }
        let dir = self.workdir.as_ref()?;
        std::fs::read_to_string(dir.join(format!("tape{}", lun.abs()))).ok()
    }

    /// Unit numbers currently holding a tape, ascending.
    pub fn open_units(&self) -> Vec<i32> {
        self.units.keys().copied().collect()
    }

    /// `true` if unit `lun` is a module's output in this run (opened new or
    /// stored to), `false` for an input only read from disk or an unused unit.
    pub fn is_output(&self, lun: i32) -> bool {
        self.written.contains(&lun.abs()) || self.texts.contains_key(&lun.abs())
    }

    /// Write every coded, non-scratch **output** unit (20 and above, plus 8-9)
    /// to `tape<n>` in the work directory, and every text unit. Returns the
    /// numbers written. Binary units are skipped (no blocked-binary writer),
    /// and so is a unit only read from disk: NJOY never writes a unit it
    /// opened `old` (`openz`, `util.f90:199-212`; gh:#1508).
    pub fn write_coded_files(&self) -> Result<Vec<i32>, NjoyError> {
        let Some(dir) = &self.workdir else { return Ok(Vec::new()) };
        let mut done = Vec::new();
        for (&n, u) in &self.units {
            if u.binary || (10..=19).contains(&n) || !self.written.contains(&n) {
                continue;
            }
            let mut text = Vec::new();
            u.tape.write(&mut text)?;
            std::fs::write(dir.join(format!("tape{n}")), text).map_err(NjoyError::Io)?;
            done.push(n);
        }
        for (n, t) in &self.texts {
            std::fs::write(dir.join(format!("tape{n}")), t).map_err(NjoyError::Io)?;
            done.push(*n);
        }
        Ok(done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_rules_follow_openz() {
        let mut t = TapeUnits::new(None);
        assert!(t.openz(0, 1).is_ok() && t.openz(6, 1).is_ok());
        assert!(t.openz(1000, 1).is_err());
        assert!(t.openz(999, 1).is_ok());
        t.openz(-25, 1).unwrap();
        assert!(t.repoz(-25).is_ok());
        assert!(t.repoz(25).is_err(), "mode mismatch");
        t.openz(12, 1).unwrap();
        t.closz(12).unwrap();
        assert!(!t.open_units().contains(&12), "scratch unit destroyed");
        t.closz(-25).unwrap();
        assert!(t.open_units().contains(&25), "regular unit kept");
        assert!(t.closz(1000).is_err());
    }

    /// gh:#1508: a unit read from disk is an input and is not written back;
    /// a unit opened new or stored to is an output and is.
    #[test]
    fn only_output_units_are_written_back() {
        let dir = std::env::temp_dir().join(format!("njoy-tapes-1508-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = "a tape label                                                         0 0  0\n";
        std::fs::write(dir.join("tape20"), input).unwrap();
        let mut t = TapeUnits::new(Some(dir.clone()));
        t.openz(20, 0).unwrap();
        assert!(!t.is_output(20));
        let tape = t.get(20).unwrap().tape.clone();
        t.openz(21, 1).unwrap();
        t.store(21, tape, None).unwrap();
        t.openz(22, 1).unwrap();
        t.put_text(23, "text\n".into());
        let done = t.write_coded_files().unwrap();
        assert_eq!(done, vec![21, 22, 23]);
        assert_eq!(std::fs::read_to_string(dir.join("tape20")).unwrap(), input, "input untouched");
        assert!(dir.join("tape21").exists() && dir.join("tape22").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
