//! # SHRT-17 and SHRT-45R: test descriptions, initial conditions and the
//! # tabulated transient boundary conditions
//!
//! Source: Sumner & Wei (2012), ANL-ARC-226 Rev. 1, *Benchmark
//! Specifications and Data Requirements for EBR-II Shutdown Heat Removal
//! Tests SHRT-17 and SHRT-45R* (A226; PDF page = printed page + 16). Every
//! table below is transcribed in `reference-data/ebr2/` and pinned to it by
//! `ebr2::csv_equality`. **AI transcription, not yet reviewed by a human**
//! (2026-11-10).
//!
//! The raw tables keep the source's units (s, rpm, MW, gpm, F, A). Use
//! [`ShrtTest`]'s methods for uom quantities, linearly interpolated in time.
//!
//! **Interpolation (a choice, not the source's):** piecewise linear between
//! tabulated points, held constant before the first and after the last
//! point (900 s, or 899.5 s for the auxiliary EM pump). The specification
//! prints the tables; it does not say how participants should interpolate
//! them.
//!
//! **Flow units.** Primary flows are printed in gpm calibrated at 800 F and
//! intermediate flows in gpm at 582 F. [`gpm_to_mass_rate`] converts with
//! the density of sodium at the calibration temperature (CoolProp `LiqNa`
//! through TUAS).
//!
//! **Source discrepancy, SHRT-45R intermediate flow.** Table 3.2.4.2.1
//! (p.36) starts at 5079.5 gpm, and its unit is printed "gpm at XX F";
//! Table 3.2.1.1 (p.24) gives 5474.5 gpm at 582 F as the initial
//! intermediate flow. This module uses **the table**, at **582 F**: it is the
//! time-dependent boundary condition, so using it keeps the initial state and
//! the transient on one source, and 582 F is the calibration temperature of
//! every other intermediate flow in A226. The 7.2 % difference is an
//! uncertainty of the boundary condition, and is recorded in the V&V doc.

use uom::si::angular_velocity::revolution_per_minute;
use uom::si::electric_current::ampere;
use uom::si::f64::*;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::power::megawatt;
use uom::si::thermodynamic_temperature::degree_fahrenheit;
use uom::si::time::second;
use uom::si::volume_rate::cubic_meter_per_second;

/// The two Shutdown Heat Removal Tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShrtTest {
    /// SHRT-17 (20 June 1984, Run 129C): **protected** loss of flow, pumps
    /// tripped and scram (A226 Table 3.1.1.1 p.9).
    Shrt17,
    /// SHRT-45R (3 April 1986, Run 138B): **unprotected** loss of flow, no
    /// scram, auxiliary EM pump on battery (A226 Table 3.2.1.1 p.24).
    Shrt45r,
}

// ---------------------------------------------------------------------------
// Test descriptions and initial conditions
// ---------------------------------------------------------------------------

/// Initial power, MW: SHRT-17 57.3 (from ANL-EBR-132), SHRT-45R 60.0.
/// A226 Tables 3.1.1.1 p.9 and 3.2.1.1 p.24 **(CSV)**.
///
/// SHRT-17's power table (Table 3.1.4.2.1 p.19) totals 57.29 MW at t = 0;
/// the model uses the table, which is the boundary condition.
pub const INITIAL_POWER_MW: [f64; 2] = [57.3, 60.0];

/// Initial primary flow, gpm at 800 F: SHRT-17 8500 (through the core),
/// SHRT-45R 8971.9 (at the pumps; 8766.6 through the core, A226 p.26).
/// A226 Tables 3.1.1.1 p.9 and 3.2.1.1 p.24 **(CSV)**.
pub const INITIAL_PRIMARY_FLOW_GPM_800F: [f64; 2] = [8500.0, 8971.9];

/// SHRT-45R initial flows, gpm at 800 F, A226 p.26 **(CSV, text)**:
/// `[pump 1, pump 2, Z-pipe inlet, core]` = 4489.1, 4482.8, 8703.2, 8766.6.
pub const SHRT45R_INITIAL_FLOWS_GPM_800F: [f64; 4] = [4489.1, 4482.8, 8703.2, 8766.6];

/// Initial intermediate flow, gpm at 582 F: SHRT-17 5614.6, SHRT-45R
/// 5474.5. A226 Tables 3.1.1.1 p.9 and 3.2.1.1 p.24 **(CSV)**. (See the
/// module doc for the SHRT-45R disagreement with its own table.)
pub const INITIAL_INTERMEDIATE_FLOW_GPM_582F: [f64; 2] = [5614.6, 5474.5];

/// Initial core inlet temperature, F: SHRT-17 664.8, SHRT-45R 650.8.
/// A226 Tables 3.1.1.1 p.9 and 3.2.1.1 p.24 **(CSV)**.
pub const INITIAL_CORE_INLET_F: [f64; 2] = [664.8, 650.8];

/// IHX intermediate-side inlet temperature, F: SHRT-17 573.8, SHRT-45R
/// 552.1. A226 pp.11, 26 **(CSV, text)**. (SHRT-17's table starts at
/// 574.02 F.)
pub const INITIAL_IHX_INTERMEDIATE_INLET_F: [f64; 2] = [573.8, 552.1];

/// Pump discharge pressure excluding gravity head, psi: SHRT-17 pump 1
/// 42.9, pump 2 41.9; SHRT-45R pump 1 41.84 (pump 2 not measured, assume
/// equal). A226 pp.11, 26 **(CSV, text)**: `[SHRT-17 #1, SHRT-17 #2,
/// SHRT-45R #1]`.
pub const PUMP_DISCHARGE_PRESSURE_PSI: [f64; 3] = [42.9, 41.9, 41.84];

/// Upper plenum pressure, psi: SHRT-17 6.36, SHRT-45R 6.51. A226 pp.11, 26
/// **(CSV, text)**.
pub const UPPER_PLENUM_PRESSURE_PSI: [f64; 2] = [6.36, 6.51];

/// SHRT-17 initial power split, MW: fission 53.93, decay heat 3.36 (ANS LWR
/// standard). A226 p.17 **(CSV, text)**.
pub const SHRT17_INITIAL_FISSION_DECAY_MW: [f64; 2] = [53.93, 3.36];

/// SHRT-45R assumed pre-test history, A226 p.24 **(CSV, text)** and
/// A228 p.8 **(CSV)**: shut down about 1.5 months, then `[power MW, days]`
/// = 17.9 MW for 4.03 d, then 60.0 MW for 1.64 d (177 MWd).
pub const SHRT45R_PRE_TEST_HISTORY: [[f64; 2]; 2] = [[17.9, 4.03], [60.0, 1.64]];

// ---------------------------------------------------------------------------
// Tabulated boundary conditions (generated from the CSVs, 2026-11-10)
// ---------------------------------------------------------------------------

/// SHRT-17 primary pump speeds, A226 Table 3.1.4.1.1 p.14 **(CSV)**.
/// Columns `[time s, pump 1 rpm, pump 2 rpm]`; 26 rows, from `reference-data/ebr2/arc226_table3_1_4_1_1.csv`.
pub const SHRT17_PUMP_SPEEDS: [[f64; 3]; 26] = [
    [0.0, 799.03, 764.92],
    [2.0, 727.77, 695.42],
    [4.0, 566.79, 532.61],
    [6.5, 407.49, 379.64],
    [9.0, 323.18, 301.59],
    [12.5, 244.06, 230.18],
    [17.5, 172.50, 167.39],
    [24.5, 109.25, 113.98],
    [33.0, 56.46, 74.62],
    [42.0, 0.69, 46.54],
    [51.0, 0.76, 2.29],
    [90.0, 0.76, 2.44],
    [120.0, 0.61, 2.44],
    [180.0, 0.76, 2.44],
    [240.0, 0.84, 2.59],
    [300.0, 0.61, 2.44],
    [360.0, 0.61, 2.44],
    [420.0, 0.61, 2.44],
    [480.0, 0.61, 2.44],
    [540.0, 0.76, 2.44],
    [600.0, 0.76, 2.75],
    [660.0, 0.69, 2.44],
    [720.0, 0.69, 2.44],
    [780.0, 0.76, 2.59],
    [840.0, 0.76, 2.44],
    [900.0, 0.92, 2.44],
];

/// SHRT-17 fission, decay-heat and total power, A226 Table 3.1.4.2.1 p.19
/// **(CSV)**. Fission is the measured normalised fission power scaled to
/// 53.93 MW; decay heat is calculated with the ANS (LWR) standard.
/// IAEA-TECDOC-1819 Table 76 (p.438) prints 0.09 MW fission at 300 s where
/// this table prints 0.10; this table is used (it is the specification).
/// Columns `[time s, fission MW, decay heat MW, total MW]`; 28 rows, from `reference-data/ebr2/arc226_table3_1_4_2_1_power.csv`.
pub const SHRT17_POWER: [[f64; 4]; 28] = [
    [0.0, 53.93, 3.36, 57.29],
    [1.0, 8.24, 3.12, 11.36],
    [2.0, 6.86, 2.95, 9.81],
    [3.5, 5.61, 2.77, 8.38],
    [5.0, 4.76, 2.63, 7.39],
    [7.5, 3.82, 2.46, 6.29],
    [10.0, 3.23, 2.33, 5.56],
    [15.0, 2.47, 2.15, 4.62],
    [22.5, 1.85, 1.95, 3.80],
    [30.0, 1.46, 1.82, 3.28],
    [40.0, 1.10, 1.70, 2.79],
    [50.0, 0.84, 1.60, 2.44],
    [60.0, 0.66, 1.52, 2.19],
    [90.0, 0.38, 1.35, 1.73],
    [120.0, 0.25, 1.24, 1.49],
    [180.0, 0.13, 1.09, 1.22],
    [240.0, 0.10, 1.00, 1.10],
    [300.0, 0.10, 0.93, 1.02],
    [360.0, 0.08, 0.88, 0.96],
    [420.0, 0.08, 0.83, 0.92],
    [480.0, 0.08, 0.80, 0.88],
    [540.0, 0.08, 0.77, 0.84],
    [600.0, 0.08, 0.74, 0.82],
    [660.0, 0.08, 0.71, 0.79],
    [720.0, 0.07, 0.69, 0.76],
    [780.0, 0.08, 0.67, 0.75],
    [840.0, 0.07, 0.65, 0.72],
    [900.0, 0.07, 0.63, 0.70],
];

/// SHRT-17 intermediate IHX inlet flow, A226 Table 3.1.4.2.1 (number reused
/// in the source) p.21 **(CSV)**.
/// Columns `[time s, flow gpm at 582 F]`; 21 rows, from `reference-data/ebr2/arc226_table3_1_4_2_1_ihx_flow.csv`.
pub const SHRT17_INTERMEDIATE_FLOW: [[f64; 2]; 21] = [
    [0.0, 5619.55],
    [1.0, 4221.68],
    [2.5, 2834.03],
    [6.0, 1593.92],
    [16.5, 760.20],
    [30.0, 511.05],
    [60.0, 383.93],
    [120.0, 328.28],
    [180.0, 329.81],
    [240.0, 343.09],
    [300.0, 371.17],
    [360.0, 392.10],
    [420.0, 397.20],
    [480.0, 403.33],
    [540.0, 398.22],
    [600.0, 424.77],
    [660.0, 399.76],
    [720.0, 369.12],
    [780.0, 357.38],
    [840.0, 340.53],
    [900.0, 310.92],
];

/// SHRT-17 intermediate IHX inlet temperature, A226 Table 3.1.4.2.2 p.21
/// **(CSV)**.
/// Columns `[time s, temperature F]`; 16 rows, from `reference-data/ebr2/arc226_table3_1_4_2_2.csv`.
pub const SHRT17_INTERMEDIATE_INLET_TEMPERATURE: [[f64; 2]; 16] = [
    [0.0, 574.02],
    [60.0, 573.86],
    [120.0, 573.65],
    [180.0, 573.73],
    [240.0, 573.37],
    [300.0, 572.63],
    [360.0, 572.17],
    [420.0, 571.53],
    [480.0, 571.02],
    [540.0, 570.87],
    [600.0, 570.26],
    [660.0, 568.82],
    [720.0, 566.43],
    [780.0, 563.69],
    [840.0, 560.39],
    [900.0, 557.83],
];

/// SHRT-45R primary pump speeds, A226 Table 3.2.4.1.1 p.29 **(CSV)**. The
/// small negative speeds after 95 s are printed so and kept.
/// Columns `[time s, pump 1 rpm, pump 2 rpm]`; 30 rows, from `reference-data/ebr2/arc226_table3_2_4_1_1.csv`.
pub const SHRT45R_PUMP_SPEEDS: [[f64; 3]; 30] = [
    [0.0, 792.05, 762.27],
    [1.0, 769.10, 739.07],
    [1.5, 731.19, 701.21],
    [3.0, 606.46, 575.65],
    [4.5, 511.19, 482.21],
    [7.0, 397.55, 373.96],
    [11.0, 283.90, 266.69],
    [17.0, 213.66, 198.62],
    [25.5, 154.38, 144.98],
    [36.5, 105.14, 100.40],
    [48.5, 72.67, 68.91],
    [64.5, 46.45, 42.42],
    [82.0, 25.45, 19.69],
    [90.0, 17.13, 1.38],
    [95.0, 10.39, -0.67],
    [98.5, -0.67, -0.67],
    [120.0, -1.06, -0.99],
    [180.0, -0.72, -0.67],
    [240.0, -0.67, -0.53],
    [300.0, -1.26, -1.23],
    [360.0, -1.53, -1.38],
    [420.0, -0.87, -0.77],
    [480.0, -0.70, -0.50],
    [540.0, -1.60, -1.50],
    [600.0, -1.55, -1.57],
    [660.0, -1.55, -1.50],
    [720.0, -1.33, -1.31],
    [780.0, -0.96, -0.87],
    [840.0, -1.72, -1.62],
    [900.0, -1.28, -1.14],
];

/// SHRT-45R auxiliary EM pump data, A226 Table 3.2.4.1.2 p.31 **(CSV)**.
/// Columns `[time s, current A, rectifier voltage V, terminal voltage V, impedance micro-ohm]`; 32 rows, from `reference-data/ebr2/arc226_table3_2_4_1_2.csv`.
pub const SHRT45R_AUX_EM_PUMP: [[f64; 5]; 32] = [
    [0.0, 5797.14, 0.784, 0.234, 40.04],
    [13.5, 6121.30, 0.761, 0.207, 34.11],
    [47.0, 6200.54, 0.749, 0.188, 30.41],
    [79.5, 6186.53, 0.743, 0.181, 29.18],
    [121.5, 6167.36, 0.737, 0.167, 27.07],
    [123.2, 6172.51, 0.738, 0.188, 30.48],
    [147.0, 6130.74, 0.735, 0.186, 30.24],
    [254.5, 6075.52, 0.724, 0.178, 29.48],
    [372.5, 6026.88, 0.712, 0.174, 29.96],
    [532.0, 5999.42, 0.700, 0.168, 28.36],
    [597.5, 5989.97, 0.692, 0.170, 28.38],
    [616.0, 6002.85, 0.691, 0.172, 28.57],
    [623.0, 6064.94, 0.702, 0.175, 28.68],
    [628.0, 6579.34, 0.794, 0.195, 29.88],
    [633.0, 6597.94, 0.794, 0.199, 30.04],
    [640.5, 7322.64, 0.918, 0.231, 31.29],
    [644.5, 7445.66, 0.943, 0.237, 31.95],
    [651.0, 8936.83, 1.205, 0.305, 34.05],
    [659.5, 8945.13, 1.210, 0.305, 34.07],
    [667.0, 9119.65, 1.239, 0.309, 34.26],
    [671.5, 9438.08, 1.293, 0.323, 34.61],
    [680.0, 9442.66, 1.295, 0.325, 34.65],
    [681.5, 9335.09, 1.279, 0.323, 34.55],
    [685.0, 9359.97, 1.280, 0.324, 34.33],
    [686.5, 9203.76, 1.253, 0.314, 34.37],
    [701.5, 9210.34, 1.254, 0.315, 34.40],
    [702.5, 9111.64, 1.240, 0.311, 33.82],
    [741.0, 9118.79, 1.243, 0.312, 34.29],
    [743.0, 9203.19, 1.255, 0.316, 34.31],
    [792.0, 9244.39, 1.264, 0.318, 34.03],
    [842.0, 9261.55, 1.268, 0.311, 33.45],
    [899.5, 9238.09, 1.265, 0.311, 33.59],
];

/// SHRT-45R intermediate IHX inlet flow, A226 Table 3.2.4.2.1 p.36 **(CSV)**;
/// unit printed "gpm at XX F" (see the module doc).
/// Columns `[time s, flow gpm]`; 31 rows, from `reference-data/ebr2/arc226_table3_2_4_2_1.csv`.
pub const SHRT45R_INTERMEDIATE_FLOW: [[f64; 2]; 31] = [
    [0.0, 5079.5],
    [0.5, 4756.0],
    [1.0, 3772.6],
    [1.5, 3118.3],
    [3.0, 2382.0],
    [5.0, 1632.5],
    [7.5, 1297.4],
    [12.0, 886.9],
    [16.0, 709.6],
    [22.5, 562.4],
    [32.0, 456.2],
    [46.0, 398.3],
    [62.5, 361.0],
    [80.0, 371.7],
    [98.5, 360.1],
    [120.0, 382.9],
    [150.0, 411.4],
    [180.0, 422.0],
    [210.0, 430.8],
    [240.0, 433.5],
    [300.0, 457.8],
    [360.0, 455.5],
    [420.0, 450.8],
    [480.0, 463.3],
    [540.0, 482.1],
    [600.0, 472.8],
    [660.0, 455.4],
    [720.0, 449.5],
    [780.0, 442.7],
    [840.0, 418.2],
    [900.0, 429.9],
];

/// SHRT-45R intermediate IHX inlet temperature, A226 Table 3.2.4.2.2 p.37
/// **(CSV)**.
/// Columns `[time s, temperature F]`; 16 rows, from `reference-data/ebr2/arc226_table3_2_4_2_2.csv`.
pub const SHRT45R_INTERMEDIATE_INLET_TEMPERATURE: [[f64; 2]; 16] = [
    [0.0, 552.1],
    [60.0, 551.8],
    [120.0, 551.9],
    [180.0, 551.7],
    [240.0, 551.3],
    [300.0, 550.5],
    [360.0, 549.6],
    [420.0, 549.4],
    [480.0, 549.1],
    [540.0, 546.6],
    [600.0, 543.0],
    [660.0, 538.8],
    [720.0, 534.2],
    [780.0, 530.6],
    [840.0, 528.8],
    [900.0, 527.9],
];

// ---------------------------------------------------------------------------
// Interpolation and typed access
// ---------------------------------------------------------------------------

/// Linear interpolation of column `column` of a table whose column 0 is
/// time in seconds, held constant outside the tabulated span. The rows must
/// be in increasing time (all of A226's are; a test checks it).
pub fn interpolate<const M: usize>(table: &[[f64; M]], column: usize, t_s: f64) -> f64 {
    let first = table[0];
    let last = table[table.len() - 1];
    if !(t_s > first[0]) {
        return first[column];
    }
    if t_s >= last[0] {
        return last[column];
    }
    let i = table.partition_point(|row| row[0] <= t_s);
    let (a, b) = (table[i - 1], table[i]);
    let f = (t_s - a[0]) / (b[0] - a[0]);
    a[column] + f * (b[column] - a[column])
}

/// The US gallon, exactly 3.785411784e-3 m3 (NIST SP 811, Appendix B).
/// uom 0.38's `gallon_per_minute` uses 3.785412e-3 (rounded to seven
/// figures, 5.7e-8 relative), so the exact value is applied here instead.
pub const US_GALLON_M3: f64 = 3.785_411_784e-3;

/// Sodium mass flow for a volumetric flow in US gpm calibrated at
/// `calibration_temperature`: `rho(T_cal) Q`, with CoolProp `LiqNa`
/// (TUAS `liquid_sodium`, valid 400-1000 K).
pub fn gpm_to_mass_rate(
    gpm: f64,
    calibration_temperature: ThermodynamicTemperature,
) -> Result<MassRate, tuas_boussinesq_solver::tuas_lib_error::TuasLibError> {
    let rho = super::model::sodium_density(calibration_temperature)?;
    Ok(rho * VolumeRate::new::<cubic_meter_per_second>(gpm * US_GALLON_M3 / 60.0))
}

/// 800 F, the calibration temperature of A226's primary flows.
pub fn primary_flow_calibration_temperature() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_fahrenheit>(800.0)
}

/// 582 F, the calibration temperature of A226's intermediate flows.
pub fn intermediate_flow_calibration_temperature() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_fahrenheit>(582.0)
}

impl ShrtTest {
    fn index(self) -> usize {
        match self {
            ShrtTest::Shrt17 => 0,
            ShrtTest::Shrt45r => 1,
        }
    }

    /// The test's name as printed.
    pub fn name(self) -> &'static str {
        match self {
            ShrtTest::Shrt17 => "SHRT-17",
            ShrtTest::Shrt45r => "SHRT-45R",
        }
    }

    /// The last tabulated time, 900 s for both tests.
    pub fn end_time(self) -> Time {
        Time::new::<second>(900.0)
    }

    /// Primary pump speeds `(pump 1, pump 2)` at time `t`.
    pub fn pump_speeds(self, t: Time) -> (AngularVelocity, AngularVelocity) {
        let ts = t.get::<second>();
        let (a, b) = match self {
            ShrtTest::Shrt17 => (
                interpolate(&SHRT17_PUMP_SPEEDS, 1, ts),
                interpolate(&SHRT17_PUMP_SPEEDS, 2, ts),
            ),
            ShrtTest::Shrt45r => (
                interpolate(&SHRT45R_PUMP_SPEEDS, 1, ts),
                interpolate(&SHRT45R_PUMP_SPEEDS, 2, ts),
            ),
        };
        (
            AngularVelocity::new::<revolution_per_minute>(a),
            AngularVelocity::new::<revolution_per_minute>(b),
        )
    }

    /// Intermediate IHX inlet flow (gpm at 582 F) at time `t`.
    pub fn intermediate_flow_gpm(self, t: Time) -> f64 {
        let ts = t.get::<second>();
        match self {
            ShrtTest::Shrt17 => interpolate(&SHRT17_INTERMEDIATE_FLOW, 1, ts),
            ShrtTest::Shrt45r => interpolate(&SHRT45R_INTERMEDIATE_FLOW, 1, ts),
        }
    }

    /// Intermediate IHX inlet mass flow at time `t` (gpm at 582 F x sodium
    /// density at 582 F).
    pub fn intermediate_flow(
        self,
        t: Time,
    ) -> Result<MassRate, tuas_boussinesq_solver::tuas_lib_error::TuasLibError> {
        gpm_to_mass_rate(
            self.intermediate_flow_gpm(t),
            intermediate_flow_calibration_temperature(),
        )
    }

    /// Intermediate IHX inlet temperature at time `t`.
    pub fn intermediate_inlet_temperature(self, t: Time) -> ThermodynamicTemperature {
        let ts = t.get::<second>();
        let f = match self {
            ShrtTest::Shrt17 => interpolate(&SHRT17_INTERMEDIATE_INLET_TEMPERATURE, 1, ts),
            ShrtTest::Shrt45r => interpolate(&SHRT45R_INTERMEDIATE_INLET_TEMPERATURE, 1, ts),
        };
        ThermodynamicTemperature::new::<degree_fahrenheit>(f)
    }

    /// SHRT-17's tabulated `(fission, decay heat)` power at time `t`; `None`
    /// for SHRT-45R, whose power is a **result** of the test, not a boundary
    /// condition (A226 p.25; gh:#1315).
    pub fn tabulated_power(self, t: Time) -> Option<(Power, Power)> {
        let ts = t.get::<second>();
        match self {
            ShrtTest::Shrt17 => Some((
                Power::new::<megawatt>(interpolate(&SHRT17_POWER, 1, ts)),
                Power::new::<megawatt>(interpolate(&SHRT17_POWER, 2, ts)),
            )),
            ShrtTest::Shrt45r => None,
        }
    }

    /// Auxiliary EM pump current at time `t`: SHRT-45R's Table 3.2.4.1.2;
    /// zero for SHRT-17, whose auxiliary pump was unpowered (Table 3.1.1.1).
    pub fn aux_em_pump_current(self, t: Time) -> ElectricCurrent {
        let amps = match self {
            ShrtTest::Shrt17 => 0.0,
            ShrtTest::Shrt45r => interpolate(&SHRT45R_AUX_EM_PUMP, 1, t.get::<second>()),
        };
        ElectricCurrent::new::<ampere>(amps)
    }

    /// The test-description initial power (Table 3.1.1.1 / 3.2.1.1).
    pub fn initial_power(self) -> Power {
        Power::new::<megawatt>(INITIAL_POWER_MW[self.index()])
    }

    /// The test-description initial core inlet temperature.
    pub fn initial_core_inlet_temperature(self) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_fahrenheit>(INITIAL_CORE_INLET_F[self.index()])
    }

    /// The test-description initial primary flow (gpm at 800 F; SHRT-17
    /// through the core, SHRT-45R at the pumps) as a mass flow.
    pub fn initial_primary_flow(
        self,
    ) -> Result<MassRate, tuas_boussinesq_solver::tuas_lib_error::TuasLibError> {
        gpm_to_mass_rate(
            INITIAL_PRIMARY_FLOW_GPM_800F[self.index()],
            primary_flow_calibration_temperature(),
        )
    }

    /// The test-description initial intermediate flow (gpm at 582 F).
    pub fn initial_intermediate_flow_gpm(self) -> f64 {
        INITIAL_INTERMEDIATE_FLOW_GPM_582F[self.index()]
    }
}

/// Sodium density at a calibration temperature, in kg/m3 (CoolProp
/// `LiqNa`), for printing and for checks.
pub fn calibration_density_kg_per_m3(
    calibration_temperature: ThermodynamicTemperature,
) -> Result<f64, tuas_boussinesq_solver::tuas_lib_error::TuasLibError> {
    Ok(super::model::sodium_density(calibration_temperature)?.get::<kilogram_per_cubic_meter>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::thermodynamic_temperature::kelvin;

    fn s(x: f64) -> Time {
        Time::new::<second>(x)
    }

    #[test]
    fn every_table_is_in_increasing_time() {
        fn increasing<const M: usize>(t: &[[f64; M]]) -> bool {
            t.windows(2).all(|w| w[1][0] > w[0][0])
        }
        assert!(increasing(&SHRT17_PUMP_SPEEDS));
        assert!(increasing(&SHRT17_POWER));
        assert!(increasing(&SHRT17_INTERMEDIATE_FLOW));
        assert!(increasing(&SHRT17_INTERMEDIATE_INLET_TEMPERATURE));
        assert!(increasing(&SHRT45R_PUMP_SPEEDS));
        assert!(increasing(&SHRT45R_AUX_EM_PUMP));
        assert!(increasing(&SHRT45R_INTERMEDIATE_FLOW));
        assert!(increasing(&SHRT45R_INTERMEDIATE_INLET_TEMPERATURE));
    }

    #[test]
    fn interpolation_hits_nodes_midpoints_and_holds_the_ends() {
        let t17 = ShrtTest::Shrt17;
        let (p1, p2) = t17.pump_speeds(s(0.0));
        assert_eq!(p1.get::<revolution_per_minute>(), 799.03);
        assert!((p2.get::<revolution_per_minute>() - 764.92).abs() < 1e-9);
        let (p1, _) = t17.pump_speeds(s(1.0));
        assert!((p1.get::<revolution_per_minute>() - 0.5 * (799.03 + 727.77)).abs() < 1e-9);
        let (p1, _) = t17.pump_speeds(s(-5.0));
        assert_eq!(p1.get::<revolution_per_minute>(), 799.03);
        let (p1, _) = t17.pump_speeds(s(5000.0));
        assert!((p1.get::<revolution_per_minute>() - 0.92).abs() < 1e-9);
        let (f, d) = t17.tabulated_power(s(1.0)).unwrap();
        assert!((f.get::<megawatt>() - 8.24).abs() < 1e-12);
        assert!((d.get::<megawatt>() - 3.12).abs() < 1e-12);
        assert!(ShrtTest::Shrt45r.tabulated_power(s(1.0)).is_none());
        assert_eq!(t17.aux_em_pump_current(s(10.0)).get::<ampere>(), 0.0);
        let i = ShrtTest::Shrt45r.aux_em_pump_current(s(0.0)).get::<ampere>();
        assert!((i - 5797.14).abs() < 1e-9);
        let t = t17.intermediate_inlet_temperature(s(0.0));
        assert!((t.get::<degree_fahrenheit>() - 574.02).abs() < 1e-9);
        assert!((ShrtTest::Shrt45r.intermediate_flow_gpm(s(0.0)) - 5079.5).abs() < 1e-12);
        assert_eq!(t17.end_time().get::<second>(), 900.0);
        assert_eq!(t17.name(), "SHRT-17");
        assert_eq!(ShrtTest::Shrt45r.name(), "SHRT-45R");
        let (p1, _) = ShrtTest::Shrt45r.pump_speeds(s(95.0));
        assert!((p1.get::<revolution_per_minute>() - 10.39).abs() < 1e-9);
        let t = ShrtTest::Shrt45r.intermediate_inlet_temperature(s(900.0));
        assert!((t.get::<degree_fahrenheit>() - 527.9).abs() < 1e-9);
    }

    /// gpm -> kg/s at the calibration temperatures, against a hand value.
    ///
    /// Prediction: sodium at 800 F (699.8 K) is about 850 kg/m3, so 8500 gpm
    /// (0.5363 m3/s) is about 455 kg/s; the bound is +-2 %. Result
    /// (2026-11-10): rho(800 F) = 846.72 kg/m3, so 454.1 kg/s;
    /// rho(582 F) = 877.37 kg/m3.
    #[test]
    fn gpm_conversion_uses_the_calibration_density() {
        let rho800 = calibration_density_kg_per_m3(primary_flow_calibration_temperature()).unwrap();
        let rho582 =
            calibration_density_kg_per_m3(intermediate_flow_calibration_temperature()).unwrap();
        let w17 = ShrtTest::Shrt17.initial_primary_flow().unwrap().get::<kilogram_per_second>();
        let wi = ShrtTest::Shrt17
            .intermediate_flow(s(0.0))
            .unwrap()
            .get::<kilogram_per_second>();
        println!(
            "rho(800 F) = {rho800:.2}, rho(582 F) = {rho582:.2} kg/m3; SHRT-17 primary {w17:.1} kg/s, intermediate {wi:.1} kg/s (initial {} gpm)",
            ShrtTest::Shrt17.initial_intermediate_flow_gpm()
        );
        assert!((w17 / 455.0 - 1.0).abs() < 0.02);
        let hand = rho582 * 5619.55 * 3.785_411_784e-3 / 60.0;
        assert!((wi - hand).abs() < 1e-9 * hand);
        assert!(ShrtTest::Shrt45r.initial_core_inlet_temperature().get::<kelvin>() > 600.0);
        assert!((ShrtTest::Shrt45r.initial_power().get::<megawatt>() - 60.0).abs() < 1e-12);
    }
}
