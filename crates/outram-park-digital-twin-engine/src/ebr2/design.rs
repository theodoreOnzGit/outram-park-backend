//! # EBR-II design data: subassemblies, fuel, core, primary system, pumps
//!
//! Every constant names its document, table and printed page. Keys:
//!
//! - **A226**: Sumner & Wei (2012), ANL-ARC-226 Rev. 1 (PDF page = printed
//!   page + 16);
//! - **A228**: Fei, Mohamed & Kim (2013), ANL-ARC-228 Rev. 1 (PDF page =
//!   printed page).
//!
//! Constants marked **(CSV)** are transcribed in `reference-data/ebr2/` and
//! pinned to those files by `ebr2::csv_equality`. Constants marked
//! **(gh:#1313)** or **(gh:#1315)** were transcribed from the same reports
//! into those issues; no CSV holds them yet, so no test can pin them. All of
//! it is **AI transcription, not yet reviewed by a human** (2026-11-10).
//!
//! Raw constants keep the units the source prints. The `fn`s return uom
//! quantities, and they are what the model uses.

use uom::si::f64::*;
use uom::si::length::{centimeter, meter, millimeter};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::moment_of_inertia::kilogram_square_meter;
use uom::si::power::megawatt;
use uom::si::pressure::pascal;
use uom::si::ratio::percent;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::torque::newton_meter;
use uom::si::volume::{cubic_meter, liter};

// ---------------------------------------------------------------------------
// Fuel elements and subassemblies (A228 Table 3.1.1 p.17, Table 3.1.2 p.18,
// Table 3.3 pp.22-24)
// ---------------------------------------------------------------------------

/// Elements (pins) per MARK-II / MARK-IIA driver subassembly, 91.
/// A228 Table 3.1.1 p.17 **(CSV)**; A228 Table 3.3 p.23 agrees.
pub const DRIVER_PINS_PER_SUBASSEMBLY: u32 = 91;

/// Pins per control / safety subassembly (MARK-II AI), 61.
/// A228 Table 3.3 p.23 **(CSV)**.
pub const CONTROL_SAFETY_PINS_PER_SUBASSEMBLY: u32 = 61;

/// Driver fuel enrichment, 67 wt % U-235. A228 Table 3.1.1 p.17 **(CSV)**.
pub const DRIVER_ENRICHMENT_WT_PERCENT: f64 = 67.0;

/// Driver fuel-slug length at 0 at.% burnup, 0.3429 m.
/// A228 Table 3.1.1 p.17 **(CSV)**; Table 3.3 p.23 gives 34.29 cm.
pub const FUEL_SLUG_LENGTH_M: f64 = 0.3429;

/// Driver fuel-slug diameter, 3.3655 mm. A228 Table 3.1.1 p.17 **(CSV)**.
/// (Table 3.3 p.23 gives 0.3366 cm, the same value to its precision.)
pub const FUEL_SLUG_DIAMETER_MM: f64 = 3.3655;

/// Driver cladding outer diameter, 0.4420 cm. A228 Table 3.3 p.23 **(CSV)**.
///
/// **Source discrepancy.** Table 3.1.1 p.17 prints 4.4196 mm and a wall of
/// 0.305 mm; the `reference-data/ebr2` README records a 0.305 vs 0.315 mm
/// wall disagreement between the reports. This module takes **Table 3.3's
/// OD and ID** (0.4420 / 0.3810 cm, wall 0.305 mm): it is the table that
/// also gives the volume fractions used below, so the two are consistent.
pub const CLADDING_OUTER_DIAMETER_CM: f64 = 0.4420;

/// Driver cladding inner diameter, 0.3810 cm. A228 Table 3.3 p.23 **(CSV)**.
pub const CLADDING_INNER_DIAMETER_CM: f64 = 0.3810;

/// Driver spacer-wire diameter, 0.1245 cm. A228 Table 3.3 p.23 **(CSV)**.
/// (Table 3.1.1 p.17 prints 1.24 mm; Table 3.3's extra digit is used.)
pub const WIRE_WRAP_DIAMETER_CM: f64 = 0.1245;

/// Subassembly lattice pitch, 58.929 mm. A228 Table 3.1.2 p.18 **(CSV)**.
pub const SUBASSEMBLY_PITCH_MM: f64 = 58.929;

/// Outer hex duct, flat-to-flat outside, 58.166 mm. A228 Table 3.1.2 p.18
/// **(CSV)**.
pub const HEX_DUCT_FLAT_OUTSIDE_MM: f64 = 58.166;

/// Outer hex duct, flat-to-flat inside, 56.134 mm. A228 Table 3.1.2 p.18
/// **(CSV)**.
pub const HEX_DUCT_FLAT_INSIDE_MM: f64 = 56.134;

/// Driver subassembly volume fractions at 20 C, `[fuel alloy, 316 SS,
/// sodium]` in %: 26.92, 22.29, 50.79. A228 Table 3.3 p.23 **(CSV)**.
///
/// Check (in the tests): 91 slugs of 3.3655 mm in a 58.929 mm hex cell give
/// a fuel fraction of 26.918 %, so these are fractions of the **lattice
/// cell** (pitch hexagon), not of the duct interior.
pub const DRIVER_VOLUME_FRACTIONS_PERCENT: [f64; 3] = [26.92, 22.29, 50.79];

/// Run 138B core-driver subassemblies, 87 (Table 3.3 p.24 **(CSV)**: 83
/// standard drivers plus the fuelled experiments X402A, X412, C2776A and
/// XX09, gh:#1313).
///
/// **Approximation used by the model:** all 87 are counted as full 91-pin
/// drivers. The 13 half-worth drivers carry fewer fuelled pins (number not
/// transcribed), and the 8 control and 2 safety rods carry 61 fuelled pins
/// each, partly in the core; neither correction is applied. Run 129C
/// (SHRT-17) has its own count, which is not transcribed as a number; the
/// Run 138B count is used for both tests.
pub const RUN_138B_CORE_DRIVERS: u32 = 87;

/// Run 138B subassembly counts, A228 Table 3.3 p.24 **(CSV)**:
/// `[core drivers, control rods, safety rods, experimental, reflector,
/// blanket, total]`. The listed counts sum to 633, not 637: see the CSV
/// header and gh:#1313.
pub const RUN_138B_SUBASSEMBLY_COUNTS: [u32; 7] = [87, 8, 2, 5, 201, 330, 637];

/// Core equivalent diameter, 35.10 cm. A228 Table 3.3 p.22 **(CSV)**.
pub const CORE_EQUIVALENT_DIAMETER_CM: f64 = 35.10;

/// Core height at 0 and 4.5 at.% burnup, 34.29 and 36.65 cm.
/// A228 Table 3.3 p.22 **(CSV)**.
pub const CORE_HEIGHT_CM: [f64; 2] = [34.29, 36.65];

/// U-5Fs alloy composition, wt %: `[U, Zr, Mo, Ru, Rh, Pd, Nb]` =
/// 95.0, 0.10, 2.46, 1.96, 0.28, 0.19, 0.01. A228 Table 3.2.2 p.21 **(CSV)**.
pub const U5FS_COMPOSITION_WT_PERCENT: [f64; 7] = [95.0, 0.10, 2.46, 1.96, 0.28, 0.19, 0.01];

/// Element names in the order of [`U5FS_COMPOSITION_WT_PERCENT`].
pub const U5FS_COMPOSITION_ELEMENTS: [&str; 7] = [
    "Uranium",
    "Zirconium",
    "Molybdenum",
    "Ruthenium",
    "Rhodium",
    "Palladium",
    "Niobium",
];

// ---------------------------------------------------------------------------
// Plant design and operating data (A228 Table 3.3 p.22; A226 p.3)
// ---------------------------------------------------------------------------

/// Heat output, design / operating: 62.5 / 60.0 MW. A228 Table 3.3 p.22
/// **(CSV)**.
pub const HEAT_OUTPUT_MW: [f64; 2] = [62.5, 60.0];

/// Primary sodium temperature to / from the reactor, 343 / 443 C.
/// A228 Table 3.3 p.22 **(CSV)**. Run 138B design data, not a SHRT
/// initial condition.
pub const PRIMARY_INLET_OUTLET_C: [f64; 2] = [343.0, 443.0];

/// Average core temperatures, `[fuel, steel, coolant]` C: 487, 412, 393.
/// A228 Table 3.3 p.22 **(CSV)**.
pub const AVERAGE_CORE_TEMPERATURES_C: [f64; 3] = [487.0, 412.0, 393.0];

/// Primary sodium flow rate, 566 L/s. A228 Table 3.3 p.22 **(CSV)**.
pub const PRIMARY_FLOW_L_PER_S: f64 = 566.0;

/// Primary system sodium capacity, 340 000 L. A228 Table 3.3 p.22 **(CSV)**.
/// A226 p.3 gives "approximately 340 m3 at 371C" **(CSV)**, which fixes the
/// temperature at which the volume is quoted ([`PRIMARY_SODIUM_REFERENCE_C`]).
pub const PRIMARY_SODIUM_CAPACITY_L: f64 = 340_000.0;

/// The temperature at which [`PRIMARY_SODIUM_CAPACITY_L`] is quoted, 371 C.
/// A226 p.3 **(CSV)**.
pub const PRIMARY_SODIUM_REFERENCE_C: f64 = 371.0;

/// Secondary sodium to the heat exchanger, 289 C, at 345 L/s.
/// A228 Table 3.3 p.22 **(CSV)**: `[temperature C, flow L/s]`.
pub const SECONDARY_DESIGN: [f64; 2] = [289.0, 345.0];

/// Share of primary flow through the high-pressure plenum (rows 1-7),
/// "approximately 85 %". A226 p.3 **(CSV)**.
pub const HIGH_PRESSURE_PLENUM_FLOW_PERCENT: f64 = 85.0;

/// Rated mass flows, `[primary, intermediate, steam]` kg/s: 485, 315, 32
/// ("typical operation"). A226 p.3 **(CSV)**.
pub const RATED_MASS_FLOWS_KG_PER_S: [f64; 3] = [485.0, 315.0, 32.0];

// ---------------------------------------------------------------------------
// IHX geometry (A226 Table 5.2.7.1 p.128, transcribed in gh:#1315)
// ---------------------------------------------------------------------------

/// IHX tube count, 3026. A226 Table 5.2.7.1 p.128 **(gh:#1315)**.
pub const IHX_TUBE_COUNT: u32 = 3026;
/// IHX tube outer diameter, 1.588 cm. A226 Table 5.2.7.1 p.128 **(gh:#1315)**.
pub const IHX_TUBE_OD_CM: f64 = 1.588;
/// IHX tube wall, 0.124 cm. A226 Table 5.2.7.1 p.128 **(gh:#1315)**.
pub const IHX_TUBE_WALL_CM: f64 = 0.124;
/// IHX tube pitch, 2.064 cm. A226 Table 5.2.7.1 p.128 **(gh:#1315)**.
pub const IHX_TUBE_PITCH_CM: f64 = 2.064;
/// IHX tube length, 3.137 m. A226 Table 5.2.7.1 p.128 **(gh:#1315)**.
pub const IHX_TUBE_LENGTH_M: f64 = 3.137;
/// IHX well casing inner diameter, 1.295 m. A226 Table 5.2.7.1 p.128
/// **(gh:#1315)**.
pub const IHX_CASING_ID_M: f64 = 1.295;

// ---------------------------------------------------------------------------
// Primary pumps: NATDEMO model (A226 Eqs 3.1-3.7 pp.15-16, Table 3.1.4.1.2
// p.17)
// ---------------------------------------------------------------------------

/// NATDEMO homologous head coefficients, A226 Eq 3.7 p.16 **(CSV)**:
/// `[b1, b2, b3m, b3t, b3l, b4t, b4l, wt, eps_m]`
/// = 1.1740, 0.0818, -0.2558, -0.5923, -0.0471, 1.9, 1.0, 0.06, 0.55.
///
/// Check (in the tests): `b1 + b2 + b3m = 1.0000`, so the curve passes
/// through the rated point `H = H_r` at `s = w = 1`.
pub const PUMP_HEAD_COEFFICIENTS: [f64; 9] =
    [1.1740, 0.0818, -0.2558, -0.5923, -0.0471, 1.9, 1.0, 0.06, 0.55];

/// Rated head `H_r`, 358 530 Pa. A226 Eq 3.7 p.16 **(CSV)**.
pub const PUMP_RATED_HEAD_PA: f64 = 358_530.0;
/// Rated flow `w_r`, 250.2 kg/s (per pump). A226 Eq 3.7 p.16 **(CSV)**.
pub const PUMP_RATED_FLOW_KG_PER_S: f64 = 250.2;
/// Rated speed `s_r`, 870 rpm. A226 Eq 3.7 p.16 and Table 3.1.4.1.2 p.17
/// **(CSV)**.
pub const PUMP_RATED_SPEED_RPM: f64 = 870.0;
/// Pump moment of inertia, 16.0 kg m2. A226 Table 3.1.4.1.2 p.17 **(CSV)**.
pub const PUMP_INERTIA_KG_M2: f64 = 16.0;
/// Rated torque, 1300.0 N m. A226 Table 3.1.4.1.2 p.17 **(CSV)**.
pub const PUMP_RATED_TORQUE_N_M: f64 = 1300.0;
/// Friction torque, 37.2 N m (0.0286 of rated). A226 Table 3.1.4.1.2 p.17
/// **(CSV)**.
pub const PUMP_FRICTION_TORQUE_N_M: f64 = 37.2;

/// Auxiliary EM pump rating: 0.25 psi at 500 gpm (900 F sodium), 1.4 V
/// rectifier voltage, 9000 A. A226 p.30 **(CSV, text)**.
/// `[pressure psi, flow gpm, sodium F, rectifier V, current A]`.
pub const AUX_EM_PUMP_RATING: [f64; 5] = [0.25, 500.0, 900.0, 1.4, 9000.0];

// ---------------------------------------------------------------------------
// U-5Fs fuel properties (A226 Section 4.2.1 pp.100-102, transcribed in
// gh:#1313)
// ---------------------------------------------------------------------------

/// U-5Fs reference density, 18.2e3 kg/m3 at 300 K. A226 p.101
/// **(gh:#1313)**.
pub const U5FS_DENSITY_KG_PER_M3: f64 = 18.2e3;

/// U-5Fs heat capacity `c(T) = a + b T + c T^2` kJ/(kg K), T in K,
/// A226 Eq 4.2 p.101 **(gh:#1313)**, rows `[T_lo K, T_hi K, a, b, c]`.
///
/// **Flagged in the source transcription:** the text layer is garbled here
/// and gh:#1313 asks for the form to be checked on the rendered page. Two
/// further observations, made while writing this module and **not
/// resolved**: the first branch gives 0.342 kJ/(kg K) at 833 K while the
/// second gives 0.246 at the same temperature (not continuous), and the
/// first branch's 0.307 at 760 K is about twice the heat capacity of
/// alpha-uranium. Used as transcribed; the fuel node's heat capacity and so
/// its time constant inherit this doubt.
pub const U5FS_HEAT_CAPACITY: [[f64; 5]; 3] = [
    [273.0, 833.0, 0.13964, -1.9785e-5, 3.1566e-7],
    [833.0, 913.0, 0.46364, -2.615e-4, 0.0],
    [913.0, 1283.0, 2.40126, -3.5126e-3, 1.4012e-6],
];

/// Fully dense U-5Fs thermal conductivity below the solidus,
/// `k = 14.1 + 2.98e-2 T + 3.01e-6 T^2` W/(m C), T in C. A226 Eq 4.5 p.101
/// **(gh:#1313)**: `[14.1, 2.98e-2, 3.01e-6]`.
///
/// The porosity correction (Eq 4.6 p.102) is garbled in the text layer and
/// is **not applied**: the model's fuel is fully dense, which overstates the
/// conductivity of the burned, sodium-logged SHRT fuel.
pub const U5FS_CONDUCTIVITY: [f64; 3] = [14.1, 2.98e-2, 3.01e-6];

// ---------------------------------------------------------------------------
// Typed accessors
// ---------------------------------------------------------------------------

/// [`FUEL_SLUG_LENGTH_M`] as a [`Length`].
pub fn fuel_slug_length() -> Length {
    Length::new::<meter>(FUEL_SLUG_LENGTH_M)
}

/// [`FUEL_SLUG_DIAMETER_MM`] as a [`Length`].
pub fn fuel_slug_diameter() -> Length {
    Length::new::<millimeter>(FUEL_SLUG_DIAMETER_MM)
}

/// [`CLADDING_OUTER_DIAMETER_CM`] as a [`Length`].
pub fn cladding_outer_diameter() -> Length {
    Length::new::<centimeter>(CLADDING_OUTER_DIAMETER_CM)
}

/// [`CLADDING_INNER_DIAMETER_CM`] as a [`Length`].
pub fn cladding_inner_diameter() -> Length {
    Length::new::<centimeter>(CLADDING_INNER_DIAMETER_CM)
}

/// [`WIRE_WRAP_DIAMETER_CM`] as a [`Length`].
pub fn wire_wrap_diameter() -> Length {
    Length::new::<centimeter>(WIRE_WRAP_DIAMETER_CM)
}

/// [`SUBASSEMBLY_PITCH_MM`] as a [`Length`].
pub fn subassembly_pitch() -> Length {
    Length::new::<millimeter>(SUBASSEMBLY_PITCH_MM)
}

/// [`HEX_DUCT_FLAT_INSIDE_MM`] as a [`Length`].
pub fn hex_duct_flat_inside() -> Length {
    Length::new::<millimeter>(HEX_DUCT_FLAT_INSIDE_MM)
}

/// The driver volume fractions `[fuel, steel, sodium]` as [`Ratio`]s.
pub fn driver_volume_fractions() -> [Ratio; 3] {
    DRIVER_VOLUME_FRACTIONS_PERCENT.map(Ratio::new::<percent>)
}

/// Run 138B operating heat output, 60.0 MW.
pub fn operating_power() -> Power {
    Power::new::<megawatt>(HEAT_OUTPUT_MW[1])
}

/// `[fuel, steel, coolant]` average core temperatures (A228 Table 3.3).
pub fn average_core_temperatures() -> [ThermodynamicTemperature; 3] {
    AVERAGE_CORE_TEMPERATURES_C.map(ThermodynamicTemperature::new::<degree_celsius>)
}

/// `[inlet, outlet]` design primary temperatures (A228 Table 3.3).
pub fn primary_inlet_outlet() -> [ThermodynamicTemperature; 2] {
    PRIMARY_INLET_OUTLET_C.map(ThermodynamicTemperature::new::<degree_celsius>)
}

/// [`PRIMARY_SODIUM_CAPACITY_L`] as a [`Volume`].
pub fn primary_sodium_capacity() -> Volume {
    Volume::new::<liter>(PRIMARY_SODIUM_CAPACITY_L)
}

/// [`PRIMARY_SODIUM_REFERENCE_C`] as a temperature.
pub fn primary_sodium_reference_temperature() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(PRIMARY_SODIUM_REFERENCE_C)
}

/// [`HIGH_PRESSURE_PLENUM_FLOW_PERCENT`] as a [`Ratio`].
pub fn high_pressure_plenum_flow_share() -> Ratio {
    Ratio::new::<percent>(HIGH_PRESSURE_PLENUM_FLOW_PERCENT)
}

/// IHX tube outer diameter.
pub fn ihx_tube_outer_diameter() -> Length {
    Length::new::<centimeter>(IHX_TUBE_OD_CM)
}

/// IHX tube inner diameter, `OD - 2 wall` (derived).
pub fn ihx_tube_inner_diameter() -> Length {
    Length::new::<centimeter>(IHX_TUBE_OD_CM - 2.0 * IHX_TUBE_WALL_CM)
}

/// IHX tube pitch.
pub fn ihx_tube_pitch() -> Length {
    Length::new::<centimeter>(IHX_TUBE_PITCH_CM)
}

/// IHX tube length.
pub fn ihx_tube_length() -> Length {
    Length::new::<meter>(IHX_TUBE_LENGTH_M)
}

/// IHX casing inner diameter.
pub fn ihx_casing_inner_diameter() -> Length {
    Length::new::<meter>(IHX_CASING_ID_M)
}

/// Pump rated head [`PUMP_RATED_HEAD_PA`].
pub fn pump_rated_head() -> Pressure {
    Pressure::new::<pascal>(PUMP_RATED_HEAD_PA)
}

/// Pump rated flow [`PUMP_RATED_FLOW_KG_PER_S`] (per pump).
pub fn pump_rated_flow() -> MassRate {
    MassRate::new::<kilogram_per_second>(PUMP_RATED_FLOW_KG_PER_S)
}

/// Pump inertia, rated torque and friction torque (A226 Table 3.1.4.1.2),
/// for a caller that drives the pumps with
/// `tampines::components::pump_coastdown::InertialPumpCoastdown` instead of
/// the tabulated speeds.
pub fn pump_inertia_and_torques() -> (MomentOfInertia, Torque, Torque) {
    (
        MomentOfInertia::new::<kilogram_square_meter>(PUMP_INERTIA_KG_M2),
        Torque::new::<newton_meter>(PUMP_RATED_TORQUE_N_M),
        Torque::new::<newton_meter>(PUMP_FRICTION_TORQUE_N_M),
    )
}

/// U-5Fs heat capacity from [`U5FS_HEAT_CAPACITY`], in J/(kg K). Below
/// 273 K the first branch and above 1283 K (the solidus) the last branch is
/// extrapolated; the model keeps the fuel far inside that range.
pub fn u5fs_heat_capacity(temperature: ThermodynamicTemperature) -> SpecificHeatCapacity {
    let t = temperature.get::<kelvin>();
    let row = U5FS_HEAT_CAPACITY
        .iter()
        .find(|r| t < r[1])
        .unwrap_or(&U5FS_HEAT_CAPACITY[2]);
    let kj = row[2] + row[3] * t + row[4] * t * t;
    SpecificHeatCapacity::new::<uom::si::specific_heat_capacity::joule_per_kilogram_kelvin>(
        kj * 1.0e3,
    )
}

/// Fully dense U-5Fs conductivity from [`U5FS_CONDUCTIVITY`] (no porosity
/// correction; see there).
pub fn u5fs_conductivity(temperature: ThermodynamicTemperature) -> ThermalConductivity {
    let t_c = temperature.get::<degree_celsius>();
    let [a, b, c] = U5FS_CONDUCTIVITY;
    ThermalConductivity::new::<uom::si::thermal_conductivity::watt_per_meter_kelvin>(
        a + b * t_c + c * t_c * t_c,
    )
}

/// [`U5FS_DENSITY_KG_PER_M3`] as a [`MassDensity`].
pub fn u5fs_density() -> MassDensity {
    MassDensity::new::<kilogram_per_cubic_meter>(U5FS_DENSITY_KG_PER_M3)
}

/// The hexagonal lattice-cell area, `(sqrt 3 / 2) p^2` (derived).
pub fn lattice_cell_area() -> Area {
    let p = subassembly_pitch();
    0.5 * 3.0_f64.sqrt() * p * p
}

/// The fuelled core volume of `drivers` subassemblies: lattice-cell area x
/// fuel-slug length (derived).
pub fn fuelled_core_volume(drivers: u32) -> Volume {
    f64::from(drivers) * lattice_cell_area() * fuel_slug_length()
}

/// Primary sodium inventory as a volume in m3 at the reference temperature
/// (a convenience for printing; derived).
pub fn primary_sodium_capacity_m3() -> f64 {
    primary_sodium_capacity().get::<cubic_meter>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::area::square_centimeter;

    /// The Table 3.3 fuel volume fraction is a fraction of the lattice cell:
    /// 91 slugs of 3.3655 mm in a 58.929 mm hex cell.
    ///
    /// Prediction before running: 26.9 %. Result (2026-11-10): 26.918 %
    /// against the printed 26.92 % (0.007 % relative); the cell is
    /// 30.074 cm2.
    #[test]
    fn fuel_volume_fraction_is_of_the_lattice_cell() {
        let slug = fuel_slug_diameter();
        let slug_area = std::f64::consts::FRAC_PI_4 * slug * slug;
        let fraction = f64::from(DRIVER_PINS_PER_SUBASSEMBLY) * slug_area / lattice_cell_area();
        let printed = driver_volume_fractions()[0];
        println!(
            "fuel fraction {:.4} % vs printed {:.2} %; cell {:.3} cm2",
            fraction.get::<percent>(),
            printed.get::<percent>(),
            lattice_cell_area().get::<square_centimeter>()
        );
        assert!((fraction.get::<percent>() - 26.92).abs() < 0.02);
    }

    /// The NATDEMO curve passes through the rated point:
    /// `b1 + b2 + b3m = 1.1740 + 0.0818 - 0.2558 = 1.0000`.
    #[test]
    fn natdemo_coefficients_pass_through_the_rated_point() {
        let [b1, b2, b3m, ..] = PUMP_HEAD_COEFFICIENTS;
        assert!((b1 + b2 + b3m - 1.0).abs() < 1e-12);
    }

    /// The accessors return the transcribed numbers in SI.
    #[test]
    fn accessors_convert_units() {
        assert!((cladding_outer_diameter().get::<millimeter>() - 4.420).abs() < 1e-12);
        assert!((cladding_inner_diameter().get::<millimeter>() - 3.810).abs() < 1e-12);
        assert!((wire_wrap_diameter().get::<millimeter>() - 1.245).abs() < 1e-12);
        assert!((hex_duct_flat_inside().get::<millimeter>() - 56.134).abs() < 1e-12);
        assert!((operating_power().get::<megawatt>() - 60.0).abs() < 1e-12);
        assert!((average_core_temperatures()[0].get::<degree_celsius>() - 487.0).abs() < 1e-9);
        assert!((primary_inlet_outlet()[1].get::<degree_celsius>() - 443.0).abs() < 1e-9);
        assert!((primary_sodium_capacity_m3() - 340.0).abs() < 1e-9);
        assert!((primary_sodium_reference_temperature().get::<kelvin>() - 644.15).abs() < 1e-9);
        assert!((high_pressure_plenum_flow_share().get::<percent>() - 85.0).abs() < 1e-12);
        assert!((ihx_tube_inner_diameter().get::<millimeter>() - 13.40).abs() < 1e-9);
        assert!((ihx_tube_outer_diameter().get::<millimeter>() - 15.88).abs() < 1e-9);
        assert!((ihx_tube_pitch().get::<millimeter>() - 20.64).abs() < 1e-9);
        assert!((ihx_tube_length().get::<meter>() - 3.137).abs() < 1e-12);
        assert!((ihx_casing_inner_diameter().get::<meter>() - 1.295).abs() < 1e-12);
        assert!((pump_rated_head().get::<pascal>() - 358_530.0).abs() < 1e-9);
        assert!((pump_rated_flow().get::<kilogram_per_second>() - 250.2).abs() < 1e-12);
        let (i, tr, tf) = pump_inertia_and_torques();
        assert_eq!(i.get::<kilogram_square_meter>(), 16.0);
        assert_eq!(tr.get::<newton_meter>(), 1300.0);
        assert_eq!(tf.get::<newton_meter>(), 37.2);
        assert_eq!(u5fs_density().get::<kilogram_per_cubic_meter>(), 18.2e3);
        // 87 cells x 30.07 cm2 x 34.29 cm
        let v = fuelled_core_volume(RUN_138B_CORE_DRIVERS).get::<cubic_meter>();
        assert!((v - 87.0 * 30.0738e-4 * 0.3429).abs() < 1e-6, "{v}");
    }

    /// The U-5Fs correlations evaluate as transcribed (hand values), and the
    /// heat capacity's discontinuity at 833 K is reported, not hidden.
    #[test]
    fn u5fs_properties_evaluate_as_transcribed() {
        let t760 = ThermodynamicTemperature::new::<kelvin>(760.0);
        let cp = u5fs_heat_capacity(t760).value;
        let hand = (0.13964 - 1.9785e-5 * 760.0 + 3.1566e-7 * 760.0 * 760.0) * 1e3;
        assert!((cp - hand).abs() < 1e-9);
        let below = u5fs_heat_capacity(ThermodynamicTemperature::new::<kelvin>(832.999)).value;
        let above = u5fs_heat_capacity(ThermodynamicTemperature::new::<kelvin>(833.0)).value;
        println!("U-5Fs cp at 833 K: first branch {below:.1}, second branch {above:.1} J/(kg K)");
        assert!(below > above, "the transcribed branches are discontinuous at 833 K");
        let hot = u5fs_heat_capacity(ThermodynamicTemperature::new::<kelvin>(1000.0)).value;
        // 2.40126 - 3.5126 + 1.4012 = 0.28986 kJ/(kg K) (gh:#1313's "0.29")
        assert!((hot - 289.86).abs() < 1e-6, "{hot}");
        let k = u5fs_conductivity(ThermodynamicTemperature::new::<degree_celsius>(487.0)).value;
        assert!((k - (14.1 + 2.98e-2 * 487.0 + 3.01e-6 * 487.0 * 487.0)).abs() < 1e-9);
    }
}
