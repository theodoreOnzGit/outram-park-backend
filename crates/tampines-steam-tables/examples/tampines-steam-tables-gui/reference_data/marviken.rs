//! Marviken full-scale critical-flow test 23 / 24 measured envelopes.
//!
//! # Source
//!
//! NUREG/CR-2671 (Marviken full-scale critical-flow tests), Figure 8:24
//! (report p.100 / PDF p.116), for the 500 mm bore, `L/D = 0.3` nozzle;
//! digitised with `graphreader`. Reading uncertainty approximately +/-6 % (RMS
//! 6.3 % against the Table 8:3 endpoints), degrading to about +/-15 % across the
//! steep 3.0-3.3 MPa transition of test 24.
//!
//! # In-repo source fixture
//!
//! `src/steam_turbine_equations/converging_diverging_nozzles/tests/marviken_tests.rs`
//! (`TEST_23_POINTS`, `TEST_24_POINTS`, `TEST_2{3,4}_WATER_TEMPERATURE_DEGC`).
//!
//! # What the numbers are
//!
//! `(nozzle inlet stagnation pressure in kPa, measured nozzle mass flux in
//! kg/(m^2 s))`. The **enthalpy is not measured**: the fixture reconstructs the
//! stagnation enthalpy from the vessel water temperature (NUREG/CR-2671
//! Table 4:2 row 6) as subcooled liquid at that temperature, or as saturated
//! liquid once the vessel water has flashed. This example reproduces exactly
//! that rule, live, from IAPWS-IF97 -- see `curves::marviken_states`.
//!
//! # V&V status -- READ THIS BEFORE CITING
//!
//! Per the crate `CLAUDE.md` and `marviken_tests.rs`: **test 23 is validated**
//! (mean deviation 12.6 %, worst 23.1 %) and **test 24 is NOT validated**
//! (mean -48.5 %, worst -70.2 %, 31 of 40 points outside the band). Test 24 is
//! kept as an honest characterisation case. Do not describe this crate's
//! choked-flow work as Marviken-validated for subcooled stagnation states.
// PROVENANCE / REGENERATION
//
// Every number in this file was copied **verbatim and mechanically** (no hand
// transcription, no rounding, no interpolation) out of the `#[cfg(test)]`
// fixtures named below, which live in this same crate. The fixtures hold their
// data as `let` bindings inside `#[test]` functions, so no library exporter can
// reach them without hoisting ~35 files' worth of data to `const`s; duplicating
// them here, with this citation block, is the alternative sanctioned by the
// task brief for GitHub issue #26.
//
// To re-derive: re-read the cited source file and copy the literal array. The
// extraction is a plain "collect every N-number tuple/array literal inside the
// test function body" pass -- deterministic, and checkable by diffing the
// numbers against the cited file.
//
// If a fixture below is ever edited, THIS FILE GOES STALE. The
// `reference_data_matches_source_counts` test in `mod.rs` pins the row counts
// so a size change is caught; a value change is not, so re-check on edit.

/// One Marviken test's digitised measured envelope.
#[derive(Clone, Copy, Debug)]
pub struct MarvikenTest {
    /// Human-readable label, e.g. `"Marviken test 23"`.
    pub label: &'static str,
    /// Vessel minimum water temperature in degrees Celsius (NUREG/CR-2671
    /// Table 4:2 row 6) -- the temperature of the subcooled zone feeding the
    /// discharge pipe, used to reconstruct the stagnation enthalpy.
    pub water_temperature_degc: f64,
    /// Nominal subcooling in kelvin, as reported.
    pub nominal_subcooling_kelvin: f64,
    /// `true` only for envelopes the crate's own V&V accepts as validated.
    pub validated: bool,
    /// `(stagnation pressure in kPa, measured mass flux in kg/(m^2 s))`.
    pub points: &'static [(f64, f64)],
}

/// Marviken nozzle bore in millimetres (500 mm, `L/D = 0.3`).
pub const MARVIKEN_NOZZLE_BORE_MM: f64 = 500.0;
/// Marviken nozzle length in millimetres.
pub const MARVIKEN_NOZZLE_LENGTH_MM: f64 = 166.0;

/// Both digitised Marviken envelopes.
pub const MARVIKEN_TESTS: &[MarvikenTest] = &[
    MarvikenTest {
        label: "Marviken test 23 (3 K subcooling, VALIDATED)",
        water_temperature_degc: 260.0,
        nominal_subcooling_kelvin: 3.0,
        validated: true,
        points: &[
            (3724.711, 19501.04),
            (3778.902, 19501.04),
            (3829.48, 19501.04),
            (3887.283, 19209.979),
            (3959.538, 19501.04),
            (4024.566, 19646.57),
            (4075.145, 19792.1),
            (4125.723, 19792.1),
            (4190.751, 19792.1),
            (4248.555, 20228.69),
            (4313.584, 20374.22),
            (4385.838, 20956.341),
            (4443.642, 21101.871),
            (4494.22, 21247.401),
            (4537.572, 21392.931),
            (4580.925, 22120.582),
            (4631.503, 22848.233),
            (4667.63, 23284.823),
            (4696.532, 23721.414),
            (4747.11, 24158.004),
            (4797.688, 28669.439),
            (4812.139, 24594.595),
            (4841.04, 25322.245),
            (4877.168, 31725.572),
            (4891.618, 29397.089),
            (4898.844, 27214.137),
            (4913.295, 25467.775),
            (4942.197, 32744.283),
            (4974.711, 33035.343),
        ],
    },
    MarvikenTest {
        label: "Marviken test 24 (33 K subcooling, NOT validated)",
        water_temperature_degc: 230.0,
        nominal_subcooling_kelvin: 33.0,
        validated: false,
        points: &[
            (2828.757, 16735.967),
            (2868.497, 17318.087),
            (2904.624, 16881.497),
            (2947.977, 16735.967),
            (2984.104, 17172.557),
            (3027.457, 22266.112),
            (3049.133, 20083.16),
            (3063.584, 21247.401),
            (3085.26, 20519.751),
            (3121.387, 30124.74),
            (3150.289, 26049.896),
            (3164.74, 27214.137),
            (3193.642, 34636.175),
            (3215.318, 33180.873),
            (3273.121, 35509.356),
            (3287.572, 37546.778),
            (3316.474, 39147.609),
            (3352.601, 40020.79),
            (3417.63, 41767.152),
            (3453.757, 40020.79),
            (3576.59, 43513.514),
            (3612.717, 44386.694),
            (3634.393, 43513.514),
            (3706.647, 44386.694),
            (3778.902, 45405.405),
            (3822.254, 44823.285),
            (3901.734, 45405.405),
            (3916.185, 46424.116),
            (3959.538, 46133.056),
            (4060.694, 47442.827),
            (4075.145, 48898.129),
            (4161.85, 51808.732),
            (4255.78, 50935.551),
            (4306.358, 51808.732),
            (4356.936, 52827.443),
            (4421.965, 53700.624),
            (4515.896, 53409.563),
            (4580.925, 54573.805),
            (4703.757, 54137.214),
            (4772.399, 56611.227),
        ],
    },
];
