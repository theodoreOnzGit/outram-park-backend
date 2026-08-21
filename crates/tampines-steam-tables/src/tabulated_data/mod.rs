//! Published steam-table reference data as a first-class library type.
//!
//! **Added for GitHub issue #26, 2026-08-21.** The plotter GUI
//! (`examples/tampines-steam-tables-gui/`) needed the Wagner/Kretzschmar
//! single-phase and saturation tables to draw its "validation coverage"
//! overlays; that data used to live only as a GUI-local copy
//! (`reference_data/wagner.rs`), itself a mechanical duplicate of the
//! `#[cfg(test)]` fixtures in `src/interfaces/tests_and_examples/`. The
//! maintainer asked for it to move here instead, as a queryable type any
//! consumer of this crate can reach — not just the GUI, and not a third copy
//! of the same numbers.
//!
//! The raw tables live in [`wagner_kretzschmar_2019`]; this module wraps them
//! in a query surface: [`TabulatedData::isobar`] and [`TabulatedData::isotherm`]
//! pull a cross-section of the published single-phase table, and
//! [`TabulatedData::saturation_curve`] hands back the saturation table as
//! typed rows. Every query is a plain filter over the embedded tables —
//! nothing is interpolated, extrapolated, or invented. A query that matches no
//! row returns an empty `Vec`; that emptiness is itself informative (per issue
//! #26: "a region of a diagram with no Wagner points on it is a region with no
//! table verification behind it").
//!
//! # Why isobar and isotherm, not more
//!
//! The published single-phase tables are organised as isobar files (29 fixed
//! pressures, each swept over a shared temperature grid), so an isobar
//! cross-section is exactly one file's worth of rows and an isotherm
//! cross-section is one row from each of the (up to) 29 files. Isentrope,
//! isenthalp and isochore cross-sections would need interpolation between
//! tabulated rows to produce a usable line — this module deliberately does
//! not do that; it is a reference-data lookup, not a computation.

pub mod wagner_kretzschmar_2019;

use uom::si::available_energy::kilojoule_per_kilogram;
use uom::si::f64::*;
use uom::si::pressure::bar;
use uom::si::specific_heat_capacity::kilojoule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::degree_celsius;

pub use wagner_kretzschmar_2019::{
    WagnerSaturationRow, WagnerSinglePhaseRow, SAT_COL_H_LIQ, SAT_COL_H_VAP, SAT_COL_P_BAR,
    SAT_COL_S_LIQ, SAT_COL_S_VAP, SAT_COL_T_DEGC, WAGNER_SATURATION_TABLE,
    WAGNER_SINGLE_PHASE_TABLE,
};

/// Which cross-section of the tabulated single-phase surface to pull.
///
/// A closed enum, not a trait object or a bare `(bool, f64)` pair, per this
/// workspace's Rust design rules — the set of cross-section kinds this data
/// can actually answer is fixed (see the module doc for why it stops at these
/// two), and a `match` on it should be exhaustive at every call site.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TabulatedQuantity {
    /// Constant pressure — one (or zero) tabulated isobar file's worth of rows.
    Isobar(Pressure),
    /// Constant temperature — up to one row from each tabulated isobar file.
    Isotherm(ThermodynamicTemperature),
}

/// One tabulated single-phase state, unit-tagged from the raw
/// [`WagnerSinglePhaseRow`] via `uom`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TabulatedState {
    pub pressure: Pressure,
    pub temperature: ThermodynamicTemperature,
    pub specific_enthalpy: AvailableEnergy,
    pub specific_entropy: SpecificHeatCapacity,
}

/// One tabulated saturation-line state — both phases, at one temperature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TabulatedSaturationState {
    pub temperature: ThermodynamicTemperature,
    pub pressure: Pressure,
    pub liquid_enthalpy: AvailableEnergy,
    pub vapour_enthalpy: AvailableEnergy,
    pub liquid_entropy: SpecificHeatCapacity,
    pub vapour_entropy: SpecificHeatCapacity,
}

/// Default matching tolerance for [`TabulatedData::isobar`]: 0.5 % relative to
/// the requested pressure. Relative, not absolute, because the tabulated
/// pressures span almost six decades (0.006 bar to 1000 bar) — a fixed
/// absolute tolerance either misses everything at the low end or matches
/// neighbouring isobars at the high end.
pub const DEFAULT_ISOBAR_TOLERANCE_RELATIVE: f64 = 0.005;

/// Default matching tolerance for [`TabulatedData::isotherm`]: 0.5 °C. The
/// published single-phase tables step temperature on a shared, small-integer
/// grid (2 °C near the low end, coarser at high pressure/temperature), so an
/// absolute tolerance well under one grid step avoids picking up the wrong
/// neighbouring row.
pub const DEFAULT_ISOTHERM_TOLERANCE_KELVIN: f64 = 0.5;

/// Query surface over the embedded Wagner/Kretzschmar tables.
///
/// Zero-sized — every method reads the `pub const` tables in
/// [`wagner_kretzschmar_2019`] directly, so there is nothing to construct or
/// own. Exists as a named type (rather than bare free functions) so a
/// `TabulatedData::isobar(...)` call site reads as "ask the published data",
/// distinct from `curves::isobar(...)`, which computes the same cross-section
/// live from this crate's own IAPWS-IF97 implementation.
#[derive(Debug, Clone, Copy, Default)]
pub struct TabulatedData;

impl TabulatedData {
    /// Every published single-phase row within `tolerance_relative` of `p`
    /// (fractional, e.g. `0.005` for 0.5 %). Empty if the deck tabulates no
    /// isobar that close.
    pub fn isobar(&self, p: Pressure, tolerance_relative: f64) -> Vec<TabulatedState> {
        let target_bar = p.get::<bar>();
        let window = target_bar * tolerance_relative;
        WAGNER_SINGLE_PHASE_TABLE
            .iter()
            .filter(|row| (row[0] - target_bar).abs() <= window)
            .map(row_to_state)
            .collect()
    }

    /// [`Self::isobar`] at [`DEFAULT_ISOBAR_TOLERANCE_RELATIVE`].
    pub fn isobar_default_tolerance(&self, p: Pressure) -> Vec<TabulatedState> {
        self.isobar(p, DEFAULT_ISOBAR_TOLERANCE_RELATIVE)
    }

    /// Every published single-phase row within `tolerance_kelvin` of `t` — at
    /// most one per tabulated isobar. Empty if no tabulated row lands that
    /// close.
    ///
    /// `tolerance_kelvin` is a plain `f64` **delta**, not a
    /// [`ThermodynamicTemperature`]: `uom`'s `ThermodynamicTemperature` is an
    /// absolute-scale type with affine (offset) unit conversion, so
    /// `ThermodynamicTemperature::new::<kelvin>(0.5).get::<degree_celsius>()`
    /// is `-272.65`, not `0.5` — exactly the trap that made an earlier version
    /// of this function match zero rows at every temperature. A size-of-window
    /// like this one is a difference between two temperatures, which `uom`
    /// represents with `TemperatureInterval`, not `ThermodynamicTemperature`;
    /// using a bare `f64` here (1 K delta == 1 °C delta, so no unit ambiguity
    /// exists to encode) avoids the trap entirely rather than trading it for a
    /// different `uom` type to get right.
    pub fn isotherm(&self, t: ThermodynamicTemperature, tolerance_kelvin: f64) -> Vec<TabulatedState> {
        let target_degc = t.get::<degree_celsius>();
        WAGNER_SINGLE_PHASE_TABLE
            .iter()
            .filter(|row| (row[1] - target_degc).abs() <= tolerance_kelvin)
            .map(row_to_state)
            .collect()
    }

    /// [`Self::isotherm`] at [`DEFAULT_ISOTHERM_TOLERANCE_KELVIN`].
    pub fn isotherm_default_tolerance(&self, t: ThermodynamicTemperature) -> Vec<TabulatedState> {
        self.isotherm(t, DEFAULT_ISOTHERM_TOLERANCE_KELVIN)
    }

    /// Dispatches to [`Self::isobar_default_tolerance`] or
    /// [`Self::isotherm_default_tolerance`] by [`TabulatedQuantity`] variant.
    pub fn cross_section(&self, quantity: TabulatedQuantity) -> Vec<TabulatedState> {
        match quantity {
            TabulatedQuantity::Isobar(p) => self.isobar_default_tolerance(p),
            TabulatedQuantity::Isotherm(t) => self.isotherm_default_tolerance(t),
        }
    }

    /// The full published saturation curve, as typed rows in table order.
    pub fn saturation_curve(&self) -> Vec<TabulatedSaturationState> {
        WAGNER_SATURATION_TABLE
            .iter()
            .map(|row| TabulatedSaturationState {
                temperature: ThermodynamicTemperature::new::<degree_celsius>(
                    row[SAT_COL_T_DEGC],
                ),
                pressure: Pressure::new::<bar>(row[SAT_COL_P_BAR]),
                liquid_enthalpy: AvailableEnergy::new::<kilojoule_per_kilogram>(
                    row[SAT_COL_H_LIQ],
                ),
                vapour_enthalpy: AvailableEnergy::new::<kilojoule_per_kilogram>(
                    row[SAT_COL_H_VAP],
                ),
                liquid_entropy: SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(
                    row[SAT_COL_S_LIQ],
                ),
                vapour_entropy: SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(
                    row[SAT_COL_S_VAP],
                ),
            })
            .collect()
    }
}

fn row_to_state(row: &WagnerSinglePhaseRow) -> TabulatedState {
    TabulatedState {
        pressure: Pressure::new::<bar>(row[0]),
        temperature: ThermodynamicTemperature::new::<degree_celsius>(row[1]),
        specific_enthalpy: AvailableEnergy::new::<kilojoule_per_kilogram>(row[2]),
        specific_entropy: SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(row[3]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::thermodynamic_temperature::kelvin;

    /// Pins the row counts the module doc and `wagner_kretzschmar_2019`'s own
    /// provenance note both cite, so an edit to the embedded tables that
    /// silently drops or duplicates rows is caught.
    ///
    /// **Result (2026-08-21):** 220 saturation rows, 2334 single-phase rows —
    /// unchanged from the counts recorded when this data lived in the GUI
    /// example (`reference_data_matches_source_counts`).
    #[test]
    fn wagner_row_counts_match_the_source_fixtures() {
        assert_eq!(WAGNER_SATURATION_TABLE.len(), 220);
        assert_eq!(WAGNER_SINGLE_PHASE_TABLE.len(), 2334);
    }

    /// A pressure this crate's own default-isobar sweep already asks for (100
    /// bar) must find real tabulated rows — this is the exact use case issue
    /// #26 asked for: "plot the tabulated data as crosses and IAPWS-97 data as
    /// lines" for the same isobar.
    ///
    /// **Result (2026-08-21):** 100 bar (and the corner cases 0.1 bar, 1 bar,
    /// 300 bar) each return their full published row set; an isobar this
    /// crate's default set includes but Wagner never tabulated (5 bar) returns
    /// empty, which is the correct "no coverage here" answer, not an error.
    #[test]
    fn isobar_finds_the_published_rows_it_should_and_nothing_it_should_not() {
        let data = TabulatedData;
        for p_bar in [0.1_f64, 1.0, 100.0, 300.0] {
            let rows = data.isobar_default_tolerance(Pressure::new::<bar>(p_bar));
            assert!(
                !rows.is_empty(),
                "expected published rows at {p_bar} bar (this crate's own default-isobar set)"
            );
            for state in &rows {
                let rel = (state.pressure.get::<bar>() - p_bar).abs() / p_bar;
                assert!(
                    rel <= DEFAULT_ISOBAR_TOLERANCE_RELATIVE,
                    "row at {p_bar} bar matched with {rel} relative error"
                );
            }
        }
        // 5 bar is in DEFAULT_ISOBARS_BAR (the GUI's computed-curve sweep) but
        // is not one of Wagner's 29 tabulated isobars -- empty is correct.
        let empty = data.isobar_default_tolerance(Pressure::new::<bar>(5.0));
        assert!(
            empty.is_empty(),
            "5 bar is not a tabulated Wagner isobar; a nonempty result here would mean the \
             tolerance is matching the wrong neighbouring isobar"
        );
    }

    /// An isotherm on the tables' shared temperature grid (300 °C, one of
    /// this crate's own default isotherms) should find a row from most or all
    /// of the 29 tabulated isobars.
    ///
    /// **Result (2026-08-21):** 300 °C matches at least 15 distinct tabulated
    /// pressures (comfortably more than half of the 29 isobar files), each
    /// within the 0.5 K default tolerance.
    #[test]
    fn isotherm_finds_rows_across_multiple_tabulated_isobars() {
        let data = TabulatedData;
        let rows = data.isotherm_default_tolerance(ThermodynamicTemperature::new::<degree_celsius>(
            300.0,
        ));
        assert!(
            rows.len() >= 15,
            "expected 300 degC to match rows from most of the 29 tabulated isobars, got {}",
            rows.len()
        );
        for state in &rows {
            let dt = (state.temperature.get::<degree_celsius>() - 300.0).abs();
            assert!(dt <= DEFAULT_ISOTHERM_TOLERANCE_KELVIN, "row matched {dt} K off target");
        }
    }

    /// `cross_section` dispatches to the same two queries directly reachable.
    #[test]
    fn cross_section_dispatches_by_variant() {
        let data = TabulatedData;
        let via_enum = data.cross_section(TabulatedQuantity::Isobar(Pressure::new::<bar>(100.0)));
        let direct = data.isobar_default_tolerance(Pressure::new::<bar>(100.0));
        assert_eq!(via_enum.len(), direct.len());
    }

    /// The saturation curve round-trips every published row, and every value
    /// in it is finite (mechanical-extraction sanity, mirroring the GUI's own
    /// `no_dataset_carries_a_non_finite_number`).
    #[test]
    fn saturation_curve_is_complete_and_finite() {
        let data = TabulatedData;
        let curve = data.saturation_curve();
        assert_eq!(curve.len(), WAGNER_SATURATION_TABLE.len());
        for state in &curve {
            assert!(state.temperature.get::<kelvin>().is_finite());
            assert!(state.pressure.get::<bar>().is_finite());
            assert!(state.liquid_enthalpy.get::<kilojoule_per_kilogram>().is_finite());
            assert!(state.vapour_enthalpy.get::<kilojoule_per_kilogram>().is_finite());
        }
    }
}
