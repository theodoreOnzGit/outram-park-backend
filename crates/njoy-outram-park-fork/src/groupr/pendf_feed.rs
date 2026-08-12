// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! PENDF **smooth-file cross-section reader** — the tape-navigation half of
//! `getsig`'s initialization branch (`groupr.f90:6646-6753`).
//!
//! # What this reads
//!
//! Upstream `getsig` runs in two modes: called once with `e == 0` to *locate*
//! the reaction's cross-section record on the PENDF tape and load it into the
//! in-memory `sigma(*)` array (`groupr.f90:6669-6753`), then called repeatedly
//! with the current incident energy to *retrieve* `sigma(e)` via `gety1`
//! (`groupr.f90:6756-6801`). The retrieval half — sampling a pointwise cross
//! section and reporting the next break energy — is already ported as
//! [`crate::groupr::panel::PointwiseXs`] (`getsig`/`gtsig`'s vector reduction,
//! see that module's docs). This module is only the **locate + load** half:
//! it finds the MF=3 (or MF=13) section for a requested MAT/MT on a real
//! [`crate::endf::tape::Tape`] and decodes its TAB1 body into a
//! [`PointwiseXs::LinLin`], exactly as `getsig` fills `sigma(*)` before the
//! first `gety1` call.
//!
//! # Fortran routine -> Rust item map (`groupr.f90:6646-6753`)
//!
//! | Fortran | lines | Rust |
//! |---|---|---|
//! | MT classification (`mtd` -> `mf`,`mt`) | 6680-6711 | [`classify_mtd`] |
//! | `findf`+`contio` (locate + HEAD CONT, `awr`) | 6712-6718 | [`read_pendf_cross_section`] |
//! | `tab1io` (+ `moreio` continuation) load of `sigma(*)` | 6747 (via `gety1`'s first call) | [`read_pendf_cross_section`] (via [`crate::endf::records::SectionCursor::read_tab1`]) |
//!
//! # Scope / gaps (honest — AI draft, untrusted until reviewed)
//!
//! - **DONE:** the ordinary case — a reaction whose data lives as a single
//!   MF=3 (or MF=13, "redundant" summed cross section) TAB1 record, which
//!   covers the overwhelming majority of GROUPR reaction requests (elastic,
//!   capture, fission, discrete/continuum inelastic levels, ...).
//! - **NOT PORTED — MF=10 (photon-production yields keyed by `izap`/`lfs`,
//!   `groupr.f90:6719-6746`).** That branch reads *multiple* TAB1 records in
//!   one section and searches them by isotope/final-state identifiers. This
//!   module does not attempt the `mfd`-based File-10 dispatch at all (see
//!   [`MtdClass`]'s doc) — a caller wanting photon-production yields needs a
//!   separate reader.
//! - **NOT PORTED — MF=5 (energy-distribution "spectra", `groupr.f90:6672-6679`)
//!   and the derived non-cross-section quantities `MT=257/258/259`
//!   (average energy / lethargy / reciprocal velocity,
//!   `groupr.f90:6687-6694,6758-6772`).** These do not pull a pointwise
//!   cross-section TAB1 at all — `MT=257/258/259` are analytic functions of
//!   the incident energy computed at retrieval time, and `MF=5` needs the
//!   secondary-energy-distribution feeder ([`crate::groupr::matrix`]'s
//!   `Continuum6` gap). [`classify_mtd`] reports these as
//!   [`MtdClass::DerivedQuantity`] / a `mfd == 5` caller keeps its own
//!   `NotPorted`; this module does not attempt them.
//! - **NOT PORTED — the charged-particle elastic branch**
//!   (`mtd == 2 .and. izap > 1`, `groupr.f90:6773-6781`, `sig ≡ 1`) and the
//!   `awrp`-relative mass-ratio rescale (`groupr.f90:6718`). Both are
//!   retrieval-time special cases orthogonal to the tape read; a caller that
//!   needs them must apply the correction itself.

use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::groupr::panel::PointwiseXs;
use crate::NjoyError;
use std::sync::Arc;

/// The result of classifying a GROUPR-requested reaction number `mtd` into the
/// ENDF file (`MF`) that holds it — `getsig`'s `mf`/`mt` selection
/// (`groupr.f90:6680-6711`), for the plain File-3 request path.
///
/// Upstream `mfd` is a packed reaction-*request* code: plain MT for a File-3
/// cross section, `mfd >= 10000000/20000000/30000000` for a File 21/22/23
/// scatter-matrix request (all still read MF=3, `groupr.f90:6682-6684`), and
/// `mfd >= 40000000` for a File-10 photon-production yield request
/// (`:6685`). This function takes only the reaction number `mtd`, not `mfd`
/// — it cannot distinguish "read this MT from MF=3" from "read this MT from
/// MF=10" on `mtd` alone, since both request kinds share the same MT space.
/// Unpacking `mfd` to choose MF=10 is [`crate::groupr::input`]'s concern (the
/// caller already knows which request it is asking for); this classifier only
/// resolves the ordinary File-3/File-13 destination — see the module gap list
/// for the File-10 (photon-production) and derived-quantity cases it does not
/// attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MtdClass {
    /// An ordinary reaction cross section: read MF=3 (or MF=13 for a
    /// "redundant"/summed cross section request), MT = `mtd`
    /// (`groupr.f90:6695-6707`).
    CrossSection {
        /// The ENDF file to read (3 or 13).
        mf: i32,
        /// The ENDF reaction number (equals `mtd` for every case
        /// `getsig` handles at `:6695-6707`).
        mt: i32,
    },
    /// `MT=257/258/259` (average energy / lethargy / reciprocal velocity) —
    /// not a stored cross section at all; `getsig` computes these directly
    /// from the incident energy at retrieval time (`groupr.f90:6687-6694,
    /// 6758-6772`). **Not ported** by this feeder.
    DerivedQuantity,
}

/// Classify a GROUPR reaction number `mtd` for the smooth-file (non-matrix)
/// retrieval path — `getsig`'s `mf`/`mt` selection (`groupr.f90:6686-6711`),
/// restricted to the plain MF=3/MT=`mtd` case (this feeder does not unpack the
/// packed `mfd` matrix-request encoding — see [`MtdClass`]).
///
/// # Simplification — ENDF format version (`iverf`) gates dropped
/// Several of upstream's range checks are additionally gated on the ENDF
/// format version (`iverf.lt.6` for `MT=700-799`; `iverf.ge.6` for `MT=500`,
/// `599`, `600-849`, `850-891`, `groupr.f90:6699-6704`) — a book-keeping detail
/// of which MT numbering scheme the evaluation uses, not a physical
/// distinction. This port accepts the **union** of every version's ranges
/// rather than threading an `iverf` parameter through, so it is slightly more
/// permissive than a single-version upstream run (accepting, e.g., an
/// ENDF-5-style `MT=750` alongside ENDF-6-style `MT=650`) but never rejects an
/// `mtd` upstream would accept for *some* format version.
///
/// # Errors
/// [`NjoyError::EndfParse`] for an `mtd` outside every recognised range
/// (`groupr.f90:6708-6711`, `illegal mt`).
pub fn classify_mtd(mtd: i32) -> Result<MtdClass, NjoyError> {
    if mtd == 257 || mtd == 258 || mtd == 259 {
        return Ok(MtdClass::DerivedQuantity);
    }
    // groupr.f90:6695-6707 — the plain-MT ranges getsig recognises, all -> MT=mtd
    // on MF=3, except the aliases folded onto a different reaction (:6705-6707).
    let recognised = mtd <= 150
        || (152..=200).contains(&mtd)
        || (201..=250).contains(&mtd)
        || (300..451).contains(&mtd)
        || mtd == 500
        || (600..=849).contains(&mtd)
        || (850..=891).contains(&mtd)
        || mtd == 599
        || mtd == 251
        || mtd == 252
        || mtd == 253
        || mtd == 261
        || mtd == 452
        || mtd == 455
        || mtd == 456;
    if !recognised {
        return Err(NjoyError::EndfParse(format!(
            "getsig: {mtd} invalid in endf"
        )));
    }
    // groupr.f90:6705-6707 — mubar/nubar/chi-like aliases read off a different MT.
    let mt = match mtd {
        251 | 252 | 253 => 2,
        452 | 455 | 456 => 18,
        other => other,
    };
    Ok(MtdClass::CrossSection { mf: 3, mt })
}

/// A PENDF pointwise cross section plus its atomic-weight ratio — the tape-read
/// result of `getsig`'s initialization branch (`groupr.f90:6712-6753`).
#[derive(Debug, Clone)]
pub struct PendfCrossSection {
    /// Atomic-weight ratio `AWR = mass_target / mass_neutron` (`sigma(2)` /
    /// `c2h` of the section HEAD CONT, `groupr.f90:6717`), dimensionless.
    pub awr: f64,
    /// The pointwise cross section \[barn vs eV\], ready for
    /// [`PointwiseXs::value`]/[`PointwiseXs::next_break`].
    pub xs: PointwiseXs,
}

/// Locate + read a PENDF reaction cross section for `(mat, mtd)` — the
/// tape-navigation + load half of `getsig`'s initialization branch
/// (`groupr.f90:6669-6753`), restricted to the ordinary MF=3/MF=13
/// single-TAB1 case (see the module gap list for what is excluded).
///
/// # Mirrors `getsig`, `groupr.f90:6669-6753`
/// 1. Classify `mtd` with [`classify_mtd`] (`groupr.f90:6680-6711`).
/// 2. `findf(mat, mf, mt, tape)` + read the section HEAD CONT
///    (`groupr.f90:6712-6716`): here, a direct `(mat, mf, mt)` index lookup on
///    [`Tape`] (see [`crate::groupr::urr_pendf::read_urr_from_tape`] for the
///    same simplification applied to the URR feeder) replaces the sequential
///    `findf` tape walk. `awr = HEAD.c2` (`groupr.f90:6717`, `sigma(2)`).
/// 3. Read the TAB1 body (`groupr.f90:6747`'s implicit load, the `tab1io` a
///    real `gety1` first call performs): the `(E, sigma)` pairs and the
///    interpolation-region table.
/// 4. Require the whole TAB1 to be single-region lin-lin (`INT = 2`): every
///    PENDF pointwise cross section is linearized by RECONR before GROUPR
///    ever sees it, so this is the physically expected case, not a shortcut —
///    a multi-region or non-lin-lin TAB1 on a genuine PENDF tape would itself
///    be a malformed/unlinearized input. Returns [`NjoyError::NotPorted`] for
///    any other region layout (general multi-region `gety1` is
///    [`PointwiseXs`]'s own documented limitation, not new here).
///
/// # Errors
/// - [`NjoyError::EndfParse`] for an unrecognised `mtd` (via [`classify_mtd`])
///   or a malformed TAB1 record.
/// - [`NjoyError::SectionNotFound`] if no `(mat, mf, mt)` section exists on
///   `tape`.
/// - [`NjoyError::NotPorted`] for [`MtdClass::DerivedQuantity`]
///   (`MT=257/258/259`) or a non-lin-lin/multi-region TAB1. A File-10
///   photon-production request is out of scope for this function entirely —
///   see the module gap list; a caller wanting MF=10 must read it itself.
pub fn read_pendf_cross_section(
    tape: &Tape,
    mat: i32,
    mtd: i32,
) -> Result<PendfCrossSection, NjoyError> {
    let (mf, mt) =
        match classify_mtd(mtd)? {
            MtdClass::CrossSection { mf, mt } => (mf, mt),
            MtdClass::DerivedQuantity => return Err(NjoyError::NotPorted(
                "groupr::getsig derived quantity (MT=257/258/259, groupr.f90:6687-6694,6758-6772)",
            )),
        };

    let section = tape
        .section(mat, mf, mt)
        .ok_or(NjoyError::SectionNotFound { mat, mf, mt })?;

    let mut cur = SectionCursor::new(&section.rows);
    // Section HEAD CONT: c1 = ZA, c2 = AWR (groupr.f90:6716-6717).
    let head = cur.read_cont()?;
    let awr = head.c2;

    // TAB1 body: (E, sigma) pairs + interpolation-region table.
    let tab1 = cur.read_tab1()?;
    if tab1.interp.len() != 1 || tab1.interp[0].1 != 2 {
        return Err(NjoyError::NotPorted(
            "groupr::getsig general (non-lin-lin / multi-region) MF=3 interpolation",
        ));
    }

    Ok(PendfCrossSection {
        awr,
        xs: PointwiseXs::LinLin(Arc::new(tab1.pairs)),
    })
}
