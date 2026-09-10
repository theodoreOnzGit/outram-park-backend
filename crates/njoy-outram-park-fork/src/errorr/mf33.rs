// Ported from NJOY2016 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine errorr` orchestration, l.446-469 (iverf), l.713-792 (dictionary), l.965-976
//     (gridd/lumpmt), l.1004-1009 (egngpn), l.1024-1054 (grpav/covcal), l.1056-1060 (covout).
//   - `subroutine egngpn`, l.9716-9807 (`user_group_bounds` + `uniong` call, ign=-1 promotion).
//   - `subroutine grpav` temperature search, l.8919-8942 (`pendf_temperature`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! End-to-end ERRORR for MF=33 cross-section covariances — the in-memory
//! equivalent of the `subroutine errorr` pipeline for the deck
//! `nendf npend 0 nout 0 0 / matd ign iwt iprint irelco / mprint tempin /
//! 0 33 1 1 -1 2e6 0`.
//!
//! ```text
//! scan_reactions (dictionary)  -> which MF=33 MTs, lumps, MF=32 present?
//! gridd + lumpmt               -> covariance energies, akxy, lump components
//! egngpn = groups + uniong     -> egn, un
//! grpav                        -> union-group sigma_g and flux from the PENDF
//! covcal                       -> absolute union-group covariances
//! sigc + covout                -> user-group matrices, output tape layout
//! ```
//!
//! **What this covers (verified against the NJOY2016 binary — see
//! `tests/errorr_mf33_golden.rs`):** `mfcov = 33`, `iread = 0`, `ngout = 0`
//! (group cross sections computed from the PENDF at infinite dilution),
//! `nstan = nin = 0`, no MF=32, any built-in `ign`, `iwt` 2–12 (1 with a
//! caller-supplied TAB1), `irelco` 0/1, lumped reactions (`MT` 851–870).
//!
//! **What it refuses (`NotPorted`) rather than approximating:** a material
//! with MF=32 (the `resprx`/`rescon` resonance-parameter chain, ERRORJ),
//! ENDF/B-IV tapes, ratio-to-standard NC-type records (`LTY` 1–3), MF=31/
//! 34/35/40, `iread` 1/2, GENDF input (`colaps`).

use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

use super::covcal::covcal;
use super::covout::{covout, sigc, ErrorrResult};
use super::gridd::{gridd, lumpmt, scan_reactions, uniong, NDIG};
use super::groups::neutron_group_structure;
use super::grpav::grpav;
use super::weight::{ErrorrWeight, WeightSampler};

/// `elo`/`eps` — `egn(1)` is snapped to exactly `1e-5` eV when within
/// `1e-9` (`errorr.f90:1052`, `:8917`).
const ELO: f64 = 1.0e-5;
const ELO_EPS: f64 = 1.0e-9;

/// Input for [`run_mf33`] — the subset of the ERRORR deck this path uses.
#[derive(Debug, Clone)]
pub struct Mf33Config {
    /// `matd`.
    pub matd: i32,
    /// `ign` — built-in group structure, or `1`/`-1` with `user_egn`
    /// (`-1` supplements the user grid with the covariance grid).
    pub ign: i32,
    /// Card 12b bounds for `ign = ±1`.
    pub user_egn: Option<Vec<f64>>,
    /// `iwt` selection (`egnwtf`).
    pub weight: ErrorrWeight,
    /// `tempin` \[K\] — must match the PENDF material's temperature.
    pub tempin: f64,
    /// `irelco` — `1` relative (default), `0` absolute.
    pub irelco: i32,
}

/// `iverf` of a material from its MF=1/MT=451 header
/// (`errorr.f90:456-466`).
fn endf_version(tape: &Tape, mat: i32) -> Result<i32, NjoyError> {
    let sec = tape
        .section(mat, 1, 451)
        .ok_or(NjoyError::SectionNotFound {
            mat,
            mf: 1,
            mt: 451,
        })?;
    let mut cur = SectionCursor::new(&sec.rows);
    let _head = cur.read_cont()?;
    let c = cur.read_cont()?;
    Ok(if c.n1 != 0 && c.n2 == 0 {
        4
    } else if c.n2 == 0 {
        5
    } else {
        6
    })
}

/// The temperature carried by a PENDF material's MF=1/MT=451 `hdatio`
/// record (`grpav`, `errorr.f90:8919-8931`).
fn pendf_temperature(pendf: &Tape, mat: i32, iverf: i32) -> Result<f64, NjoyError> {
    let sec = pendf
        .section(mat, 1, 451)
        .ok_or(NjoyError::SectionNotFound {
            mat,
            mf: 1,
            mt: 451,
        })?;
    let idx = (iverf - 3).max(1) as usize; // row of the hdatio head
    sec.rows
        .get(idx)
        .map(|r| r[0])
        .ok_or_else(|| NjoyError::EndfParse("errorr: PENDF MF=1/451 too short".into()))
}

/// The user group bounds `egn(1:ngn+1)` before the union (`egngpn`,
/// `errorr.f90:9766, 9786-9790`): the built-in structure or the user
/// grid, `sigfig`'d to [`NDIG`].
fn user_group_bounds(cfg: &Mf33Config) -> Result<Vec<f64>, NjoyError> {
    let raw = if cfg.ign.abs() == 1 {
        cfg.user_egn.clone().ok_or_else(|| {
            NjoyError::EndfParse("errorr: ign=+-1 needs the card-12 group bounds".into())
        })?
    } else {
        neutron_group_structure(cfg.ign)?
    };
    if raw.len() < 2 {
        return Err(NjoyError::EndfParse(
            "errorr: fewer than two group bounds".into(),
        ));
    }
    Ok(raw.iter().map(|&e| sigfig(e, NDIG, 0)).collect())
}

/// Run ERRORR for MF=33 on an ENDF tape and its PENDF.
///
/// See the module docs for the exact deck this reproduces and its scope.
///
/// # Errors
/// - [`NjoyError::NotPorted`] for MF=32 present, ENDF/B-IV, `LTY` 1–3.
/// - [`NjoyError::EndfParse`] for no MF=33 on file, a PENDF temperature
///   that does not match `tempin` (`grpav`'s "unable to find temp"), nubar
///   in the reaction list, or malformed records.
/// - [`NjoyError::SectionNotFound`] for a missing PENDF cross section.
pub fn run_mf33(endf: &Tape, pendf: &Tape, cfg: &Mf33Config) -> Result<ErrorrResult, NjoyError> {
    let matd = cfg.matd;
    let iverf = endf_version(endf, matd)?;
    if iverf <= 4 {
        return Err(NjoyError::NotPorted(
            "errorr: ENDF/B-IV covariance layout (iverf=4)",
        ));
    }
    let temp = pendf_temperature(pendf, matd, iverf)?;
    if (temp - cfg.tempin).abs() > cfg.tempin / 10_000.0 {
        return Err(NjoyError::EndfParse(format!(
            "errorr::grpav: unable to find temp={:e} on the pendf tape (found {:e})",
            cfg.tempin, temp
        )));
    }

    // dictionary + covariance energies + derivation coefficients
    let mut reactions = scan_reactions(endf, matd)?;
    if reactions.mf32_present {
        return Err(NjoyError::NotPorted(
            "errorr: MF=32 resonance-parameter covariances (resprx/rescon, ERRORJ method)",
        ));
    }
    let g = gridd(endf, matd, &mut reactions)?;
    lumpmt(endf, matd, &mut reactions)?;

    // egngpn: user grid, union grid, ign=-1 promotion
    let mut egn = user_group_bounds(cfg)?;
    let un = uniong(&egn, &g.eni)?;
    if cfg.ign == -1 {
        egn = un.clone();
    }
    if (egn[0] - ELO).abs() <= ELO_EPS {
        egn[0] = ELO;
    }

    // grpav: union-group cross sections and flux from the PENDF
    let sampler = WeightSampler::new(cfg.weight.clone(), cfg.tempin);
    let groups = grpav(pendf, matd, &reactions.iga, &un, &sampler)?;
    let flx = groups.flux_vector();

    // covcal: absolute covariances on the union grid
    let fine = covcal(endf, matd, &reactions, &groups, &flx)?;

    // za/awr from the first MF=33 section with NL > 0 (covcal, errorr.f90:1935-1938)
    let (za, awr) = endf
        .sections()
        .iter()
        .filter(|s| s.key.mat == matd && s.key.mf == 33)
        .find_map(|s| {
            let r = s.rows.first()?;
            (r[5] != 0.0).then_some((r[0], r[1]))
        })
        .unwrap_or((0.0, 0.0));

    // sigc + covout
    let coarse = sigc(&egn, &groups, &flx, &reactions);
    let blocks = covout(
        &fine, &un, &egn, &reactions, &g.derived, &coarse, cfg.irelco,
    );

    Ok(ErrorrResult {
        matd,
        za,
        awr,
        iverf,
        tempin: cfg.tempin,
        irelco: cfg.irelco,
        egn,
        un,
        reactions,
        derived: g.derived,
        coarse,
        blocks,
        messages: groups.messages,
    })
}
