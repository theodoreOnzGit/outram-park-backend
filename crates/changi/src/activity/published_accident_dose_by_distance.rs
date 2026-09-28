// SPDX-License-Identifier: GPL-3.0-only
//! **A published dose-versus-distance table for two HTR-10 design-basis
//! accidents, stored as reference data. Nothing here computes a dose.**
//!
//! # What this is
//!
//! The source's individual dose to a member of the public, in **mSv** (a
//! dose per accident, not a rate), at thirteen distances from 0.25 km to
//! 75 km from the release point. It gives two organ/body quantities,
//! **thyroid** and **whole-body**, for each of two accidents:
//!
//! - **Depressurization accident** ([`AccidentCase::Depressurization`]):
//!   loss of primary helium through a ruptured 65 mm fuel-element charging
//!   tube. The paper's Section 4.1.1 sums four sources: the primary-helium
//!   activity, fission products desorbed from primary-circuit surfaces, dust
//!   mobilised from "dead-water regions", and activity bound in the helium
//!   purification system. Fission products in the coated particles are
//!   taken as not released, because the paper's cited transient analysis
//!   puts peak fuel temperature at 1033 °C, below the 1600 °C limit.
//! - **Water ingress accident** ([`AccidentCase::WaterIngress`]): a two-ended
//!   rupture of two steam-generator heat-transfer tubes with the steam relief
//!   system failing, admitting at most 129.9 kg of water. The paper's
//!   Section 4.1.2 sums three sources: about 23 % of the primary-helium
//!   activity, wash-off of the activity deposited on the steam generator,
//!   and activity in up to 4.88 kg of corroded graphite.
//!
//! The paper names these two as the design-basis accidents that lead to the
//! largest potential dose to the public. The releases behind this table are
//! the paper's Table 8, which is **not** digitised in this workspace. In both
//! cases the release goes out through the 40 m exhaust stack, and the paper
//! credits no filtering and no plate-out in the reactor building.
//!
//! It is a **published model result, not a measurement.** The source
//! calculated it with the German code **STOERNEU**. The paper's Section 4.2
//! states this basis:
//!
//! - **Pathways:** gamma and beta submersion, gamma radiation from
//!   contaminated ground, inhalation, and ingestion.
//! - **Geometry:** a 40 m stack; the reactor building is 28 m high and 30 m
//!   wide.
//!
//! **The paper does NOT state**, for this table: the integration period of
//! the dose (the table is in mSv, with no time basis), the receptor age
//! group, the meteorology or dispersion conditions, whether the values are
//! for a worst azimuth, the dose coefficients, or whether "whole-body" means
//! effective dose. Do not read any of those into it. In particular, do not
//! assume Table 7's "azimuth of maximum dose" or its adult receptor carry
//! over; Table 7 was computed with a different code (AIRDOS-EPA).
//!
//! # What the paper says about it
//!
//! Comparing this table with its Table 10 (the emergency intervention levels
//! of Chinese Nuclear Safety Criterion HAD 002/03), the paper concludes the
//! doses are much lower than the lowest sheltering level, so no intervention
//! (evacuation, sheltering or stable iodine) would be needed even for the
//! worst of the accidents it analysed. Table 10's lowest sheltering levels
//! are 5 mSv whole-body and 50 mSv for the thyroid and other important
//! organs. The test
//! `every_dose_is_below_the_lowest_sheltering_level_the_paper_compares_against`
//! checks that statement against the stored numbers. It holds: the largest
//! whole-body dose (0.20 mSv, water ingress, 0.25 km) is 25 times below 5 mSv,
//! and the largest thyroid dose (1.1 mSv, same case and distance) is about 45
//! times below 50 mSv. That is the paper's comparison, reproduced; it is not
//! an endorsement of it and not an emergency-planning finding of this
//! workspace.
//!
//! # What this is NOT
//!
//! - **Not computed here, and not wired into any model.** Nothing in this
//!   crate or in `htgr_sim_v1` reads it. `changi` still computes no dose
//!   quantity (see [`crate::activity`]).
//! - **Parked here, not settled here.** Dose is to live in the placeholder
//!   crate `buangkok` eventually, but the maintainer (2026-09-28) has asked
//!   for the dose tables to stay in `changi` until they decide. Do not move
//!   it, and do not build dose computation around it, unasked.
//! - **Not a basis for emergency planning, emergency-zone sizing, siting,
//!   licensing or any safety decision**, for HTR-10 or any other plant.
//!   `RESPONSIBLE_USE.md` applies in full. This workspace uses it for
//!   research-grade safety analysis only: a published number that a future
//!   research calculation may be compared with.
//! - **Not a beyond-design-basis result.** It covers the two design-basis
//!   accidents above, with no release from the coated particles.
//!
//! # Units
//!
//! Distance is a `uom` [`Length`](uom::si::f64::Length). **The doses are
//! plain `f64` in mSv**, and the field names say so, for the reason given in
//! [`crate::activity::published_dose_by_distance`]: `uom` 0.38 has no
//! sievert quantity, and `AvailableEnergy` (J/kg) was rejected on purpose.
//!
//! # Provenance
//!
//! Liu Yuanzhong and Cao Jianzhu, *"Fission product release and its
//! environment impact for normal reactor operations and for relevant
//! accidents"*, **Nuclear Engineering and Design 218 (2002) 81–90**, Table 9
//! (p. 88), "Individual doses caused by accidents of the HTR-10 (mSv)". The
//! basis above comes from the paper's Sections 4.1–4.2 (pp. 86–89).
//!
//! Access terms, digitisation and verification are in
//! `crates/changi/docs/References.md`. The document carries no reuse licence
//! and is **not** redistributed here. Only the cited table of 65 numbers is,
//! which is ordinary scientific citation.

use uom::si::f64::Length;
use uom::si::length::kilometer;

/// The table, compiled in so a missing file is a build error rather than a
/// silently empty table.
const HTR10_ACCIDENT_DOSE_BY_DISTANCE_CSV: &str =
    include_str!("../../reference/htr10_accident_individual_dose_by_distance.csv");

/// Which of the paper's two tabulated accidents a dose belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccidentCase {
    /// Primary-circuit depressurization through a ruptured 65 mm
    /// fuel-element charging tube (paper Section 4.1.1).
    Depressurization,
    /// Water ingress through two ruptured steam-generator tubes with the
    /// steam relief system failed (paper Section 4.1.2).
    WaterIngress,
}

/// The two doses the table gives for one accident at one distance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PublishedAccidentDoses {
    /// Thyroid dose, in **millisieverts**, as published. The paper says only
    /// "Thyroid".
    pub thyroid_msv: f64,
    /// Whole-body dose, in **millisieverts**, as published. The paper says
    /// only "Whole-body"; it does not say whether this is an effective dose.
    pub whole_body_msv: f64,
}

/// One row of the published accident dose-versus-distance table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PublishedAccidentDoseAtDistance {
    /// Distance from the release point (the stack). The paper does not say
    /// along which azimuth.
    pub distance: Length,
    /// Doses for the depressurization accident.
    pub depressurization: PublishedAccidentDoses,
    /// Doses for the water ingress accident.
    pub water_ingress: PublishedAccidentDoses,
}

impl PublishedAccidentDoseAtDistance {
    /// The doses for one accident case at this distance.
    #[must_use]
    pub fn doses(&self, case: AccidentCase) -> PublishedAccidentDoses {
        match case {
            AccidentCase::Depressurization => self.depressurization,
            AccidentCase::WaterIngress => self.water_ingress,
        }
    }
}

/// Every row of the published HTR-10 accident dose-versus-distance table, in
/// the source's order (increasing distance, 0.25 km to 75 km).
#[must_use]
pub fn htr10_accident_dose_by_distance() -> Vec<PublishedAccidentDoseAtDistance> {
    HTR10_ACCIDENT_DOSE_BY_DISTANCE_CSV
        .lines()
        .skip(1)
        .filter_map(|line| {
            // A row that does not parse is dropped; the row-count test
            // catches that.
            let v: Vec<f64> = line
                .split(',')
                .map(|f| f.trim().parse::<f64>())
                .collect::<Result<_, _>>()
                .ok()?;
            if v.len() != 5 {
                return None;
            }
            Some(PublishedAccidentDoseAtDistance {
                distance: Length::new::<kilometer>(v[0]),
                depressurization: PublishedAccidentDoses {
                    thyroid_msv: v[1],
                    whole_body_msv: v[2],
                },
                water_ingress: PublishedAccidentDoses {
                    thyroid_msv: v[3],
                    whole_body_msv: v[4],
                },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CASES: [AccidentCase; 2] = [AccidentCase::Depressurization, AccidentCase::WaterIngress];

    fn km(r: &PublishedAccidentDoseAtDistance) -> f64 {
        r.distance.get::<kilometer>()
    }

    #[test]
    fn the_table_has_all_thirteen_published_distances() {
        let d: Vec<f64> = htr10_accident_dose_by_distance().iter().map(km).collect();
        assert_eq!(
            d,
            vec![0.25, 0.75, 1.5, 2.5, 4.0, 7.5, 15.0, 25.0, 35.0, 45.0, 55.0, 65.0, 75.0],
            "Liu and Cao Table 9"
        );
    }

    #[test]
    fn spot_checks_against_the_published_table() {
        let r = htr10_accident_dose_by_distance();
        let row = |i: usize| {
            let x = r[i];
            (
                km(&x),
                x.depressurization.thyroid_msv,
                x.depressurization.whole_body_msv,
                x.water_ingress.thyroid_msv,
                x.water_ingress.whole_body_msv,
            )
        };
        assert_eq!(row(0), (0.25, 1.7e-1, 7.7e-2, 1.1, 2.0e-1));
        assert_eq!(row(4), (4.0, 1.4e-2, 6.3e-3, 8.7e-2, 1.7e-2));
        assert_eq!(row(6), (15.0, 9.8e-4, 6.6e-4, 4.6e-3, 1.6e-3));
        assert_eq!(row(12), (75.0, 2.4e-6, 2.0e-6, 8.0e-6, 4.7e-6));
        assert_eq!(
            r[1].doses(AccidentCase::WaterIngress),
            PublishedAccidentDoses {
                thyroid_msv: 5.2e-1,
                whole_body_msv: 8.8e-2
            }
        );
    }

    /// The paper's own comparison (Section 4.2 with its Table 10): every dose
    /// is below the lowest sheltering level of HAD 002/03, 5 mSv whole-body
    /// and 50 mSv thyroid. Checked, not forced.
    ///
    /// Result (2026-09-28): holds for all 52 doses. Largest whole-body:
    /// 0.20 mSv (water ingress, 0.25 km), 25x below 5 mSv. Largest thyroid:
    /// 1.1 mSv (water ingress, 0.25 km), ~45x below 50 mSv. This reproduces
    /// the paper's argument; it is not an emergency-planning finding.
    #[test]
    fn every_dose_is_below_the_lowest_sheltering_level_the_paper_compares_against() {
        // HAD 002/03 lower bounds for sheltering, as printed in the paper's
        // Table 10: whole-body 5–50 mSv; lung, thyroid and other important
        // organs 50–500 mSv.
        const SHELTERING_WHOLE_BODY_LOWER_MSV: f64 = 5.0;
        const SHELTERING_THYROID_LOWER_MSV: f64 = 50.0;
        let r = htr10_accident_dose_by_distance();
        let all = |f: fn(&PublishedAccidentDoses) -> f64| {
            r.iter()
                .flat_map(|x| CASES.map(|c| f(&x.doses(c))))
                .fold(0.0_f64, f64::max)
        };
        let max_wb = all(|d| d.whole_body_msv);
        let max_th = all(|d| d.thyroid_msv);
        assert_eq!(max_wb, 2.0e-1);
        assert_eq!(max_th, 1.1);
        assert!(max_wb < SHELTERING_WHOLE_BODY_LOWER_MSV);
        assert!(max_th < SHELTERING_THYROID_LOWER_MSV);
    }

    /// Records what the published curves do. Not a constraint imposed on the
    /// data.
    ///
    /// Result (2026-09-28): all four columns are largest at the nearest
    /// tabulated distance, 0.25 km, and fall strictly at every later
    /// distance. That differs from the normal-operation Table 7, which peaks
    /// at 1.5 km; the paper does not discuss either shape, and Table 9 was
    /// computed with a different code on an unstated meteorological basis.
    #[test]
    fn every_column_falls_strictly_from_0_25_km() {
        let r = htr10_accident_dose_by_distance();
        for case in CASES {
            for w in r.windows(2) {
                let (a, b) = (w[0].doses(case), w[1].doses(case));
                assert!(b.thyroid_msv < a.thyroid_msv, "{case:?} thyroid at {} km", km(&w[1]));
                assert!(
                    b.whole_body_msv < a.whole_body_msv,
                    "{case:?} whole-body at {} km",
                    km(&w[1])
                );
            }
        }
    }

    /// Records a pattern in the published numbers. Not imposed.
    ///
    /// Result (2026-09-28): at every distance, water ingress gives the larger
    /// dose in both columns, and in both accidents the thyroid dose exceeds
    /// the whole-body dose. Water ingress thyroid / depressurization thyroid
    /// is 6.5x at 0.25 km, peaks at 7.1x at 1.5 km, and falls to 3.3x at
    /// 75 km.
    #[test]
    fn water_ingress_exceeds_depressurization_and_thyroid_exceeds_whole_body() {
        for x in htr10_accident_dose_by_distance() {
            let (d, w) = (x.depressurization, x.water_ingress);
            assert!(w.thyroid_msv > d.thyroid_msv, "{} km", km(&x));
            assert!(w.whole_body_msv > d.whole_body_msv, "{} km", km(&x));
            assert!(d.thyroid_msv > d.whole_body_msv, "{} km", km(&x));
            assert!(w.thyroid_msv > w.whole_body_msv, "{} km", km(&x));
        }
    }

    #[test]
    fn every_entry_is_positive_and_finite() {
        for x in htr10_accident_dose_by_distance() {
            assert!(km(&x) > 0.0 && km(&x).is_finite());
            for c in CASES {
                let d = x.doses(c);
                assert!(d.thyroid_msv > 0.0 && d.thyroid_msv.is_finite());
                assert!(d.whole_body_msv > 0.0 && d.whole_body_msv.is_finite());
            }
        }
    }
}
