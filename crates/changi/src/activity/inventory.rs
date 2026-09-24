// SPDX-License-Identifier: GPL-3.0-only
//! **A published core inventory, so a source term can be a real one.**
//!
//! # Why this exists
//!
//! [`SourceTerm`](super::source::SourceTerm) takes activities in becquerels
//! and says nothing about where they come from — deliberately, since it is a
//! dispersion input and not a reactor model. The consequence is that every
//! number in this crate's tests and examples has been an **illustrative
//! fixture**: `1.0e12` Bq of I-131 and so on, chosen to be round rather than
//! to be true.
//!
//! That is fine for exercising the arithmetic and useless for anything else.
//! This module supplies one **published** inventory so a caller can build a
//! source term whose magnitudes mean something, and so the crate's own
//! examples stop quoting numbers that came from nowhere.
//!
//! # What this is NOT
//!
//! **An inventory is not a source term.** It is what is *in the core*, not
//! what gets *out*. Turning one into the other needs a release fraction —
//! how much escapes the fuel, the vessel and the building — and this module
//! supplies none, because that is a reactor and containment question rather
//! than a dispersion one. A caller that multiplies these figures by a leak
//! fraction is doing the modelling; this module only removes the need to
//! invent the starting magnitude.
//!
//! `RESPONSIBLE_USE.md` applies: nothing here may be quoted as a source term
//! for HTR-10 or any other plant.
//!
//! # Provenance
//!
//! Liu Yuanzhong and Cao Jianzhu, *"Fission product release and its
//! environment impact for normal reactor operations and for relevant
//! accidents"*, **Nuclear Engineering and Design 218 (2002) 81–90**, Table 1,
//! "fission product inventories for equilibrium core of HTR-10" — computed
//! there with ORIGEN2 at an average burnup of 80 000 MWd/t.
//!
//! Full access terms and the transcription steps are in
//! `crates/changi/docs/References.md`. The document itself carries no reuse
//! licence and is **not** redistributed here; only the cited table of 22
//! values is, which is ordinary scientific citation.

use uom::si::f64::Radioactivity;
use uom::si::radioactivity::becquerel;

/// The table, compiled in so a missing file is a build error rather than a
/// silently empty inventory.
const HTR10_EQUILIBRIUM_CORE_CSV: &str =
    include_str!("../../reference/htr10_equilibrium_core_inventory.csv");

/// One row of the published inventory.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InventoryEntry {
    /// Nuclide label as the source tabulates it, e.g. `"Cs-137"`.
    pub nuclide: &'static str,
    /// Core inventory.
    pub activity: Radioactivity,
}

/// Every nuclide in the published HTR-10 equilibrium-core inventory.
///
/// Twenty-two entries, in the order the source tabulates them. This is **not**
/// the whole fission-product set — it is the subset Liu and Cao report, which
/// is the radiologically interesting one for a release, not a complete
/// depletion output.
#[must_use]
pub fn htr10_equilibrium_core() -> Vec<InventoryEntry> {
    HTR10_EQUILIBRIUM_CORE_CSV
        .lines()
        .skip(1)
        .filter_map(|line| line.split_once(','))
        .filter_map(|(name, bq)| {
            // `&'static str` from the compiled-in table, so an entry borrows
            // the CSV rather than allocating.
            let nuclide = name.trim();
            let activity = bq.trim().parse::<f64>().ok()?;
            Some(InventoryEntry {
                nuclide,
                activity: Radioactivity::new::<becquerel>(activity),
            })
        })
        .collect()
}

/// Look one nuclide up by label.
///
/// Returns `None` for a nuclide the table does not list, which is the honest
/// answer — a caller asking for something outside the 22 must not be handed a
/// zero that reads like a measurement.
#[must_use]
pub fn htr10_core_inventory(nuclide: &str) -> Option<Radioactivity> {
    htr10_equilibrium_core()
        .into_iter()
        .find(|e| e.nuclide == nuclide)
        .map(|e| e.activity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_has_all_twenty_two_published_nuclides() {
        assert_eq!(htr10_equilibrium_core().len(), 22, "Liu and Cao Table 1");
    }

    #[test]
    fn spot_checks_against_the_published_table() {
        // Both ends of the table, and the largest entry.
        let bq = |n: &str| htr10_core_inventory(n).map(|a| a.get::<becquerel>());
        assert_eq!(bq("H-3"), Some(3.81e12));
        assert_eq!(bq("Xe-133"), Some(2.05e16));
        assert_eq!(bq("Ag-110m"), Some(2.16e12));
    }

    #[test]
    fn an_unlisted_nuclide_is_none_rather_than_zero() {
        assert_eq!(htr10_core_inventory("Pu-239"), None);
        assert_eq!(htr10_core_inventory(""), None);
    }

    #[test]
    fn every_entry_is_positive_and_finite() {
        for e in htr10_equilibrium_core() {
            let bq = e.activity.get::<becquerel>();
            assert!(
                bq > 0.0 && bq.is_finite(),
                "{} has a non-physical inventory {bq:e}",
                e.nuclide
            );
        }
    }
}
