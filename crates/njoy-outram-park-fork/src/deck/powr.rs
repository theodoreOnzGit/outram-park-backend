// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/powr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - the card reads of `powr` (l.235-245), `fast` (l.413-426) and `therm`
//     (l.1592-1608), and the unit handling of `powr` (l.241-296).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **POWR from a deck** (gh:#1373): its cards in upstream's read order and
//! the run on the coded GENDF a previous module left. The library goes on unit
//! `|nout|` as text.
//!
//! Upstream reads each material's cards inside `fast` and `therm`, between
//! GENDF passes; reading them all first gives the same values, because no
//! card depends on the tape. Refused by name: `lib = 3` (EPRI-CPM is not
//! translated; its cards are not read, so the run stops at POWR) and a
//! blocked-binary GENDF (`ngendf < 0`: NJOY then reads full-precision binary
//! records and positions by record, which the card-by-card reader does not
//! model; convert it with MODER first).

use crate::deck::cards::{CardReader, Slot};
use crate::deck::tapes::TapeUnits;
use crate::powr::{run_powr, FastMaterial, PowrCards, ThermMaterial};
use crate::NjoyError;

/// Read POWR's cards: card 1 and 2, then for `lib = 1` cards 3-5 until a
/// `matd <= 0`, for `lib = 2` cards 3-5 until a `matd = 0`. For `lib = 3`
/// only cards 1 and 2 are read.
///
/// Presets as upstream: `iprint = iclaps = 0`; `lib = 1` card 3 `rtemp = 300`,
/// `iff = 1`, `nsgz = 0`, `izref = 1`, `matd` keeps its last value (1 at the
/// start); `hid` and `fsp` blank; `lib = 2` `idtemp = 300`, `idname` blank,
/// card 4 and 5 zero. `lib` has no preset (0).
///
/// # Errors
/// A malformed card.
pub fn read_powr(r: &mut CardReader) -> Result<PowrCards, NjoyError> {
    let c1 = r.read_ints(&[0, 0])?;
    let c2 = r.read_ints(&[0, 0, 0])?;
    let mut cards = PowrCards {
        ngendf: c1[0],
        nout: c1[1],
        lib: c2[0],
        iprint: c2[1],
        iclaps: c2[2],
        fast: Vec::new(),
        therm: Vec::new(),
    };
    if cards.lib == 1 {
        let mut matd = 1;
        while matd > 0 {
            let mut c3 = [Slot::Int(matd), Slot::Real(300.0), Slot::Int(1), Slot::Int(0), Slot::Int(1)];
            r.read(&mut c3)?;
            matd = c3[0].int();
            if matd > 0 {
                let mut hid = [Slot::string(12, " ")];
                r.read(&mut hid)?;
                let mut fsp = [Slot::string(40, " ")];
                r.read(&mut fsp)?;
                cards.fast.push(FastMaterial {
                    matd,
                    rtemp: c3[1].real(),
                    iff: c3[2].int(),
                    nsgz: c3[3].int(),
                    izref: c3[4].int(),
                    hid: hid[0].text().to_string(),
                    fsp: fsp[0].text().to_string(),
                });
            }
        }
    } else if cards.lib == 2 {
        let mut matd = 0;
        loop {
            let mut c3 = [Slot::Int(matd), Slot::Int(300), Slot::string(12, " ")];
            r.read(&mut c3)?;
            matd = c3[0].int();
            if matd == 0 {
                break;
            }
            let c4 = r.read_ints(&[0, 0, 0])?;
            let c5 = r.read_reals(&[0.0; 8])?;
            let mut z = [0.0; 8];
            z.copy_from_slice(&c5[..8]);
            cards.therm.push(ThermMaterial {
                matd,
                idtemp: c3[1].int(),
                idname: c3[2].text().to_string(),
                itrc: c4[0],
                mti: c4[1],
                mtc: c4[2],
                z,
            });
        }
    }
    Ok(cards)
}

/// Read POWR's cards and, unless `parse_only`, run it on unit `ngendf`'s
/// GENDF and put the library on unit `|nout|`.
///
/// # Errors
/// A malformed card; a blocked-binary or missing GENDF; `lib = 3`; POWR's own
/// errors ([`run_powr`]).
pub fn run_powr_deck(r: &mut CardReader, units: &mut TapeUnits, parse_only: bool) -> Result<(), NjoyError> {
    let c = read_powr(r)?;
    if parse_only {
        return Ok(());
    }
    if c.lib == 3 {
        return run_powr(&c, "").map(|_| ());
    }
    if c.ngendf < 0 {
        return Err(NjoyError::NotPorted(
            "POWR on a blocked-binary GENDF (ngendf < 0); convert it to a coded tape with MODER first",
        ));
    }
    let gendf = units
        .text_or_file(c.ngendf)
        .ok_or_else(|| NjoyError::InvalidInput(format!("powr: unit {} holds no gendf tape", c.ngendf)))?;
    let text = run_powr(&c, &gendf)?;
    units.put_text(c.nout.abs(), text);
    Ok(())
}
