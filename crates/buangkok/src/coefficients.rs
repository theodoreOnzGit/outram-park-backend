// SPDX-License-Identifier: GPL-3.0-only
//! Freely usable dose coefficients from the US EPA Federal Guidance Reports,
//! for the five nuclides `htgr_sim_v1` tracks (Kr-85, Xe-133, I-131, Cs-137,
//! Ag-110m) plus Cs-137's short-lived daughter Ba-137m, all ages. The two
//! FGR-15 tables also carry the 18 further nuclides of Liu & Cao (2002)
//! Tables 5 and 8, **Adult only** (younger ages NaN = missing), added
//! 2026-09-29 for the gh:#379 cross-check -- the workspace's one source of
//! FGR coefficients. The progeny table still holds only Cs-137 -> Ba-137m, so
//! e.g. Kr-88 -> Rb-88 is not corrected for.
//!
//! **Not a port.** The pyDOSEIA port ([`crate::pydoseia`]) ships no
//! coefficient data and takes caller-supplied tables in upstream's CSV layout.
//! This module is such a caller-supplied set, compiled in from `reference/`,
//! and returned as the port's own table types so the port's lookups
//! ([`crate::pydoseia::dcf::external_dcf`],
//! [`crate::pydoseia::dcf::InhalationDcfTable`]) do the selecting.
//! Added 2026-09-29 when the maintainer asked for a dose-rate map in
//! `htgr_sim_v1`.
//!
//! | Table | Source | CSV |
//! |---|---|---|
//! | air submersion, Sv m^3 Bq^-1 s^-1 | FGR-15 (EPA 402-R-25-001, **July 2025 revision**), Table 4-6 | `fgr15_2025_air_submersion_dose_rate_coefficients.csv` |
//! | ground surface, Sv m^2 Bq^-1 s^-1 | FGR-15 (2025), Table 4-1 | `fgr15_2025_ground_surface_dose_rate_coefficients.csv` |
//! | Cs-137 -> Ba-137m, 0.944; T1/2 2.552 min | FGR-15 (2025), worked Example 4, pp. 269-270 | `fgr15_2025_short_lived_progeny_*.csv` |
//! | inhalation, committed Sv/Bq, adult | FGR-11 (EPA-520/1-88-020, 1988), Table 2.1, "Effective" | `fgr11_inhalation_committed_dose_coefficients.csv` |
//!
//! Page numbers, the licence basis (EPA's statement: non-commercial, scientific
//! and educational use) and how each value was read are in
//! `crates/buangkok/docs/References.md`. The withdrawn 2019 FGR-15
//! (EPA-402/R-19/002) is **not** used: EPA says its tables contain errors.
//!
//! # Two known inconsistencies, stated rather than hidden
//!
//! - FGR-15 (2025) coefficients are **ICRP 103** effective dose; FGR-11's are
//!   ICRP 26/30 **committed effective dose equivalent** for Reference Man.
//!   Adding them is common screening practice but mixes two weighting
//!   schemes.
//! - FGR-11 is **adult only**; the other five age columns of its CSV are blank
//!   (NaN), so a non-adult inhalation lookup returns `None`, never a number.
//!
//! Research-grade only: never a dose to a real person, and not for emergency,
//! regulatory, occupational or medical use (`RESPONSIBLE_USE.md`).

use crate::pydoseia::dcf::{
    self, AgeBracket, ExternalDcfTable, InhalationDcfTable, ProgenyChains, ProgenyCorrection,
};

const AIR_SUBMERSION_CSV: &str =
    include_str!("../reference/fgr15_2025_air_submersion_dose_rate_coefficients.csv");
const GROUND_SURFACE_CSV: &str =
    include_str!("../reference/fgr15_2025_ground_surface_dose_rate_coefficients.csv");
const PROGENY_LINKS_CSV: &str =
    include_str!("../reference/fgr15_2025_short_lived_progeny_links.csv");
const PROGENY_HALF_LIVES_CSV: &str =
    include_str!("../reference/fgr15_2025_short_lived_progeny_half_lives.csv");
const INHALATION_CSV: &str =
    include_str!("../reference/fgr11_inhalation_committed_dose_coefficients.csv");

/// The progeny correction used with these tables: include daughters with a
/// half-life of at most 1800 s (pyDOSEIA's default `ignore_half_life`).
/// For the five nuclides here that adds exactly one term, Ba-137m to Cs-137,
/// taken in secular equilibrium (0.944 Bq of Ba-137m per Bq of Cs-137).
pub const PROGENY: ProgenyCorrection = ProgenyCorrection::IncludeShortLived {
    ignore_half_life_s: 1800.0,
};

/// FGR-15 (2025) Table 4-6, air submersion.
///
/// # Panics
/// Never for the shipped CSV (a test parses it).
#[must_use]
pub fn fgr15_air_submersion() -> ExternalDcfTable {
    ExternalDcfTable::from_csv(AIR_SUBMERSION_CSV).expect("shipped FGR-15 submersion CSV parses")
}

/// FGR-15 (2025) Table 4-1, ground surface.
///
/// # Panics
/// Never for the shipped CSV (a test parses it).
#[must_use]
pub fn fgr15_ground_surface() -> ExternalDcfTable {
    ExternalDcfTable::from_csv(GROUND_SURFACE_CSV).expect("shipped FGR-15 ground CSV parses")
}

/// The one short-lived progeny link these nuclides need (Cs-137 -> Ba-137m).
///
/// # Panics
/// Never for the shipped CSVs (a test parses them).
#[must_use]
pub fn fgr15_short_lived_progeny() -> ProgenyChains {
    ProgenyChains::from_csv(PROGENY_LINKS_CSV, PROGENY_HALF_LIVES_CSV)
        .expect("shipped progeny CSVs parse")
}

/// FGR-11 Table 2.1 inhalation, adult only, in the port's inhalation-table
/// layout (the `Type` column holds FGR-11's D/W/Y clearance class).
///
/// # Panics
/// Never for the shipped CSV (a test parses it).
#[must_use]
pub fn fgr11_inhalation() -> InhalationDcfTable {
    InhalationDcfTable::from_csv(INHALATION_CSV).expect("shipped FGR-11 inhalation CSV parses")
}

/// An external dose-rate coefficient through the port's own lookup
/// ([`dcf::external_dcf`], with [`PROGENY`]), or `None` when the table has no
/// row for `nuclide` at that age. **`None` means missing, never zero.**
#[must_use]
pub fn external_coefficient(
    table: &ExternalDcfTable,
    chains: &ProgenyChains,
    nuclide: &str,
    age: AgeBracket,
) -> Option<f64> {
    let c = dcf::external_dcf(table, chains, nuclide, age, PROGENY).selected(PROGENY);
    c.is_finite().then_some(c)
}

/// The FGR-11 inhalation coefficient for `nuclide` at `age`, taking the
/// **largest over FGR-11's lung clearance classes** (the same rule as the
/// port's `LungAbsorptionType::Max`; with no chemical-form information that is
/// the conservative choice). `None` when there is no entry -- the noble gases,
/// and every non-adult age, since FGR-11 is adult only.
#[must_use]
pub fn fgr11_inhalation_max_over_classes(
    table: &InhalationDcfTable,
    nuclide: &str,
    age: AgeBracket,
) -> Option<f64> {
    let c = dcf::nan_max(
        table
            .rows
            .iter()
            .filter(|r| r.nuclide == nuclide)
            .map(|r| r.sv_per_bq[age.column()]),
    );
    c.is_finite().then_some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped CSVs hold exactly the values read off the reports. The
    /// literals below are the Adult column as printed (FGR-15 2025 Tables 4-6
    /// and 4-1; FGR-11 Table 2.1 "Effective"), so an edited CSV fails here.
    #[test]
    fn shipped_coefficients_match_the_reports() {
        let sub = fgr15_air_submersion();
        let gs = fgr15_ground_surface();
        let a = AgeBracket::Adult;
        for (n, s, g) in [
            ("Kr-85", 2.40e-16, 9.62e-18),
            ("Xe-133", 1.22e-15, 1.85e-17),
            ("I-131", 1.68e-14, 2.43e-16),
            ("Cs-137", 9.37e-17, 3.01e-18),
            ("Ag-110m", 1.28e-13, 1.73e-15),
            ("Ba-137m", 2.68e-14, 3.87e-16),
        ] {
            assert_eq!(sub.lookup_exact(n, a), s, "{n} submersion");
            assert_eq!(gs.lookup_exact(n, a), g, "{n} ground");
        }
        let inh = fgr11_inhalation();
        assert_eq!(
            fgr11_inhalation_max_over_classes(&inh, "I-131", a),
            Some(8.89e-9)
        );
        assert_eq!(
            fgr11_inhalation_max_over_classes(&inh, "Cs-137", a),
            Some(8.63e-9)
        );
        // Class Y is the largest of D 1.07e-8, W 8.34e-9, Y 2.17e-8.
        assert_eq!(
            fgr11_inhalation_max_over_classes(&inh, "Ag-110m", a),
            Some(2.17e-8)
        );
    }

    /// Missing is `None`, never zero: noble gases have no FGR-11 inhalation
    /// entry, FGR-11 has no child ages, and an unknown nuclide has no row.
    #[test]
    fn missing_coefficients_are_none_not_zero() {
        let inh = fgr11_inhalation();
        let (sub, chains) = (fgr15_air_submersion(), fgr15_short_lived_progeny());
        assert_eq!(
            fgr11_inhalation_max_over_classes(&inh, "Kr-85", AgeBracket::Adult),
            None
        );
        assert_eq!(
            fgr11_inhalation_max_over_classes(&inh, "Xe-133", AgeBracket::Adult),
            None
        );
        assert_eq!(
            fgr11_inhalation_max_over_classes(&inh, "I-131", AgeBracket::Infant),
            None
        );
        // ~~Sr-90 Adult~~ has a row since 2026-09-29 (the Liu & Cao release
        // list, gh:#379, Adult only): a nuclide with no row, and an appended
        // row's blank younger age, are the missing cases now.
        assert_eq!(
            external_coefficient(&sub, &chains, "Pu-239", AgeBracket::Adult),
            None
        );
        assert_eq!(
            external_coefficient(&sub, &chains, "Sr-90", AgeBracket::Infant),
            None
        );
        assert!(external_coefficient(&sub, &chains, "Sr-90", AgeBracket::Adult).is_some());
    }

    /// Cs-137's external coefficients carry Ba-137m at 0.944, through the
    /// port's progeny lookup; the other four nuclides are parent only.
    #[test]
    fn cs137_carries_ba137m_in_secular_equilibrium() {
        let (sub, gs, chains) = (
            fgr15_air_submersion(),
            fgr15_ground_surface(),
            fgr15_short_lived_progeny(),
        );
        let a = AgeBracket::Adult;
        let cs_sub = external_coefficient(&sub, &chains, "Cs-137", a).unwrap();
        assert_eq!(cs_sub, 9.37e-17 + 2.68e-14 * 0.944);
        let cs_gs = external_coefficient(&gs, &chains, "Cs-137", a).unwrap();
        assert_eq!(cs_gs, 3.01e-18 + 3.87e-16 * 0.944);
        assert_eq!(
            external_coefficient(&sub, &chains, "I-131", a),
            Some(1.68e-14)
        );
    }
}
