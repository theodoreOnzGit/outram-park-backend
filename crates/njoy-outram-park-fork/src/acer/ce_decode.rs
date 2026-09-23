// SPDX-License-Identifier: GPL-3.0

//! **Decode a continuous-energy neutron ACE table into usable physics.**
//!
//! [`crate::acer::read`] parses an ACE file into a [`RawAceTable`] -- the
//! `NXS`/`JXS`/`XSS` triple, verbatim. That is enough to *compare* two tables
//! word by word, which is what the NJOY2016 parity tests do, and it is not
//! enough to *transport* with: nothing here interpreted a single block until
//! this module existed. The consequence was concrete -- the ~102 MB
//! `reference-data/ace` submodule was read by no code in the workspace, only
//! cited in doc comments.
//!
//! # Upstream is the specification
//!
//! OpenMC's C++ reads HDF5, not ACE; its ACE reader is Python. The block
//! layout below is taken from:
//!
//! - `openmc/data/neutron.py:551-586` (`IncidentNeutron.from_ace`) for the ESZ
//!   block and the reaction loop;
//! - `openmc/data/reaction.py:1005-1056` (`Reaction.from_ace`) for MTR, LQR,
//!   TYR, LSIG and SIG;
//!
//! both read at OpenMC `afa7a14`, and cross-checked against this crate's own
//! ACER **writer** (`acer::build`), which assigns the same locators in the
//! other direction and is already verified byte-for-byte against NJOY2016.
//!
//! # Index convention, which is the easy thing to get wrong
//!
//! OpenMC's Python keeps `nxs`/`jxs` as **1-based** arrays with index 0 unused,
//! so `ace.nxs[3]` there is `NXS(3)`. [`RawAceTable`] stores them **0-based**,
//! so the same quantity is `nxs[nxs::NES]` with `NES = 2`. Every index below
//! goes through the named constants in [`crate::acer::nxs`] and
//! [`crate::acer::jxs`] rather than a literal, because an off-by-one here does
//! not crash -- it silently reads the neighbouring block and produces cross
//! sections that look plausible.
//!
//! `JXS` locators are **1-based into `XSS`**, which is why every access goes
//! through [`RawAceTable::at`] or subtracts one explicitly.
//!
//! Energies and `Q` values are **MeV in the file** and eV everywhere in this
//! workspace.

use crate::acer::read::RawAceTable;
use crate::acer::{jxs, nxs};
use crate::error::NjoyError;

/// eV per MeV. ACE stores energies and `Q` values in MeV.
pub const EV_PER_MEV: f64 = 1.0e6;

/// One reaction decoded out of the MTR/LQR/TYR/LSIG/SIG blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct AceReaction {
    /// ENDF MT number, from the MTR block.
    pub mt: i32,
    /// `QI` \[eV\], from LQR (stored there in MeV).
    pub q_value: f64,
    /// TYR: the neutron yield. **Negative means the secondary distribution is
    /// in the centre-of-mass frame** (`reaction.py:1056`), `19` means fission,
    /// and `|ty| > 100` means an energy-dependent yield stored in DLW.
    pub ty: i32,
    /// 0-based index into the nuclide energy grid where this reaction starts.
    pub threshold_index: usize,
    /// Cross section \[barn\], one value per grid point from `threshold_index`.
    pub xs: Vec<f64>,
}

impl AceReaction {
    /// Whether the secondary distribution is in the centre-of-mass frame.
    pub fn center_of_mass(&self) -> bool {
        self.ty < 0
    }
}

/// A continuous-energy neutron ACE table, decoded.
#[derive(Debug, Clone, PartialEq)]
pub struct CeNeutronAce {
    /// The ZAID as written in the header, e.g. `"92235.00c"`.
    pub zaid: String,
    /// `NXS(2)`: `1000*Z + A`.
    pub za: i32,
    /// Atomic weight ratio.
    pub awr: f64,
    /// Table temperature \[eV\].
    pub kt_ev: f64,
    /// The nuclide energy grid \[eV\], ascending.
    pub energy: Vec<f64>,
    /// Total cross section \[barn\] on `energy`.
    pub total: Vec<f64>,
    /// Absorption (`MT=101`) \[barn\] on `energy`.
    pub absorption: Vec<f64>,
    /// Elastic (`MT=2`) \[barn\] on `energy`.
    pub elastic: Vec<f64>,
    /// Heating number \[eV\] on `energy`.
    pub heating: Vec<f64>,
    /// Every reaction in MTR order.
    pub reactions: Vec<AceReaction>,
}

impl CeNeutronAce {
    /// The cross section for `mt` on the full grid, zero-filled below its
    /// threshold, or `None` if the table has no such reaction.
    ///
    /// `MT=2` comes from ESZ rather than SIG, because ACE stores elastic in the
    /// ESZ block and **not** as an MTR entry -- a reader that looks for it in
    /// MTR finds nothing and silently reports a nuclide that cannot scatter.
    pub fn xs_on_grid(&self, mt: i32) -> Option<Vec<f64>> {
        if mt == 2 {
            return Some(self.elastic.clone());
        }
        if mt == 1 {
            return Some(self.total.clone());
        }
        let rx = self.reactions.iter().find(|r| r.mt == mt)?;
        let mut out = vec![0.0; self.energy.len()];
        out[rx.threshold_index..rx.threshold_index + rx.xs.len()].copy_from_slice(&rx.xs);
        Some(out)
    }
}

fn slice_at(t: &RawAceTable, start_1based: i32, len: usize, what: &str) -> Result<Vec<f64>, NjoyError> {
    if start_1based <= 0 {
        return Err(NjoyError::EndfParse(format!(
            "ACE table has no {what} block (locator {start_1based})"
        )));
    }
    let s = (start_1based - 1) as usize;
    let end = s + len;
    if end > t.xss.len() {
        return Err(NjoyError::EndfParse(format!(
            "ACE {what} block runs past XSS: need {end} words, have {}",
            t.xss.len()
        )));
    }
    Ok(t.xss[s..end].to_vec())
}

/// Decode a continuous-energy neutron table.
///
/// # Errors
///
/// A missing ESZ block, or any block whose declared extent runs past `XSS`.
/// Both mean the table is not the continuous-energy neutron class this decodes
/// -- refusing is deliberate, since reading a thermal or dosimetry table
/// through this path would produce numbers rather than an error.
pub fn decode_ce(t: &RawAceTable) -> Result<CeNeutronAce, NjoyError> {
    let n_energy = t.nxs[nxs::NES] as usize;
    if n_energy == 0 {
        return Err(NjoyError::EndfParse(
            "ACE table declares NXS(3) = 0 energies; not a continuous-energy table".into(),
        ));
    }
    let esz = t.jxs[jxs::ESZ];
    // ESZ is five consecutive NES-long arrays:
    //   energy, total, absorption, elastic, heating   (neutron.py:557-563)
    let block = slice_at(t, esz, 5 * n_energy, "ESZ")?;
    let energy: Vec<f64> = block[0..n_energy].iter().map(|e| e * EV_PER_MEV).collect();
    let total = block[n_energy..2 * n_energy].to_vec();
    let absorption = block[2 * n_energy..3 * n_energy].to_vec();
    let elastic = block[3 * n_energy..4 * n_energy].to_vec();
    let heating: Vec<f64> = block[4 * n_energy..5 * n_energy]
        .iter()
        .map(|h| h * EV_PER_MEV)
        .collect();

    let n_rx = t.nxs[nxs::NTR] as usize;
    let mut reactions = Vec::with_capacity(n_rx);
    if n_rx > 0 {
        let mtr = slice_at(t, t.jxs[jxs::MTR], n_rx, "MTR")?;
        let lqr = slice_at(t, t.jxs[jxs::LQR], n_rx, "LQR")?;
        let tyr = slice_at(t, t.jxs[jxs::TYR], n_rx, "TYR")?;
        let lsig = slice_at(t, t.jxs[jxs::LSIG], n_rx, "LSIG")?;
        let sig = t.jxs[jxs::SIG];
        if sig <= 0 {
            return Err(NjoyError::EndfParse("ACE table has MTR but no SIG block".into()));
        }
        for i in 0..n_rx {
            // `loc` is 1-based within SIG; SIG itself is 1-based within XSS,
            // so the pair index is `SIG + loc - 1 - 1` zero-based
            // (reaction.py:1025-1035).
            let loc = lsig[i] as i32;
            let head = slice_at(t, sig + loc - 1, 2, "SIG")?;
            let threshold_index = (head[0] as i64 - 1).max(0) as usize;
            let n = head[1] as usize;
            if threshold_index + n > n_energy {
                return Err(NjoyError::EndfParse(format!(
                    "ACE reaction MT={} spans grid points {}..{} but the grid has {n_energy}",
                    mtr[i] as i32,
                    threshold_index,
                    threshold_index + n
                )));
            }
            let mut xs = slice_at(t, sig + loc + 1, n, "SIG")?;
            // "Fix negatives -- known issue for Y89 in JEFF 3.2"
            // (reaction.py:1041-1045). Upstream zeroes them and warns; a
            // negative cross section would otherwise make a macroscopic total
            // that a sampler can drive negative.
            for v in xs.iter_mut() {
                if *v < 0.0 {
                    *v = 0.0;
                }
            }
            reactions.push(AceReaction {
                mt: mtr[i] as i32,
                q_value: lqr[i] * EV_PER_MEV,
                ty: tyr[i] as i32,
                threshold_index,
                xs,
            });
        }
    }

    Ok(CeNeutronAce {
        zaid: t.header.zaid.clone(),
        za: t.nxs[nxs::ZA],
        awr: t.header.awr,
        kt_ev: t.header.kt_mev * EV_PER_MEV,
        energy,
        total,
        absorption,
        elastic,
        heating,
        reactions,
    })
}

/// The partial-fission MTs, in the order ENDF defines them: first-chance
/// `(n,f)`, `(n,n'f)`, `(n,2nf)`, `(n,3nf)`.
pub const PARTIAL_FISSION_MTS: [i32; 4] = [19, 20, 21, 38];

impl CeNeutronAce {
    /// Total fission cross section \[barn\] on the full grid, or `None` for a
    /// non-fissionable table.
    ///
    /// # Read this before looking up MT=18 yourself
    ///
    /// **An ACE table for a fissionable nuclide frequently has no MT=18.** When
    /// the evaluation carries partial fission, ACER writes MT=19/20/21/38 and
    /// omits the total. Measured on the reference library in
    /// `reference-data/ace`: NJOY2016's U-234 at 293.6 K has MTs
    /// `16 17 19 20 21 37 38 51..91 102` -- **no 18**. A reader that asks for
    /// MT=18 and treats `None` as "not fissionable" gets a nuclide that cannot
    /// fission, and a `k` of zero, from data that fissions perfectly well.
    ///
    /// Equally, do **not** reach for the ESZ absorption array to find fission.
    /// That array is MT=101, *disappearance*, and it **excludes** fission. On
    /// the same U-234 table at 1 meV: `absorption = 518.0919 b` is capture
    /// alone, while `total - elastic = 518.4364 b`; the 0.3445 b difference is
    /// exactly MT=19. Both numbers are correct and they answer different
    /// questions.
    ///
    /// So: prefer MT=18 when present, else sum the partials. Never both -- an
    /// evaluation carrying MT=18 *and* partials would otherwise double-count.
    pub fn fission_xs(&self) -> Option<Vec<f64>> {
        if let Some(total) = self.xs_on_grid(18) {
            return Some(total);
        }
        let mut out: Option<Vec<f64>> = None;
        for mt in PARTIAL_FISSION_MTS {
            if let Some(x) = self.xs_on_grid(mt) {
                match out.as_mut() {
                    None => out = Some(x),
                    Some(acc) => {
                        for (a, b) in acc.iter_mut().zip(x.iter()) {
                            *a += b;
                        }
                    }
                }
            }
        }
        out
    }

    /// Whether this table can fission at all.
    pub fn is_fissionable(&self) -> bool {
        self.reactions
            .iter()
            .any(|r| r.mt == 18 || PARTIAL_FISSION_MTS.contains(&r.mt))
    }
}

/// MTs that are **always** derived from others and must never be summed with
/// them: total, nonelastic, the `(n,disappearance)` lump, gas production, the
/// heating number and the damage-energy production.
pub const ALWAYS_REDUNDANT_MTS: [i32; 11] = [1, 3, 27, 101, 203, 204, 205, 206, 207, 301, 444];

impl CeNeutronAce {
    /// The MTs that partition the non-elastic cross section **without
    /// double-counting**, i.e. the set to sum when reconstructing `total`.
    ///
    /// # Why this is not just "every MT in MTR"
    ///
    /// ACE stores lumps *and* the levels that make them up, side by side in the
    /// same MTR block, and nothing in the file marks which is which. Summing
    /// the block verbatim counts the inelastic continuum twice.
    ///
    /// Measured on NJOY2016's U-235 at 293.6 K, 5.5 MeV: MTR's **last** entry
    /// is `MT=4` (total inelastic) at 2.223186 b, and MT=51..91 sum to the same
    /// 2.223186 b. Summing MTR as written gives 9.595 b against a tabulated
    /// total of 7.372 b -- **30 % high**, from data that is entirely correct.
    ///
    /// The same relationship holds for `MT=103` over the `(n,p)` levels
    /// 600..649, for `MT=107` over the `(n,alpha)` levels 800..849, and for
    /// `MT=18` over the partial-fission MTs (see [`Self::fission_xs`]).
    ///
    /// The rule applied here: prefer the **lump** when it is present and drop
    /// its levels; fall back to the levels when it is not. U-234 exercises the
    /// fallback (no MT=18, partials present); U-235 exercises the preference
    /// (MT=4 and MT=18 present, levels dropped).
    pub fn channel_mts(&self) -> Vec<i32> {
        let has = |mt: i32| self.reactions.iter().any(|r| r.mt == mt);
        let has_total_inelastic = has(4);
        let has_np_total = has(103);
        let has_na_total = has(107);
        let has_total_fission = has(18);

        self.reactions
            .iter()
            .map(|r| r.mt)
            .filter(|&mt| {
                if ALWAYS_REDUNDANT_MTS.contains(&mt) {
                    return false;
                }
                // Inelastic levels and the continuum, under MT=4.
                if has_total_inelastic && (51..=91).contains(&mt) {
                    return false;
                }
                // (n,p) levels under MT=103, (n,alpha) levels under MT=107.
                if has_np_total && (600..=649).contains(&mt) {
                    return false;
                }
                if has_na_total && (800..=849).contains(&mt) {
                    return false;
                }
                // Partial fission under MT=18.
                if has_total_fission && PARTIAL_FISSION_MTS.contains(&mt) {
                    return false;
                }
                true
            })
            .collect()
    }

    /// `elastic + sum(channel_mts)`, which must reproduce [`Self::total`]
    /// (`total`) to round-off.
    pub fn reconstructed_total(&self) -> Vec<f64> {
        let mut out = self.elastic.clone();
        for mt in self.channel_mts() {
            if let Some(x) = self.xs_on_grid(mt) {
                for (a, b) in out.iter_mut().zip(x.iter()) {
                    *a += b;
                }
            }
        }
        out
    }
}
