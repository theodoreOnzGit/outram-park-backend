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
//! - **DONE (2026-09-10) — MF=10 residual production (`mfd >= 40000000`,
//!   `groupr.f90:6719-6746`):** [`decode_extended_mfd`] decodes the card-9
//!   `4zzzaaam` form (`:684-699`) and [`read_pendf_mf10_cross_section`]
//!   walks the section's TAB1 subsections for the one with `L1 = izar`,
//!   `L2 = lfs`, taking its `C2` as `QI` and `lrflag = 0`. Golden-tested
//!   against NJOY's GENDF for U-235 `MT=4` into the ground state and the
//!   235m isomer (`tests/groupr_u235_mf10_golden.rs`); note RECONR rewrites
//!   MF=10 onto its union grid, so the PENDF's table is the one GROUPR
//!   integrates.
//! - **DONE (2026-09-10):** the derived non-cross-section quantities
//!   `MT=257/258/259` (average energy / lethargy / reciprocal velocity,
//!   `groupr.f90:6687-6694,6758-6772`) — analytic functions of the incident
//!   energy served as [`PointwiseXs::Derived`] with `getsig`'s `1.01 E`
//!   retrieval step; golden-tested against an NJOY GENDF in
//!   `tests/groupr_u238_derived_quantities_golden.rs`.
//! - **NOT PORTED — MF=5 (energy-distribution "spectra",
//!   `groupr.f90:6672-6679`)**, which needs the secondary-energy-distribution
//!   feeder ([`crate::groupr::matrix`]'s `Continuum6` gap); a `mfd == 5`
//!   caller keeps its own `NotPorted`.
//! - **NOT PORTED — the charged-particle elastic branch**
//!   (`mtd == 2 .and. izap > 1`, `groupr.f90:6773-6781`, `sig ≡ 1`) and the
//!   `awrp`-relative mass-ratio rescale (`groupr.f90:6718`). Both are
//!   retrieval-time special cases orthogonal to the tape read; a caller that
//!   needs them must apply the correction itself.

use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::groupr::panel::{DerivedQuantity, PointwiseXs};
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
    /// 6758-6772`). Served as [`PointwiseXs::Derived`] by
    /// [`read_pendf_cross_section`] (no tape read, `:6687-6694`).
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

/// `gety1`'s initialisation scan (`endf.f90`, label 100): the "first"
/// energy GROUPR's group loop compares group tops against
/// (`if (ehi.le.first) go to 580`, `groupr.f90:520`). Leading zero points
/// are skipped: `first` is the energy of the last zero point before the
/// first non-zero one, times `down = 0.999999` when that is not the first
/// point; a table starting with a non-zero value, or with a single leading
/// zero, gives `x(1)`.
pub fn gety1_first_energy(pairs: &[(f64, f64)]) -> f64 {
    const DOWN: f64 = 0.999_999;
    let np = pairs.len();
    if np == 0 {
        return 1.0e10;
    }
    let mut ip = 0usize; // 0-based
    loop {
        let (x, y) = pairs[ip];
        if y != 0.0 || ip + 1 >= np.saturating_sub(1) {
            return x;
        }
        if pairs[ip + 1].1 != 0.0 {
            return if ip > 0 { DOWN * x } else { x };
        }
        ip += 1;
    }
}

/// The residual-production request forms of GROUPR's card 9 `mfd`
/// (`groupr.f90:684-699`): `mfd = f*10000000 + izar*10 + lfs` with
/// `f = 1` (MF=3 by residual), `2` (MF=3*MF=6), `3` (MF=3*MF=9), `4`
/// (MF=10); `40000000` alone is the MF=10 fission special case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtendedMfd {
    /// The file class `f` (1..=4).
    pub file_class: i32,
    /// Residual `ZA` (`izar`; `-1` for the fission special case).
    pub izar: i32,
    /// Residual level / isomer number (`lfs`).
    pub lfs: i32,
    /// `izam` as written to the GENDF HEAD `C2` for this numeric form
    /// (`:696-698`, `:875`): `mod(mfd, 10000000) = izar*10 + lfs` (times 10
    /// when `lfs >= 10`), e.g. `922351` for U-235m; `-1` for fission. (The
    /// card-9a residual form builds `izar*1000 + lfs` instead, `:673`.)
    pub izam: i32,
}

/// Decode a card-9 `mfd >= 10000000` (`groupr.f90:684-699`); `None` for an
/// ordinary `mfd`.
pub fn decode_extended_mfd(mfd: i32) -> Option<ExtendedMfd> {
    if mfd < 10_000_000 {
        return None;
    }
    if mfd == 40_000_000 {
        return Some(ExtendedMfd {
            file_class: 4,
            izar: -1,
            lfs: 0,
            izam: -1,
        });
    }
    let file_class = mfd / 10_000_000;
    let rest = mfd - 10_000_000 * file_class;
    let izar = rest / 10;
    let lfs = rest - 10 * izar;
    let izam = if lfs < 10 { rest } else { 10 * rest };
    Some(ExtendedMfd {
        file_class,
        izar,
        lfs,
        izam,
    })
}

/// `getsig`'s MF=10 branch (`groupr.f90:6719-6746`, `mfd >= 40000000`): read
/// the MF=10 section `mt` of `mat` and select the subsection whose TAB1
/// carries `L1 = izar`, `L2 = lfs`; its `C2` is the level's `QI` and
/// `lrflag` is forced to `0` (`:6749-6750`, `mf /= 3`).
///
/// # Errors
/// [`NjoyError::SectionNotFound`] when MF=10/`mt` is absent;
/// [`NjoyError::EndfParse`] when no subsection matches (`can't find
/// mf,mt,izar,lfs`, `:6730-6733`); [`NjoyError::NotPorted`] for a non-lin-lin
/// or multi-region TAB1, as for MF=3.
pub fn read_pendf_mf10_cross_section(
    tape: &Tape,
    mat: i32,
    mt: i32,
    izar: i32,
    lfs: i32,
) -> Result<PendfCrossSection, NjoyError> {
    let section = tape
        .section(mat, 10, mt)
        .ok_or(NjoyError::SectionNotFound { mat, mf: 10, mt })?;
    let mut cur = SectionCursor::new(&section.rows);
    let head = cur.read_cont()?;
    let awr = head.c2;
    let nfs = head.n1;
    for _ in 0..nfs {
        let tab1 = cur.read_tab1()?;
        if tab1.head.l1 == izar && tab1.head.l2 == lfs {
            if tab1.interp.len() != 1 || tab1.interp[0].1 != 2 {
                return Err(NjoyError::NotPorted(
                    "groupr::getsig general (non-lin-lin / multi-region) MF=10 interpolation",
                ));
            }
            return Ok(PendfCrossSection {
                awr,
                qi: tab1.head.c2,
                lr: 0,
                xs: PointwiseXs::LinLin(Arc::new(tab1.pairs)),
            });
        }
    }
    Err(NjoyError::EndfParse(format!(
        "getsig: can't find mf,mt,izar,lfs = 10 {mt} {izar} {lfs}"
    )))
}

/// A PENDF pointwise cross section plus its atomic-weight ratio — the tape-read
/// result of `getsig`'s initialization branch (`groupr.f90:6712-6753`).
#[derive(Debug, Clone)]
pub struct PendfCrossSection {
    /// Atomic-weight ratio `AWR = mass_target / mass_neutron` (`sigma(2)` /
    /// `c2h` of the section HEAD CONT, `groupr.f90:6717`), dimensionless.
    pub awr: f64,
    /// Reaction `QI` \[eV\] — `c2` of the section's TAB1 (`q = c2h`,
    /// `groupr.f90:6748`); `0` for a derived quantity. Sets `getdis`'s
    /// threshold `(awr+1)*(-q)/awr` (`:6751`) for discrete two-body levels.
    pub qi: f64,
    /// `LR` — `l2` of the section's TAB1 (`lrflag = l2h`, `groupr.f90:6749`,
    /// forced to `0` off MF=3); selects `getdis`'s multiplicity `yld`.
    pub lr: i32,
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
/// - [`NjoyError::NotPorted`] for a non-lin-lin/multi-region TAB1. A File-10
///   photon-production request is out of scope for this function entirely —
///   see the module gap list; a caller wanting MF=10 must read it itself.
pub fn read_pendf_cross_section(
    tape: &Tape,
    mat: i32,
    mtd: i32,
) -> Result<PendfCrossSection, NjoyError> {
    let (mf, mt) = match classify_mtd(mtd)? {
        MtdClass::CrossSection { mf, mt } => (mf, mt),
        MtdClass::DerivedQuantity => {
            // `:6687-6694`: no TAB1 is read; `enext = emin`, `thresh = 0`,
            // `lrflag = 0`. Upstream leaves `awr` at whatever the previous
            // reaction set; report the material's own AWR from MF=1/451 when
            // the tape has it (0 otherwise) — informational only.
            let awr = tape
                .section(mat, 1, 451)
                .and_then(|s| s.rows.first().map(|r| r[1]))
                .unwrap_or(0.0);
            let q = DerivedQuantity::from_mt(mtd).expect("classified as derived");
            return Ok(PendfCrossSection {
                awr,
                qi: 0.0,
                lr: 0,
                xs: PointwiseXs::Derived(q),
            });
        }
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

    // `q = c2h`, `lrflag = l2h` (0 unless MF=3) (groupr.f90:6748-6750).
    let qi = tab1.head.c2;
    let lr = if mf == 3 { tab1.head.l2 } else { 0 };

    Ok(PendfCrossSection {
        awr,
        qi,
        lr,
        xs: PointwiseXs::LinLin(Arc::new(tab1.pairs)),
    })
}
