// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/ccccr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - the card reads of `ccccr` (l.199-224), `ruinis` (l.471-503) and
//     `ruinbr` (l.1981-1986), and the unit handling of `ccccr` (l.189-275).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **CCCCR from a deck** (gh:#1369, decks 02 and 76): its cards in upstream's
//! read order, and the run on the GENDF a previous GROUPR left. The CCCC files
//! go on their units as raw bytes (gfortran unformatted records), whatever the
//! sign of the unit (`openz(-nisot,1)`, `ccccr.f90:205-207`).

use crate::ccccr::{run_ccccr, BrkoxsCards, CcccrCards, IsotopeCard, IsotxsCards, IsotxsIsotope};
use crate::deck::cards::{CardReader, Slot};
use crate::deck::tapes::TapeUnits;
use crate::NjoyError;

/// Read CCCCR's cards (`ccccr.f90:199-224`, then `ruinis` when `nisot /= 0` and
/// `ruinbr` when `nbrks /= 0`, in that order).
///
/// Presets as upstream: `nisot = nbrks = ndlay = 0`, `ivers = 0`, `huse` and
/// `hsetid` blank. The other items have no preset; they are taken as 0.
///
/// # Errors
/// A malformed card.
pub fn read_ccccr(r: &mut CardReader) -> Result<CcccrCards, NjoyError> {
    let u = r.read_ints(&[0, 0, 0, 0])?;
    let mut c2 = [Slot::Int(0), Slot::Int(0), Slot::string(12, " ")];
    r.read(&mut c2)?;
    let mut c3 = [Slot::string(72, " ")];
    r.read(&mut c3)?;
    let control = r.read_ints(&[0, 0, 0, 0, 0])?;
    let niso = control[2].max(0) as usize;
    let mut isotopes = Vec::with_capacity(niso);
    for _ in 0..niso {
        let mut c5 = [
            Slot::string(6, ""),
            Slot::string(6, ""),
            Slot::string(6, ""),
            Slot::string(6, ""),
            Slot::Int(0),
            Slot::Real(0.0),
        ];
        r.read(&mut c5)?;
        let name = |k: usize| crate::ccccr::core::text6(c5[k].text());
        isotopes.push(IsotopeCard { names: [name(0), name(1), name(2), name(3)], imat: c5[4].int(), xspo: c5[5].real() });
    }
    let isotxs = if u[1] != 0 {
        let ctl = r.read_ints(&[0, 0, 0, 0])?;
        let spec = if ctl[3] > 0 { r.read_reals(&vec![0.0; control[0].max(0) as usize])? } else { Vec::new() };
        let mut isos = Vec::with_capacity(niso);
        for _ in 0..niso {
            let mut c = [Slot::Int(0), Slot::Real(0.0), Slot::Real(0.0), Slot::Real(0.0), Slot::Real(0.0), Slot::Real(0.0), Slot::Real(0.0)];
            r.read(&mut c)?;
            isos.push(IsotxsIsotope { kbr: c[0].int(), values: [c[1].real(), c[2].real(), c[3].real(), c[4].real(), c[5].real(), c[6].real()] });
        }
        Some(IsotxsCards { control: [ctl[0], ctl[1], ctl[2], ctl[3]], spec, isotopes: isos })
    } else {
        None
    };
    let brkoxs = if u[2] != 0 {
        let c = r.read_ints(&[0, 0])?;
        let atem = if c[0] > 0 { r.read_reals(&vec![0.0; c[0] as usize])? } else { Vec::new() };
        let asig = if c[1] > 0 { r.read_reals(&vec![0.0; c[1] as usize])? } else { Vec::new() };
        Some(BrkoxsCards { nti: c[0], nzi: c[1], atem, asig })
    } else {
        None
    };
    Ok(CcccrCards {
        units: [u[0], u[1], u[2], u[3]],
        lprint: c2[0].int(),
        ivers: c2[1].int(),
        huse: match &c2[2] {
            Slot::Str { value, .. } => value.clone(),
            _ => String::new(),
        },
        hsetid: match &c3[0] {
            Slot::Str { value, .. } => value.clone(),
            _ => String::new(),
        },
        control: [control[0], control[1], control[2], control[3], control[4]],
        isotopes,
        isotxs,
        brkoxs,
    })
}

/// Run CCCCR on the GENDF of unit `nin` and put each CCCC file on its unit.
///
/// # Errors
/// No GENDF on `nin`; those of [`run_ccccr`].
pub fn run_ccccr_deck(c: &CcccrCards, units: &mut TapeUnits, notes: &mut Vec<String>) -> Result<(), NjoyError> {
    let [nin, nisot, nbrks, ndlay] = c.units;
    let g = units
        .gendf(nin)?
        .ok_or_else(|| NjoyError::InvalidInput(format!("ccccr: unit {nin} holds no gendf tape")))?;
    let out = run_ccccr(c, &g)?;
    if let Some(b) = out.isotxs {
        units.put_bytes(nisot, b);
    }
    if let Some(b) = out.brkoxs {
        units.put_bytes(nbrks, b);
    }
    if let Some(b) = out.dlayxs {
        units.put_bytes(ndlay, b);
    }
    notes.extend(out.notes);
    Ok(())
}
