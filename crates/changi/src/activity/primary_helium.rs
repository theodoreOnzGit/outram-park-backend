// SPDX-License-Identifier: GPL-3.0-only
//! **A published HTR-10 primary-helium activity at the end of a 20-year life.**
//!
//! # What this is
//!
//! The activity of each important fission product (and tritium) that the
//! source calculates is *circulating in the primary helium* of HTR-10 at the
//! end of 20 years of full-power operation, in Bq. The table's title states
//! the unit (Bq) and the basis (end of a 20-year lifetime at full power).
//!
//! It sits between the other two published tables in this module:
//! [`super::inventory`] is what is *in the core*, this one is what the source
//! calculates has got *into the coolant*, and [`super::airborne_release`] is
//! what it calculates gets *out* to the environment each year. The activity
//! the source calculates is *deposited on the primary-circuit surfaces*
//! (its Table 4) is a different quantity and is not digitised here.
//!
//! # Basis of the source's calculation (so a reader knows what it includes)
//!
//! - Twenty nuclides: ten noble gases, five iodines, Sr-89, Cs-134, Cs-137,
//!   Ag-110m and **H-3**. **C-14 is not in the table**, although the source's
//!   text states a primary-helium C-14 total (6.3×10^4 Bq); asking for it here
//!   returns `None`.
//! - The source credits helium purification (99 % for I, Kr, Xe, C and
//!   tritium; 90 % for Sr, Ag, Cs, Rb, which it calls conservative),
//!   plate-out of metals and iodine in the circuit, and a primary-helium
//!   leakage of 1 % of the inventory per day.
//!
//! # What this is NOT
//!
//! - **Not wired into any model.** Nothing in this crate or in `htgr_sim_v1`
//!   reads it.
//! - **Not a release.** Helium in the circuit is not effluent; a release
//!   needs a leak or discharge path, which is a modelling decision for the
//!   maintainer.
//! - **Not a dose**, and nothing here computes one (see the scope limit in
//!   [`crate::activity`]).
//! - `RESPONSIBLE_USE.md` applies: nothing here may be quoted as a coolant
//!   activity for HTR-10 or any other plant for any operational, licensing or
//!   safety purpose.
//!
//! # Provenance
//!
//! Liu Yuanzhong and Cao Jianzhu, *"Fission product release and its
//! environment impact for normal reactor operations and for relevant
//! accidents"*, **Nuclear Engineering and Design 218 (2002) 81–90**, Table 3
//! (p. 84), "Activities of important fission products in the primary helium
//! of the HTR-10 at the end of 20a lifetime of full power operation (Bq)".
//!
//! Access terms and how the values were obtained (the maintainer's kovan
//! digitisation and an independent text-layer transcription, which agree on
//! all 20 values) are in `crates/changi/docs/References.md`. The document
//! carries no reuse licence and is **not** redistributed here; only the cited
//! table of 20 values is, which is ordinary scientific citation.

use uom::si::f64::Radioactivity;

use super::inventory::parse_nuclide_bq_csv;

/// The table, compiled in so a missing file is a build error rather than a
/// silently empty table.
const HTR10_PRIMARY_HELIUM_CSV: &str =
    include_str!("../../reference/htr10_primary_helium_activity_end_of_life.csv");

/// One row of the published primary-helium activity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrimaryHeliumActivityEntry {
    /// Nuclide label as the source tabulates it, e.g. `"Kr-88"`.
    pub nuclide: &'static str,
    /// Activity circulating in the primary helium at the end of 20 years of
    /// full-power operation.
    pub activity: Radioactivity,
}

/// Every nuclide in the published HTR-10 end-of-life primary-helium activity.
///
/// Twenty entries, in the order the source tabulates them (Kr-85 before
/// Kr-85m, Xe-133 before Xe-133m, Xe-135 before Xe-135m — the reverse of its
/// Table 5's order for those pairs).
#[must_use]
pub fn htr10_primary_helium_end_of_life() -> Vec<PrimaryHeliumActivityEntry> {
    parse_nuclide_bq_csv(HTR10_PRIMARY_HELIUM_CSV)
        .into_iter()
        .map(|(nuclide, activity)| PrimaryHeliumActivityEntry { nuclide, activity })
        .collect()
}

/// Look one nuclide's end-of-life primary-helium activity up by label.
///
/// Returns `None` for a nuclide the table does not list (including C-14,
/// whose total the source states in text but does not tabulate). Absent means
/// not reported, not zero.
#[must_use]
pub fn htr10_primary_helium_activity(nuclide: &str) -> Option<Radioactivity> {
    htr10_primary_helium_end_of_life()
        .into_iter()
        .find(|e| e.nuclide == nuclide)
        .map(|e| e.activity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::radioactivity::becquerel;

    fn bq(n: &str) -> Option<f64> {
        htr10_primary_helium_activity(n).map(|a| a.get::<becquerel>())
    }

    fn sum_of(names: &[&str]) -> f64 {
        htr10_primary_helium_end_of_life()
            .iter()
            .filter(|e| names.contains(&e.nuclide))
            .map(|e| e.activity.get::<becquerel>())
            .sum()
    }

    #[test]
    fn the_table_has_all_twenty_published_nuclides() {
        assert_eq!(
            htr10_primary_helium_end_of_life().len(),
            20,
            "Liu and Cao Table 3"
        );
    }

    #[test]
    fn spot_checks_against_the_published_table() {
        // First entry, the space-normalised Xe-131m, smallest, largest noble
        // gas, last.
        assert_eq!(bq("Kr-83m"), Some(5.4e8));
        assert_eq!(bq("Xe-131m"), Some(9.3e6));
        assert_eq!(bq("Sr-89"), Some(1.9));
        assert_eq!(bq("Kr-88"), Some(3.7e9));
        assert_eq!(bq("H-3"), Some(5.7e9));
    }

    /// Checks the transcription against the group totals the source states in
    /// its own text (end of Section 2.4, p. 84), an independent check on the 20 entries.
    ///
    /// Methodology: sum each nuclide group and compare with the stated
    /// primary-helium totals: noble gases 1.4×10^10 Bq, iodine isotopes
    /// 4.9×10^8 Bq, long-lived solid isotopes 2.2×10^3 Bq, tritium
    /// 5.7×10^9 Bq. "Long-lived solid" is taken as the four non-gaseous
    /// entries (Sr-89, Cs-134, Cs-137, Ag-110m); the paper does not list the
    /// group, and Sr-89 (1.9 Bq) cannot move the sum either way. The totals
    /// carry two significant figures, so an order-of-magnitude slip in any
    /// large entry of a group would fail this; it cannot catch a small error
    /// in a small entry.
    ///
    /// Result (2026-09-28): noble gases 1.3838e10 (−1.2 %), iodines 4.851e8
    /// (−1.0 %), long-lived solids 2217.9 (+0.8 %), tritium 5.7e9 (exact).
    /// All within two-significant-figure rounding. The 5 % tolerance is set
    /// by that rounding, not fitted to the residuals.
    #[test]
    fn group_sums_reproduce_the_totals_the_source_states() {
        let noble = sum_of(&[
            "Kr-83m", "Kr-85", "Kr-85m", "Kr-87", "Kr-88", "Xe-131m", "Xe-133", "Xe-133m",
            "Xe-135", "Xe-135m",
        ]);
        assert!((noble / 1.4e10 - 1.0).abs() < 0.05, "noble {noble:e}");

        let iodine = sum_of(&["I-131", "I-132", "I-133", "I-134", "I-135"]);
        assert!((iodine / 4.9e8 - 1.0).abs() < 0.05, "iodine {iodine:e}");

        let solids = sum_of(&["Sr-89", "Cs-134", "Cs-137", "Ag-110m"]);
        assert!((solids / 2.2e3 - 1.0).abs() < 0.05, "solids {solids:e}");

        assert_eq!(bq("H-3"), Some(5.7e9));

        // The groups partition the table: nothing is left out of the check.
        let all: f64 = htr10_primary_helium_end_of_life()
            .iter()
            .map(|e| e.activity.get::<becquerel>())
            .sum();
        assert!((all - (noble + iodine + solids + 5.7e9)).abs() < 1e-6 * all);
    }

    #[test]
    fn an_unlisted_nuclide_is_none_rather_than_zero() {
        // C-14's primary-helium total is stated in the text but not tabulated;
        // Ar-41 is in Table 5, Rb-88 and Sr-90 in Table 1, none in Table 3.
        assert_eq!(bq("C-14"), None);
        assert_eq!(bq("Ar-41"), None);
        assert_eq!(bq("Rb-88"), None);
        assert_eq!(bq("Sr-90"), None);
        assert_eq!(bq(""), None);
    }

    #[test]
    fn every_entry_is_positive_and_finite() {
        for e in htr10_primary_helium_end_of_life() {
            let v = e.activity.get::<becquerel>();
            assert!(v > 0.0 && v.is_finite(), "{} has {v:e}", e.nuclide);
        }
    }
}
