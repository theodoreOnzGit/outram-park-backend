// SPDX-License-Identifier: GPL-3.0-only
//! **A published annual airborne release from HTR-10 under normal operation.**
//!
//! # What this is
//!
//! The activity of each nuclide that the source calculates is released to the
//! environment as airborne effluent in **one year of normal operation** of
//! HTR-10. The table itself gives no unit or time basis; the paper's text
//! calls it the annual amount and states totals in becquerels, so each entry
//! here is activity released over one year, in Bq.
//!
//! It is the counterpart to [`super::inventory`]. That module is what is *in
//! the core*; this one is what the source calculates *gets out* during normal
//! operation. That is **not** an accident source term, which the same paper
//! tabulates separately (its Table 8, ~~not digitised here~~ **CORRECTED
//! 2026-09-28**: now in [`crate::activity::accident_airborne_release`]).
//!
//! # Basis of the source's calculation (so a reader knows what it includes)
//!
//! - Twenty-two nuclides: noble gases, iodines, Sr-89, Cs-134, Cs-137,
//!   Ag-110m, and three nuclides that are not fission products of the core
//!   inventory: **H-3**, **C-14** and **Ar-41**. Ar-41 comes from neutron
//!   activation of argon in the reactor-cavity air.
//! - Contributions from cavity-air activation, primary-helium leakage, the
//!   contaminated-helium tank, fuel-handling vacuum systems, tritiated
//!   secondary-steam leakage, and maintenance.
//! - **Filtration is not credited.** The source says its calculation is
//!   conservative and leaves out the filter system, so these figures are
//!   upper estimates of what a filtered plant would release.
//!
//! # What this is NOT
//!
//! - **Not wired into any model.** Nothing in this crate or in `htgr_sim_v1`
//!   reads it. Whether and how it replaces a leak-rate-times-inventory
//!   estimate is a modelling decision for the maintainer.
//! - **Not a dose**, and nothing here computes one (see the scope limit in
//!   [`crate::activity`]).
//! - `RESPONSIBLE_USE.md` applies: nothing here may be quoted as a release
//!   figure for HTR-10 or any other plant for any operational, licensing or
//!   safety purpose.
//!
//! # Provenance
//!
//! Liu Yuanzhong and Cao Jianzhu, *"Fission product release and its
//! environment impact for normal reactor operations and for relevant
//! accidents"*, **Nuclear Engineering and Design 218 (2002) 81–90**, Table 5
//! (p. 85), "Amount of airborne radioactivity released into the environment
//! in the HTR-10 normal operation conditions".
//!
//! Access terms and how the values were obtained (the maintainer's kovan
//! digitisation and an independent text-layer transcription, which agree on
//! all 22 values) are in `crates/changi/docs/References.md`. The document carries no
//! reuse licence and is **not** redistributed here; only the cited table of 22
//! values is, which is ordinary scientific citation.

use uom::si::f64::Radioactivity;

use super::inventory::parse_nuclide_bq_csv;

/// The table, compiled in so a missing file is a build error rather than a
/// silently empty table.
const HTR10_NORMAL_OPERATION_RELEASE_CSV: &str =
    include_str!("../../reference/htr10_normal_operation_annual_airborne_release.csv");

/// One row of the published annual release.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AirborneReleaseEntry {
    /// Nuclide label as the source tabulates it, e.g. `"Ar-41"`.
    pub nuclide: &'static str,
    /// Activity released to the environment over one year of normal
    /// operation (unfiltered, per the source's conservative basis).
    pub annual_release: Radioactivity,
}

/// Every nuclide in the published HTR-10 normal-operation annual airborne
/// release.
///
/// Twenty-two entries, in the order the source tabulates them. Note that the
/// source orders Kr-85m before Kr-85, Xe-133m before Xe-133 and Xe-135m
/// before Xe-135, which is not the order its Table 1 uses.
#[must_use]
pub fn htr10_normal_operation_annual_release() -> Vec<AirborneReleaseEntry> {
    parse_nuclide_bq_csv(HTR10_NORMAL_OPERATION_RELEASE_CSV)
        .into_iter()
        .map(|(nuclide, annual_release)| AirborneReleaseEntry {
            nuclide,
            annual_release,
        })
        .collect()
}

/// Look one nuclide's annual airborne release up by label.
///
/// Returns `None` for a nuclide the table does not list. A nuclide that is
/// absent was not reported, which is not the same thing as a zero release.
#[must_use]
pub fn htr10_annual_airborne_release(nuclide: &str) -> Option<Radioactivity> {
    htr10_normal_operation_annual_release()
        .into_iter()
        .find(|e| e.nuclide == nuclide)
        .map(|e| e.annual_release)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::radioactivity::becquerel;

    fn bq(n: &str) -> Option<f64> {
        htr10_annual_airborne_release(n).map(|a| a.get::<becquerel>())
    }

    #[test]
    fn the_table_has_all_twenty_two_published_nuclides() {
        assert_eq!(
            htr10_normal_operation_annual_release().len(),
            22,
            "Liu and Cao Table 5"
        );
    }

    #[test]
    fn spot_checks_against_the_published_table() {
        // First entry, smallest, both non-fission-product dominants, last.
        assert_eq!(bq("Kr-83m"), Some(3.8e8));
        assert_eq!(bq("Sr-89"), Some(8.1));
        assert_eq!(bq("H-3"), Some(7.9e10));
        assert_eq!(bq("C-14"), Some(3.1e7));
        assert_eq!(bq("Ar-41"), Some(1.0e11));
    }

    /// Checks the transcription against the totals the source states in its
    /// own text, which is an independent check on the 22 entries.
    ///
    /// Methodology: sum the table and compare it with the stated totals,
    /// "2×10^11 Bq" overall and "2.2×10^10 Bq" excluding Ar-41 and H-3.
    /// The stated figures carry one and two significant figures, so a
    /// transcription slip of an order of magnitude in any large entry would
    /// fail this. It cannot catch a small error in a small entry.
    ///
    /// Result (2026-09-28): total = 2.004e11 Bq (0.2 % from 2e11); excluding
    /// Ar-41 and H-3 = 2.137e10 Bq, which is 2.9 % below the stated 2.2e10.
    /// That residual is in the source itself (2.137e10 rounds to 2.1e10, not
    /// 2.2e10). Its likely cause is rounding of the 2-significant-figure
    /// entries, but the paper does not say. The 5 % tolerance below is set
    /// by two-significant-figure rounding, not fitted to that residual.
    #[test]
    fn the_sum_reproduces_the_totals_the_source_states() {
        let rows = htr10_normal_operation_annual_release();
        let total: f64 = rows
            .iter()
            .map(|e| e.annual_release.get::<becquerel>())
            .sum();
        assert!((total / 2.0e11 - 1.0).abs() < 0.05, "total {total:e}");

        let rest: f64 = rows
            .iter()
            .filter(|e| e.nuclide != "Ar-41" && e.nuclide != "H-3")
            .map(|e| e.annual_release.get::<becquerel>())
            .sum();
        assert!((rest / 2.2e10 - 1.0).abs() < 0.05, "rest {rest:e}");
    }

    #[test]
    fn ar41_dominates_and_tritium_is_second_as_the_source_states() {
        let mut rows = htr10_normal_operation_annual_release();
        rows.sort_by(|a, b| {
            b.annual_release
                .get::<becquerel>()
                .total_cmp(&a.annual_release.get::<becquerel>())
        });
        assert_eq!(rows[0].nuclide, "Ar-41");
        assert_eq!(rows[1].nuclide, "H-3");
    }

    #[test]
    fn an_unlisted_nuclide_is_none_rather_than_zero() {
        // Rb-88 and Sr-90 are in Table 1's inventory but not in Table 5.
        assert_eq!(bq("Rb-88"), None);
        assert_eq!(bq("Sr-90"), None);
        assert_eq!(bq(""), None);
    }

    #[test]
    fn every_entry_is_positive_and_finite() {
        for e in htr10_normal_operation_annual_release() {
            let v = e.annual_release.get::<becquerel>();
            assert!(v > 0.0 && v.is_finite(), "{} has {v:e}", e.nuclide);
        }
    }
}
