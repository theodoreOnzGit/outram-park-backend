//! **Bounding air ingress**, the Map tab's static comparison (#453, epic #450).
//!
//! > **A BOUNDING CASE, NOT A TRANSIENT (see #420).** Nothing here is stepped
//! > by the plant model, and nothing here comes from the running simulation.
//! > The air-ingress O2 supply is unpublished (#420), so the live DLOFC
//! > scenario oxidises nothing and fails no fuel by oxidation. This case
//! > instead ASSUMES the worst: every particle exposed to air at 1400 °C for
//! > 140 h. Research, education and V&V only; not a dose to any real person,
//! > not a siting, emergency or licensing figure (`RESPONSIBLE_USE.md`).
//!
//! # What is computed, and where
//!
//! All of it is `sembawang::lwr_comparison::bounding_comparison`: the **same
//! function** the `sembawang` example `lwr_nureg1465_counterpart` calls
//! (#452), so the map and the example cannot drift apart. This module only
//! caches the result and names its labels.
//!
//! | Piece | Source |
//! |---|---|
//! | oxidation failure f_ox = 1.2e-3 | KORA AVR 92/22, IAEA-TECDOC-978 **Table 5-7**, pinned to the committed row (`sembawang/reference/tecdoc978/`) |
//! | fuel failure over the hold | `boon-lay` (the PANAMA-I report's equations), increment only |
//! | release | TRISO-ATOPS (`boon-lay` fork) from real normal-operation pools, `sembawang` venting (#446: Upstream, checked against FullFlowThrough), plus Liu & Cao circulating activity at 100 % |
//! | LWR arms | RG 1.183 Rev. 1 Table 2 (NUREG-1465 AST) **per 1 %/day** of leak rate, no removal credit; WASH-1400 PWR 8. NuScale DCA Table B-5 inventory scaled to 10 MWth |
//! | dose | `buangkok` single plume, ground release, 1 m/s, worst class, FGR-15/FGR-11, 96 h |
//!
//! **Not the map's live weather.** The table is the worst-class screening of
//! #452 at the map's receptor distances, not a dose along the live plume.
//! **Graphite–air kinetics** stay in [`super::depressurisation`] (Contescu,
//! supply-limited), where the zero O2 supply of #420 keeps them idle.
//! TECDOC-978 Fig. 5-23's 1400 °C Nabielek *prediction* is shown for context
//! only. Fig. 5-18 (compact weight loss) and Fig. 5-21 are committed as data,
//! not used here: 5-18 would check `boon-lay`'s graphite–air kinetics, which
//! is that crate's V&V (#457), and 5-21 repeats the Fig. 5-23 measurements.
//!
//! **The LWR doses are lower bounds**: `buangkok`'s FGR tables miss some
//! nuclides (#456). [`BoundingComparison::incomplete_fraction`] says by how
//! much, and the map prints it.

use std::sync::OnceLock;

use sembawang::htr10::Htr10Geometry;
use sembawang::lwr_comparison::{bound, bounding_comparison, kora, BoundingComparison};
use uom::si::f64::Time;
use uom::si::time::hour;

use super::atmospheric_dispersion::RECEPTOR_DISTANCES_M;

/// Dose window \[h\]: the NRC 96 h the #452 example uses.
pub const WINDOW_H: f64 = 96.0;

/// HTR-10 thermal power \[MWth\], the LWR scaling target: the published
/// 10 MW of `outram_park_digital_twin_engine::htr10::design`.
pub const HTR10_MWTH: f64 = 10.0;

/// The map button.
pub const BUTTON_LABEL: &str = "Bounding air ingress (1400 °C / 140 h, KORA)";

/// Carried wherever the case is shown (maintainer direction, #453).
pub const CASE_LABEL: &str = "Bounding case, not a transient; see #420";

/// The LWR column's label (maintainer direction, #453). NUREG-1465 Table 3.13
/// itself stops at the containment, so the dose arm is its RG 1.183 Rev. 1
/// revision leaking at 1 %/day. `L_a` is plant-specific and not in RG 1.183.
pub const LWR_LABEL: &str = "LWR comparison: NUREG-1465 design source term (as revised in \
     RG 1.183 Rev. 1 Table 2) scaled to 10 MWth, per 1 %/day containment leak (L_a is \
     plant-specific and NOT assumed; no removal credit); different accident physics; see #450";

/// The second LWR column's label.
pub const WASH_LABEL: &str = "WASH-1400 PWR 8: beyond design basis, containment not isolated, \
     no melt -- the closest LWR analogue of this bounding case";

/// HTR-10 particle and pebble geometry, from `tampines` as the #452 example
/// reads it.
fn htr10_geometry() -> Htr10Geometry {
    let particle = tampines::pebble_bed::triso::TrisoParticle::htr10();
    let pebble = tampines::pebble_bed::pebble::Pebble::htr10();
    Htr10Geometry {
        kernel_radius: particle.kernel_radius,
        sic_thickness: particle.silicon_carbide_outer_radius - particle.inner_pyc_outer_radius,
        graphite_thickness: pebble.outer_radius - pebble.fuelled_zone_radius,
    }
}

/// The comparison at the map's receptor distances, computed once per
/// process (it is static: no plant state enters it). `Err` carries the
/// release chain's message, and the map shows it; nothing is substituted.
pub fn comparison() -> &'static Result<BoundingComparison, String> {
    static CELL: OnceLock<Result<BoundingComparison, String>> = OnceLock::new();
    CELL.get_or_init(|| {
        bounding_comparison(
            htr10_geometry(),
            Time::new::<hour>(WINDOW_H),
            HTR10_MWTH,
            &RECEPTOR_DISTANCES_M,
        )
        .map_err(|e| e.to_string())
    })
}

/// One line of provenance for the oxidation failure fraction, read from the
/// committed Table 5-7 row, with Fig. 5-23's Nabielek prediction at the same
/// time for context.
pub fn f_ox_provenance() -> String {
    let row = kora::sphere_test(bound::HOLD_CELSIUS, bound::HOLD_HOURS);
    let nabielek = kora::nabielek_1400c_prediction(bound::HOLD_HOURS);
    match row {
        Some(t) => format!(
            "f_ox = {:.1e}: KORA {} ({} of {} particles failed in air at {} °C for {} h), \
             IAEA-TECDOC-978 Table 5-7 -- measured. Fig. 5-23's Nabielek model predicts {} at \
             {} h (not used; context only).",
            t.failed_fraction,
            t.sample,
            t.failed,
            t.particles,
            t.max_celsius,
            t.hours,
            nabielek.map_or("--".to_string(), |p| format!("{p:.2e}")),
            bound::HOLD_HOURS
        ),
        None => "f_ox: Table 5-7 row not found".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The map's bounding table is `sembawang`'s, at the map's receptor
    /// distances, and has the shape of the #452 result: finite, positive,
    /// falling with distance; HTR-10 has full FGR coverage, the LWR arms do
    /// not (#456). The library's own numbers (400 m: 66.1 / 58.3 / 1510
    /// mSv) are recorded on #452. This is a harness check, not validation.
    ///
    /// Recorded 2026-09-30 (`--bounding-air-ingress`), class F at every
    /// distance, maximum dose over 96 h \[mSv\]:
    ///
    /// | x \[m\] | HTR-10 bound | RG 1.183 per 1 %/d `L_a` | WASH-1400 PWR 8 |
    /// |---:|---:|---:|---:|
    /// | 100 | 709.7 | 16 202 | 626.0 |
    /// | 500 | 45.49 | 1038 | 40.12 |
    /// | 1000 | 14.318 | 326.9 | 12.63 |
    ///
    /// FGR-incomplete share of released Bq: HTR-10 0, RG 1.183 0.284,
    /// WASH-1400 0.035 (the LWR doses are lower bounds, #456). At 100 m a
    /// ground-level Gaussian plume is at the near edge of the
    /// Pasquill-Gifford curves; the number is a screening figure, not a
    /// dose to anyone.
    #[test]
    fn the_bounding_table_is_sembawangs_at_the_map_distances() {
        let c = comparison().as_ref().expect("bounding chain runs");
        assert_eq!(c.rows.len(), RECEPTOR_DISTANCES_M.len());
        for (row, d) in c.rows.iter().zip(RECEPTOR_DISTANCES_M) {
            assert_eq!(row.distance_m, d);
            for v in [
                row.htr10_bound_sv,
                row.rg1183_per_percent_per_day_sv,
                row.wash1400_pwr8_sv,
            ] {
                assert!(v.is_finite() && v > 0.0, "{row:?}");
            }
        }
        for w in c.rows.windows(2) {
            assert!(w[1].htr10_bound_sv < w[0].htr10_bound_sv);
        }
        assert_eq!(c.incomplete_fraction[0], 0.0);
        assert!(c.incomplete_fraction[1] > 0.0 && c.incomplete_fraction[2] > 0.0);
        // 1000 m is also a #452 distance: the same number to the digit printed there.
        let at_1km = c.rows.iter().find(|r| r.distance_m == 1000.0).unwrap();
        assert!((1e3 * at_1km.htr10_bound_sv - 14.318).abs() < 5e-4);
        assert!(f_ox_provenance().contains("AVR 92/22"));
    }
}
