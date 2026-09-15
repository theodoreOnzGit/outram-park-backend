// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Lab-frame ENDF File-6 **LAW = 1** double-differential evaluation: `f6lab`
//! (`groupr.f90:9063-9338`).
//!
//! Unlike [`super::cm`]'s `f6ddx`/`f6cm` (LAW = 1, **CM** frame, needing the
//! CM→lab transform), `f6lab` evaluates data that is *already* in the lab
//! frame (`LCT != 2`, or `LCT = 3` with a heavy emitted particle) — so no
//! kinematic transform is needed, only interpolation **between the two
//! bracketing incident-energy subsections** (`clo` at `elo <= e`, `chi` at
//! `ehi >= e`) and, within each, between the two bracketing secondary
//! energies.
//!
//! Header check (per the crate's "read upstream first" rule): `f6lab`'s own
//! comment reads *"Retrieve the Legendre coefficients of the double
//! differential cross section at ep in the laboratory system from a part of
//! File 6 in law 1 format. Only the continuum part is returned ... Call with
//! ep=0 to initialize."* — this is the only `f6lab` in `groupr.f90`, matching
//! the lab-frame, continuum-only, initialize-with-`ep=0` data path this
//! module needs.
//!
//! # Scope
//! Ported for **`LANG = 1`** (Legendre-in-lab) only, covering both incident-
//! energy interpolation schemes NJOY dispatches on: the "base"/"corresponding
//! energy" scaling (`INT` codes 1-10 and 16-30, `groupr.f90:9151-9209`) and
//! the "true unit-base" scheme (`INT` codes 11-15, `groupr.f90:9210-9243`,
//! which additionally blends the secondary-energy grid point itself via
//! `f1`/`f2`). The tabulated-angular-distribution branch (`LANG = 11..15`,
//! `groupr.f90:9196-9243` — note this is a **different** `lang.ge.11` gate
//! than the `INT` codes above; ENDF reuses the 11-15 numeric range for both
//! flags with unrelated meanings) is **not ported**, consistent with this
//! crate's existing `f6ddx`/`f6cm` LANG scope; see [`f6lab`]'s `Err` docs.

use super::cm::Cm6Point;
use super::shared::EMAX;
use crate::endf::interp::{terp1, IntLaw};
use crate::NjoyError;

/// One bracketing incident-energy subsection for [`f6lab`]: the lab-frame
/// File-6 LAW-1 continuum secondary-energy records at a fixed incident
/// energy, same per-record layout as [`Cm6Point`] (`E'` + `LANG`-dependent
/// coefficients). Discrete lines (ENDF `ND`), if any, are **not** included
/// here — `f6lab` itself only ever returns the continuum part (see the module
/// docs); they are handled separately by the caller.
#[derive(Debug, Clone)]
pub struct Law1LabTable {
    /// Incident energy `E` \[eV\] this subsection is for.
    pub e_in: f64,
    /// Continuum secondary-energy records, ascending in `E'`.
    pub points: Vec<Cm6Point>,
}

/// Lab-frame File-6 LAW-1 double-differential Legendre coefficients at
/// secondary energy `ep` \[eV\], interpolated between the two bracketing
/// incident-energy tables `lo` (`E = elo <= e`) and `hi` (`E = ehi >= e`).
///
/// Faithful port of NJOY2016 `f6lab` (`groupr.f90:9063-9338`), `LANG = 1`
/// only (see the module docs for the `LANG = 11..15` gap). Call with
/// `ep = 0.0` first to get the initial `epnext` break point (mirrors NJOY's
/// own `ep=0` initialization convention, `groupr.f90:9101`); `term` is
/// meaningless (all-zero) on that call. Then call with ascending `ep` values;
/// each call returns the next break point to call at.
///
/// # Parameters
/// - `lo`, `hi` — the bracketing incident-energy subsections, `lo.e_in <
///   hi.e_in`.
/// - `int_code` — the raw ENDF secondary-energy-vs-incident-energy
///   interpolation code (`INT`; e.g. `22` for the "unit base lin-lin" GROUPR
///   forces onto ordinary `INT = 2` data, `groupr.f90:7666`).
/// - `lep` — the within-table secondary-energy interpolation law (ENDF `LEP`).
/// - `e` — the incident energy to interpolate to \[eV\], `elo <= e <= ehi`.
/// - `ep` — the lab secondary energy to evaluate at \[eV\], or `0.0` to
///   initialize.
/// - `nl` — number of Legendre coefficients to return (`P_0 .. P_{nl-1}`).
///
/// # Returns
/// `(term, epnext)`: `term[l]` is the lab `P_l` moment (`0.0` on the `ep = 0`
/// init call); `epnext` is the next secondary-energy break point \[eV\] (or
/// [`EMAX`] once the table is exhausted).
///
/// # Errors
/// - [`NjoyError::NotPorted`] for `lang` in `11..=15` (tabulated angular
///   distribution — `groupr.f90:9196-9243`).
/// - [`NjoyError::EndfParse`] for any other `lang != 1`, malformed input
///   (`nl == 0`, empty tables, `elo >= ehi`), or a bad interpolation law.
///
/// # Two things about the "does a predecessor record exist" guards
///
/// **1. The base they compare against (fixed 2026-09-13).** Upstream sets
/// (`groupr.f90:9119-9121`)
///
/// ```text
///   ilo = 7 + nclo*ndlo          ! the first CONTINUUM point
///   llo = ilo
///   if (clo(llo).le.zero) llo = llo + nclo   ! skip a leading E' <= 0 point
/// ```
///
/// and then guards the coefficient interpolation with `llo.ne.ilo`
/// (`groupr.f90:9214`). That asks **"does this panel have a predecessor
/// record?"**, and `ilo` is the *unskipped* first point — so when a leading
/// zero-energy point is present, `llo` already differs from `ilo` at the very
/// first panel and upstream interpolates there, using that zero point as the
/// left end.
///
/// This port previously compared against `lo_first`, the *post-skip* index
/// (upstream's initial `llo`, not its `ilo`), which is equal to `lo_ptr` on the
/// first panel and therefore **silently dropped that panel's contribution**.
/// Since [`super::cm::cm2lab`] and [`super::lab7::ll2lab`] both always emit a
/// leading `E' = 0` point, every CM-frame and LAW=7 feed lost its first
/// secondary-energy panel. The guards now compare against index `0`, which is
/// this module's `ilo`: [`Law1LabTable`] holds continuum points only (discrete
/// lines live in a separate structure), so `ndlo = 0` and upstream's
/// `ilo = 7` is our index `0`.
///
/// **2. A genuine upstream slip, still not reproduced.** The unit-base
/// (`INT` 11-15) branch guards the **low** table's contribution with
/// `if (llo.ne.ihi.and.llo.le.mlo)` (`groupr.f90:9236`) — the low table's
/// running pointer against the *high* table's initial pointer, where every
/// sibling check compares a table's pointer against its own base. That reads
/// as a copy-paste slip and is numerically inert whenever `ilo == ihi` (both
/// tables `ND = 0`, so both bases sit at offset 7), which is the overwhelmingly
/// common case. This port uses the self-consistent own-base check in that
/// branch too. Flagged for the record.
#[allow(clippy::too_many_arguments)]
pub fn f6lab(
    lo: &Law1LabTable,
    hi: &Law1LabTable,
    int_code: u32,
    lang: u32,
    lep: IntLaw,
    e: f64,
    ep: f64,
    nl: usize,
) -> Result<(Vec<f64>, f64), NjoyError> {
    /// NJOY's `up = 1.00001` (`groupr.f90:9094`).
    const UP: f64 = 1.000_01;
    /// NJOY's `dn = .99999` (`groupr.f90:9095`).
    const DN: f64 = 0.999_99;
    /// NJOY's `small = 1.e-10` (`groupr.f90:9097`).
    const SMALL: f64 = 1.0e-10;
    /// NJOY's `emin = 1.e-5` (`groupr.f90:9096`).
    const EMIN: f64 = 1.0e-5;

    if nl == 0 {
        return Err(NjoyError::EndfParse("f6lab: nl must be >= 1".into()));
    }
    if lo.points.is_empty() || hi.points.is_empty() {
        return Err(NjoyError::EndfParse(
            "f6lab: bracketing tables must have >= 1 secondary-energy point".into(),
        ));
    }
    let elo = lo.e_in;
    let ehi = hi.e_in;
    if !(elo < ehi) {
        return Err(NjoyError::EndfParse(
            "f6lab: bracketing incident energies must satisfy elo < ehi".into(),
        ));
    }
    if lang != 1 {
        if (11..=15).contains(&lang) {
            return Err(NjoyError::NotPorted(
                "groupr::f6lab tabulated LANG=11-15 coefficient interpolation \
                 (groupr.f90:9196-9243)",
            ));
        }
        return Err(NjoyError::EndfParse(format!(
            "f6lab: illegal lang = {lang} (groupr.f90:9198 `call error('f6lab','illegal lang.')`)"
        )));
    }

    // `intt`: the base ENDF interpolation law (1-5) underlying `int_code`,
    // after stripping the unit-base (+10) / corresponding-energy (+20) offset
    // (groupr.f90:9109-9111 — two independent, sequential checks, not an
    // else-if chain; replicated literally).
    let mut intt = int_code;
    if intt > 20 {
        intt = int_code - 20;
    }
    if intt > 10 {
        intt = int_code - 10;
    }
    let base_law = IntLaw::from_code(intt);
    let unit_base = (11..=15).contains(&int_code);

    // `ilo`/`ihi` (fixed, `groupr.f90:9114,9120`): the very first continuum
    // point of each table, used only for the `coeffs[0] > 0` "already
    // nonzero at the start" check below. `lo_first`/`hi_first` (Fortran
    // `llo`/`lhi` initial values, `groupr.f90:9115,9121`): skip one leading
    // zero-energy placeholder point, same convention as `f6ddx_init`
    // (`groupr.f90:8561`, ported at [`super::cm::Cm6Emission::f6ddx_init`]).
    let lo_first = if lo.points[0].ep <= 0.0 && lo.points.len() > 1 {
        1
    } else {
        0
    };
    let hi_first = if hi.points[0].ep <= 0.0 && hi.points.len() > 1 {
        1
    } else {
        0
    };
    let lo_last = lo.points.len() - 1;
    let hi_last = hi.points.len() - 1;

    // `xlo`/`xhi` (groupr.f90:9116,9122): the corresponding-energy secondary-
    // energy-max scale factors, only nonunity for `int_code > 20`.
    let mut xlo = 1.0_f64;
    let mut xhi = 1.0_f64;
    if int_code > 20 {
        xlo = lo.points[lo_last].ep;
        xhi = hi.points[hi_last].ep;
    }

    if ep == 0.0 {
        // --Initialization (groupr.f90:9101-9142).
        let xend = xlo + (e - elo) * (xhi - xlo) / (ehi - elo);
        let starts_nonzero = lo.points[0].coeffs[0] > 0.0 || hi.points[0].coeffs[0] > 0.0;
        let epnext = if starts_nonzero {
            EMIN
        } else if !unit_base {
            let mut en = lo.points[lo_first].ep / xlo;
            let en_hi = hi.points[hi_first].ep / xhi;
            if en_hi < en * (1.0 - SMALL) {
                en = en_hi;
            }
            en * xend
        } else {
            terp1(
                elo,
                lo.points[lo_first].ep,
                ehi,
                hi.points[hi_first].ep,
                e,
                IntLaw::LinLin,
            )?
        };
        return Ok((vec![0.0_f64; nl], epnext));
    }

    // --Normal entry: find the secondary-energy bracket (groupr.f90:9146-9209).
    let mut xend = 1.0_f64;
    let mut f1 = 0.0_f64;
    let mut f2 = 0.0_f64;
    let lo_ptr;
    let hi_ptr;
    let mut epnext;

    if !unit_base {
        xend = xlo + (e - elo) * (xhi - xlo) / (ehi - elo);
        let mut p = lo_first;
        epnext = loop {
            if p > lo_last {
                break EMAX;
            }
            let cand = lo.points[p].ep * xend / xlo;
            if ep < cand * (1.0 - SMALL) {
                break cand;
            }
            p += 1;
        };
        lo_ptr = p;
        let mut q = hi_first;
        let epn = loop {
            if q > hi_last {
                break EMAX;
            }
            let cand = hi.points[q].ep * xend / xhi;
            if ep < cand * (1.0 - SMALL) {
                break cand;
            }
            q += 1;
        };
        hi_ptr = q;
        if epn < epnext * (1.0 - SMALL) {
            epnext = epn;
        }
    } else {
        let mut p = lo_first;
        let mut q = hi_first;
        loop {
            epnext = terp1(
                elo,
                lo.points[p].ep,
                ehi,
                hi.points[q].ep,
                e,
                IntLaw::LinLin,
            )?;
            if ep < epnext * (1.0 - SMALL) {
                break;
            }
            if p >= lo_last {
                epnext = EMAX;
                break;
            }
            p += 1;
            if q >= hi_last {
                return Err(NjoyError::EndfParse(
                    "f6lab: unit-base (INT 11-15) interpolation needs matching-length \
                     secondary-energy grids in the two bracketing tables"
                        .into(),
                ));
            }
            q += 1;
        }
        lo_ptr = p;
        hi_ptr = q;
        if lo_ptr != 0 && lo_ptr <= lo_last {
            let eplast = terp1(
                elo,
                lo.points[lo_ptr - 1].ep,
                ehi,
                hi.points[hi_ptr - 1].ep,
                e,
                IntLaw::LinLin,
            )?;
            f1 = (epnext - ep) / (epnext - eplast);
            f2 = 1.0 - f1;
        }
    }

    // --Interpolate for coefficients, LANG=1 (groupr.f90:9151-9209).
    let mut term = vec![0.0_f64; nl];
    for l in 1..=nl {
        let mut term1 = 0.0_f64;
        let mut term2 = 0.0_f64;
        if !unit_base {
            if lo_ptr != 0 && lo_ptr <= lo_last && l <= lo.points[lo_ptr].coeffs.len() {
                term1 = terp1(
                    lo.points[lo_ptr - 1].ep,
                    lo.points[lo_ptr - 1].coeffs[l - 1],
                    lo.points[lo_ptr].ep,
                    lo.points[lo_ptr].coeffs[l - 1],
                    ep * xlo / xend,
                    lep,
                )?;
                term1 *= xlo;
            }
            if hi_ptr != 0 && hi_ptr <= hi_last && l <= hi.points[hi_ptr].coeffs.len() {
                term2 = terp1(
                    hi.points[hi_ptr - 1].ep,
                    hi.points[hi_ptr - 1].coeffs[l - 1],
                    hi.points[hi_ptr].ep,
                    hi.points[hi_ptr].coeffs[l - 1],
                    ep * xhi / xend,
                    lep,
                )?;
                term2 *= xhi;
            }
            let mut tl = terp1(elo, term1, ehi, term2, e, base_law)?;
            tl /= xend;
            term[l - 1] = tl;
        } else {
            if lo_ptr != 0 && lo_ptr <= lo_last && l <= lo.points[lo_ptr].coeffs.len() {
                let epp = f1 * lo.points[lo_ptr].ep + f2 * lo.points[lo_ptr - 1].ep;
                term1 = terp1(
                    lo.points[lo_ptr - 1].ep,
                    lo.points[lo_ptr - 1].coeffs[l - 1],
                    lo.points[lo_ptr].ep,
                    lo.points[lo_ptr].coeffs[l - 1],
                    epp,
                    lep,
                )?;
            }
            if hi_ptr != 0 && hi_ptr <= hi_last && l <= hi.points[hi_ptr].coeffs.len() {
                let epp = f1 * hi.points[hi_ptr].ep + f2 * hi.points[hi_ptr - 1].ep;
                term2 = terp1(
                    hi.points[hi_ptr - 1].ep,
                    hi.points[hi_ptr - 1].coeffs[l - 1],
                    hi.points[hi_ptr].ep,
                    hi.points[hi_ptr].coeffs[l - 1],
                    epp,
                    lep,
                )?;
            }
            term[l - 1] = terp1(elo, term1, ehi, term2, e, base_law)?;
        }
    }

    // --Finished with law 1: nudge epnext for histogram (lep==1) secondary
    //   interpolation (groupr.f90:9291-9298), same convention as `f6ddx`.
    if lep != IntLaw::Histogram {
        return Ok((term, epnext));
    }
    if (epnext - EMAX).abs() < SMALL * EMAX {
        return Ok((term, epnext));
    }
    if ep >= DN * DN * epnext {
        epnext *= UP;
    } else {
        epnext *= DN;
    }
    Ok((term, epnext))
}
