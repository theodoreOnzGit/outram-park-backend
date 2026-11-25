// Translated from NJOY2016.80 `src/groupr.f90`: `getff` label 400 for
// `mfd = 31-36` (`:7076-7104`, `:7351-7353`), `parts` (`:7415-7525`) and the
// section head of `subroutine groupr` (`:871`, `:899`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! GROUPR's `mfd = 31-36`: charged-particle and residual production matrices
//! from MF=4 two-body data (gh:#1370, 2026-11-24).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use crate::deck::tapes::TapeUnits;
use crate::NjoyError;

use super::{GendfText, Shield};

/// `mfd = 31-36` (gh:#1370, 2026-11-24): a charged particle or residual whose
/// angular distribution is the MF=4 two-body one, which `mfchk2` (`groupr.f90:717`,
/// `:1575-1597`) routes here from a card-9 `mfd = 21-26`. `getff` sends it to label
/// 400 for MT 2, 51-90 and the MT ranges of `:7076-7101` (`parts`, then `getdis` with
/// `mft = 4`), and writes it as `MF = mfd - 10` (`:871`, `:899`), `nl = lord + 1`,
/// `ng = ngn + 1`, `nz = 1` (`init`, `:5804-5813`).
///
/// # Errors
/// `getff`'s "do not know how to handle mf,mt" for any other MT (`:7102-7104`);
/// [`NjoyError::NotPorted`] when `aprime` depends on `awr` and the MF=3 and MF=4
/// heads' AWR differ: `parts` runs on every `getff` call, and its first call, at
/// `e = 0` before `getfle` has set `awr` from MF=4, uses MF=3's (`:7352-7353`,
/// `:9713`); this port takes one `aprime`.
#[allow(clippy::too_many_arguments)]
pub(super) fn write_mf4_charged_two_body(
    g: &mut GendfText,
    sh: &mut Shield,
    inp: &crate::groupr::input::GrouprInput,
    units: &mut TapeUnits,
    pend: &crate::endf::tape::Tape,
    za: f64,
    egn: &[f64],
    emaxx: f64,
    mtd: i32,
    mfd: i32,
    iverf: i32,
) -> Result<(), NjoyError> {
    let label400 = mtd == 2
        || (51..91).contains(&mtd)
        || if iverf < 6 { (700..719).contains(&mtd) || (720..739).contains(&mtd) || (740..759).contains(&mtd) || (760..779).contains(&mtd) || (780..799).contains(&mtd) } else { (600..649).contains(&mtd) || (650..699).contains(&mtd) || (700..749).contains(&mtd) || (750..799).contains(&mtd) || (800..849).contains(&mtd) || (875..891).contains(&mtd) };
    if !label400 {
        return Err(NjoyError::InvalidInput(format!("groupr getff: do not know how to handle mf,mt: {mfd},{mtd}")));
    }
    let Some(sec) = pend.section(inp.matb, 3, mtd) else { return Ok(()) };
    let mut cur = crate::endf::records::SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    let tab = cur.read_tab1()?;
    let lrflag = tab.head.l2;
    let nl = inp.lord.max(0) as usize + 1;
    let (mut feed, first) = crate::groupr::njoy_panel::SigFeed::new(&tab, mtd);
    let nend = &units.get(inp.units.nendf)?.tape;
    let ang = crate::groupr::file4::File4Angular::from_tape(nend, inp.matb, mtd, crate::groupr::file4::NLD)?;
    let awr = getfle_mf4_awr(nend, inp.matb, mtd).unwrap_or(head.c2);
    let (aprime, law4) = crate::deck::groupr_auto::parts(mfd, mtd, awr, iverf);
    if law4 && awr != head.c2 {
        return Err(NjoyError::NotPorted("groupr mfd = 31-36: a residual whose MF=3 and MF=4 AWR differ (parts' first call uses MF=3's)"));
    }
    let mut tb = crate::groupr::two_body::TwoBodyFeed::new(ang, egn, awr, tab.head.c2, lrflag)?.with_getsig_awr(head.c2).with_outgoing_mass(aprime);
    if law4 {
        tb = tb.with_law4_cosine_flip();
    }
    let mut pf = crate::groupr::njoy_panel::PanelFeed::TwoBodyNl(tb, nl);
    let rows = sh.group(&mut feed, &mut pf, egn, first, emaxx, false, nl)?;
    if rows.is_empty() {
        return Ok(());
    }
    g.begin(inp.matb, mfd - 10, mtd);
    g.cont([za, 0.0], [nl as i32, sh.nz as i32, lrflag, (egn.len() - 1) as i32]);
    for (ig, ng2, ig2lo, data) in rows {
        g.list([inp.temperatures[0], 0.0], ng2 as i32, ig2lo, ig as i32, &data);
    }
    g.send();
    Ok(())
}

/// The AWR `getdis` uses with `mft = 4`: `getfle` sets the module `awr` to the
/// MF=4 head's C2 (`groupr.f90:9711-9713`), which can differ from the MF=3 head's
/// (Be-9 ENDF/B-VIII.0: 8.934760 against 8.934780; gh:#1370). `None` when the
/// section is absent (then `from_tape` has already said so, or it is isotropic).
pub(super) fn getfle_mf4_awr(nend: &crate::endf::tape::Tape, mat: i32, mt: i32) -> Option<f64> {
    nend.section(mat, 4, mt).and_then(|s| s.rows.first()).map(|r| r[1])
}
