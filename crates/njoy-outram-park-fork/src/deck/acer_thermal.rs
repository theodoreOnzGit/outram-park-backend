// Translated from NJOY2016.80 `src/acer.f90` (`subroutine acer`, the `iopt = 2`
// branch, `:376-447` and `:464-467`) and `src/aceth.f90` (`acesix`, `throut`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **ACER `iopt = 2` (thermal data) as a deck module** (gh:#1369).
//!
//! `acesix` reads THERMR's PENDF (`npend`): the incident nuclide's MF=3 MT=2 for
//! the temperature block within 1 K of `tempd`, THERMR's MF=3 and MF=6 for
//! `mti` (and the elastic `mte`), and writes the thermal ACE table and the
//! directory line (`throut`, `aceth.f90:2420-2429`).
//!
//! The table itself is not translated again here. The deck driver records every
//! THERMR run on the PENDF ([`crate::deck::tapes::ThermrRun`]); this module
//! finds the run for the card's `mti` and temperature and builds the table with
//! [`AceTable::thermal_from_mf7_on_pendf`], which is gated word for word against
//! NJOY2016's table (`tests/acer_thermal_word_parity.rs`), reading THERMR's
//! tape back through the same `tpend` rounding. Card 4's `(iz, aw)` pairs reach
//! the header as `izn/awn` (`acer.f90:289-296`; card 8a then overwrites
//! `izn(1..nza)`, `:396-399`).
//!
//! Not ported (each stops the run with [`NjoyError::NotPorted`]): `nmix > 1`,
//! an ENDF-4/5 thermal tape, a THERMR run with `iinc = 1`, mcnpx formats, ACER
//! `itype != 1`.

use crate::acer::fortran_fmt::{fortran_e, fortran_f};
use crate::acer::thermal::{ace_incident_grid, ElasticRead, IncidentPendf, InelasticForm, ThermalAceOptions};
use crate::acer::AceTable;
use crate::deck::modules::AcerCards;
use crate::deck::tapes::TapeUnits;
use crate::endf::records::SectionCursor;
use crate::endf::tape::{Section, Tape};
use crate::thermr::mf7::parse_mf7_at_temperature;
use crate::NjoyError;

/// The sections of the temperature block of `matd` whose head temperature is
/// within `ttol = 1` K of `tempd` (`acesix`, `aceth.f90:112-129`), and that
/// temperature.
fn block_at(tape: &Tape, matd: i32, tempd: f64) -> Option<(&[Section], f64)> {
    let secs = tape.sections();
    let is_head = |s: &Section| s.key.mat == matd && s.key.mf == 1 && s.key.mt == 451;
    let mut i = 0;
    while i < secs.len() {
        if !is_head(&secs[i]) {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < secs.len() && secs[j].key.mat == matd && !is_head(&secs[j]) {
            j += 1;
        }
        let iverf = crate::moder::layout::iverf_from_mf1(&secs[i].rows);
        let thead = match iverf {
            4 => 1,
            5 => 2,
            _ => 3,
        };
        let t = secs[i].rows[thead][0];
        if (t - tempd).abs() < 1.0 {
            return Some((&secs[i..j], t));
        }
        i = j;
    }
    None
}

/// `thrlod`'s `iwt` -> table form (`iwt = 0` skewed bins, 1 equal, 2 tabulated).
pub(crate) fn form_of_iwt(iwt: i32) -> Result<InelasticForm, NjoyError> {
    match iwt {
        0 => Ok(InelasticForm::Skewed),
        1 => Ok(InelasticForm::Equiprobable),
        2 => Ok(InelasticForm::Continuous),
        _ => Err(NjoyError::InvalidInput(format!("acer: illegal iwt = {iwt}"))),
    }
}

/// `throut`'s directory line for a Type 1 table (`aceth.f90:2420-2429`):
/// `hz`, `aw0`, `' filename route'`, `itype`, `' 1 '`, `len2`, `lrec = 0`,
/// `nern = 0`, `tz`.
pub(crate) fn thermal_directory_line(zaid: &str, awr: f64, len2: i32, kt_mev: f64) -> String {
    format!("{:<10}{} filename route{:2} 1 {:9}{:6}{:6}{}\n", zaid, fortran_f(awr, 12, 6), 1, len2, 0, 0, fortran_e(kt_mev, 3, 10))
}

/// ACER `iopt = 2` from a deck: reads THERMR's PENDF on `npend`, writes the ACE
/// text on `nace` and the directory line on `ndir`. `date` is the header date.
///
/// # Errors
/// [`NjoyError::NotPorted`] for what the module docs list;
/// [`NjoyError::InvalidInput`] for a missing unit, THERMR run or temperature.
pub fn run_acer_thermal(c: &AcerCards, units: &mut TapeUnits, date: String) -> Result<(), NjoyError> {
    let th = c.thermal.as_ref().ok_or_else(|| NjoyError::InvalidInput("acer: thermal cards missing".into()))?;
    if c.mcnpx || c.itype != 1 {
        return Err(NjoyError::NotPorted("ACER iopt = 2 with itype != 1 or mcnpx formats"));
    }
    if th.nmix != 1 {
        return Err(NjoyError::NotPorted("ACER iopt = 2 with nmix > 1"));
    }
    if c.nace == 0 && c.iprint == 0 {
        return Ok(());
    }
    let form = form_of_iwt(th.iwt)?;
    units.openz(c.npend, 0)?;
    units.openz(c.nace, 1)?;
    let pend = units.get(c.npend)?.clone();
    let run = pend
        .meta
        .thermr
        .iter()
        .rev()
        .find(|r| r.cards.mtref == th.mti && r.cards.matdp == c.matd)
        .ok_or_else(|| NjoyError::InvalidInput(format!("acer: no THERMR run with mtref = {} on unit {}", th.mti, c.npend)))?;
    let tc = &run.cards;
    if tc.iinc != 2 {
        return Err(NjoyError::NotPorted("ACER iopt = 2 on a THERMR free-gas run"));
    }
    let endf = run
        .endf
        .as_ref()
        .ok_or_else(|| NjoyError::InvalidInput("acer: the THERMR run had no thermal ENDF tape".into()))?;
    let mf1e = endf.sections().iter().find(|s| s.key.mf == 1 && s.key.mt == 451);
    if !mf1e.is_some_and(|s| crate::moder::layout::iverf_from_mf1(&s.rows) >= 6) {
        return Err(NjoyError::NotPorted("ACER iopt = 2 on an ENDF-4/5 thermal tape (gateff)"));
    }
    // `acesix`: the PENDF block within `ttol = 1` K of `tempd`; its `aw0` and
    // MT=2 are what THERMR ran on.
    let (block, t) = block_at(&pend.tape, c.matd, c.tempd)
        .ok_or_else(|| NjoyError::InvalidInput("acer: desired temperature not found on the PENDF".into()))?;
    let awr = block[0].rows[0][1];
    let el = block
        .iter()
        .find(|s| s.key.mf == 3 && s.key.mt == 2)
        .ok_or(NjoyError::SectionNotFound { mat: c.matd, mf: 3, mt: 2 })?;
    let mut cur = SectionCursor::new(&el.rows);
    cur.read_cont()?;
    let elastic = cur.read_tab1()?;
    let incident = IncidentPendf { mat: c.matd, moderator_za: th.iza.clone(), awr, elastic: elastic.pairs };
    let mf7 = parse_mf7_at_temperature(endf, tc.matde, Some(t))?;
    let grid = ace_incident_grid(th.emax);
    let opts = ThermalAceOptions {
        n_outgoing: th.nbint as usize,
        n_cosines: tc.nbin as usize,
        emax_ev: tc.emax,
        natom: f64::from(tc.natom),
        form,
    };
    let temp = tc.temps.iter().copied().find(|&x| (x - t).abs() <= 1.0e-4 * x).unwrap_or(t);
    let suffix = (c.suff * 100.0).round() as u32;
    // `mte`, `ielas` (`aceth.f90:134-147`): which reaction `acesix` reads as the
    // coherent (Bragg) one and which as incoherent.
    let (mtec, mtei) = match (th.mte, th.ielas) {
        (0, _) => (0, 0),
        (m, 0) => (m, 0),
        (m, 2) => (m, m + 1),
        (m, _) => (0, m),
    };
    let data_coherent = mf7.coherent_elastic.is_some();
    let data_incoherent = mf7.incoherent_elastic.is_some();
    let mt_coherent = tc.mtref + 1;
    let mt_incoherent = if data_coherent { tc.mtref + 2 } else { tc.mtref + 1 };
    let elastic = if th.mte == 0 {
        ElasticRead::NoElastic
    } else if mtec != 0 && mtec == mt_coherent && !data_coherent && data_incoherent && mtei == 0 {
        ElasticRead::BraggFromIncoherentMf3
    } else if (mtec == 0 || (data_coherent && mtec == mt_coherent)) && (mtei == 0 || (data_incoherent && mtei == mt_incoherent)) {
        ElasticRead::AsTape
    } else {
        return Err(NjoyError::NotPorted("ACER iopt = 2: mte/ielas selects an elastic reaction THERMR did not write as such"));
    };
    let table = crate::acer::acesix::with_binary_units(c.npend < 0, || -> Result<AceTable, NjoyError> {
        let thermr = crate::thermr::on_pendf::thermr_tables(&mf7, temp, opts.natom, opts.n_cosines, opts.emax_ev, tc.tol, Some(&incident))?;
        AceTable::thermal_from_thermr_elastic(&thermr, th.tname.trim(), suffix, &grid, opts.n_outgoing, opts.form, elastic)
    });
    let mut table = table?;
    // `izn/awn`: card 4's pairs, then card 8a over `izn(1..nza)`.
    let mut izn = [0i32; 16];
    let mut awn = [0.0f64; 16];
    for (k, &(iz, aw)) in c.extra.iter().take(16).enumerate() {
        izn[k] = iz;
        awn[k] = aw;
    }
    let nza = th.iza.len().min(16);
    izn[..nza].fill(0);
    for (k, &za) in th.iza.iter().take(16).enumerate() {
        izn[k] = za;
    }
    table.iz = izn;
    table.aw = awn;
    table.comment = c.hk.clone();
    table.date = date;
    let ace = table.to_type1_text()?;
    if c.ndir != 0 {
        units.put_text(c.ndir, thermal_directory_line(&table.zaid, table.awr, table.nxs[0], table.kt_mev));
    }
    units.put_text(c.nace, ace);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iwt_maps_to_the_three_forms() {
        assert_eq!(form_of_iwt(0).unwrap(), InelasticForm::Skewed);
        assert_eq!(form_of_iwt(1).unwrap(), InelasticForm::Equiprobable);
        assert_eq!(form_of_iwt(2).unwrap(), InelasticForm::Continuous);
        assert!(form_of_iwt(3).is_err());
    }

    /// `throut`'s `'(a10,f12.6,'' filename route'',i2,'' 1 '',i9,2i6,1p,e10.3)'`.
    #[test]
    fn directory_line_has_throuts_layout() {
        let l = thermal_directory_line(" zrzrh.80t", 89.1324, 1090181, 2.551e-8);
        assert_eq!(l, " zrzrh.80t   89.132400 filename route 1 1   1090181     0     0 2.551E-08\n");
    }
}
