// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/wimsr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - the input section of `subroutine wimsr`, l.165-268.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **WIMSR from a deck** (gh:#1369, deck 11): its cards, and the run on the GENDF a
//! previous GROUPR left.
//!
//! The cards are read in upstream's order (`wimsr.f90:165-268`) into the existing
//! [`WimsrInput`]; the run is [`crate::wimsr::run_gendf_records`] on a binary GENDF
//! unit (GROUPR's words at full precision, as NJOY reads them back), or on a coded
//! unit's text read back as records. `nout = iabs(nout)` (`:168`): the
//! library is always text.

use crate::deck::cards::{CardReader, Slot};
use crate::deck::tapes::TapeUnits;
use crate::wimsr::WimsrInput;
use crate::NjoyError;

/// `nymax` (`wimsr.f90:30`): the size of `yield`/`ifisp`.
const NYMAX: usize = 100;

/// WIMSR's cards as read.
#[derive(Debug, Clone)]
pub struct WimsrCards {
    /// Card 1 `ngendf` (signed: negative is blocked binary).
    pub ngendf: i32,
    /// Card 1 `nout`, after `iabs` (`:168`).
    pub nout: i32,
    /// Cards 2-8.
    pub input: WimsrInput,
}

fn bad(msg: &str) -> NjoyError {
    NjoyError::InvalidInput(format!("wimsr: {msg}"))
}

/// Read WIMSR's cards (`wimsr.f90:167-268`).
///
/// Upstream presets `iprint = 0, iverw = 4, igroup = 0` (card 2), `igref = 0`
/// (card 2a), `iburn = 0` (card 3) and `ip1opt = 1, inorf = isof = ifprod = jp1 = 0`
/// (card 4) before each read. The card-4 items `ntemp … mtc` and card 3's
/// `mat, rdfid` have no preset: they are module variables, zero on a first call,
/// and taken as 0 here.
///
/// # Errors
/// A malformed card; `igroup` other than 0 or 9 (upstream then leaves `ngnd`,
/// `nfg`, `nrg` unset); `ntis > nymax` (upstream's "too many time-dependent
/// isotopes").
pub fn read_wimsr(r: &mut CardReader) -> Result<WimsrCards, NjoyError> {
    let c1 = r.read_ints(&[0, 0])?;
    let (ngendf, nout) = (c1[0], c1[1].abs());
    let c2 = r.read_ints(&[0, 4, 0])?;
    let (iprint, iverw, igroup) = (c2[0], c2[1], c2[2]);
    let (ngnd, nfg, nrg, igref) = match igroup {
        0 => (69, 14, 13, 14),
        9 => {
            let c = r.read_ints(&[0, 0, 0, 0])?;
            let igref = if c[3] == 0 { c[1] } else { c[3] };
            (c[0], c[1], c[2], igref)
        }
        _ => return Err(bad("igroup other than 0 or 9 (upstream leaves ngnd unset)")),
    };
    let mut c3 = [Slot::Int(0), Slot::Int(0), Slot::Real(0.0), Slot::Int(0)];
    r.read(&mut c3)?;
    let (mat, rdfid, iburn) = (c3[0].int(), c3[2].real(), c3[3].int());
    let mut c4 = [
        Slot::Int(0),
        Slot::Int(0),
        Slot::Real(0.0),
        Slot::Int(0),
        Slot::Real(0.0),
        Slot::Int(0),
        Slot::Int(0),
        Slot::Int(1),
        Slot::Int(0),
        Slot::Int(0),
        Slot::Int(0),
        Slot::Int(0),
    ];
    r.read(&mut c4)?;
    let mut efiss = 0.0;
    let mut burn = Vec::new();
    if iburn > 0 {
        let mut c5 = [Slot::Int(0), Slot::Real(0.0)];
        r.read(&mut c5)?;
        let ntis = c5[0].int().max(0) as usize;
        efiss = c5[1].real();
        if ntis > NYMAX {
            return Err(bad("too many time-dependent isotopes"));
        }
        for _ in 0..ntis {
            let mut c6 = [Slot::Int(0), Slot::Real(0.0)];
            r.read(&mut c6)?;
            burn.push((c6[0].int(), c6[1].real()));
        }
    }
    let nrgu = nrg.max(0) as usize;
    let glam = r.read_reals(&vec![0.0; nrgu])?;
    let jp1 = c4[11].int();
    let p1flx_user = if jp1 >= 1 { r.read_reals(&vec![0.0; jp1 as usize])? } else { Vec::new() };
    let input = WimsrInput {
        iprint,
        iverw,
        ngnd: ngnd.max(0) as usize,
        nfg: nfg.max(0) as usize,
        nrg: nrgu,
        igref: igref.max(0) as usize,
        mat,
        rdfid,
        iburn,
        ntemp: c4[0].int().max(0) as usize,
        nsigz: c4[1].int().max(0) as usize,
        sgref: c4[2].real(),
        ires: c4[3].int().max(0) as usize,
        sigp: c4[4].real(),
        mti: c4[5].int(),
        mtc: c4[6].int(),
        ip1opt: c4[7].int(),
        inorf: c4[8].int(),
        isof: c4[9].int(),
        ifprod: c4[10].int(),
        p1flx_user,
        efiss,
        burn,
        glam,
    };
    Ok(WimsrCards { ngendf, nout, input })
}

/// Run WIMSR on the GENDF of unit `ngendf` and put the library on `nout`
/// (`wimsr.f90:270-283`; `wimout` only when `nout /= 0`).
///
/// # Errors
/// No GENDF on `ngendf`; those of [`crate::wimsr::run_gendf_records`].
pub fn run_wimsr(c: &WimsrCards, units: &mut TapeUnits) -> Result<(), NjoyError> {
    let g = units
        .gendf(c.ngendf)?
        .ok_or_else(|| bad(&format!("unit {} holds no gendf tape", c.ngendf)))?;
    let out = crate::wimsr::run_gendf_records(&g, &c.input)?;
    if c.nout != 0 {
        units.put_text(c.nout, out.text());
    }
    Ok(())
}
