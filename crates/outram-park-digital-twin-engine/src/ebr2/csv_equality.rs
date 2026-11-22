//! The Rust tables equal the transcriptions in `reference-data/ebr2/`.
//!
//! `design`, `shrt` and `kinetics` were generated from those CSVs on
//! 2026-11-10, and are compiled in because wasm has no filesystem. These
//! tests re-read the CSVs at test time and assert equality, so that neither
//! copy can drift from the other.
//!
//! - Tables: every cell, exactly.
//! - Scalars: every constant that came from a CSV, matched by its row key.
//!
//! Values that came from the gh:#1313/#1314/#1315 transcriptions have no CSV,
//! so no test here covers them; their modules say so.

use std::path::PathBuf;

use super::design::*;
use super::kinetics::DOPPLER_NOMINAL_FUEL_TEMPERATURE_C;
use super::shrt::*;

fn reference_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/ebr2")
}

/// Every record of `file`, `#` comment lines and the header skipped.
fn records(file: &str) -> Vec<Vec<String>> {
    let path = reference_dir().join(file);
    let mut reader = csv::ReaderBuilder::new()
        .comment(Some(b'#'))
        .has_headers(true)
        .from_path(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    reader
        .records()
        .map(|r| r.unwrap().iter().map(|s| s.trim().to_string()).collect())
        .collect()
}

/// The numbers in a printed cell, in order ("0.0286 TR (37.2)" -> 0.0286,
/// 37.2). Digit groups separated by a single space ("340 000") are joined.
fn numbers(cell: &str) -> Vec<f64> {
    let joined: String = {
        let chars: Vec<char> = cell.chars().collect();
        let mut out = String::new();
        for (i, c) in chars.iter().enumerate() {
            let digit_space_digit = *c == ' '
                && i > 0
                && i + 1 < chars.len()
                && chars[i - 1].is_ascii_digit()
                && chars[i + 1].is_ascii_digit();
            if !digit_space_digit {
                out.push(*c);
            }
        }
        out
    };
    joined
        .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .filter_map(|t| t.parse::<f64>().ok())
        .collect()
}

/// The record whose column `key_col` equals each of `keys`, in order.
fn row(records: &[Vec<String>], keys: &[(usize, &str)]) -> Vec<String> {
    records
        .iter()
        .find(|r| keys.iter().all(|(c, k)| r[*c] == *k))
        .unwrap_or_else(|| panic!("no row with {keys:?}"))
        .clone()
}

fn assert_table<const M: usize>(file: &str, table: &[[f64; M]]) {
    let recs = records(file);
    assert_eq!(recs.len(), table.len(), "{file}: row count");
    for (r, rust) in recs.iter().zip(table) {
        assert_eq!(r.len(), M, "{file}: column count");
        for (cell, value) in r.iter().zip(rust) {
            let parsed: f64 = cell.parse().unwrap_or_else(|_| panic!("{file}: '{cell}'"));
            assert_eq!(parsed, *value, "{file}: {r:?}");
        }
    }
}

#[test]
fn boundary_condition_tables_equal_the_csvs() {
    assert_table("arc226_table3_1_4_1_1.csv", &SHRT17_PUMP_SPEEDS);
    assert_table("arc226_table3_1_4_2_1_power.csv", &SHRT17_POWER);
    assert_table("arc226_table3_1_4_2_1_ihx_flow.csv", &SHRT17_INTERMEDIATE_FLOW);
    assert_table("arc226_table3_1_4_2_2.csv", &SHRT17_INTERMEDIATE_INLET_TEMPERATURE);
    assert_table("arc226_table3_2_4_1_1.csv", &SHRT45R_PUMP_SPEEDS);
    assert_table("arc226_table3_2_4_1_2.csv", &SHRT45R_AUX_EM_PUMP);
    assert_table("arc226_table3_2_4_2_1.csv", &SHRT45R_INTERMEDIATE_FLOW);
    assert_table("arc226_table3_2_4_2_2.csv", &SHRT45R_INTERMEDIATE_INLET_TEMPERATURE);
}

#[test]
fn subassembly_and_fuel_constants_equal_the_csvs() {
    let t311 = records("arc228_table3_1_1.csv");
    let cell = |item: &str, col: usize| numbers(&row(&t311, &[(0, item)])[col]);
    assert_eq!(cell("number_of_elements", 2), vec![f64::from(DRIVER_PINS_PER_SUBASSEMBLY)]);
    assert_eq!(cell("enrichment", 2), vec![DRIVER_ENRICHMENT_WT_PERCENT]);
    assert_eq!(cell("fuel_slug_length", 2), vec![FUEL_SLUG_LENGTH_M]);
    assert_eq!(cell("fuel_slug_diameter", 2), vec![FUEL_SLUG_DIAMETER_MM]);

    let t312 = records("arc228_table3_1_2.csv");
    let cell = |item: &str| numbers(&row(&t312, &[(0, item)])[2]);
    assert_eq!(cell("pitch"), vec![SUBASSEMBLY_PITCH_MM]);
    assert_eq!(cell("outer_hex_flat_to_flat_outside"), vec![HEX_DUCT_FLAT_OUTSIDE_MM]);
    assert_eq!(cell("outer_hex_flat_to_flat_inside"), vec![HEX_DUCT_FLAT_INSIDE_MM]);

    let t322 = records("arc228_table3_2_2.csv");
    assert_eq!(t322.len(), 7);
    for ((r, name), wt) in t322
        .iter()
        .zip(U5FS_COMPOSITION_ELEMENTS)
        .zip(U5FS_COMPOSITION_WT_PERCENT)
    {
        assert_eq!(r[0], name);
        assert_eq!(numbers(&r[1]), vec![wt]);
    }
}

#[test]
fn run_138b_design_data_equal_the_csv() {
    let t = records("arc228_table3_3.csv");
    let v = |section: &str, item: &str| numbers(&row(&t, &[(0, section), (1, item)])[3]);
    assert_eq!(v("driver_20C", "outer_cladding_diameter"), vec![CLADDING_OUTER_DIAMETER_CM]);
    assert_eq!(v("driver_20C", "inner_cladding_diameter"), vec![CLADDING_INNER_DIAMETER_CM]);
    assert_eq!(v("driver_20C", "wire_wrap_diameter"), vec![WIRE_WRAP_DIAMETER_CM]);
    assert_eq!(
        [
            v("driver_20C", "volume_fraction_fuel_alloy")[0],
            v("driver_20C", "volume_fraction_stainless_steel_type_316")[0],
            v("driver_20C", "volume_fraction_sodium")[0],
        ],
        DRIVER_VOLUME_FRACTIONS_PERCENT
    );
    assert_eq!(
        v("control_safety_20C", "number_of_pins_per_assembly"),
        vec![f64::from(CONTROL_SAFETY_PINS_PER_SUBASSEMBLY)]
    );
    let counts: Vec<f64> = [
        "no_core_driver_subassemblies",
        "no_control_rods",
        "no_safety_rods",
        "no_experimental_subassemblies",
        "no_reflector_subassemblies",
        "no_blanket_subassemblies",
        "total_no_subassemblies",
    ]
    .iter()
    .map(|item| v("summary", item)[0])
    .collect();
    assert_eq!(counts, RUN_138B_SUBASSEMBLY_COUNTS.map(f64::from).to_vec());
    assert_eq!(f64::from(RUN_138B_CORE_DRIVERS), counts[0]);
    assert_eq!(v("core", "equivalent_diameter"), vec![CORE_EQUIVALENT_DIAMETER_CM]);
    assert_eq!(v("core", "core_height_0_at%_burnup"), vec![CORE_HEIGHT_CM[0]]);
    assert_eq!(v("core", "core_height_4.5_at%_burnup"), vec![CORE_HEIGHT_CM[1]]);
    assert_eq!(v("general", "heat_output_design/operating"), HEAT_OUTPUT_MW.to_vec());
    assert_eq!(
        [
            v("general", "primary_sodium_temperature_to_reactor")[0],
            v("general", "primary_sodium_temperature_from_reactor")[0]
        ],
        PRIMARY_INLET_OUTLET_C
    );
    assert_eq!(
        [
            v("general", "average_core_temperature_fuel")[0],
            v("general", "average_core_temperature_steel")[0],
            v("general", "average_core_temperature_coolant")[0]
        ],
        AVERAGE_CORE_TEMPERATURES_C
    );
    assert_eq!(v("general", "primary_sodium_flow_rate"), vec![PRIMARY_FLOW_L_PER_S]);
    assert_eq!(v("general", "primary_system_sodium_capacity"), vec![PRIMARY_SODIUM_CAPACITY_L]);
    assert_eq!(
        [
            v("general", "secondary_sodium_temperature_to_heat_exchanger")[0],
            v("general", "secondary_sodium_flow_rate")[0]
        ],
        SECONDARY_DESIGN
    );
}

#[test]
fn pump_model_equals_the_csv() {
    let t = records("arc226_pump_model.csv");
    let v = |symbol: &str| numbers(&row(&t, &[(1, symbol)])[2]);
    let symbols = ["b1", "b2", "b3m", "b3t", "b3l", "b4t", "b4l", "wt", "eps_m"];
    let coefficients: Vec<f64> = symbols.iter().map(|s| v(s)[0]).collect();
    assert_eq!(coefficients, PUMP_HEAD_COEFFICIENTS.to_vec());
    assert_eq!(v("Hr"), vec![PUMP_RATED_HEAD_PA]);
    assert_eq!(v("wr"), vec![PUMP_RATED_FLOW_KG_PER_S]);
    assert_eq!(*v("sr").last().unwrap(), PUMP_RATED_SPEED_RPM);
    assert_eq!(v("NR_rated_speed"), vec![PUMP_RATED_SPEED_RPM]);
    assert_eq!(v("Ip_inertia"), vec![PUMP_INERTIA_KG_M2]);
    assert_eq!(v("TR_rated_torque"), vec![PUMP_RATED_TORQUE_N_M]);
    assert_eq!(v("TfR_friction_torque"), vec![0.0286, PUMP_FRICTION_TORQUE_N_M]);
}

#[test]
fn initial_conditions_equal_the_csvs() {
    let desc = records("arc226_table3_1_1_1_and_3_2_1_1.csv");
    let d = |item: &str, col: usize| numbers(&row(&desc, &[(0, item)])[col])[0];
    assert_eq!([d("initial_power", 1), d("initial_power", 2)], INITIAL_POWER_MW);
    assert_eq!(
        [d("initial_primary_coolant_flow", 1), d("initial_primary_coolant_flow", 2)],
        INITIAL_PRIMARY_FLOW_GPM_800F
    );
    assert_eq!(
        [d("initial_intermediate_coolant_flow", 1), d("initial_intermediate_coolant_flow", 2)],
        INITIAL_INTERMEDIATE_FLOW_GPM_582F
    );
    assert_eq!(
        [d("initial_core_inlet_temperature", 1), d("initial_core_inlet_temperature", 2)],
        INITIAL_CORE_INLET_F
    );

    let text = records("arc226_initial_conditions_text.csv");
    let x = |test: &str, quantity: &str| numbers(&row(&text, &[(0, test), (1, quantity)])[2]);
    assert_eq!(
        [x("SHRT-17", "initial_fission_power")[0], x("SHRT-17", "initial_decay_heat")[0]],
        SHRT17_INITIAL_FISSION_DECAY_MW
    );
    assert_eq!(
        [
            x("SHRT-17", "ihx_intermediate_inlet_temperature")[0],
            x("SHRT-45R", "ihx_intermediate_inlet_temperature")[0]
        ],
        INITIAL_IHX_INTERMEDIATE_INLET_F
    );
    assert_eq!(
        [
            x("SHRT-17", "pump1_discharge_pressure_excluding_gravity_head")[0],
            x("SHRT-17", "pump2_discharge_pressure_excluding_gravity_head")[0],
            x("SHRT-45R", "pump1_discharge_pressure_excluding_gravity_head")[0]
        ],
        PUMP_DISCHARGE_PRESSURE_PSI
    );
    assert_eq!(
        [x("SHRT-17", "upper_plenum_pressure")[0], x("SHRT-45R", "upper_plenum_pressure")[0]],
        UPPER_PLENUM_PRESSURE_PSI
    );
    assert_eq!(
        [
            x("SHRT-45R", "pump1_flow")[0],
            x("SHRT-45R", "pump2_flow")[0],
            x("SHRT-45R", "z_pipe_inlet_flow")[0],
            x("SHRT-45R", "core_flow")[0]
        ],
        SHRT45R_INITIAL_FLOWS_GPM_800F
    );
    assert_eq!(
        [
            x("both", "primary_mass_flow_rated")[0],
            x("both", "intermediate_mass_flow_rated")[0],
            x("both", "steam_mass_flow_rated")[0]
        ],
        RATED_MASS_FLOWS_KG_PER_S
    );
    // "approximately 340 m3 at 371C": the "3" of "m3" is the middle number.
    let tank = x("both", "primary_tank_sodium");
    assert_eq!(
        [tank[0], tank[2]],
        [PRIMARY_SODIUM_CAPACITY_L / 1000.0, PRIMARY_SODIUM_REFERENCE_C]
    );
    assert_eq!(
        x("both", "high_pressure_plenum_flow_share"),
        vec![HIGH_PRESSURE_PLENUM_FLOW_PERCENT]
    );
    assert_eq!(x("SHRT-45R", "aux_em_pump_rating"), AUX_EM_PUMP_RATING.to_vec());
    let history = x("SHRT-45R", "pre_test_history");
    assert_eq!(
        history[1..5],
        [
            SHRT45R_PRE_TEST_HISTORY[0][0],
            SHRT45R_PRE_TEST_HISTORY[0][1],
            SHRT45R_PRE_TEST_HISTORY[1][0],
            SHRT45R_PRE_TEST_HISTORY[1][1]
        ]
    );

    let a228 = records("arc228_text_values.csv");
    let y = |quantity: &str| numbers(&row(&a228, &[(0, quantity)])[1])[0];
    assert_eq!(
        [
            y("run_138B_low_power_level"),
            y("run_138B_low_power_duration"),
            y("run_138B_full_power_level"),
            y("run_138B_full_power_duration")
        ],
        [
            SHRT45R_PRE_TEST_HISTORY[0][0],
            SHRT45R_PRE_TEST_HISTORY[0][1],
            SHRT45R_PRE_TEST_HISTORY[1][0],
            SHRT45R_PRE_TEST_HISTORY[1][1]
        ]
    );
    assert_eq!(
        y("nominal_average_fuel_temperature_for_Doppler_constant"),
        DOPPLER_NOMINAL_FUEL_TEMPERATURE_C
    );
}

#[test]
fn number_extraction_reads_printed_cells() {
    assert_eq!(numbers("0.0286 TR (37.2)"), vec![0.0286, 37.2]);
    assert_eq!(numbers("340 000"), vec![340_000.0]);
    assert_eq!(numbers("62.5/60.0"), vec![62.5, 60.0]);
    assert_eq!(numbers("57.3 MW (ANL-EBR-132)")[0], 57.3);
}
