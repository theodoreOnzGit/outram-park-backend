//! **HTR-10 vs LWR comparison**, the Map tab's static table behind the
//! "Bounding air ingress" toggle (#453, epic #450).
//!
//! ~~**Framing (maintainer decisions, 2026-09-30, #450):** the PRIMARY columns
//! are **design basis against design basis** ... The SECONDARY columns are the
//! **beyond-design-basis bounding** pair: the KORA bound (below) and
//! WASH-1400 PWR 8.~~ **CHANGED 2026-09-30 (maintainer decision, #464): paired
//! by initiating event and severity**, DLOFC (HTR) vs LOCA (LWR), columns in
//! tier order (`sembawang::lwr_comparison::ARM_COLUMNS`):
//!
//! - **Design basis: DLOFC vs LOCA.** HTR-10 depressurisation (Liu & Cao 2002
//!   Table 8) vs the LWR MHA LOCA (RG 1.183 Rev. 1, NuScale inventory scaled
//!   to 10 MWth, `L_a` 0.20 %/day, no removal; natural deposition pending
//!   literature).
//! - **Beyond design basis: DLOFC + air ingress (bounding) vs LOCA + core
//!   melt.** The KORA bound (below) vs the LWR LOCA with ECCS failure:
//!   NUREG-1465 Table 3.13, all phases, into an intact containment leaking at
//!   `L_a` (a *contained* core melt; no early failure or bypass).
//! - **Context:** WASH-1400 PWR 8, no-melt, uncontained; paired by
//!   containment state, not initiator.
//!
//! **Crediting basis (maintainer decision, 2026-09-30):** "The comparison
//! neglects the pools because we are comparing technology at the reactor
//! level, not what is surrounding the reactor. If NuScale gives pool credit,
//! then HTGR can be submerged in a pool as well." Each side is credited only
//! with its reactor-level inherent barrier: the containment vessel leaking at
//! `L_a` (LWR), the TRISO particles (HTR). Nothing surrounding the reactor is
//! credited on either side (NuScale's pool, building, sprays, filters; the
//! HTR confinement/building, #409, or a pool). Pool scrubbing is excluded by
//! design, not pending literature ([`BASIS_LABEL`]).
//!
//! > **A BOUNDING CASE, NOT A TRANSIENT (see #420).** Nothing here is stepped
//! > by the plant model, and nothing here comes from the running simulation.
//! > ~~The air-ingress O2 supply is unpublished (#420), so the live DLOFC
//! > scenario oxidises nothing and fails no fuel by oxidation.~~ **Since
//! > 2026-09-30** the live DLOFC has air ingress via the Gao & Shi
//! > cavity-ventilation rate (#420), which oxidises bed graphite; it still
//! > fails no fuel by oxidation (no KORA law in the live model). This case
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
//! | LWR arms | ~~RG 1.183 Rev. 1 Table 2 (NUREG-1465 AST) **per 1 %/day** of leak rate~~ **CORRECTED 2026-09-30** (the code leaks at NuScale's `L_a` = 0.20 %/day since 3236b50dc3): design-basis LOCA, RG 1.183 Rev. 1 Table 2; LOCA + core melt, NUREG-1465 Table 3.13 all phases; both at `L_a` 0.20 %/day, no removal credit; WASH-1400 PWR 8 (context). NuScale DCA Table B-5 inventory scaled to 10 MWth |
//! | dose | `buangkok` single plume, ground release, 1 m/s, worst class, FGR-15/FGR-11, 96 h |
//!
//! **Not the map's live weather.** The table is the worst-class screening of
//! #452 at the map's receptor distances, not a dose along the live plume.
//! **Graphite–air kinetics** stay in [`super::depressurisation`] (Contescu,
//! supply-limited), ~~where the zero O2 supply of #420 keeps them idle~~
//! fed since 2026-09-30 by the Gao & Shi cavity ventilation (#420).
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
use sembawang::lwr_comparison::{
    bound, bounding_comparison, kora, BoundingComparison, NaturalDeposition,
    CONTAINED_CORE_MELT_ASSUMPTION, NATURAL_DEPOSITION_PENDING, REACTOR_LEVEL_BASIS,
};
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

/// The HTR-10 design-basis column's label (#452).
pub const DBA_LABEL: &str = "HTR-10 design basis: DLOFC, depressurisation (DN65 charging-tube \
     rupture), Liu & Cao 2002 Table 8 published release";

/// The LWR design-basis column's label. (The tier headings, formerly
/// `BDB_LABEL` here, are the library's `Tier::label` since 2026-09-30,
/// #464, so the map and the headless CSV read one copy.) ~~"LWR comparison: NUREG-1465 design
/// source term (as revised in RG 1.183 Rev. 1 Table 2) scaled to 10 MWth, per
/// 1 %/day containment leak (L_a is plant-specific and NOT assumed; no removal
/// credit); different accident physics; see #450"~~ **CHANGED 2026-09-30**
/// (maintainer decision, #450): NuScale's `L_a` is now sourced.
pub const LWR_LABEL: &str = "LWR comparison, design basis: NUREG-1465 source term as revised \
     in RG 1.183 Rev. 1 (MHA LOCA), NuScale inventory scaled to 10 MWth, L_a 0.20 %/day \
     (NRC Phase 4 SER Ch. 6), 24 h then half; no removal credit; different accident \
     physics; see #450";

/// The LWR severe-LOCA column's label (#464).
pub const SEVERE_LABEL: &str = "LWR comparison, beyond design basis: LOCA with ECCS failure -> \
     core melt, NUREG-1465 Table 3.13 PWR all phases (Table 3.6 timing), NuScale inventory \
     scaled to 10 MWth, into an INTACT containment leaking at L_a 0.20 %/day, 24 h then half; \
     no removal credit; see #464";

/// The context column's label. ~~"WASH-1400 PWR 8: beyond design basis,
/// containment not isolated, no melt -- the closest LWR analogue of the KORA
/// bounding case"~~ **CHANGED 2026-09-30 (#464):** context only, paired by
/// containment state, not initiator.
pub const WASH_LABEL: &str = "Context: WASH-1400 PWR 8 -- no-melt, uncontained (containment \
     not isolated); paired by containment state, not initiator";

/// The contained-core-melt assumption of the severe-LOCA column.
pub const CONTAINED_LABEL: &str = CONTAINED_CORE_MELT_ASSUMPTION;

/// The crediting basis (maintainer decision, 2026-09-30).
pub const BASIS_LABEL: &str = REACTOR_LEVEL_BASIS;

/// Why the natural-deposition column is empty.
pub const DEPOSITION_LABEL: &str = NATURAL_DEPOSITION_PENDING;

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
            &NaturalDeposition::PENDING_LITERATURE,
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
    /// distances: the cached [`comparison`] EQUALS a fresh call of
    /// `sembawang::lwr_comparison::bounding_comparison` (the same library
    /// function, no copy), and has the shape of the #452/#464 result: finite,
    /// positive, falling with distance; the LWR LOCA + core melt column at or
    /// above the design-basis LOCA column; HTR-10's KORA bound has full FGR
    /// coverage, the LWR arms do not (#456). This is a harness check, not
    /// validation.
    ///
    /// ~~The library's own numbers (400 m: 66.1 / 58.3 / 1510 mSv) are
    /// recorded on #452.~~ **CORRECTED 2026-09-30:** 1510 mSv was the LWR DBA
    /// per 1 %/day of `L_a`; the arm has leaked at 0.20 %/day since
    /// 3236b50dc3 (301 mSv at 400 m).
    ///
    /// ~~Recorded 2026-09-30, before #464:~~ (100 m: 4.50e-2 / 3234 / pending
    /// / 709.7 / 626.0; 500 m: 2.88e-3 / 207.3 / pending / 45.49 / 40.12;
    /// 1000 m: 9.08e-4 / 65.2 / pending / 14.318 / 12.63 mSv). The LWR DBA
    /// column moved -0.08 % on 2026-09-30 when the containment integrator's
    /// source became step-overlap weighted (#464).
    ///
    /// Recorded 2026-09-30 after #464 (`--bounding-air-ingress`), class F at
    /// every distance, maximum dose over 96 h \[mSv\], columns in tier order:
    ///
    /// | x \[m\] | DB: HTR-10 DLOFC | DB: LWR LOCA, no removal | DB: nat. dep. | BDB: HTR-10 KORA bound | BDB: LWR LOCA + core melt, no removal | BDB: nat. dep. | Context: WASH-1400 PWR 8 |
    /// |---:|---:|---:|---:|---:|---:|---:|---:|
    /// | 100 | 4.50e-2 | 3231 | pending | 709.7 | 10095 | pending | 626.0 |
    /// | 500 | 2.88e-3 | 207.1 | pending | 45.49 | 647.0 | pending | 40.12 |
    /// | 1000 | 9.08e-4 | 65.18 | pending | 14.318 | 203.7 | pending | 12.63 |
    ///
    /// The LWR doses are lower bounds (#456). At 100 m a ground-level
    /// Gaussian plume is at the near edge of the Pasquill-Gifford curves; the
    /// numbers are screening figures, not a dose to anyone.
    #[test]
    fn the_bounding_table_is_sembawangs_at_the_map_distances() {
        let c = comparison().as_ref().expect("bounding chain runs");
        let fresh = bounding_comparison(
            htr10_geometry(),
            Time::new::<hour>(WINDOW_H),
            HTR10_MWTH,
            &RECEPTOR_DISTANCES_M,
            &NaturalDeposition::PENDING_LITERATURE,
        )
        .expect("bounding chain runs");
        assert_eq!(c, &fresh, "the map's rows are the library's rows");
        assert_eq!(c.rows.len(), RECEPTOR_DISTANCES_M.len());
        for (row, d) in c.rows.iter().zip(RECEPTOR_DISTANCES_M) {
            assert_eq!(row.distance_m, d);
            assert_eq!(
                row.lwr_dba_natural_deposition_sv, None,
                "pending literature"
            );
            assert_eq!(
                row.lwr_severe_loca_natural_deposition_sv, None,
                "pending literature"
            );
            for v in [
                row.htr10_dba_depressurisation_sv,
                row.lwr_dba_no_removal_sv,
                row.htr10_bound_sv,
                row.lwr_severe_loca_no_removal_sv,
                row.wash1400_pwr8_sv,
            ] {
                assert!(v.is_finite() && v > 0.0, "{row:?}");
            }
            assert!(row.lwr_severe_loca_no_removal_sv >= row.lwr_dba_no_removal_sv);
        }
        for w in c.rows.windows(2) {
            assert!(w[1].htr10_bound_sv < w[0].htr10_bound_sv);
        }
        assert_eq!(c.incomplete.htr10_bound, 0.0);
        assert!(
            c.incomplete.lwr_dba > 0.0
                && c.incomplete.lwr_severe_loca > 0.0
                && c.incomplete.wash1400 > 0.0
        );
        // 1000 m is also a #452 distance: the same number to the digit printed there.
        let at_1km = c.rows.iter().find(|r| r.distance_m == 1000.0).unwrap();
        assert!((1e3 * at_1km.htr10_bound_sv - 14.318).abs() < 5e-4);
        assert!(f_ox_provenance().contains("AVR 92/22"));
    }
}
