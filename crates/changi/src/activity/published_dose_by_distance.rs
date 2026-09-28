// SPDX-License-Identifier: GPL-3.0-only
//! **A published dose-versus-distance table for HTR-10 normal operation,
//! stored as reference data. Nothing here computes a dose.**
//!
//! # What this is
//!
//! The source's individual effective dose to an adult member of the public,
//! in mSv per year, at twelve distances from 0.5 km to 75 km from the release
//! point. It is given only along the azimuth where the dose is largest. It is
//! the dose the same paper calculates from its annual normal-operation
//! airborne release (its Table 5, in [`crate::activity::airborne_release`]).
//!
//! It is a **published model result, not a measurement.** The source
//! calculated it with the US EPA code **AIRDOS-EPA** (Moore et al., 1979),
//! which the authors say they partly modified. The paper states this basis:
//!
//! - **Release:** routine airborne effluent from one year of normal
//!   operation, from a 40 m exhaust stack (the reactor building is 12 m) with
//!   a 9 m/s exit velocity. It is the unfiltered, conservative release of the
//!   paper's Table 5.
//! - **Pathways:** gamma submersion in the plume, gamma radiation from
//!   contaminated ground, inhalation, and ingestion.
//! - **Receptor:** adults. The food-consumption rates are the source's
//!   Table 6, which is not reproduced here.
//! - **Meteorology:** measurements from an observatory 7.5 km from the site.
//!   The dose differs by direction, and the table gives only the direction of
//!   maximum dose.
//! - **Dose factors:** USDOE/EH-0070 (1988) and IAEA (1996).
//!
//! # What this is NOT
//!
//! - **Not computed here, and not wired into any model.** Nothing in this
//!   crate or in `htgr_sim_v1` reads it. `changi` still computes no dose
//!   quantity (see [`crate::activity`]). This module stores numbers a
//!   published paper printed. It does not bring dose assessment into this
//!   crate's current scope. That is a maintainer decision taken in
//!   `RESPONSIBLE_USE.md`.
//! - **Not an accident dose.** The same paper tabulates accident doses
//!   separately (its Table 9), which is not digitised here.
//! - `RESPONSIBLE_USE.md` applies in full. Nothing here may be quoted as a
//!   dose to the public from HTR-10 or any other plant for any operational,
//!   licensing, siting, emergency-planning or safety purpose.
//!
//! # Units
//!
//! Distance is a `uom` [`Length`](uom::si::f64::Length). **The dose is a plain `f64` in mSv per
//! year**, and the field name says so. `uom` 0.38 has no sievert quantity:
//! there is no equivalent-dose or absorbed-dose module in `uom::si`. It was
//! checked 2026-09-28, and no crate in this workspace defines one either.
//! `AvailableEnergy` (J/kg) has the right dimension but was rejected on
//! purpose. A sievert is J/kg only after radiation and tissue weighting, and
//! a type that lets a dose be added to a specific energy would hide that.
//!
//! # Provenance
//!
//! Liu Yuanzhong and Cao Jianzhu, *"Fission product release and its
//! environment impact for normal reactor operations and for relevant
//! accidents"*, **Nuclear Engineering and Design 218 (2002) 81–90**, Table 7
//! (p. 87, printed sideways), "Individual effective doses (mSv a⁻¹) to the
//! public at various distance (km) from the release point in the azimuth
//! where the maximum dose occurs". The basis above comes from the paper's
//! Section 3.2 (pp. 85–86).
//!
//! Access terms, digitisation and verification are in
//! `crates/changi/docs/References.md`. The document carries no reuse licence
//! and is **not** redistributed here. Only the cited table of 12 values is,
//! which is ordinary scientific citation.

use uom::si::f64::Length;
use uom::si::length::kilometer;

/// The table, compiled in so a missing file is a build error rather than a
/// silently empty table.
const HTR10_NORMAL_OPERATION_DOSE_BY_DISTANCE_CSV: &str =
    include_str!("../../reference/htr10_normal_operation_individual_dose_by_distance.csv");

/// One row of the published dose-versus-distance table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PublishedDoseAtDistance {
    /// Distance from the release point (the stack), along the azimuth of
    /// maximum dose.
    pub distance: Length,
    /// Individual effective dose to an adult member of the public, in
    /// **millisieverts per year**, as published. A plain `f64` because `uom`
    /// has no sievert quantity (see the module docs).
    pub effective_dose_msv_per_year: f64,
}

/// Every row of the published HTR-10 normal-operation dose-versus-distance
/// table, in the source's order (increasing distance, 0.5 km to 75 km).
#[must_use]
pub fn htr10_normal_operation_dose_by_distance() -> Vec<PublishedDoseAtDistance> {
    HTR10_NORMAL_OPERATION_DOSE_BY_DISTANCE_CSV
        .lines()
        .skip(1)
        .filter_map(|line| line.split_once(','))
        .filter_map(|(km, msv)| {
            // A row that does not parse is dropped; the row-count test
            // catches that.
            let km = km.trim().parse::<f64>().ok()?;
            let msv = msv.trim().parse::<f64>().ok()?;
            Some(PublishedDoseAtDistance {
                distance: Length::new::<kilometer>(km),
                effective_dose_msv_per_year: msv,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<(f64, f64)> {
        htr10_normal_operation_dose_by_distance()
            .into_iter()
            .map(|r| (r.distance.get::<kilometer>(), r.effective_dose_msv_per_year))
            .collect()
    }

    #[test]
    fn the_table_has_all_twelve_published_distances() {
        let d: Vec<f64> = rows().iter().map(|r| r.0).collect();
        assert_eq!(
            d,
            vec![0.5, 1.5, 2.5, 4.0, 7.5, 15.0, 25.0, 35.0, 45.0, 55.0, 65.0, 75.0],
            "Liu and Cao Table 7"
        );
    }

    #[test]
    fn spot_checks_against_the_published_table() {
        let r = rows();
        assert_eq!(r[0], (0.5, 1.1e-4));
        assert_eq!(r[3], (4.0, 7.6e-5));
        assert_eq!(r[5], (15.0, 2.5e-5));
        assert_eq!(r[11], (75.0, 7.7e-6));
    }

    /// Checks the table against the one number the paper's text states about
    /// it: a maximum individual effective dose of 1.4×10⁻⁴ mSv/a (Section 3.2).
    ///
    /// The text does not say where the maximum is. The table puts it at
    /// **1.5 km**, and this test records that. It is the only row with that
    /// value.
    #[test]
    fn the_maximum_is_the_value_the_text_states_and_sits_at_1_5_km() {
        let r = rows();
        let max = r
            .iter()
            .copied()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        assert_eq!(max, (1.5, 1.4e-4));
    }

    /// Records what the published curve does. It is not a physics constraint
    /// imposed on the data.
    ///
    /// Result (2026-09-28): the dose **rises** from 0.5 km (1.1e-4) to a peak
    /// at 1.5 km (1.4e-4). It then **falls strictly** at every later
    /// distance, reaching 7.7e-6 at 75 km. That is 18 times below the peak.
    /// A peak off the stack is what an elevated (40 m) release gives. The
    /// paper does not discuss the shape.
    #[test]
    fn the_curve_rises_to_1_5_km_then_falls_strictly() {
        let r = rows();
        assert!(r[0].1 < r[1].1, "rises from 0.5 km to 1.5 km");
        for w in r[1..].windows(2) {
            assert!(w[1].1 < w[0].1, "falls from {} km to {} km", w[0].0, w[1].0);
        }
    }

    #[test]
    fn every_entry_is_positive_and_finite() {
        for (km, msv) in rows() {
            assert!(km > 0.0 && km.is_finite());
            assert!(msv > 0.0 && msv.is_finite(), "{km} km has {msv:e}");
        }
    }
}
