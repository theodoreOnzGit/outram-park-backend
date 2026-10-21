// SPDX-License-Identifier: GPL-3.0

//! **Reading the ACE photon-production blocks: MTRP, LSIGP, SIGP, LANDP, ANDP,
//! LDLWP, DLWP** — GitHub #307 item 5.
//!
//! [`super::photon_blocks`] **writes** these blocks, ported from the photon half
//! of `acelpp` (`acefc.f90:8214-9014`). Nothing read them back, which left the
//! crate asymmetric in the one direction that matters for a reader of somebody
//! else's library: a table NJOY or MCNP produced carries photon production, and
//! this port could say what it had written but not what it had been given.
//!
//! # Upstream is the specification
//!
//! Ported from `openmc/data/reaction.py::_get_photon_products_ace` (OpenMC
//! `afa7a14`), which walks the same seven blocks:
//!
//! | block | JXS (1-based) | holds |
//! |---|---|---|
//! | MTRP | 13 | `MT*1000 + k` per photon subsection |
//! | LSIGP | 14 | locator into SIGP |
//! | SIGP | 15 | `MFTYPE`, then a yield (12/16) or a cross section (13) |
//! | LANDP | 16 | locator into ANDP, `0` meaning isotropic in the laboratory |
//! | ANDP | 17 | the photon cosine distributions |
//! | LDLWP | 18 | locator into DLWP |
//! | DLWP | 19 | the photon energy laws — LAW=2 (a line) or LAW=4 (a spectrum) |
//!
//! `NXS(7) = NTRP` is the entry count. **An entry is one photon subsection, not
//! one reaction**: U-235 ENDF/B-VIII.0 has 583 of them across 44 reactions, so
//! `MTRP / 1000` is the neutron MT and the remainder is which subsection of it.
//!
//! # Two things that would be easy to get wrong
//!
//! - **`MFTYPE` decides what the numbers mean.** `13` is a production *cross
//!   section* on the table's own energy grid, from `IE` (a 1-based ESZ index);
//!   `12` and `16` are a *yield* to be multiplied by the `MTMULT` reaction's
//!   cross section. Treating a yield as a cross section understates production
//!   by the size of the cross section itself, and reads as a plausible small
//!   number rather than as an error.
//! - **The energy laws reuse the DLW reader.** DLWP has the identical
//!   `[LNW, LAW, IDAT]` + applicability layout, so
//!   [`super::ce_laws::decode_law_chain`] reads it against `JXS(19)` — which is
//!   why ACE LAW=2 (a discrete line) is decoded there rather than refused. Same
//!   for ANDP through [`super::ce_laws::decode_angular_block`]. One reader per
//!   format, not one per block.
//!
//! # Scope: this is data, not transport
//!
//! `outram-mc-libs` transports **neutrons only** — stated in its own
//! `tally::filter` docs — so nothing consumes this yet, and
//! `Nuclide::from_ace` deliberately does not carry it (see that function's
//! scope note). Decoding it here is the data crate's job; a photon transport
//! kernel is a separate piece of work and would find the data already read.

use crate::acer::ce_laws::{decode_angular_block, decode_law_chain, AceEnergyLaw};
use crate::acer::ce_decode::EV_PER_MEV;
use crate::acer::read::RawAceTable;
use crate::acer::{jxs, nxs};
use crate::error::NjoyError;

/// How a photon-production entry states its production rate.
#[derive(Debug, Clone, PartialEq)]
pub enum AcePhotonRate {
    /// `MFTYPE = 12` (from ENDF MF=12) or `16` (from a photon subsection of
    /// MF=6): photons **per reaction** of MT [`mtmult`](Self::Yield::mtmult), to
    /// be multiplied by that reaction's own cross section.
    Yield {
        /// `12` or `16`, kept because ACE distinguishes them even though a
        /// consumer treats both the same way.
        mftype: i32,
        /// The MT whose cross section this yield multiplies.
        mtmult: i32,
        /// Incident energy grid \[eV\], ascending.
        energy: Vec<f64>,
        /// Yield aligned with [`energy`](Self::Yield::energy).
        y: Vec<f64>,
    },
    /// `MFTYPE = 13`: a production **cross section** \[barn\] on the table's own
    /// energy grid, starting at `threshold_index` (0-based into the ESZ grid).
    Xs {
        /// 0-based index into the table's energy grid where `xs` starts. ACE
        /// stores `IE` 1-based; it is converted here so it indexes a Rust slice.
        threshold_index: usize,
        /// Cross section \[barn\], one value per grid point from
        /// `threshold_index`.
        xs: Vec<f64>,
    },
}

/// One photon-production entry: one subsection of one neutron reaction.
#[derive(Debug, Clone)]
pub struct AcePhotonEntry {
    /// `MT*1000 + k` exactly as MTRP stores it.
    pub mtrp: i32,
    /// The neutron MT that produces these photons (`mtrp / 1000`).
    pub neutron_mt: i32,
    /// Which subsection of that MT this is (`mtrp % 1000`, 1-based).
    pub subsection: i32,
    /// The production rate and how to read it.
    pub rate: AcePhotonRate,
    /// The photon's energy law: LAW=2 for a discrete line, LAW=4 for a spectrum.
    pub law: AceEnergyLaw,
    /// The photon's cosine distribution, or `None` when LANDP says isotropic in
    /// the **laboratory** — which is a statement by the evaluation, not a
    /// missing table (`reaction.py:684-690` substitutes a uniform law there).
    pub angular: Option<crate::acer::angular::ElasticAngular>,
}

/// Decode every photon-production entry in a continuous-energy ACE table.
///
/// `Ok(vec![])` when `NXS(7) = 0`, which is a legal table meaning "no photon
/// production" — every nuclide whose evaluation has no MF=12/13/15, and every
/// table written with `iopt` photon output suppressed. That is not an error and
/// not the same as "not decoded".
///
/// # Errors
///
/// A locator that runs past `XSS`, or an `MFTYPE` outside `{12, 13, 16}` —
/// upstream raises on the same set (`reaction.py:670`).
pub fn decode_photon_production(t: &RawAceTable) -> Result<Vec<AcePhotonEntry>, NjoyError> {
    let ntrp = t.nxs[nxs::NTRP].max(0) as usize;
    if ntrp == 0 {
        return Ok(Vec::new());
    }
    let mtrp_at = locator(t, jxs::MTRP, "MTRP")?;
    let lsigp_at = locator(t, jxs::LSIGP, "LSIGP")?;
    let sigp = t.jxs[jxs::SIGP];
    if sigp <= 0 {
        return Err(NjoyError::EndfParse(
            "ACE table declares NXS(7) photon entries but has no SIGP block".into(),
        ));
    }
    let ldlwp = t.jxs[jxs::LDLWP];
    let dlwp = t.jxs[jxs::DLWP];
    if ldlwp <= 0 || dlwp <= 0 {
        return Err(NjoyError::EndfParse(
            "ACE table declares NXS(7) photon entries but has no LDLWP/DLWP block".into(),
        ));
    }

    let mut out = Vec::with_capacity(ntrp);
    for i in 0..ntrp {
        need(t, mtrp_at + i, 1, "MTRP")?;
        let mtrp = t.xss[mtrp_at + i] as i32;
        need(t, lsigp_at + i, 1, "LSIGP")?;
        let loca = t.xss[lsigp_at + i] as i64;
        let rate = read_rate(t, (sigp - 1) as usize + (loca as usize) - 1)?;

        need(t, (ldlwp - 1) as usize + i, 1, "LDLWP")?;
        let loc = t.xss[(ldlwp - 1) as usize + i] as i64;
        let law = decode_law_chain(t, (dlwp - 1) as usize, loc)?;

        // LANDP/ANDP, read by the neutron block's own reader. A photon
        // distribution is tabulated in the laboratory, so `lct = 1`.
        let angular =
            decode_angular_block(t, t.jxs[jxs::LANDP], t.jxs[jxs::ANDP], i, 1)?;

        out.push(AcePhotonEntry {
            mtrp,
            neutron_mt: mtrp / 1000,
            subsection: mtrp % 1000,
            rate,
            law,
            angular,
        });
    }
    Ok(out)
}

/// The photon-production yield or cross section at `at`, the 0-based first word
/// of a SIGP entry.
fn read_rate(t: &RawAceTable, at: usize) -> Result<AcePhotonRate, NjoyError> {
    need(t, at, 1, "SIGP MFTYPE")?;
    let mftype = t.xss[at] as i32;
    match mftype {
        12 | 16 => {
            need(t, at + 1, 1, "SIGP MTMULT")?;
            let mtmult = t.xss[at + 1] as i32;
            let (energy, y, _) =
                crate::acer::ce_laws::read_ace_tab1(t, at + 2, "SIGP photon yield")?;
            Ok(AcePhotonRate::Yield {
                mftype,
                mtmult,
                energy,
                y,
            })
        }
        13 => {
            need(t, at + 1, 2, "SIGP IE/NE")?;
            // IE is 1-based into the table's energy grid.
            let ie = t.xss[at + 1] as i64;
            if ie < 1 {
                return Err(NjoyError::EndfParse(format!(
                    "SIGP MFTYPE=13 has IE = {ie}, but ACE stores it 1-based"
                )));
            }
            let n = t.xss[at + 2] as usize;
            need(t, at + 3, n, "SIGP photon production cross section")?;
            Ok(AcePhotonRate::Xs {
                threshold_index: (ie - 1) as usize,
                xs: t.xss[at + 3..at + 3 + n].to_vec(),
            })
        }
        other => Err(NjoyError::EndfParse(format!(
            "SIGP MFTYPE = {other}; upstream accepts only 12, 13 and 16 \
             (openmc/data/reaction.py:670)"
        ))),
    }
}

/// A `JXS` locator as a 0-based `XSS` index, or an error naming the block.
fn locator(t: &RawAceTable, which: usize, name: &str) -> Result<usize, NjoyError> {
    let v = t.jxs[which];
    if v <= 0 {
        return Err(NjoyError::EndfParse(format!(
            "ACE table declares NXS(7) photon entries but has no {name} block"
        )));
    }
    Ok((v - 1) as usize)
}

fn need(t: &RawAceTable, at: usize, n: usize, what: &str) -> Result<(), NjoyError> {
    if at + n > t.xss.len() {
        return Err(NjoyError::EndfParse(format!(
            "ACE {what}: words {at}..{} run past XSS ({})",
            at + n,
            t.xss.len()
        )));
    }
    Ok(())
}

/// Total photon production \[barn\] from every `MFTYPE = 13` entry at grid index
/// `k`, for a reader that wants the aggregate rather than the subsections.
///
/// **Yield entries are excluded and the count of them is returned**, because a
/// yield cannot be turned into a cross section without the multiplying
/// reaction's own σ — which lives in the ESZ/SIG blocks, not here. Returning the
/// count rather than silently dropping them is the difference between a partial
/// sum a caller knows about and one it does not.
pub fn photon_production_xs_at(entries: &[AcePhotonEntry], k: usize) -> (f64, usize) {
    let mut total = 0.0;
    let mut skipped = 0usize;
    for e in entries {
        match &e.rate {
            AcePhotonRate::Xs {
                threshold_index,
                xs,
            } => {
                if k >= *threshold_index {
                    if let Some(v) = xs.get(k - threshold_index) {
                        total += v;
                    }
                }
            }
            AcePhotonRate::Yield { .. } => skipped += 1,
        }
    }
    (total, skipped)
}

/// The MeV → eV factor, re-exported so a caller converting a discrete line's
/// energy does not hunt for it.
pub const EV_PER_MEV_PHOTON: f64 = EV_PER_MEV;
