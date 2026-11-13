// Ported from NJOY2016.80 `src/acepn.f90` (commit ab27c6414a0f):
//   - `subroutine phnfix`, l.1855-1959: ACER `iopt = 7` on a photo-nuclear (`u`) file;
//   - `subroutine phnout`, l.2460-2828: the Type 1 writer and directory line.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! ACER `iopt = 7` for a photo-nuclear (`u`) Type 1 ACE file (gh:#1369): read
//! it, replace the comment, and write it again with its directory line.
//!
//! `phnfix` reads the table, applies the ZAID suffix, the comment and the
//! `(iz, aw)` list, and calls `phnout`. `phnout` prints each block according to
//! the table's own layout, which [`super::photonuclear::layout::walk`] already
//! reproduces byte for byte on the read side, so the table is re-printed
//! through [`super::read::RawAceTable::to_type1_string`].
//!
//! ~~Not ported: the plot file (`phnplo`, `:2830-3573`), which
//! [`phnfix_type1`] leaves as `None` and the deck driver refuses by name;~~
//! **CORRECTED 2026-11-13 (gh:#1369):** the plot file is [`super::phnplo`],
//! written when `ngend != 0`. Not ported: a
//! ZAID suffix (`suff >= 0`); `nxtra != 0`; Type 2 and mcnpx formats.

use super::acefix::{AcefixOutput, AcefixRequest};
use super::fortran_fmt::fixed;
use crate::NjoyError;

/// `phnfix` + `phnout`, Type 1, non-mcnpx, and `phnplo` when `req.plot`
/// (`acepn.f90:1955-1956`).
///
/// # Errors
/// [`NjoyError::NotPorted`] for a ZAID suffix or `nxtra != 0`;
/// [`NjoyError::EndfParse`] when the text is not a Type 1 table.
pub fn phnfix_type1(text: &str, req: &AcefixRequest) -> Result<AcefixOutput, NjoyError> {
    if req.suff >= 0.0 {
        return Err(NjoyError::NotPorted("ACER iopt = 7 on a photo-nuclear file with a ZAID suffix"));
    }
    if req.nxtra != 0 {
        return Err(NjoyError::NotPorted("ACER iopt = 7 on a photo-nuclear file with nxtra != 0"));
    }
    let mut raw = super::read::parse_type1(text)?;
    // `if (len_trim(hk).eq.0) hk=hko` (`:1947-1949`).
    if !req.hk.trim().is_empty() {
        raw.header.raw_text[2] = fixed(&req.hk, 70).into_bytes();
        raw.header.comment = req.hk.clone();
    }
    let ace = raw.to_type1_string();
    let xsdir = crate::deck::acer_photon::photon_directory_line(&ace, raw.nxs[0])?;
    // `phnplo(nplot, hk)`: `hk` is the card's comment, or the file's when blank.
    let plot = if req.plot {
        let hk = if req.hk.trim().is_empty() { raw.header.comment.clone() } else { req.hk.clone() };
        Some(super::phnplo::phnplo(&raw, hk.trim_end()))
    } else {
        None
    };
    Ok(AcefixOutput { ace, xsdir, plot })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ZAID suffix is refused by name rather than ignored.
    #[test]
    fn a_zaid_suffix_is_refused() {
        let req = AcefixRequest { suff: 0.0, hk: String::new(), nxtra: 0, plot: false };
        let e = phnfix_type1("", &req).unwrap_err();
        assert!(format!("{e}").contains("ZAID suffix"), "{e}");
    }
}
