//! Edwards-O'Brien pipe blowdown reference data.
//!
//! # Source
//!
//! Tomlinson & Aumiller, B-T-3271 -- the Edwards-O'Brien (1970) horizontal pipe
//! blowdown benchmark. Table 1 gives the pipe geometry and the Hendrie
//! non-isothermal initial temperature profile; Figure 3 gives the measured
//! pressure history at gauge station GS-1.
//!
//! # In-repo source fixture
//!
//! `tests/edwards_blowdown.rs` (`NODE_CENTRE_FT`, `NODE_T_DEGF`,
//! `GS1_DATA_PSIA`, `P_INIT_PA`).
//!
//! # What is, and is not, a thermodynamic state
//!
//! * **`EDWARDS_NODE_T_DEGF` IS a full state.** Combined with the measured
//!   initial pressure (1000 psig = 7.0 MPa absolute) each of the 24 nodes is a
//!   real, measured `(p, T)` initial condition, so it plots on every diagram.
//! * **`EDWARDS_GS1_DATA_PSIA` is NOT a full state.** It is a measured
//!   *pressure* history only -- neither enthalpy, entropy nor quality was
//!   measured, and the crate's own blowdown trajectory is a 6.5-minute
//!   simulation output, not data. It is therefore offered **only** on the T-p
//!   diagram, paired with the IAPWS-IF97 saturation temperature at each measured
//!   pressure (which the UI labels as a computed, not measured, ordinate), and
//!   the layer is **disabled** on the p-h, T-s and h-s tabs rather than being
//!   filled with an invented enthalpy or quality.
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

/// Initial pipe pressure, 1000 psig expressed as absolute pascals.
pub const EDWARDS_P_INIT_PA: f64 = 7.0e6;
/// Containment / ambient back-pressure in pascals.
pub const EDWARDS_P_AMBIENT_PA: f64 = 1.0e5;
/// Total pipe length in metres (13.44 ft).
pub const EDWARDS_PIPE_LENGTH_M: f64 = 4.096;
/// Pipe inside diameter in metres (2.88 in).
pub const EDWARDS_PIPE_ID_M: f64 = 0.073;

/// Volume-centre axial position of each of the 24 nodes, in feet from the
/// closed end (B-T-3271 Table 1).
pub const EDWARDS_NODE_CENTRE_FT: [f64; 24] = [
    0.260, 0.780, 1.300, 1.820, 2.385, 3.000, 3.615, 4.215, 4.820, 5.425, 6.025, 6.640, 7.255,
    7.820, 8.340, 8.940, 9.630, 10.320, 10.920, 11.440, 11.905, 12.370, 12.890, 13.295,
];

/// Measured initial temperature of each of the 24 nodes, in degrees Fahrenheit
/// (the Hendrie non-isothermal profile, B-T-3271 Table 1).
pub const EDWARDS_NODE_T_DEGF: [f64; 24] = [
    447.5, 448.4, 449.4, 450.3, 451.4, 452.5, 451.7, 450.8, 450.0, 449.2, 448.3, 447.5, 448.0,
    448.5, 448.9, 449.4, 450.0, 446.5, 443.4, 440.8, 438.4, 436.0, 437.0, 437.8,
];

/// Digitised Edwards experimental pressure at gauge station GS-1,
/// `(time in seconds, pressure in psia)`, traced from B-T-3271 Figure 3 with
/// graphreader.com and subsampled every 0.02 s. **Pressure only -- see the
/// module doc: this is not a thermodynamic state.**
pub const EDWARDS_GS1_DATA_PSIA: [(f64, f64); 16] = [
    (0.000, 985.0),
    (0.020, 350.777),
    (0.040, 364.038),
    (0.060, 367.358),
    (0.080, 343.024),
    (0.100, 312.239),
    (0.120, 298.234),
    (0.140, 296.0),
    (0.160, 288.68),
    (0.180, 288.394),
    (0.200, 289.0),
    (0.220, 283.112),
    (0.240, 271.433),
    (0.260, 252.96),
    (0.280, 228.0),
    (0.300, 190.093),
];
