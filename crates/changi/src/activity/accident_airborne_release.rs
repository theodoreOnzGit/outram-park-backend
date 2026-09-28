// SPDX-License-Identifier: GPL-3.0-only
//! **A published airborne release from HTR-10 for two design-basis
//! accidents, stored as reference data.**
//!
//! # What this is
//!
//! The activity of each nuclide that the source calculates is released to the
//! environment, **in Bq per accident**, for the same two accidents as the
//! paper's Table 9 doses ([`published_accident_dose_by_distance`][T9]):
//!
//! - **Depressurization accident** ([`AccidentCase::Depressurization`][D]):
//!   primary helium lost through a ruptured 65 mm fuel-element charging tube.
//!   The paper's Section 4.1.1 sums the primary-helium activity, fission
//!   products desorbed from primary-circuit surfaces, dust-bound activity
//!   (10 % of the dust assumed released) and activity bound in the helium
//!   purification system (100 % of the noble gases, H-3 and C-14, 10 % of the
//!   iodine and metal fission products, if its isolation fails).
//! - **Water ingress accident** ([`AccidentCase::WaterIngress`][W]): two-ended
//!   rupture of two steam-generator tubes with the steam relief system
//!   failed. The paper's Section 4.1.2 sums about 23 % of the primary-helium
//!   activity, water wash-off of the whole steam-generator deposit, and the
//!   activity in up to 4.88 kg of corroded graphite.
//!
//! No release from the coated particles is assumed in either case. Both go
//! out through the 40 m stack with no filtering or plate-out credited. The
//! [`AccidentCase`][AC] enum is the one Table 9's loader defines, reused here so
//! the two tables name the accidents identically.
//!
//! It is a **published model result, not a measurement.** The paper does not
//! name a code for the release calculation. STOERNEU, named in its Section
//! 4.2, is the code for the *doses* computed from these releases (Table 9).
//! The paper does **not** state the release duration or time profile, or the
//! inventory state (e.g. end of life) the release is taken from.
//!
//! Eighteen nuclides: eight noble gases, four iodines (no I-134), Sr-90,
//! Cs-134, Cs-137, Ag-110m, H-3 and C-14. Sr-90 appears here although the
//! paper's Tables 3 and 5 list Sr-89 instead.
//!
//! # The C-14 label is a correction
//!
//! **The paper prints the last row as "C-4"** (text layer and rendered page
//! agree, and so does the maintainer's kovan record). There is no nuclide
//! C-4. The paper's Section 4.1.1.4 names C-14 among the released species,
//! its Table 5 lists C-14 in the same position (after H-3), and the water
//! ingress value is 0.30 of the paper's stated primary-helium C-14 total, the
//! same fraction as every noble gas (see the test
//! `water_ingress_noble_gases_h3_and_c14_are_0_30_of_primary_helium`). The row
//! is therefore stored as **C-14**. Asking for `"C-4"` returns `None`.
//!
//! # A check against the paper's own statement, and what it found
//!
//! For water ingress the paper assumes "approximate 23 %" of the primary-helium
//! activity is released. Wash-off acts on plated-out deposits, which do not
//! include noble gases, so for noble gases and H-3 the helium should be the
//! main source (the paper does not break down the corroded-graphite
//! contribution by nuclide), and their Table 8 values divided by the Table 3
//! primary-helium values should be about 0.23. **They are not: all eight
//! noble gases and H-3 give 0.295 to 0.315**, and C-14 gives 0.30 against the
//! text's stated total. The
//! uniformity across half-lives from hours to years says a single fraction
//! of about 0.30 was applied. The paper does not explain the difference; it
//! may be a different helium inventory from Table 3's end-of-life one, or a
//! different fraction. It is recorded, not reconciled.
//!
//! # What this is NOT
//!
//! - **Not wired into any model.** Nothing in this crate or in `htgr_sim_v1`
//!   reads it, and `changi` computes no dose from it.
//! - **Not a basis for emergency planning, emergency-zone sizing, siting,
//!   licensing or any safety decision**, for HTR-10 or any other plant.
//!   `RESPONSIBLE_USE.md` applies in full.
//! - **Not a beyond-design-basis source term** (no particle failure), and not
//!   the normal-operation release ([`crate::activity::airborne_release`], Table 5).
//!
//! # Provenance
//!
//! Liu Yuanzhong and Cao Jianzhu, *"Fission product release and its
//! environment impact for normal reactor operations and for relevant
//! accidents"*, **Nuclear Engineering and Design 218 (2002) 81–90**, Table 8
//! (p. 88), "The HTR-10 accidental radioactivity release (Bq)". The basis
//! above comes from the paper's Section 4.1 (pp. 86–89).
//!
//! Access terms, digitisation and verification are in
//! `crates/changi/docs/References.md`. The document carries no reuse licence
//! and is **not** redistributed here; only the cited table of 36 values is,
//! which is ordinary scientific citation.
//!
//! [T9]: crate::activity::published_accident_dose_by_distance
//! [AC]: crate::activity::published_accident_dose_by_distance::AccidentCase
//! [D]: crate::activity::published_accident_dose_by_distance::AccidentCase::Depressurization
//! [W]: crate::activity::published_accident_dose_by_distance::AccidentCase::WaterIngress

use uom::si::f64::Radioactivity;
use uom::si::radioactivity::becquerel;

pub use super::published_accident_dose_by_distance::AccidentCase;

/// The table, compiled in so a missing file is a build error rather than a
/// silently empty table.
const HTR10_ACCIDENT_RELEASE_CSV: &str =
    include_str!("../../reference/htr10_accident_airborne_release.csv");

/// One row of the published accident release.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccidentReleaseEntry {
    /// Nuclide label, as the source tabulates it except that the source's
    /// "C-4" is stored as `"C-14"` (see the module docs).
    pub nuclide: &'static str,
    /// Activity released to the environment in the depressurization accident.
    pub depressurization: Radioactivity,
    /// Activity released to the environment in the water ingress accident.
    pub water_ingress: Radioactivity,
}

impl AccidentReleaseEntry {
    /// The release for one accident case.
    #[must_use]
    pub fn release(&self, case: AccidentCase) -> Radioactivity {
        match case {
            AccidentCase::Depressurization => self.depressurization,
            AccidentCase::WaterIngress => self.water_ingress,
        }
    }
}

/// Every nuclide in the published HTR-10 accident release, in the source's
/// order (eighteen entries).
///
/// The CSV has three columns, so the two-column
/// `inventory::parse_nuclide_bq_csv` cannot read it; this parser
/// follows the same rules (header skipped, a row that does not parse is
/// dropped, which the row-count test catches).
#[must_use]
pub fn htr10_accident_release() -> Vec<AccidentReleaseEntry> {
    HTR10_ACCIDENT_RELEASE_CSV
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut f = line.split(',').map(str::trim);
            let nuclide = f.next()?;
            let d = f.next()?.parse::<f64>().ok()?;
            let w = f.next()?.parse::<f64>().ok()?;
            if f.next().is_some() {
                return None;
            }
            Some(AccidentReleaseEntry {
                nuclide,
                depressurization: Radioactivity::new::<becquerel>(d),
                water_ingress: Radioactivity::new::<becquerel>(w),
            })
        })
        .collect()
}

/// Look one nuclide's release up by label and accident.
///
/// Returns `None` for a nuclide the table does not list. An absent nuclide
/// was not reported, which is not the same thing as a zero release.
#[must_use]
pub fn htr10_accident_airborne_release(nuclide: &str, case: AccidentCase) -> Option<Radioactivity> {
    htr10_accident_release()
        .into_iter()
        .find(|e| e.nuclide == nuclide)
        .map(|e| e.release(case))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity::primary_helium::htr10_primary_helium_activity;

    const D: AccidentCase = AccidentCase::Depressurization;
    const W: AccidentCase = AccidentCase::WaterIngress;

    fn bq(n: &str, c: AccidentCase) -> Option<f64> {
        htr10_accident_airborne_release(n, c).map(|a| a.get::<becquerel>())
    }

    #[test]
    fn the_table_has_all_eighteen_published_nuclides_in_order() {
        let names: Vec<&str> = htr10_accident_release().iter().map(|e| e.nuclide).collect();
        assert_eq!(
            names,
            vec![
                "Kr-83m", "Kr-85m", "Kr-85", "Kr-88", "Xe-131m", "Xe-133m", "Xe-133", "Xe-135",
                "I-131", "I-132", "I-133", "I-135", "Sr-90", "Cs-134", "Cs-137", "Ag-110m", "H-3",
                "C-14",
            ],
            "Liu and Cao Table 8"
        );
    }

    #[test]
    fn spot_checks_against_the_published_table() {
        assert_eq!(bq("Kr-83m", D), Some(6.3e8));
        assert_eq!(bq("Kr-83m", W), Some(1.7e8));
        assert_eq!(bq("Xe-133", D), Some(2.2e10));
        assert_eq!(bq("I-131", W), Some(2.2e8));
        assert_eq!(bq("Sr-90", D), Some(1.4e3));
        assert_eq!(bq("Cs-137", W), Some(3.1e8));
        assert_eq!(bq("H-3", D), Some(1.1e10));
        assert_eq!(bq("C-14", D), Some(3.2e6));
        assert_eq!(bq("C-14", W), Some(1.9e4));
    }

    #[test]
    fn the_printed_c4_label_is_not_a_lookup_key_and_absent_is_none() {
        assert_eq!(bq("C-4", D), None);
        // In Tables 1/3/5 but not Table 8.
        assert_eq!(bq("I-134", W), None);
        assert_eq!(bq("Sr-89", D), None);
        assert_eq!(bq("Ar-41", D), None);
    }

    /// Checks the paper's water-ingress assumption ("approximate 23 %" of the
    /// primary-helium activity released) against the stored numbers. Checked,
    /// not forced.
    ///
    /// Methodology: for the species wash-off does not carry (noble gases,
    /// H-3, C-14), divide the Table 8 release by the
    /// Table 3 primary-helium activity (C-14: by the 6.3e4 Bq total the
    /// paper's text states, since Table 3 does not list it). Both numbers
    /// carry two significant figures, so each ratio is uncertain by up to
    /// about ±10 %.
    ///
    /// Result (2026-09-28): the ten ratios lie in 0.295–0.315 (C-14: 0.302).
    /// That is 0.30 ± 5 %, **not** 0.23; 0.23 is outside rounding of every
    /// one of them. The paper does not explain the difference. The band
    /// asserted below is 0.30 ± 10 %, set by rounding, and 0.23 falls outside
    /// it.
    #[test]
    fn water_ingress_noble_gases_h3_and_c14_are_0_30_of_primary_helium() {
        let mut ratios: Vec<(&str, f64)> = htr10_accident_release()
            .iter()
            .filter(|e| {
                e.nuclide.starts_with("Kr") || e.nuclide.starts_with("Xe") || e.nuclide == "H-3"
            })
            .map(|e| {
                let he = htr10_primary_helium_activity(e.nuclide)
                    .expect("in Table 3")
                    .get::<becquerel>();
                (e.nuclide, e.water_ingress.get::<becquerel>() / he)
            })
            .collect();
        ratios.push(("C-14", bq("C-14", W).unwrap() / 6.3e4));
        assert_eq!(ratios.len(), 10);
        for (n, r) in ratios {
            assert!((0.27..=0.33).contains(&r), "{n}: {r}");
            assert!(
                !(0.207..=0.253).contains(&r),
                "{n}: {r} is within 10 % of 0.23"
            );
        }
    }

    /// Records a pattern in the published numbers. Not imposed.
    ///
    /// Result (2026-09-28): the depressurization release of every noble gas
    /// and H-3 is at least the Table 3 primary-helium activity (ratios 1.17
    /// for Kr-83m to 50 for Kr-85), consistent with the whole primary helium
    /// plus the helium-purification-system inventory being released; the
    /// long-lived species (Kr-85, Xe-131m, Xe-133) carry the largest excess.
    #[test]
    fn depressurization_noble_gases_and_h3_exceed_the_primary_helium_activity() {
        for e in htr10_accident_release().iter().filter(|e| {
            e.nuclide.starts_with("Kr") || e.nuclide.starts_with("Xe") || e.nuclide == "H-3"
        }) {
            let he = htr10_primary_helium_activity(e.nuclide)
                .unwrap()
                .get::<becquerel>();
            assert!(e.depressurization.get::<becquerel>() > he, "{}", e.nuclide);
        }
    }

    /// Records a pattern in the published numbers. Not imposed.
    ///
    /// Result (2026-09-28): depressurization releases more of every noble
    /// gas, H-3 and C-14 (total 4.51e10 Bq against 5.67e9 Bq, dominated by
    /// Xe-133 and H-3), while water ingress releases more I-131 (8.8x),
    /// I-133, Sr-90, Cs-134 and Cs-137 (2.4x), the species wash-off of the
    /// steam-generator deposit carries. Iodine in total: 5.94e8 against
    /// 6.80e8 Bq. This is consistent in direction with Table 9's larger
    /// water-ingress thyroid and whole-body doses, but no quantitative link
    /// is claimed.
    #[test]
    fn depressurization_releases_more_gas_water_ingress_more_iodine_131_and_caesium() {
        let t = |c| {
            htr10_accident_release()
                .iter()
                .map(|e| e.release(c).get::<becquerel>())
                .sum::<f64>()
        };
        assert!((t(D) / 4.51e10 - 1.0).abs() < 1e-3);
        assert!((t(W) / 5.67e9 - 1.0).abs() < 1e-3);
        for n in [
            "Kr-83m", "Kr-85m", "Kr-85", "Kr-88", "Xe-131m", "Xe-133m", "Xe-133", "Xe-135", "H-3",
            "C-14",
        ] {
            assert!(bq(n, D) > bq(n, W), "{n}");
        }
        for n in ["I-131", "I-133", "Sr-90", "Cs-134", "Cs-137"] {
            assert!(bq(n, W) > bq(n, D), "{n}");
        }
    }

    #[test]
    fn every_entry_is_positive_and_finite() {
        for e in htr10_accident_release() {
            for c in [D, W] {
                let v = e.release(c).get::<becquerel>();
                assert!(v > 0.0 && v.is_finite(), "{} {c:?} has {v:e}", e.nuclide);
            }
        }
    }
}
