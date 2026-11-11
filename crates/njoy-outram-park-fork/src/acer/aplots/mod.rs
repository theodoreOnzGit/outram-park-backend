// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da; the 2016.80 line numbers
// after 7547 are 3 higher): `subroutine aplots` (`acefc.f90:15215-16833`) and the routines it calls.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible; this derivative file is
// distributed under AGPL-3.0-only. This is a modified, non-LANL version, not endorsed by LANL/DOE.
// See crate root LICENSE.njoy + NOTICE.

//! ACER's plot file: `aplots` (gh:#1369).
//!
//! `aplots` writes the **viewr input text** of the standard plots of an ACE
//! file: the unit `ngend` of an `iopt = 7` ACER card. The text is read back by
//! `viewr` (see [`crate::viewr`]); the plot itself is not drawn here.
//!
//! | file | upstream routine(s) |
//! |---|---|
//! | `log_pages` | `aplots`: log-log principal, resonance, heating, damage, non-threshold |
//! | `ur_pages` | `aplots`: unresolved-resonance pages |
//! | `lin_pages` | `aplots`: lin-lin principal, heating, damage, lin-log non-threshold |
//! | `thresh_pages` | `aplotr`, `aplopf` |
//! | `angular_pages` | `aplof4`, `aplonu`, `aplodn` |
//! | `emission_pages` | `aplodd`, `getl7` |
//! | `photon_pages` | `aplopp` |
//! | `particle_pages` | `aploxp` |
//! | `scale` | `ascle`, `ascll`, `mtname` |
//! | `fmt` | Fortran edit descriptors, the page header, the `xss` accessor |
//!
//! Every routine is a line-for-line translation, quirks included (see each
//! file's module doc); the output is text and is compared line by line with
//! NJOY2016's own plot tape in `tests/deck_driver_tapes_vs_njoy2016.rs`.

mod angular_pages;
mod emission_pages;
mod fmt;
mod lin_pages;
mod log_pages;
mod mtname_table;
mod particle_pages;
mod photon_pages;
pub mod scale;
mod thresh_pages;
mod ur_pages;

pub use fmt::{AceTable, Plot};

use crate::NjoyError;

/// `aplots(nout, hk)`: the viewr input text for the ACE table `a`, whose
/// comment is `hk` (page titles).
///
/// Colour pages (`ipcol = 2`, `iwcol = 3`) as upstream always writes them.
///
/// # Errors
/// `InvalidInput` where upstream calls `error` (`pltumx` or `maxe` too small)
/// or an interpolation law `terp1` refuses.
pub fn aplots(a: &AceTable, hk: &str) -> Result<String, NjoyError> {
    let hk = fmt::pad70(hk);
    let ipcol: i64 = 2;
    let iwcol: i64 = 3;
    let mut w = Plot::default();
    w.line(&format!("1 2 .30{}/", fmt::fi(ipcol, 3)));
    log_pages::principal_log(a, &mut w, &hk, iwcol);
    log_pages::resonance_total(a, &mut w, &hk, iwcol);
    log_pages::resonance_absorption(a, &mut w, &hk, iwcol);
    ur_pages::ur_pages(a, &mut w, &hk, iwcol)?;
    log_pages::heating_damage_log(a, &mut w, &hk, iwcol);
    log_pages::nonthreshold_log(a, &mut w, &hk, iwcol);
    lin_pages::principal_lin(a, &mut w, &hk, iwcol);
    lin_pages::heating_damage_lin(a, &mut w, &hk, iwcol);
    lin_pages::nonthreshold_linlog(a, &mut w, &hk, iwcol);
    thresh_pages::aplotr(a, &mut w, &hk, iwcol);
    thresh_pages::aplopf(a, &mut w, &hk, iwcol);
    angular_pages::aplof4(a, &mut w, &hk, iwcol)?;
    if a.nu > 0 {
        angular_pages::aplonu(a, &mut w, &hk, iwcol);
    }
    if a.nr != 0 {
        emission_pages::aplodd(a, &mut w, &hk, iwcol)?;
    }
    if a.ndnf > 0 {
        angular_pages::aplodn(a, &mut w, &hk, iwcol);
    }
    photon_pages::aplopp(a, &mut w, &hk, iwcol);
    if a.ntype > 0 {
        particle_pages::aploxp(a, &mut w, &hk, iwcol);
    }
    w.line("99/");
    w.line("stop");
    Ok(w.text)
}
