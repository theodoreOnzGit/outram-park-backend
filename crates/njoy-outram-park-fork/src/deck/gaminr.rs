// Translated from NJOY2016.80 (commit ac5adf5) `src/gaminr.f90`: the `read(nsysi,*)`
// statements of `ruing` (`:556-563`), `genggp` (`:675-679`), `gnwtf` (`:803-805`)
// and the reaction and material cards of `gaminr` (`:258-263`, `:424`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **GAMINR from a deck** (gh:#1369): its cards, and the run on the tape units
//! ([`crate::gaminr::tape`]).
//!
//! The cards of all materials are read before the run. Upstream reads card 6
//! and card 7 between materials, but nothing on a tape changes which cards
//! follow, so the stream is the same.

use crate::deck::cards::{CardReader, Slot};
use crate::deck::tapes::TapeUnits;
use crate::gaminr::tape::{run_gaminr_tape, GaminrCards};
use crate::NjoyError;

/// Read GAMINR's cards.
///
/// # Errors
/// A malformed card.
pub fn read_gaminr(r: &mut CardReader) -> Result<GaminrCards, NjoyError> {
    let u = r.read_ints(&[0, 0, 0, 0])?;
    let c2 = r.read_ints(&[0, 0, 0, 0, 1])?;
    let mut t = [Slot::string(66, " ")];
    r.read(&mut t)?;
    let title = t[0].text().to_string();
    let (matb, igg, iwt, lord, iprint) = (c2[0], c2[1], c2[2], c2[3], c2[4]);
    let egg_read = if igg == 1 {
        let n = r.read_ints(&[0])?[0];
        let ngp = usize::try_from(n + 1).map_err(|_| NjoyError::InvalidInput("genggp: negative ngg".into()))?;
        Some(r.read_reals(&vec![0.0; ngp])?)
    } else {
        None
    };
    if iwt == 1 {
        // `read(nsysi,*) (wght(i),i=1,iwmax)`, `iwmax = 200`; the run refuses it.
        r.read_reals(&[0.0; 200])?;
    }
    let mut materials = Vec::new();
    let mut mat = matb;
    let (mut mfd, mut mtd) = (0, 0);
    loop {
        let mut list = Vec::new();
        let mut all = false;
        loop {
            let mut s = [Slot::Int(mfd), Slot::Int(mtd), Slot::string(66, " ")];
            r.read(&mut s)?;
            mfd = s[0].int();
            mtd = s[1].int();
            if mfd == -1 {
                all = true;
                break;
            }
            if mfd == 0 {
                break;
            }
            list.push((mfd, mtd));
        }
        materials.push((mat, if all { None } else { Some(list) }));
        mat = r.read_ints(&[0])?[0];
        if mat == 0 {
            break;
        }
    }
    Ok(GaminrCards { units: [u[0], u[1], u[2], u[3]], igg, iwt, lord, iprint, title, egg_read, materials })
}

/// Run GAMINR on the deck's units: ENDF `nendf`, PENDF `npend`, GAM-out on
/// `ngam2` (text, and the records at full precision for a negative unit).
///
/// # Errors
/// Those of [`run_gaminr_tape`] and of the units.
pub fn run_gaminr(c: &GaminrCards, units: &mut TapeUnits) -> Result<(), NjoyError> {
    let [nendf, npend, _, ngam2] = c.units;
    units.openz(nendf, 0)?;
    units.openz(npend, 0)?;
    units.openz(ngam2, 1)?;
    let endf = units.get(nendf)?.tape.clone();
    let pend = units.get(npend)?.tape.clone();
    let g = run_gaminr_tape(c, &endf, &pend)?;
    units.put_gendf(ngam2, g.text(), crate::errorr::gendf::GendfTape { tpid: pend.tpid.clone(), records: g.records.clone() });
    Ok(())
}
