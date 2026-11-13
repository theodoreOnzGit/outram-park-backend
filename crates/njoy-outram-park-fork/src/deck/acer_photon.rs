// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 (commit ab27c6414a0f) `src/acer.f90` (`:486-505`,
// the `iopt = 4/5` calls), `src/acepa.f90` (`phoout`, `:1019-1031`) and
// `src/acepn.f90` (`phnout`, `:2815-2826`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! ACER `iopt = 4` (photo-atomic) and `iopt = 5` (photo-nuclear) from a deck
//! (gh:#1369). The tables come from the existing builders
//! ([`crate::acer::photoatomic::photoatomic_ace`],
//! [`crate::acer::photonuclear::build::photonuclear_ace`]); this file only
//! binds the deck's units to them and writes the directory line.

use crate::acer::fortran_fmt::{fortran_e, fortran_f};
use crate::acer::photoatomic::{photoatomic_ace, PhotoatomicOptions};
use crate::acer::photonuclear::build::{photonuclear_ace, PhotonuclearOptions};
use crate::acer::read::AceFileType;
use crate::deck::modules::AcerCards;
use crate::deck::tapes::TapeUnits;
use crate::NjoyError;

/// The directory line `phoout`/`phnout`/`dosout` write for a Type 1 table:
/// `'(a10,f12.6,'' filename route'',i2,'' 1 '',i8,2i6,1p,e10.3)'` with
/// `itype = 1`, `lrec = nern = 0`. `aw0` and `tz` are read back from the
/// table's own header (`a10,f12.6,1x,1p,e11.4`), which printed them.
pub(crate) fn photon_directory_line(ace: &str, len2: i32) -> Result<String, NjoyError> {
    let head = ace.lines().next().unwrap_or("");
    let field = |a: usize, b: usize| -> Result<f64, NjoyError> {
        head.get(a..b)
            .and_then(|s| crate::deck::cards::parse_fortran_real(s.trim()))
            .ok_or_else(|| NjoyError::InvalidInput(format!("acer: ACE header {head:?} has no aw0/tz")))
    };
    let (aw0, tz) = (field(10, 22)?, field(23, 34)?);
    Ok(format!("{:<10}{} filename route{:2} 1 {:8}{:6}{:6}{}\n", &head[..10], fortran_f(aw0, 12, 6), 1, len2, 0, 0, fortran_e(tz, 3, 10)))
}

/// ACER `iopt = 4` or `5` on the deck's units: `acepho(nendf, npend = nlax,
/// ...)` or `acephn(nendf, npend, ...)` (`acer.f90:486-505`). The ACE text
/// goes on `nace`, the directory line on `ndir`.
///
/// # Errors
/// [`NjoyError::NotPorted`] for mcnpx or Type 2 output and for what the
/// builders refuse; a missing unit or a malformed tape otherwise.
pub fn run_acer_photon(c: &AcerCards, units: &mut TapeUnits, date: String) -> Result<(), NjoyError> {
    if c.mcnpx || c.itype != 1 {
        return Err(NjoyError::NotPorted("ACER iopt = 4/5 with itype != 1 or mcnpx formats"));
    }
    units.openz(c.nendf, 0)?;
    let endf = units.get(c.nendf)?.tape.clone();
    let raw = if c.iopt == 4 {
        // `nlax = npend`: the atomic-relaxation sublibrary, or none.
        let relax = if c.npend != 0 {
            units.openz(c.npend, 0)?;
            Some(units.get(c.npend)?.tape.clone())
        } else {
            None
        };
        let opts = PhotoatomicOptions { suffix: c.suff, comment: c.hk.clone(), date, ..Default::default() };
        photoatomic_ace(&endf, c.matd, &opts, relax.as_ref())?.into_raw(&opts, AceFileType::Type1Ascii)
    } else {
        units.openz(c.npend, 0)?;
        let pendf = units.get(c.npend)?.tape.clone();
        let opts = PhotonuclearOptions { suffix: c.suff, comment: c.hk.clone(), date, ..Default::default() };
        photonuclear_ace(&endf, &pendf, c.matd, &opts)?.into_raw(&opts, AceFileType::Type1Ascii)
    };
    let ace = raw.to_type1_string();
    if c.ndir != 0 {
        let line = photon_directory_line(&ace, raw.nxs[0])?;
        units.put_text(c.ndir, line);
    }
    units.put_text(c.nace, ace);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::photon_directory_line;

    /// The directory line's layout, `phoout` (`acepa.f90:1023`).
    #[test]
    fn directory_line_follows_phoout() {
        let ace = " 92000.31p  236.005800  0.0000E+00   11/12/26\n";
        let l = photon_directory_line(ace, 71807).unwrap();
        assert_eq!(l, " 92000.31p  236.005800 filename route 1 1    71807     0     0 0.000E+00\n");
    }
}
