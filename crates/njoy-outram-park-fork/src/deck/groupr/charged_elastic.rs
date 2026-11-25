// Translated from NJOY2016.80 `src/groupr.f90` (`ab27c64`): `getsig`'s
// charged-particle elastic branch (`:6717-6718`, `:6773-6780`), `getff` label 800
// for `izap > 1`, `mfd = 3`, `mtd = 2` (`:7063`, `:7389-7391`), `getmf6` label
// 194/500 (`:7936-7940`, `:8094-8098`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! GROUPR's `mfd = 3`, `mtd = 2` for an incident charged particle: the elastic
//! cross section with the Coulomb term (gh:#1370, 2026-11-24). The physics is in
//! [`crate::groupr::charged_elastic`] and [`crate::groupr::two_body`].

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use crate::NjoyError;

use super::{write_vector_section, GendfText, Shield};

/// Write `MF=3/MT=2` the way upstream does when the projectile is charged
/// (`izap > 1`, from MF=1/MT=451's `NSUB`), and return `true`; return `false`,
/// writing nothing, for any other reaction or a neutron.
///
/// `getsig` divides the MF=3 AWR by `awrp` (`:6718`) and then gives 1 at every
/// energy, with `enext = 1.1 e` (`:6773-6780`); `first` is still the PENDF
/// section's first energy (`gety1` at `e = 0`). `getff` goes to `getmf6`, which
/// finds the projectile's `LAW = 5` subsection and calls `getdis` with `mft = 8`.
///
/// # Errors
/// From [`crate::groupr::charged_elastic::from_mf6_mt2`] (refusals by name) and
/// the panel.
#[allow(clippy::too_many_arguments)]
pub(super) fn write_mt2(
    g: &mut GendfText,
    sh: &mut Shield,
    inp: &crate::groupr::input::GrouprInput,
    nend: &crate::endf::tape::Tape,
    egn: &[f64],
    emaxx: f64,
    za: f64,
    awr_mf3: f64,
    tab: &crate::endf::records::Tab1,
    mtd: i32,
    iverf: i32,
) -> Result<bool, NjoyError> {
    if mtd != 2 {
        return Ok(false);
    }
    let (izap, awrp) = crate::groupr::charged_elastic::projectile(nend, inp.matb, iverf);
    if izap <= 1 {
        return Ok(false);
    }
    let law5 = crate::groupr::charged_elastic::from_mf6_mt2(nend, inp.matb, izap, izap, awrp, crate::groupr::file4::NLD)?;
    let awr = if awrp != 0.0 { awr_mf3 / awrp } else { awr_mf3 };
    let lrflag = tab.head.l2;
    let tb = crate::groupr::two_body::TwoBodyFeed::new(law5.angular, egn, awr, tab.head.c2, lrflag)?
        .with_outgoing_mass(law5.aprime)
        .with_charged_elastic(law5.constants, inp.lord.max(0) as usize);
    let first = crate::endf::gety1::Gety1::new(tab).get(0.0).xnext;
    let (mut feed, _) = crate::groupr::njoy_panel::SigFeed::non_cross_section(2);
    let mut pf = crate::groupr::njoy_panel::PanelFeed::TwoBody(tb);
    let rows = sh.group(&mut feed, &mut pf, egn, first, emaxx, false, 1)?;
    write_vector_section(g, sh, inp, za, egn, mtd, lrflag, rows);
    Ok(true)
}

/// The charged projectile's elastic **matrix**, `mfd = 21-25` on `MT = 2` (`getff`
/// label 800, `getmf6` with `zad = 1001 .. 2004`, `:7587-7591`): when the projectile
/// is charged, write the section the way upstream does and return `true`; return
/// `false` for anything else. `getsig` still gives 1 on a `1.1 e` grid (`mtd = 2`,
/// `izap > 1`, `:6773`); `getdis` adds the Coulomb term and cuts at `wcut`, but goes
/// to label 410 after each band (`mfd /= 3`); `nl = lord + 1`; the section is
/// `MF = mfd` (`:820-826`). Upstream allocates `ff` for `ngn + 1` groups here, so
/// any group structure is defined (unlike `mfd = 3`).
///
/// # Errors
/// [`NjoyError::NotPorted`] for `mfd = 26` (the residual's search, `jzap > 2004`)
/// and from [`crate::groupr::charged_elastic::from_mf6_mt2`]; its `EndfParse`
/// "desired particle not found" where upstream's `getmf6` stops.
#[allow(clippy::too_many_arguments)]
pub(super) fn write_mt2_matrix(
    g: &mut GendfText,
    sh: &mut Shield,
    inp: &crate::groupr::input::GrouprInput,
    nend: &crate::endf::tape::Tape,
    pend: &crate::endf::tape::Tape,
    za: f64,
    egn: &[f64],
    emaxx: f64,
    mtd: i32,
    mfd: i32,
    iverf: i32,
) -> Result<bool, NjoyError> {
    if mtd != 2 || !(21..=26).contains(&mfd) {
        return Ok(false);
    }
    let (izap, awrp) = crate::groupr::charged_elastic::projectile(nend, inp.matb, iverf);
    if izap <= 1 {
        return Ok(false);
    }
    let zad = match mfd {
        21 => 1001,
        22 => 1002,
        23 => 1003,
        24 => 2003,
        25 => 2004,
        _ => return Err(NjoyError::NotPorted("groupr getmf6: mfd = 26 on MT = 2 for a charged projectile (the residual's LAW = 4 recoil)")),
    };
    let Some(sec) = pend.section(inp.matb, 3, 2) else { return Ok(true) };
    let mut cur = crate::endf::records::SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    let tab = cur.read_tab1()?;
    let law5 = crate::groupr::charged_elastic::from_mf6_mt2(nend, inp.matb, zad, izap, awrp, crate::groupr::file4::NLD)?;
    let awr = if awrp != 0.0 { head.c2 / awrp } else { head.c2 };
    let lrflag = tab.head.l2;
    let nl = inp.lord.max(0) as usize + 1;
    let tb = crate::groupr::two_body::TwoBodyFeed::new(law5.angular, egn, awr, tab.head.c2, lrflag)?
        .with_outgoing_mass(law5.aprime)
        .with_coulomb(law5.constants);
    let first = crate::endf::gety1::Gety1::new(&tab).get(0.0).xnext;
    let (mut feed, _) = crate::groupr::njoy_panel::SigFeed::non_cross_section(2);
    let mut pf = crate::groupr::njoy_panel::PanelFeed::TwoBodyNl(tb, nl);
    let rows = sh.group(&mut feed, &mut pf, egn, first, emaxx, false, nl)?;
    if rows.is_empty() {
        return Ok(true);
    }
    g.begin(inp.matb, mfd, mtd);
    g.cont([za, 0.0], [nl as i32, sh.nz as i32, lrflag, (egn.len() - 1) as i32]);
    for (ig, ng2, ig2lo, data) in rows {
        g.list([inp.temperatures[0], 0.0], ng2 as i32, ig2lo, ig as i32, &data);
    }
    g.send();
    Ok(true)
}
