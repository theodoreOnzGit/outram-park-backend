// Ported from NJOY2016 `src/aceth.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine thrfix`, l.550-637: ACER `iopt = 7` on a thermal (`t`) file;
//   - `subroutine throut`, l.2288-2429: the Type 1 writer and directory line.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! ACER `iopt = 7` for a thermal (`t`) Type 1 ACE file: read it, replace the
//! comment, write it again with the directory line, and the plot file
//! (`tplots`, [`super::tplots`]) when `ngend != 0` (gh:#1369).
//!
//! `thrfix` reads the table (`len2`, `NXS`, `JXS`, `xss`), applies the ZAID
//! suffix, the comment and the `(iz, aw)` list, and calls `throut`, which
//! writes each block through `write_integer`/`write_real_list` according to
//! the table's own layout: the same walk [`super::read::RawAceTable::derive_xss_is_int`]
//! makes for the thermal class, so the file is re-printed through it.
//!
//! Not ported (`NotPorted`): ~~a ZAID suffix (`suff >= 0`: `newsuff` on a thermal
//! name),~~ `nxtra != 0`, the Type 2 container and mcnpx formats.
//! **CORRECTED 2026-11-13 (gh:#1369):** `suff >= 0` is applied by
//! [`super::newsuff::newsuff`] (upstream test 61).

use super::acefix::{AcefixOutput, AcefixRequest};
use super::fortran_fmt::fixed;
use super::newsuff::newsuff;
use crate::deck::acer_thermal::thermal_directory_line;
use crate::NjoyError;

/// `thrfix` + `throut`, Type 1, non-mcnpx.
///
/// # Errors
/// [`NjoyError::NotPorted`] for the options listed in the module docs;
/// [`NjoyError::EndfParse`] when the text is not a Type 1 thermal table.
pub fn thrfix_type1(text: &str, req: &AcefixRequest) -> Result<AcefixOutput, NjoyError> {
    if req.nxtra != 0 {
        return Err(NjoyError::NotPorted("ACER iopt = 7 on a thermal file with nxtra != 0"));
    }
    let mut raw = super::read::parse_type1(text)?;
    // `if (suff.ge. 0._kr) call newsuff(mcnpx,suff,hz)` (`aceth.f90:618`).
    if req.suff >= 0.0 {
        let hz = newsuff(false, req.suff, &String::from_utf8_lossy(&raw.header.raw_text[0]));
        raw.header.zaid = hz.trim().to_string();
        raw.header.raw_text[0] = hz.into_bytes();
    }
    // `if (len_trim(hk).eq.0) hk=hko`.
    if !req.hk.trim().is_empty() {
        raw.header.raw_text[2] = fixed(&req.hk, 70).into_bytes();
        raw.header.comment = req.hk.clone();
    }
    let ace = raw.to_type1_string();
    let len2 = raw.nxs[0];
    let zaid = String::from_utf8_lossy(&raw.header.raw_text[0]).to_string();
    let xsdir = thermal_directory_line(&zaid, raw.header.awr, len2, raw.header.kt_mev);
    let plot = if req.plot {
        let hk = if req.hk.trim().is_empty() { raw.header.comment.clone() } else { req.hk.clone() };
        Some(super::tplots::tplots(&raw, hk.trim_end())?)
    } else {
        None
    };
    Ok(AcefixOutput { ace, xsdir, plot })
}
